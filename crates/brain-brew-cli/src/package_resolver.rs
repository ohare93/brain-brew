use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use brain_brew_formats::manifest;
use brain_brew_formats::package_semver;
use brain_brew_formats::safe_relative_path::SafeRelativePath;

pub(crate) const DEFAULT_DISCOVERY_MAX_DEPTH: usize = 32;
pub(crate) const DEFAULT_DISCOVERY_MAX_ENTRIES: usize = 100_000;
pub(crate) const DEFAULT_DISCOVERY_MAX_MANIFESTS: usize = 1_000;

const MAX_DISCOVERY_DEPTH_OVERRIDE: usize = 256;
const MAX_DISCOVERY_ENTRIES_OVERRIDE: usize = 10_000_000;
const MAX_DISCOVERY_MANIFESTS_OVERRIDE: usize = 100_000;

/// One validated policy for every recursive package-root walk.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DiscoveryPolicy {
    max_depth: usize,
    max_entries: usize,
    max_manifests: usize,
    depth_overridden: bool,
    entries_overridden: bool,
    manifests_overridden: bool,
    ignores: Vec<DiscoveryIgnore>,
}

impl Default for DiscoveryPolicy {
    fn default() -> Self {
        Self {
            max_depth: DEFAULT_DISCOVERY_MAX_DEPTH,
            max_entries: DEFAULT_DISCOVERY_MAX_ENTRIES,
            max_manifests: DEFAULT_DISCOVERY_MAX_MANIFESTS,
            depth_overridden: false,
            entries_overridden: false,
            manifests_overridden: false,
            ignores: Vec::new(),
        }
    }
}

impl DiscoveryPolicy {
    pub(crate) fn set_max_depth(&mut self, raw: &str) -> Result<(), String> {
        reject_duplicate_override("--discovery-max-depth", self.depth_overridden)?;
        self.max_depth = parse_limit("--discovery-max-depth", raw, MAX_DISCOVERY_DEPTH_OVERRIDE)?;
        self.depth_overridden = true;
        Ok(())
    }

    pub(crate) fn set_max_entries(&mut self, raw: &str) -> Result<(), String> {
        reject_duplicate_override("--discovery-max-entries", self.entries_overridden)?;
        self.max_entries = parse_limit(
            "--discovery-max-entries",
            raw,
            MAX_DISCOVERY_ENTRIES_OVERRIDE,
        )?;
        self.entries_overridden = true;
        Ok(())
    }

    pub(crate) fn set_max_manifests(&mut self, raw: &str) -> Result<(), String> {
        reject_duplicate_override("--discovery-max-manifests", self.manifests_overridden)?;
        self.max_manifests = parse_limit(
            "--discovery-max-manifests",
            raw,
            MAX_DISCOVERY_MANIFESTS_OVERRIDE,
        )?;
        self.manifests_overridden = true;
        Ok(())
    }

    pub(crate) fn add_ignore(&mut self, raw: &str) -> Result<(), String> {
        let safe = SafeRelativePath::new(raw).map_err(|error| {
            format!(
                "--package-ignore value {raw:?} is not a safe package-root-relative path/pattern: {error}"
            )
        })?;
        self.ignores.push(DiscoveryIgnore::new(safe));
        self.ignores.sort();
        self.ignores.dedup();
        Ok(())
    }
}

pub(crate) fn apply_discovery_option(
    flag: &str,
    value: &str,
    policy: &mut DiscoveryPolicy,
) -> Result<bool, String> {
    match flag {
        "--discovery-max-depth" => policy.set_max_depth(value)?,
        "--discovery-max-entries" => policy.set_max_entries(value)?,
        "--discovery-max-manifests" => policy.set_max_manifests(value)?,
        "--package-ignore" => policy.add_ignore(value)?,
        _ => return Ok(false),
    }
    Ok(true)
}

fn reject_duplicate_override(flag: &str, already_set: bool) -> Result<(), String> {
    if already_set {
        Err(format!("duplicate argument {flag:?}"))
    } else {
        Ok(())
    }
}

fn parse_limit(flag: &str, raw: &str, maximum: usize) -> Result<usize, String> {
    let value = raw
        .parse::<usize>()
        .map_err(|_| format!("{flag} requires a positive decimal integer, found {raw:?}"))?;
    if value == 0 {
        return Err(format!("{flag} must be greater than zero"));
    }
    if value > maximum {
        return Err(format!(
            "{flag} value {value} is effectively unbounded; maximum supported override is {maximum}"
        ));
    }
    Ok(value)
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct DiscoveryIgnore {
    display: String,
    components: Vec<String>,
}

impl DiscoveryIgnore {
    fn new(path: SafeRelativePath) -> Self {
        Self {
            display: path.as_str().to_owned(),
            components: path.as_str().split('/').map(str::to_owned).collect(),
        }
    }

    fn matches(&self, relative: &[String]) -> bool {
        match_components(&self.components, relative)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct DiscoveryStats {
    pub(crate) roots_inspected: usize,
    pub(crate) entries_inspected: usize,
    pub(crate) directories_inspected: usize,
    pub(crate) regular_files_inspected: usize,
    pub(crate) manifests_found: usize,
    pub(crate) built_in_pruned: usize,
    pub(crate) configured_pruned: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DiscoveryResult {
    pub(crate) manifests: Vec<PathBuf>,
    pub(crate) stats: DiscoveryStats,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DiscoveryBudget {
    Depth,
    Entries,
    Manifests,
}

impl DiscoveryBudget {
    fn name(self) -> &'static str {
        match self {
            Self::Depth => "depth",
            Self::Entries => "entries",
            Self::Manifests => "manifests",
        }
    }

    fn flag(self) -> &'static str {
        match self {
            Self::Depth => "--discovery-max-depth",
            Self::Entries => "--discovery-max-entries",
            Self::Manifests => "--discovery-max-manifests",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DiscoveryError {
    BudgetExceeded {
        budget: DiscoveryBudget,
        package_root: PathBuf,
        current_path: PathBuf,
        consumed: usize,
        limit: usize,
    },
    Filesystem {
        package_root: PathBuf,
        current_path: PathBuf,
        operation: &'static str,
        message: String,
    },
    RejectedEntry {
        package_root: PathBuf,
        current_path: PathBuf,
        kind: &'static str,
    },
    IdentityReuse {
        package_root: PathBuf,
        current_path: PathBuf,
        first_path: PathBuf,
    },
    NonUtf8Entry {
        package_root: PathBuf,
        current_path: PathBuf,
    },
}

impl fmt::Display for DiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BudgetExceeded {
                budget,
                package_root,
                current_path,
                consumed,
                limit,
            } => write!(
                formatter,
                "package discovery budget exceeded: budget={} package_root={} current_path={} consumed={} limit={}; raise {} explicitly after reviewing the package root",
                budget.name(),
                package_root.display(),
                current_path.display(),
                consumed,
                limit,
                budget.flag()
            ),
            Self::Filesystem {
                package_root,
                current_path,
                operation,
                message,
            } => write!(
                formatter,
                "package discovery failed closed: package_root={} current_path={} operation={operation}: {message}",
                package_root.display(),
                current_path.display()
            ),
            Self::RejectedEntry {
                package_root,
                current_path,
                kind,
            } => write!(
                formatter,
                "package discovery rejected {kind} {} under package root {}; discovery never follows links and accepts only regular files/directories outside pruned trees",
                current_path.display(),
                package_root.display()
            ),
            Self::IdentityReuse {
                package_root,
                current_path,
                first_path,
            } => write!(
                formatter,
                "package discovery rejected reused directory identity at {} under package root {}; it was already inspected as {} (possible alias, replacement, or filesystem cycle)",
                current_path.display(),
                package_root.display(),
                first_path.display()
            ),
            Self::NonUtf8Entry {
                package_root,
                current_path,
            } => write!(
                formatter,
                "package discovery rejected non-UTF-8 entry under package root {} while inspecting {}; package paths must have deterministic portable names",
                package_root.display(),
                current_path.display()
            ),
        }
    }
}

impl std::error::Error for DiscoveryError {}

/// Discover manifests once for the registry planner, in stable root/relative-path order.
pub(crate) fn discover_package_manifests(
    roots: &[PathBuf],
    policy: &DiscoveryPolicy,
) -> Result<DiscoveryResult, DiscoveryError> {
    let mut traversal_roots = Vec::new();
    for requested in roots {
        let authorized = authorize_package_root(requested)?;
        traversal_roots.push((authorized, requested.clone()));
    }
    traversal_roots.sort();

    let mut state = DiscoveryState {
        policy,
        stats: DiscoveryStats::default(),
        manifests: Vec::new(),
        directory_identities: BTreeMap::new(),
    };
    for (authorized, requested) in traversal_roots {
        state.stats.roots_inspected += 1;
        state.inspect_entry_budget(&requested, &authorized)?;
        state.walk_directory(&requested, &authorized, &authorized, &[], 0)?;
    }
    state.manifests.sort_by(|left, right| {
        left.root
            .cmp(&right.root)
            .then_with(|| left.relative.cmp(&right.relative))
    });
    Ok(DiscoveryResult {
        manifests: state
            .manifests
            .into_iter()
            .map(|manifest| manifest.path)
            .collect(),
        stats: state.stats,
    })
}

fn authorize_package_root(requested: &Path) -> Result<PathBuf, DiscoveryError> {
    let absolute = std::path::absolute(requested).map_err(|error| DiscoveryError::Filesystem {
        package_root: requested.to_path_buf(),
        current_path: requested.to_path_buf(),
        operation: "make package root absolute",
        message: error.to_string(),
    })?;
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        use std::path::Component;
        match component {
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(DiscoveryError::Filesystem {
                        package_root: requested.to_path_buf(),
                        current_path: absolute,
                        operation: "normalize package root",
                        message: "parent component escapes the filesystem root".to_owned(),
                    });
                }
            }
        }
    }

    let mut current = PathBuf::new();
    for component in normalized.components() {
        current.push(component.as_os_str());
        let metadata =
            fs::symlink_metadata(&current).map_err(|error| DiscoveryError::Filesystem {
                package_root: requested.to_path_buf(),
                current_path: current.clone(),
                operation: "inspect package root component",
                message: error.to_string(),
            })?;
        if metadata.file_type().is_symlink() {
            return Err(DiscoveryError::RejectedEntry {
                package_root: requested.to_path_buf(),
                current_path: current,
                kind: "symlink",
            });
        }
    }
    let metadata =
        fs::symlink_metadata(&normalized).map_err(|error| DiscoveryError::Filesystem {
            package_root: requested.to_path_buf(),
            current_path: normalized.clone(),
            operation: "inspect package root",
            message: error.to_string(),
        })?;
    let kind = file_kind(&metadata);
    if kind != "directory" {
        return Err(DiscoveryError::RejectedEntry {
            package_root: requested.to_path_buf(),
            current_path: normalized,
            kind,
        });
    }
    Ok(normalized)
}

struct DiscoveredManifest {
    root: PathBuf,
    relative: String,
    path: PathBuf,
}

struct DiscoveryState<'a> {
    policy: &'a DiscoveryPolicy,
    stats: DiscoveryStats,
    manifests: Vec<DiscoveredManifest>,
    directory_identities: BTreeMap<FileIdentity, PathBuf>,
}

impl DiscoveryState<'_> {
    fn walk_directory(
        &mut self,
        package_root: &Path,
        canonical_root: &Path,
        directory: &Path,
        relative: &[String],
        depth: usize,
    ) -> Result<(), DiscoveryError> {
        if depth > self.policy.max_depth {
            return Err(self.budget_error(
                DiscoveryBudget::Depth,
                package_root,
                directory,
                depth,
                self.policy.max_depth,
            ));
        }
        let before = self.metadata(package_root, directory, "inspect directory")?;
        let kind = file_kind(&before);
        if kind != "directory" {
            return Err(DiscoveryError::RejectedEntry {
                package_root: package_root.to_path_buf(),
                current_path: directory.to_path_buf(),
                kind,
            });
        }
        let identity = file_identity(directory, &before);
        if let Some(first_path) = self
            .directory_identities
            .insert(identity, directory.to_path_buf())
        {
            return Err(DiscoveryError::IdentityReuse {
                package_root: package_root.to_path_buf(),
                current_path: directory.to_path_buf(),
                first_path,
            });
        }
        self.stats.directories_inspected += 1;

        let iterator = fs::read_dir(directory).map_err(|error| DiscoveryError::Filesystem {
            package_root: package_root.to_path_buf(),
            current_path: directory.to_path_buf(),
            operation: "read directory",
            message: error.to_string(),
        })?;
        let mut entries = Vec::new();
        for entry in iterator {
            let entry = entry.map_err(|error| DiscoveryError::Filesystem {
                package_root: package_root.to_path_buf(),
                current_path: directory.to_path_buf(),
                operation: "read directory entry",
                message: error.to_string(),
            })?;
            let path = entry.path();
            self.inspect_entry_budget(package_root, &path)?;
            let name =
                entry
                    .file_name()
                    .into_string()
                    .map_err(|_| DiscoveryError::NonUtf8Entry {
                        package_root: package_root.to_path_buf(),
                        current_path: path.clone(),
                    })?;
            entries.push((name, path));
        }
        entries.sort_by(|left, right| left.0.cmp(&right.0));

        let after = self.metadata(package_root, directory, "recheck directory")?;
        if file_kind(&after) != "directory" || file_identity(directory, &after) != identity {
            return Err(DiscoveryError::Filesystem {
                package_root: package_root.to_path_buf(),
                current_path: directory.to_path_buf(),
                operation: "recheck directory identity",
                message: "directory was replaced during traversal".to_owned(),
            });
        }

        for (name, path) in entries {
            let mut child_relative = relative.to_vec();
            child_relative.push(name.clone());
            if built_in_ignore(&name) {
                self.stats.built_in_pruned += 1;
                continue;
            }
            if self
                .policy
                .ignores
                .iter()
                .any(|ignore| ignore.matches(&child_relative))
            {
                self.stats.configured_pruned += 1;
                continue;
            }

            let metadata = self.metadata(package_root, &path, "inspect entry")?;
            match file_kind(&metadata) {
                "directory" => self.walk_directory(
                    package_root,
                    canonical_root,
                    &path,
                    &child_relative,
                    depth + 1,
                )?,
                "regular file" => {
                    self.stats.regular_files_inspected += 1;
                    if name == "brainbrew.yaml" {
                        self.stats.manifests_found += 1;
                        if self.stats.manifests_found > self.policy.max_manifests {
                            return Err(self.budget_error(
                                DiscoveryBudget::Manifests,
                                package_root,
                                &path,
                                self.stats.manifests_found,
                                self.policy.max_manifests,
                            ));
                        }
                        let relative = child_relative.join("/");
                        // The manifest result crosses into package path authorization, so require
                        // the same portable relative syntax used by manifest-owned paths.
                        SafeRelativePath::new(&relative).map_err(|error| {
                            DiscoveryError::Filesystem {
                                package_root: package_root.to_path_buf(),
                                current_path: path.clone(),
                                operation: "authorize discovered manifest path",
                                message: error.to_string(),
                            }
                        })?;
                        self.manifests.push(DiscoveredManifest {
                            root: canonical_root.to_path_buf(),
                            relative,
                            path,
                        });
                    }
                }
                kind => {
                    return Err(DiscoveryError::RejectedEntry {
                        package_root: package_root.to_path_buf(),
                        current_path: path,
                        kind,
                    });
                }
            }
        }
        Ok(())
    }

    fn inspect_entry_budget(
        &mut self,
        package_root: &Path,
        current_path: &Path,
    ) -> Result<(), DiscoveryError> {
        self.stats.entries_inspected += 1;
        if self.stats.entries_inspected > self.policy.max_entries {
            return Err(self.budget_error(
                DiscoveryBudget::Entries,
                package_root,
                current_path,
                self.stats.entries_inspected,
                self.policy.max_entries,
            ));
        }
        Ok(())
    }

    fn metadata(
        &self,
        package_root: &Path,
        path: &Path,
        operation: &'static str,
    ) -> Result<fs::Metadata, DiscoveryError> {
        fs::symlink_metadata(path).map_err(|error| DiscoveryError::Filesystem {
            package_root: package_root.to_path_buf(),
            current_path: path.to_path_buf(),
            operation,
            message: error.to_string(),
        })
    }

    fn budget_error(
        &self,
        budget: DiscoveryBudget,
        package_root: &Path,
        current_path: &Path,
        consumed: usize,
        limit: usize,
    ) -> DiscoveryError {
        DiscoveryError::BudgetExceeded {
            budget,
            package_root: package_root.to_path_buf(),
            current_path: current_path.to_path_buf(),
            consumed,
            limit,
        }
    }
}

fn built_in_ignore(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | ".jj"
            | ".hg"
            | ".svn"
            | ".devenv"
            | ".direnv"
            | "target"
            | "build"
            | "output"
            | "outputs"
            | "dist"
            | "site"
            | "_site"
            | "node_modules"
            | ".docusaurus"
            | ".cache"
            | ".brainbrew-cache"
            | ".brainbrew-transactions"
            | "result"
    ) || name.starts_with("result-")
        || (name.starts_with(".brainbrew-")
            && (name.ends_with(".stage") || name.ends_with(".backup")))
}

fn match_components(pattern: &[String], path: &[String]) -> bool {
    match pattern.split_first() {
        None => path.is_empty(),
        Some((head, tail)) if head == "**" => {
            match_components(tail, path)
                || (!path.is_empty() && match_components(pattern, &path[1..]))
        }
        Some((head, tail)) => {
            let Some((candidate, rest)) = path.split_first() else {
                return false;
            };
            component_glob_matches(head, candidate) && match_components(tail, rest)
        }
    }
}

fn component_glob_matches(pattern: &str, candidate: &str) -> bool {
    let pattern = pattern.chars().collect::<Vec<_>>();
    let candidate = candidate.chars().collect::<Vec<_>>();
    let mut matches = vec![vec![false; candidate.len() + 1]; pattern.len() + 1];
    matches[0][0] = true;
    for pattern_index in 0..pattern.len() {
        for candidate_index in 0..=candidate.len() {
            if !matches[pattern_index][candidate_index] {
                continue;
            }
            match pattern[pattern_index] {
                '*' => {
                    matches[pattern_index + 1][candidate_index] = true;
                    if candidate_index < candidate.len() {
                        matches[pattern_index][candidate_index + 1] = true;
                    }
                }
                '?' if candidate_index < candidate.len() => {
                    matches[pattern_index + 1][candidate_index + 1] = true;
                }
                literal
                    if candidate_index < candidate.len()
                        && literal == candidate[candidate_index] =>
                {
                    matches[pattern_index + 1][candidate_index + 1] = true;
                }
                _ => {}
            }
        }
    }
    matches[pattern.len()][candidate.len()]
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FileIdentity(u64, u64);

#[cfg(unix)]
fn file_identity(_path: &Path, metadata: &fs::Metadata) -> FileIdentity {
    use std::os::unix::fs::MetadataExt;
    FileIdentity(metadata.dev(), metadata.ino())
}

#[cfg(not(unix))]
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FileIdentity(PathBuf);

#[cfg(not(unix))]
fn file_identity(path: &Path, _metadata: &fs::Metadata) -> FileIdentity {
    FileIdentity(path.to_path_buf())
}

fn file_kind(metadata: &fs::Metadata) -> &'static str {
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        "symlink"
    } else if file_type.is_dir() {
        "directory"
    } else if file_type.is_file() {
        "regular file"
    } else {
        special_file_kind(&file_type)
    }
}

#[cfg(unix)]
fn special_file_kind(file_type: &fs::FileType) -> &'static str {
    use std::os::unix::fs::FileTypeExt;
    if file_type.is_socket() {
        "unsupported socket"
    } else if file_type.is_fifo() {
        "unsupported fifo"
    } else if file_type.is_char_device() || file_type.is_block_device() {
        "unsupported device"
    } else {
        "unsupported special entry"
    }
}

#[cfg(not(unix))]
fn special_file_kind(_file_type: &fs::FileType) -> &'static str {
    "unsupported special entry"
}

pub(crate) fn validate_package_dependencies(
    packages: &[(&PathBuf, &manifest::FederatedDeckManifest)],
) -> Result<(), String> {
    let mut by_id = BTreeMap::new();
    for (path, manifest) in packages {
        let Some(package) = &manifest.package else {
            continue;
        };
        if let Some(previous) = by_id.insert(package.id.clone(), (*path, package)) {
            let conflict = if previous.1.version == package.version {
                "duplicate package identity"
            } else {
                "conflicting package versions"
            };
            return Err(format!(
                "{conflict} {}: {} in {} and {} in {}",
                package.id,
                previous.1.version,
                previous.0.display(),
                package.version,
                path.display()
            ));
        }
    }

    let mut graph = BTreeMap::<String, Vec<DependencyEdge>>::new();
    for (path, manifest) in packages {
        let Some(package) = &manifest.package else {
            continue;
        };
        let mut dependencies = Vec::new();
        for declaration in &package.depends_on {
            let dependency = package_semver::parse_exact_dependency(declaration).map_err(|reason| {
                format!(
                    "invalid package dependency {declaration:?} declared by {}@{} in {}: {reason}",
                    package.id,
                    package.version,
                    path.display()
                )
            })?;
            let Some((dependency_path, dependency_package)) = by_id.get(&dependency.id) else {
                return Err(format!(
                    "package dependency {}@{} required by {}@{} in {} was not found",
                    dependency.id,
                    dependency.version,
                    package.id,
                    package.version,
                    path.display()
                ));
            };
            if dependency_package.version != dependency.version {
                return Err(format!(
                    "package dependency {}@{} required by {}@{} in {} resolved to version {} in {}",
                    dependency.id,
                    dependency.version,
                    package.id,
                    package.version,
                    path.display(),
                    dependency_package.version,
                    dependency_path.display()
                ));
            }
            dependencies.push(DependencyEdge {
                target: dependency.id,
                declaration: declaration.clone(),
            });
        }

        if let Some(base_package) = &package.base_package {
            let Some((base_path, base)) = by_id.get(base_package) else {
                // This should also be diagnosed by the exact dependency pass,
                // but retain a specific fail-closed guard for constructed data.
                return Err(format!(
                    "base package {base_package} required by {}@{} in {} was not found",
                    package.id,
                    package.version,
                    path.display()
                ));
            };
            let compatible = package_semver::requirements_match(
                &package.compatible_base_versions,
                &base.version,
            )
            .map_err(|error| {
                format!(
                    "invalid compatible_base_versions declared by {}@{} in {}: {error}",
                    package.id,
                    package.version,
                    path.display()
                )
            })?;
            if !compatible {
                return Err(format!(
                    "base package {}@{} in {} is incompatible with {}@{} in {}; compatible_base_versions requires one of [{}]",
                    base.id,
                    base.version,
                    base_path.display(),
                    package.id,
                    package.version,
                    path.display(),
                    package.compatible_base_versions.join(" OR ")
                ));
            }
        }
        graph.insert(package.id.clone(), dependencies);
    }

    let mut complete = BTreeSet::new();
    let mut active = Vec::new();
    for package in graph.keys() {
        visit_package(package, &graph, &by_id, &mut complete, &mut active)?;
    }
    Ok(())
}

#[derive(Clone)]
struct DependencyEdge {
    target: String,
    declaration: String,
}

fn visit_package(
    package: &str,
    graph: &BTreeMap<String, Vec<DependencyEdge>>,
    packages: &BTreeMap<String, (&PathBuf, &manifest::PackageMetadata)>,
    complete: &mut BTreeSet<String>,
    active: &mut Vec<(String, Option<String>)>,
) -> Result<(), String> {
    if complete.contains(package) {
        return Ok(());
    }
    if let Some(index) = active
        .iter()
        .position(|(candidate, _)| candidate == package)
    {
        let cycle = &active[index..];
        let mut trace = String::from("package dependency cycle:");
        for (id, incoming) in cycle {
            let (path, metadata) = packages[id];
            trace.push_str(&format!(
                "\n  {}@{} ({})",
                metadata.id,
                metadata.version,
                path.display()
            ));
            let edge = incoming
                .as_ref()
                .expect("a dependency cycle has an outgoing edge");
            trace.push_str(&format!(" --depends_on {edge}-->"));
        }
        let (path, metadata) = packages[package];
        trace.push_str(&format!(
            "\n  {}@{} ({})",
            metadata.id,
            metadata.version,
            path.display()
        ));
        return Err(trace);
    }
    active.push((package.to_owned(), None));
    for dependency in graph.get(package).into_iter().flatten() {
        if let Some(last) = active.last_mut() {
            last.1 = Some(dependency.declaration.clone());
        }
        visit_package(&dependency.target, graph, packages, complete, active)?;
    }
    active.pop();
    complete.insert(package.to_owned());
    Ok(())
}

#[cfg(test)]
mod discovery_tests {
    use super::*;

    #[test]
    fn moderate_tree_has_stable_results_and_exact_visit_prune_counts() {
        let temp = tempfile::tempdir().unwrap();
        let package = temp.path().join("package");
        fs::create_dir_all(&package).unwrap();
        fs::write(package.join("brainbrew.yaml"), "base: deck.yaml\n").unwrap();
        for index in 0..200 {
            fs::write(package.join(format!("visible-{index:03}.txt")), "visible").unwrap();
        }
        let generated = temp.path().join("target/generated");
        fs::create_dir_all(&generated).unwrap();
        for index in 0..500 {
            fs::write(generated.join(format!("ignored-{index:03}.txt")), "ignored").unwrap();
        }

        let roots = vec![temp.path().to_path_buf()];
        let first = discover_package_manifests(&roots, &DiscoveryPolicy::default()).unwrap();
        let second = discover_package_manifests(&roots, &DiscoveryPolicy::default()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.manifests, vec![package.join("brainbrew.yaml")]);
        assert_eq!(first.stats.roots_inspected, 1);
        assert_eq!(first.stats.entries_inspected, 204);
        assert_eq!(first.stats.directories_inspected, 2);
        assert_eq!(first.stats.regular_files_inspected, 201);
        assert_eq!(first.stats.manifests_found, 1);
        assert_eq!(first.stats.built_in_pruned, 1);
        assert_eq!(first.stats.configured_pruned, 0);
    }

    #[test]
    fn configured_globs_match_whole_components_and_builtins_take_precedence() {
        let mut policy = DiscoveryPolicy::default();
        policy.add_ignore("vendor/**/generated-?").unwrap();
        policy.add_ignore(".git/**").unwrap();
        let patterns = &policy.ignores;
        assert!(patterns[1].matches(&[
            "vendor".to_owned(),
            "nested".to_owned(),
            "generated-a".to_owned(),
        ]));
        assert!(!patterns[1].matches(&[
            "vendor".to_owned(),
            "nested".to_owned(),
            "not-generated-a".to_owned(),
        ]));
        assert!(!built_in_ignore("my-target"));
        assert!(!built_in_ignore("builder"));
        assert!(built_in_ignore("target"));
        assert!(built_in_ignore("build"));

        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join(".git/hidden")).unwrap();
        let result = discover_package_manifests(&[temp.path().to_path_buf()], &policy).unwrap();
        assert_eq!(result.stats.built_in_pruned, 1);
        assert_eq!(result.stats.configured_pruned, 0);
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_candidate_tree_fails_closed_when_permissions_are_enforced() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let unreadable = temp.path().join("unreadable");
        fs::create_dir(&unreadable).unwrap();
        fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o000)).unwrap();
        let probe = fs::read_dir(&unreadable);
        if probe.is_ok() {
            // Privileged CI users can bypass mode bits; the special-entry test
            // still exercises the portable fail-closed classification path.
            fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o700)).unwrap();
            return;
        }
        let error =
            discover_package_manifests(&[temp.path().to_path_buf()], &DiscoveryPolicy::default())
                .unwrap_err();
        fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o700)).unwrap();
        let message = error.to_string();
        assert!(message.contains("failed closed"), "{message}");
        assert!(message.contains("read directory"), "{message}");
        assert!(message.contains("unreadable"), "{message}");
    }
}
