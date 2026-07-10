use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use brain_brew_core::{CanonicalDeck, Overlay};
use brain_brew_formats::canonical_source_document::CanonicalSourceDocument;
use brain_brew_formats::overlay_source_document::OverlaySourceDocument;
use brain_brew_formats::source_document::{IncludeRequest, SourceFile, SourceProvenance};
use brain_brew_formats::{canonical_yaml, lockfile, manifest, media_map, source_includes};
use serde_json::json;
use serde_yaml::Value;

use crate::commands::lock::locked_package_manifest_paths;
use crate::output::package_json;
use crate::package_resolver::{discover_package_manifests, validate_package_dependencies};

pub(crate) fn format_source(input: &str) -> Result<String, String> {
    let mut errors = Vec::new();
    match source_includes::format_preserving_file_includes(input, canonical_yaml::format_str) {
        Ok(formatted) => return Ok(formatted),
        Err(error) => errors.push(format!("deck: {error}")),
    }
    match source_includes::format_preserving_file_includes(
        input,
        canonical_yaml::overlay_format_str,
    ) {
        Ok(formatted) => return Ok(formatted),
        Err(error) => errors.push(format!("overlay: {error}")),
    }
    match manifest::format_str(input) {
        Ok(formatted) => return Ok(formatted),
        Err(error) => errors.push(format!("manifest: {error}")),
    }
    match lockfile::format_str(input) {
        Ok(formatted) => return Ok(formatted),
        Err(error) => errors.push(format!("lockfile: {error}")),
    }
    match media_map::format_str(input) {
        Ok(formatted) => return Ok(formatted),
        Err(error) => errors.push(format!("media map: {error}")),
    }
    Err(format!(
        "unrecognized Brain Brew source file ({})",
        errors.join("; ")
    ))
}

pub(crate) fn format_source_at(_path: &Path, input: &str) -> Result<String, String> {
    format_source(input)
}

pub(crate) fn canonical_source_document(
    path: &Path,
    input: &str,
) -> Result<CanonicalSourceDocument, String> {
    let context = source_context_for_path(path)?;
    CanonicalSourceDocument::parse_with_includes(source_file(path, input, &context)?, |request| {
        load_source_include(request, &context)
    })
    .map_err(|error| error.to_string())
}

pub(crate) fn overlay_source_document(
    path: &Path,
    input: &str,
) -> Result<OverlaySourceDocument, String> {
    let context = source_context_for_path(path)?;
    OverlaySourceDocument::parse_with_includes(source_file(path, input, &context)?, |request| {
        load_source_include(request, &context)
    })
    .map_err(|error| error.to_string())
}

fn source_file(path: &Path, input: &str, context: &SourceContext) -> Result<SourceFile, String> {
    let absolute =
        fs::canonicalize(path).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(SourceFile::new(
        SourceProvenance::new(absolute.display().to_string())
            .with_source_root(context.root.display().to_string()),
        input,
    ))
}

fn load_source_include(
    request: &IncludeRequest,
    context: &SourceContext,
) -> Result<SourceFile, String> {
    let path = resolve_include_target_for_context(request.target(), context)?;
    let absolute =
        fs::canonicalize(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let text = fs::read_to_string(&absolute)
        .map_err(|error| format!("{}: {error}", absolute.display()))?;
    Ok(SourceFile::new(
        SourceProvenance::new(absolute.display().to_string())
            .with_source_root(context.root.display().to_string()),
        text,
    ))
}

pub(crate) fn read_deck(path: &Path) -> Result<CanonicalDeck, String> {
    let context = source_context_for_path(path)?;
    read_deck_with_context(path, &context)
}

fn read_deck_from_package(
    path: &Path,
    package_root: &Path,
    include_roots: &[PathBuf],
) -> Result<CanonicalDeck, String> {
    read_deck_with_context(
        path,
        &SourceContext {
            root: package_root.to_path_buf(),
            include_roots: include_roots.to_vec(),
        },
    )
}

fn read_deck_with_context(path: &Path, context: &SourceContext) -> Result<CanonicalDeck, String> {
    let input = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let input = resolve_source_includes(&input, path, context)?;
    canonical_yaml::from_str(&input).map_err(|error| format!("{}: {error}", path.display()))
}

fn read_overlay_from_package(
    path: &Path,
    package_root: &Path,
    include_roots: &[PathBuf],
) -> Result<Overlay, String> {
    read_overlay_with_context(
        path,
        &SourceContext {
            root: package_root.to_path_buf(),
            include_roots: include_roots.to_vec(),
        },
    )
}

fn read_overlay_with_context(path: &Path, context: &SourceContext) -> Result<Overlay, String> {
    let input = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    overlay_from_source_text_with_context(path, &input, context)
}

pub(crate) fn overlay_from_source_text(path: &Path, input: &str) -> Result<Overlay, String> {
    let context = source_context_for_path(path)?;
    overlay_from_source_text_with_context(path, input, &context)
}

fn overlay_from_source_text_with_context(
    path: &Path,
    input: &str,
    context: &SourceContext,
) -> Result<Overlay, String> {
    let input = resolve_source_includes(input, path, context)?;
    canonical_yaml::overlay_from_str(&input).map_err(|error| format!("{}: {error}", path.display()))
}

pub(crate) fn read_manifest(path: &Path) -> Result<manifest::FederatedDeckManifest, String> {
    let input = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    manifest::from_str(&input).map_err(|error| format!("{}: {error}", path.display()))
}

pub(crate) fn read_deck_and_overlays(
    deck_path: &Path,
    overlay_paths: &[String],
) -> Result<(CanonicalDeck, Vec<(String, Overlay)>), String> {
    let context = source_context_for_path(deck_path)?;
    let deck = read_deck_with_context(deck_path, &context)?;
    let overlays = overlay_paths
        .iter()
        .map(|path| {
            Ok((
                path.clone(),
                read_overlay_with_context(Path::new(path), &context)?,
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok((deck, overlays))
}

pub(crate) fn read_and_compose_deck(
    deck_path: &Path,
    overlay_paths: &[String],
) -> Result<CanonicalDeck, String> {
    let (deck, overlays) = read_deck_and_overlays(deck_path, overlay_paths)?;
    deck.compose(
        &overlays
            .into_iter()
            .map(|(_, overlay)| overlay)
            .collect::<Vec<_>>(),
    )
    .map_err(|error| error.to_string())
}

pub(crate) fn read_and_compose_manifest_target_with_packages(
    manifest_path: &Path,
    target: &str,
    include_paths: &[PathBuf],
    package_roots: &[PathBuf],
) -> Result<CanonicalDeck, String> {
    let plan =
        plan_manifest_target_with_packages(manifest_path, target, include_paths, package_roots)?;
    plan.compose()
}

#[derive(Clone)]
pub(crate) struct PlannedOverlay {
    pub(crate) id: String,
    pub(crate) file: PathBuf,
    pub(crate) display_file: String,
    pub(crate) package_root: PathBuf,
    pub(crate) include_roots: Vec<PathBuf>,
}

pub(crate) struct ManifestTargetPlan {
    pub(crate) package: Option<manifest::PackageMetadata>,
    pub(crate) target: String,
    pub(crate) base_label: String,
    pub(crate) base: CanonicalDeck,
    pub(crate) overlays: Vec<(PlannedOverlay, Overlay)>,
}

impl ManifestTargetPlan {
    pub(crate) fn compose(&self) -> Result<CanonicalDeck, String> {
        self.base
            .compose(
                &self
                    .overlays
                    .iter()
                    .map(|(_, overlay)| overlay.clone())
                    .collect::<Vec<_>>(),
            )
            .map_err(|error| error.to_string())
    }
}

pub(crate) fn plan_manifest_target_with_packages(
    manifest_path: &Path,
    target: &str,
    include_paths: &[PathBuf],
    package_roots: &[PathBuf],
) -> Result<ManifestTargetPlan, String> {
    let registry = ManifestRegistry::load(manifest_path, include_paths, package_roots)?;
    registry.plan_root_target(target)
}

struct LoadedManifest {
    path: PathBuf,
    root: PathBuf,
    include_roots: Vec<PathBuf>,
    manifest: manifest::FederatedDeckManifest,
}

struct ManifestRegistry {
    root_index: usize,
    manifests: Vec<LoadedManifest>,
    packages: BTreeMap<String, usize>,
}

impl ManifestRegistry {
    fn load(
        manifest_path: &Path,
        include_paths: &[PathBuf],
        package_roots: &[PathBuf],
    ) -> Result<Self, String> {
        let mut paths = vec![manifest_path.to_path_buf()];
        paths.extend(include_paths.iter().cloned());
        paths.extend(discover_package_manifests(package_roots)?);
        paths.sort();
        paths.dedup();

        let lock_path = manifest_root(manifest_path).join("brainbrew.lock");
        let locked_paths = locked_package_manifest_paths(&lock_path)?;
        let has_locked_paths = !locked_paths.is_empty();
        paths.extend(locked_paths);
        paths.sort();
        paths.dedup();

        let root_path = manifest_path.to_path_buf();
        let root_index = paths
            .iter()
            .position(|path| path == &root_path)
            .unwrap_or(0);
        let manifests = paths
            .iter()
            .map(|path| {
                let manifest = read_manifest(path)?;
                let root = manifest_root(path);
                let include_roots = include_roots_from_manifest(&root, &manifest);
                Ok(LoadedManifest {
                    path: path.clone(),
                    root,
                    include_roots,
                    manifest,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        if !package_roots.is_empty() || has_locked_paths {
            validate_package_dependencies(
                &manifests
                    .iter()
                    .map(|loaded| (&loaded.path, &loaded.manifest))
                    .collect::<Vec<_>>(),
            )?;
        }

        let mut packages = BTreeMap::new();
        for (index, loaded) in manifests.iter().enumerate() {
            let Some(package) = &loaded.manifest.package else {
                continue;
            };
            if let Some(previous) = packages.insert(package.id.clone(), index) {
                return Err(format!(
                    "duplicate package id {} in {} and {}",
                    package.id,
                    manifests[previous].path.display(),
                    loaded.path.display()
                ));
            }
        }

        Ok(Self {
            root_index,
            manifests,
            packages,
        })
    }

    fn plan_root_target(&self, target: &str) -> Result<ManifestTargetPlan, String> {
        self.plan_target(self.root_index, target, &mut Vec::new())
    }

    fn plan_target(
        &self,
        manifest_index: usize,
        target: &str,
        stack: &mut Vec<(usize, String)>,
    ) -> Result<ManifestTargetPlan, String> {
        if stack
            .iter()
            .any(|(index, name)| *index == manifest_index && name == target)
        {
            return Err(format!("manifest target dependency cycle at {target}"));
        }
        stack.push((manifest_index, target.to_owned()));

        let loaded = &self.manifests[manifest_index];
        let target_entry = loaded.manifest.targets.get(target).ok_or_else(|| {
            format!(
                "manifest target {target:?} does not exist in {}; available targets: {}",
                loaded.path.display(),
                loaded
                    .manifest
                    .targets
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;

        let (base_label, base) = if let Some(extends) = &target_entry.extends {
            let (base_manifest_index, base_target) =
                self.resolve_target_ref(manifest_index, extends)?;
            let base_plan = self.plan_target(base_manifest_index, base_target, stack)?;
            (extends.clone(), base_plan.compose()?)
        } else {
            (
                loaded.manifest.base.clone(),
                read_deck_from_package(
                    &loaded.root.join(&loaded.manifest.base),
                    &loaded.root,
                    &loaded.include_roots,
                )?,
            )
        };

        let planned_overlays = self.expand_target_overlays(manifest_index, target)?;
        let overlays = planned_overlays
            .into_iter()
            .map(|planned| {
                let overlay = read_overlay_from_package(
                    &planned.file,
                    &planned.package_root,
                    &planned.include_roots,
                )?;
                Ok((planned, overlay))
            })
            .collect::<Result<Vec<_>, String>>()?;

        stack.pop();
        Ok(ManifestTargetPlan {
            package: loaded.manifest.package.clone(),
            target: target.to_owned(),
            base_label,
            base,
            overlays,
        })
    }

    fn expand_target_overlays(
        &self,
        manifest_index: usize,
        target: &str,
    ) -> Result<Vec<PlannedOverlay>, String> {
        let loaded = &self.manifests[manifest_index];
        let target_entry = loaded
            .manifest
            .targets
            .get(target)
            .ok_or_else(|| format!("manifest target {target:?} does not exist"))?;
        let mut visited = BTreeSet::new();
        let mut stack = Vec::new();
        let mut expanded = Vec::new();
        for overlay in &target_entry.overlays {
            self.visit_overlay_ref(
                manifest_index,
                overlay,
                &mut visited,
                &mut stack,
                &mut expanded,
            )?;
        }
        Ok(expanded)
    }

    fn visit_overlay_ref(
        &self,
        current_manifest_index: usize,
        overlay_ref: &str,
        visited: &mut BTreeSet<(usize, String)>,
        stack: &mut Vec<(usize, String)>,
        expanded: &mut Vec<PlannedOverlay>,
    ) -> Result<(), String> {
        let (manifest_index, overlay_id) =
            self.resolve_overlay_ref(current_manifest_index, overlay_ref)?;
        let key = (manifest_index, overlay_id.to_owned());
        if visited.contains(&key) {
            return Ok(());
        }
        if stack.contains(&key) {
            return Err(format!(
                "manifest overlay dependency cycle at {overlay_ref}"
            ));
        }
        let loaded = &self.manifests[manifest_index];
        let entry = loaded.manifest.overlays.get(overlay_id).ok_or_else(|| {
            format!(
                "manifest overlay {overlay_id:?} does not exist in {}",
                loaded.path.display()
            )
        })?;
        stack.push(key.clone());
        for dependency in &entry.depends_on {
            self.visit_overlay_ref(manifest_index, dependency, visited, stack, expanded)?;
        }
        stack.pop();

        visited.insert(key);
        let should_qualify = manifest_index != self.root_index;
        let display_file = if should_qualify {
            if let Some(package) = &loaded.manifest.package {
                format!("{}:{}", package.id, entry.file)
            } else {
                entry.file.clone()
            }
        } else {
            entry.file.clone()
        };
        let id = if should_qualify {
            if let Some(package) = &loaded.manifest.package {
                format!("{}:{}", package.id, overlay_id)
            } else {
                overlay_id.to_owned()
            }
        } else {
            overlay_id.to_owned()
        };
        expanded.push(PlannedOverlay {
            id,
            file: loaded.root.join(&entry.file),
            display_file,
            package_root: loaded.root.clone(),
            include_roots: loaded.include_roots.clone(),
        });
        Ok(())
    }

    fn resolve_target_ref<'a>(
        &self,
        current_manifest_index: usize,
        target_ref: &'a str,
    ) -> Result<(usize, &'a str), String> {
        if let Some((package_id, target)) = target_ref.split_once(':') {
            let Some(index) = self.packages.get(package_id) else {
                return Err(format!(
                    "package {package_id:?} required by target ref {target_ref:?} was not included"
                ));
            };
            Ok((*index, target))
        } else {
            Ok((current_manifest_index, target_ref))
        }
    }

    fn resolve_overlay_ref<'a>(
        &self,
        current_manifest_index: usize,
        overlay_ref: &'a str,
    ) -> Result<(usize, &'a str), String> {
        if let Some((package_id, overlay_id)) = overlay_ref.split_once(':') {
            let Some(index) = self.packages.get(package_id) else {
                return Err(format!(
                    "package {package_id:?} required by overlay ref {overlay_ref:?} was not included"
                ));
            };
            Ok((*index, overlay_id))
        } else {
            Ok((current_manifest_index, overlay_ref))
        }
    }
}

pub(crate) fn target_package_json(
    path: &Path,
    manifest: &manifest::FederatedDeckManifest,
) -> Result<serde_json::Value, String> {
    let targets = manifest
        .targets
        .keys()
        .map(|target| {
            let expanded = expand_manifest_target(manifest, target)?;
            let overlays = expanded
                .overlays
                .iter()
                .map(|overlay| json!({"id": overlay.id, "file": overlay.file}))
                .collect::<Vec<_>>();
            let qualified_name = manifest
                .package
                .as_ref()
                .map(|package| format!("{}:{target}", package.id));
            Ok(json!({
                "name": target,
                "qualified_name": qualified_name,
                "extends": manifest.targets[target].extends.as_ref(),
                "overlays": overlays,
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(json!({
        "manifest": path.display().to_string(),
        "package": manifest.package.as_ref().map(package_json),
        "targets": targets,
    }))
}

pub(crate) fn expand_manifest_target(
    manifest: &manifest::FederatedDeckManifest,
    target: &str,
) -> Result<manifest::ExpandedTarget, String> {
    manifest.expand_target(target).map_err(|error| match error {
        manifest::ManifestError::MissingTarget(_) => format!(
            "manifest target {target:?} does not exist; available targets: {}",
            manifest
                .targets
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ),
        other => other.to_string(),
    })
}

pub(crate) fn verify_canonical_deck_format(path: &Path) -> Result<(), String> {
    verify_format_with(path, canonical_yaml::format_str)
}

pub(crate) fn verify_overlay_format(path: &Path) -> Result<(), String> {
    verify_format_with(path, canonical_yaml::overlay_format_str)
}

pub(crate) fn verify_manifest_format(path: &Path) -> Result<(), String> {
    verify_format_with(path, manifest::format_str)
}

fn verify_format_with<E>(
    path: &Path,
    format: impl FnOnce(&str) -> Result<String, E>,
) -> Result<(), String>
where
    E: ToString,
{
    let input = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let formatted = source_includes::format_preserving_file_includes(&input, format)?;
    if formatted != input {
        return Err(format!("{} is not in canonical format", path.display()));
    }
    Ok(())
}

pub(crate) fn root_relative_path(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

pub(crate) fn configured_crowdanki_out(
    manifest: &manifest::FederatedDeckManifest,
    target: &str,
) -> Option<PathBuf> {
    manifest
        .targets
        .get(target)?
        .exports
        .crowdanki
        .as_ref()?
        .out
        .as_deref()
        .map(PathBuf::from)
}

pub(crate) fn manifest_root(path: &Path) -> PathBuf {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

pub(crate) struct SourceContext {
    pub(crate) root: PathBuf,
    pub(crate) include_roots: Vec<PathBuf>,
}

pub(crate) fn workspace_root_for_source_path(path: &Path) -> PathBuf {
    nearest_manifest_root(path).unwrap_or_else(|| manifest_root(path))
}

pub(crate) fn source_context_for_path(path: &Path) -> Result<SourceContext, String> {
    let root = workspace_root_for_source_path(path);
    let manifest_path = root.join("brainbrew.yaml");
    let include_roots = if manifest_path.exists() {
        let manifest = read_manifest(&manifest_path)?;
        include_roots_from_manifest(&root, &manifest)
    } else {
        Vec::new()
    };
    Ok(SourceContext {
        root,
        include_roots,
    })
}

fn nearest_manifest_root(path: &Path) -> Option<PathBuf> {
    let mut current = path.parent()?;
    loop {
        if current.join("brainbrew.yaml").exists() {
            return Some(current.to_path_buf());
        }
        current = current.parent()?;
    }
}

pub(crate) fn include_roots_from_manifest(
    root: &Path,
    manifest: &manifest::FederatedDeckManifest,
) -> Vec<PathBuf> {
    manifest
        .include_roots
        .iter()
        .map(|include_root| root_relative_path(root, Path::new(include_root)))
        .collect()
}

pub(crate) fn top_level_media_include_path(value: &Value) -> Result<Option<String>, String> {
    let Some(media_value) = value
        .as_mapping()
        .and_then(|mapping| mapping.get(Value::String("media".to_owned())))
    else {
        return Ok(None);
    };
    let Value::Tagged(tagged) = media_value else {
        return Ok(None);
    };
    if tagged.tag != "include" {
        return Ok(None);
    }
    let Value::String(path) = &tagged.value else {
        return Err("media !include path must be a scalar string".to_owned());
    };
    Ok(Some(path.clone()))
}

pub(crate) fn resolve_include_target_for_context(
    include_path: &str,
    context: &SourceContext,
) -> Result<PathBuf, String> {
    source_includes::resolve_include_target(include_path, &context.root, &context.include_roots)
        .map_err(|error| error.to_string())
}

fn resolve_source_includes(
    input: &str,
    path: &Path,
    context: &SourceContext,
) -> Result<String, String> {
    source_includes::resolve_file_includes(input, path, &context.root, &context.include_roots)
        .map_err(|error| error.to_string())
}
