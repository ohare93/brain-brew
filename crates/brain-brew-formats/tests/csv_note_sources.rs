use std::collections::BTreeMap;

use brain_brew_core::{FieldValue, StableId};
use brain_brew_formats::canonical_source_document::CanonicalSourceDocument;
use brain_brew_formats::csv_note_source::{
    CsvNoteSourceDescriptor, CsvNoteSourceMaterializer, CsvSourceFile, CsvSourceRequestKind,
};
use brain_brew_formats::source_document::{SourceFile, SourceProvenance};
use brain_brew_formats::source_includes;

fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

fn source(name: &str, text: impl Into<String>) -> SourceFile {
    SourceFile::new(SourceProvenance::new(name), text)
}

fn csv_source(name: &str, bytes: impl Into<Vec<u8>>) -> CsvSourceFile {
    CsvSourceFile::new(SourceProvenance::new(name), bytes)
}

fn deck_source(notes: &str) -> String {
    format!(
        "deck:\n  id: deck.csv-fixture\n  name: CSV fixture\n  description: ''\n  adapter_ids: {{}}\nnote_types:\n  note-type.basic:\n    name: Basic\n    field_order:\n      - field.front\n      - field.back\n    fields:\n      field.front:\n        name: Front\n      field.back:\n        name: Back\n    card_template_order: []\n    card_templates: {{}}\n    styling: ''\n    adapter_ids: {{}}\n{notes}media: {{}}\ntombstones: []\n"
    )
}

fn descriptor(fields: &str) -> String {
    format!(
        "version: 1\nprimary_table: main\ntables:\n  main:\n    path: data/notes.csv\nparameters: {{}}\njoins: []\nnote:\n  id: main.stable_id\n  note_type_id: note-type.basic\n  fields:\n{fields}  tags:\n    column: main.tags\n    delimiter: '|'\n  adapter_ids:\n    crowdanki:\n      column: main.guid\n"
    )
}

fn valid_descriptor() -> String {
    descriptor(
        "    field.front:\n      column: main.front\n      type: scalar\n    field.back:\n      column: main.back\n      type: scalar\n",
    )
}

fn inline_notes() -> &'static str {
    "notes:\n  note.one:\n    note_type_id: note-type.basic\n    fields:\n      field.front: Front\n      field.back: Back\n    tags: []\n    adapter_ids: {}\n"
}

fn csv_declaration() -> &'static str {
    "notes: !csv\n  descriptor: sources/notes.yaml\n  parameters: {}\n"
}

fn parse_csv_document(csv: impl Into<Vec<u8>>) -> CanonicalSourceDocument {
    let csv = csv.into();
    CanonicalSourceDocument::parse_with_csv_sources(
        source("deck.yaml", deck_source(csv_declaration())),
        |request| Err(format!("unexpected include {}", request.target())),
        |request| match request.kind() {
            CsvSourceRequestKind::Descriptor => Ok(csv_source(
                "sources/notes.yaml",
                valid_descriptor().into_bytes(),
            )),
            CsvSourceRequestKind::Table { alias } if alias == "main" => {
                Ok(csv_source("data/notes.csv", csv.clone()))
            }
            other => Err(format!("unexpected CSV source request {other:?}")),
        },
    )
    .expect("CSV-backed deck parses")
}

#[test]
fn csv_note_source_materializes_rfc_csv_into_existing_note_shape_deterministically() {
    let csv = concat!(
        "stable_id,front,back,tags,guid\r\n",
        "note.two,\"Zażółć, gęślą\",\"line one\nline two\",\"utf8|quoted\",guid-two\r\n",
        "note.one,,Back,,\r\n",
    );
    let document = parse_csv_document(csv.as_bytes());

    assert!(
        document.deck().notes.is_empty(),
        "root source stays unexpanded"
    );
    let ids = document
        .resolved_deck()
        .notes
        .keys()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    assert_eq!(ids, ["note.one", "note.two"]);

    let one = &document.resolved_deck().notes[&sid("note.one")];
    assert_eq!(one.note_type_id, sid("note-type.basic"));
    assert_eq!(
        one.fields[&sid("field.front")],
        FieldValue::Scalar(String::new())
    );
    assert_eq!(
        one.fields[&sid("field.back")],
        FieldValue::Scalar("Back".to_owned())
    );
    assert!(one.tags.is_empty());
    assert!(one.adapter_ids.is_empty());

    let two = &document.resolved_deck().notes[&sid("note.two")];
    assert_eq!(
        two.fields[&sid("field.front")],
        FieldValue::Scalar("Zażółć, gęślą".to_owned())
    );
    assert_eq!(
        two.fields[&sid("field.back")],
        FieldValue::Scalar("line one\nline two".to_owned())
    );
    assert_eq!(
        two.tags.iter().cloned().collect::<Vec<_>>(),
        ["quoted", "utf8"]
    );
    assert_eq!(two.adapter_ids.get("crowdanki"), Some("guid-two"));
}

#[test]
fn csv_note_declaration_is_preserved_by_document_and_generic_formatting() {
    let csv = b"stable_id,front,back,tags,guid\nnote.one,Front,Back,,guid-one\n";
    let document = parse_csv_document(csv.as_slice());
    let emitted = document.emit().expect("CSV declaration emits");

    assert!(emitted.root().text().contains(csv_declaration()));
    assert!(!emitted.root().text().contains("note.one:"));
    assert_eq!(
        CanonicalSourceDocument::parse_with_csv_sources(
            emitted.root().clone(),
            |request| Err(format!("unexpected include {}", request.target())),
            |request| match request.kind() {
                CsvSourceRequestKind::Descriptor => Ok(csv_source(
                    "sources/notes.yaml",
                    valid_descriptor().into_bytes(),
                )),
                CsvSourceRequestKind::Table { .. } => Ok(csv_source("data/notes.csv", csv)),
            },
        )
        .unwrap()
        .emit()
        .unwrap()
        .root()
        .text(),
        emitted.root().text()
    );

    let formatted = source_includes::format_preserving_file_includes(
        &deck_source(csv_declaration()),
        brain_brew_formats::canonical_yaml::format_str,
    )
    .expect("generic formatter preserves !csv");
    assert!(formatted.contains(csv_declaration()));
    assert!(!formatted.contains("note.one:"));
}

#[test]
fn direct_note_map_formatting_remains_unchanged() {
    let input = deck_source(inline_notes());
    let formatted = source_includes::format_preserving_file_includes(
        &input,
        brain_brew_formats::canonical_yaml::format_str,
    )
    .unwrap();
    assert_eq!(formatted, input);
}

#[test]
fn csv_declaration_rejects_parameters_and_unknown_keys_in_this_slice() {
    for (label, declaration, expected) in [
        (
            "parameters",
            "notes: !csv\n  descriptor: sources/notes.yaml\n  parameters:\n    language: de\n",
            "parameters are not supported",
        ),
        (
            "unknown key",
            "notes: !csv\n  descriptor: sources/notes.yaml\n  parameters: {}\n  unsupported: true\n",
            "unknown field `unsupported`",
        ),
    ] {
        let error = CanonicalSourceDocument::parse_with_csv_sources(
            source("deck.yaml", deck_source(declaration)),
            |request| Err(format!("unexpected include {}", request.target())),
            |request| Err(format!("unexpected CSV load {}", request.target())),
        )
        .expect_err(label)
        .to_string();
        assert!(error.contains(expected), "{label}: {error}");
    }
}

#[test]
fn csv_notes_round_trip_with_structural_and_scalar_includes_in_canonical_order() {
    let root = format!(
        "deck:\n  id: deck.csv-includes\n  name: CSV includes\n  description: !include content/description.md\n  adapter_ids: {{}}\nnote_types: !include schema/note-types.yaml\n{}media: !include media.yaml\ntombstones: []\n",
        csv_declaration()
    );
    let note_types = "note-type.basic:\n  name: Basic\n  field_order:\n    - field.front\n    - field.back\n  fields:\n    field.front:\n      name: Front\n    field.back:\n      name: Back\n  card_template_order: []\n  card_templates: {}\n  styling: ''\n  adapter_ids: {}\n";
    let csv = b"stable_id,front,back,tags,guid\nnote.one,Front,Back,,guid-one\n";
    let load_include = |request: &brain_brew_formats::source_document::IncludeRequest| match request
        .target()
    {
        "content/description.md" => Ok(source("content/description.md", "Included description\n")),
        "schema/note-types.yaml" => Ok(source("schema/note-types.yaml", note_types)),
        "media.yaml" => Ok(source("media.yaml", "{}\n")),
        other => Err(format!("unexpected include {other}")),
    };
    let load_csv =
        |request: &brain_brew_formats::csv_note_source::CsvSourceRequest| match request.kind() {
            CsvSourceRequestKind::Descriptor => Ok(csv_source(
                "sources/notes.yaml",
                valid_descriptor().into_bytes(),
            )),
            CsvSourceRequestKind::Table { .. } => Ok(csv_source("data/notes.csv", csv)),
        };

    let document = CanonicalSourceDocument::parse_with_csv_sources(
        source("deck.yaml", root.clone()),
        load_include,
        load_csv,
    )
    .expect("combined source parses");
    assert!(
        document
            .resolved_deck()
            .notes
            .contains_key(&sid("note.one"))
    );
    assert_eq!(
        document.resolved_deck().description,
        "Included description\n"
    );

    let emitted = document.emit().expect("combined source emits");
    assert_eq!(emitted.root().text(), root);
    assert!(emitted.included().is_empty());
    assert!(!emitted.root().text().contains("note.one:"));
    let note_types_at = emitted.root().text().find("note_types: !include").unwrap();
    let notes_at = emitted.root().text().find("notes: !csv").unwrap();
    let media_at = emitted.root().text().find("media: !include").unwrap();
    assert!(note_types_at < notes_at && notes_at < media_at);

    let formatted = source_includes::format_preserving_file_includes(
        &root,
        brain_brew_formats::canonical_yaml::format_str,
    )
    .expect("combined source formats without loading or expansion");
    assert_eq!(formatted, root);
}

#[test]
fn descriptor_and_csv_failures_are_strict_and_diagnostic() {
    let note_types = parse_csv_document(
        b"stable_id,front,back,tags,guid\nnote.one,Front,Back,,guid-one\n".as_slice(),
    )
    .resolved_deck()
    .note_types
    .clone();

    for (label, descriptor_text, csv, expected) in [
        (
            "unknown descriptor key",
            valid_descriptor().replace("version: 1", "version: 1\nunknown: value"),
            "stable_id,front,back,tags,guid\nnote.one,Front,Back,,guid-one\n",
            "unknown field `unknown`",
        ),
        (
            "missing mapped header",
            valid_descriptor(),
            "stable_id,front,tags,guid\nnote.one,Front,,guid-one\n",
            "mapped header \"back\" is absent",
        ),
        (
            "invalid id",
            valid_descriptor(),
            "stable_id,front,back,tags,guid\ninvalid id,Front,Back,,guid-one\n",
            "invalid stable id",
        ),
        (
            "duplicate id",
            valid_descriptor(),
            "stable_id,front,back,tags,guid\nnote.one,Front,Back,,guid-one\nnote.one,Other,Back,,guid-two\n",
            "duplicate stable note ID note.one",
        ),
        (
            "incomplete record",
            valid_descriptor(),
            "stable_id,front,back,tags,guid\nnote.one,Front,Back\n",
            "found record with 3 fields, but the previous record has 5 fields",
        ),
        (
            "unknown field mapping",
            descriptor(
                "    field.front:\n      column: main.front\n      type: scalar\n    field.unknown:\n      column: main.back\n      type: scalar\n",
            ),
            "stable_id,front,back,tags,guid\nnote.one,Front,Back,,guid-one\n",
            "unknown field mapping field.unknown",
        ),
        (
            "missing field mapping",
            descriptor("    field.front:\n      column: main.front\n      type: scalar\n"),
            "stable_id,front,back,tags,guid\nnote.one,Front,Back,,guid-one\n",
            "missing field mapping field.back",
        ),
    ] {
        let parsed = CsvNoteSourceDescriptor::parse(source("sources/notes.yaml", descriptor_text));
        let result = parsed.and_then(|descriptor| {
            CsvNoteSourceMaterializer::new(descriptor).materialize(
                &BTreeMap::from([("main".to_owned(), csv_source("data/notes.csv", csv))]),
                &note_types,
            )
        });
        let error = result.expect_err(label).to_string();
        assert!(error.contains(expected), "{label}: {error}");
        assert!(
            error.contains("sources/notes.yaml") || error.contains("data/notes.csv"),
            "{label}: {error}"
        );
    }
}

#[test]
fn csv_cell_failures_name_logical_row_column_and_header() {
    let note_types = parse_csv_document(
        b"stable_id,front,back,tags,guid\nnote.seed,Front,Back,,guid-seed\n".as_slice(),
    )
    .resolved_deck()
    .note_types
    .clone();
    let descriptor =
        CsvNoteSourceDescriptor::parse(source("sources/notes.yaml", valid_descriptor())).unwrap();
    let error = CsvNoteSourceMaterializer::new(descriptor)
        .materialize(
            &BTreeMap::from([(
                "main".to_owned(),
                csv_source(
                    "data/notes.csv",
                    "stable_id,front,back,tags,guid\nnote.one,Front,Back,,one\nnote.one,Other,Back,,two\n",
                ),
            )]),
            &note_types,
        )
        .expect_err("duplicate ID fails")
        .to_string();

    assert!(
        error.contains("data/notes.csv:row 3:column 1 (stable_id)"),
        "{error}"
    );
}

#[test]
fn malformed_encoding_headers_tags_and_unsupported_slice_features_fail_closed() {
    let note_types = parse_csv_document(
        b"stable_id,front,back,tags,guid\nnote.seed,Front,Back,,guid-seed\n".as_slice(),
    )
    .resolved_deck()
    .note_types
    .clone();

    let cases: Vec<(&str, String, Vec<u8>, &str)> = vec![
        (
            "invalid utf8",
            valid_descriptor(),
            b"stable_id,front,back,tags,guid\nnote.one,\xff,Back,,one\n".to_vec(),
            "invalid UTF-8",
        ),
        (
            "bom",
            valid_descriptor(),
            b"\xef\xbb\xbfstable_id,front,back,tags,guid\nnote.one,Front,Back,,one\n".to_vec(),
            "UTF-8 BOM",
        ),
        (
            "duplicate header",
            valid_descriptor(),
            b"stable_id,front,front,tags,guid\nnote.one,Front,Back,,one\n".to_vec(),
            "duplicate header \"front\"",
        ),
        (
            "empty header",
            valid_descriptor(),
            b"stable_id,front,,tags,guid\nnote.one,Front,Back,,one\n".to_vec(),
            "empty header",
        ),
        (
            "duplicate tag",
            valid_descriptor(),
            b"stable_id,front,back,tags,guid\nnote.one,Front,Back,one|one,guid\n".to_vec(),
            "duplicate tag \"one\"",
        ),
        (
            "empty tag segment",
            valid_descriptor(),
            b"stable_id,front,back,tags,guid\nnote.one,Front,Back,one||two,guid\n".to_vec(),
            "empty tag segment",
        ),
        (
            "joins deferred",
            valid_descriptor().replace("joins: []", "joins:\n  - left: main.id\n    right: other.id\n    required: true"),
            b"stable_id,front,back,tags,guid\nnote.one,Front,Back,,one\n".to_vec(),
            "joins are not supported",
        ),
        (
            "parameters deferred",
            valid_descriptor().replace(
                "parameters: {}",
                "parameters:\n  language:\n    type: localized_column\n    default: ''\n    separator: ':'",
            ),
            b"stable_id,front,back,tags,guid\nnote.one,Front,Back,,one\n".to_vec(),
            "parameters are not supported",
        ),
    ];

    for (label, descriptor_text, bytes, expected) in cases {
        let parsed = CsvNoteSourceDescriptor::parse(source("sources/notes.yaml", descriptor_text));
        let result = parsed.and_then(|descriptor| {
            CsvNoteSourceMaterializer::new(descriptor).materialize(
                &BTreeMap::from([("main".to_owned(), csv_source("data/notes.csv", bytes))]),
                &note_types,
            )
        });
        let error = result.expect_err(label).to_string();
        assert!(error.contains(expected), "{label}: {error}");
    }
}
