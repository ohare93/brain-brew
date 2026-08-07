use std::cell::RefCell;
use std::env;
use std::io::{self, IsTerminal};

use brain_brew_core::{
    CanonicalDeck, ComposePrecondition, DomainDiagnostic, FieldValue, SemanticChangeKind,
    SemanticDiff, TombstoneAddress,
};
use brain_brew_formats::manifest;
use serde_json::{Value, json};

pub(crate) const JSON_ERROR_ALREADY_PRINTED: &str = "__brainbrew_json_error_already_printed";
pub(crate) const DIFFERENCES_FOUND: &str = "__brainbrew_differences_found";
pub(crate) const DIAGNOSTIC_SCHEMA_VERSION: u32 = 1;
pub(crate) const TYPED_ERROR_PENDING: &str = "__brainbrew_typed_error_pending";

#[derive(Clone, Debug)]
pub(crate) struct PendingDiagnosticError {
    command: String,
    context: Value,
    message: String,
    diagnostics: Vec<DomainDiagnostic>,
}

thread_local! {
    static PENDING_DIAGNOSTIC_ERROR: RefCell<Option<PendingDiagnosticError>> = const { RefCell::new(None) };
}

pub(crate) fn domain_error(
    command: &str,
    context: Value,
    message: impl Into<String>,
    diagnostics: Vec<DomainDiagnostic>,
) -> String {
    PENDING_DIAGNOSTIC_ERROR.with(|pending| {
        *pending.borrow_mut() = Some(PendingDiagnosticError {
            command: command.to_owned(),
            context,
            message: message.into(),
            diagnostics,
        });
    });
    TYPED_ERROR_PENDING.to_owned()
}

pub(crate) fn compose_error(
    command: &str,
    context: Value,
    report: &brain_brew_core::ComposeReport,
) -> String {
    domain_error(
        command,
        context,
        "composition failed",
        report
            .errors
            .iter()
            .map(|error| error.diagnostic())
            .collect(),
    )
}

pub(crate) fn validation_error(
    command: &str,
    context: Value,
    report: &brain_brew_core::ValidationReport,
) -> String {
    domain_error(
        command,
        context,
        "validation failed",
        report
            .errors
            .iter()
            .map(|error| error.diagnostic())
            .collect(),
    )
}

pub(crate) fn print_pending_error(json_output: bool) -> bool {
    let pending = PENDING_DIAGNOSTIC_ERROR.with(|pending| pending.borrow_mut().take());
    let Some(pending) = pending else {
        return false;
    };
    if json_output {
        print_json_diagnostic_error(
            &pending.command,
            pending.context,
            &pending.message,
            &pending.diagnostics,
        );
    } else {
        eprintln!("{}", render_diagnostics(&pending.diagnostics));
    }
    true
}

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
        let (code, category) = ("adapter_error", "adapter");
        let (source, path) = (None, None);
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

    pub(crate) fn human_message(&self) -> &str {
        &self.message
    }

    fn json(&self) -> Value {
        let mut value = json!({
            "schema_version": self.version,
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

pub(crate) fn diagnostic_json(diagnostic: &DomainDiagnostic) -> Value {
    let graph = diagnostic.field_graph_error.as_ref().map(|error| {
        json!({
            "kind": format!("{:?}", error.kind),
            "note_id": error.note_id.as_str(),
            "field_id": error.field_id.as_str(),
            "consuming_path": error.consuming_path,
            "dependency": error.dependency,
            "representation": error.representation.map(|kind| kind.as_str()),
            "cycle": error.cycle,
        })
    });
    json!({
        "code": diagnostic.code,
        "category": diagnostic.category.as_str(),
        "path": diagnostic.path.as_ref().map(ToString::to_string).unwrap_or_else(|| diagnostic.address.clone()),
        "address": diagnostic.address,
        "overlay": diagnostic.overlay_id.as_ref().map(|id| id.as_str()),
        "source": diagnostic.source_id.as_ref().map(|id| id.as_str()),
        "intent": diagnostic.intent.map(|intent| intent.as_str()),
        "entity_kind": diagnostic.entity_kind.map(|kind| kind.as_str()),
        "expected": diagnostic.expected.as_ref().map(precondition_json),
        "actual": diagnostic.actual.as_ref().map(precondition_json),
        "conflict": (diagnostic.first_conflict_participant.is_some()
            || diagnostic.current_conflict_participant.is_some()).then(|| json!({
                "first": diagnostic.first_conflict_participant.as_ref().map(|id| id.as_str()),
                "current": diagnostic.current_conflict_participant.as_ref().map(|id| id.as_str()),
            })),
        "original_removal": diagnostic.original_removal.as_ref().map(|record| record.address.to_string()),
        "field_graph": graph,
        "details": {},
        "children": diagnostic.children.iter().map(diagnostic_json).collect::<Vec<_>>(),
        "message": diagnostic.message,
    })
}

fn precondition_json(value: &ComposePrecondition) -> Value {
    match value {
        ComposePrecondition::Fingerprint(value) => {
            json!({"kind": "fingerprint", "value": value.to_string()})
        }
        ComposePrecondition::Value(value) => json!({"kind": "value", "value": value}),
        ComposePrecondition::FieldValue(value) => match value {
            FieldValue::Scalar(value) => {
                json!({"kind": "field_value", "representation": "scalar", "value": value})
            }
            FieldValue::Images(images) => json!({
                "kind": "field_value",
                "representation": "images",
                "media_ids": images.iter().map(|image| image.media_id.as_str()).collect::<Vec<_>>(),
            }),
            FieldValue::Message(_) => json!({"kind": "field_value", "representation": "message"}),
            FieldValue::MessageItems(_) => {
                json!({"kind": "field_value", "representation": "message_items"})
            }
        },
        ComposePrecondition::Missing => json!({"kind": "missing"}),
    }
}

pub(crate) fn render_diagnostics(diagnostics: &[DomainDiagnostic]) -> String {
    let mut lines = Vec::new();
    for diagnostic in diagnostics {
        let path = diagnostic
            .path
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| diagnostic.address.clone());
        lines.push(format!("{path}: {}", diagnostic.message));
        for child in &diagnostic.children {
            let child_path = child
                .path
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| child.address.clone());
            lines.push(format!("  {child_path}: {}", child.message));
        }
    }
    lines.join("\n")
}

pub(crate) fn print_json_diagnostic_error(
    command: &str,
    context: Value,
    message: &str,
    diagnostics: &[DomainDiagnostic],
) {
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "error": {
                "schema_version": DIAGNOSTIC_SCHEMA_VERSION,
                "version": DIAGNOSTIC_SCHEMA_VERSION,
                "command": command,
                "context": context,
                "code": diagnostics.first().map(|item| item.code).unwrap_or("command_failed"),
                "category": diagnostics.first().map(|item| item.category.as_str()).unwrap_or("command"),
                "path": diagnostics.first().map(|item| item.path.as_ref().map(ToString::to_string).unwrap_or_else(|| item.address.clone())),
                "source": diagnostics.first().and_then(|item| item.source_id.as_ref()).map(|id| id.as_str()),
                "message": message,
                "diagnostics": diagnostics.iter().map(diagnostic_json).collect::<Vec<_>>(),
                "details": {},
            }
        }))
        .unwrap()
    );
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

pub(crate) fn print_json_error(command: &str, message: &str) {
    let error = CliError::from_message(message);
    let mut value = error.json();
    if let Some(object) = value.as_object_mut() {
        object.insert("command".to_owned(), json!(command));
        object.insert("context".to_owned(), Value::Null);
        object.insert("diagnostics".to_owned(), json!([]));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({"error": value})).unwrap()
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
