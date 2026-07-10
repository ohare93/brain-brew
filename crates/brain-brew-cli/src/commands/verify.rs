use std::fs;
use std::path::Path;

use brain_brew_core::{CanonicalDeck, Overlay, TranslationCoverageCategory, validate_deck_content};
use brain_brew_formats::{crowdanki, manifest, media_map, strict_yaml};

use crate::args::parse_verify_args;
use crate::help;
use crate::io::{
    SourceContext, include_roots_from_manifest, manifest_root, read_manifest,
    resolve_include_target_for_context, root_relative_path, top_level_media_include_path,
    verify_canonical_deck_format, verify_manifest_format, verify_overlay_format,
};
use crate::media_assets::{validate_media_assets, validate_media_references};
use crate::output;
use crate::path_authorization::PathAuthorizer;
use crate::planner::{ManifestRegistry, PlanSourceKind, TargetPlan};

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        print!("{}", help::command("verify").expect("verify help exists"));
        return Ok(());
    }
    let verify_args = parse_verify_args(args)?;
    verify_manifest_format(&verify_args.manifest_path)?;
    let manifest = read_manifest(&verify_args.manifest_path)?;
    let root = manifest_root(&verify_args.manifest_path);
    let registry = ManifestRegistry::load(
        &verify_args.manifest_path,
        &verify_args.include_paths,
        &verify_args.package_roots,
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
    let media_root = verify_args
        .media_root
        .as_ref()
        .map(|path| root_relative_path(&root, path));
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
        verify_translation_coverage_policy(&plan, policy)?;
        let deck = plan.compose()?;
        deck.validate().map_err(|error| error.to_string())?;
        if let Some(media_root) = &media_root {
            validate_media_assets(&deck, media_root)?;
        } else {
            validate_media_references(&deck)?;
        }
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

    let suffix = if target_names.len() == 1 { "" } else { "s" };
    let mut details = vec![("manifest", verify_args.manifest_path.display().to_string())];
    if let Some(media_root) = &media_root {
        details.push(("media root", media_root.display().to_string()));
    }
    output::print_success(
        format!("verified {} target{suffix}", target_names.len()),
        &details,
    );
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
) -> Result<(), String> {
    let mut current = plan.base.clone();
    for (planned, overlay) in &plan.overlays {
        if let Some(report) = current.translation_coverage(overlay) {
            if policy == manifest::TranslationCoveragePolicy::Strict {
                let missing = report
                    .entries
                    .iter()
                    .filter(|entry| {
                        entry.category == TranslationCoverageCategory::UntranslatedFallback
                    })
                    .take(10)
                    .map(|entry| format!("{} source {:?}", entry.path, entry.source))
                    .collect::<Vec<_>>();
                if !missing.is_empty() {
                    return Err(format!(
                        "translation coverage strict policy failed for target {} overlay {} ({}): {} untranslated fallback(s): {}",
                        plan.target,
                        planned.id,
                        planned.display_file,
                        report
                            .entries
                            .iter()
                            .filter(|entry| entry.category
                                == TranslationCoverageCategory::UntranslatedFallback)
                            .count(),
                        missing.join(", ")
                    ));
                }
            }

            if let Some(message) = stale_translation_warning_message(
                &plan.target,
                &planned.id,
                &planned.display_file,
                &report.entries,
            ) {
                if policy == manifest::TranslationCoveragePolicy::Strict {
                    return Err(format!("translation stale strict policy failed: {message}"));
                }
                eprintln!("warning: {message}");
            }
        }
        current = current.compose(std::slice::from_ref(overlay)).map_err(|error| {
            format!(
                "failed to compose overlay {} for target {} while checking translation coverage: {error}",
                planned.id, plan.target
            )
        })?;
    }
    Ok(())
}

pub(crate) fn emit_stale_translation_warnings(plan: &TargetPlan) -> Result<(), String> {
    for message in stale_translation_warning_messages(plan)? {
        eprintln!("warning: {message}");
    }
    Ok(())
}

pub(crate) fn emit_stale_translation_warnings_for_overlays(
    target: &str,
    base: &CanonicalDeck,
    overlays: &[(String, Overlay)],
) -> Result<(), String> {
    for message in stale_translation_warning_messages_for_overlays(target, base, overlays)? {
        eprintln!("warning: {message}");
    }
    Ok(())
}

fn stale_translation_warning_messages(plan: &TargetPlan) -> Result<Vec<String>, String> {
    let mut messages = Vec::new();
    let mut current = plan.base.clone();
    for (planned, overlay) in &plan.overlays {
        if let Some(report) = current.translation_coverage(overlay)
            && let Some(message) = stale_translation_warning_message(
                &plan.target,
                &planned.id,
                &planned.display_file,
                &report.entries,
            )
        {
            messages.push(message);
        }
        current = current.compose(std::slice::from_ref(overlay)).map_err(|error| {
            format!(
                "failed to compose overlay {} for target {} while checking stale translations: {error}",
                planned.id, plan.target
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
        if let Some(report) = current.translation_coverage(overlay)
            && let Some(message) = stale_translation_warning_message(
                target,
                overlay.id.as_str(),
                display_file,
                &report.entries,
            )
        {
            messages.push(message);
        }
        current = current
            .compose(std::slice::from_ref(overlay))
            .map_err(|error| {
                format!(
                    "failed to compose overlay {} while checking stale translations: {error}",
                    overlay.id
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
