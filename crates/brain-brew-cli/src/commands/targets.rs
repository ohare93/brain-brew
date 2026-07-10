use serde_json::json;

use crate::args::{parse_targets_args, split_json_flag};
use crate::output::package_json;
use crate::package_resolver::discover_package_manifests;
use crate::planner::ManifestRegistry;

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let (json_output, rest) = split_json_flag(args);
    let mut target_args = parse_targets_args(&rest)?;
    if target_args.manifest_paths.is_empty() {
        target_args.manifest_paths = discover_package_manifests(&target_args.package_roots)?;
    }
    let Some(root_manifest) = target_args.manifest_paths.first().cloned() else {
        return Err("no Brain Brew package manifests were discovered".to_owned());
    };
    let explicit = target_args
        .manifest_paths
        .iter()
        .skip(1)
        .cloned()
        .collect::<Vec<_>>();
    let registry = ManifestRegistry::load(&root_manifest, &explicit, &target_args.package_roots)?;

    if json_output {
        let mut packages = Vec::new();
        let mut all_targets = Vec::new();
        for loaded in registry.manifests() {
            let mut targets = Vec::new();
            for target in loaded.manifest.targets.keys() {
                let reference = loaded
                    .identity
                    .as_ref()
                    .map(|identity| format!("{}:{target}", identity.id))
                    .unwrap_or_else(|| target.clone());
                let plan = registry.plan(&reference)?;
                let overlays = plan
                    .overlays
                    .iter()
                    .map(|(overlay, _)| {
                        json!({
                            "id": overlay.id,
                            "qualified_id": overlay.qualified_id,
                            "file": overlay.display_file,
                            "kind": format!("{:?}", overlay.kind).to_ascii_lowercase(),
                            "declared_kind": overlay.declared_kind,
                            "package": overlay.package.as_ref().map(|package| json!({
                                "id": package.id,
                                "version": package.version,
                            })),
                        })
                    })
                    .collect::<Vec<_>>();
                let value = json!({
                    "name": target,
                    "qualified_name": plan.qualified_name,
                    "extends": loaded.manifest.targets[target].extends.as_ref(),
                    "overlays": overlays,
                });
                targets.push(value.clone());
                all_targets.push(value);
            }
            packages.push(json!({
                "manifest": loaded.path.display().to_string(),
                "package": loaded.manifest.package.as_ref().map(package_json),
                "targets": targets,
            }));
        }
        let package = registry.root().manifest.package.as_ref().map(package_json);
        println!(
            "{}",
            serde_json::to_string_pretty(
                &json!({"package": package, "targets": all_targets, "packages": packages})
            )
            .unwrap()
        );
    } else {
        for target in registry.target_references() {
            // Planning is intentional: text and JSON listings reject the same
            // missing refs, cycles, identity mismatches, and ambiguous graph.
            registry.plan(&target)?;
            println!("{target}");
        }
    }
    Ok(())
}
