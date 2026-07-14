use std::fs;
use std::path::Path;

use brain_brew_core::{
    CanonicalDeck, Overlay, TranslationCoverageCategory, TranslationStackCoverageStatus,
    validate_deck_content,
};
use brain_brew_formats::{crowdanki, manifest, media_map, strict_yaml};

use crate::args::parse_verify_args;
use crate::help;
use crate::io::{
    SourceContext, include_roots_from_manifest, manifest_root, read_manifest,
    resolve_include_target_for_context, top_level_media_include_path, verify_canonical_deck_format,
    verify_manifest_format, verify_overlay_format,
};
use crate::media_assets::verify_owned_media;
use crate::media_ownership::MediaRootSelections;
use crate::output;
use crate::path_authorization::PathAuthorizer;
use crate::planner::{ManifestRegistry, PlanSourceKind, TargetPlan};

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        print!("{}", help::command("verify").expect("verify help exists"));
        return Ok(());
    }
    let verify_args = parse_verify_args(args)?;
    if verify_args.media_mode == crate::media_verification::MediaVerificationMode::ReferenceOnly
        && !verify_args.media_roots.is_empty()
    {
        return Err("--media-mode reference-only cannot be combined with --media-root because reference-only mode intentionally skips all media root and byte validation".to_owned());
    }
    verify_manifest_format(&verify_args.manifest_path)?;
    let manifest = read_manifest(&verify_args.manifest_path)?;
    let root = manifest_root(&verify_args.manifest_path);
    let registry = ManifestRegistry::load_with_policy(
        &verify_args.manifest_path,
        &verify_args.include_paths,
        &verify_args.package_roots,
        &verify_args.discovery_policy,
    )?;
    for loaded in registry.manifests() {
        verify_manifest_format(&loaded.path)?;
    }
    let target_names = if verify_args.all_targets {
        registry
            .root()
            .manifest
            .targets
            .keys()
            .cloned()
            .collect::<Vec<_>>()
    } else if let Some(target) = verify_args.target {
        vec![target]
    } else {
        return Err(help::usage_error(
            "verify",
            "usage: brainbrew verify [--manifest brainbrew.yaml] --all-targets",
        ));
    };

    let base_deck_path = PathAuthorizer::new("package", &root)?
        .authorize_read(&verify_args.manifest_path, "base", &manifest.base)
        .map_err(|error| error.to_string())?
        .into_path_buf();
    verify_canonical_deck_format(&base_deck_path)?;
    verify_included_media_map_format(
        &base_deck_path,
        &SourceContext {
            root: root.clone(),
            include_roots: include_roots_from_manifest(
                &verify_args.manifest_path,
                &root,
                &manifest,
            )?,
        },
    )?;
    let media_roots = MediaRootSelections::parse(&registry, &verify_args.media_roots, &root)?;
    let mut warnings = Vec::new();
    let mut media_targets = Vec::new();
    for target in &target_names {
        let target_reference = if verify_args.all_targets {
            registry
                .root()
                .identity
                .as_ref()
                .map(|identity| format!("{}:{target}", identity.id))
                .unwrap_or_else(|| target.clone())
        } else {
            target.clone()
        };
        let plan = registry.plan(&target_reference)?;
        verify_plan_source_formats(&plan)?;
        let policy = verify_args.translation_coverage.unwrap_or_else(|| {
            plan.target_manifest
                .targets
                .get(&plan.target)
                .map(|target| target.translation_coverage)
                .unwrap_or_default()
        });
        verify_translation_coverage_policy(&plan, policy, &mut warnings)?;
        let deck = plan.compose().map_err(|report| {
            output::compose_error(
                "verify",
                serde_json::json!({"target": target, "manifest": verify_args.manifest_path}),
                &report,
            )
        })?;
        deck.validate().map_err(|report| {
            output::validation_error(
                "verify",
                serde_json::json!({"target": target, "manifest": verify_args.manifest_path}),
                &report,
            )
        })?;
        let result = verify_owned_media(&plan, &deck, &media_roots, verify_args.media_mode, false)?;
        warnings.extend(
            result
                .warnings
                .into_iter()
                .map(|warning| format!("target {target}: {warning}")),
        );
        media_targets.push(serde_json::json!({
            "target": target,
            "declarations": deck.media.len(),
            "mode": verify_args.media_mode.name(),
            "release_ready": verify_args.media_mode.release_ready(deck.media.len()),
        }));
        if !verify_args.skip_content_validation {
            verify_deck_content(target, &deck)?;
        }
        verify_configured_exports(
            &plan.target_manifest_root,
            &plan.target_manifest,
            &plan.target,
            &deck,
        )?;
    }

    let release_ready = media_targets
        .iter()
        .all(|target| target["release_ready"].as_bool() == Some(true));
    if verify_args.json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "status": "verified",
                "manifest": verify_args.manifest_path.display().to_string(),
                "targets": target_names.len(),
                "media": {
                    "mode": verify_args.media_mode.name(),
                    "release_ready": release_ready,
                    "targets": media_targets,
                },
                "warnings": warnings,
            }))
            .expect("verify status JSON serializes")
        );
    } else {
        for warning in &warnings {
            eprintln!("warning: {warning}");
        }
        let suffix = if target_names.len() == 1 { "" } else { "s" };
        let mut details = vec![("manifest", verify_args.manifest_path.display().to_string())];
        details.push((
            "media verification",
            if release_ready {
                verify_args.media_mode.name().to_owned()
            } else {
                "reference_only (NOT RELEASE-READY)".to_owned()
            },
        ));
        if media_roots.supplied() {
            details.push(("media roots", verify_args.media_roots.join(", ")));
        }
        output::print_success(
            format!("verified {} target{suffix}", target_names.len()),
            &details,
        );
    }
    Ok(())
}

fn verify_included_media_map_format(
    deck_path: &Path,
    context: &SourceContext,
) -> Result<(), String> {
    let input = fs::read_to_string(deck_path)
        .map_err(|error| format!("{}: {error}", deck_path.display()))?;
    strict_yaml::reject_duplicate_keys(&input)
        .map_err(|error| format!("{}: {error}", deck_path.display()))?;
    let value = serde_yaml::from_str::<serde_yaml::Value>(&input)
        .map_err(|error| format!("{}: {error}", deck_path.display()))?;
    let Some(include_path) = top_level_media_include_path(&value)
        .map_err(|error| format!("{}: {error}", deck_path.display()))?
    else {
        return Ok(());
    };
    let media_path = resolve_include_target_for_context(&include_path, context)?;
    let input = fs::read_to_string(&media_path)
        .map_err(|error| format!("{}: {error}", media_path.display()))?;
    media_map::from_str(&input).map_err(|error| format!("{}: {error}", media_path.display()))?;
    let formatted = media_map::format_str(&input)
        .map_err(|error| format!("{}: {error}", media_path.display()))?;
    if formatted != input {
        return Err(format!(
            "{} is not in canonical format",
            media_path.display()
        ));
    }
    Ok(())
}

fn verify_deck_content(target: &str, deck: &CanonicalDeck) -> Result<(), String> {
    let rendered = deck
        .render_variables()
        .map_err(|error| format!("content validation failed for target {target}: {error}"))?;
    let report = validate_deck_content(&rendered);
    if report.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "content validation failed for target {target}: {report}"
        ))
    }
}

fn verify_plan_source_formats(plan: &TargetPlan) -> Result<(), String> {
    verify_canonical_deck_format(&plan.base_source.path)?;
    for source in plan.sources() {
        match &source.kind {
            PlanSourceKind::Overlay { .. } => verify_overlay_format(&source.path)?,
            PlanSourceKind::MediaInclude => {
                let input = fs::read_to_string(&source.path)
                    .map_err(|error| format!("{}: {error}", source.path.display()))?;
                let formatted = media_map::format_str(&input)
                    .map_err(|error| format!("{}: {error}", source.path.display()))?;
                if formatted != input {
                    return Err(format!(
                        "{} is not in canonical format",
                        source.path.display()
                    ));
                }
            }
            PlanSourceKind::Base | PlanSourceKind::ScalarInclude { .. } => {}
        }
    }
    Ok(())
}

fn verify_translation_coverage_policy(
    plan: &TargetPlan,
    policy: manifest::TranslationCoveragePolicy,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let overlays = plan
        .overlays
        .iter()
        .map(|(_, overlay)| overlay.clone())
        .collect::<Vec<_>>();
    if !overlays
        .iter()
        .any(|overlay| overlay.translations.is_some())
    {
        return Ok(());
    }
    let report = plan
        .base
        .translation_stack_coverage(&overlays)
        .map_err(|report| {
            output::compose_error(
                "verify",
                serde_json::json!({"target": plan.target}),
                &report,
            )
        })?;
    let missing = report
        .entries
        .iter()
        .filter(|entry| entry.status == TranslationStackCoverageStatus::UntranslatedFallback)
        .collect::<Vec<_>>();
    let stale = report
        .entries
        .iter()
        .filter(|entry| entry.status == TranslationStackCoverageStatus::Stale)
        .collect::<Vec<_>>();

    if policy == manifest::TranslationCoveragePolicy::Strict && !missing.is_empty() {
        let details = missing
            .iter()
            .take(10)
            .map(|entry| {
                format!(
                    "{} source {:?} introduced by {:?}",
                    entry.source_path, entry.source_text, entry.owner
                )
            })
            .collect::<Vec<_>>();
        return Err(format!(
            "translation coverage strict policy failed for target {}: {} untranslated fallback(s): {}",
            plan.target,
            missing.len(),
            details.join(", ")
        ));
    }
    if policy == manifest::TranslationCoveragePolicy::Strict && !stale.is_empty() {
        let details = stale
            .iter()
            .take(10)
            .map(|entry| {
                format!(
                    "{} old {:?} -> new {:?} introduced by {:?}",
                    entry.source_path,
                    entry.old_source_text.as_deref().unwrap_or(""),
                    entry.source_text,
                    entry.owner
                )
            })
            .collect::<Vec<_>>();
        return Err(format!(
            "translation stale strict policy failed for target {}: {} stale translation(s): {}",
            plan.target,
            stale.len(),
            details.join(", ")
        ));
    }
    if !stale.is_empty() {
        warnings.push(format!(
            "stale translation warning for target {}: {} stale translation(s): {}",
            plan.target,
            stale.len(),
            stale
                .iter()
                .take(10)
                .map(|entry| {
                    format!(
                        "{} old {:?} -> new {:?} introduced by {:?}",
                        entry.source_path,
                        entry.old_source_text.as_deref().unwrap_or(""),
                        entry.source_text,
                        entry.owner
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    Ok(())
}

pub(crate) fn stale_translation_warnings(plan: &TargetPlan) -> Result<Vec<String>, String> {
    stale_translation_warning_messages(plan)
}

pub(crate) fn stale_translation_warnings_for_overlays(
    target: &str,
    base: &CanonicalDeck,
    overlays: &[(String, Overlay)],
) -> Result<Vec<String>, String> {
    stale_translation_warning_messages_for_overlays(target, base, overlays)
}

pub(crate) fn emit_stale_translation_warnings(plan: &TargetPlan) -> Result<(), String> {
    for message in stale_translation_warnings(plan)? {
        eprintln!("warning: {message}");
    }
    Ok(())
}

pub(crate) fn emit_stale_translation_warnings_for_overlays(
    target: &str,
    base: &CanonicalDeck,
    overlays: &[(String, Overlay)],
) -> Result<(), String> {
    for message in stale_translation_warnings_for_overlays(target, base, overlays)? {
        eprintln!("warning: {message}");
    }
    Ok(())
}

fn stale_translation_warning_messages(plan: &TargetPlan) -> Result<Vec<String>, String> {
    let mut messages = Vec::new();
    let mut current = plan.base.clone();
    for (planned, overlay) in &plan.overlays {
        let report = current.translation_coverage(overlay).map_err(|error| {
            format!(
                "failed to resolve translation source fields for target {}: {error}",
                plan.target
            )
        })?;
        if let Some(message) = stale_translation_warning_message(
            &plan.target,
            &planned.id,
            &planned.display_file,
            &report.entries,
        ) {
            messages.push(message);
        }
        current = current
            .compose(std::slice::from_ref(overlay))
            .map_err(|report| {
                output::compose_error(
                    "composition",
                    serde_json::json!({"target": plan.target, "overlay": planned.id}),
                    &report,
                )
            })?;
    }
    Ok(messages)
}

fn stale_translation_warning_messages_for_overlays(
    target: &str,
    base: &CanonicalDeck,
    overlays: &[(String, Overlay)],
) -> Result<Vec<String>, String> {
    let mut messages = Vec::new();
    let mut current = base.clone();
    for (display_file, overlay) in overlays {
        let report = current.translation_coverage(overlay).map_err(|error| {
            format!("failed to resolve translation source fields for target {target}: {error}")
        })?;
        if let Some(message) = stale_translation_warning_message(
            target,
            overlay.id.as_str(),
            display_file,
            &report.entries,
        ) {
            messages.push(message);
        }
        current = current
            .compose(std::slice::from_ref(overlay))
            .map_err(|report| {
                output::compose_error(
                    "composition",
                    serde_json::json!({"target": target, "overlay": overlay.id.as_str()}),
                    &report,
                )
            })?;
    }
    Ok(messages)
}

fn stale_translation_warning_message(
    target: &str,
    overlay_id: &str,
    display_file: &str,
    entries: &[brain_brew_core::TranslationCoverageEntry],
) -> Option<String> {
    let stale_translations = stale_translation_warning_details(entries);
    if stale_translations.is_empty() {
        return None;
    }
    Some(format!(
        "stale translation warning for target {target} overlay {overlay_id} ({display_file}): {} stale translation(s): {}",
        stale_translations.len(),
        stale_translations
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

fn stale_translation_warning_details(
    entries: &[brain_brew_core::TranslationCoverageEntry],
) -> Vec<String> {
    entries
        .iter()
        .filter(|entry| entry.category == TranslationCoverageCategory::StaleTranslation)
        .map(|entry| {
            format!(
                "{} old {:?} -> new {:?}",
                entry.path,
                entry.old_source.as_deref().unwrap_or(""),
                entry.source
            )
        })
        .collect()
}

fn verify_configured_exports(
    root: &Path,
    manifest: &manifest::FederatedDeckManifest,
    target: &str,
    deck: &CanonicalDeck,
) -> Result<(), String> {
    let Some(target_entry) = manifest.targets.get(target) else {
        return Ok(());
    };
    let Some(export) = &target_entry.exports.crowdanki else {
        return Ok(());
    };
    if let Some(golden) = &export.golden {
        verify_crowdanki_golden(root, target, golden, &export.golden_allowlist, deck)?;
    }
    Ok(())
}

fn verify_crowdanki_golden(
    root: &Path,
    target: &str,
    golden: &str,
    golden_allowlist: &[String],
    deck: &CanonicalDeck,
) -> Result<(), String> {
    let authorizer = PathAuthorizer::new("workspace", root)?;
    let declaring = root.join("brainbrew.yaml");
    let mut golden_path = authorizer
        .authorize_read(
            &declaring,
            format!("targets.{target}.exports.crowdanki.golden"),
            golden,
        )
        .map_err(|error| error.to_string())?
        .into_path_buf();
    if golden_path.is_dir() {
        golden_path = authorizer
            .authorize_read(
                &declaring,
                format!("targets.{target}.exports.crowdanki.golden"),
                &format!("{golden}/deck.json"),
            )
            .map_err(|error| error.to_string())?
            .into_path_buf();
    }
    let expected = fs::read_to_string(&golden_path)
        .map_err(|error| format!("{}: {error}", golden_path.display()))?;
    let actual = crowdanki::export_deck(deck)
        .map_err(|error| error.to_string())?
        .deck_json;
    let expected_json = serde_json::from_str::<serde_json::Value>(&expected)
        .map_err(|error| format!("{}: {error}", golden_path.display()))?;
    let actual_json =
        serde_json::from_str::<serde_json::Value>(&actual).expect("CrowdAnki export is valid JSON");
    let options = crowdanki::CrowdAnkiParityOptions {
        allowed_path_globs: golden_allowlist.iter().cloned().collect(),
    };
    if let Err(report) = crowdanki::compare_deck_json_values(&expected_json, &actual_json, &options)
    {
        return Err(format!(
            "CrowdAnki golden mismatch for target {target}: {}\n{report}",
            golden_path.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use brain_brew_core::TranslationCoverageEntry;

    #[test]
    fn stale_translation_warning_message_matches_existing_text() {
        let entries = vec![
            TranslationCoverageEntry {
                category: TranslationCoverageCategory::StaleTranslation,
                path: "notes.note.finland.fields.field.capital".to_owned(),
                source: "Helsinki changed".to_owned(),
                old_source: Some("Helsinki".to_owned()),
                translated: Some("Helsinki".to_owned()),
                context: None,
            },
            TranslationCoverageEntry {
                category: TranslationCoverageCategory::DirectTranslation,
                path: "notes.note.sweden.fields.field.capital".to_owned(),
                source: "Stockholm".to_owned(),
                old_source: None,
                translated: Some("Stockholm".to_owned()),
                context: None,
            },
        ];

        assert_eq!(
            stale_translation_warning_message(
                "en-standard",
                "overlay.translation.en",
                "overlays/languages/en.yaml",
                &entries,
            ),
            Some(
                "stale translation warning for target en-standard overlay overlay.translation.en (overlays/languages/en.yaml): 1 stale translation(s): notes.note.finland.fields.field.capital old \"Helsinki\" -> new \"Helsinki changed\""
                    .to_owned()
            )
        );
    }
}
