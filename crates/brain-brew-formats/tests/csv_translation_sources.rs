use brain_brew_core::{FieldValue, StableId, TargetAdaptationIntent, TranslationCoverageCategory};
use brain_brew_formats::canonical_yaml;
use brain_brew_formats::crowdanki;
use brain_brew_formats::csv_note_source::{CsvSourceFile, CsvSourceRequestKind};
use brain_brew_formats::overlay_source_document::OverlaySourceDocument;
use brain_brew_formats::source_document::{SourceFile, SourceProvenance};

fn source(name: &str, text: impl Into<String>) -> SourceFile {
    SourceFile::new(SourceProvenance::new(name), text)
}

fn csv_source(name: &str, bytes: impl Into<Vec<u8>>) -> CsvSourceFile {
    CsvSourceFile::new(SourceProvenance::new(name), bytes)
}

fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

fn deck(rows: &[(&str, &str, &str)]) -> brain_brew_core::CanonicalDeck {
    let notes = rows
        .iter()
        .map(|(id, text, guid)| {
            format!(
                "  {id}:\n    note_type_id: note-type.basic\n    fields:\n      field.front: {text:?}\n    tags: []\n    adapter_ids:\n      crowdanki: {guid:?}\n"
            )
        })
        .collect::<String>();
    canonical_yaml::from_str(&format!(
        "deck:\n  id: deck.csv-translations\n  name: Fixture\n  description: ''\n  adapter_ids:\n    crowdanki:uuid: 11111111-1111-1111-1111-111111111111\nnote_types:\n  note-type.basic:\n    name: Basic\n    field_order:\n      - field.front\n    fields:\n      field.front:\n        name: Front\n    card_template_order: []\n    card_templates: {{}}\n    styling: ''\n    adapter_ids:\n      crowdanki:uuid: 22222222-2222-2222-2222-222222222222\nnotes:\n{notes}media: {{}}\ntombstones: []\n"
    ))
    .unwrap()
}

const DESCRIPTOR: &str = "version: 1
primary_table: main
tables:
  main:
    path: data/notes.csv
parameters:
  language:
    type: localized_column
    default: ''
    separator: ':'
joins: []
note:
  id: main.stable_id
  note_type_id: note-type.basic
  fields:
    field.front:
      column: main.front
      localized_by: language
      type: scalar
  tags:
    column: main.tags
    delimiter: '|'
  adapter_ids:
    crowdanki:
      column: main.guid
      localized_by: language
";

fn overlay_source(inline: &str) -> String {
    format!(
        "id: overlay.translation.de\nkind: translation\ntranslations:\n  from_csv:\n    - descriptor: sources/descriptor.yaml\n      parameters:\n        language: de\n      exclude:\n        source_texts: []\n        note_ids: []\n        paths: []\n{inline}"
    )
}

fn parse(
    source_deck: &brain_brew_core::CanonicalDeck,
    overlay: &str,
    csv: &[u8],
) -> Result<OverlaySourceDocument, String> {
    OverlaySourceDocument::parse_with_csv_translations(
        source("overlay.yaml", overlay),
        source_deck,
        |request| Err(format!("unexpected include {}", request.target())),
        |request| match request.kind() {
            CsvSourceRequestKind::Descriptor => {
                Ok(csv_source("sources/descriptor.yaml", DESCRIPTOR))
            }
            CsvSourceRequestKind::Table { alias } if alias == "main" => {
                Ok(csv_source("sources/data/notes.csv", csv))
            }
            other => Err(format!("unexpected CSV request {other:?}")),
        },
    )
    .map_err(|error| error.to_string())
}

#[test]
fn csv_pairs_materialize_into_existing_coverage_composition_and_export() {
    let source_deck = deck(&[
        ("note.one", "Hello", "guid-one"),
        ("note.two", "Hello", "guid-two"),
    ]);
    let csv = b"stable_id,front,front:de,tags,guid,guid:de\nnote.one,Hello,Hallo,,guid-one,guid-one-de\nnote.two,Hello,Hallo,,guid-two,guid-two-de\n";
    let input =
        overlay_source("  require_complete: true\n  no_change:\n    - Basic\n    - Fixture\n");
    let document = parse(&source_deck, &input, csv).expect("CSV translations materialize");
    let overlay = document.resolved_overlay();
    let translations = overlay.translations.as_ref().unwrap();

    assert_eq!(translations.direct["Hello"], "Hallo");
    assert_eq!(
        translations.adapter_ids["crowdanki"]["guid-one"],
        "guid-one-de"
    );
    let coverage = source_deck.translation_coverage(overlay).unwrap();
    assert!(!coverage.has_untranslated_fallbacks());
    assert_eq!(
        coverage
            .entries
            .iter()
            .filter(|entry| entry.category == TranslationCoverageCategory::DirectTranslation)
            .count(),
        2
    );

    let composed = source_deck.compose(std::slice::from_ref(overlay)).unwrap();
    assert_eq!(
        composed.notes[&sid("note.one")].fields[&sid("field.front")],
        FieldValue::Scalar("Hallo".to_owned())
    );
    assert_eq!(
        composed.notes[&sid("note.one")]
            .adapter_ids
            .get("crowdanki"),
        Some("guid-one-de")
    );
    let exported = crowdanki::export_deck(&composed).unwrap().deck_json;
    assert!(exported.contains("Hallo"));

    let emitted = document.emit().unwrap().root().text().to_owned();
    assert!(emitted.contains("from_csv:"));
    assert!(!emitted.contains("Hello: Hallo"));
    let inventory = OverlaySourceDocument::parse_with_csv_inventory(
        source("overlay.yaml", emitted.clone()),
        |request| Err(format!("unexpected include {}", request.target())),
        |request| match request.kind() {
            CsvSourceRequestKind::Descriptor => {
                Ok(csv_source("sources/descriptor.yaml", DESCRIPTOR))
            }
            CsvSourceRequestKind::Table { .. } => Ok(csv_source("sources/data/notes.csv", csv)),
        },
    )
    .unwrap();
    assert_eq!(inventory.emit().unwrap().root().text(), emitted);
}

#[test]
fn global_multi_target_equal_coexistence_and_uncovered_occurrences_are_contextual() {
    let source_deck = deck(&[
        ("note.one", "Same", "guid-one"),
        ("note.two", "Same", "guid-two"),
        ("note.outside", "Same", "guid-outside"),
    ]);
    let csv = b"stable_id,front,front:de,tags,guid,guid:de\nnote.one,Same,Gleich,,guid-one,guid-one\nnote.two,Same,Same,,guid-two,guid-two\n";
    let document = parse(&source_deck, &overlay_source(""), csv).unwrap();
    let translations = document.resolved_overlay().translations.as_ref().unwrap();

    assert!(translations.direct.is_empty());
    assert!(translations.no_change.is_empty());
    assert_eq!(
        translations.contextual["notes.note.one.fields.field.front"]["Same"],
        "Gleich"
    );
    assert_eq!(
        translations.contextual["notes.note.two.fields.field.front"]["Same"],
        "Same"
    );
}

#[test]
fn globally_covered_source_equal_pairs_materialize_no_change() {
    let source_deck = deck(&[("note.one", "Same", "guid-one")]);
    let csv =
        b"stable_id,front,front:de,tags,guid,guid:de\nnote.one,Same,Same,,guid-one,guid-one\n";
    let document = parse(&source_deck, &overlay_source(""), csv).unwrap();
    let translations = document.resolved_overlay().translations.as_ref().unwrap();
    assert_eq!(translations.no_change.iter().collect::<Vec<_>>(), ["Same"]);
    assert!(translations.direct.is_empty());
    assert!(translations.contextual.is_empty());
    assert!(translations.adapter_ids.is_empty());
}

#[test]
fn blank_pairs_materialize_adaptation_deletion_ignore_and_provenance() {
    let source_deck = deck(&[
        ("note.add", "", "guid-add"),
        ("note.delete", "Remove", "guid-delete"),
        ("note.empty", "", "guid-empty"),
    ]);
    let csv = b"stable_id,front,front:de,tags,guid,guid:de\nnote.add,,Added,,guid-add,guid-add\nnote.delete,Remove,,,guid-delete,guid-delete\nnote.empty,,,,guid-empty,guid-empty\n";
    let document = parse(&source_deck, &overlay_source(""), csv).unwrap();
    let translations = document.resolved_overlay().translations.as_ref().unwrap();

    let add = &translations.target_adaptations["notes.note.add.fields.field.front"];
    assert_eq!(add.intent, TargetAdaptationIntent::Adapt);
    assert_eq!(add.expected_source, "");
    assert_eq!(add.target, "Added");
    let delete = &translations.target_adaptations["notes.note.delete.fields.field.front"];
    assert_eq!(delete.intent, TargetAdaptationIntent::Delete);
    assert_eq!(delete.expected_source, "Remove");
    assert_eq!(delete.target, "");
    assert_eq!(translations.target_adaptations.len(), 2);

    let provenance = document
        .csv_translation_provenance()
        .adaptation("notes.note.delete.fields.field.front")
        .unwrap();
    assert_eq!(provenance.file().source_name(), "sources/data/notes.csv");
    assert_eq!(provenance.logical_row(), Some(3));
    assert_eq!(provenance.header(), "front:de");
    assert_eq!(provenance.column(), 3);
    assert_eq!(
        provenance.canonical_path(),
        "notes.note.delete.fields.field.front"
    );
}

#[test]
fn adapter_blank_policy_parameter_and_exclusion_boundaries_fail_explicitly() {
    let source_deck = deck(&[("note.one", "Hello", "guid-one")]);
    let csv = b"stable_id,front,front:de,tags,guid,guid:de\nnote.one,Hello,Hallo,,guid-one,\n";
    let error =
        parse(&source_deck, &overlay_source(""), csv).expect_err("one-sided adapter blank fails");
    assert!(error.contains("row 2:column 6 (guid:de)"), "{error}");
    assert!(error.contains("exactly one blank"), "{error}");

    let empty_language = overlay_source("").replace("language: de", "language: ''");
    let error =
        parse(&source_deck, &empty_language, csv).expect_err("source-to-itself pairing fails");
    assert!(error.contains("must be non-empty"), "{error}");

    let excluded = overlay_source("").replace("note_ids: []", "note_ids: [note.one]");
    let error = parse(&source_deck, &excluded, csv)
        .expect_err("adapter validation precedes exclusion behavior");
    assert!(error.contains("exactly one blank"), "{error}");

    let valid =
        b"stable_id,front,front:de,tags,guid,guid:de\nnote.one,Hello,Hallo,,guid-one,guid-one-de\n";
    let error = parse(&source_deck, &excluded, valid).expect_err("task 0080 boundary fails");
    assert!(error.contains("ownership-transfer task 0080"), "{error}");
}

#[test]
fn inline_identical_and_disjoint_entries_merge_but_conflicts_are_transactional() {
    let source_deck = deck(&[("note.one", "Hello", "guid-one")]);
    let csv =
        b"stable_id,front,front:de,tags,guid,guid:de\nnote.one,Hello,Hallo,,guid-one,guid-one-de\n";
    let identical = overlay_source(
        "  direct:\n    Hello: Hallo\n  variables:\n    label:\n      Label: Beschriftung\n",
    );
    let document = parse(&source_deck, &identical, csv).unwrap();
    let translations = document.resolved_overlay().translations.as_ref().unwrap();
    assert_eq!(translations.direct["Hello"], "Hallo");
    assert_eq!(translations.variables["label"]["Label"], "Beschriftung");

    let conflict = overlay_source("  direct:\n    Hello: Servus\n");
    let error = parse(&source_deck, &conflict, csv).expect_err("conflict fails atomically");
    assert!(error.contains("conflict"), "{error}");
}
