use brain_brew_core::{
    FieldValue, SourceTranslationImpact, StableId, TargetAdaptationIntent,
    TranslationCoverageCategory,
};
use brain_brew_formats::canonical_yaml;
use brain_brew_formats::crowdanki;
use brain_brew_formats::csv_note_source::{CsvSourceFile, CsvSourceRequestKind};
use brain_brew_formats::overlay_source_document::{OverlaySourceDocument, TranslationDecision};
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

const NO_EXCLUSIONS: &str = "source_texts: []\n        note_ids: []\n        paths: []";

fn overlay_source(inline: &str) -> String {
    overlay_source_with_exclusions(NO_EXCLUSIONS, inline)
}

fn overlay_source_with_exclusions(exclusions: &str, inline: &str) -> String {
    format!(
        "id: overlay.translation.de\nkind: translation\ntranslations:\n  from_csv:\n    - descriptor: sources/descriptor.yaml\n      parameters:\n        language: de\n      exclude:\n        {exclusions}\n{inline}"
    )
}

fn parse(
    source_deck: &brain_brew_core::CanonicalDeck,
    overlay: &str,
    csv: &[u8],
) -> Result<OverlaySourceDocument, String> {
    parse_with_descriptor(source_deck, overlay, DESCRIPTOR, csv)
}

fn parse_with_descriptor(
    source_deck: &brain_brew_core::CanonicalDeck,
    overlay: &str,
    descriptor: &str,
    csv: &[u8],
) -> Result<OverlaySourceDocument, String> {
    OverlaySourceDocument::parse_with_csv_translations(
        source("overlay.yaml", overlay),
        source_deck,
        |request| Err(format!("unexpected include {}", request.target())),
        |request| match request.kind() {
            CsvSourceRequestKind::Descriptor => {
                Ok(csv_source("sources/descriptor.yaml", descriptor))
            }
            CsvSourceRequestKind::Table { alias } if alias == "main" => {
                Ok(csv_source("sources/data/notes.csv", csv))
            }
            other => Err(format!("unexpected CSV request {other:?}")),
        },
    )
    .map_err(|error| error.to_string())
}

fn two_source_overlay(first_exclusions: &str, second_exclusions: &str, inline: &str) -> String {
    format!(
        "id: overlay.translation.de\nkind: translation\ntranslations:\n  from_csv:\n    - descriptor: sources/one/descriptor.yaml\n      parameters:\n        language: de\n      exclude:\n        {first_exclusions}\n    - descriptor: sources/two/descriptor.yaml\n      parameters:\n        language: de\n      exclude:\n        {second_exclusions}\n{inline}"
    )
}

fn parse_with_sources(
    source_deck: &brain_brew_core::CanonicalDeck,
    overlay: &str,
    sources: &[(&str, &[u8])],
) -> Result<OverlaySourceDocument, String> {
    OverlaySourceDocument::parse_with_csv_translations(
        source("overlay.yaml", overlay),
        source_deck,
        |request| Err(format!("unexpected include {}", request.target())),
        |request| match request.kind() {
            CsvSourceRequestKind::Descriptor => sources
                .iter()
                .find(|(descriptor, _)| *descriptor == request.target())
                .map(|(descriptor, _)| csv_source(descriptor, DESCRIPTOR))
                .ok_or_else(|| format!("unexpected descriptor {}", request.target())),
            CsvSourceRequestKind::Table { alias } if alias == "main" => {
                let descriptor = request.referring_source().source_name();
                let (_, csv) = sources
                    .iter()
                    .find(|(candidate, _)| *candidate == descriptor)
                    .ok_or_else(|| format!("unexpected table referrer {descriptor}"))?;
                let prefix = descriptor
                    .strip_suffix("descriptor.yaml")
                    .expect("test descriptor path suffix");
                let table = format!("{prefix}data/notes.csv");
                Ok(csv_source(&table, *csv))
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
fn multiple_csv_declarations_merge_disjoint_units_and_reject_overlapping_paths() {
    let source_deck = deck(&[
        ("note.one", "One", "guid-one"),
        ("note.two", "Two", "guid-two"),
    ]);
    let one = b"stable_id,front,front:de,tags,guid,guid:de\nnote.one,One,Eins,,guid-one,guid-one\n";
    let two = b"stable_id,front,front:de,tags,guid,guid:de\nnote.two,Two,Zwei,,guid-two,guid-two\n";
    let overlay = two_source_overlay(NO_EXCLUSIONS, NO_EXCLUSIONS, "");
    let document = parse_with_sources(
        &source_deck,
        &overlay,
        &[
            ("sources/one/descriptor.yaml", one),
            ("sources/two/descriptor.yaml", two),
        ],
    )
    .unwrap();
    let translations = document.resolved_overlay().translations.as_ref().unwrap();
    assert_eq!(translations.direct["One"], "Eins");
    assert_eq!(translations.direct["Two"], "Zwei");
    assert_eq!(
        document
            .csv_translation_provenance()
            .units()
            .map(|unit| unit.declaration())
            .collect::<Vec<_>>(),
        ["translations.from_csv[0]", "translations.from_csv[1]"]
    );

    let overlap = parse_with_sources(
        &deck(&[("note.one", "One", "guid-one")]),
        &overlay,
        &[
            ("sources/one/descriptor.yaml", one),
            ("sources/two/descriptor.yaml", one),
        ],
    )
    .expect_err("two CSV declarations cannot own the same occurrence");
    assert!(
        overlap.contains("CSV-owned path notes.note.one.fields.field.front is already owned by translations.from_csv[0]"),
        "{overlap}"
    );
    assert!(overlap.contains("translations.from_csv[1]"), "{overlap}");
}

#[test]
fn cross_declaration_transfer_rejects_csv_ownership_in_both_orders() {
    let source_deck = deck(&[("note.one", "Hello", "guid-one")]);
    let csv =
        b"stable_id,front,front:de,tags,guid,guid:de\nnote.one,Hello,Hallo,,guid-one,guid-one\n";
    let path_exclusion = "source_texts: []\n        note_ids: []\n        paths:\n          - notes.note.one.fields.field.front";
    let sources = [
        ("sources/one/descriptor.yaml", csv.as_slice()),
        ("sources/two/descriptor.yaml", csv.as_slice()),
    ];

    let transferred_first = two_source_overlay(
        path_exclusion,
        NO_EXCLUSIONS,
        "  direct:\n    Hello: Hallo\n",
    );
    let error = parse_with_sources(&source_deck, &transferred_first, &sources)
        .expect_err("later CSV ownership conflicts with an earlier transfer");
    assert!(
        error.contains("CSV-owned path notes.note.one.fields.field.front conflicts with native ownership transferred by translations.from_csv[0]"),
        "{error}"
    );
    assert!(error.contains("translations.from_csv[1]"), "{error}");

    let transferred_second = two_source_overlay(
        NO_EXCLUSIONS,
        path_exclusion,
        "  direct:\n    Hello: Hallo\n",
    );
    let error = parse_with_sources(&source_deck, &transferred_second, &sources)
        .expect_err("later transfer cannot displace earlier CSV ownership");
    assert!(
        error.contains("excluded path notes.note.one.fields.field.front is still CSV-owned by translations.from_csv[0]"),
        "{error}"
    );
    assert!(error.contains("translations.from_csv[1]"), "{error}");
}

#[test]
fn adapter_global_map_deduplicates_and_rejects_invalid_pairs() {
    let shared = deck(&[
        ("note.one", "One", "guid-shared"),
        ("note.two", "Two", "guid-shared"),
    ]);
    let equal = b"stable_id,front,front:de,tags,guid,guid:de\nnote.one,One,Eins,,guid-shared,guid-de\nnote.two,Two,Zwei,,guid-shared,guid-de\n";
    let document = parse(&shared, &overlay_source(""), equal).unwrap();
    let adapters = &document
        .resolved_overlay()
        .translations
        .as_ref()
        .unwrap()
        .adapter_ids["crowdanki"];
    assert_eq!(adapters.len(), 1);
    assert_eq!(adapters["guid-shared"], "guid-de");

    let conflicting = b"stable_id,front,front:de,tags,guid,guid:de\nnote.one,One,Eins,,guid-shared,guid-de\nnote.two,Two,Zwei,,guid-shared,guid-other\n";
    let error = parse(&shared, &overlay_source(""), conflicting)
        .expect_err("one adapter source cannot map to two targets");
    assert!(error.contains("row 3:column 6 (guid:de)"), "{error}");
    assert!(
        error.contains("conflicting adapter ID translations")
            && error.contains("guid-shared")
            && error.contains("guid-de")
            && error.contains("guid-other"),
        "{error}"
    );

    let empty = deck(&[("note.empty", "Hello", "")]);
    let both_blank = b"stable_id,front,front:de,tags,guid,guid:de\nnote.empty,Hello,Hallo,,,\n";
    let document = parse(&empty, &overlay_source(""), both_blank).unwrap();
    let translations = document.resolved_overlay().translations.as_ref().unwrap();
    assert_eq!(translations.direct["Hello"], "Hallo");
    assert!(translations.adapter_ids.is_empty());

    let mismatch =
        b"stable_id,front,front:de,tags,guid,guid:de\nnote.one,One,Eins,,guid-csv,guid-de\n";
    let error = parse(
        &deck(&[("note.one", "One", "guid-live")]),
        &overlay_source(""),
        mismatch,
    )
    .expect_err("CSV adapter source must match the resolved source deck");
    assert!(
        error.contains("CSV translation adapter ID source mismatch"),
        "{error}"
    );
    assert!(
        error.contains("notes.note.one.adapter_ids.crowdanki"),
        "{error}"
    );
    assert!(
        error.contains("guid-csv") && error.contains("guid-live"),
        "{error}"
    );
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
    let units = document
        .csv_translation_provenance()
        .units()
        .collect::<Vec<_>>();
    assert_eq!(units.len(), 2);
    assert!(
        units
            .iter()
            .all(|unit| unit.descriptor().source_name() == "sources/descriptor.yaml")
    );
    assert!(units.iter().any(|unit| {
        unit.category().as_str() == "adaptation"
            && unit.source().is_empty()
            && unit.target() == "Added"
            && unit.declaration() == "translations.from_csv[0]"
    }));
    assert!(units.iter().any(|unit| {
        unit.category().as_str() == "deletion"
            && unit.source() == "Remove"
            && unit.target().is_empty()
    }));
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
}

#[test]
fn maintained_transfer_fixture_preserves_deck_coverage_and_crowdanki_bytes() {
    let source_deck = deck(&[
        ("note.france", "Capital", "guid-france"),
        ("note.germany", "Capital", "guid-germany"),
    ]);
    let csv = include_bytes!("fixtures/csv_translation_transfer/notes.csv");
    let before = parse(
        &source_deck,
        include_str!("fixtures/csv_translation_transfer/before.yaml"),
        csv,
    )
    .unwrap();
    let after = parse(
        &source_deck,
        include_str!("fixtures/csv_translation_transfer/after.yaml"),
        csv,
    )
    .unwrap();
    assert_eq!(
        before.emit().unwrap().root().text(),
        include_str!("fixtures/csv_translation_transfer/before.yaml")
    );
    assert_eq!(
        after.emit().unwrap().root().text(),
        include_str!("fixtures/csv_translation_transfer/after.yaml")
    );
    let before_coverage = source_deck
        .translation_coverage(before.resolved_overlay())
        .unwrap();
    let after_coverage = source_deck
        .translation_coverage(after.resolved_overlay())
        .unwrap();
    assert!(!before_coverage.has_untranslated_fallbacks());
    assert!(!after_coverage.has_untranslated_fallbacks());
    assert_eq!(
        before_coverage
            .entries
            .iter()
            .map(|entry| (entry.path.clone(), entry.category))
            .collect::<Vec<_>>(),
        after_coverage
            .entries
            .iter()
            .map(|entry| (entry.path.clone(), entry.category))
            .collect::<Vec<_>>()
    );
    let before_deck = source_deck
        .compose(&[before.resolved_overlay().clone()])
        .unwrap();
    let after_deck = source_deck
        .compose(&[after.resolved_overlay().clone()])
        .unwrap();
    assert_eq!(after_deck, before_deck);
    assert_eq!(
        crowdanki::export_deck(&after_deck).unwrap().deck_json,
        crowdanki::export_deck(&before_deck).unwrap().deck_json
    );
    let remaining = after
        .csv_translation_provenance()
        .units()
        .collect::<Vec<_>>();
    assert_eq!(remaining.len(), 1);
    assert_eq!(
        remaining[0].canonical_path(),
        "notes.note.germany.adapter_ids.crowdanki"
    );
}

#[test]
fn path_transfer_prevents_global_leakage_and_preserves_composed_output() {
    let source_deck = deck(&[
        ("note.one", "Hello", "guid-one"),
        ("note.two", "Hello", "guid-two"),
    ]);
    let csv = b"stable_id,front,front:de,tags,guid,guid:de\nnote.one,Hello,Hallo,,guid-one,guid-one-de\nnote.two,Hello,Hallo,,guid-two,guid-two-de\n";
    let before = parse(&source_deck, &overlay_source(""), csv).unwrap();
    let transferred = parse(
        &source_deck,
        &overlay_source_with_exclusions(
            "source_texts: []\n        note_ids: []\n        paths:\n          - notes.note.one.fields.field.front",
            "  contextual:\n    notes.note.one.fields.field.front:\n      Hello: Hallo\n",
        ),
        csv,
    )
    .unwrap();
    let translations = transferred
        .resolved_overlay()
        .translations
        .as_ref()
        .unwrap();
    assert!(!translations.direct.contains_key("Hello"));
    assert!(!translations.no_change.contains("Hello"));
    assert_eq!(
        translations.contextual["notes.note.two.fields.field.front"]["Hello"],
        "Hallo"
    );
    assert_eq!(
        translations.contextual["notes.note.one.fields.field.front"]["Hello"],
        "Hallo"
    );

    let before_deck = source_deck
        .compose(&[before.resolved_overlay().clone()])
        .unwrap();
    let after_deck = source_deck
        .compose(&[transferred.resolved_overlay().clone()])
        .unwrap();
    assert_eq!(after_deck, before_deck);
    assert_eq!(
        crowdanki::export_deck(&after_deck).unwrap().deck_json,
        crowdanki::export_deck(&before_deck).unwrap().deck_json
    );
    let coverage = source_deck
        .translation_coverage(transferred.resolved_overlay())
        .unwrap();
    assert_eq!(
        coverage
            .entries
            .iter()
            .filter(|entry| entry.category == TranslationCoverageCategory::ContextualTranslation)
            .count(),
        2
    );
}

#[test]
fn path_transfer_contextualizes_remaining_csv_owned_no_change() {
    let source_deck = deck(&[
        ("note.one", "Same", "guid-one"),
        ("note.two", "Same", "guid-two"),
    ]);
    let csv = b"stable_id,front,front:de,tags,guid,guid:de\nnote.one,Same,Same,,guid-one,guid-one\nnote.two,Same,Same,,guid-two,guid-two\n";
    let document = parse(
        &source_deck,
        &overlay_source_with_exclusions(
            "source_texts: []\n        note_ids: []\n        paths:\n          - notes.note.one.fields.field.front",
            "  contextual:\n    notes.note.one.fields.field.front:\n      Same: Same\n",
        ),
        csv,
    )
    .unwrap();
    let translations = document.resolved_overlay().translations.as_ref().unwrap();
    assert!(!translations.no_change.contains("Same"));
    assert_eq!(
        translations.contextual["notes.note.two.fields.field.front"]["Same"],
        "Same"
    );
}

#[test]
fn moved_native_entry_becomes_stale_while_csv_pair_regenerates_current() {
    let source_deck = deck(&[("note.one", "Hello", "guid-one")]);
    let csv =
        b"stable_id,front,front:de,tags,guid,guid:de\nnote.one,Hello,Hallo,,guid-one,guid-one-de\n";
    let mut moved = parse(
        &source_deck,
        &overlay_source_with_exclusions(
            "source_texts: [Hello]\n        note_ids: []\n        paths: []",
            "  direct:\n    Hello: Hallo\n",
        ),
        csv,
    )
    .unwrap();
    moved
        .apply_source_translation_impact(
            "notes.note.one.fields.field.front",
            "Hello",
            "Hello updated",
            SourceTranslationImpact::MarkStale {
                target: "Hallo".to_owned(),
                context: None,
            },
        )
        .unwrap();
    let edited_deck = deck(&[("note.one", "Hello updated", "guid-one")]);
    let moved_coverage = edited_deck
        .translation_coverage(moved.resolved_overlay())
        .unwrap();
    assert!(moved_coverage.entries.iter().any(|entry| {
        entry.category == TranslationCoverageCategory::StaleTranslation
            && entry.path == "notes.note.one.fields.field.front"
    }));

    let updated_csv = b"stable_id,front,front:de,tags,guid,guid:de\nnote.one,Hello updated,Hallo,,guid-one,guid-one-de\n";
    let regenerated = parse(&edited_deck, &overlay_source(""), updated_csv).unwrap();
    let csv_coverage = edited_deck
        .translation_coverage(regenerated.resolved_overlay())
        .unwrap();
    assert!(csv_coverage.entries.iter().all(|entry| {
        entry.category != TranslationCoverageCategory::StaleTranslation
            && entry.category != TranslationCoverageCategory::StaleDirectKey
    }));
    assert_eq!(
        regenerated
            .resolved_overlay()
            .translations
            .as_ref()
            .unwrap()
            .direct["Hello updated"],
        "Hallo"
    );
}

#[test]
fn selector_union_transfers_text_adaptation_and_adapter_units() {
    let source_deck = deck(&[
        ("note.text", "Reusable", "guid-text"),
        ("note.delete", "Remove", "guid-delete"),
        ("note.adapter", "Keep", "guid-adapter"),
    ]);
    let csv = b"stable_id,front,front:de,tags,guid,guid:de\nnote.text,Reusable,Wiederverwendbar,,guid-text,guid-text-de\nnote.delete,Remove,,,guid-delete,guid-delete-de\nnote.adapter,Keep,Keep,,guid-adapter,guid-adapter-de\n";
    let input = overlay_source_with_exclusions(
        "source_texts:\n          - Reusable\n          - Remove\n        note_ids:\n          - note.delete\n        paths:\n          - notes.note.adapter.adapter_ids.crowdanki",
        "  direct:\n    Reusable: Wiederverwendbar\n  target_adaptations:\n    notes.note.delete.fields.field.front:\n      intent: delete\n      ownership: translation\n      expected_source: Remove\n      target: ''\n      reason: moved to native YAML\n  adapter_ids:\n    crowdanki:\n      guid-adapter: guid-adapter-de\n      guid-delete: guid-delete-de\n",
    );
    let document = parse(&source_deck, &input, csv).unwrap();
    let translations = document.resolved_overlay().translations.as_ref().unwrap();
    assert_eq!(translations.direct["Reusable"], "Wiederverwendbar");
    assert_eq!(
        translations.target_adaptations["notes.note.delete.fields.field.front"].reason,
        "moved to native YAML"
    );
    assert_eq!(
        translations.adapter_ids["crowdanki"]["guid-adapter"],
        "guid-adapter-de"
    );

    let units = document
        .csv_translation_provenance()
        .units()
        .collect::<Vec<_>>();
    assert!(
        units
            .iter()
            .all(|unit| unit.canonical_path() != "notes.note.text.fields.field.front")
    );
    assert!(
        units
            .iter()
            .all(|unit| unit.canonical_path() != "notes.note.delete.fields.field.front")
    );
    assert!(
        units
            .iter()
            .all(|unit| unit.canonical_path() != "notes.note.delete.adapter_ids.crowdanki")
    );
    assert!(
        units
            .iter()
            .all(|unit| unit.canonical_path() != "notes.note.adapter.adapter_ids.crowdanki")
    );
    assert!(
        units
            .iter()
            .any(|unit| unit.canonical_path() == "notes.note.text.adapter_ids.crowdanki")
    );
}

#[test]
fn invalid_duplicate_unmatched_and_incomplete_selectors_fail() {
    let source_deck = deck(&[("note.one", "Hello", "guid-one")]);
    let csv =
        b"stable_id,front,front:de,tags,guid,guid:de\nnote.one,Hello,Hallo,,guid-one,guid-one-de\n";
    for (exclusions, expected) in [
        (
            "source_texts: ['']\n        note_ids: []\n        paths: []",
            "must not be empty",
        ),
        (
            "source_texts: [Hello, Hello]\n        note_ids: []\n        paths: []",
            "duplicate",
        ),
        (
            "source_texts: []\n        note_ids: [note.one, note.one]\n        paths: []",
            "duplicate",
        ),
        (
            "source_texts: []\n        note_ids: []\n        paths: [notes.note.one.fields.field.front, notes.note.one.fields.field.front]",
            "duplicate",
        ),
        (
            "source_texts: []\n        note_ids: ['bad id']\n        paths: []",
            "invalid stable id",
        ),
        (
            "source_texts: []\n        note_ids: []\n        paths: [not-a-path]",
            "invalid canonical import occurrence path",
        ),
        (
            "source_texts: [Unknown]\n        note_ids: []\n        paths: []",
            "matched no otherwise-importable occurrence",
        ),
        (
            "source_texts: []\n        note_ids: [note.missing]\n        paths: []",
            "matched no otherwise-importable occurrence",
        ),
        (
            "source_texts: []\n        note_ids: []\n        paths: [notes.note.missing.fields.field.front]",
            "matched no otherwise-importable occurrence",
        ),
    ] {
        let input = overlay_source_with_exclusions(exclusions, "");
        let error = parse(&source_deck, &input, csv).expect_err(expected);
        assert!(error.contains(expected), "{error}");
    }

    let incomplete = overlay_source_with_exclusions(
        "source_texts: [Hello]\n        note_ids: []\n        paths: []",
        "",
    );
    let error = parse(&source_deck, &incomplete, csv).expect_err("missing inline transfer");
    assert!(error.contains("inline"), "{error}");

    let conflict = overlay_source_with_exclusions(
        "source_texts: [Hello]\n        note_ids: []\n        paths: []",
        "  direct:\n    Hello: Servus\n",
    );
    let error = parse(&source_deck, &conflict, csv).expect_err("conflicting transfer");
    assert!(error.contains("conflict"), "{error}");

    let incomplete_note = overlay_source_with_exclusions(
        "source_texts: []\n        note_ids: [note.one]\n        paths: []",
        "  direct:\n    Hello: Hallo\n",
    );
    let error = parse(&source_deck, &incomplete_note, csv)
        .expect_err("whole-note transfer must cover its adapter ID");
    assert!(
        error.contains("adapter-ID") && error.contains("inline"),
        "{error}"
    );
}

#[test]
fn selectors_emit_canonically_in_declared_source_order() {
    let source_deck = deck(&[("note.one", "Hello", "guid-one")]);
    let csv =
        b"stable_id,front,front:de,tags,guid,guid:de\nnote.one,Hello,Hallo,,guid-one,guid-one-de\n";
    let input = overlay_source_with_exclusions(
        "source_texts: [Hello]\n        note_ids: [note.one]\n        paths: [notes.note.one.fields.field.front]",
        "  direct:\n    Hello: Hallo\n  adapter_ids:\n    crowdanki:\n      guid-one: guid-one-de\n",
    );
    let document = parse(&source_deck, &input, csv).unwrap();
    let emitted = document.emit().unwrap().root().text().to_owned();
    assert!(emitted.contains("source_texts:\n          - Hello\n        note_ids:\n          - note.one\n        paths:\n          - notes.note.one.fields.field.front\n"), "{emitted}");
    let reparsed = parse(&source_deck, &emitted, csv).unwrap();
    assert_eq!(reparsed.emit().unwrap().root().text(), emitted);
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
    let csv_owner = document
        .csv_translation_provenance()
        .units()
        .find(|unit| unit.canonical_path() == "notes.note.one.fields.field.front")
        .expect("identical inline value remains CSV-owned");
    assert_eq!(csv_owner.category().as_str(), "direct");
    assert_eq!(csv_owner.source(), "Hello");
    assert_eq!(csv_owner.target(), "Hallo");

    let conflict = overlay_source("  direct:\n    Hello: Servus\n");
    let error = parse(&source_deck, &conflict, csv).expect_err("conflict fails atomically");
    assert!(error.contains("conflict"), "{error}");
}

#[test]
fn translation_descriptor_requires_localized_mapping_and_resolved_note_path() {
    let no_localized_mapping = "version: 1
primary_table: main
tables:
  main:
    path: data/notes.csv
parameters: {}
joins: []
note:
  id: main.stable_id
  note_type_id: note-type.basic
  fields:
    field.front:
      column: main.front
      type: scalar
  tags:
    column: main.tags
    delimiter: '|'
  adapter_ids: {}
";
    let source_deck = deck(&[("note.one", "Hello", "guid-one")]);
    let no_parameter_overlay = "id: overlay.translation.de
kind: translation
translations:
  from_csv:
    - descriptor: sources/descriptor.yaml
      parameters: {}
      exclude:
        source_texts: []
        note_ids: []
        paths: []
";
    let error = parse_with_descriptor(
        &source_deck,
        no_parameter_overlay,
        no_localized_mapping,
        b"stable_id,front,tags\nnote.one,Hello,\n",
    )
    .expect_err("translation descriptors need a localized mapping");
    assert_eq!(
        error,
        "overlay.yaml:translations.from_csv[0]: sources/descriptor.yaml: CSV translation descriptor has no localized scalar field or adapter-ID mapping"
    );

    let error = parse(
        &source_deck,
        &overlay_source(""),
        b"stable_id,front,front:de,tags,guid,guid:de\nnote.missing,Missing,Fehlt,,guid-missing,guid-missing-de\n",
    )
    .expect_err("translation CSV paths must exist in the source deck");
    assert_eq!(
        error,
        "overlay.yaml:translations.from_csv[0]: sources/descriptor.yaml: CSV translation path notes.note.missing.fields.field.front is absent or is not a scalar field in the resolved source deck"
    );
}

#[test]
fn csv_owned_translation_mutation_emits_but_cannot_reparse_as_valid_ownership() {
    let source_deck = deck(&[("note.one", "Hello", "guid-one")]);
    let csv =
        b"stable_id,front,front:de,tags,guid,guid:de\nnote.one,Hello,Hallo,,guid-one,guid-one-de\n";
    let original = parse(&source_deck, &overlay_source(""), csv).unwrap();
    let original_source = original.emit().unwrap().root().text().to_owned();
    let mut edited = original.clone();
    edited
        .set_translation_decision(
            "notes.note.one.fields.field.front",
            "Hello",
            TranslationDecision::Direct("Servus".to_owned()),
        )
        .unwrap();
    let emitted = edited.emit().unwrap().root().text().to_owned();
    assert!(emitted.contains("Hello: Servus"), "{emitted}");

    let error = parse(&source_deck, &emitted, csv)
        .expect_err("a conflicting inline mutation cannot take CSV ownership");
    assert!(
        error.contains("CSV translation conflict at notes.note.one.fields.field.front")
            && error.contains("inline and imported decisions differ"),
        "{error}"
    );
    assert_eq!(original.emit().unwrap().root().text(), original_source);
}
