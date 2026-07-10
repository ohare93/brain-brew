use brain_brew_core::ComposePrecondition;
use serde_json::{Value, json};

use crate::args::{parse_manifest_target_args, split_json_flag};
use crate::output::{self, one_line, package_json, semantic_kind_name};
use crate::planner::{OverlayExpansionOrigin, TargetExpansionOrigin, plan_manifest_target};

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let (json_output, rest) = split_json_flag(args);
    let manifest_args = parse_manifest_target_args(&rest)?;
    let plan = plan_manifest_target(
        &manifest_args.manifest_path,
        &manifest_args.target,
        &manifest_args.include_paths,
        &manifest_args.package_roots,
        &manifest_args.discovery_policy,
    )?;

    if !json_output {
        if let Some(package) = &plan.package {
            println!("package: {}@{}", package.id, package.version);
        }
        println!("target: {}", plan.qualified_name);
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
        .map(|(overlay, _)| {
            let origin = match &overlay.origin {
                OverlayExpansionOrigin::Target {
                    qualified_target,
                    reference,
                } => json!({
                    "kind": "target",
                    "target": qualified_target,
                    "reference": reference,
                }),
                OverlayExpansionOrigin::Dependency {
                    declaring_overlay,
                    reference,
                } => json!({
                    "kind": "overlay_dependency",
                    "overlay": declaring_overlay,
                    "reference": reference,
                }),
            };
            json!({
                "id": overlay.id,
                "qualified_id": overlay.qualified_id,
                "file": overlay.display_file,
                "origin": origin,
            })
        })
        .collect::<Vec<_>>();
    let target_expansion = plan
        .target_expansion
        .iter()
        .map(|target| {
            let origin = match &target.origin {
                TargetExpansionOrigin::Selection => json!({"kind": "selection"}),
                TargetExpansionOrigin::Extends {
                    declaring_target,
                    reference,
                } => json!({
                    "kind": "extends",
                    "target": declaring_target,
                    "reference": reference,
                }),
            };
            json!({
                "qualified_name": target.qualified_name,
                "package": target.owner.as_ref().map(|package| json!({
                    "id": package.id,
                    "version": package.version,
                })),
                "origin": origin,
            })
        })
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
                        "qualified_name": plan.qualified_name,
                        "owner": plan.owner.as_ref().map(|package| json!({
                            "id": package.id,
                            "version": package.version,
                        })),
                        "target_expansion": target_expansion,
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
                            "code": error.kind.code(),
                            "category": error.kind.category(),
                            "path": error.path,
                            "deck_path": error.deck_path.as_ref().map(ToString::to_string),
                            "entity_kind": error.entity_kind.map(|kind| kind.as_str()),
                            "intent": error.intent.map(|intent| intent.as_str()),
                            "overlay": error.overlay_id.as_ref().map(ToString::to_string),
                            "expected": error.expected.as_ref().map(precondition_json),
                            "actual": error.actual.as_ref().map(precondition_json),
                            "message": error.message,
                        })
                    })
                    .collect::<Vec<_>>();
                output::print_json_error_value(json!({
                    "message": "target composition failed",
                    "errors": errors,
                    "package": plan.package.as_ref().map(package_json),
                    "target": plan.target,
                    "qualified_name": plan.qualified_name,
                    "owner": plan.owner.as_ref().map(|package| json!({
                        "id": package.id,
                        "version": package.version,
                    })),
                    "target_expansion": target_expansion,
                    "base": plan.base_label,
                    "overlay_stack": overlay_stack,
                }));
                Err(output::JSON_ERROR_ALREADY_PRINTED.to_owned())
            } else {
                for error in report.errors {
                    eprintln!("{:?} {}: {}", error.kind, error.path, error.message);
                }
                Err("target composition failed".to_owned())
            }
        }
    }
}

fn precondition_json(value: &ComposePrecondition) -> Value {
    match value {
        ComposePrecondition::Fingerprint(fingerprint) => {
            json!({"kind": "entity_fingerprint", "value": fingerprint.to_string()})
        }
        ComposePrecondition::Value(value) => json!({"kind": "value", "value": value}),
        ComposePrecondition::FieldValue(value) => {
            json!({"kind": "field_value", "value": format!("{value:?}")})
        }
        ComposePrecondition::Missing => json!({"kind": "missing"}),
    }
}
