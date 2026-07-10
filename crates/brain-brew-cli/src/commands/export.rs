use std::fs;
use std::path::Path;

use brain_brew_core::CanonicalDeck;
use brain_brew_formats::crowdanki;

use crate::args::{parse_manifest_target_args, parse_overlay_out_media};
use crate::commands::verify;
use crate::help;
use crate::io::{
    configured_crowdanki_out, manifest_root, read_deck_and_overlays, root_relative_path,
};
use crate::media_assets::{copy_media_assets, validate_media_assets};
use crate::output;
use crate::path_authorization::PathAuthorizer;
use crate::planner::plan_manifest_target;

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
        let manifest_args = parse_manifest_target_args(&args[1..])?;
        let plan = plan_manifest_target(
            &manifest_args.manifest_path,
            &manifest_args.target,
            &manifest_args.include_paths,
            &manifest_args.package_roots,
        )?;
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
        verify::emit_stale_translation_warnings(&plan)?;
        let deck = plan.compose()?;
        let media_root = manifest_args
            .media_root
            .as_ref()
            .map(|path| root_relative_path(&root, path));
        return write_crowdanki_export(&deck, &out_dir, media_root.as_deref());
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
    verify::emit_stale_translation_warnings_for_overlays("ad-hoc", &base, &overlays)?;
    let deck = base
        .compose(
            &overlays
                .iter()
                .map(|(_, overlay)| overlay.clone())
                .collect::<Vec<_>>(),
        )
        .map_err(|error| error.to_string())?;
    write_crowdanki_export(&deck, &out_dir, export_args.media_root.as_deref())
}

fn write_crowdanki_export(
    deck: &CanonicalDeck,
    out_dir: &Path,
    media_root: Option<&Path>,
) -> Result<(), String> {
    if let Some(media_root) = media_root {
        validate_media_assets(deck, media_root)?;
    }
    let export = crowdanki::export_deck(deck).map_err(|error| error.to_string())?;
    fs::create_dir_all(out_dir).map_err(|error| format!("{}: {error}", out_dir.display()))?;
    fs::write(out_dir.join("deck.json"), export.deck_json)
        .map_err(|error| format!("{}: {error}", out_dir.display()))?;

    if let Some(media_root) = media_root {
        copy_media_assets(deck, media_root, out_dir)?;
    }

    let mut details = vec![("output", out_dir.join("deck.json").display().to_string())];
    details.extend(output::deck_stats(deck));
    if let Some(media_root) = media_root {
        details.push(("media root", media_root.display().to_string()));
    }
    if !export.omitted_tombstones.is_empty() {
        details.push((
            "omitted tombstones",
            export
                .omitted_tombstones
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", "),
        ));
    }
    output::print_success("exported CrowdAnki deck", &details);
    Ok(())
}
