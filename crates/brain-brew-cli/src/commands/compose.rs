use std::path::Path;

use brain_brew_formats::canonical_yaml;

use crate::args::{parse_manifest_target_output_args, parse_overlay_and_optional_out};
use crate::commands::verify;
use crate::help;
use crate::io::read_deck_and_overlays;
use crate::output;
use crate::planner::plan_manifest_target;
use crate::workspace_mutation::write_output_file;

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        print!("{}", help::command("compose").expect("compose help exists"));
        return Ok(());
    }

    if args
        .iter()
        .any(|arg| arg == "--manifest" || arg == "--target")
    {
        let manifest_args = parse_manifest_target_output_args(args)?;
        let plan = plan_manifest_target(
            &manifest_args.manifest_path,
            &manifest_args.target,
            &manifest_args.include_paths,
            &manifest_args.package_roots,
        )?;
        verify::emit_stale_translation_warnings(&plan)?;
        let deck = plan.compose()?;
        let yaml = canonical_yaml::to_string(&deck).map_err(|error| error.to_string())?;
        if let Some(path) = manifest_args.out_path {
            write_canonical_output(&path, yaml.into_bytes(), manifest_args.force)?;
            let mut details = vec![
                (
                    "manifest",
                    manifest_args.manifest_path.display().to_string(),
                ),
                ("target", manifest_args.target.clone()),
                ("output", path.display().to_string()),
            ];
            details.extend(output::deck_stats(&deck));
            output::print_success(
                format!("composed target {}", manifest_args.target),
                &details,
            );
        } else {
            print!("{yaml}");
        }
        return Ok(());
    }

    if args.is_empty() {
        return Err(help::usage_error(
            "compose",
            "usage: brainbrew compose <deck.yaml> [--overlay overlay.yaml ...] [--out resolved.yaml]",
        ));
    }
    if args[0].starts_with('-') {
        return Err(format!("unexpected argument {:?}", args[0]));
    }
    let deck_path = Path::new(&args[0]);
    let (overlay_paths, out_path, force) = parse_overlay_and_optional_out(&args[1..])?;
    let (base, overlays) = read_deck_and_overlays(deck_path, &overlay_paths)?;
    verify::emit_stale_translation_warnings_for_overlays("ad-hoc", &base, &overlays)?;
    let deck = base
        .compose(
            &overlays
                .iter()
                .map(|(_, overlay)| overlay.clone())
                .collect::<Vec<_>>(),
        )
        .map_err(|error| error.to_string())?;
    let yaml = canonical_yaml::to_string(&deck).map_err(|error| error.to_string())?;
    if let Some(path) = out_path {
        write_canonical_output(&path, yaml.into_bytes(), force)?;
        let mut details = vec![
            ("source", deck_path.display().to_string()),
            ("overlays", overlay_paths.len().to_string()),
            ("output", path.display().to_string()),
        ];
        details.extend(output::deck_stats(&deck));
        output::print_success("composed deck", &details);
    } else {
        print!("{yaml}");
    }
    Ok(())
}

fn write_canonical_output(path: &Path, bytes: Vec<u8>, force: bool) -> Result<(), String> {
    write_output_file(path, bytes, force, |replacement| {
        let text = std::str::from_utf8(replacement)
            .map_err(|error| format!("generated compose output is not UTF-8: {error}"))?;
        canonical_yaml::from_str(text)
            .map(|_| ())
            .map_err(|error| format!("{}: {error}", path.display()))
    })
}
