use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use brain_brew_formats::lockfile::{
    self, FederationLock, LockedPackage, LockedPackageMetadata, LockedSource,
};
use flate2::read::GzDecoder;
use nix_nar::Encoder;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tar::Archive;
use tempfile::TempDir;

use crate::help;
use crate::io::read_manifest;
use crate::output;

const USER_AGENT: &str = concat!("brainbrew/", env!("CARGO_PKG_VERSION"));

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
    let fetch_requested = args.source.to_fetch_source()?;
    let fetched = fetch_source(&fetch_requested, None)?;
    let requested = args.source.to_requested_source(&args.lock_path)?;
    let package_manifest = fetched.source_path.join(&args.package_manifest);
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

    let mut lock = read_lock_or_empty(&args.lock_path)?;
    lock.packages.insert(
        args.package_id.clone(),
        LockedPackage {
            manifest: args.package_manifest.display().to_string(),
            package: LockedPackageMetadata {
                version: package.version.clone(),
            },
            original: Some(requested.original_source()),
            locked: requested.locked_source(&fetched),
        },
    );
    let formatted = lockfile::to_string(&lock);
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
        if let Some(expected_hash) = &package.locked.nar_hash
            && &fetched.nar_hash != expected_hash
        {
            return Err(format!(
                "locked package {package_id} nar_hash mismatch: expected {expected_hash}, found {}",
                fetched.nar_hash
            ));
        }
        let manifest_path = fetched.source_path.join(&package.manifest);
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
                let path = canonicalize_for_lock(path)?;
                Ok(RequestedSource {
                    source_type: "path".to_owned(),
                    url: None,
                    path: Some(path.display().to_string()),
                    reference: None,
                    rev: None,
                })
            }
            _ => self.to_requested_source(Path::new(".")),
        }
    }

    fn to_requested_source(&self, lock_path: &Path) -> Result<RequestedSource, String> {
        match self {
            Self::Path(path) => {
                let path = path_for_lock(path, lock_path)?;
                Ok(RequestedSource {
                    source_type: "path".to_owned(),
                    url: None,
                    path: Some(path.display().to_string()),
                    reference: None,
                    rev: None,
                })
            }
            Self::Git {
                url,
                reference,
                rev,
            } => Ok(RequestedSource {
                source_type: "git".to_owned(),
                url: Some(url.clone()),
                path: None,
                reference: reference.clone(),
                rev: rev.clone(),
            }),
            Self::Tarball { url } => Ok(RequestedSource {
                source_type: "tarball".to_owned(),
                url: Some(url.clone()),
                path: None,
                reference: None,
                rev: None,
            }),
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
    lockfile::from_str(&input).map_err(|error| error.to_string())
}

fn read_lock_or_empty(path: &Path) -> Result<FederationLock, String> {
    if path.exists() {
        read_lock(path)
    } else {
        Ok(FederationLock {
            version: 1,
            packages: BTreeMap::new(),
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct RequestedSource {
    pub(crate) source_type: String,
    pub(crate) url: Option<String>,
    pub(crate) path: Option<String>,
    pub(crate) reference: Option<String>,
    pub(crate) rev: Option<String>,
}

impl RequestedSource {
    fn original_source(&self) -> LockedSource {
        LockedSource {
            source_type: self.source_type.clone(),
            url: self.url.clone(),
            path: self.path.clone(),
            reference: self.reference.clone(),
            rev: self.rev.clone(),
            nar_hash: None,
        }
    }

    fn locked_source(&self, fetched: &FetchedSource) -> LockedSource {
        match self.source_type.as_str() {
            "path" => LockedSource {
                source_type: "path".to_owned(),
                url: None,
                path: self.path.clone(),
                reference: None,
                rev: None,
                nar_hash: Some(fetched.nar_hash.clone()),
            },
            "git" => LockedSource {
                source_type: "git".to_owned(),
                url: self.url.clone(),
                path: None,
                reference: None,
                rev: fetched.rev.clone().or_else(|| self.rev.clone()),
                nar_hash: Some(fetched.nar_hash.clone()),
            },
            "tarball" => LockedSource {
                source_type: "tarball".to_owned(),
                url: self.url.clone(),
                path: None,
                reference: None,
                rev: None,
                nar_hash: Some(fetched.nar_hash.clone()),
            },
            other => LockedSource {
                source_type: other.to_owned(),
                url: self.url.clone(),
                path: self.path.clone(),
                reference: self.reference.clone(),
                rev: self.rev.clone(),
                nar_hash: Some(fetched.nar_hash.clone()),
            },
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
    let expected_hash = source.nar_hash.as_deref();
    if !(mode == FetchLockedMode::VerifyLivePath && source.source_type == "path")
        && let Some(cached) = cached_source(expected_hash)?
    {
        return Ok(cached);
    }

    let requested = RequestedSource {
        source_type: source.source_type.clone(),
        url: source.url.clone(),
        path: source
            .path
            .as_deref()
            .map(|path| lock_relative_path(lock_path, path).display().to_string()),
        reference: source.reference.clone(),
        rev: source.rev.clone(),
    };
    fetch_source(&requested, expected_hash)
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
            if let Some(expected_hash) = &package.locked.nar_hash
                && &fetched.nar_hash != expected_hash
            {
                return Err(format!(
                    "locked package {package_id} nar_hash mismatch: expected {expected_hash}, found {}",
                    fetched.nar_hash
                ));
            }
            let manifest_path = fetched.source_path.join(&package.manifest);
            verify_locked_manifest_metadata(package_id, package, &manifest_path)?;
            Ok(manifest_path)
        })
        .collect()
}

fn fetch_source(
    source: &RequestedSource,
    expected_hash: Option<&str>,
) -> Result<FetchedSource, String> {
    match source.source_type.as_str() {
        "path" => {
            let Some(path) = &source.path else {
                return Err("path source requires path".to_owned());
            };
            snapshot_source_tree(Path::new(path), expected_hash, None)
        }
        "git" => fetch_git_source(source, expected_hash),
        "tarball" => {
            let Some(url) = &source.url else {
                return Err("tarball source requires url".to_owned());
            };
            fetch_tarball_source(url, expected_hash, None)
        }
        other => Err(format!("unsupported locked source type {other:?}")),
    }
}

fn fetch_git_source(
    source: &RequestedSource,
    expected_hash: Option<&str>,
) -> Result<FetchedSource, String> {
    let Some(url) = &source.url else {
        return Err("git source requires url".to_owned());
    };
    let Some(repo) = GithubRepo::parse(url) else {
        return Err(format!(
            "native git locking currently supports GitHub HTTPS URLs; use --tarball for {url:?}"
        ));
    };
    let rev = if let Some(rev) = &source.rev {
        rev.clone()
    } else {
        resolve_github_rev(&repo, source.reference.as_deref())?
    };
    let tarball = repo.codeload_tarball_url(&rev);
    fetch_tarball_source(&tarball, expected_hash, Some(rev))
}

fn fetch_tarball_source(
    url: &str,
    expected_hash: Option<&str>,
    rev: Option<String>,
) -> Result<FetchedSource, String> {
    if let Some(cached) = cached_source(expected_hash)? {
        return Ok(FetchedSource { rev, ..cached });
    }

    let bytes = read_url_or_file(url)?;
    let extracted = TempDir::new().map_err(|error| error.to_string())?;
    unpack_tarball(&bytes, extracted.path())?;
    let source_root = normalized_extracted_root(extracted.path())?;
    snapshot_source_tree(&source_root, expected_hash, rev)
}

fn snapshot_source_tree(
    source_path: &Path,
    expected_hash: Option<&str>,
    rev: Option<String>,
) -> Result<FetchedSource, String> {
    let source_path = source_path
        .canonicalize()
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    if !source_path.is_dir() {
        return Err(format!("{} is not a directory", source_path.display()));
    }

    let staging = TempDir::new().map_err(|error| error.to_string())?;
    let staged_source = staging.path().join("source");
    copy_source_tree_filtered(&source_path, &staged_source)?;
    let nar_hash = nar_hash_path(&staged_source)?;

    if let Some(expected_hash) = expected_hash
        && nar_hash != expected_hash
    {
        return Err(format!(
            "nar_hash mismatch: expected {expected_hash}, found {nar_hash}"
        ));
    }

    let cache_path = cache_source_path(&nar_hash);
    if !cache_path.exists() {
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
        }
        fs::rename(&staged_source, &cache_path).or_else(|_| {
            copy_source_tree_filtered(&staged_source, &cache_path)?;
            fs::remove_dir_all(&staged_source)
                .map_err(|error| format!("{}: {error}", staged_source.display()))
        })?;
    }

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
    let actual_hash = nar_hash_path(&path)?;
    if actual_hash == expected_hash {
        return Ok(Some(FetchedSource {
            source_path: path,
            nar_hash: actual_hash,
            rev: None,
        }));
    }
    fs::remove_dir_all(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(None)
}

fn read_url_or_file(url: &str) -> Result<Vec<u8>, String> {
    if let Some(path) = url.strip_prefix("file://") {
        return fs::read(path).map_err(|error| format!("{path}: {error}"));
    }
    let path = Path::new(url);
    if path.exists() {
        return fs::read(path).map_err(|error| format!("{}: {error}", path.display()));
    }

    let response = ureq::get(url)
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(|error| format!("failed to fetch {url}: {error}"))?;
    let mut reader = response.into_reader();
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|error| format!("failed to read {url}: {error}"))?;
    Ok(bytes)
}

fn read_json_url(url: &str) -> Result<Value, String> {
    let response = ureq::get(url)
        .set("Accept", "application/vnd.github+json")
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(|error| format!("failed to fetch {url}: {error}"))?;
    let mut reader = response.into_reader();
    let mut body = String::new();
    reader
        .read_to_string(&mut body)
        .map_err(|error| format!("failed to read {url}: {error}"))?;
    serde_json::from_str(&body).map_err(|error| format!("failed to parse {url} as JSON: {error}"))
}

fn unpack_tarball(bytes: &[u8], destination: &Path) -> Result<(), String> {
    if bytes.starts_with(&[0x1f, 0x8b]) {
        let decoder = GzDecoder::new(Cursor::new(bytes));
        let mut archive = Archive::new(decoder);
        archive
            .unpack(destination)
            .map_err(|error| format!("failed to unpack tar.gz: {error}"))?;
    } else {
        let mut archive = Archive::new(Cursor::new(bytes));
        archive
            .unpack(destination)
            .map_err(|error| format!("failed to unpack tar archive: {error}"))?;
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
        let path = url
            .strip_prefix("https://github.com/")
            .or_else(|| url.strip_prefix("http://github.com/"))?;
        let mut parts = path.trim_end_matches('/').split('/');
        let owner = parts.next()?.to_owned();
        let name = parts.next()?.trim_end_matches(".git").to_owned();
        if owner.is_empty() || name.is_empty() || parts.next().is_some() {
            return None;
        }
        Some(Self { owner, name })
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

fn resolve_github_rev(repo: &GithubRepo, reference: Option<&str>) -> Result<String, String> {
    let reference = if let Some(reference) = reference {
        reference.to_owned()
    } else {
        read_json_url(&repo.api_url())?
            .get("default_branch")
            .and_then(Value::as_str)
            .ok_or_else(|| "GitHub repository response did not include default_branch".to_owned())?
            .to_owned()
    };
    read_json_url(&repo.commit_api_url(&reference))?
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

fn copy_source_tree_filtered(source: &Path, destination: &Path) -> Result<(), String> {
    if destination.exists() {
        fs::remove_dir_all(destination)
            .map_err(|error| format!("{}: {error}", destination.display()))?;
    }
    fs::create_dir_all(destination)
        .map_err(|error| format!("{}: {error}", destination.display()))?;
    copy_dir_contents(source, destination)
}

fn copy_dir_contents(source: &Path, destination: &Path) -> Result<(), String> {
    for entry in fs::read_dir(source).map_err(|error| format!("{}: {error}", source.display()))? {
        let entry = entry.map_err(|error| error.to_string())?;
        let file_name = entry.file_name();
        if should_skip_source_entry(&file_name.to_string_lossy()) {
            continue;
        }
        let source_path = entry.path();
        let destination_path = destination.join(&file_name);
        let metadata = fs::symlink_metadata(&source_path)
            .map_err(|error| format!("{}: {error}", source_path.display()))?;
        if metadata.file_type().is_symlink() {
            copy_symlink(&source_path, &destination_path)?;
        } else if metadata.is_dir() {
            fs::create_dir_all(&destination_path)
                .map_err(|error| format!("{}: {error}", destination_path.display()))?;
            copy_dir_contents(&source_path, &destination_path)?;
        } else if metadata.is_file() {
            fs::copy(&source_path, &destination_path).map_err(|error| {
                format!(
                    "failed to copy {} to {}: {error}",
                    source_path.display(),
                    destination_path.display()
                )
            })?;
            let permissions = metadata.permissions();
            fs::set_permissions(&destination_path, permissions)
                .map_err(|error| format!("{}: {error}", destination_path.display()))?;
        }
    }
    Ok(())
}

fn should_skip_source_entry(name: &str) -> bool {
    matches!(name, ".git" | ".jj" | ".hg" | ".svn" | "target" | "result")
        || name.starts_with("result-")
}

#[cfg(unix)]
fn copy_symlink(source: &Path, destination: &Path) -> Result<(), String> {
    use std::os::unix::fs::symlink;

    let target = fs::read_link(source).map_err(|error| format!("{}: {error}", source.display()))?;
    symlink(&target, destination).map_err(|error| {
        format!(
            "failed to copy symlink {} to {}: {error}",
            source.display(),
            destination.display()
        )
    })
}

#[cfg(windows)]
fn copy_symlink(source: &Path, destination: &Path) -> Result<(), String> {
    use std::os::windows::fs::{symlink_dir, symlink_file};

    let target = fs::read_link(source).map_err(|error| format!("{}: {error}", source.display()))?;
    let metadata =
        fs::metadata(source).map_err(|error| format!("{}: {error}", source.display()))?;
    if metadata.is_dir() {
        symlink_dir(&target, destination)
    } else {
        symlink_file(&target, destination)
    }
    .map_err(|error| {
        format!(
            "failed to copy symlink {} to {}: {error}",
            source.display(),
            destination.display()
        )
    })
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
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
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

        let http_repo = GithubRepo::parse("http://github.com/anki-geo/ultimate-geography")
            .expect("GitHub HTTP repo URL parses");
        assert_eq!(http_repo.owner, "anki-geo");
        assert_eq!(http_repo.name, "ultimate-geography");

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
        let source = RequestedSource {
            source_type: "git".to_owned(),
            url: Some("https://example.com/anki-geo/ultimate-geography.git".to_owned()),
            path: None,
            reference: None,
            rev: Some("abc123".to_owned()),
        };

        let error = fetch_git_source(&source, Some("sha256-example"))
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
            assert!(should_skip_source_entry(name), "{name} should be skipped");
        }
        for name in ["deck.yaml", "results", "target-notes", ".github"] {
            assert!(!should_skip_source_entry(name), "{name} should be included");
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
            original: None,
            locked: LockedSource {
                source_type: "path".to_owned(),
                url: None,
                path: Some(".".to_owned()),
                reference: None,
                rev: None,
                nar_hash: None,
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
}
