use brain_brew_core::{CanonicalDeck, FieldValue, StableId};
use brain_brew_formats::csv_note_source::{CsvSourceFile, CsvSourceRequestKind};
use brain_brew_formats::overlay_source_document::OverlaySourceDocument;
use brain_brew_formats::source_document::{SourceFile, SourceProvenance};
use brain_brew_formats::{canonical_yaml, crowdanki};

fn source(name: &str, text: impl Into<String>) -> SourceFile {
    SourceFile::new(SourceProvenance::new(name), text)
}

fn csv_source(name: &str, bytes: impl Into<Vec<u8>>) -> CsvSourceFile {
    CsvSourceFile::new(SourceProvenance::new(name), bytes)
}

fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

fn deck() -> CanonicalDeck {
    canonical_yaml::from_str(
        "deck:\n  id: deck.sparse\n  name: Sparse\n  description: ''\n  adapter_ids: {}\nnote_types:\n  note-type.country:\n    name: Country\n    field_order: [field.country-name]\n    fields:\n      field.country-name:\n        name: Name\n    card_template_order: []\n    card_templates: {}\n    styling: ''\n    adapter_ids: {}\nnotes:\n  note.france:\n    note_type_id: note-type.country\n    fields:\n      field.country-name: France\n    tags: []\n    adapter_ids: {}\n  note.germany:\n    note_type_id: note-type.country\n    fields:\n      field.country-name: Germany\n    tags: []\n    adapter_ids: {}\nmedia: {}\ntombstones: []\n",
    )
    .unwrap()
}

const DESCRIPTOR: &str = "version: 1
primary_table: main
tables:
  main:
    path: data/regions.csv
parameters: {}
joins: []
note:
  id: main.stable_id
  note_type_id: note-type.country
  fields:
    field.region-code:
      column: main.region_code
      type: scalar
  tags:
    column: main.tags
    delimiter: '|'
  adapter_ids: {}
";

fn overlay(inline: &str, exclude: &str) -> String {
    format!(
        "id: overlay.extension.experimental\nkind: extension\nfield_additions:\n  note-type.country:\n    fields:\n      field.region-code: Region code\n    values:\n      from_csv:\n        - descriptor: sources/descriptor.yaml\n          parameters: {{}}{exclude}\n{inline}"
    )
}

fn parse(input: &str, csv: &[u8]) -> Result<OverlaySourceDocument, String> {
    parse_with_descriptor(input, csv, DESCRIPTOR)
}

fn parse_with_descriptor(
    input: &str,
    csv: &[u8],
    descriptor: &str,
) -> Result<OverlaySourceDocument, String> {
    OverlaySourceDocument::parse_with_csv_sparse_fields(
        source("overlay.yaml", input),
        &deck(),
        |request| Err(format!("unexpected include {}", request.target())),
        |request| match request.kind() {
            CsvSourceRequestKind::Descriptor => {
                Ok(csv_source("sources/descriptor.yaml", descriptor))
            }
            CsvSourceRequestKind::Table { alias } if alias == "main" => {
                Ok(csv_source("sources/data/regions.csv", csv))
            }
            other => Err(format!("unexpected CSV request {other:?}")),
        },
    )
    .map_err(|error| error.to_string())
}

#[test]
fn sparse_non_empty_cells_materialize_with_provenance_and_format_preservation() {
    let input = overlay("", "");
    let document = parse(
        &input,
        b"stable_id,region_code,tags\nnote.france,WE,\nnote.germany,,\nnote.ignored,,\n",
    )
    .unwrap();
    let value = document.resolved_overlay().note_changes[&sid("note.france")].fields
        [&sid("field.region-code")]
        .value
        .as_ref()
        .unwrap();
    assert_eq!(value, &FieldValue::Scalar("WE".to_owned()));
    assert!(
        !document
            .resolved_overlay()
            .note_changes
            .contains_key(&sid("note.germany"))
    );

    let location = document
        .csv_sparse_field_provenance()
        .field(&sid("note.france"), &sid("field.region-code"))
        .unwrap();
    assert_eq!(
        location.declaration_path(),
        "field_additions.note-type.country.values.from_csv[0]"
    );
    assert_eq!(
        location.descriptor().unwrap().source_name(),
        "sources/descriptor.yaml"
    );
    assert_eq!(
        location.file().unwrap().source_name(),
        "sources/data/regions.csv"
    );
    assert_eq!(location.logical_row(), Some(2));
    assert_eq!(location.header(), Some("region_code"));
    assert_eq!(location.column(), Some(2));

    let composed = deck()
        .compose(&[document.resolved_overlay().clone()])
        .unwrap();
    assert_eq!(
        composed.notes[&sid("note.france")].fields[&sid("field.region-code")],
        FieldValue::Scalar("WE".to_owned())
    );
    assert_eq!(
        composed.notes[&sid("note.germany")].fields[&sid("field.region-code")],
        FieldValue::Scalar(String::new())
    );

    let emitted = document.emit().unwrap().root().text().to_owned();
    assert!(
        emitted.contains("values:\n      from_csv:\n        - descriptor: sources/descriptor.yaml")
    );
    assert!(!emitted.contains("note.france:"));
    let reparsed = parse(
        &emitted,
        b"stable_id,region_code,tags\nnote.france,WE,\nnote.germany,,\n",
    )
    .unwrap();
    assert_eq!(reparsed.resolved_overlay(), document.resolved_overlay());
}

#[test]
fn experimental_fixture_preserves_composition_and_crowdanki_output_after_transfer() {
    let base =
        canonical_yaml::from_str(include_str!("fixtures/csv_sparse_fields/base.yaml")).unwrap();
    let parse_fixture = |name: &str| {
        OverlaySourceDocument::parse_with_csv_sparse_fields(
            source(
                name,
                match name {
                    "before.yaml" => include_str!("fixtures/csv_sparse_fields/before.yaml"),
                    "after.yaml" => include_str!("fixtures/csv_sparse_fields/after.yaml"),
                    _ => unreachable!(),
                },
            ),
            &base,
            |request| Err(format!("unexpected include {}", request.target())),
            |request| match request.kind() {
                CsvSourceRequestKind::Descriptor => Ok(csv_source(
                    "descriptor.yaml",
                    include_bytes!("fixtures/csv_sparse_fields/descriptor.yaml"),
                )),
                CsvSourceRequestKind::Table { alias } if alias == "main" => Ok(csv_source(
                    "regions.csv",
                    include_bytes!("fixtures/csv_sparse_fields/regions.csv"),
                )),
                other => Err(format!("unexpected CSV request {other:?}")),
            },
        )
        .unwrap()
    };
    let before = parse_fixture("before.yaml");
    let after = parse_fixture("after.yaml");
    assert_eq!(
        before.emit().unwrap().root().text(),
        include_str!("fixtures/csv_sparse_fields/before.yaml")
    );
    assert_eq!(
        after.emit().unwrap().root().text(),
        include_str!("fixtures/csv_sparse_fields/after.yaml")
    );
    let before_deck = base.compose(&[before.resolved_overlay().clone()]).unwrap();
    let after_deck = base.compose(&[after.resolved_overlay().clone()]).unwrap();
    assert_eq!(after_deck, before_deck);
    assert_eq!(
        crowdanki::export_deck(&after_deck).unwrap().deck_json,
        crowdanki::export_deck(&before_deck).unwrap().deck_json
    );
    assert!(
        after
            .csv_sparse_field_provenance()
            .field(&sid("note.france"), &sid("field.region-code"))
            .is_none()
    );
    assert!(
        after
            .csv_sparse_field_provenance()
            .field(&sid("note.germany"), &sid("field.region-code"))
            .is_some()
    );
}

#[test]
fn sparse_csv_rejects_unknown_non_empty_notes_and_inline_collisions() {
    let unknown = parse(
        &overlay("", ""),
        b"stable_id,region_code,tags\nnote.unknown,XX,\n",
    )
    .unwrap_err();
    assert!(unknown.contains("unknown note note.unknown"), "{unknown}");

    let collision = parse(
        &overlay("      note.france:\n        field.region-code: WE\n", ""),
        b"stable_id,region_code,tags\nnote.france,WE,\n",
    )
    .unwrap_err();
    assert!(
        collision.contains("conflicts with inline ownership"),
        "{collision}"
    );
}

#[test]
fn sparse_csv_reuses_optional_joins_and_literal_localized_parameters() {
    let descriptor = "version: 1
primary_table: main
tables:
  main:
    path: main.csv
  info:
    path: info.csv
parameters:
  variant:
    type: localized_column
    default: ''
    separator: ':'
joins:
  - left: main.code
    right: info.code
    required: false
note:
  id: main.stable_id
  note_type_id: note-type.country
  fields:
    field.region-code:
      column: info.region
      localized_by: variant
      type: scalar
  tags:
    column: main.tags
    delimiter: '|'
  adapter_ids: {}
";
    let input = "id: overlay.extension.experimental
kind: extension
field_additions:
  note-type.country:
    fields:
      field.region-code: Region code
    values:
      from_csv:
        - descriptor: descriptor.yaml
          parameters:
            variant: exp
";
    let parse_joined = |descriptor: &str| {
        OverlaySourceDocument::parse_with_csv_sparse_fields(
            source("overlay.yaml", input),
            &deck(),
            |request| Err(format!("unexpected include {}", request.target())),
            |request| match request.kind() {
                CsvSourceRequestKind::Descriptor => Ok(csv_source("descriptor.yaml", descriptor)),
                CsvSourceRequestKind::Table { alias } if alias == "main" => Ok(csv_source(
                    "main.csv",
                    b"stable_id,code,tags\nnote.france,FR,\nnote.germany,DE,\n",
                )),
                CsvSourceRequestKind::Table { alias } if alias == "info" => Ok(csv_source(
                    "info.csv",
                    b"code,region,region:exp\nFR,West,WE\n",
                )),
                other => Err(format!("unexpected CSV request {other:?}")),
            },
        )
    };
    let document = parse_joined(descriptor).unwrap();
    assert_eq!(
        document.resolved_overlay().note_changes[&sid("note.france")].fields
            [&sid("field.region-code")]
            .value,
        Some(FieldValue::Scalar("WE".to_owned()))
    );
    assert!(
        document
            .csv_sparse_field_provenance()
            .field(&sid("note.germany"), &sid("field.region-code"))
            .is_none()
    );

    let required = descriptor.replace("required: false", "required: true");
    let error = parse_joined(&required).unwrap_err().to_string();
    assert!(error.contains("required join"), "{error}");
}

#[test]
fn sparse_csv_rejects_fields_not_added_here_and_invalid_typed_cells() {
    let undeclared = parse_with_descriptor(
        &overlay("", ""),
        b"stable_id,region_code,tags\nnote.france,WE,\n",
        &DESCRIPTOR.replace("field.region-code:", "field.country-name:"),
    )
    .unwrap_err();
    assert!(
        undeclared.contains("is not added by field_additions"),
        "{undeclared}"
    );

    let invalid_image = parse_with_descriptor(
        &overlay("", ""),
        b"stable_id,region_code,tags\nnote.france,not a stable id,\n",
        &DESCRIPTOR.replace("type: scalar", "type: image"),
    )
    .unwrap_err();
    assert!(
        invalid_image.contains("invalid stable id"),
        "{invalid_image}"
    );
}

#[test]
fn sparse_csv_exclusion_requires_inline_transfer() {
    let input = overlay(
        "      note.france:\n        field.region-code: WE\n",
        "\n          exclude:\n            note_ids:\n              - note.france",
    );
    let document = parse(
        &input,
        b"stable_id,region_code,tags\nnote.france,WE,\nnote.germany,CE,\n",
    )
    .unwrap();
    assert!(
        document
            .csv_sparse_field_provenance()
            .field(&sid("note.france"), &sid("field.region-code"))
            .is_none()
    );
    assert!(
        document
            .csv_sparse_field_provenance()
            .field(&sid("note.germany"), &sid("field.region-code"))
            .is_some()
    );

    let missing = parse(
        &overlay(
            "",
            "\n          exclude:\n            note_ids:\n              - note.france",
        ),
        b"stable_id,region_code,tags\nnote.france,WE,\n",
    )
    .unwrap_err();
    assert!(
        missing.contains("missing or conflicting inline ownership"),
        "{missing}"
    );

    let conflicting = parse(
        &overlay(
            "      note.france:\n        field.region-code: CE\n",
            "\n          exclude:\n            note_ids:\n              - note.france",
        ),
        b"stable_id,region_code,tags\nnote.france,WE,\n",
    )
    .unwrap_err();
    assert!(
        conflicting.contains("missing or conflicting inline ownership"),
        "{conflicting}"
    );
}

#[test]
fn sparse_csv_rejects_duplicate_ownership_across_declarations() {
    let input = overlay("", "").replace(
        "          parameters: {}",
        "          parameters: {}\n        - descriptor: sources/descriptor.yaml\n          parameters: {}",
    );
    let error = parse(&input, b"stable_id,region_code,tags\nnote.france,WE,\n").unwrap_err();
    assert!(error.contains("duplicate CSV ownership"), "{error}");
}
