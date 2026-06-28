use std::fs;
use std::path::Path;

use brain_brew_core::{CanonicalDeck, Overlay, TranslationCoverageCategory};
use brain_brew_formats::{crowdanki, manifest};

use crate::args::parse_verify_args;
use crate::help;
use crate::io::{
    ManifestTargetPlan, manifest_root, plan_manifest_target_with_packages, read_manifest,
    root_relative_path, verify_canonical_deck_format, verify_manifest_format,
    verify_overlay_format,
};
use crate::media_assets::validate_media_assets;
use crate::output;

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        print!("{}", help::command("verify").expect("verify help exists"));
        return Ok(());
    }
    let verify_args = parse_verify_args(args)?;
    verify_manifest_format(&verify_args.manifest_path)?;
    let manifest = read_manifest(&verify_args.manifest_path)?;
    let root = manifest_root(&verify_args.manifest_path);
    let target_names = if verify_args.all_targets {
        manifest.targets.keys().cloned().collect::<Vec<_>>()
    } else if let Some(target) = verify_args.target {
        vec![target]
    } else {
        return Err(help::usage_error(
            "verify",
            "usage: brainbrew verify [--manifest brainbrew.yaml] --all-targets",
        ));
    };

    verify_canonical_deck_format(&root.join(&manifest.base))?;
    let media_root = verify_args
        .media_root
        .as_ref()
        .map(|path| root_relative_path(&root, path));
    for target in &target_names {
        let plan = plan_manifest_target_with_packages(
            &verify_args.manifest_path,
            target,
            &verify_args.include_paths,
            &verify_args.package_roots,
        )?;
        for (overlay, _) in &plan.overlays {
            verify_overlay_format(&overlay.file)?;
        }
        let policy = verify_args.translation_coverage.unwrap_or_else(|| {
            manifest
                .targets
                .get(target)
                .map(|target| target.translation_coverage)
                .unwrap_or_default()
        });
        verify_translation_coverage_policy(&plan, policy)?;
        let deck = plan.compose()?;
        deck.validate().map_err(|error| error.to_string())?;
        if let Some(media_root) = &media_root {
            validate_media_assets(&deck, media_root)?;
        }
        verify_configured_exports(&root, &manifest, target, &deck)?;
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

fn verify_translation_coverage_policy(
    plan: &ManifestTargetPlan,
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

            let stale_translations = stale_translation_warning_details(&report.entries);
            if !stale_translations.is_empty() {
                let message = format!(
                    "stale translation warning for target {} overlay {} ({}): {} stale translation(s): {}",
                    plan.target,
                    planned.id,
                    planned.display_file,
                    stale_translations.len(),
                    stale_translations
                        .iter()
                        .take(10)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                );
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

pub(crate) fn emit_stale_translation_warnings(plan: &ManifestTargetPlan) -> Result<(), String> {
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

fn stale_translation_warning_messages(plan: &ManifestTargetPlan) -> Result<Vec<String>, String> {
    let mut messages = Vec::new();
    let mut current = plan.base.clone();
    for (planned, overlay) in &plan.overlays {
        if let Some(report) = current.translation_coverage(overlay) {
            let stale_translations = stale_translation_warning_details(&report.entries);
            if !stale_translations.is_empty() {
                messages.push(format!(
                    "stale translation warning for target {} overlay {} ({}): {} stale translation(s): {}",
                    plan.target,
                    planned.id,
                    planned.display_file,
                    stale_translations.len(),
                    stale_translations.iter().take(10).cloned().collect::<Vec<_>>().join(", ")
                ));
            }
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
        if let Some(report) = current.translation_coverage(overlay) {
            let stale_translations = stale_translation_warning_details(&report.entries);
            if !stale_translations.is_empty() {
                messages.push(format!(
                    "stale translation warning for target {target} overlay {} ({}): {} stale translation(s): {}",
                    overlay.id,
                    display_file,
                    stale_translations.len(),
                    stale_translations.iter().take(10).cloned().collect::<Vec<_>>().join(", ")
                ));
            }
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
    let mut golden_path = root.join(golden);
    if golden_path.is_dir() {
        golden_path = golden_path.join("deck.json");
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
