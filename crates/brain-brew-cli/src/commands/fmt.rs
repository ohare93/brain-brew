use std::fs;
use std::path::Path;

use crate::help;
use crate::io::format_source_at;
use crate::output;

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        print!("{}", help::command("fmt").expect("fmt help exists"));
        return Ok(());
    }
    if args.len() != 1 {
        return Err(help::usage_error("fmt", "usage: brainbrew fmt <deck.yaml>"));
    }
    let path = Path::new(&args[0]);
    let input = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let formatted =
        format_source_at(path, &input).map_err(|error| format!("{}: {error}", path.display()))?;
    fs::write(path, formatted).map_err(|error| format!("{}: {error}", path.display()))?;
    output::print_success("formatted source", &[("path", path.display().to_string())]);
    Ok(())
}
