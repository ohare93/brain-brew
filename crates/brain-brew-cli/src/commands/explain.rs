use serde_json::json;

use crate::args::{parse_manifest_target_args, split_json_flag};
use crate::io::plan_manifest_target_with_packages;
use crate::output::{one_line, package_json, semantic_kind_name};

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let (json_output, rest) = split_json_flag(args);
    let manifest_args = parse_manifest_target_args(&rest)?;
    let plan = plan_manifest_target_with_packages(
        &manifest_args.manifest_path,
        &manifest_args.target,
        &manifest_args.include_paths,
        &manifest_args.package_roots,
    )?;

    if !json_output {
        if let Some(package) = &plan.package {
            println!("package: {}@{}", package.id, package.version);
        }
        println!("target: {}", plan.target);
        println!("base: {}", plan.base_label);
        println!("overlay stack:");
        if plan.overlays.is_empty() {
            println!("  (none)");
        } else {
            for (index, (overlay, _)) in plan.overlays.iter().enumerate() {
                println!("  {}. {} ({})", index + 1, overlay.id, overlay.display_file);
            }
        }
    }

    let overlay_stack = plan
        .overlays
        .iter()
        .map(|(overlay, _)| json!({"id": overlay.id, "file": overlay.display_file}))
        .collect::<Vec<_>>();
    let overlays = plan
        .overlays
        .iter()
        .map(|(_, overlay)| overlay.clone())
        .collect::<Vec<_>>();
    match plan.base.compose(&overlays) {
        Ok(deck) => {
            let diff = plan.base.semantic_diff(&deck);
            if json_output {
                let changes = diff
                    .changes
                    .iter()
                    .map(|change| {
                        json!({
                            "kind": semantic_kind_name(change.kind),
                            "path": change.path,
                            "before": change.before,
                            "after": change.after,
                        })
                    })
                    .collect::<Vec<_>>();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "package": plan.package.as_ref().map(package_json),
                        "target": plan.target,
                        "base": plan.base_label,
                        "overlay_stack": overlay_stack,
                        "changes": changes,
                        "errors": [],
                    }))
                    .unwrap()
                );
            } else {
                println!("changes:");
                if diff.is_empty() {
                    println!("  none");
                } else {
                    for change in diff.changes {
                        println!("  {} {}", semantic_kind_name(change.kind), change.path);
                        if let Some(before) = change.before {
                            println!("    before: {}", one_line(&before));
                        }
                        if let Some(after) = change.after {
                            println!("    after: {}", one_line(&after));
                        }
                    }
                }
            }
            Ok(())
        }
        Err(report) => {
            if json_output {
                let errors = report
                    .errors
                    .iter()
                    .map(|error| {
                        json!({
                            "kind": format!("{:?}", error.kind),
                            "path": error.path,
                            "message": error.message,
                        })
                    })
                    .collect::<Vec<_>>();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "package": plan.package.as_ref().map(package_json),
                        "target": plan.target,
                        "base": plan.base_label,
                        "overlay_stack": overlay_stack,
                        "changes": [],
                        "errors": errors,
                    }))
                    .unwrap()
                );
            } else {
                for error in report.errors {
                    eprintln!("{:?} {}: {}", error.kind, error.path, error.message);
                }
            }
            Err("target composition failed".to_owned())
        }
    }
}
