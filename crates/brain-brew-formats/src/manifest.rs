use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::Deserialize;

/// Public manifest for a Federated Deck workspace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FederatedDeckManifest {
    pub package: Option<PackageMetadata>,
    pub base: String,
    pub overlays: BTreeMap<String, OverlayManifestEntry>,
    pub targets: BTreeMap<String, BuildTarget>,
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

/// One named composition goal in a Federated Deck manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildTarget {
    pub extends: Option<String>,
    pub overlays: Vec<String>,
    pub exports: TargetExports,
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
    let yaml: ManifestYaml = serde_yaml::from_str(input).map_err(ManifestError::Parse)?;
    Ok(yaml.into_manifest())
}

/// Parse and re-emit a Federated Deck manifest using deterministic formatting.
pub fn format_str(input: &str) -> Result<String, ManifestError> {
    let manifest = from_str(input)?;
    Ok(to_string(&manifest))
}

/// Emit a Federated Deck manifest as deterministic YAML.
pub fn to_string(manifest: &FederatedDeckManifest) -> String {
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

    if manifest.overlays.is_empty() {
        out.push_str("overlays: {}\n");
    } else {
        out.push_str("overlays:\n");
        for (id, overlay) in &manifest.overlays {
            out.push_str(&format!("  {id}:\n"));
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
            out.push_str(&format!("  {id}:\n"));
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
            if !target.exports.is_empty() {
                out.push_str("    exports:\n");
                if let Some(export) = &target.exports.crowdanki {
                    out.push_str("      crowdanki:\n");
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
    out
}

fn yaml_scalar(value: &str) -> String {
    if !value.is_empty()
        && !value.starts_with([
            ' ', '-', '?', ':', '@', '`', '&', '*', '!', '|', '>', '#', '{', '[', ',',
        ])
        && !value.ends_with(' ')
        && value.chars().all(|ch| {
            ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '.' | ',' | '_' | '-' | '/' | ':')
        })
        && !value.chars().all(|ch| ch.is_ascii_digit())
        && !matches!(
            value,
            "true" | "false" | "True" | "False" | "TRUE" | "FALSE" | "null" | "Null" | "NULL"
        )
    {
        value.to_owned()
    } else {
        format!("'{}'", value.replace('\'', "''"))
    }
}

#[derive(Debug)]
pub enum ManifestError {
    Parse(serde_yaml::Error),
    MissingTarget(String),
    MissingOverlay(String),
    DependencyCycle(Vec<String>),
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
    overlays: BTreeMap<String, OverlayManifestEntryYaml>,
    #[serde(default)]
    targets: BTreeMap<String, BuildTargetYaml>,
}

impl ManifestYaml {
    fn into_manifest(self) -> FederatedDeckManifest {
        FederatedDeckManifest {
            package: self.package.map(PackageMetadataYaml::into_metadata),
            base: self.base,
            overlays: self
                .overlays
                .into_iter()
                .map(|(id, overlay)| (id, overlay.into_entry()))
                .collect(),
            targets: self
                .targets
                .into_iter()
                .map(|(id, target)| (id, target.into_target()))
                .collect(),
        }
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
    exports: TargetExportsYaml,
}

impl BuildTargetYaml {
    fn into_target(self) -> BuildTarget {
        BuildTarget {
            extends: self.extends,
            overlays: self.overlays,
            exports: self.exports.into_exports(),
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
