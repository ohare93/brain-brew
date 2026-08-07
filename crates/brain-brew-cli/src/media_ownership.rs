//! Package-qualified media-root selection and final declaration ownership.
//!
//! A media path never chooses its filesystem root. The registry/planner chooses
//! the declaration owner first; this module then binds that owner to exactly one
//! caller-authorized root.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::planner::{ManifestRegistry, MediaDeclarationProvenance, TargetPlan};

#[derive(Clone, Debug, Default)]
pub(crate) struct MediaRootSelections {
    roots: BTreeMap<String, PathBuf>,
    supplied: bool,
}

impl MediaRootSelections {
    pub(crate) fn parse(
        registry: &ManifestRegistry,
        raw_values: &[String],
        caller_root: &Path,
    ) -> Result<Self, String> {
        let known = registry
            .manifests()
            .iter()
            .filter_map(|loaded| loaded.identity.as_ref().map(|identity| identity.id.clone()))
            .collect::<BTreeSet<_>>();
        let root_key = registry
            .root()
            .identity
            .as_ref()
            .map(|identity| identity.id.clone())
            .unwrap_or_else(|| root_workspace_key(&registry.root().root));
        let mut roots = BTreeMap::new();
        for raw in raw_values {
            let (package, path) = match raw.split_once('=') {
                Some((package, path)) => {
                    if package.is_empty() || path.is_empty() {
                        return Err(format!(
                            "invalid package-qualified media root {raw:?}; expected <package-id>=<directory>"
                        ));
                    }
                    if !known.contains(package) {
                        return Err(format!(
                            "unknown package {package:?} in --media-root {raw:?}; known packages: {}",
                            known.iter().cloned().collect::<Vec<_>>().join(", ")
                        ));
                    }
                    (package.to_owned(), PathBuf::from(path))
                }
                None => (root_key.clone(), PathBuf::from(raw)),
            };
            if roots.contains_key(&package) {
                return Err(format!(
                    "duplicate media root for package {package:?}; select exactly one root"
                ));
            }
            let requested = if path.is_absolute() {
                path
            } else {
                caller_root.join(path)
            };
            let canonical = fs::canonicalize(&requested).map_err(|error| {
                format!(
                    "selected media root for package {package:?} {}: {error}",
                    requested.display()
                )
            })?;
            if !canonical.is_dir() {
                return Err(format!(
                    "selected media root for package {package:?} {} is not a directory",
                    canonical.display()
                ));
            }
            roots.insert(package, canonical);
        }
        Ok(Self {
            roots,
            supplied: !raw_values.is_empty(),
        })
    }

    pub(crate) fn supplied(&self) -> bool {
        self.supplied
    }

    pub(crate) fn require_for_plan(&self, plan: &TargetPlan) -> Result<(), String> {
        if !self.supplied {
            return Ok(());
        }
        for declaration in plan
            .media_declarations
            .values()
            .filter(|declaration| declaration.asset_source.is_none())
        {
            self.require_for_declaration(&plan.qualified_name, declaration)?;
        }
        Ok(())
    }

    pub(crate) fn require_for_declaration(
        &self,
        target: &str,
        declaration: &MediaDeclarationProvenance,
    ) -> Result<&Path, String> {
        let key = declaration_key(declaration);
        self.roots.get(&key).map(PathBuf::as_path).ok_or_else(|| {
            format!(
                "missing media root for target {target}, package {}, declaration {}, path {:?}; pass --media-root {}=<directory>",
                declaration.package_label(),
                declaration.id,
                declaration.path,
                declaration.package_label()
            )
        })
    }

    pub(crate) fn explicit_for_declaration(
        &self,
        declaration: &MediaDeclarationProvenance,
    ) -> Option<&Path> {
        self.roots
            .get(&declaration_key(declaration))
            .map(PathBuf::as_path)
    }
}

pub(crate) fn declaration_key(declaration: &MediaDeclarationProvenance) -> String {
    declaration
        .package
        .as_ref()
        .map(|package| package.id.clone())
        .unwrap_or_else(|| root_workspace_key(&declaration.package_root))
}

fn root_workspace_key(root: &Path) -> String {
    format!("<root:{}>", root.display())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn unqualified_root_is_root_package_only_and_duplicates_and_unknowns_fail() {
        let temp = tempfile::tempdir().unwrap();
        let package = temp.path().join("package");
        let media = temp.path().join("media");
        fs::create_dir_all(&package).unwrap();
        fs::create_dir_all(&media).unwrap();
        fs::write(package.join("deck.yaml"), "deck:\n  id: deck.x\n  name: X\n  description: X\nnote_types: {}\nnotes: {}\nmedia: {}\ntombstones: []\n").unwrap();
        fs::write(package.join("brainbrew.yaml"), "package:\n  id: example.root\n  version: 1.0.0\nbase: deck.yaml\noverlays: {}\ntargets:\n  base:\n    overlays: []\n").unwrap();
        let registry = ManifestRegistry::load(&package.join("brainbrew.yaml"), &[], &[]).unwrap();

        let selected =
            MediaRootSelections::parse(&registry, &[media.display().to_string()], temp.path())
                .unwrap();
        assert!(selected.roots.contains_key("example.root"));

        let duplicate = MediaRootSelections::parse(
            &registry,
            &[
                media.display().to_string(),
                format!("example.root={}", media.display()),
            ],
            temp.path(),
        )
        .unwrap_err();
        assert!(duplicate.contains("duplicate media root"), "{duplicate}");

        let unknown = MediaRootSelections::parse(
            &registry,
            &[format!("example.unknown={}", media.display())],
            temp.path(),
        )
        .unwrap_err();
        assert!(unknown.contains("unknown package"), "{unknown}");
    }
}
