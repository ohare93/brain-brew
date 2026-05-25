use serde_json::json;

use crate::args::{parse_targets_args, split_json_flag};
use crate::commands::lock::locked_package_manifest_paths;
use crate::io::{manifest_root, read_manifest, target_package_json};
use crate::output::package_json;
use crate::package_resolver::{discover_package_manifests, validate_package_dependencies};

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let (json_output, rest) = split_json_flag(args);
    let target_args = parse_targets_args(&rest)?;
    let mut manifest_paths = target_args.manifest_paths;
    manifest_paths.extend(discover_package_manifests(&target_args.package_roots)?);
    manifest_paths.sort();
    manifest_paths.dedup();
    let lock_manifest_paths = manifest_paths
        .iter()
        .map(|path| locked_package_manifest_paths(&manifest_root(path).join("brainbrew.lock")))
        .collect::<Result<Vec<_>, String>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    let has_lock_manifest_paths = !lock_manifest_paths.is_empty();
    manifest_paths.extend(lock_manifest_paths);
    manifest_paths.sort();
    manifest_paths.dedup();
    let manifests = manifest_paths
        .iter()
        .map(|path| Ok((path, read_manifest(path)?)))
        .collect::<Result<Vec<_>, String>>()?;
    if !target_args.package_roots.is_empty() || has_lock_manifest_paths {
        validate_package_dependencies(
            &manifests
                .iter()
                .map(|(path, manifest)| (*path, manifest))
                .collect::<Vec<_>>(),
        )?;
    }

    if json_output {
        let packages = manifests
            .iter()
            .map(|(path, manifest)| target_package_json(path, manifest))
            .collect::<Result<Vec<_>, String>>()?;
        let package = manifests
            .first()
            .and_then(|(_, manifest)| manifest.package.as_ref())
            .map(package_json);
        let targets = packages
            .iter()
            .flat_map(|package| package["targets"].as_array().cloned().unwrap_or_default())
            .collect::<Vec<_>>();
        println!(
            "{}",
            serde_json::to_string_pretty(
                &json!({"package": package, "targets": targets, "packages": packages})
            )
            .unwrap()
        );
    } else {
        let qualify = manifests.len() > 1;
        for (_, manifest) in manifests {
            let prefix = manifest.package.as_ref().map(|package| package.id.as_str());
            for target in manifest.targets.keys() {
                if qualify {
                    if let Some(prefix) = prefix {
                        println!("{prefix}:{target}");
                    } else {
                        println!("{target}");
                    }
                } else {
                    println!("{target}");
                }
            }
        }
    }
    Ok(())
}
