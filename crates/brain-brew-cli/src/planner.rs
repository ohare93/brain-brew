//! Registry-aware planning for every manifest-backed CLI operation.
//!
//! This is the only filesystem package/target expansion engine used by command
//! routes. `brain-brew-formats` retains its pure single-manifest expansion API
//! for codec consumers, but the CLI never uses it to select or expand targets.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use brain_brew_core::{CanonicalDeck, ChangeIntent, FieldValue, Overlay, OverlayKind};
use brain_brew_formats::canonical_source_document::NoteAuthoringProvenance;
use brain_brew_formats::csv_note_source::CsvTranslationAuthoringProvenance;
use brain_brew_formats::manifest::{self, FederatedDeckManifest};
use brain_brew_formats::media;
use brain_brew_formats::source_document::{
    IncludedSourceKind, SourceProvenance as DocumentProvenance,
};
use sha2::{Digest, Sha256};

use crate::commands::lock::locked_package_manifest_paths;
use crate::io::{
    canonical_document_from_package, compose_translation_free_source, include_roots_from_manifest,
    manifest_root, overlay_document_from_package, overlay_document_from_package_with_source_deck,
    read_manifest,
};
use crate::package_resolver::{
    DiscoveryPolicy, DiscoveryResult, DiscoveryStats, discover_package_manifests,
    validate_package_dependencies,
};
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

impl RegistrySourceKind {
    pub(crate) fn ownership_name(self) -> &'static str {
        match self {
            Self::RootManifest => "root",
            Self::ExplicitInclude => "include",
            Self::PackageRoot => "path",
            Self::SiblingLock => "locked",
        }
    }

    pub(crate) fn is_root_workspace(self) -> bool {
        self == Self::RootManifest
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PlanSourceKind {
    Base,
    Overlay { kind: OverlayKind },
    ScalarInclude { schema_path: String },
    MediaInclude,
    NoteTypesInclude,
    CsvDescriptor,
    CsvTable { alias: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceProvenance {
    pub(crate) package: Option<PackageIdentity>,
    pub(crate) package_root: PathBuf,
    pub(crate) manifest: PathBuf,
    pub(crate) registry_source: RegistrySourceKind,
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
    pub(crate) csv_translation_provenance: CsvTranslationAuthoringProvenance,
    pub(crate) origin: OverlayExpansionOrigin,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MediaDeclarationProvenance {
    pub(crate) id: String,
    pub(crate) path: String,
    pub(crate) sha256: String,
    pub(crate) package: Option<PackageIdentity>,
    pub(crate) package_root: PathBuf,
    pub(crate) source_kind: RegistrySourceKind,
    /// File that contains the declaration (for example an included media map).
    pub(crate) source: PathBuf,
    /// Canonical Deck or Overlay document through which typed mutation occurs.
    pub(crate) document_source: PathBuf,
}

impl MediaDeclarationProvenance {
    pub(crate) fn package_label(&self) -> &str {
        self.package
            .as_ref()
            .map(|package| package.id.as_str())
            .unwrap_or("<root-workspace>")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MediaReferenceBinding {
    /// Stable-ID or rendered-path reference selected by the composed deck.
    pub(crate) reference: String,
    pub(crate) declaration: MediaDeclarationProvenance,
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
    pub(crate) base_note_authoring_provenance: NoteAuthoringProvenance,
    pub(crate) base_source: SourceProvenance,
    pub(crate) base_includes: Vec<SourceProvenance>,
    pub(crate) overlays: Vec<(PlannedOverlay, Overlay)>,
    pub(crate) target_expansion: Vec<TargetExpansion>,
    pub(crate) media_declarations: BTreeMap<String, MediaDeclarationProvenance>,
}

impl TargetPlan {
    pub(crate) fn compose(&self) -> Result<CanonicalDeck, brain_brew_core::ComposeReport> {
        self.base.compose(
            &self
                .overlays
                .iter()
                .map(|(_, overlay)| overlay.clone())
                .collect::<Vec<_>>(),
        )
    }

    pub(crate) fn media_reference_bindings(
        &self,
        deck: &CanonicalDeck,
    ) -> Result<Vec<MediaReferenceBinding>, String> {
        bind_media_references(&self.qualified_name, deck, &self.media_declarations)
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
    discovery_policy: &DiscoveryPolicy,
) -> Result<TargetPlan, String> {
    ManifestRegistry::load_with_policy(
        manifest_path,
        include_paths,
        package_roots,
        discovery_policy,
    )?
    .plan(target)
}

pub(crate) struct ManifestRegistry {
    root_index: usize,
    manifests: Vec<RegistryManifest>,
    packages: BTreeMap<String, usize>,
    discovery_stats: DiscoveryStats,
}

impl ManifestRegistry {
    pub(crate) fn load(
        manifest_path: &Path,
        include_paths: &[PathBuf],
        package_roots: &[PathBuf],
    ) -> Result<Self, String> {
        Self::load_with_policy(
            manifest_path,
            include_paths,
            package_roots,
            &DiscoveryPolicy::default(),
        )
    }

    pub(crate) fn load_with_policy(
        manifest_path: &Path,
        include_paths: &[PathBuf],
        package_roots: &[PathBuf],
        discovery_policy: &DiscoveryPolicy,
    ) -> Result<Self, String> {
        let discovery = discover_package_manifests(package_roots, discovery_policy)
            .map_err(|error| error.to_string())?;
        Self::load_with_discovery(manifest_path, include_paths, discovery)
    }

    pub(crate) fn discover_manifest_paths(
        package_roots: &[PathBuf],
        discovery_policy: &DiscoveryPolicy,
    ) -> Result<Vec<PathBuf>, String> {
        discover_package_manifests(package_roots, discovery_policy)
            .map(|result| result.manifests)
            .map_err(|error| error.to_string())
    }

    pub(crate) fn discover(
        package_roots: &[PathBuf],
        discovery_policy: &DiscoveryPolicy,
    ) -> Result<Self, String> {
        let discovery = discover_package_manifests(package_roots, discovery_policy)
            .map_err(|error| error.to_string())?;
        let Some(root) = discovery.manifests.first().cloned() else {
            return Err("no Brain Brew package manifests were discovered".to_owned());
        };
        Self::load_with_discovery(&root, &[], discovery)
    }

    fn load_with_discovery(
        manifest_path: &Path,
        include_paths: &[PathBuf],
        discovery: DiscoveryResult,
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
        for path in &discovery.manifests {
            insert_candidate(
                &mut candidates,
                canonical_manifest_path(path)?,
                RegistrySourceKind::PackageRoot,
            );
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
            discovery_stats: discovery.stats,
        })
    }

    pub(crate) fn manifests(&self) -> &[RegistryManifest] {
        &self.manifests
    }

    pub(crate) fn discovery_stats(&self) -> &DiscoveryStats {
        &self.discovery_stats
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
            for include in included_provenance(loaded, base_document.included_sources())?
                .into_iter()
                .chain(csv_source_provenance(loaded, base_document.csv_sources())?)
            {
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
                for include in included_provenance(loaded, document.included_sources())?
                    .into_iter()
                    .chain(csv_source_provenance(loaded, document.csv_sources())?)
                {
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
        let base_note_authoring_provenance = base_document.authoring_provenance().clone();
        let base_source = source_provenance(base_loaded, base_path, PlanSourceKind::Base)?;
        let mut base_includes = included_provenance(base_loaded, base_document.included_sources())?;
        base_includes.extend(csv_source_provenance(
            base_loaded,
            base_document.csv_sources(),
        )?);

        let mut inventory = Vec::new();
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
            inventory.push((overlay_blueprint, file, document));
        }
        let has_csv_translations = inventory
            .iter()
            .any(|(_, _, document)| !document.csv_translation_sources().is_empty());
        let source_deck = has_csv_translations
            .then(|| {
                compose_translation_free_source(
                    &base,
                    &inventory
                        .iter()
                        .map(|(_, _, document)| document.resolved_overlay().clone())
                        .collect::<Vec<_>>(),
                )
            })
            .transpose()?;

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
                        &base_source.path,
                        media,
                    ),
                )
            })
            .collect::<BTreeMap<_, _>>();
        for (overlay_blueprint, file, inventory_document) in inventory {
            let loaded = &self.manifests[overlay_blueprint.manifest_index];
            let entry = &loaded.manifest.overlays[&overlay_blueprint.overlay_id];
            let document = if let Some(source_deck) = &source_deck {
                overlay_document_from_package_with_source_deck(
                    &file,
                    &loaded.root,
                    &loaded.include_roots,
                    source_deck,
                )?
            } else {
                inventory_document
            };
            let overlay = document.resolved_overlay().clone();
            let kind = overlay.kind;
            let qualified_id = self.qualified_overlay(
                overlay_blueprint.manifest_index,
                &overlay_blueprint.overlay_id,
            );
            let source = source_provenance(loaded, file.clone(), PlanSourceKind::Overlay { kind })?;
            let mut includes = included_provenance(loaded, document.included_sources())?;
            includes.extend(csv_source_provenance(loaded, document.csv_sources())?);
            for (id, change) in &overlay.media_changes {
                match change.intent {
                    ChangeIntent::Remove => {
                        media_declarations.remove(id.as_str());
                    }
                    _ => {
                        if let Some(media) = &change.media {
                            if let Some(previous) = media_declarations.get(id.as_str())
                                && previous.package_root != loaded.root
                                && change.intent == ChangeIntent::Merge
                            {
                                return Err(format!(
                                    "cross-package media stable-ID collision for target {selected_qualified}: declaration {id} is owned by package {} at {}, but package {} attempts an ambiguous merge from {}; use an explicit replace/override with expected_base to transfer ownership",
                                    previous.package_label(),
                                    previous.source.display(),
                                    loaded
                                        .identity
                                        .as_ref()
                                        .map(|package| package.id.as_str())
                                        .unwrap_or("<root-workspace>"),
                                    source.path.display()
                                ));
                            }
                            media_declarations.insert(
                                id.to_string(),
                                media_owner(loaded, &source.path, &source.path, media),
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
                    csv_translation_provenance: document.csv_translation_provenance().clone(),
                    origin: overlay_blueprint.origin,
                },
                overlay,
            ));
        }

        validate_media_declaration_collisions(&selected_qualified, &media_declarations)?;

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
            base_note_authoring_provenance,
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
    Ok(source_provenance_from_bytes(loaded, path, kind, &bytes))
}

fn source_provenance_from_bytes(
    loaded: &RegistryManifest,
    path: PathBuf,
    kind: PlanSourceKind,
    bytes: &[u8],
) -> SourceProvenance {
    SourceProvenance {
        package: loaded.identity.clone(),
        package_root: loaded.root.clone(),
        manifest: loaded.path.clone(),
        registry_source: loaded.discovery,
        path,
        kind,
        sha256: format!("{:x}", Sha256::digest(bytes)),
    }
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
                IncludedSourceKind::NoteTypeDeclarations => PlanSourceKind::NoteTypesInclude,
            };
            source_provenance(loaded, provenance_path(source.provenance())?, kind)
        })
        .collect()
}

fn csv_source_provenance(
    loaded: &RegistryManifest,
    sources: &[(
        brain_brew_formats::csv_note_source::CsvSourceRequestKind,
        brain_brew_formats::csv_note_source::CsvSourceFile,
    )],
) -> Result<Vec<SourceProvenance>, String> {
    sources
        .iter()
        .map(|(kind, source)| {
            let kind = match kind {
                brain_brew_formats::csv_note_source::CsvSourceRequestKind::Descriptor => {
                    PlanSourceKind::CsvDescriptor
                }
                brain_brew_formats::csv_note_source::CsvSourceRequestKind::Table { alias } => {
                    PlanSourceKind::CsvTable {
                        alias: alias.clone(),
                    }
                }
            };
            Ok(source_provenance_from_bytes(
                loaded,
                provenance_path(source.provenance())?,
                kind,
                source.bytes(),
            ))
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
    document_source: &Path,
    media: &brain_brew_core::MediaReference,
) -> MediaDeclarationProvenance {
    MediaDeclarationProvenance {
        id: media.id.to_string(),
        path: media.path.clone(),
        sha256: media.sha256.clone(),
        package: loaded.identity.clone(),
        package_root: loaded.root.clone(),
        source_kind: loaded.discovery,
        source: source.to_path_buf(),
        document_source: document_source.to_path_buf(),
    }
}

fn validate_media_declaration_collisions(
    target: &str,
    declarations: &BTreeMap<String, MediaDeclarationProvenance>,
) -> Result<(), String> {
    let mut paths = BTreeMap::<&str, &MediaDeclarationProvenance>::new();
    for declaration in declarations.values() {
        if let Some(previous) = paths.insert(&declaration.path, declaration)
            && (previous.package_root != declaration.package_root
                || previous.sha256 != declaration.sha256)
        {
            return Err(format!(
                "media path/output collision for target {target}: declaration {} owned by package {} at {} and declaration {} owned by package {} at {} both resolve to {:?}",
                previous.id,
                previous.package_label(),
                previous.source.display(),
                declaration.id,
                declaration.package_label(),
                declaration.source.display(),
                declaration.path
            ));
        }
    }
    Ok(())
}

fn bind_media_references(
    target: &str,
    deck: &CanonicalDeck,
    declarations: &BTreeMap<String, MediaDeclarationProvenance>,
) -> Result<Vec<MediaReferenceBinding>, String> {
    let report = media::reference_report(deck);
    if !report.errors.is_empty() {
        return Err(format!(
            "media reference ownership failed for target {target}: {}",
            report
                .errors
                .iter()
                .map(|error| error.message.clone())
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }

    let mut bindings = BTreeMap::<String, MediaReferenceBinding>::new();
    for note in deck.notes.values() {
        for value in note.fields.values() {
            let FieldValue::Images(images) = value else {
                continue;
            };
            for image in images {
                let declaration = declarations.get(image.media_id.as_str()).ok_or_else(|| {
                    format!(
                        "media reference ownership failed for target {target}: unknown stable ID {}",
                        image.media_id
                    )
                })?;
                let reference = format!("id:{}", image.media_id);
                bindings.insert(
                    reference.clone(),
                    MediaReferenceBinding {
                        reference,
                        declaration: declaration.clone(),
                    },
                );
            }
        }
    }
    for path in media::referenced_paths(deck) {
        let candidates = declarations
            .values()
            .filter(|declaration| declaration.path == path)
            .collect::<Vec<_>>();
        let Some(first) = candidates.first() else {
            return Err(format!(
                "media reference ownership failed for target {target}: path {path:?} is undeclared"
            ));
        };
        // Same-package aliases are the compatibility policy for historical maps:
        // they are unambiguous only when they select the exact same owner root,
        // path, and hash bytes. Cross-package aliases are rejected earlier.
        if candidates.iter().skip(1).any(|candidate| {
            candidate.package_root != first.package_root
                || candidate.path != first.path
                || candidate.sha256 != first.sha256
        }) {
            return Err(format!(
                "ambiguous media reference ownership for target {target}, path {path:?}: {}",
                candidates
                    .iter()
                    .map(|candidate| format!(
                        "{} (package {}, hash {:?}, source {})",
                        candidate.id,
                        candidate.package_label(),
                        candidate.sha256,
                        candidate.source.display()
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        let reference = format!("path:{path}");
        bindings.insert(
            reference.clone(),
            MediaReferenceBinding {
                reference,
                declaration: (*first).clone(),
            },
        );
    }
    Ok(bindings.into_values().collect())
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
    fn csv_provenance_hashes_the_injected_materialized_bytes() {
        use brain_brew_formats::csv_note_source::{CsvSourceFile, CsvSourceRequestKind};
        use brain_brew_formats::source_document::SourceProvenance as DocumentProvenance;

        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("notes.csv");
        fs::write(&path, b"bytes changed after materialization").unwrap();
        let manifest_path = root.path().join("brainbrew.yaml");
        fs::write(
            &manifest_path,
            "base: deck.yaml\noverlays: {}\ntargets: {}\n",
        )
        .unwrap();
        let loaded = RegistryManifest {
            path: manifest_path,
            root: fs::canonicalize(root.path()).unwrap(),
            include_roots: Vec::new(),
            identity: None,
            discovery: RegistrySourceKind::RootManifest,
            manifest: manifest::from_str("base: deck.yaml\noverlays: {}\ntargets: {}\n").unwrap(),
        };
        let materialized = b"exact materialized bytes";
        let source = CsvSourceFile::new(
            DocumentProvenance::new(path.display().to_string()),
            materialized,
        );

        let provenance = csv_source_provenance(
            &loaded,
            &[(
                CsvSourceRequestKind::Table {
                    alias: "main".to_owned(),
                },
                source,
            )],
        )
        .unwrap();
        assert_eq!(
            provenance[0].sha256,
            format!("{:x}", Sha256::digest(materialized))
        );
        assert_ne!(
            provenance[0].sha256,
            format!("{:x}", Sha256::digest(fs::read(&path).unwrap()))
        );
    }

    #[test]
    fn csv_translation_inference_uses_later_translation_free_source_occurrences() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("overlays/sources/data")).unwrap();
        fs::write(
            root.path().join("deck.yaml"),
            "deck:\n  id: deck.csv-global\n  name: Global\n  description: ''\n  adapter_ids: {}\nnote_types:\n  note-type.basic:\n    name: Basic\n    field_order: [field.front]\n    fields:\n      field.front:\n        name: Front\n    card_template_order: []\n    card_templates: {}\n    styling: ''\n    adapter_ids: {}\nnotes:\n  note.one:\n    note_type_id: note-type.basic\n    fields:\n      field.front: Hello\n    tags: []\n    adapter_ids: {}\nmedia: {}\ntombstones: []\n",
        )
        .unwrap();
        fs::write(
            root.path().join("overlays/de.yaml"),
            "id: overlay.translation.de\nkind: translation\ntranslations:\n  from_csv:\n    - descriptor: sources/descriptor.yaml\n      parameters:\n        language: de\n      exclude:\n        source_texts: []\n        note_ids: []\n        paths: []\n",
        )
        .unwrap();
        fs::write(
            root.path().join("overlays/extension.yaml"),
            "id: overlay.extension.later\nkind: extension\nnotes:\n  note.two:\n    intent: add\n    note:\n      note_type_id: note-type.basic\n      fields:\n        field.front: Hello\n      tags: []\n      adapter_ids: {}\n",
        )
        .unwrap();
        fs::write(
            root.path().join("overlays/sources/descriptor.yaml"),
            "version: 1\nprimary_table: main\ntables:\n  main:\n    path: data/notes.csv\nparameters:\n  language:\n    type: localized_column\n    default: ''\n    separator: ':'\njoins: []\nnote:\n  id: main.stable_id\n  note_type_id: note-type.basic\n  fields:\n    field.front:\n      column: main.front\n      localized_by: language\n      type: scalar\n  tags:\n    column: main.tags\n    delimiter: '|'\n  adapter_ids: {}\n",
        )
        .unwrap();
        fs::write(
            root.path().join("overlays/sources/data/notes.csv"),
            "stable_id,front,front:de,tags\nnote.one,Hello,Hallo,\n",
        )
        .unwrap();
        fs::write(
            root.path().join("brainbrew.yaml"),
            "base: deck.yaml\noverlays:\n  overlay.translation.de:\n    file: overlays/de.yaml\n    kind: translation\n  overlay.extension.later:\n    file: overlays/extension.yaml\n    kind: extension\ntargets:\n  de:\n    overlays: [overlay.translation.de, overlay.extension.later]\n",
        )
        .unwrap();

        let plan = ManifestRegistry::load(&root.path().join("brainbrew.yaml"), &[], &[])
            .unwrap()
            .plan("de")
            .unwrap();
        let translations = plan.overlays[0].1.translations.as_ref().unwrap();
        assert!(!translations.direct.contains_key("Hello"));
        assert!(!translations.no_change.contains("Hello"));
        assert_eq!(
            translations.contextual["notes.note.one.fields.field.front"]["Hello"],
            "Hallo"
        );
        let composed = plan.compose().unwrap();
        assert_eq!(
            composed.notes[&brain_brew_core::StableId::new("note.one").unwrap()].fields
                [&brain_brew_core::StableId::new("field.front").unwrap()]
                .as_scalar(),
            Some("Hallo")
        );
        assert_eq!(
            composed.notes[&brain_brew_core::StableId::new("note.two").unwrap()].fields
                [&brain_brew_core::StableId::new("field.front").unwrap()]
                .as_scalar(),
            Some("Hello")
        );

        fs::write(
            root.path().join("overlays/second.yaml"),
            "id: overlay.translation.second\nkind: translation\ntranslations:\n  from_csv:\n    - descriptor: sources/second.yaml\n      parameters:\n        language: de\n      exclude:\n        source_texts: []\n        note_ids: []\n        paths: []\n",
        )
        .unwrap();
        fs::write(
            root.path().join("overlays/sources/second.yaml"),
            fs::read_to_string(root.path().join("overlays/sources/descriptor.yaml"))
                .unwrap()
                .replace("data/notes.csv", "data/second.csv"),
        )
        .unwrap();
        fs::write(
            root.path().join("overlays/sources/data/second.csv"),
            "stable_id,front,front:de,tags\nnote.one,Hallo,Guten Tag,\n",
        )
        .unwrap();
        fs::write(
            root.path().join("brainbrew.yaml"),
            "base: deck.yaml\noverlays:\n  overlay.translation.de:\n    file: overlays/de.yaml\n    kind: translation\n  overlay.extension.later:\n    file: overlays/extension.yaml\n    kind: extension\n  overlay.translation.second:\n    file: overlays/second.yaml\n    kind: translation\ntargets:\n  de:\n    overlays: [overlay.translation.de, overlay.extension.later, overlay.translation.second]\n",
        )
        .unwrap();
        let error = match ManifestRegistry::load(&root.path().join("brainbrew.yaml"), &[], &[])
            .unwrap()
            .plan("de")
        {
            Ok(_) => panic!("a second CSV overlay treated prior translated output as source"),
            Err(error) => error,
        };
        assert!(error.contains("CSV translation source mismatch"), "{error}");
        assert!(
            error.contains("descriptor cell is \"Hallo\", resolved source is \"Hello\""),
            "{error}"
        );
    }

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
