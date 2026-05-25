use std::env;
use std::io::{self, IsTerminal};

use brain_brew_core::{CanonicalDeck, SemanticChangeKind, SemanticDiff};
use brain_brew_formats::manifest;
use serde_json::json;

pub(crate) fn print_json_diff(diff: &SemanticDiff) {
    let changes = diff
        .changes
        .iter()
        .map(|change| {
            json!({
                "kind": semantic_kind_name(change.kind),
                "path": change.path,
                "before": change.before,
                "after": change.after,
            })
        })
        .collect::<Vec<_>>();
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({"changes": changes})).unwrap()
    );
}

pub(crate) fn print_human_diff(diff: &SemanticDiff) {
    if diff.is_empty() {
        println!("{} no semantic changes", success_marker());
        return;
    }

    let suffix = if diff.changes.len() == 1 { "" } else { "s" };
    println!("{} semantic change{suffix}", diff.changes.len());

    for change in &diff.changes {
        println!();
        println!("{} {}", change_marker(change.kind), change.path);
        print_change_values(change);
    }
}

pub(crate) fn print_success(message: impl AsRef<str>, details: &[(&str, String)]) {
    println!("{} {}", success_marker(), message.as_ref());
    for (label, value) in details {
        println!("  {}: {}", subtle(label), value);
    }
}

pub(crate) fn print_error(message: &str) {
    eprintln!("{}", error_text(message));
}

pub(crate) fn deck_stats(deck: &CanonicalDeck) -> Vec<(&'static str, String)> {
    vec![
        ("deck", deck.name.clone()),
        ("notes", deck.notes.len().to_string()),
        ("note types", deck.note_types.len().to_string()),
        ("card templates", card_template_count(deck).to_string()),
        ("media references", deck.media.len().to_string()),
    ]
}

pub(crate) fn semantic_kind_name(kind: SemanticChangeKind) -> &'static str {
    match kind {
        SemanticChangeKind::Added => "added",
        SemanticChangeKind::Removed => "removed",
        SemanticChangeKind::Modified => "modified",
        SemanticChangeKind::Tombstoned => "tombstoned",
    }
}

pub(crate) fn package_json(package: &manifest::PackageMetadata) -> serde_json::Value {
    json!({
        "id": package.id,
        "version": package.version,
        "compatible_base_versions": package.compatible_base_versions,
        "depends_on": package.depends_on,
    })
}

pub(crate) fn one_line(value: &str) -> String {
    value.replace('\n', "\\n")
}

fn print_change_values(change: &brain_brew_core::SemanticChange) {
    match (&change.before, &change.after) {
        (Some(before), Some(after)) => {
            print_value_lines('-', before);
            print_value_lines('+', after);
        }
        (Some(before), None) => print_value_lines('-', before),
        (None, Some(after)) => print_value_lines('+', after),
        (None, None) => println!("  {} entity", semantic_kind_name(change.kind)),
    }
}

fn print_value_lines(prefix: char, value: &str) {
    let marker = match prefix {
        '-' => removed_marker(),
        '+' => added_marker(),
        _ => prefix.to_string(),
    };
    if value.contains('\n') {
        println!("  {marker} |");
        for line in value.lines() {
            println!("    {line}");
        }
    } else if value.is_empty() {
        println!("  {marker} \"\"");
    } else {
        println!("  {marker} {value}");
    }
}

fn card_template_count(deck: &CanonicalDeck) -> usize {
    deck.note_types
        .values()
        .map(|note_type| note_type.card_templates.len())
        .sum()
}

fn success_marker() -> String {
    color_stdout("✓", "32")
}

fn change_marker(kind: SemanticChangeKind) -> String {
    match kind {
        SemanticChangeKind::Added => added_marker(),
        SemanticChangeKind::Removed => removed_marker(),
        SemanticChangeKind::Modified => color_stdout("~", "33"),
        SemanticChangeKind::Tombstoned => color_stdout("×", "31"),
    }
}

fn added_marker() -> String {
    color_stdout("+", "32")
}

fn removed_marker() -> String {
    color_stdout("-", "31")
}

fn subtle(text: &str) -> String {
    color_stdout(text, "2")
}

fn error_text(text: &str) -> String {
    color_stderr(text, "31")
}

fn color_stdout(text: &str, code: &str) -> String {
    if color_enabled(io::stdout().is_terminal()) {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_owned()
    }
}

fn color_stderr(text: &str, code: &str) -> String {
    if color_enabled(io::stderr().is_terminal()) {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_owned()
    }
}

fn color_enabled(is_terminal: bool) -> bool {
    match env::var("BRAINBREW_COLOR") {
        Ok(value) if value == "always" => true,
        Ok(value) if value == "never" => false,
        _ => env::var_os("NO_COLOR").is_none() && is_terminal,
    }
}
