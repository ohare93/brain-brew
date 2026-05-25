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
        "targets" => commands::targets::run(&args[1..]),
        "verify" => commands::verify::run(&args[1..]),
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
        other => Err(format!("unknown command {other:?}")),
    }
}

fn print_usage() {
    print!("{}", help::general());
}
