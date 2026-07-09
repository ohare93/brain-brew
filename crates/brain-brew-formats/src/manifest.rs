use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::Deserialize;

use crate::yaml_scalar::{
    is_emittable_key as is_emittable_yaml_key, key as yaml_key, scalar as yaml_scalar,
};

/// Public manifest for a Federated Deck workspace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FederatedDeckManifest {
    pub package: Option<PackageMetadata>,
    pub base: String,
    pub include_roots: Vec<String>,
    pub overlays: BTreeMap<String, OverlayManifestEntry>,
    pub targets: BTreeMap<String, BuildTarget>,
    pub languages: BTreeMap<String, LanguageManifestEntry>,
    pub translation_profile: TranslationProfile,
}

impl FederatedDeckManifest {
    /// Expand one target into the deterministic overlay stack implied by its dependencies.
    pub fn expand_target(&self, target: &str) -> Result<ExpandedTarget, ManifestError> {
        let target_entry = self
            .targets
            .get(target)
            .ok_or_else(|| ManifestError::MissingTarget(target.to_owned()))?;
        let mut visited = BTreeSet::new();
        let mut stack = Vec::new();
        let mut overlays = Vec::new();

        for overlay in &target_entry.overlays {
            self.visit_overlay(overlay, &mut visited, &mut stack, &mut overlays)?;
        }

        Ok(ExpandedTarget {
            name: target.to_owned(),
            base: self.base.clone(),
            extends: target_entry.extends.clone(),
            overlays,
        })
    }

    fn validate_language_catalog(&self) -> Result<(), ManifestError> {
        self.validate_emittable_yaml_keys()?;
        self.validate_translation_profile()?;

        for (code, language) in &self.languages {
            if language.source && !language.translation_overlays.is_empty() {
                return Err(ManifestError::SourceLanguageHasTranslationOverlays(
                    code.clone(),
                ));
            }
            if !language.targets.contains_key(&language.primary_target) {
                return Err(ManifestError::MissingLanguagePrimaryTargetLabel {
                    language: code.clone(),
                    primary_target: language.primary_target.clone(),
                });
            }
            for (label, target) in &language.targets {
                if !self.targets.contains_key(target) {
                    return Err(ManifestError::MissingLanguageTarget {
                        language: code.clone(),
                        label: label.clone(),
                        target: target.clone(),
                    });
                }
            }
            for (label, overlay) in &language.translation_overlays {
                let Some(entry) = self.overlays.get(overlay) else {
                    return Err(ManifestError::MissingLanguageTranslationOverlay {
                        language: code.clone(),
                        label: label.clone(),
                        overlay: overlay.clone(),
                    });
                };
                if let Some(kind) = &entry.kind
                    && kind != "translation"
                {
                    return Err(ManifestError::LanguageTranslationOverlayHasWrongKind {
                        language: code.clone(),
                        label: label.clone(),
                        overlay: overlay.clone(),
                        kind: kind.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    fn validate_emittable_yaml_keys(&self) -> Result<(), ManifestError> {
        validate_map_keys("overlays", self.overlays.keys())?;
        validate_map_keys("targets", self.targets.keys())?;
        validate_map_keys("languages", self.languages.keys())?;
        for language in self.languages.values() {
            validate_map_keys(
                "languages.translation_overlays",
                language.translation_overlays.keys(),
            )?;
            validate_map_keys("languages.targets", language.targets.keys())?;
        }
        Ok(())
    }

    fn validate_translation_profile(&self) -> Result<(), ManifestError> {
        let mut category_keys = BTreeSet::new();
        for category in &self.translation_profile.metadata_categories {
            if !category_keys.insert(category.key.clone()) {
                return Err(ManifestError::DuplicateMetadataCategoryKey(
                    category.key.clone(),
                ));
            }
        }
        for key in &self.translation_profile.metadata_category_order {
            if !category_keys.contains(key) {
                return Err(ManifestError::UnknownMetadataCategoryOrderKey(key.clone()));
            }
        }
        Ok(())
    }

    fn visit_overlay(
        &self,
        overlay: &str,
        visited: &mut BTreeSet<String>,
        stack: &mut Vec<String>,
        expanded: &mut Vec<ExpandedOverlay>,
    ) -> Result<(), ManifestError> {
        if visited.contains(overlay) {
            return Ok(());
        }
        if stack.iter().any(|candidate| candidate == overlay) {
            let mut cycle = stack.clone();
            cycle.push(overlay.to_owned());
            return Err(ManifestError::DependencyCycle(cycle));
        }

        let entry = self
            .overlays
            .get(overlay)
            .ok_or_else(|| ManifestError::MissingOverlay(overlay.to_owned()))?;
        stack.push(overlay.to_owned());
        for dependency in &entry.depends_on {
            self.visit_overlay(dependency, visited, stack, expanded)?;
        }
        stack.pop();

        visited.insert(overlay.to_owned());
        expanded.push(ExpandedOverlay {
            id: overlay.to_owned(),
            file: entry.file.clone(),
        });
        Ok(())
    }
}

/// Package identity and dependency metadata for a Federated Deck workspace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageMetadata {
    pub id: String,
    pub version: String,
    pub compatible_base_versions: Vec<String>,
    pub depends_on: Vec<String>,
}

/// One overlay available in a Federated Deck manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OverlayManifestEntry {
    pub file: String,
    pub kind: Option<String>,
    pub depends_on: Vec<String>,
}

/// Translation coverage policy enforced during verification for one target.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TranslationCoveragePolicy {
    /// Missing translations are reported by `brainbrew translations` but do not fail verify.
    #[default]
    Lenient,
    /// Missing untranslated fallbacks fail verify for release-ready targets.
    Strict,
}

/// One named composition goal in a Federated Deck manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildTarget {
    pub extends: Option<String>,
    pub overlays: Vec<String>,
    pub translation_coverage: TranslationCoveragePolicy,
    pub exports: TargetExports,
}

/// One source or target language available in a Federated Deck workspace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LanguageManifestEntry {
    pub display_name: String,
    pub source: bool,
    pub translation_overlays: BTreeMap<String, String>,
    pub primary_target: String,
    pub targets: BTreeMap<String, String>,
}

/// Workspace-wide translation classification used by language-first tooling.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TranslationProfile {
    pub structural_fields: Vec<String>,
    pub metadata_categories: Vec<MetadataCategory>,
    pub metadata_paths: Vec<String>,
    pub metadata_exclude_paths: Vec<String>,
    pub metadata_category_order: Vec<String>,
}

/// One configurable metadata checklist category.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetadataCategory {
    pub key: String,
    pub label: String,
    pub paths: Vec<String>,
}

impl TranslationProfile {
    fn is_empty(&self) -> bool {
        self.structural_fields.is_empty()
            && self.metadata_categories.is_empty()
            && self.metadata_paths.is_empty()
            && self.metadata_exclude_paths.is_empty()
            && self.metadata_category_order.is_empty()
    }
}

/// Optional reproducibility checks and default outputs for one target.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TargetExports {
    pub crowdanki: Option<CrowdAnkiTargetExport>,
}

impl TargetExports {
    pub fn is_empty(&self) -> bool {
        self.crowdanki.is_none()
    }
}

/// CrowdAnki export defaults and golden-file verification for one target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrowdAnkiTargetExport {
    pub out: Option<String>,
    pub golden: Option<String>,
    pub golden_allowlist: Vec<String>,
}

/// Expanded target ready for filesystem loading and composition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpandedTarget {
    pub name: String,
    pub base: String,
    pub extends: Option<String>,
    pub overlays: Vec<ExpandedOverlay>,
}

/// One overlay in an expanded deterministic overlay stack.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpandedOverlay {
    pub id: String,
    pub file: String,
}

/// Parse a Federated Deck manifest from strict YAML.
pub fn from_str(input: &str) -> Result<FederatedDeckManifest, ManifestError> {
    crate::strict_yaml::reject_duplicate_keys(input).map_err(ManifestError::Parse)?;
    let yaml: ManifestYaml = serde_yaml::from_str(input).map_err(ManifestError::Parse)?;
    crate::strict_yaml::reject_unintended_scalars(
        input,
        crate::strict_yaml::ScalarPolicy::Manifest,
    )
    .map_err(ManifestError::Parse)?;
    yaml.into_manifest()
}

/// Parse and re-emit a Federated Deck manifest using deterministic formatting.
pub fn format_str(input: &str) -> Result<String, ManifestError> {
    let manifest = from_str(input)?;
    to_string(&manifest)
}

/// Emit a Federated Deck manifest as deterministic YAML.
pub fn to_string(manifest: &FederatedDeckManifest) -> Result<String, ManifestError> {
    manifest.validate_language_catalog()?;
    let mut out = String::new();
    if let Some(package) = &manifest.package {
        out.push_str("package:\n");
        out.push_str(&format!("  id: {}\n", yaml_scalar(&package.id)));
        out.push_str(&format!("  version: {}\n", yaml_scalar(&package.version)));
        if !package.compatible_base_versions.is_empty() {
            out.push_str("  compatible_base_versions:\n");
            for version in &package.compatible_base_versions {
                out.push_str(&format!("    - {}\n", yaml_scalar(version)));
            }
        }
        if !package.depends_on.is_empty() {
            out.push_str("  depends_on:\n");
            for dependency in &package.depends_on {
                out.push_str(&format!("    - {}\n", yaml_scalar(dependency)));
            }
        }
    }
    out.push_str(&format!("base: {}\n", yaml_scalar(&manifest.base)));
    if !manifest.include_roots.is_empty() {
        out.push_str("include_roots:\n");
        for root in &manifest.include_roots {
            out.push_str(&format!("  - {}\n", yaml_scalar(root)));
        }
    }

    if manifest.overlays.is_empty() {
        out.push_str("overlays: {}\n");
    } else {
        out.push_str("overlays:\n");
        for (id, overlay) in &manifest.overlays {
            out.push_str(&format!("  {}:\n", emitted_key(id)));
            out.push_str(&format!("    file: {}\n", yaml_scalar(&overlay.file)));
            if let Some(kind) = &overlay.kind {
                out.push_str(&format!("    kind: {}\n", yaml_scalar(kind)));
            }
            if !overlay.depends_on.is_empty() {
                out.push_str("    depends_on:\n");
                for dependency in &overlay.depends_on {
                    out.push_str(&format!("      - {}\n", yaml_scalar(dependency)));
                }
            }
        }
    }

    if manifest.targets.is_empty() {
        out.push_str("targets: {}\n");
    } else {
        out.push_str("targets:\n");
        for (id, target) in &manifest.targets {
            out.push_str(&format!("  {}:\n", emitted_key(id)));
            if let Some(extends) = &target.extends {
                out.push_str(&format!("    extends: {}\n", yaml_scalar(extends)));
            }
            if target.overlays.is_empty() {
                out.push_str("    overlays: []\n");
            } else {
                out.push_str("    overlays:\n");
                for overlay in &target.overlays {
                    out.push_str(&format!("      - {}\n", yaml_scalar(overlay)));
                }
            }
            if target.translation_coverage != TranslationCoveragePolicy::Lenient {
                out.push_str(&format!(
                    "    translation_coverage: {}\n",
                    translation_coverage_policy_name(target.translation_coverage)
                ));
            }
            if !target.exports.is_empty() {
                out.push_str("    exports:\n");
                if let Some(export) = &target.exports.crowdanki {
                    if export.out.is_none()
                        && export.golden.is_none()
                        && export.golden_allowlist.is_empty()
                    {
                        out.push_str("      crowdanki: {}\n");
                    } else {
                        out.push_str("      crowdanki:\n");
                    }
                    if let Some(path) = &export.out {
                        out.push_str(&format!("        out: {}\n", yaml_scalar(path)));
                    }
                    if let Some(path) = &export.golden {
                        out.push_str(&format!("        golden: {}\n", yaml_scalar(path)));
                    }
                    if !export.golden_allowlist.is_empty() {
                        out.push_str("        golden_allowlist:\n");
                        for path in &export.golden_allowlist {
                            out.push_str(&format!("          - {}\n", yaml_scalar(path)));
                        }
                    }
                }
            }
        }
    }

    if !manifest.languages.is_empty() {
        out.push_str("languages:\n");
        for (code, language) in &manifest.languages {
            out.push_str(&format!("  {}:\n", emitted_key(code)));
            out.push_str(&format!(
                "    display_name: {}\n",
                yaml_scalar(&language.display_name)
            ));
            if language.source {
                out.push_str("    source: true\n");
            }
            if !language.translation_overlays.is_empty() {
                out.push_str("    translation_overlays:\n");
                for (label, overlay) in &language.translation_overlays {
                    out.push_str(&format!(
                        "      {}: {}\n",
                        emitted_key(label),
                        yaml_scalar(overlay)
                    ));
                }
            }
            out.push_str(&format!(
                "    primary_target: {}\n",
                yaml_scalar(&language.primary_target)
            ));
            if language.targets.is_empty() {
                out.push_str("    targets: {}\n");
            } else {
                out.push_str("    targets:\n");
                for (label, target) in &language.targets {
                    out.push_str(&format!(
                        "      {}: {}\n",
                        emitted_key(label),
                        yaml_scalar(target)
                    ));
                }
            }
        }
    }

    if !manifest.translation_profile.is_empty() {
        out.push_str("translation_profile:\n");
        if !manifest.translation_profile.structural_fields.is_empty() {
            out.push_str("  structural_fields:\n");
            for field in &manifest.translation_profile.structural_fields {
                out.push_str(&format!("    - {}\n", yaml_scalar(field)));
            }
        }
        if !manifest.translation_profile.metadata_categories.is_empty() {
            out.push_str("  metadata_categories:\n");
            for category in &manifest.translation_profile.metadata_categories {
                out.push_str(&format!("    - key: {}\n", yaml_scalar(&category.key)));
                out.push_str(&format!("      label: {}\n", yaml_scalar(&category.label)));
                if category.paths.is_empty() {
                    out.push_str("      paths: []\n");
                } else {
                    out.push_str("      paths:\n");
                    for path in &category.paths {
                        out.push_str(&format!("        - {}\n", yaml_scalar(path)));
                    }
                }
            }
        }
        if !manifest.translation_profile.metadata_paths.is_empty() {
            out.push_str("  metadata_paths:\n");
            for path in &manifest.translation_profile.metadata_paths {
                out.push_str(&format!("    - {}\n", yaml_scalar(path)));
            }
        }
        if !manifest
            .translation_profile
            .metadata_exclude_paths
            .is_empty()
        {
            out.push_str("  metadata_exclude_paths:\n");
            for path in &manifest.translation_profile.metadata_exclude_paths {
                out.push_str(&format!("    - {}\n", yaml_scalar(path)));
            }
        }
        if !manifest
            .translation_profile
            .metadata_category_order
            .is_empty()
        {
            out.push_str("  metadata_category_order:\n");
            for category in &manifest.translation_profile.metadata_category_order {
                out.push_str(&format!("    - {}\n", yaml_scalar(category)));
            }
        }
    }
    Ok(out)
}

fn translation_coverage_policy_name(policy: TranslationCoveragePolicy) -> &'static str {
    match policy {
        TranslationCoveragePolicy::Lenient => "lenient",
        TranslationCoveragePolicy::Strict => "strict",
    }
}

fn emitted_key(value: &str) -> String {
    yaml_key(value).expect("manifest key was not prevalidated")
}

fn validate_map_keys<K>(
    section: &'static str,
    keys: impl IntoIterator<Item = K>,
) -> Result<(), ManifestError>
where
    K: AsRef<str>,
{
    for key in keys {
        let key = key.as_ref();
        if !is_emittable_yaml_key(key) {
            return Err(ManifestError::UnemittableYamlKey {
                section,
                key: key.to_owned(),
            });
        }
    }
    Ok(())
}

fn parse_translation_coverage_policy(
    value: &str,
) -> Result<TranslationCoveragePolicy, ManifestError> {
    match value {
        "lenient" => Ok(TranslationCoveragePolicy::Lenient),
        "strict" => Ok(TranslationCoveragePolicy::Strict),
        other => Err(ManifestError::InvalidTranslationCoveragePolicy(
            other.to_owned(),
        )),
    }
}

#[derive(Debug)]
pub enum ManifestError {
    Parse(serde_yaml::Error),
    MissingTarget(String),
    MissingOverlay(String),
    DependencyCycle(Vec<String>),
    InvalidTranslationCoveragePolicy(String),
    SourceLanguageHasTranslationOverlays(String),
    MissingLanguagePrimaryTargetLabel {
        language: String,
        primary_target: String,
    },
    MissingLanguageTarget {
        language: String,
        label: String,
        target: String,
    },
    MissingLanguageTranslationOverlay {
        language: String,
        label: String,
        overlay: String,
    },
    LanguageTranslationOverlayHasWrongKind {
        language: String,
        label: String,
        overlay: String,
        kind: String,
    },
    DuplicateMetadataCategoryKey(String),
    UnknownMetadataCategoryOrderKey(String),
    UnemittableYamlKey {
        section: &'static str,
        key: String,
    },
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "failed to parse manifest YAML: {error}"),
            Self::MissingTarget(target) => write!(f, "manifest target {target:?} does not exist"),
            Self::MissingOverlay(overlay) => {
                write!(f, "manifest overlay {overlay:?} does not exist")
            }
            Self::DependencyCycle(cycle) => {
                write!(
                    f,
                    "manifest overlay dependency cycle: {}",
                    cycle.join(" -> ")
                )
            }
            Self::InvalidTranslationCoveragePolicy(policy) => write!(
                f,
                "invalid translation coverage policy {policy:?}; expected lenient or strict"
            ),
            Self::SourceLanguageHasTranslationOverlays(language) => write!(
                f,
                "manifest source language {language:?} must not declare translation_overlays"
            ),
            Self::MissingLanguagePrimaryTargetLabel {
                language,
                primary_target,
            } => write!(
                f,
                "manifest language {language:?} primary_target {primary_target:?} is not present in its targets map"
            ),
            Self::MissingLanguageTarget {
                language,
                label,
                target,
            } => write!(
                f,
                "manifest language {language:?} target {label:?} references missing build target {target:?}"
            ),
            Self::MissingLanguageTranslationOverlay {
                language,
                label,
                overlay,
            } => write!(
                f,
                "manifest language {language:?} translation overlay {label:?} references missing overlay {overlay:?}"
            ),
            Self::LanguageTranslationOverlayHasWrongKind {
                language,
                label,
                overlay,
                kind,
            } => write!(
                f,
                "manifest language {language:?} translation overlay {label:?} references overlay {overlay:?} with kind {kind:?}; expected translation"
            ),
            Self::DuplicateMetadataCategoryKey(key) => {
                write!(
                    f,
                    "manifest translation profile metadata category key {key:?} is duplicated"
                )
            }
            Self::UnknownMetadataCategoryOrderKey(key) => write!(
                f,
                "manifest translation profile metadata_category_order references unknown category key {key:?}"
            ),
            Self::UnemittableYamlKey { section, key } => {
                write!(f, "manifest {section} key {key:?} cannot be emitted safely")
            }
        }
    }
}

impl std::error::Error for ManifestError {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestYaml {
    #[serde(default)]
    package: Option<PackageMetadataYaml>,
    base: String,
    #[serde(default)]
    include_roots: Vec<String>,
    #[serde(default)]
    overlays: BTreeMap<String, OverlayManifestEntryYaml>,
    #[serde(default)]
    targets: BTreeMap<String, BuildTargetYaml>,
    #[serde(default)]
    languages: BTreeMap<String, LanguageManifestEntryYaml>,
    #[serde(default)]
    translation_profile: TranslationProfileYaml,
}

impl ManifestYaml {
    fn into_manifest(self) -> Result<FederatedDeckManifest, ManifestError> {
        let manifest = FederatedDeckManifest {
            package: self.package.map(PackageMetadataYaml::into_metadata),
            base: self.base,
            include_roots: self.include_roots,
            overlays: self
                .overlays
                .into_iter()
                .map(|(id, overlay)| (id, overlay.into_entry()))
                .collect(),
            targets: self
                .targets
                .into_iter()
                .map(|(id, target)| Ok((id, target.into_target()?)))
                .collect::<Result<_, ManifestError>>()?,
            languages: self
                .languages
                .into_iter()
                .map(|(code, language)| (code, language.into_entry()))
                .collect(),
            translation_profile: self.translation_profile.into_profile(),
        };
        manifest.validate_language_catalog()?;
        Ok(manifest)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PackageMetadataYaml {
    id: String,
    version: String,
    #[serde(default)]
    compatible_base_versions: Vec<String>,
    #[serde(default)]
    depends_on: Vec<String>,
}

impl PackageMetadataYaml {
    fn into_metadata(self) -> PackageMetadata {
        PackageMetadata {
            id: self.id,
            version: self.version,
            compatible_base_versions: self.compatible_base_versions,
            depends_on: self.depends_on,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OverlayManifestEntryYaml {
    file: String,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    depends_on: Vec<String>,
}

impl OverlayManifestEntryYaml {
    fn into_entry(self) -> OverlayManifestEntry {
        OverlayManifestEntry {
            file: self.file,
            kind: self.kind,
            depends_on: self.depends_on,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BuildTargetYaml {
    #[serde(default)]
    extends: Option<String>,
    #[serde(default)]
    overlays: Vec<String>,
    #[serde(default)]
    translation_coverage: Option<String>,
    #[serde(default)]
    exports: TargetExportsYaml,
}

impl BuildTargetYaml {
    fn into_target(self) -> Result<BuildTarget, ManifestError> {
        let translation_coverage = self
            .translation_coverage
            .as_deref()
            .map(parse_translation_coverage_policy)
            .transpose()?
            .unwrap_or_default();
        Ok(BuildTarget {
            extends: self.extends,
            overlays: self.overlays,
            translation_coverage,
            exports: self.exports.into_exports(),
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LanguageManifestEntryYaml {
    display_name: String,
    #[serde(default)]
    source: bool,
    #[serde(default)]
    translation_overlays: BTreeMap<String, String>,
    primary_target: String,
    #[serde(default)]
    targets: BTreeMap<String, String>,
}

impl LanguageManifestEntryYaml {
    fn into_entry(self) -> LanguageManifestEntry {
        LanguageManifestEntry {
            display_name: self.display_name,
            source: self.source,
            translation_overlays: self.translation_overlays,
            primary_target: self.primary_target,
            targets: self.targets,
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct TranslationProfileYaml {
    #[serde(default)]
    structural_fields: Vec<String>,
    #[serde(default)]
    metadata_categories: Vec<MetadataCategoryYaml>,
    #[serde(default)]
    metadata_paths: Vec<String>,
    #[serde(default)]
    metadata_exclude_paths: Vec<String>,
    #[serde(default)]
    metadata_category_order: Vec<String>,
}

impl TranslationProfileYaml {
    fn into_profile(self) -> TranslationProfile {
        TranslationProfile {
            structural_fields: self.structural_fields,
            metadata_categories: self
                .metadata_categories
                .into_iter()
                .map(MetadataCategoryYaml::into_category)
                .collect(),
            metadata_paths: self.metadata_paths,
            metadata_exclude_paths: self.metadata_exclude_paths,
            metadata_category_order: self.metadata_category_order,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MetadataCategoryYaml {
    key: String,
    label: String,
    #[serde(default)]
    paths: Vec<String>,
}

impl MetadataCategoryYaml {
    fn into_category(self) -> MetadataCategory {
        MetadataCategory {
            key: self.key,
            label: self.label,
            paths: self.paths,
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetExportsYaml {
    #[serde(default)]
    crowdanki: Option<CrowdAnkiTargetExportYaml>,
}

impl TargetExportsYaml {
    fn into_exports(self) -> TargetExports {
        TargetExports {
            crowdanki: self.crowdanki.map(CrowdAnkiTargetExportYaml::into_export),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CrowdAnkiTargetExportYaml {
    #[serde(default)]
    out: Option<String>,
    #[serde(default)]
    golden: Option<String>,
    #[serde(default)]
    golden_allowlist: Vec<String>,
}

impl CrowdAnkiTargetExportYaml {
    fn into_export(self) -> CrowdAnkiTargetExport {
        CrowdAnkiTargetExport {
            out: self.out,
            golden: self.golden,
            golden_allowlist: self.golden_allowlist,
        }
    }
}
