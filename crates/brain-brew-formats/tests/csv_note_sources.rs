use std::collections::BTreeMap;

use brain_brew_core::{FieldImageReference, FieldValue, StableId};
use brain_brew_formats::canonical_source_document::{
    CanonicalScalarTarget, CanonicalSourceDocument,
};
use brain_brew_formats::csv_note_source::{
    CsvNoteSourceDescriptor, CsvNoteSourceMaterializer, CsvSourceFile, CsvSourceRequestKind,
};
use brain_brew_formats::source_document::{SourceFile, SourceProvenance};
use brain_brew_formats::{crowdanki, media, source_includes};

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

const MIXED_BASELINE: &str = include_str!("fixtures/csv_notes_mixed/baseline.yaml");
const MIXED_MIGRATED: &str = include_str!("fixtures/csv_notes_mixed/mixed.yaml");
const MIXED_DESCRIPTOR: &[u8] = include_bytes!("fixtures/csv_notes_mixed/descriptor.yaml");
const MIXED_CSV: &[u8] = include_bytes!("fixtures/csv_notes_mixed/notes.csv");

fn parse_migration_fixture(root: &str) -> Result<CanonicalSourceDocument, String> {
    CanonicalSourceDocument::parse_with_csv_sources(
        source("deck.yaml", root),
        |request| Err(format!("unexpected include {}", request.target())),
        |request| match request.kind() {
            CsvSourceRequestKind::Descriptor => {
                Ok(csv_source("sources/notes.yaml", MIXED_DESCRIPTOR))
            }
            CsvSourceRequestKind::Table { alias } if alias == "main" => {
                Ok(csv_source("data/notes.csv", MIXED_CSV))
            }
            other => Err(format!("unexpected CSV source request {other:?}")),
        },
    )
    .map_err(|error| error.to_string())
}

#[test]
fn csv_to_inline_storage_migration_preserves_canonical_and_crowdanki_bytes() {
    let baseline = parse_migration_fixture(MIXED_BASELINE).expect("baseline materializes");
    let migrated = parse_migration_fixture(MIXED_MIGRATED).expect("mixed source materializes");

    assert!(
        baseline
            .resolved_deck()
            .semantic_diff(migrated.resolved_deck())
            .is_empty(),
        "moving one complete note changes no CanonicalDeck semantics"
    );
    assert_eq!(
        crowdanki::export_deck(baseline.resolved_deck())
            .unwrap()
            .deck_json,
        crowdanki::export_deck(migrated.resolved_deck())
            .unwrap()
            .deck_json,
        "representative CrowdAnki output stays byte-equal"
    );
}

#[test]
fn full_three_item_source_sequence_materializes_two_csv_sources_and_inline_disjointly() {
    use brain_brew_formats::canonical_source_document::NoteAuthoringSourceKind;

    let root = MIXED_MIGRATED.replace(
        "  - !inline\n",
        "  - !csv\n    descriptor: sources/extra.yaml\n    parameters: {}\n  - !inline\n",
    );
    let extra_descriptor = valid_descriptor().replace("data/notes.csv", "data/extra.csv");
    let extra_csv = b"stable_id,front,back,tags,guid\nnote.spain,Spain,Madrid,europe,guid-spain\n";
    let mut loaded = Vec::new();
    let document = CanonicalSourceDocument::parse_with_csv_sources(
        source("deck.yaml", root.clone()),
        |request| Err(format!("unexpected include {}", request.target())),
        |request| {
            loaded.push(request.target().to_owned());
            match (request.kind(), request.target()) {
                (CsvSourceRequestKind::Descriptor, "sources/notes.yaml") => {
                    Ok(csv_source("sources/notes.yaml", MIXED_DESCRIPTOR))
                }
                (CsvSourceRequestKind::Descriptor, "sources/extra.yaml") => Ok(csv_source(
                    "sources/extra.yaml",
                    extra_descriptor.clone().into_bytes(),
                )),
                (CsvSourceRequestKind::Table { alias }, "data/notes.csv") if alias == "main" => {
                    Ok(csv_source("data/notes.csv", MIXED_CSV))
                }
                (CsvSourceRequestKind::Table { alias }, "data/extra.csv") if alias == "main" => {
                    Ok(csv_source("data/extra.csv", extra_csv))
                }
                other => Err(format!("unexpected CSV source request {other:?}")),
            }
        },
    )
    .expect("three-item source sequence materializes");

    assert_eq!(
        loaded,
        [
            "sources/notes.yaml",
            "data/notes.csv",
            "sources/extra.yaml",
            "data/extra.csv",
        ]
    );
    assert_eq!(
        document
            .resolved_deck()
            .notes
            .keys()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["note.france", "note.germany", "note.spain"]
    );
    assert_eq!(
        document.resolved_deck().notes[&sid("note.france")].fields[&sid("field.back")],
        FieldValue::Scalar("Paris".to_owned())
    );
    assert_eq!(
        document.resolved_deck().notes[&sid("note.germany")].fields[&sid("field.back")],
        FieldValue::Scalar("Berlin".to_owned())
    );
    assert_eq!(
        document.resolved_deck().notes[&sid("note.spain")].fields[&sid("field.back")],
        FieldValue::Scalar("Madrid".to_owned())
    );
    assert_eq!(document.emit().unwrap().root().text(), root);

    let provenance = document.authoring_provenance();
    assert_eq!(
        provenance.note(&sid("note.france")).unwrap().source_kind(),
        NoteAuthoringSourceKind::Inline
    );
    let spain = provenance.note(&sid("note.spain")).unwrap();
    assert_eq!(spain.source_kind(), NoteAuthoringSourceKind::Csv);
    assert_eq!(spain.declaration_path(), "notes[1]");
    assert_eq!(
        spain.descriptor().unwrap().source_name(),
        "sources/extra.yaml"
    );
    assert_eq!(spain.file().unwrap().source_name(), "data/extra.csv");
    assert_eq!(spain.logical_row(), Some(2));
}

#[test]
fn mixed_sources_reject_collisions_even_when_materialized_notes_are_equal() {
    let without_exclusion =
        MIXED_MIGRATED.replace("    exclude:\n      note_ids:\n        - note.france\n", "");
    for (label, root) in [
        ("equal", without_exclusion.clone()),
        (
            "different",
            without_exclusion.replace("field.back: Paris", "field.back: Lyon"),
        ),
    ] {
        let error = parse_migration_fixture(&root).expect_err(label).to_string();
        assert!(
            error.contains("duplicate ownership of note ID note.france"),
            "{label}: {error}"
        );
        assert!(
            error.contains("source order never overrides"),
            "{label}: {error}"
        );
    }
}

#[test]
fn csv_exclusions_reject_unknown_duplicates_and_missing_transfers() {
    let unknown = MIXED_MIGRATED.replace("note.france\n  - !inline", "note.unknown\n  - !inline");
    let error = parse_migration_fixture(&unknown)
        .expect_err("unknown exclusion fails")
        .to_string();
    assert!(
        error.contains("unknown excluded note ID note.unknown"),
        "{error}"
    );

    let duplicate = MIXED_MIGRATED.replace(
        "        - note.france\n  - !inline",
        "        - note.france\n        - note.france\n  - !inline",
    );
    let error = parse_migration_fixture(&duplicate)
        .expect_err("duplicate exclusion fails")
        .to_string();
    assert!(
        error.contains("duplicate excluded note ID note.france"),
        "{error}"
    );

    let missing_transfer = MIXED_BASELINE.replace(
        "  parameters: {}\nmedia:",
        "  parameters: {}\n  exclude:\n    note_ids:\n      - note.france\nmedia:",
    );
    let error = parse_migration_fixture(&missing_transfer)
        .expect_err("an exclusion without a new owner fails")
        .to_string();
    assert!(error.contains("ownership transfer is missing"), "{error}");
}

#[test]
fn note_source_sequences_require_explicit_known_tags_and_well_formed_values() {
    let cases = [
        (
            "untagged",
            "notes:\n  - descriptor: sources/notes.yaml\n    parameters: {}\n",
            "must be explicitly tagged !csv or !inline",
        ),
        (
            "unknown tag",
            "notes:\n  - !json\n    path: notes.json\n",
            "unsupported notes source item tag !json",
        ),
        (
            "non-map inline",
            "notes:\n  - !inline note.france\n",
            "must contain a note map",
        ),
        (
            "direct inline wrapper",
            "notes: !inline\n  note.france: {}\n",
            "unsupported direct notes tag !inline",
        ),
    ];
    for (label, declaration, expected) in cases {
        let error = CanonicalSourceDocument::parse_with_csv_sources(
            source("deck.yaml", deck_source(declaration)),
            |request| Err(format!("unexpected include {}", request.target())),
            |request| Err(format!("unexpected CSV source {}", request.target())),
        )
        .expect_err(label)
        .to_string();
        assert!(error.contains(expected), "{label}: {error}");
    }
}

#[test]
fn mixed_source_emission_and_authoring_provenance_are_deterministic() {
    use brain_brew_formats::canonical_source_document::NoteAuthoringSourceKind;

    let document = parse_migration_fixture(MIXED_MIGRATED).unwrap();
    assert_eq!(document.emit().unwrap().root().text(), MIXED_MIGRATED);
    assert_eq!(
        source_includes::format_preserving_file_includes(
            MIXED_MIGRATED,
            brain_brew_formats::canonical_yaml::format_str,
        )
        .unwrap(),
        MIXED_MIGRATED
    );

    let provenance = document.authoring_provenance();
    assert_eq!(
        provenance
            .notes()
            .map(|(id, _)| id.to_string())
            .collect::<Vec<_>>(),
        ["note.france", "note.germany"]
    );
    let france = provenance.note(&sid("note.france")).unwrap();
    assert_eq!(france.source_kind(), NoteAuthoringSourceKind::Inline);
    assert_eq!(france.root_declaration().source_name(), "deck.yaml");
    assert_eq!(france.declaration_path(), "notes[1]");
    assert_eq!(france.canonical_path(), "notes.note.france");
    assert!(france.descriptor().is_none());
    assert!(france.file().is_none());

    let germany = provenance.note(&sid("note.germany")).unwrap();
    assert_eq!(germany.source_kind(), NoteAuthoringSourceKind::Csv);
    assert_eq!(germany.declaration_path(), "notes[0]");
    assert_eq!(
        germany.descriptor().unwrap().source_name(),
        "sources/notes.yaml"
    );
    assert_eq!(germany.table(), Some("main"));
    assert_eq!(germany.file().unwrap().source_name(), "data/notes.csv");
    assert_eq!(germany.logical_row(), Some(3));
    assert_eq!(germany.header(), Some("stable_id"));
    assert_eq!(germany.column(), Some(1));
    assert_eq!(germany.canonical_path(), "notes.note.germany");

    let front = provenance
        .field(&sid("note.germany"), &sid("field.front"))
        .unwrap();
    assert_eq!(front.source_kind(), NoteAuthoringSourceKind::Csv);
    assert_eq!(front.logical_row(), Some(3));
    assert_eq!(front.header(), Some("front"));
    assert_eq!(front.column(), Some(2));
    assert_eq!(
        front.canonical_path(),
        "notes.note.germany.fields.field.front"
    );
    assert_eq!(provenance.fields().count(), 4);
}

#[test]
fn mixed_inline_notes_preserve_scalar_includes_without_expanding_csv_notes() {
    let root = MIXED_MIGRATED.replace("field.back: Paris", "field.back: !include paris.txt");
    let mut document = CanonicalSourceDocument::parse_with_csv_sources(
        source("deck.yaml", root.clone()),
        |request| match request.target() {
            "paris.txt" => Ok(source("paris.txt", "Paris")),
            other => Err(format!("unexpected include {other}")),
        },
        |request| match request.kind() {
            CsvSourceRequestKind::Descriptor => {
                Ok(csv_source("sources/notes.yaml", MIXED_DESCRIPTOR))
            }
            CsvSourceRequestKind::Table { alias } if alias == "main" => {
                Ok(csv_source("data/notes.csv", MIXED_CSV))
            }
            other => Err(format!("unexpected CSV source request {other:?}")),
        },
    )
    .unwrap();

    assert_eq!(
        document.resolved_deck().notes[&sid("note.france")].fields[&sid("field.back")],
        FieldValue::Scalar("Paris".to_owned())
    );
    assert_eq!(document.emit().unwrap().root().text(), root);
    assert!(
        !document
            .emit()
            .unwrap()
            .root()
            .text()
            .contains("note.germany:")
    );
    document
        .set_scalar(
            CanonicalScalarTarget::NoteField {
                note_id: sid("note.france"),
                field_id: sid("field.back"),
            },
            "Paris",
            "Lyon",
        )
        .unwrap();
    let edited = document.emit().unwrap();
    assert_eq!(edited.root().text(), root);
    assert_eq!(edited.included_source("paris.txt").unwrap().text(), "Lyon");
    assert_eq!(
        source_includes::format_preserving_file_includes(
            &root,
            brain_brew_formats::canonical_yaml::format_str,
        )
        .unwrap(),
        root
    );
}

#[test]
fn direct_inline_and_single_csv_sources_expose_ownership_without_wrappers() {
    use brain_brew_formats::canonical_source_document::NoteAuthoringSourceKind;

    let inline =
        CanonicalSourceDocument::parse(source("deck.yaml", deck_source(inline_notes()))).unwrap();
    assert_eq!(
        inline
            .authoring_provenance()
            .note(&sid("note.one"))
            .unwrap()
            .source_kind(),
        NoteAuthoringSourceKind::Inline
    );
    let csv = parse_csv_document(
        b"stable_id,front,back,tags,guid\nnote.one,Front,Back,,guid-one\n".as_slice(),
    );
    assert_eq!(
        csv.authoring_provenance()
            .field(&sid("note.one"), &sid("field.back"))
            .unwrap()
            .source_kind(),
        NoteAuthoringSourceKind::Csv
    );
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
fn csv_declaration_rejects_unknown_keys_and_non_literal_parameter_values() {
    for (label, declaration, expected) in [
        (
            "non-literal parameter",
            "notes: !csv\n  descriptor: sources/notes.yaml\n  parameters:\n    language:\n      inferred: de\n",
            "invalid type: map, expected a string",
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
            "undeclared joined alias",
            valid_descriptor().replace("joins: []", "joins:\n  - left: main.id\n    right: other.id\n    required: true"),
            b"stable_id,front,back,tags,guid\nnote.one,Front,Back,,one\n".to_vec(),
            "undeclared table alias",
        ),
        (
            "unsupported parameter type",
            valid_descriptor().replace(
                "parameters: {}",
                "parameters:\n  language:\n    type: expression\n    default: ''\n    separator: ':'",
            ),
            b"stable_id,front,back,tags,guid\nnote.one,Front,Back,,one\n".to_vec(),
            "unsupported type \"expression\"",
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

const JOINED_DESCRIPTOR: &str = include_str!("fixtures/csv_notes_joins/descriptor.yaml");
const JOINED_MAIN: &[u8] = include_bytes!("fixtures/csv_notes_joins/main.csv");
const JOINED_COUNTRY: &[u8] = include_bytes!("fixtures/csv_notes_joins/country.csv");
const JOINED_GUID: &[u8] = include_bytes!("fixtures/csv_notes_joins/guid.csv");

fn joined_document(language: Option<&str>) -> CanonicalSourceDocument {
    let parameters = language.map_or_else(
        || "parameters: {}\n".to_owned(),
        |value| format!("parameters:\n    language: {value:?}\n"),
    );
    let declaration = format!("notes: !csv\n  descriptor: sources/joined.yaml\n  {parameters}");
    CanonicalSourceDocument::parse_with_csv_sources(
        source("deck.yaml", deck_source(&declaration)),
        |request| Err(format!("unexpected include {}", request.target())),
        |request| match request.kind() {
            CsvSourceRequestKind::Descriptor => {
                Ok(csv_source("sources/joined.yaml", JOINED_DESCRIPTOR))
            }
            CsvSourceRequestKind::Table { alias } => match alias.as_str() {
                "main" => Ok(csv_source("data/main.csv", JOINED_MAIN)),
                "country" => Ok(csv_source("data/country.csv", JOINED_COUNTRY)),
                "guid" => Ok(csv_source("data/guid.csv", JOINED_GUID)),
                _ => Err(format!("unexpected alias {alias}")),
            },
        },
    )
    .expect("joined fixture materializes")
}

fn joined_materialization(
    descriptor_text: impl Into<String>,
    main: impl Into<Vec<u8>>,
    country: impl Into<Vec<u8>>,
    guid: impl Into<Vec<u8>>,
    parameters: BTreeMap<String, String>,
) -> Result<BTreeMap<StableId, brain_brew_core::Note>, String> {
    let note_types = joined_document(None).resolved_deck().note_types.clone();
    let descriptor = CsvNoteSourceDescriptor::parse(source("sources/joined.yaml", descriptor_text))
        .map_err(|error| error.to_string())?;
    CsvNoteSourceMaterializer::new(descriptor)
        .with_parameters(&parameters)
        .map_err(|error| error.to_string())?
        .materialize(
            &BTreeMap::from([
                ("guid".to_owned(), csv_source("data/guid.csv", guid)),
                ("main".to_owned(), csv_source("data/main.csv", main)),
                (
                    "country".to_owned(),
                    csv_source("data/country.csv", country),
                ),
            ]),
            &note_types,
        )
        .map_err(|error| error.to_string())
}

#[test]
fn explicit_flat_joins_and_literal_localized_columns_materialize_fixture() {
    let defaulted = joined_document(None);
    let explicit_empty = joined_document(Some(""));
    assert_eq!(defaulted.resolved_deck(), explicit_empty.resolved_deck());

    for (language, country, capital, guid) in [
        (None, "France", "Paris", "guid-fr"),
        (Some("de"), "Frankreich", "Paris", "guid-fr-de"),
        (Some("zh-tw"), "法國", "巴黎", "guid-fr-zh-tw"),
    ] {
        let document = joined_document(language);
        let note = &document.resolved_deck().notes[&sid("note.fr")];
        assert_eq!(
            note.fields[&sid("field.front")],
            FieldValue::Scalar(country.to_owned())
        );
        assert_eq!(
            note.fields[&sid("field.back")],
            FieldValue::Scalar(capital.to_owned())
        );
        assert_eq!(note.adapter_ids.get("crowdanki"), Some(guid));
        assert_eq!(
            note.tags.iter().cloned().collect::<Vec<_>>(),
            ["europe", "geo"]
        );
        assert_eq!(
            document
                .resolved_deck()
                .notes
                .keys()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            ["note.de", "note.fr"]
        );
    }
}

#[test]
fn declaration_preserves_explicit_literal_parameter_values() {
    let emitted = joined_document(Some("zh-tw")).emit().unwrap();
    assert!(
        emitted
            .root()
            .text()
            .contains("parameters:\n    language: zh-tw\n")
    );
}

#[test]
fn optional_join_absence_contributes_empty_cells_but_present_keys_stay_unique() {
    let descriptor = JOINED_DESCRIPTOR.replace(
        "right: country.country\n  - left:",
        "right: country.country\n    required: false\n  - left:",
    );
    let main = concat!(
        "stable_id,country,capital,capital:de,capital:zh-tw,tags\n",
        "note.fr,country.fr,Paris,Paris,巴黎,geo\n",
        "note.missing,country.missing,Nowhere,Nirgendwo,無,geo\n",
    );
    let guid = concat!(
        "country,guid,guid:de,guid:zh-tw\n",
        "country.fr,guid-fr,guid-fr-de,guid-fr-zh-tw\n",
        "country.missing,guid-missing,guid-missing-de,guid-missing-zh-tw\n",
    );
    let notes = joined_materialization(
        descriptor.clone(),
        main,
        JOINED_COUNTRY,
        guid,
        BTreeMap::new(),
    )
    .unwrap();
    assert_eq!(
        notes[&sid("note.missing")].fields[&sid("field.front")],
        FieldValue::Scalar(String::new())
    );
    assert_eq!(
        notes[&sid("note.missing")].adapter_ids.get("crowdanki"),
        Some("guid-missing")
    );

    let duplicate_country = concat!(
        "country,name,name:de,name:zh-tw\n",
        "country.fr,France,Frankreich,法國\n",
        "country.fr,Duplicate,Duplikat,重複\n",
    );
    let error = joined_materialization(descriptor, main, duplicate_country, guid, BTreeMap::new())
        .expect_err("optional present matches remain unique");
    assert!(
        error.contains("duplicate join right key \"country.fr\""),
        "{error}"
    );
}

#[test]
fn optional_join_absence_contributes_empty_scalar_for_image_mapping() {
    let descriptor = JOINED_DESCRIPTOR
        .replace(
            "right: country.country\n  - left:",
            "right: country.country\n    required: false\n  - left:",
        )
        .replace(
            "    field.front:\n      column: country.name\n      localized_by: language\n      type: scalar\n",
            "    field.front:\n      column: country.name\n      type: image\n",
        );
    let main = concat!(
        "stable_id,country,capital,capital:de,capital:zh-tw,tags\n",
        "note.missing,country.missing,Nowhere,Nirgendwo,無,geo\n",
    );
    let guid = concat!(
        "country,guid,guid:de,guid:zh-tw\n",
        "country.missing,guid-missing,guid-missing-de,guid-missing-zh-tw\n",
    );

    let notes =
        joined_materialization(descriptor, main, JOINED_COUNTRY, guid, BTreeMap::new()).unwrap();

    assert_eq!(
        notes[&sid("note.missing")].fields[&sid("field.front")],
        FieldValue::Scalar(String::new())
    );
}

#[test]
fn join_key_cardinality_and_required_match_fail_closed() {
    let cases = [
        (
            "empty left key",
            "stable_id,country,capital,capital:de,capital:zh-tw,tags\nnote.one,,X,X,X,geo\n",
            String::from_utf8(JOINED_COUNTRY.to_vec()).unwrap(),
            "join left key must not be empty",
        ),
        (
            "required miss",
            "stable_id,country,capital,capital:de,capital:zh-tw,tags\nnote.one,country.missing,X,X,X,geo\n",
            String::from_utf8(JOINED_COUNTRY.to_vec()).unwrap(),
            "required join to table \"country\" has no match",
        ),
        (
            "empty right key",
            "stable_id,country,capital,capital:de,capital:zh-tw,tags\nnote.one,country.fr,X,X,X,geo\n",
            "country,name,name:de,name:zh-tw\n,Empty,Leer,空\ncountry.fr,France,Frankreich,法國\n".to_owned(),
            "join right key must not be empty",
        ),
        (
            "duplicate right key",
            "stable_id,country,capital,capital:de,capital:zh-tw,tags\nnote.one,country.fr,X,X,X,geo\n",
            "country,name,name:de,name:zh-tw\ncountry.fr,France,Frankreich,法國\ncountry.fr,Other,Andere,其他\n".to_owned(),
            "duplicate join right key \"country.fr\"",
        ),
    ];
    for (label, main, country, expected) in cases {
        let result = joined_materialization(
            JOINED_DESCRIPTOR,
            main,
            country,
            JOINED_GUID,
            BTreeMap::new(),
        );
        let error = result.expect_err(label);
        assert!(error.contains(expected), "{label}: {error}");
    }

    let repeated_left = concat!(
        "stable_id,country,capital,capital:de,capital:zh-tw,tags\n",
        "note.one,country.fr,Paris,Paris,巴黎,geo\n",
        "note.two,country.fr,Paris,Paris,巴黎,geo\n",
    );
    let notes = joined_materialization(
        JOINED_DESCRIPTOR,
        repeated_left,
        JOINED_COUNTRY,
        JOINED_GUID,
        BTreeMap::new(),
    )
    .expect("left keys may repeat");
    assert_eq!(notes.len(), 2);
}

#[test]
fn recursive_implicit_and_ambiguous_join_declarations_fail_closed() {
    let cases = [
        (
            "left is joined",
            JOINED_DESCRIPTOR.replace("left: main.country", "left: country.country"),
            "must belong to primary table",
        ),
        (
            "right is primary",
            JOINED_DESCRIPTOR.replace("right: country.country", "right: main.country"),
            "must belong to a joined table",
        ),
        (
            "joined twice",
            JOINED_DESCRIPTOR.replace("right: guid.country", "right: country.country"),
            "joined more than once",
        ),
        (
            "unjoined alias",
            JOINED_DESCRIPTOR.replace(
                "  - left: main.country\n    right: guid.country\n    required: true\n",
                "",
            ),
            "non-primary table \"guid\" has no explicit join",
        ),
        (
            "undeclared alias",
            JOINED_DESCRIPTOR.replace("right: guid.country", "right: absent.country"),
            "undeclared table alias",
        ),
        (
            "invalid alias",
            JOINED_DESCRIPTOR.replacen("  guid:\n", "  bad.alias:\n", 1),
            "invalid table alias",
        ),
        (
            "duplicate alias collision",
            JOINED_DESCRIPTOR.replacen("  guid:\n", "  country:\n", 1),
            "duplicate key",
        ),
        (
            "missing named primary",
            JOINED_DESCRIPTOR.replace("primary_table: main", "primary_table: absent"),
            "primary table \"absent\" is not declared",
        ),
        (
            "unqualified column",
            JOINED_DESCRIPTOR.replace("left: main.country", "left: country"),
            "must be qualified",
        ),
        (
            "empty column header",
            JOINED_DESCRIPTOR.replace("left: main.country", "left: main."),
            "empty alias or header",
        ),
        (
            "joined note id",
            JOINED_DESCRIPTOR.replace("id: main.stable_id", "id: country.country"),
            "note ID column must belong to primary table",
        ),
        (
            "recursive declaration property",
            JOINED_DESCRIPTOR.replace("required: true", "required: true\n    joins: []"),
            "unknown field `joins`",
        ),
    ];
    for (label, descriptor, expected) in cases {
        let error = CsvNoteSourceDescriptor::parse(source("sources/joined.yaml", descriptor))
            .expect_err(label)
            .to_string();
        assert!(error.contains(expected), "{label}: {error}");
    }
}

const TYPED_MEDIA_DESCRIPTOR: &[u8] =
    include_bytes!("fixtures/csv_notes_typed_media/descriptor.yaml");
const TYPED_MEDIA_CSV: &[u8] = include_bytes!("fixtures/csv_notes_typed_media/notes.csv");
const TYPED_MEDIA_YAML: &str = include_str!("fixtures/csv_notes_typed_media/media.yaml");
const TYPED_MEDIA_FLAG: &[u8] = include_bytes!("fixtures/csv_notes_typed_media/flag.svg");
const TYPED_MEDIA_MAP: &[u8] = include_bytes!("fixtures/csv_notes_typed_media/map.svg");

fn typed_media_deck_source(tombstone: bool) -> String {
    let tombstones = if tombstone {
        "tombstones:\n  - kind: media_reference\n    path: media.media.flag.france\n"
    } else {
        "tombstones: []\n"
    };
    format!(
        "deck:\n  id: deck.csv-media\n  name: CSV media\n  description: ''\n  adapter_ids:\n    crowdanki:uuid: 43c5ba66-9a65-11e8-90c9-a0481cc15658\nnote_types:\n  note-type.country:\n    name: Country\n    field_order:\n      - field.country\n      - field.legacy-image\n      - field.flag\n      - field.map\n    fields:\n      field.country:\n        name: Country\n      field.legacy-image:\n        name: Legacy Image\n      field.flag:\n        name: Flag\n      field.map:\n        name: Map\n    card_template_order:\n      - template.country\n    card_templates:\n      template.country:\n        name: Country\n        question_format: '{{{{Country}}}}'\n        answer_format: '{{{{Flag}}}}{{{{Map}}}}'\n        adapter_ids: {{}}\n    styling: ''\n    adapter_ids:\n      crowdanki:uuid: aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa\nnotes: !csv\n  descriptor: sources/typed-media.yaml\n  parameters: {{}}\nmedia: !include media.yaml\n{tombstones}"
    )
}

fn parse_typed_media_document(
    csv: impl Into<Vec<u8>>,
    tombstone: bool,
) -> Result<CanonicalSourceDocument, String> {
    let csv = csv.into();
    CanonicalSourceDocument::parse_with_csv_sources(
        source("deck.yaml", typed_media_deck_source(tombstone)),
        |request| match request.target() {
            "media.yaml" => Ok(source("media.yaml", TYPED_MEDIA_YAML)),
            other => Err(format!("unexpected include {other}")),
        },
        |request| match request.kind() {
            CsvSourceRequestKind::Descriptor => Ok(csv_source(
                "sources/typed-media.yaml",
                TYPED_MEDIA_DESCRIPTOR,
            )),
            CsvSourceRequestKind::Table { alias } if alias == "main" => {
                Ok(csv_source("data/notes.csv", csv.clone()))
            }
            other => Err(format!("unexpected CSV source request {other:?}")),
        },
    )
    .map_err(|error| error.to_string())
}

#[test]
fn typed_image_cells_materialize_canonical_images_and_empty_scalars() {
    let document = parse_typed_media_document(TYPED_MEDIA_CSV, false).unwrap();
    let france = &document.resolved_deck().notes[&sid("note.france")];
    assert_eq!(
        france.fields[&sid("field.flag")],
        FieldValue::Images(vec![FieldImageReference {
            media_id: sid("media.flag.france"),
        }])
    );
    assert_eq!(
        france.fields[&sid("field.map")],
        FieldValue::Images(vec![FieldImageReference {
            media_id: sid("media.map.france"),
        }])
    );
    assert_eq!(
        france.fields[&sid("field.legacy-image")],
        FieldValue::Scalar("<img src=\"flags/france.svg\" />".to_owned())
    );

    let empty = &document.resolved_deck().notes[&sid("note.empty")];
    assert_eq!(
        empty.fields[&sid("field.flag")],
        FieldValue::Scalar(String::new())
    );
    assert_eq!(
        empty.fields[&sid("field.map")],
        FieldValue::Scalar(String::new())
    );

    media::validate_references(document.resolved_deck()).unwrap();
    media::validate_hashes(
        document.resolved_deck(),
        &BTreeMap::from([
            ("flags/france.svg".to_owned(), TYPED_MEDIA_FLAG.to_vec()),
            ("maps/france.svg".to_owned(), TYPED_MEDIA_MAP.to_vec()),
        ]),
    )
    .unwrap();
}

#[test]
fn typed_image_cells_reuse_stable_id_and_canonical_media_validation() {
    let malformed = String::from_utf8(TYPED_MEDIA_CSV.to_vec())
        .unwrap()
        .replace(
            "media.flag.france,media.map.france",
            "<img>,media.map.france",
        );
    let error = parse_typed_media_document(malformed, false).expect_err("malformed ID fails");
    assert!(
        error.contains("data/notes.csv:row 2:column 4 (flag): invalid stable id \"<img>\""),
        "{error}"
    );

    let multi = String::from_utf8(TYPED_MEDIA_CSV.to_vec())
        .unwrap()
        .replace(
            "media.flag.france,media.map.france",
            "media.flag.france|media.map.france,media.map.france",
        );
    let error =
        parse_typed_media_document(multi, false).expect_err("multiple image IDs in one cell fail");
    assert!(error.contains("invalid stable id"), "{error}");

    let otherwise_invalid = String::from_utf8(TYPED_MEDIA_CSV.to_vec())
        .unwrap()
        .replace("media.flag.france", "media.bad.fields.id");
    let error = parse_typed_media_document(otherwise_invalid, false)
        .expect_err("canonical DeckPath-invalid media ID fails");
    assert!(
        error.contains("contains reserved DeckPath marker .fields."),
        "{error}"
    );

    let unknown = String::from_utf8(TYPED_MEDIA_CSV.to_vec())
        .unwrap()
        .replace("media.flag.france", "media.flag.unknown");
    let error = parse_typed_media_document(unknown, false).expect_err("unknown media fails");
    assert!(
        error.contains("unknown media id \"media.flag.unknown\""),
        "{error}"
    );
    assert!(
        error.contains("notes.note.france.fields.field.flag"),
        "{error}"
    );

    let error = parse_typed_media_document(TYPED_MEDIA_CSV, true)
        .expect_err("tombstoned media fails canonical validation");
    assert!(
        error.contains("unknown media id \"media.flag.france\""),
        "{error}"
    );
    assert!(
        error.contains("notes.note.france.fields.field.flag"),
        "{error}"
    );
}

#[test]
fn typed_csv_images_keep_crowdanki_lowering_byte_equivalent_to_raw_html() {
    let document = parse_typed_media_document(TYPED_MEDIA_CSV, false).unwrap();
    let typed = document.resolved_deck();
    let mut raw = typed.clone();
    let france = raw.notes.get_mut(&sid("note.france")).unwrap();
    france.fields.insert(
        sid("field.flag"),
        FieldValue::Scalar("<img src=\"flags/france.svg\" />".to_owned()),
    );
    france.fields.insert(
        sid("field.map"),
        FieldValue::Scalar("<img src=\"maps/france.svg\" />".to_owned()),
    );

    let typed_export = crowdanki::export_deck(typed).unwrap().deck_json;
    let raw_export = crowdanki::export_deck(&raw).unwrap().deck_json;
    assert_eq!(typed_export, raw_export);

    let json: serde_json::Value = serde_json::from_str(&typed_export).unwrap();
    let france = json["notes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|note| note["guid"] == "note.france")
        .unwrap();
    assert_eq!(
        france["fields"],
        serde_json::json!([
            "France",
            "<img src=\"flags/france.svg\" />",
            "<img src=\"flags/france.svg\" />",
            "<img src=\"maps/france.svg\" />"
        ])
    );
}

#[test]
fn localized_parameter_declarations_arguments_and_headers_are_strict() {
    let descriptor_cases = [
        (
            "localized image field",
            JOINED_DESCRIPTOR.replacen("type: scalar", "type: image", 1),
            "image field mapping field.front cannot use localized_by; media ID columns must remain unsuffixed",
        ),
        (
            "unknown parameter type",
            JOINED_DESCRIPTOR.replace("type: localized_column", "type: formula"),
            "unsupported type \"formula\"",
        ),
        (
            "nonempty default",
            JOINED_DESCRIPTOR.replace("default: ''", "default: de"),
            "default must be empty",
        ),
        (
            "unknown localized_by",
            JOINED_DESCRIPTOR.replace("localized_by: language", "localized_by: locale"),
            "unknown localized_by parameter \"locale\"",
        ),
        (
            "parameter declaration property",
            JOINED_DESCRIPTOR.replace("separator: ':'", "separator: ':'\n    normalize: lowercase"),
            "unknown field `normalize`",
        ),
        (
            "tags cannot be localized",
            JOINED_DESCRIPTOR.replace(
                "column: main.tags\n    delimiter",
                "column: main.tags\n    localized_by: language\n    delimiter",
            ),
            "unknown field `localized_by`",
        ),
    ];
    for (label, descriptor, expected) in descriptor_cases {
        let error = CsvNoteSourceDescriptor::parse(source("sources/joined.yaml", descriptor))
            .expect_err(label)
            .to_string();
        assert!(error.contains(expected), "{label}: {error}");
    }

    let unknown = joined_materialization(
        JOINED_DESCRIPTOR,
        JOINED_MAIN,
        JOINED_COUNTRY,
        JOINED_GUID,
        BTreeMap::from([("locale".to_owned(), "de".to_owned())]),
    )
    .expect_err("unknown argument fails");
    assert!(unknown.contains("unknown CSV source parameter argument \"locale\""));

    let literal_case = joined_materialization(
        JOINED_DESCRIPTOR,
        JOINED_MAIN,
        JOINED_COUNTRY,
        JOINED_GUID,
        BTreeMap::from([("language".to_owned(), "DE".to_owned())]),
    )
    .expect_err("literal values are not normalized or given fallback headers");
    assert!(
        literal_case.contains("mapped header \"capital:DE\" is absent"),
        "{literal_case}"
    );
}
