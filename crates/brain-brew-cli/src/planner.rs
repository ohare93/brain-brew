//! Registry-aware planning for every manifest-backed CLI operation.
//!
//! This is the only filesystem package/target expansion engine used by command
//! routes. `brain-brew-formats` retains its pure single-manifest expansion API
//! for codec consumers, but the CLI never uses it to select or expand targets.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use brain_brew_core::{CanonicalDeck, ChangeIntent, Overlay, OverlayKind};
use brain_brew_formats::manifest::{self, FederatedDeckManifest};
use brain_brew_formats::source_document::{
    IncludedSourceKind, SourceProvenance as DocumentProvenance,
};
use sha2::{Digest, Sha256};

use crate::commands::lock::locked_package_manifest_paths;
use crate::io::{
    canonical_document_from_package, include_roots_from_manifest, manifest_root,
    overlay_document_from_package, read_manifest,
};
use crate::package_resolver::{discover_package_manifests, validate_package_dependencies};
use crate::path_authorization::PathAuthorizer;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct PackageIdentity {
    pub(crate) id: String,
    pub(crate) version: String,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum RegistrySourceKind {
    RootManifest,
    ExplicitInclude,
    PackageRoot,
    SiblingLock,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PlanSourceKind {
    Base,
    Overlay { kind: OverlayKind },
    ScalarInclude { schema_path: String },
    MediaInclude,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceProvenance {
    pub(crate) package: Option<PackageIdentity>,
    pub(crate) package_root: PathBuf,
    pub(crate) manifest: PathBuf,
    pub(crate) path: PathBuf,
    pub(crate) kind: PlanSourceKind,
    pub(crate) sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TargetExpansionOrigin {
    Selection,
    Extends {
        declaring_target: String,
        reference: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TargetExpansion {
    pub(crate) qualified_name: String,
    pub(crate) owner: Option<PackageIdentity>,
    pub(crate) origin: TargetExpansionOrigin,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum OverlayExpansionOrigin {
    Target {
        qualified_target: String,
        reference: String,
    },
    Dependency {
        declaring_overlay: String,
        reference: String,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct PlannedOverlay {
    pub(crate) id: String,
    pub(crate) qualified_id: String,
    pub(crate) file: PathBuf,
    pub(crate) display_file: String,
    pub(crate) package: Option<PackageIdentity>,
    pub(crate) kind: OverlayKind,
    pub(crate) declared_kind: Option<String>,
    pub(crate) source: SourceProvenance,
    pub(crate) includes: Vec<SourceProvenance>,
    pub(crate) origin: OverlayExpansionOrigin,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MediaDeclarationProvenance {
    pub(crate) id: String,
    pub(crate) path: String,
    pub(crate) package: Option<PackageIdentity>,
    pub(crate) package_root: PathBuf,
    pub(crate) source: PathBuf,
}

pub(crate) struct TargetPlan {
    pub(crate) package: Option<manifest::PackageMetadata>,
    pub(crate) owner: Option<PackageIdentity>,
    pub(crate) target: String,
    pub(crate) qualified_name: String,
    pub(crate) target_manifest_root: PathBuf,
    pub(crate) target_manifest: FederatedDeckManifest,
    pub(crate) base_label: String,
    pub(crate) base: CanonicalDeck,
    pub(crate) base_source: SourceProvenance,
    pub(crate) base_includes: Vec<SourceProvenance>,
    pub(crate) overlays: Vec<(PlannedOverlay, Overlay)>,
    pub(crate) target_expansion: Vec<TargetExpansion>,
    pub(crate) media_declarations: BTreeMap<String, MediaDeclarationProvenance>,
}

impl TargetPlan {
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

    pub(crate) fn sources(&self) -> impl Iterator<Item = &SourceProvenance> {
        std::iter::once(&self.base_source)
            .chain(self.base_includes.iter())
            .chain(self.overlays.iter().flat_map(|(planned, _)| {
                std::iter::once(&planned.source).chain(planned.includes.iter())
            }))
    }
}

#[derive(Clone)]
pub(crate) struct RegistryManifest {
    pub(crate) path: PathBuf,
    pub(crate) root: PathBuf,
    pub(crate) include_roots: Vec<PathBuf>,
    pub(crate) identity: Option<PackageIdentity>,
    pub(crate) discovery: RegistrySourceKind,
    pub(crate) manifest: FederatedDeckManifest,
}

pub(crate) fn plan_manifest_target(
    manifest_path: &Path,
    target: &str,
    include_paths: &[PathBuf],
    package_roots: &[PathBuf],
) -> Result<TargetPlan, String> {
    ManifestRegistry::load(manifest_path, include_paths, package_roots)?.plan(target)
}

pub(crate) struct ManifestRegistry {
    root_index: usize,
    manifests: Vec<RegistryManifest>,
    packages: BTreeMap<String, usize>,
}

impl ManifestRegistry {
    pub(crate) fn load(
        manifest_path: &Path,
        include_paths: &[PathBuf],
        package_roots: &[PathBuf],
    ) -> Result<Self, String> {
        let root_path = canonical_manifest_path(manifest_path)?;
        let mut candidates = BTreeMap::<PathBuf, RegistrySourceKind>::new();
        insert_candidate(
            &mut candidates,
            root_path.clone(),
            RegistrySourceKind::RootManifest,
        );
        for path in include_paths {
            insert_candidate(
                &mut candidates,
                canonical_manifest_path(path)?,
                RegistrySourceKind::ExplicitInclude,
            );
        }
        for path in discover_package_manifests(package_roots)? {
            insert_candidate(&mut candidates, path, RegistrySourceKind::PackageRoot);
        }

        // Locks are siblings of selected package manifests. Iterate to a fixed
        // point so a package discovered through an include/root has exactly the
        // same lock behavior as one found through another route.
        let mut scanned_locks = BTreeSet::new();
        loop {
            let selected = candidates.keys().cloned().collect::<Vec<_>>();
            let mut changed = false;
            for path in selected {
                let lock_path = manifest_root(&path).join("brainbrew.lock");
                if !scanned_locks.insert(lock_path.clone()) {
                    continue;
                }
                for locked in locked_package_manifest_paths(&lock_path)? {
                    let locked = canonical_manifest_path(&locked)?;
                    changed |=
                        insert_candidate(&mut candidates, locked, RegistrySourceKind::SiblingLock);
                }
            }
            if !changed {
                break;
            }
        }

        let mut manifests = Vec::new();
        for (path, discovery) in candidates {
            let manifest = read_manifest(&path)?;
            let root = manifest_root(&path);
            let include_roots = include_roots_from_manifest(&path, &root, &manifest)?;
            let identity = manifest.package.as_ref().map(|package| PackageIdentity {
                id: package.id.clone(),
                version: package.version.clone(),
            });
            manifests.push(RegistryManifest {
                path,
                root,
                include_roots,
                identity,
                discovery,
                manifest,
            });
        }
        manifests.sort_by(|left, right| {
            source_precedence(left.discovery)
                .cmp(&source_precedence(right.discovery))
                .then_with(|| left.path.cmp(&right.path))
        });
        let root_index = manifests
            .iter()
            .position(|loaded| loaded.path == root_path)
            .ok_or_else(|| "root manifest disappeared while building registry".to_owned())?;

        validate_package_dependencies(
            &manifests
                .iter()
                .map(|loaded| (&loaded.path, &loaded.manifest))
                .collect::<Vec<_>>(),
        )?;
        validate_overlay_catalogs(&manifests)?;

        let mut packages = BTreeMap::new();
        for (index, loaded) in manifests.iter().enumerate() {
            let Some(identity) = &loaded.identity else {
                continue;
            };
            if let Some(previous) = packages.insert(identity.id.clone(), index) {
                let previous = &manifests[previous];
                let conflict = if previous.identity.as_ref() == Some(identity) {
                    "duplicate package identity"
                } else {
                    "conflicting package identity"
                };
                return Err(format!(
                    "{conflict} {}@{} in {} and {}",
                    identity.id,
                    identity.version,
                    previous.path.display(),
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

    pub(crate) fn manifests(&self) -> &[RegistryManifest] {
        &self.manifests
    }

    pub(crate) fn root(&self) -> &RegistryManifest {
        &self.manifests[self.root_index]
    }

    /// Load every registered base/overlay once and return its complete include
    /// dependency set. Workbench freshness uses this instead of re-planning all
    /// targets, so signatures retain provenance without target-count work.
    pub(crate) fn registered_sources(&self) -> Result<Vec<SourceProvenance>, String> {
        let mut sources = BTreeMap::<PathBuf, SourceProvenance>::new();
        for loaded in &self.manifests {
            let base_path = authorize_manifest_path(loaded, "base", &loaded.manifest.base)?;
            let base_document =
                canonical_document_from_package(&base_path, &loaded.root, &loaded.include_roots)?;
            let source = source_provenance(loaded, base_path, PlanSourceKind::Base)?;
            sources.insert(source.path.clone(), source);
            for include in included_provenance(loaded, base_document.included_sources())? {
                sources.insert(include.path.clone(), include);
            }
            for (overlay_id, entry) in &loaded.manifest.overlays {
                let file = authorize_manifest_path(
                    loaded,
                    format!("overlays.{overlay_id}.file"),
                    &entry.file,
                )?;
                let document =
                    overlay_document_from_package(&file, &loaded.root, &loaded.include_roots)?;
                let overlay = document.resolved_overlay();
                let kind = overlay.kind;
                let source = source_provenance(loaded, file, PlanSourceKind::Overlay { kind })?;
                sources.insert(source.path.clone(), source);
                for include in included_provenance(loaded, document.included_sources())? {
                    sources.insert(include.path.clone(), include);
                }
            }
        }
        Ok(sources.into_values().collect())
    }

    pub(crate) fn target_references(&self) -> Vec<String> {
        let qualify = self.manifests.len() > 1;
        self.manifests
            .iter()
            .flat_map(|loaded| {
                loaded.manifest.targets.keys().map(move |target| {
                    if qualify {
                        loaded
                            .identity
                            .as_ref()
                            .map(|identity| format!("{}:{target}", identity.id))
                            .unwrap_or_else(|| target.clone())
                    } else {
                        target.clone()
                    }
                })
            })
            .collect()
    }

    pub(crate) fn plan(&self, target_reference: &str) -> Result<TargetPlan, String> {
        let (manifest_index, target) = self.resolve_selected_target(target_reference)?;
        let selected_qualified = self.qualified_target(manifest_index, target);
        let mut blueprint = Blueprint::default();
        self.visit_target(
            manifest_index,
            target,
            TargetExpansionOrigin::Selection,
            &mut Vec::new(),
            &mut blueprint,
        )?;
        let base_manifest_index = blueprint
            .base_manifest
            .ok_or_else(|| format!("target {selected_qualified} did not select a base source"))?;
        let base_loaded = &self.manifests[base_manifest_index];
        let base_path = authorize_manifest_path(base_loaded, "base", &base_loaded.manifest.base)?;
        let base_document = canonical_document_from_package(
            &base_path,
            &base_loaded.root,
            &base_loaded.include_roots,
        )?;
        let base = base_document.resolved_deck().clone();
        let base_source = source_provenance(base_loaded, base_path, PlanSourceKind::Base)?;
        let base_includes = included_provenance(base_loaded, base_document.included_sources())?;

        let mut overlays = Vec::new();
        let media_declaration_source = base_includes
            .iter()
            .find(|source| source.kind == PlanSourceKind::MediaInclude)
            .map(|source| source.path.as_path())
            .unwrap_or(base_source.path.as_path());
        let mut media_declarations = base
            .media
            .values()
            .map(|media| {
                (
                    media.id.to_string(),
                    media_owner(
                        base_loaded,
                        media_declaration_source,
                        media.id.to_string(),
                        media.path.clone(),
                    ),
                )
            })
            .collect::<BTreeMap<_, _>>();
        for overlay_blueprint in blueprint.overlays {
            let loaded = &self.manifests[overlay_blueprint.manifest_index];
            let entry = &loaded.manifest.overlays[&overlay_blueprint.overlay_id];
            let file = authorize_manifest_path(
                loaded,
                format!("overlays.{}.file", overlay_blueprint.overlay_id),
                &entry.file,
            )?;
            let document =
                overlay_document_from_package(&file, &loaded.root, &loaded.include_roots)?;
            let overlay = document.resolved_overlay().clone();
            let kind = overlay.kind;
            let qualified_id = self.qualified_overlay(
                overlay_blueprint.manifest_index,
                &overlay_blueprint.overlay_id,
            );
            let source = source_provenance(loaded, file.clone(), PlanSourceKind::Overlay { kind })?;
            let includes = included_provenance(loaded, document.included_sources())?;
            for (id, change) in &overlay.media_changes {
                match change.intent {
                    ChangeIntent::Remove => {
                        media_declarations.remove(id.as_str());
                    }
                    _ => {
                        if let Some(media) = &change.media {
                            media_declarations.insert(
                                id.to_string(),
                                media_owner(
                                    loaded,
                                    &source.path,
                                    id.to_string(),
                                    media.path.clone(),
                                ),
                            );
                        }
                    }
                }
            }
            let display_file = if overlay_blueprint.manifest_index == self.root_index {
                entry.file.clone()
            } else if let Some(identity) = &loaded.identity {
                format!("{}:{}", identity.id, entry.file)
            } else {
                entry.file.clone()
            };
            overlays.push((
                PlannedOverlay {
                    id: if overlay_blueprint.manifest_index == self.root_index {
                        overlay_blueprint.overlay_id.clone()
                    } else {
                        qualified_id.clone()
                    },
                    qualified_id,
                    file,
                    display_file,
                    package: loaded.identity.clone(),
                    kind,
                    declared_kind: entry.kind.clone(),
                    source,
                    includes,
                    origin: overlay_blueprint.origin,
                },
                overlay,
            ));
        }

        let selected = &self.manifests[manifest_index];
        Ok(TargetPlan {
            package: selected.manifest.package.clone(),
            owner: selected.identity.clone(),
            target: target.to_owned(),
            qualified_name: selected_qualified,
            target_manifest_root: selected.root.clone(),
            target_manifest: selected.manifest.clone(),
            base_label: blueprint.base_label,
            base,
            base_source,
            base_includes,
            overlays,
            target_expansion: blueprint.targets,
            media_declarations,
        })
    }

    fn visit_target(
        &self,
        manifest_index: usize,
        target: &str,
        origin: TargetExpansionOrigin,
        active: &mut Vec<(usize, String)>,
        blueprint: &mut Blueprint,
    ) -> Result<(), String> {
        let key = (manifest_index, target.to_owned());
        if let Some(index) = active.iter().position(|candidate| candidate == &key) {
            let mut cycle = active[index..]
                .iter()
                .map(|(index, target)| self.qualified_target(*index, target))
                .collect::<Vec<_>>();
            cycle.push(self.qualified_target(manifest_index, target));
            return Err(format!(
                "manifest target dependency cycle: {}",
                cycle.join(" -> ")
            ));
        }
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
        let qualified = self.qualified_target(manifest_index, target);
        blueprint.targets.push(TargetExpansion {
            qualified_name: qualified.clone(),
            owner: loaded.identity.clone(),
            origin,
        });
        active.push(key);
        if let Some(extends) = &target_entry.extends {
            let (base_index, base_target) = self.resolve_target_ref(manifest_index, extends)?;
            self.visit_target(
                base_index,
                base_target,
                TargetExpansionOrigin::Extends {
                    declaring_target: qualified.clone(),
                    reference: extends.clone(),
                },
                active,
                blueprint,
            )?;
            if blueprint.base_label.is_empty() {
                blueprint.base_label = extends.clone();
            }
        } else if blueprint.base_manifest.is_none() {
            blueprint.base_manifest = Some(manifest_index);
            blueprint.base_label = loaded.manifest.base.clone();
        }
        for overlay in &target_entry.overlays {
            self.visit_overlay(
                manifest_index,
                overlay,
                OverlayExpansionOrigin::Target {
                    qualified_target: qualified.clone(),
                    reference: overlay.clone(),
                },
                &mut Vec::new(),
                blueprint,
            )?;
        }
        active.pop();
        Ok(())
    }

    fn visit_overlay(
        &self,
        current_manifest_index: usize,
        overlay_reference: &str,
        origin: OverlayExpansionOrigin,
        active: &mut Vec<(usize, String)>,
        blueprint: &mut Blueprint,
    ) -> Result<(), String> {
        let (manifest_index, overlay_id) =
            self.resolve_overlay_ref(current_manifest_index, overlay_reference)?;
        let key = (manifest_index, overlay_id.to_owned());
        if blueprint.visited_overlays.contains(&key) {
            return Ok(());
        }
        if let Some(index) = active.iter().position(|candidate| candidate == &key) {
            let mut cycle = active[index..]
                .iter()
                .map(|(index, id)| self.qualified_overlay(*index, id))
                .collect::<Vec<_>>();
            cycle.push(self.qualified_overlay(manifest_index, overlay_id));
            return Err(format!(
                "manifest overlay dependency cycle: {}",
                cycle.join(" -> ")
            ));
        }
        let loaded = &self.manifests[manifest_index];
        let entry = loaded.manifest.overlays.get(overlay_id).ok_or_else(|| {
            format!(
                "manifest overlay {overlay_id:?} does not exist in {}",
                loaded.path.display()
            )
        })?;
        active.push(key.clone());
        let declaring = self.qualified_overlay(manifest_index, overlay_id);
        for dependency in &entry.depends_on {
            self.visit_overlay(
                manifest_index,
                dependency,
                OverlayExpansionOrigin::Dependency {
                    declaring_overlay: declaring.clone(),
                    reference: dependency.clone(),
                },
                active,
                blueprint,
            )?;
        }
        active.pop();
        blueprint.visited_overlays.insert(key);
        blueprint.overlays.push(OverlayBlueprint {
            manifest_index,
            overlay_id: overlay_id.to_owned(),
            origin,
        });
        Ok(())
    }

    fn resolve_selected_target<'a>(&self, reference: &'a str) -> Result<(usize, &'a str), String> {
        if let Some((package, target)) = split_qualified(reference)? {
            let index = self.packages.get(package).copied().ok_or_else(|| {
                format!("package {package:?} required by target ref {reference:?} was not included")
            })?;
            return Ok((index, target));
        }
        let matches = self
            .manifests
            .iter()
            .enumerate()
            .filter(|(_, loaded)| loaded.manifest.targets.contains_key(reference))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => Err(format!(
                "manifest target {reference:?} does not exist; available targets: {}",
                self.target_references().join(", ")
            )),
            [index] => Ok((*index, reference)),
            _ => Err(format!(
                "ambiguous unqualified target {reference:?}; use one of: {}",
                matches
                    .iter()
                    .map(|index| self.qualified_target(*index, reference))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }

    fn resolve_target_ref<'a>(
        &self,
        current_manifest_index: usize,
        reference: &'a str,
    ) -> Result<(usize, &'a str), String> {
        if let Some((package, target)) = split_qualified(reference)? {
            let index = self.packages.get(package).copied().ok_or_else(|| {
                format!("package {package:?} required by target ref {reference:?} was not included")
            })?;
            Ok((index, target))
        } else {
            Ok((current_manifest_index, reference))
        }
    }

    fn resolve_overlay_ref<'a>(
        &self,
        current_manifest_index: usize,
        reference: &'a str,
    ) -> Result<(usize, &'a str), String> {
        if let Some((package, overlay)) = split_qualified(reference)? {
            let index = self.packages.get(package).copied().ok_or_else(|| {
                format!(
                    "package {package:?} required by overlay ref {reference:?} was not included"
                )
            })?;
            Ok((index, overlay))
        } else {
            Ok((current_manifest_index, reference))
        }
    }

    fn qualified_target(&self, manifest_index: usize, target: &str) -> String {
        self.manifests[manifest_index]
            .identity
            .as_ref()
            .map(|identity| format!("{}:{target}", identity.id))
            .unwrap_or_else(|| target.to_owned())
    }

    fn qualified_overlay(&self, manifest_index: usize, overlay: &str) -> String {
        self.manifests[manifest_index]
            .identity
            .as_ref()
            .map(|identity| format!("{}:{overlay}", identity.id))
            .unwrap_or_else(|| overlay.to_owned())
    }
}

#[derive(Default)]
struct Blueprint {
    base_manifest: Option<usize>,
    base_label: String,
    targets: Vec<TargetExpansion>,
    overlays: Vec<OverlayBlueprint>,
    visited_overlays: BTreeSet<(usize, String)>,
}

struct OverlayBlueprint {
    manifest_index: usize,
    overlay_id: String,
    origin: OverlayExpansionOrigin,
}

fn canonical_manifest_path(path: &Path) -> Result<PathBuf, String> {
    fs::canonicalize(path).map_err(|error| format!("{}: {error}", path.display()))
}

fn insert_candidate(
    candidates: &mut BTreeMap<PathBuf, RegistrySourceKind>,
    path: PathBuf,
    kind: RegistrySourceKind,
) -> bool {
    match candidates.get_mut(&path) {
        Some(existing) => {
            if source_precedence(kind) < source_precedence(*existing) {
                *existing = kind;
            }
            false
        }
        None => {
            candidates.insert(path, kind);
            true
        }
    }
}

fn source_precedence(kind: RegistrySourceKind) -> u8 {
    match kind {
        RegistrySourceKind::RootManifest => 0,
        RegistrySourceKind::ExplicitInclude => 1,
        RegistrySourceKind::PackageRoot => 2,
        RegistrySourceKind::SiblingLock => 3,
    }
}

fn validate_overlay_catalogs(manifests: &[RegistryManifest]) -> Result<(), String> {
    let mut source_identities = BTreeMap::<PathBuf, String>::new();
    for loaded in manifests {
        for (catalog_id, entry) in &loaded.manifest.overlays {
            let field = format!("overlays.{catalog_id}.file");
            let file = authorize_manifest_path(loaded, &field, &entry.file)?;
            let qualified = loaded
                .identity
                .as_ref()
                .map(|package| format!("{}:{catalog_id}", package.id))
                .unwrap_or_else(|| catalog_id.clone());
            let declared_identity = format!(
                "{qualified} ({}) declared by {}",
                entry.kind.as_deref().unwrap_or("unspecified kind"),
                loaded.path.display()
            );
            if let Some(previous) =
                source_identities.insert(file.clone(), declared_identity.clone())
                && previous != declared_identity
            {
                return Err(format!(
                    "overlay source {} is cataloged under conflicting identities: {previous}; {declared_identity}",
                    file.display()
                ));
            }
            let document = overlay_document_from_package(
                &file,
                &loaded.root,
                &loaded.include_roots,
            )
            .map_err(|error| {
                format!(
                    "overlay catalog {catalog_id:?} declared in {} could not decode {}: {error}",
                    loaded.path.display(),
                    file.display()
                )
            })?;
            let overlay = document.resolved_overlay();
            if overlay.id.as_str() != catalog_id {
                return Err(format!(
                    "overlay catalog identity mismatch in {} at overlays.{catalog_id}: catalog ID {catalog_id:?} points to {} whose overlay.id is {:?}",
                    loaded.path.display(),
                    file.display(),
                    overlay.id.as_str()
                ));
            }
            if let Some(declared_kind) = &entry.kind {
                let actual_kind = overlay_kind_name(overlay.kind);
                if declared_kind != actual_kind {
                    return Err(format!(
                        "overlay catalog kind mismatch in {} at overlays.{catalog_id}.kind: declared {declared_kind:?}, but {} has overlay.kind {actual_kind:?}",
                        loaded.path.display(),
                        file.display()
                    ));
                }
            }
        }
    }
    Ok(())
}

fn overlay_kind_name(kind: OverlayKind) -> &'static str {
    match kind {
        OverlayKind::Translation => "translation",
        OverlayKind::Extension => "extension",
        OverlayKind::Patch => "patch",
        OverlayKind::Personal => "personal",
    }
}

fn authorize_manifest_path(
    loaded: &RegistryManifest,
    field: impl Into<String>,
    raw: &str,
) -> Result<PathBuf, String> {
    PathAuthorizer::new("package", &loaded.root)?
        .authorize_read(&loaded.path, field, raw)
        .map(|path| path.into_path_buf())
        .map_err(|error| error.to_string())
}

fn source_provenance(
    loaded: &RegistryManifest,
    path: PathBuf,
    kind: PlanSourceKind,
) -> Result<SourceProvenance, String> {
    let bytes = fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(SourceProvenance {
        package: loaded.identity.clone(),
        package_root: loaded.root.clone(),
        manifest: loaded.path.clone(),
        path,
        kind,
        sha256: format!("{:x}", Sha256::digest(bytes)),
    })
}

fn included_provenance(
    loaded: &RegistryManifest,
    included: Vec<brain_brew_formats::source_document::IncludedSource>,
) -> Result<Vec<SourceProvenance>, String> {
    included
        .into_iter()
        .map(|source| {
            let kind = match source.kind() {
                IncludedSourceKind::Scalar { schema_path } => PlanSourceKind::ScalarInclude {
                    schema_path: schema_path.clone(),
                },
                IncludedSourceKind::MediaDeclarations => PlanSourceKind::MediaInclude,
            };
            source_provenance(loaded, provenance_path(source.provenance())?, kind)
        })
        .collect()
}

fn provenance_path(provenance: &DocumentProvenance) -> Result<PathBuf, String> {
    let path = PathBuf::from(provenance.source_name());
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(format!(
            "source provenance {} is not an absolute authorized path",
            provenance
        ))
    }
}

fn media_owner(
    loaded: &RegistryManifest,
    source: &Path,
    id: String,
    path: String,
) -> MediaDeclarationProvenance {
    MediaDeclarationProvenance {
        id,
        path,
        package: loaded.identity.clone(),
        package_root: loaded.root.clone(),
        source: source.to_path_buf(),
    }
}

fn split_qualified(reference: &str) -> Result<Option<(&str, &str)>, String> {
    let Some((package, item)) = reference.split_once(':') else {
        return Ok(None);
    };
    if package.is_empty() || item.is_empty() || item.contains(':') {
        return Err(format!(
            "invalid package-qualified reference {reference:?}; expected <package-id>:<name>"
        ));
    }
    Ok(Some((package, item)))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn plan_retains_package_include_overlay_and_media_provenance() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("description.md"), "!include nested.md\n").unwrap();
        fs::write(root.path().join("nested.md"), "Included description\n").unwrap();
        fs::write(
            root.path().join("media.yaml"),
            "media.flag:\n  path: flag.svg\n  sha256: ''\n",
        )
        .unwrap();
        fs::write(
            root.path().join("deck.yaml"),
            "deck:\n  id: deck.example\n  name: Example\n  description: !include description.md\nnote_types: {}\nnotes: {}\nmedia: !include media.yaml\ntombstones: []\n",
        )
        .unwrap();
        fs::write(
            root.path().join("patch.yaml"),
            "id: overlay.patch\nkind: patch\ndeck:\n  name:\n    intent: replace\n    value: Patched\n    expected_base:\n      value: Example\n",
        )
        .unwrap();
        fs::write(
            root.path().join("brainbrew.yaml"),
            "package:\n  id: example.deck\n  version: 1.0.0\nbase: deck.yaml\noverlays:\n  overlay.patch:\n    file: patch.yaml\n    kind: patch\ntargets:\n  patched:\n    overlays:\n      - overlay.patch\n",
        )
        .unwrap();

        let registry = ManifestRegistry::load(&root.path().join("brainbrew.yaml"), &[], &[])
            .expect("registry loads");
        let plan = registry
            .plan("example.deck:patched")
            .expect("qualified target plans");

        assert_eq!(plan.qualified_name, "example.deck:patched");
        assert_eq!(plan.owner.as_ref().unwrap().id, "example.deck");
        assert_eq!(plan.base_includes.len(), 3);
        assert_eq!(
            plan.base_includes
                .iter()
                .filter(|source| matches!(source.kind, PlanSourceKind::ScalarInclude { .. }))
                .count(),
            2
        );
        assert!(plan.base_includes.iter().all(|source| {
            !matches!(
                source.kind,
                PlanSourceKind::ScalarInclude { ref schema_path }
                    if schema_path != "deck.description"
            )
        }));
        assert!(
            plan.base_includes
                .iter()
                .any(|source| source.kind == PlanSourceKind::MediaInclude)
        );
        assert_eq!(plan.overlays[0].0.kind, OverlayKind::Patch);
        assert!(matches!(
            plan.overlays[0].0.origin,
            OverlayExpansionOrigin::Target { .. }
        ));
        let media = &plan.media_declarations["media.flag"];
        assert_eq!(media.package.as_ref().unwrap().id, "example.deck");
        assert_eq!(media.package_root, fs::canonicalize(root.path()).unwrap());
        assert_eq!(
            media.source,
            fs::canonicalize(root.path().join("media.yaml")).unwrap()
        );
        assert_eq!(plan.compose().unwrap().name, "Patched");
    }
}
