use std::path::Path;

use brain_brew_formats::canonical_yaml;

use crate::args::{parse_diff_overlay_args, split_json_flag};
use crate::io::read_deck;
use crate::output::{print_human_diff, print_json_diff};
use crate::overlay_draft::draft_overlay_from_diff;

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--as-overlay") {
        let overlay_args = parse_diff_overlay_args(args)?;
        let left = read_deck(&overlay_args.left_path)?;
        let right = read_deck(&overlay_args.right_path)?;
        let overlay = draft_overlay_from_diff(&left, &right, overlay_args.id, overlay_args.kind)?;
        print!(
            "{}",
            canonical_yaml::overlay_to_string(&overlay).map_err(|error| error.to_string())?
        );
        return Ok(());
    }

    let (json_output, paths) = split_json_flag(args);
    if let Some(arg) = paths.iter().find(|arg| arg.starts_with('-')) {
        return Err(format!("unexpected argument {arg:?}"));
    }
    if paths.len() != 2 {
        return Err("usage: brainbrew diff <left.yaml> <right.yaml> [--json]".to_owned());
    }
    let left = read_deck(Path::new(&paths[0]))?;
    let right = read_deck(Path::new(&paths[1]))?;
    let diff = left.semantic_diff(&right);
    if json_output {
        print_json_diff(&diff);
    } else {
        print_human_diff(&diff);
    }
    Ok(())
}
