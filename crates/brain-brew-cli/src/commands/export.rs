use std::path::Path;

use brain_brew_core::CanonicalDeck;
use brain_brew_formats::crowdanki;

use crate::args::{parse_manifest_target_export_args, parse_overlay_out_media};
use crate::commands::verify;
use crate::help;
use crate::io::{configured_crowdanki_out, manifest_root, read_deck_and_overlays};
use crate::media_assets::{collect_media_assets, validate_media_semantics, verify_owned_media};
use crate::media_ownership::MediaRootSelections;
use crate::media_verification::MediaVerificationMode;
use crate::output;
use crate::output_transaction::{OutputArtifact, publish_output_tree};
use crate::path_authorization::PathAuthorizer;
use crate::planner::{ManifestRegistry, TargetPlan};

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    if matches!(args, [flag] if flag == "--help" || flag == "-h")
        || matches!(args, [format, flag] if format == "crowdanki" && (flag == "--help" || flag == "-h"))
    {
        print!("{}", help::command("export").expect("export help exists"));
        return Ok(());
    }
    if args.first().map(String::as_str) != Some("crowdanki") {
        return Err(help::usage_error(
            "export",
            "usage: brainbrew export crowdanki <deck.yaml> [--overlay overlay.yaml ...] --out build/deck-folder",
        ));
    }
    if args
        .iter()
        .any(|arg| arg == "--manifest" || arg == "--target")
    {
        let manifest_args = parse_manifest_target_export_args(&args[1..])?;
        let registry = ManifestRegistry::load(
            &manifest_args.manifest_path,
            &manifest_args.include_paths,
            &manifest_args.package_roots,
        )?;
        let plan = registry.plan(&manifest_args.target)?;
        // Export destinations belong to the caller-selected workspace, never a
        // dependency package/cache root selected while resolving the target.
        let root = manifest_root(&manifest_args.manifest_path);
        let out_dir = if let Some(out_path) = manifest_args.out_path.clone() {
            out_path
        } else if let Some(path) = configured_crowdanki_out(&plan.target_manifest, &plan.target) {
            PathAuthorizer::new("workspace", &root)?
                .authorize_create(
                    &manifest_args.manifest_path,
                    format!("targets.{}.exports.crowdanki.out", plan.target),
                    &path,
                )
                .map_err(|error| error.to_string())?
                .into_path_buf()
        } else {
            PathAuthorizer::new("workspace", &root)?
                .authorize_create(
                    &manifest_args.manifest_path,
                    "generated CrowdAnki output",
                    &format!("build/crowdanki/{}", plan.target),
                )
                .map_err(|error| error.to_string())?
                .into_path_buf()
        };
        let warnings = verify::stale_translation_warnings(&plan)?;
        let deck = plan.compose()?;
        if manifest_args.media_mode == MediaVerificationMode::ReferenceOnly
            && !manifest_args.media_roots.is_empty()
        {
            return Err("--media-mode reference-only cannot be combined with --media-root because reference-only mode intentionally skips all media root and byte validation".to_owned());
        }
        let media_roots = MediaRootSelections::parse(&registry, &manifest_args.media_roots, &root)?;
        return write_owned_crowdanki_export(
            &plan,
            &deck,
            &out_dir,
            &media_roots,
            ExportRunOptions {
                media_mode: manifest_args.media_mode,
                json_output: manifest_args.json_output,
                warnings,
                force: manifest_args.force,
            },
        );
    }

    if args.len() < 4 {
        return Err(help::usage_error(
            "export",
            "usage: brainbrew export crowdanki <deck.yaml> [--overlay overlay.yaml ...] --out build/deck-folder",
        ));
    }
    let deck_path = Path::new(&args[1]);
    let export_args = parse_overlay_out_media(&args[2..])?;
    let Some(out_dir) = export_args.out_path else {
        return Err("missing --out".to_owned());
    };

    let (base, overlays) = read_deck_and_overlays(deck_path, &export_args.overlay_paths)?;
    let warnings = verify::stale_translation_warnings_for_overlays("ad-hoc", &base, &overlays)?;
    let deck = base
        .compose(
            &overlays
                .iter()
                .map(|(_, overlay)| overlay.clone())
                .collect::<Vec<_>>(),
        )
        .map_err(|error| error.to_string())?;
    write_crowdanki_export(
        &deck,
        &out_dir,
        export_args.media_root.as_deref(),
        export_args.media_mode,
        export_args.json_output,
        warnings,
        export_args.force,
    )
}

struct ExportRunOptions {
    media_mode: MediaVerificationMode,
    json_output: bool,
    warnings: Vec<String>,
    force: bool,
}

fn write_owned_crowdanki_export(
    plan: &TargetPlan,
    deck: &CanonicalDeck,
    out_dir: &Path,
    media_roots: &MediaRootSelections,
    mut options: ExportRunOptions,
) -> Result<(), String> {
    let verification = verify_owned_media(plan, deck, media_roots, options.media_mode, true)?;
    options.warnings.extend(verification.warnings);
    let export = crowdanki::export_deck(deck).map_err(|error| error.to_string())?;
    let mut artifacts = vec![OutputArtifact::new(
        "deck.json",
        export.deck_json.as_bytes().to_vec(),
    )];
    artifacts.extend(
        verification
            .assets
            .into_iter()
            .map(|(path, bytes)| OutputArtifact::new(path, bytes)),
    );
    publish_output_tree(out_dir, artifacts, options.force)?;
    print_export_result(
        deck,
        out_dir,
        options.media_mode,
        options.warnings,
        options.json_output,
        &export.omitted_tombstones,
    );
    Ok(())
}

fn write_crowdanki_export(
    deck: &CanonicalDeck,
    out_dir: &Path,
    media_root: Option<&Path>,
    media_mode: MediaVerificationMode,
    json_output: bool,
    mut warnings: Vec<String>,
    force: bool,
) -> Result<(), String> {
    if media_mode == MediaVerificationMode::ReferenceOnly && media_root.is_some() {
        return Err("--media-mode reference-only cannot be combined with --media-root because reference-only mode intentionally skips all media root and byte validation".to_owned());
    }
    warnings.extend(validate_media_semantics(deck, media_mode)?);
    if let Some(warning) = media_mode.development_warning(deck.media.len()) {
        warnings.push(warning);
    }
    let export = crowdanki::export_deck(deck).map_err(|error| error.to_string())?;
    let mut artifacts = vec![OutputArtifact::new(
        "deck.json",
        export.deck_json.as_bytes().to_vec(),
    )];
    match (media_mode, media_root) {
        (MediaVerificationMode::Strict, Some(media_root)) => artifacts.extend(
            collect_media_assets(deck, media_root)?
                .into_iter()
                .map(|(path, bytes)| OutputArtifact::new(path, bytes)),
        ),
        (MediaVerificationMode::Strict, None) if !deck.media.is_empty() => {
            return Err("strict media verification requires --media-root for a deck with declared media; use --media-mode reference-only only for an explicitly non-release development export".to_owned());
        }
        _ => {}
    }
    publish_output_tree(out_dir, artifacts, force)?;
    print_export_result(
        deck,
        out_dir,
        media_mode,
        warnings,
        json_output,
        &export.omitted_tombstones,
    );
    Ok(())
}

fn print_export_result(
    deck: &CanonicalDeck,
    out_dir: &Path,
    media_mode: MediaVerificationMode,
    warnings: Vec<String>,
    json_output: bool,
    omitted_tombstones: &[brain_brew_core::StableId],
) {
    let release_ready = media_mode.release_ready(deck.media.len());
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "status": "exported",
                "format": "crowdanki",
                "output": out_dir.join("deck.json").display().to_string(),
                "media": {
                    "mode": media_mode.name(),
                    "declarations": deck.media.len(),
                    "release_ready": release_ready,
                    "assets_copied": if media_mode == MediaVerificationMode::Strict { deck.media.len() } else { 0 },
                },
                "warnings": warnings,
            }))
            .expect("export status JSON serializes")
        );
        return;
    }
    for warning in &warnings {
        eprintln!("warning: {warning}");
    }
    let mut details = vec![("output", out_dir.join("deck.json").display().to_string())];
    details.extend(output::deck_stats(deck));
    details.push((
        "media verification",
        if release_ready {
            media_mode.name().to_owned()
        } else {
            "reference_only (NOT RELEASE-READY)".to_owned()
        },
    ));
    if !omitted_tombstones.is_empty() {
        details.push((
            "omitted tombstones",
            omitted_tombstones
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", "),
        ));
    }
    output::print_success("exported CrowdAnki deck", &details);
}
