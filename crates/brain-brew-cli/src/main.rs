//! Command-line entry point for Brain Brew.

use std::env;
use std::process;

mod args;
mod commands;
mod help;
mod io;
mod media_assets;
mod output;
mod overlay_draft;
mod package_resolver;

fn main() {
    if let Err(error) = run() {
        output::print_error(&error);
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let Some(command) = args.first().map(String::as_str) else {
        print_usage();
        return Ok(());
    };

    let command = canonical_command(command);

    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
        && let Some(command_help) = help::command(command)
    {
        print!("{command_help}");
        return Ok(());
    }

    match command {
        "fmt" => commands::fmt::run(&args[1..]),
        "validate" => commands::validate::run(&args[1..]),
        "compose" => commands::compose::run(&args[1..]),
        "export" => commands::export::run(&args[1..]),
        "import" => commands::import::run(&args[1..]),
        "lock" => commands::lock::run(&args[1..]),
        "media" => commands::media::run(&args[1..]),
        "targets" => commands::targets::run(&args[1..]),
        "translations" => commands::translations::run(&args[1..]),
        "verify" => commands::verify::run(&args[1..]),
        "workbench" => commands::workbench::run(&args[1..]),
        "explain" => commands::explain::run(&args[1..]),
        "diff" => commands::diff::run(&args[1..]),
        "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        "--version" | "-V" => {
            println!("brainbrew {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        other => Err(unknown_command_error(other)),
    }
}

fn canonical_command(command: &str) -> &str {
    match command {
        "translate" | "translation" => "translations",
        other => other,
    }
}

fn unknown_command_error(command: &str) -> String {
    if command.starts_with("translat") || levenshtein_one_or_two(command, "translations") {
        format!("unknown command {command:?}\n\nDid you mean:\n  brainbrew translations")
    } else {
        format!("unknown command {command:?}")
    }
}

fn levenshtein_one_or_two(left: &str, right: &str) -> bool {
    let mut previous = (0..=right.len()).collect::<Vec<_>>();
    let mut current = vec![0; right.len() + 1];
    for (left_index, left_char) in left.chars().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_char) in right.chars().enumerate() {
            let substitution = usize::from(left_char != right_char);
            current[right_index + 1] = (previous[right_index + 1] + 1)
                .min(current[right_index] + 1)
                .min(previous[right_index] + substitution);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()] <= 2
}

fn print_usage() {
    print!("{}", help::general());
}
