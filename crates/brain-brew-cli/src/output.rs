use std::env;
use std::io::{self, IsTerminal};

use brain_brew_core::{CanonicalDeck, SemanticChangeKind, SemanticDiff, TombstoneAddress};
use brain_brew_formats::manifest;
use serde_json::{Value, json};

pub(crate) const JSON_ERROR_ALREADY_PRINTED: &str = "__brainbrew_json_error_already_printed";
pub(crate) const DIFFERENCES_FOUND: &str = "__brainbrew_differences_found";

#[derive(Clone, Debug)]
pub(crate) struct CliError {
    pub(crate) version: u32,
    pub(crate) code: &'static str,
    pub(crate) category: &'static str,
    pub(crate) message: String,
    pub(crate) source: Option<String>,
    pub(crate) path: Option<String>,
    pub(crate) details: serde_json::Map<String, Value>,
}

impl CliError {
    pub(crate) fn from_message(message: impl Into<String>) -> Self {
        let message = message.into();
        let lower = message.to_ascii_lowercase();
        let (code, category) = if lower.contains("usage:")
            || lower.contains("unexpected")
            || lower.contains("requires a ")
            || lower.contains("missing --")
            || lower.contains("choose --")
            || lower.contains("duplicate argument")
        {
            ("invalid_arguments", "usage")
        } else if lower.contains("transaction") || lower.contains("recover") {
            ("transaction_failed", "filesystem")
        } else if lower.contains("yaml")
            || lower.contains("json")
            || lower.contains("schema")
            || lower.contains("parse")
            || lower.contains("invalid overlay")
        {
            ("source_invalid", "format")
        } else if lower.contains("no such file")
            || lower.contains("permission denied")
            || lower.contains("refusing to")
        {
            ("filesystem_error", "filesystem")
        } else if lower.contains("validation") || lower.contains("invalid deck") {
            ("validation_failed", "validation")
        } else {
            ("command_failed", "command")
        };
        let (source, path) = diagnostic_location(&message);
        Self {
            version: 1,
            code,
            category,
            message,
            source,
            path,
            details: serde_json::Map::new(),
        }
    }

    fn from_value(value: Value) -> Self {
        let mut object = value
            .as_object()
            .cloned()
            .unwrap_or_else(|| serde_json::Map::from_iter([("message".to_owned(), value.clone())]));
        let message = object
            .remove("message")
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_else(|| "command failed".to_owned());
        let mut error = Self::from_message(message);
        if let Some(errors) = object.get("errors").and_then(Value::as_array) {
            error.code = "validation_failed";
            error.category = "validation";
            error.path = errors
                .first()
                .and_then(|item| item.get("path"))
                .and_then(Value::as_str)
                .map(str::to_owned);
        }
        error.details = object;
        error
    }

    pub(crate) fn human_message(&self) -> &str {
        &self.message
    }

    fn json(&self) -> Value {
        let mut value = json!({
            "version": self.version,
            "code": self.code,
            "category": self.category,
            "message": self.message,
            "source": self.source,
            "path": self.path,
            "details": self.details,
        });
        // Preserve the pre-v1 route fields during the compatibility window while
        // making the versioned fields above authoritative.
        if let Some(object) = value.as_object_mut() {
            for (key, detail) in &self.details {
                object.entry(key.clone()).or_insert_with(|| detail.clone());
            }
        }
        value
    }
}

fn diagnostic_location(message: &str) -> (Option<String>, Option<String>) {
    let first_line = message.lines().next().unwrap_or(message);
    let source = first_line
        .split(':')
        .next()
        .and_then(|candidate| {
            let candidate = candidate.trim().split(" (").next().unwrap_or_default();
            (candidate.contains('/')
                || candidate.ends_with(".yaml")
                || candidate.ends_with(".json"))
            .then(|| candidate.to_owned())
        })
        .or_else(|| {
            message.split_whitespace().find_map(|word| {
                let candidate = word.trim_matches(|character: char| {
                    matches!(character, '`' | '"' | '\'' | '(' | ')' | ',' | '.' | ':')
                });
                (candidate.ends_with(".yaml") || candidate.ends_with(".json"))
                    .then(|| candidate.to_owned())
            })
        });
    let path = first_line
        .split("schema path ")
        .nth(1)
        .and_then(|suffix| suffix.split([':', ',']).next())
        .or_else(|| {
            first_line
                .split("YAML: ")
                .nth(1)
                .and_then(|suffix| suffix.split(':').next())
        })
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(str::to_owned);
    (source, path)
}

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
    let error = CliError::from_message(message);
    eprintln!("{}", error_text(error.human_message()));
}

pub(crate) fn print_json_error(message: &str) {
    print_json_cli_error(&CliError::from_message(message));
}

pub(crate) fn print_json_error_value(error: Value) {
    print_json_cli_error(&CliError::from_value(error));
}

fn print_json_cli_error(error: &CliError) {
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({"error": error.json()})).unwrap()
    );
}

pub(crate) fn deck_stats(deck: &CanonicalDeck) -> Vec<(&'static str, String)> {
    vec![
        ("deck", deck.name.clone()),
        (
            "notes",
            deck.notes
                .keys()
                .filter(|id| {
                    deck.tombstones
                        .blocking(&TombstoneAddress::Note {
                            note_id: (*id).clone(),
                        })
                        .is_none()
                })
                .count()
                .to_string(),
        ),
        (
            "note types",
            deck.note_types
                .keys()
                .filter(|id| {
                    deck.tombstones
                        .blocking(&TombstoneAddress::NoteType {
                            note_type_id: (*id).clone(),
                        })
                        .is_none()
                })
                .count()
                .to_string(),
        ),
        ("card templates", card_template_count(deck).to_string()),
        (
            "media references",
            deck.media
                .keys()
                .filter(|id| {
                    deck.tombstones
                        .blocking(&TombstoneAddress::MediaReference {
                            media_id: (*id).clone(),
                        })
                        .is_none()
                })
                .count()
                .to_string(),
        ),
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
        "base_package": package.base_package,
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
        .filter(|note_type| {
            deck.tombstones
                .blocking(&TombstoneAddress::NoteType {
                    note_type_id: note_type.id.clone(),
                })
                .is_none()
        })
        .map(|note_type| {
            note_type
                .card_templates
                .iter()
                .filter(|template| {
                    deck.tombstones
                        .blocking(&TombstoneAddress::CardTemplate {
                            note_type_id: note_type.id.clone(),
                            template_id: template.id.clone(),
                        })
                        .is_none()
                })
                .count()
        })
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
