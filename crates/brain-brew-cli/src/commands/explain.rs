use serde_json::json;

use crate::args::{parse_manifest_target_args, split_json_flag};
use crate::output::{self, one_line, package_json, semantic_kind_name};
use crate::planner::{
    OverlayExpansionOrigin, PlanSourceKind, TargetExpansionOrigin, plan_manifest_target,
};

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
        println!("sources:");
        for source in plan.sources() {
            println!(
                "  {} {} sha256:{}",
                source_kind_name(&source.kind),
                source.path.display(),
                source.sha256
            );
        }
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
    let sources = plan
        .sources()
        .map(|source| {
            json!({
                "path": source.path.display().to_string(),
                "kind": source_kind_name(&source.kind),
                "sha256": source.sha256,
                "package": source.package.as_ref().map(|package| json!({
                    "id": package.id,
                    "version": package.version,
                })),
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
                        "sources": sources,
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
        Err(report) => Err(output::compose_error(
            "explain",
            json!({
                "package": plan.package.as_ref().map(package_json),
                "target": plan.target,
                "qualified_name": plan.qualified_name,
                "target_expansion": target_expansion,
                "base": plan.base_label,
                "overlay_stack": overlay_stack,
                "sources": sources,
            }),
            &report,
        )),
    }
}

fn source_kind_name(kind: &PlanSourceKind) -> &'static str {
    match kind {
        PlanSourceKind::Base => "base",
        PlanSourceKind::Overlay { .. } => "overlay",
        PlanSourceKind::ScalarInclude { .. } => "scalar_include",
        PlanSourceKind::MediaInclude => "media_include",
        PlanSourceKind::NoteTypesInclude => "note_types_include",
        PlanSourceKind::CsvDescriptor => "csv_descriptor",
        PlanSourceKind::CsvTable { .. } => "csv_table",
    }
}
