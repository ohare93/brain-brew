use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use brain_brew_formats::lockfile::{
    self, FederationLock, LOCKFILE_VERSION, LockedPackage, LockedPackageMetadata, LockedSource,
    OriginalSource,
};
use flate2::read::GzDecoder;
use fs2::FileExt as _;
use nix_nar::Encoder;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tempfile::{NamedTempFile, TempDir};

use crate::fetch_policy::{self, FetchPolicy, budget_error};
use crate::help;
use crate::io::read_manifest;
use crate::output;
use crate::package_tree;
use crate::path_authorization::PathAuthorizer;

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        print!("{}", help::command("lock").expect("lock help exists"));
        return Ok(());
    }
    let Some(subcommand) = args.first().map(String::as_str) else {
        return Err(help::usage_error(
            "lock",
            "usage: brainbrew lock <update|verify>",
        ));
    };
    match subcommand {
        "update" => update(&args[1..]),
        "verify" => verify(&args[1..]),
        other => Err(format!("unknown lock subcommand {other:?}")),
    }
}

fn update(args: &[String]) -> Result<(), String> {
    let args = parse_lock_update_args(args)?;
    // Existing locks must pass the current schema before any source is fetched
    // or package manifest is loaded. This also makes v1 migration fail early.
    let mut lock = read_lock_or_empty(&args.lock_path)?;
    let fetch_requested = args.source.to_fetch_source()?;
    let fetched = fetch_source(&fetch_requested, None, &FetchPolicy::default())?;
    let requested = args.source.to_requested_source(&args.lock_path)?;
    let package_manifest_raw = args
        .package_manifest
        .to_str()
        .ok_or_else(|| "--package-manifest must be valid UTF-8".to_owned())?;
    let package_manifest = PathAuthorizer::new("fetched package", &fetched.source_path)?
        .authorize_read(
            &args.lock_path,
            "packages.<updated>.manifest",
            package_manifest_raw,
        )
        .map_err(|error| error.to_string())?
        .into_path_buf();
    let manifest = read_manifest(&package_manifest)?;
    let package = manifest.package.as_ref().ok_or_else(|| {
        format!(
            "locked package source {} has no package metadata in {}",
            fetched.source_path.display(),
            args.package_manifest.display()
        )
    })?;
    if package.id != args.package_id {
        return Err(format!(
            "locked package source declares package id {}, expected {}",
            package.id, args.package_id
        ));
    }

    lock.packages.insert(
        args.package_id.clone(),
        LockedPackage {
            manifest: args.package_manifest.display().to_string(),
            package: LockedPackageMetadata {
                version: package.version.clone(),
            },
            original: requested.original_source(),
            locked: requested.locked_source(&fetched)?,
        },
    );
    let formatted = lockfile::to_string(&lock).map_err(|error| error.to_string())?;
    if let Some(parent) = args.lock_path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    fs::write(&args.lock_path, formatted)
        .map_err(|error| format!("{}: {error}", args.lock_path.display()))?;

    output::print_success(
        format!("updated lock package {}", args.package_id),
        &[
            ("lock", args.lock_path.display().to_string()),
            ("version", package.version.clone()),
            ("nar_hash", fetched.nar_hash),
        ],
    );
    Ok(())
}

fn verify(args: &[String]) -> Result<(), String> {
    let args = parse_lock_verify_args(args)?;
    let lock = read_lock(&args.lock_path)?;
    for (package_id, package) in &lock.packages {
        let fetched = fetch_locked_source_for_verify(&args.lock_path, package_id, &package.locked)?;
        let expected_hash = package.locked.nar_hash();
        if fetched.nar_hash != expected_hash {
            return Err(format!(
                "locked package {package_id} nar_hash mismatch: expected {expected_hash}, found {}",
                fetched.nar_hash
            ));
        }
        let manifest_path = PathAuthorizer::new("fetched package", &fetched.source_path)?
            .authorize_read(
                &args.lock_path,
                format!("packages.{package_id}.manifest"),
                &package.manifest,
            )
            .map_err(|error| error.to_string())?
            .into_path_buf();
        verify_locked_manifest_metadata(package_id, package, &manifest_path)?;
    }

    let suffix = if lock.packages.len() == 1 { "" } else { "s" };
    output::print_success(
        format!("verified {} locked package{suffix}", lock.packages.len()),
        &[("lock", args.lock_path.display().to_string())],
    );
    Ok(())
}

#[derive(Debug)]
struct LockUpdateArgs {
    lock_path: PathBuf,
    package_id: String,
    package_manifest: PathBuf,
    source: UpdateSource,
}

#[derive(Debug)]
enum UpdateSource {
    Path(PathBuf),
    Git {
        url: String,
        reference: Option<String>,
        rev: Option<String>,
    },
    Tarball {
        url: String,
    },
}

impl UpdateSource {
    fn to_fetch_source(&self) -> Result<RequestedSource, String> {
        match self {
            Self::Path(path) => {
                let path = if path.is_absolute() {
                    path.clone()
                } else {
                    env::current_dir()
                        .map_err(|error| format!("cannot resolve current directory: {error}"))?
                        .join(path)
                };
                Ok(RequestedSource::Path {
                    path: path.display().to_string(),
                })
            }
            _ => self.to_requested_source(Path::new(".")),
        }
    }

    fn to_requested_source(&self, lock_path: &Path) -> Result<RequestedSource, String> {
        match self {
            Self::Path(path) => {
                let path = path_for_lock(path, lock_path)?;
                Ok(RequestedSource::Path {
                    path: path.display().to_string(),
                })
            }
            Self::Git {
                url,
                reference,
                rev,
            } => Ok(RequestedSource::Git {
                url: normalize_github_url(url),
                reference: reference.clone(),
                rev: rev.clone(),
            }),
            Self::Tarball { url } => Ok(RequestedSource::Tarball { url: url.clone() }),
        }
    }
}

#[derive(Debug)]
struct LockVerifyArgs {
    lock_path: PathBuf,
}

fn parse_lock_update_args(args: &[String]) -> Result<LockUpdateArgs, String> {
    let mut lock_path = PathBuf::from("brainbrew.lock");
    let mut package_id = None;
    let mut package_manifest = PathBuf::from("brainbrew.yaml");
    let mut path = None;
    let mut git = None;
    let mut tarball = None;
    let mut reference = None;
    let mut rev = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--lock" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("--lock requires a path".to_owned());
                };
                lock_path = PathBuf::from(value);
                index += 2;
            }
            "--package" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("--package requires a package id".to_owned());
                };
                package_id = Some(value.clone());
                index += 2;
            }
            "--package-manifest" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("--package-manifest requires a path".to_owned());
                };
                package_manifest = PathBuf::from(value);
                index += 2;
            }
            "--path" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("--path requires a directory".to_owned());
                };
                path = Some(PathBuf::from(value));
                index += 2;
            }
            "--git" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("--git requires a URL".to_owned());
                };
                git = Some(value.clone());
                index += 2;
            }
            "--tarball" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("--tarball requires a URL".to_owned());
                };
                tarball = Some(value.clone());
                index += 2;
            }
            "--ref" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("--ref requires a ref".to_owned());
                };
                reference = Some(value.clone());
                index += 2;
            }
            "--rev" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("--rev requires a revision".to_owned());
                };
                rev = Some(value.clone());
                index += 2;
            }
            other => return Err(format!("unexpected lock update argument {other:?}")),
        }
    }
    let Some(package_id) = package_id else {
        return Err("lock update requires --package".to_owned());
    };
    let source_count =
        usize::from(path.is_some()) + usize::from(git.is_some()) + usize::from(tarball.is_some());
    if source_count != 1 {
        return Err("lock update requires exactly one of --path, --git, or --tarball".to_owned());
    }
    let source = if let Some(path) = path {
        if reference.is_some() || rev.is_some() {
            return Err("--ref and --rev are only valid with --git".to_owned());
        }
        UpdateSource::Path(path)
    } else if let Some(url) = git {
        if reference.is_some() && rev.is_some() {
            return Err("--ref and --rev cannot be used together".to_owned());
        }
        UpdateSource::Git {
            url,
            reference,
            rev,
        }
    } else {
        if reference.is_some() || rev.is_some() {
            return Err("--ref and --rev are only valid with --git".to_owned());
        }
        UpdateSource::Tarball {
            url: tarball.expect("source_count checked"),
        }
    };

    Ok(LockUpdateArgs {
        lock_path,
        package_id,
        package_manifest,
        source,
    })
}

fn parse_lock_verify_args(args: &[String]) -> Result<LockVerifyArgs, String> {
    let mut lock_path = PathBuf::from("brainbrew.lock");
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--lock" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("--lock requires a path".to_owned());
                };
                lock_path = PathBuf::from(value);
                index += 2;
            }
            other => return Err(format!("unexpected lock verify argument {other:?}")),
        }
    }
    Ok(LockVerifyArgs { lock_path })
}

fn read_lock(path: &Path) -> Result<FederationLock, String> {
    let input = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    lockfile::from_str(&input).map_err(|error| format!("{}: {error}", path.display()))
}

fn read_lock_or_empty(path: &Path) -> Result<FederationLock, String> {
    if path.exists() {
        read_lock(path)
    } else {
        Ok(FederationLock {
            version: LOCKFILE_VERSION,
            packages: BTreeMap::new(),
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) enum RequestedSource {
    Path {
        path: String,
    },
    Git {
        url: String,
        reference: Option<String>,
        rev: Option<String>,
    },
    Tarball {
        url: String,
    },
}

impl RequestedSource {
    fn original_source(&self) -> OriginalSource {
        match self {
            Self::Path { path } => OriginalSource::Path { path: path.clone() },
            Self::Git {
                url,
                reference,
                rev,
            } => OriginalSource::Git {
                url: url.clone(),
                reference: reference.clone(),
                rev: rev.clone(),
            },
            Self::Tarball { url } => OriginalSource::Tarball { url: url.clone() },
        }
    }

    fn locked_source(&self, fetched: &FetchedSource) -> Result<LockedSource, String> {
        match self {
            Self::Path { path } => Ok(LockedSource::Path {
                path: path.clone(),
                nar_hash: fetched.nar_hash.clone(),
            }),
            Self::Git { url, .. } => {
                let rev = fetched.rev.clone().ok_or_else(|| {
                    "GitHub source did not resolve to an immutable commit revision".to_owned()
                })?;
                Ok(LockedSource::Git {
                    url: url.clone(),
                    rev,
                    nar_hash: fetched.nar_hash.clone(),
                })
            }
            Self::Tarball { url } => Ok(LockedSource::Tarball {
                url: url.clone(),
                nar_hash: fetched.nar_hash.clone(),
            }),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FetchedSource {
    pub(crate) source_path: PathBuf,
    pub(crate) nar_hash: String,
    pub(crate) rev: Option<String>,
}

pub(crate) fn fetch_locked_source(
    lock_path: &Path,
    package_id: &str,
    source: &LockedSource,
) -> Result<FetchedSource, String> {
    fetch_locked_source_with_mode(lock_path, package_id, source, FetchLockedMode::UseCache)
}

fn fetch_locked_source_for_verify(
    lock_path: &Path,
    package_id: &str,
    source: &LockedSource,
) -> Result<FetchedSource, String> {
    fetch_locked_source_with_mode(
        lock_path,
        package_id,
        source,
        FetchLockedMode::VerifyLivePath,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FetchLockedMode {
    UseCache,
    VerifyLivePath,
}

fn fetch_locked_source_with_mode(
    lock_path: &Path,
    package_id: &str,
    source: &LockedSource,
    mode: FetchLockedMode,
) -> Result<FetchedSource, String> {
    let expected_hash = source.nar_hash();
    let verify_live_path =
        mode == FetchLockedMode::VerifyLivePath && matches!(source, LockedSource::Path { .. });
    if !verify_live_path && let Some(cached) = cached_source(Some(expected_hash))? {
        return Ok(cached);
    }

    let requested = match source {
        LockedSource::Path { path, .. } => RequestedSource::Path {
            path: lock_relative_path(lock_path, path).display().to_string(),
        },
        LockedSource::Git { url, rev, .. } => RequestedSource::Git {
            url: url.clone(),
            reference: None,
            rev: Some(rev.clone()),
        },
        LockedSource::Tarball { url, .. } => RequestedSource::Tarball { url: url.clone() },
    };
    fetch_source(&requested, Some(expected_hash), &FetchPolicy::default())
        .map_err(|error| format!("locked package {package_id}: {error}"))
}

pub(crate) fn locked_package_manifest_paths(lock_path: &Path) -> Result<Vec<PathBuf>, String> {
    if !lock_path.exists() {
        return Ok(Vec::new());
    }
    let lock = read_lock(lock_path)?;
    lock.packages
        .iter()
        .map(|(package_id, package)| {
            let fetched = fetch_locked_source(lock_path, package_id, &package.locked)?;
            let expected_hash = package.locked.nar_hash();
            if fetched.nar_hash != expected_hash {
                return Err(format!(
                    "locked package {package_id} nar_hash mismatch: expected {expected_hash}, found {}",
                    fetched.nar_hash
                ));
            }
            let manifest_path = PathAuthorizer::new("fetched package", &fetched.source_path)?
                .authorize_read(
                    lock_path,
                    format!("packages.{package_id}.manifest"),
                    &package.manifest,
                )
                .map_err(|error| error.to_string())?
                .into_path_buf();
            verify_locked_manifest_metadata(package_id, package, &manifest_path)?;
            Ok(manifest_path)
        })
        .collect()
}

fn fetch_source(
    source: &RequestedSource,
    expected_hash: Option<&str>,
    policy: &FetchPolicy,
) -> Result<FetchedSource, String> {
    match source {
        RequestedSource::Path { path } => {
            snapshot_source_tree(Path::new(path), expected_hash, None)
        }
        RequestedSource::Git { .. } => fetch_git_source(source, expected_hash, policy),
        RequestedSource::Tarball { url } => fetch_tarball_source(url, expected_hash, None, policy),
    }
}

fn fetch_git_source(
    source: &RequestedSource,
    expected_hash: Option<&str>,
    policy: &FetchPolicy,
) -> Result<FetchedSource, String> {
    let RequestedSource::Git {
        url,
        reference,
        rev,
    } = source
    else {
        return Err("internal error: expected git source".to_owned());
    };
    let Some(repo) = GithubRepo::parse(url) else {
        return Err(format!(
            "native git locking currently supports GitHub HTTPS URLs; use --tarball for {url:?}"
        ));
    };
    let rev = match rev {
        Some(rev) if lockfile::is_full_git_commit(rev) => rev.clone(),
        Some(rev) => resolve_github_rev(&repo, Some(rev), policy)?,
        None => resolve_github_rev(&repo, reference.as_deref(), policy)?,
    };
    let tarball = repo.codeload_tarball_url(&rev);
    fetch_tarball_source(&tarball, expected_hash, Some(rev), policy)
}

fn fetch_tarball_source(
    url: &str,
    expected_hash: Option<&str>,
    rev: Option<String>,
    policy: &FetchPolicy,
) -> Result<FetchedSource, String> {
    if let Some(cached) = cached_source(expected_hash)? {
        return Ok(FetchedSource { rev, ..cached });
    }

    let download = fetch_policy::fetch_to_temp(
        url,
        policy,
        policy.max_download_bytes,
        Some("application/octet-stream"),
    )?;
    let extracted = TempDir::new().map_err(|error| error.to_string())?;
    let extracted_tree = extracted.path().join("archive");
    unpack_tarball(&download, &extracted_tree, policy, url)?;
    let source_root = normalized_extracted_root(&extracted_tree)?;
    snapshot_source_tree(&source_root, expected_hash, rev)
}

fn snapshot_source_tree(
    source_path: &Path,
    expected_hash: Option<&str>,
    rev: Option<String>,
) -> Result<FetchedSource, String> {
    package_tree::validate(source_path, "selected source package tree")?;

    let staging = TempDir::new().map_err(|error| error.to_string())?;
    let staged_source = staging.path().join("source");
    package_tree::copy_filtered(source_path, &staged_source)?;
    let nar_hash = validated_nar_hash(&staged_source, "staged package tree")?;

    if let Some(expected_hash) = expected_hash
        && nar_hash != expected_hash
    {
        return Err(format!(
            "nar_hash mismatch: expected {expected_hash}, found {nar_hash}"
        ));
    }

    let cache_path = publish_cache_tree(&staged_source, &nar_hash)?;
    Ok(FetchedSource {
        source_path: cache_path,
        nar_hash,
        rev,
    })
}

fn cached_source(expected_hash: Option<&str>) -> Result<Option<FetchedSource>, String> {
    let Some(expected_hash) = expected_hash else {
        return Ok(None);
    };
    let path = cache_source_path(expected_hash);
    if !path.exists() {
        return Ok(None);
    }
    let actual_hash = validated_nar_hash(&path, "cached package tree")?;
    if actual_hash != expected_hash {
        return Err(format!(
            "cached source {} nar_hash mismatch: expected {expected_hash}, found {actual_hash}; remove the tampered cache entry before retrying",
            path.display()
        ));
    }
    Ok(Some(FetchedSource {
        source_path: path,
        nar_hash: actual_hash,
        rev: None,
    }))
}

fn read_json_url(url: &str, policy: &FetchPolicy) -> Result<Value, String> {
    let download = fetch_policy::fetch_to_temp(
        url,
        policy,
        policy.max_json_bytes,
        Some("application/vnd.github+json"),
    )?;
    let body = fs::read(download.path())
        .map_err(|error| format!("package source {url:?}: failed to read staged JSON: {error}"))?;
    serde_json::from_slice(&body).map_err(|error| format!("failed to parse {url} as JSON: {error}"))
}

fn unpack_tarball(
    download: &fetch_policy::DownloadedFile,
    destination: &Path,
    policy: &FetchPolicy,
    source: &str,
) -> Result<(), String> {
    let mut input = fs::File::open(download.path())
        .map_err(|error| format!("package source {source:?}: {error}"))?;
    let mut magic = [0_u8; 2];
    let magic_len = input
        .read(&mut magic)
        .map_err(|error| format!("package source {source:?}: {error}"))?;
    input
        .seek(SeekFrom::Start(0))
        .map_err(|error| format!("package source {source:?}: {error}"))?;

    let mut staged_tar =
        NamedTempFile::new().map_err(|error| format!("package source {source:?}: {error}"))?;
    if magic_len == 2 && magic == [0x1f, 0x8b] {
        copy_decompressed_tar(
            GzDecoder::new(input),
            &mut staged_tar,
            download,
            policy,
            source,
        )?;
    } else {
        copy_decompressed_tar(input, &mut staged_tar, download, policy, source)?;
    }
    staged_tar.as_file_mut().sync_all().map_err(|error| {
        format!("package source {source:?}: failed to sync staged tar: {error}")
    })?;
    package_tree::extract_tar(
        staged_tar.path(),
        destination,
        policy,
        source,
        download.started,
    )
    .map_err(|error| format!("failed to extract package archive: {error}"))
}

fn copy_decompressed_tar(
    mut input: impl Read,
    output: &mut impl Write,
    download: &fetch_policy::DownloadedFile,
    policy: &FetchPolicy,
    source: &str,
) -> Result<(), String> {
    let ratio_limit = download.bytes.saturating_mul(policy.max_expansion_ratio);
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        fetch_policy::check_total_deadline(source, policy, download.started)?;
        let count = input.read(&mut buffer).map_err(|error| {
            format!("package source {source:?}: truncated or invalid compressed archive: {error}")
        })?;
        if count == 0 {
            break;
        }
        total = total.saturating_add(count as u64);
        if total > policy.max_decompressed_tar_bytes {
            return Err(budget_error(
                source,
                "decompressed_tar_bytes",
                total,
                policy.max_decompressed_tar_bytes,
            ));
        }
        if total > ratio_limit {
            let current_ratio = total
                .saturating_add(download.bytes.saturating_sub(1))
                .checked_div(download.bytes)
                .unwrap_or(u64::MAX);
            return Err(budget_error(
                source,
                "archive_expansion_ratio",
                current_ratio,
                policy.max_expansion_ratio,
            ));
        }
        output.write_all(&buffer[..count]).map_err(|error| {
            format!("package source {source:?}: staged tar write failed: {error}")
        })?;
    }
    Ok(())
}

fn normalized_extracted_root(path: &Path) -> Result<PathBuf, String> {
    let entries = fs::read_dir(path)
        .map_err(|error| format!("{}: {error}", path.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    if entries.len() == 1 {
        let only = entries[0].path();
        if only.is_dir() {
            return Ok(only);
        }
    }
    Ok(path.to_path_buf())
}

#[derive(Debug)]
struct GithubRepo {
    owner: String,
    name: String,
}

impl GithubRepo {
    fn parse(url: &str) -> Option<Self> {
        let path = url.strip_prefix("https://github.com/")?;
        let mut parts = path.trim_end_matches('/').split('/');
        let owner = parts.next()?.to_owned();
        let name = parts.next()?.trim_end_matches(".git").to_owned();
        if owner.is_empty() || name.is_empty() || parts.next().is_some() {
            return None;
        }
        Some(Self { owner, name })
    }

    fn canonical_url(&self) -> String {
        format!("https://github.com/{}/{}.git", self.owner, self.name)
    }

    fn api_url(&self) -> String {
        format!("https://api.github.com/repos/{}/{}", self.owner, self.name)
    }

    fn commit_api_url(&self, reference: &str) -> String {
        format!(
            "https://api.github.com/repos/{}/{}/commits/{}",
            self.owner,
            self.name,
            percent_encode_path_segment(reference)
        )
    }

    fn codeload_tarball_url(&self, rev: &str) -> String {
        format!(
            "https://codeload.github.com/{}/{}/tar.gz/{}",
            self.owner, self.name, rev
        )
    }
}

fn normalize_github_url(url: &str) -> String {
    GithubRepo::parse(url)
        .map(|repo| repo.canonical_url())
        .unwrap_or_else(|| url.to_owned())
}

fn resolve_github_rev(
    repo: &GithubRepo,
    reference: Option<&str>,
    policy: &FetchPolicy,
) -> Result<String, String> {
    let reference = if let Some(reference) = reference {
        reference.to_owned()
    } else {
        read_json_url(&repo.api_url(), policy)?
            .get("default_branch")
            .and_then(Value::as_str)
            .ok_or_else(|| "GitHub repository response did not include default_branch".to_owned())?
            .to_owned()
    };
    read_json_url(&repo.commit_api_url(&reference), policy)?
        .get("sha")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("GitHub commit response did not include sha for {reference:?}"))
}

fn percent_encode_path_segment(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn verify_locked_manifest_metadata(
    package_id: &str,
    package: &LockedPackage,
    manifest_path: &Path,
) -> Result<(), String> {
    let manifest = read_manifest(manifest_path)?;
    let metadata = manifest.package.as_ref().ok_or_else(|| {
        format!(
            "locked package {package_id} manifest {} has no package metadata",
            manifest_path.display()
        )
    })?;
    if metadata.id != *package_id {
        return Err(format!(
            "locked package {package_id} manifest {} declares package id {}",
            manifest_path.display(),
            metadata.id
        ));
    }
    if metadata.version != package.package.version {
        return Err(format!(
            "locked package {package_id} version mismatch: lock has {}, manifest has {}",
            package.package.version, metadata.version
        ));
    }
    Ok(())
}

fn validated_nar_hash(path: &Path, tree_name: &str) -> Result<String, String> {
    package_tree::validate(path, tree_name)?;
    let hash = nar_hash_path(path)?;
    package_tree::validate(path, tree_name)?;
    Ok(hash)
}

fn nar_hash_path(path: &Path) -> Result<String, String> {
    let mut encoder = Encoder::new(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = encoder
            .read(&mut buffer)
            .map_err(|error| format!("failed to encode {} as NAR: {error}", path.display()))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!(
        "sha256-{}",
        BASE64_STANDARD.encode(hasher.finalize())
    ))
}

fn publish_cache_tree(staged_source: &Path, nar_hash: &str) -> Result<PathBuf, String> {
    let cache_path = cache_source_path(nar_hash);
    let parent = cache_path
        .parent()
        .ok_or_else(|| format!("cache path {} has no parent", cache_path.display()))?;
    fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;

    let lock_path = parent
        .parent()
        .unwrap_or(parent)
        .join(".sources-publish.lock");
    let publication_lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|error| format!("{}: {error}", lock_path.display()))?;
    publication_lock
        .lock_exclusive()
        .map_err(|error| format!("failed to lock {}: {error}", lock_path.display()))?;

    let result = (|| {
        if cache_path.exists() {
            let cached_hash = validated_nar_hash(&cache_path, "cached package tree")?;
            if cached_hash != nar_hash {
                return Err(format!(
                    "cached source {} nar_hash mismatch: expected {nar_hash}, found {cached_hash}; remove the tampered cache entry before retrying",
                    cache_path.display()
                ));
            }
            return Ok(cache_path.clone());
        }

        let publication = tempfile::Builder::new()
            .prefix(".publish-")
            .tempdir_in(parent)
            .map_err(|error| format!("failed to create private cache publication tree: {error}"))?;
        let candidate = publication.path().join("source");
        package_tree::copy_complete(staged_source, &candidate)?;
        let candidate_hash = validated_nar_hash(&candidate, "publication package tree")?;
        if candidate_hash != nar_hash {
            return Err(format!(
                "publication package tree hash changed: expected {nar_hash}, found {candidate_hash}"
            ));
        }
        fs::rename(&candidate, &cache_path).map_err(|error| {
            format!(
                "failed to atomically publish {} to {}: {error}",
                candidate.display(),
                cache_path.display()
            )
        })?;

        match validated_nar_hash(&cache_path, "cached package tree") {
            Ok(published_hash) if published_hash == nar_hash => {
                sync_directory(parent)?;
                Ok(cache_path.clone())
            }
            Ok(published_hash) => {
                let _ = fs::remove_dir_all(&cache_path);
                Err(format!(
                    "published cache {} nar_hash mismatch: expected {nar_hash}, found {published_hash}; incomplete publication was removed",
                    cache_path.display()
                ))
            }
            Err(error) => {
                let _ = fs::remove_dir_all(&cache_path);
                Err(format!(
                    "published cache {} failed validation: {error}; incomplete publication was removed",
                    cache_path.display()
                ))
            }
        }
    })();
    let unlock_result = fs2::FileExt::unlock(&publication_lock)
        .map_err(|error| format!("failed to unlock {}: {error}", lock_path.display()));
    match (result, unlock_result) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Ok(path), Ok(())) => Ok(path),
    }
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), String> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("failed to sync cache directory {}: {error}", path.display()))
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), String> {
    Ok(())
}

fn path_for_lock(path: &Path, lock_path: &Path) -> Result<PathBuf, String> {
    let canonical_path = canonicalize_for_lock(path)?;
    let lock_parent = lock_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let canonical_lock_parent = canonicalize_for_lock(lock_parent)?;
    Ok(relative_path_between(&canonical_lock_parent, &canonical_path).unwrap_or(canonical_path))
}

fn canonicalize_for_lock(path: &Path) -> Result<PathBuf, String> {
    path.canonicalize()
        .map_err(|error| format!("{}: {error}", path.display()))
}

fn relative_path_between(base: &Path, target: &Path) -> Option<PathBuf> {
    let base_components = base.components().collect::<Vec<_>>();
    let target_components = target.components().collect::<Vec<_>>();
    let common_len = base_components
        .iter()
        .zip(&target_components)
        .take_while(|(base, target)| base == target)
        .count();
    if common_len == 0 {
        return None;
    }

    let mut relative = PathBuf::new();
    for component in &base_components[common_len..] {
        match component {
            std::path::Component::Normal(_) => relative.push(".."),
            _ => return None,
        }
    }
    for component in &target_components[common_len..] {
        relative.push(component.as_os_str());
    }
    if relative.as_os_str().is_empty() {
        relative.push(".");
    }
    Some(relative)
}

fn lock_relative_path(lock_path: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        lock_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(path)
    }
}

fn cache_source_path(nar_hash: &str) -> PathBuf {
    cache_root().join("sources").join(cache_key(nar_hash))
}

fn cache_key(value: &str) -> String {
    let mut key = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || byte == b'-' {
            key.push(byte as char);
        } else {
            key.push_str(&format!("_{byte:02X}"));
        }
    }
    key
}

fn cache_root() -> PathBuf {
    if let Some(path) = env::var_os("BRAINBREW_CACHE_DIR") {
        return PathBuf::from(path);
    }

    #[cfg(windows)]
    {
        if let Some(path) = env::var_os("LOCALAPPDATA") {
            return PathBuf::from(path).join("BrainBrew").join("cache");
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = env::var_os("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Caches")
                .join("brainbrew");
        }
    }

    if let Some(path) = env::var_os("XDG_CACHE_HOME") {
        return PathBuf::from(path).join("brainbrew");
    }
    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home).join(".cache").join("brainbrew");
    }
    env::temp_dir().join("brainbrew-cache")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_repo_parses_only_plain_github_repo_urls() {
        let repo = GithubRepo::parse("https://github.com/anki-geo/ultimate-geography.git")
            .expect("GitHub HTTPS repo URL parses");
        assert_eq!(repo.owner, "anki-geo");
        assert_eq!(repo.name, "ultimate-geography");

        assert!(
            GithubRepo::parse("http://github.com/anki-geo/ultimate-geography").is_none(),
            "production GitHub sources must be HTTPS"
        );
        assert_eq!(
            normalize_github_url("http://github.com/anki-geo/ultimate-geography/"),
            "http://github.com/anki-geo/ultimate-geography/"
        );

        assert!(
            GithubRepo::parse("https://github.com/anki-geo/ultimate-geography/tree/main").is_none()
        );
        assert!(GithubRepo::parse("https://example.com/anki-geo/ultimate-geography.git").is_none());
    }

    #[test]
    fn github_repo_builds_codeload_and_percent_encoded_api_urls() {
        let repo = GithubRepo {
            owner: "anki geo".to_owned(),
            name: "ultimate/geography".to_owned(),
        };

        assert_eq!(
            repo.codeload_tarball_url("abc123"),
            "https://codeload.github.com/anki geo/ultimate/geography/tar.gz/abc123"
        );
        assert_eq!(
            repo.commit_api_url("feature/deck lock"),
            "https://api.github.com/repos/anki geo/ultimate/geography/commits/feature%2Fdeck%20lock"
        );
    }

    #[test]
    fn non_github_git_url_reports_native_locking_error() {
        let source = RequestedSource::Git {
            url: "https://example.com/anki-geo/ultimate-geography.git".to_owned(),
            reference: None,
            rev: Some("abc123".to_owned()),
        };

        let error = fetch_git_source(&source, Some("sha256-example"), &FetchPolicy::default())
            .expect_err("non-GitHub URL is rejected before fetch");
        assert!(error.contains("native git locking currently supports GitHub HTTPS URLs"));
    }

    #[test]
    fn should_skip_source_entry_filters_vcs_build_and_nix_result_paths() {
        for name in [
            ".git",
            ".jj",
            ".hg",
            ".svn",
            "target",
            "result",
            "result-doc",
        ] {
            assert!(package_tree::should_skip(name), "{name} should be skipped");
        }
        for name in ["deck.yaml", "results", "target-notes", ".github"] {
            assert!(
                !package_tree::should_skip(name),
                "{name} should be included"
            );
        }
    }

    #[test]
    fn relative_path_between_sibling_checkout_paths_uses_dot_dot() {
        let base = Path::new("/workspace/consumer");
        let target = Path::new("/workspace/package");

        assert_eq!(
            relative_path_between(base, target).unwrap(),
            PathBuf::from("../package")
        );
    }

    #[test]
    fn verify_locked_manifest_metadata_reports_version_mismatch() {
        let dir = TempDir::new().expect("temp dir");
        let manifest_path = dir.path().join("brainbrew.yaml");
        fs::write(
            &manifest_path,
            r#"package:
  id: anki-geo.ultimate-geography
  version: 0.2.0
base: deck.yaml
overlays: {}
targets: {}
"#,
        )
        .expect("write manifest");
        let package = LockedPackage {
            manifest: "brainbrew.yaml".to_owned(),
            package: LockedPackageMetadata {
                version: "0.1.0".to_owned(),
            },
            original: OriginalSource::Path {
                path: ".".to_owned(),
            },
            locked: LockedSource::Path {
                path: ".".to_owned(),
                nar_hash: "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_owned(),
            },
        };

        let error = verify_locked_manifest_metadata(
            "anki-geo.ultimate-geography",
            &package,
            &manifest_path,
        )
        .expect_err("version mismatch is rejected");
        assert!(error.contains("version mismatch"));
    }

    #[test]
    fn read_lock_reports_missing_and_corrupt_lock_files() {
        let dir = TempDir::new().expect("temp dir");
        let missing = dir.path().join("missing.lock");
        let missing_error = read_lock(&missing).expect_err("missing lock file is rejected");
        assert!(missing_error.contains("missing.lock"));

        let corrupt = dir.path().join("brainbrew.lock");
        fs::write(&corrupt, "version: [\n").expect("write corrupt lock");
        let corrupt_error = read_lock(&corrupt).expect_err("corrupt lock file is rejected");
        assert!(corrupt_error.contains("failed to parse lock YAML"));
    }

    #[test]
    fn archive_staging_enforces_compressed_decompressed_and_ratio_budgets() {
        let root = TempDir::new().unwrap();
        let archive = root.path().join("fixture.tar.gz");
        write_compressible_archive(&archive);
        let source = archive.to_str().unwrap();

        let compressed_policy = FetchPolicy {
            max_download_bytes: 10,
            ..FetchPolicy::default()
        };
        let error = fetch_policy::fetch_to_temp(source, &compressed_policy, 10, None)
            .expect_err("compressed bytes are bounded");
        assert!(error.contains("compressed_download_bytes"), "{error}");

        let decompressed_policy = FetchPolicy {
            max_decompressed_tar_bytes: 1024,
            max_expansion_ratio: u64::MAX,
            ..FetchPolicy::default()
        };
        let download = fetch_policy::fetch_to_temp(
            source,
            &decompressed_policy,
            decompressed_policy.max_download_bytes,
            None,
        )
        .unwrap();
        let destination = root.path().join("decompressed-out");
        let error = unpack_tarball(&download, &destination, &decompressed_policy, source)
            .expect_err("decompressed tar bytes are bounded");
        assert!(error.contains("decompressed_tar_bytes"), "{error}");
        assert!(!destination.exists());

        let ratio_policy = FetchPolicy {
            max_expansion_ratio: 1,
            ..FetchPolicy::default()
        };
        let download = fetch_policy::fetch_to_temp(
            source,
            &ratio_policy,
            ratio_policy.max_download_bytes,
            None,
        )
        .unwrap();
        let destination = root.path().join("ratio-out");
        let error = unpack_tarball(&download, &destination, &ratio_policy, source)
            .expect_err("archive expansion ratio is bounded");
        assert!(error.contains("archive_expansion_ratio"), "{error}");
        assert!(!destination.exists());
    }

    #[test]
    fn truncated_gzip_is_rejected_without_publishing_extraction_state() {
        let root = TempDir::new().unwrap();
        let archive = root.path().join("truncated.tar.gz");
        fs::write(&archive, [0x1f, 0x8b, 0x08, 0x00, 0x00]).unwrap();
        let source = archive.to_str().unwrap();
        let policy = FetchPolicy::default();
        let download =
            fetch_policy::fetch_to_temp(source, &policy, policy.max_download_bytes, None).unwrap();
        let destination = root.path().join("out");
        let error = unpack_tarball(&download, &destination, &policy, source)
            .expect_err("truncated gzip must fail");
        assert!(
            error.contains("truncated or invalid compressed archive"),
            "{error}"
        );
        assert!(!destination.exists());
    }

    fn write_compressible_archive(path: &Path) {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use tar::{Builder, Header};

        let output = fs::File::create(path).unwrap();
        let encoder = GzEncoder::new(output, Compression::best());
        let mut builder = Builder::new(encoder);
        let bytes = vec![b'a'; 32 * 1024];
        let mut header = Header::new_gnu();
        header.set_path("pkg/repeated.txt").unwrap();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append(&header, bytes.as_slice()).unwrap();
        let encoder = builder.into_inner().unwrap();
        encoder.finish().unwrap();
    }
}
