use std::collections::{BTreeMap, BTreeSet};

use brain_brew_core::{
    AdapterIds, CanonicalDeck, CardTemplate, FieldDefinition, MediaReference, Note, NoteType,
    SemanticChangeKind, StableId, TombstoneAddress, Tombstones,
};
use brain_brew_formats::{canonical_yaml, crowdanki, source_includes};

#[test]
fn emits_canonical_deck_yaml_with_explicit_order_arrays() {
    let yaml = canonical_yaml::to_string(&ug_style_deck()).expect("deck emits");

    assert_eq!(yaml, EXPECTED_CANONICAL_YAML);
}

#[test]
fn list_message_pattern_and_items_round_trip_canonically() {
    let source = r#"deck:
  id: deck.pattern
  name: Pattern
  description: ''
  adapter_ids: {}
note_types:
  note-type.country:
    name: Country
    field_order:
      - field.country
      - field.flag-similarity
    fields:
      field.country:
        name: Country
      field.flag-similarity:
        name: Flag similarity
        message_pattern:
          kind: list
          item_format: '{country} ({description})'
          separator: ', '
          parameters:
            country:
              type: note_field_ref
              field: field.country
            description:
              type: text
    card_template_order: []
    card_templates: {}
    styling: ''
    adapter_ids: {}
notes:
  note.moldova:
    note_type_id: note-type.country
    fields:
      field.country: Moldova
      field.flag-similarity: ''
    tags: []
    adapter_ids: {}
  note.andorra:
    note_type_id: note-type.country
    fields:
      field.country: Andorra
      field.flag-similarity:
        - country: note.moldova
          description: wider, coat of arms with eagle
        - country:
            text: Sierra Leone
          description: slightly lighter blue
    tags: []
    adapter_ids: {}
media: {}
tombstones: []
"#;

    let deck = canonical_yaml::from_str(source).expect("pattern deck parses");
    deck.validate().expect("pattern deck validates");
    assert_eq!(
        deck.field_text(&sid("note.andorra"), &sid("field.flag-similarity"))
            .unwrap(),
        "Moldova (wider, coat of arms with eagle), Sierra Leone (slightly lighter blue)"
    );
    let formatted = canonical_yaml::to_string(&deck).expect("pattern deck emits");
    assert_eq!(canonical_yaml::format_str(&formatted).unwrap(), formatted);
    assert!(formatted.contains("message_pattern:\n          kind: list"));
    assert!(formatted.contains(
        "field.flag-similarity:\n        - country: note.moldova\n          description: wider, coat of arms with eagle"
    ));
    assert!(formatted.contains(
        "        - country:\n            text: Sierra Leone\n          description: slightly lighter blue"
    ));
    assert!(!formatted.contains("field.flag-similarity:\n        items:"));
    assert_eq!(canonical_yaml::from_str(&formatted).unwrap(), deck);
}

#[test]
fn legacy_items_wrapper_formats_to_the_canonical_direct_sequence() {
    let legacy = r#"deck:
  id: deck.pattern
  name: Pattern
  description: ''
  adapter_ids: {}
note_types:
  note-type.country:
    name: Country
    field_order:
      - field.country
      - field.flag-similarity
    fields:
      field.country:
        name: Country
      field.flag-similarity:
        name: Flag similarity
        message_pattern:
          kind: list
          item_format: '{country} ({description})'
          separator: ', '
          parameters:
            country:
              type: note_field_ref
              field: field.country
            description:
              type: text
    card_template_order: []
    card_templates: {}
    styling: ''
    adapter_ids: {}
notes:
  note.moldova:
    note_type_id: note-type.country
    fields:
      field.country: Moldova
      field.flag-similarity: ''
    tags: []
    adapter_ids: {}
  note.andorra:
    note_type_id: note-type.country
    fields:
      field.country: Andorra
      field.flag-similarity:
        items:
          - country: note.moldova
            description: wider
    tags: []
    adapter_ids: {}
media: {}
tombstones: []
"#;

    let formatted = canonical_yaml::format_str(legacy).expect("legacy wrapper migrates");
    assert!(formatted.contains(
        "field.flag-similarity:\n        - country: note.moldova\n          description: wider"
    ));
    assert!(!formatted.contains("field.flag-similarity:\n        items:"));
    assert_eq!(canonical_yaml::format_str(&formatted).unwrap(), formatted);
}

#[test]
fn legacy_flat_tombstone_migrates_only_from_one_unambiguous_top_level_identity() {
    let legacy = EXPECTED_CANONICAL_YAML.replace("tombstones: []", "tombstones:\n  - note.finland");
    let deck = canonical_yaml::from_str(&legacy).expect("unambiguous legacy note migrates");
    assert!(deck.tombstones.contains_address(&TombstoneAddress::Note {
        note_id: sid("note.finland"),
    }));

    let canonical = canonical_yaml::to_string(&deck).expect("migrated deck writes canonically");
    assert!(canonical.contains("tombstones:\n  - kind: note\n    path: notes.note.finland\n"));
    assert!(!canonical.contains("  - note.finland\n"));
    assert_eq!(canonical_yaml::format_str(&canonical).unwrap(), canonical);
}

#[test]
fn legacy_flat_tombstone_rejects_unknown_nested_and_cross_kind_ambiguous_ids() {
    for (legacy_id, expected) in [
        ("entity.missing", "matches no retained top-level"),
        (
            "field.capital",
            "nested field/template ownership is never inferred",
        ),
    ] {
        let source = EXPECTED_CANONICAL_YAML
            .replace("tombstones: []", &format!("tombstones:\n  - {legacy_id}"));
        let error = canonical_yaml::from_str(&source).expect_err("legacy ID must fail closed");
        assert!(error.to_string().contains(expected), "{error}");
        assert!(error.to_string().contains("kind"), "{error}");
        assert!(error.to_string().contains("path"), "{error}");
    }

    let ambiguous = EXPECTED_CANONICAL_YAML
        .replace("note-type.country", "note.finland")
        .replace("tombstones: []", "tombstones:\n  - note.finland");
    let error =
        canonical_yaml::from_str(&ambiguous).expect_err("cross-kind legacy ID is ambiguous");
    assert!(
        error.to_string().contains("multiple top-level kinds"),
        "{error}"
    );
}

#[test]
fn typed_tombstone_provenance_round_trips_and_duplicate_addresses_fail() {
    let source = EXPECTED_CANONICAL_YAML.replace(
        "tombstones: []",
        "tombstones:\n  - kind: note\n    path: notes.note.finland\n    removed_by: overlay.patch.remove-finland\n    operation: remove",
    );
    let deck = canonical_yaml::from_str(&source).expect("typed tombstone parses");
    let record = deck
        .tombstones
        .get(&TombstoneAddress::Note {
            note_id: sid("note.finland"),
        })
        .expect("typed note record");
    assert_eq!(
        record.provenance.as_ref().unwrap().overlay_id,
        sid("overlay.patch.remove-finland")
    );
    let canonical = canonical_yaml::to_string(&deck).expect("typed provenance emits");
    assert_eq!(canonical_yaml::from_str(&canonical).unwrap(), deck);

    let duplicate = canonical.replace(
        "tombstones:\n",
        "tombstones:\n  - kind: note\n    path: notes.note.finland\n",
    );
    let error = canonical_yaml::from_str(&duplicate).expect_err("duplicate typed address rejects");
    assert!(
        error
            .to_string()
            .contains("duplicate typed tombstone address"),
        "{error}"
    );
}

#[test]
fn typed_tombstone_kind_must_match_its_full_path() {
    let source = EXPECTED_CANONICAL_YAML.replace(
        "tombstones: []",
        "tombstones:\n  - kind: media_reference\n    path: notes.note.finland",
    );
    let error = canonical_yaml::from_str(&source).expect_err("kind/path mismatch rejects");
    assert!(
        error.to_string().contains("does not match typed path kind"),
        "{error}"
    );
}

#[test]
fn emits_empty_notes_as_parseable_inline_map() {
    let mut deck = ug_style_deck();
    deck.notes.clear();

    let yaml = canonical_yaml::to_string(&deck).expect("deck emits");

    assert!(yaml.contains("notes: {}\n"));
    canonical_yaml::from_str(&yaml).expect("emitted empty-notes deck parses");
}

#[test]
fn parses_emitted_yaml_back_to_semantically_equal_deck() {
    let original = ug_style_deck();
    let yaml = canonical_yaml::to_string(&original).expect("deck emits");

    let parsed = canonical_yaml::from_str(&yaml).expect("emitted yaml parses");

    assert_eq!(parsed, original);
    assert!(parsed.semantic_diff(&original).is_empty());
}

#[test]
fn semantic_diff_ignores_yaml_ordering_and_reports_content_changes() {
    let messy_yaml = r#"deck:
  description: A geography deck fixture.
  id: deck.ultimate-geography
  name: Ultimate Geography
  adapter_ids: {}
note_types:
  note-type.country:
    adapter_ids:
      crowdanki:model_id: '1548959259107'
    name: Ultimate Geography Country
    styling: |
      .card { font-family: sans-serif; }
    field_order: [field.country, field.capital, field.flag]
    fields:
      field.flag: { name: Flag }
      field.capital: { name: Capital }
      field.country: { name: Country }
    card_template_order:
      - template.country-to-capital
    card_templates:
      template.country-to-capital:
        adapter_ids: {}
        name: Country - Capital
        question_format: '{{Country}}'
        answer_format: '{{FrontSide}}<hr id=answer>{{Capital}}'
notes:
  note.finland:
    adapter_ids:
      crowdanki:guid: ug-finland-guid
    fields:
      field.flag: '<img src="fi.png">'
      field.capital: Helsinki
      field.country: Finland
    note_type_id: note-type.country
    tags: [Europe, Nordic]
media:
  media.flag.finland:
    path: flags/fi.png
    sha256: 0123456789abcdef
tombstones: []
"#;

    let formatted = canonical_yaml::format_str(messy_yaml).expect("valid yaml formats");

    assert_eq!(formatted, EXPECTED_CANONICAL_YAML);

    let canonical = canonical_yaml::from_str(EXPECTED_CANONICAL_YAML).expect("canonical parses");
    let reordered = canonical_yaml::from_str(messy_yaml).expect("reordered yaml parses");
    assert!(
        canonical.semantic_diff(&reordered).is_empty(),
        "semantic comparison should ignore YAML key ordering and formatting noise"
    );

    let changed_yaml = messy_yaml.replace("field.capital: Helsinki", "field.capital: Helsingfors");
    let changed = canonical_yaml::from_str(&changed_yaml).expect("changed yaml parses");
    let diff = canonical.semantic_diff(&changed);
    assert_eq!(diff.changes.len(), 1);
    let change = &diff.changes[0];
    assert_eq!(change.kind, SemanticChangeKind::Modified);
    assert_eq!(change.path, "notes.note.finland.fields.field.capital");
    assert_eq!(change.before.as_deref(), Some("Helsinki"));
    assert_eq!(change.after.as_deref(), Some("Helsingfors"));
}

#[test]
fn rejects_duplicate_note_field_keys_with_schema_path() {
    let yaml = EXPECTED_CANONICAL_YAML.replace(
        "      field.capital: Helsinki\n",
        "      field.capital: Helsinki\n      field.capital: Helsingfors\n",
    );

    for error in [
        canonical_yaml::from_str(&yaml).expect_err("duplicate note field is rejected"),
        canonical_yaml::format_str(&yaml).expect_err("formatter rejects duplicate note field"),
    ] {
        let message = error.to_string();
        assert!(
            message.contains("duplicate key \"field.capital\""),
            "{message}"
        );
        assert!(
            message.contains("notes.note.finland.fields.field.capital"),
            "{message}"
        );
        assert!(message.contains("line "), "{message}");
    }
}

#[test]
fn parses_formats_and_resolves_structured_message_fields() {
    let yaml = r#"deck:
  id: deck.structured-message
  name: Structured Message Fixture
  description: Structured message translation fixture.
  adapter_ids: {}
note_types:
  note-type.country:
    name: Country
    field_order:
      - field.country
      - field.flag-similarity
    fields:
      field.country:
        name: Country
      field.flag-similarity:
        name: Flag similarity
    card_template_order:
      - template.flag-country
    card_templates:
      template.flag-country:
        name: Flag - Country
        question_format: '{{Country}}'
        answer_format: '{{Flag similarity}}'
        adapter_ids: {}
    styling: ''
    adapter_ids: {}
notes:
  note.finland:
    note_type_id: note-type.country
    fields:
      field.country: Finland
      field.flag-similarity:
        message:
          - ref: notes.note.iceland.fields.field.country
          - literal: ' ('
          - text: blue background with a white cross
          - literal: ')'
    tags: []
    adapter_ids: {}
  note.iceland:
    note_type_id: note-type.country
    fields:
      field.country: Iceland
      field.flag-similarity: ''
    tags: []
    adapter_ids: {}
media: {}
tombstones: []
"#;

    let deck = canonical_yaml::from_str(yaml).expect("structured message yaml parses");

    assert_eq!(
        deck.field_text(&sid("note.finland"), &sid("field.flag-similarity"))
            .expect("structured message resolves"),
        "Iceland (blue background with a white cross)"
    );
    assert_eq!(
        deck.notes[&sid("note.finland")].fields[&sid("field.flag-similarity")]
            .as_message()
            .unwrap()
            .components
            .len(),
        4
    );

    let formatted = canonical_yaml::to_string(&deck).expect("structured message emits");
    assert!(formatted.contains("field.flag-similarity:\n        message:"));
    assert!(formatted.contains("          - ref: notes.note.iceland.fields.field.country"));
    assert!(formatted.contains("          - text: blue background with a white cross"));
}

#[test]
fn parses_formats_and_resolves_formatted_structured_message_fields() {
    let yaml = r#"deck:
  id: deck.message-format
  name: Message Format
  description: ''
  adapter_ids: {}
note_types:
  note-type.country:
    name: Country
    field_order:
      - field.country
      - field.flag-similarity
    fields:
      field.country:
        name: Country
      field.flag-similarity:
        name: Flag similarity
    card_template_order: []
    card_templates: {}
    styling: ''
    adapter_ids: {}
notes:
  note.finland:
    note_type_id: note-type.country
    fields:
      field.country: Finland
      field.flag-similarity:
        format: '{country} ({description})'
        variables:
          country:
            ref: notes.note.iceland.fields.field.country
          description:
            text: blue background with a white cross
    tags: []
    adapter_ids: {}
  note.iceland:
    note_type_id: note-type.country
    fields:
      field.country: Iceland
      field.flag-similarity: ''
    tags: []
    adapter_ids: {}
media: {}
tombstones: []
"#;

    let deck = canonical_yaml::from_str(yaml).expect("formatted structured message yaml parses");

    assert_eq!(
        deck.field_text(&sid("note.finland"), &sid("field.flag-similarity"))
            .expect("structured message resolves"),
        "Iceland (blue background with a white cross)"
    );
    let message = deck.notes[&sid("note.finland")].fields[&sid("field.flag-similarity")]
        .as_message()
        .unwrap();
    assert_eq!(message.format.as_deref(), Some("{country} ({description})"));
    assert!(message.variables.contains_key("description"));

    let formatted = canonical_yaml::to_string(&deck).expect("formatted message emits");
    assert!(
        formatted.contains("field.flag-similarity:\n        format: '{country} ({description})'")
    );
    assert!(
        formatted.contains(
            "          country:\n            ref: notes.note.iceland.fields.field.country"
        )
    );
    assert!(
        formatted.contains(
            "          description:\n            text: blue background with a white cross"
        )
    );
}

#[test]
fn translation_dictionary_overlay_translates_variables_fields_and_adapter_ids() {
    let deck = canonical_yaml::from_str(
        r#"deck:
  id: deck.translation-fixture
  name: Ultimate Geography
  description: A geography deck fixture.
  variables:
    label.deck: Ultimate Geography
  adapter_ids:
    crowdanki:uuid: deck-en
note_types:
  note-type.country:
    name: Ultimate Geography
    variables:
      label.capital: Capital
    field_order:
      - field.country
      - field.capital
    fields:
      field.capital:
        name: Capital
      field.country:
        name: Country
    card_template_order:
      - template.country-capital
    card_templates:
      template.country-capital:
        name: Country - Capital
        question_format: '<div class="type">${label.capital}</div>{{Country}}'
        answer_format: '<div>${label.capital}: {{Capital}}</div>'
        adapter_ids: {}
    styling: ''
    adapter_ids:
      crowdanki:uuid: model-en
notes:
  note.denmark:
    note_type_id: note-type.country
    fields:
      field.capital: Copenhagen
      field.country: Denmark
    tags: []
    adapter_ids:
      crowdanki:guid: note-guid-en
media: {}
tombstones: []
"#,
    )
    .expect("deck parses");
    let overlay = canonical_yaml::overlay_from_str(
        r#"id: overlay.translation.da
kind: translation
translations:
  direct:
    Copenhagen: København
    Denmark: Danmark
    Ultimate Geography: 'Ultimate Geography [DA]'
  variables:
    label.capital:
      Capital: Hovedstad
  adapter_ids:
    crowdanki:guid:
      note-guid-en: note-guid-da
    crowdanki:uuid:
      deck-en: deck-da
      model-en: model-da
"#,
    )
    .expect("translation overlay parses");

    let translated = deck.compose(&[overlay]).expect("translation composes");
    assert_eq!(translated.name, "Ultimate Geography [DA]");
    let note_type = &translated.note_types[&sid("note-type.country")];
    assert_eq!(note_type.variables["label.capital"], "Hovedstad");
    assert_eq!(
        note_type.adapter_ids.get("crowdanki:uuid"),
        Some("model-da")
    );
    let note = &translated.notes[&sid("note.denmark")];
    assert_eq!(note.fields[&sid("field.country")], "Danmark");
    assert_eq!(note.fields[&sid("field.capital")], "København");
    assert_eq!(note.adapter_ids.get("crowdanki:guid"), Some("note-guid-da"));

    let exported = crowdanki::export_deck(&translated).expect("translated deck exports");
    let json: serde_json::Value = serde_json::from_str(&exported.deck_json).unwrap();
    assert_eq!(json["crowdanki_uuid"], "deck-da");
    assert_eq!(
        json["note_models"][0]["tmpls"][0]["qfmt"],
        "<div class=\"type\">Hovedstad</div>{{Country}}"
    );
    assert_eq!(json["notes"][0]["guid"], "note-guid-da");
}

#[test]
fn translation_dictionary_parses_and_formats_stale_translations() {
    let formatted = canonical_yaml::overlay_format_str(
        r#"id: overlay.translation.da
kind: translation
stale_translations:
  - old_source: Helsinki
    new_source: Helsinki City
    target: Helsingfors
  - old_source: Capital
    new_source: Capital city
    target: Hovedstad
    context: notes.note.finland
"#,
    )
    .expect("stale translations parse and format");

    assert!(formatted.contains("stale_translations:\n"));
    assert!(formatted.contains("  - old_source: Helsinki\n"));
    assert!(formatted.contains("    new_source: Helsinki City\n"));
    assert!(formatted.contains("    target: Helsingfors\n"));
    assert!(formatted.contains("    context: notes.note.finland\n"));

    let overlay = canonical_yaml::overlay_from_str(&formatted).expect("formatted overlay parses");
    let translations = overlay.translations.expect("translation dictionary");
    assert_eq!(translations.stale_translations.len(), 2);
    assert_eq!(translations.stale_translations[0].old_source, "Helsinki");
    assert_eq!(
        translations.stale_translations[0].new_source,
        "Helsinki City"
    );
    assert_eq!(translations.stale_translations[0].target, "Helsingfors");
    assert_eq!(
        translations.stale_translations[1].context.as_deref(),
        Some("notes.note.finland")
    );
}

#[test]
fn translation_dictionary_rejects_alpha_stale_record_key() {
    let error = canonical_yaml::overlay_from_str(
        r#"id: overlay.translation.da
kind: translation
translations:
  stale_records:
    - old_source: Old source
      new_source: New source
      target: Gammel oversættelse
"#,
    )
    .expect_err("alpha stale_records key is not accepted");
    assert!(error.to_string().contains("stale_records"));
}

#[test]
fn translation_dictionary_rejects_unknown_stale_translation_fields() {
    let error = canonical_yaml::overlay_from_str(
        r#"id: overlay.translation.da
kind: translation
stale_translations:
  - old_source: Helsinki
    new_source: Helsinki City
    target: Helsingfors
    unexpected: nope
"#,
    )
    .expect_err("unknown stale translation fields are rejected");

    assert!(error.to_string().contains("unexpected"));
}

#[test]
fn translation_dictionary_parses_no_change_entries() {
    let overlay = canonical_yaml::overlay_from_str(
        r#"id: overlay.translation.da
kind: translation
translations:
  no_change:
    - Andorra
    - Sukhumi
"#,
    )
    .expect("no-change sections parse");

    let translations = overlay.translations.expect("translation dictionary");
    assert!(translations.no_change.contains("Andorra"));
    assert!(translations.no_change.contains("Sukhumi"));
}

#[test]
fn translation_dictionary_parses_nested_contextual_translations() {
    let overlay = canonical_yaml::overlay_from_str(
        r#"id: overlay.translation.da
kind: translation
translations:
  contextual:
    notes.note:
      denmark:
        Shared source: Dansk kontekst
      denmark.fields.field.country:
        Country: Land
    deck.description:
      Shared source: Dæk-kontekst
target_adaptations:
  notes.note.denmark.fields.field.country-info:
    expected_source: ''
    target: Ekstra tekst.
"#,
    )
    .expect("contextual translation sections parse");

    let translations = overlay.translations.expect("translation dictionary");
    assert_eq!(
        translations.contextual["notes.note.denmark"]["Shared source"],
        "Dansk kontekst"
    );
    assert_eq!(
        translations.contextual["notes.note.denmark.fields.field.country"]["Country"],
        "Land"
    );
    assert_eq!(
        translations.contextual["deck.description"]["Shared source"],
        "Dæk-kontekst"
    );
    assert_eq!(
        translations.target_adaptations["notes.note.denmark.fields.field.country-info"].target,
        "Ekstra tekst."
    );
    assert_eq!(
        translations.target_adaptations["notes.note.denmark.fields.field.country-info"]
            .expected_source,
        ""
    );
}

#[test]
fn translation_dictionary_rejects_legacy_changes_and_additions() {
    let error = canonical_yaml::overlay_from_str(
        r#"id: overlay.translation.da
kind: translation
translations:
  changes:
    Copenhagen: København
"#,
    )
    .expect_err("legacy changes key is not accepted");

    assert!(error.to_string().contains("changes"));

    let error = canonical_yaml::overlay_from_str(
        r#"id: overlay.translation.da
kind: translation
translations:
  additions:
    notes.note.denmark.fields.field.country-info: Ekstra tekst.
"#,
    )
    .expect_err("legacy additions key is not accepted");

    assert!(error.to_string().contains("additions"));
}

#[test]
fn translation_dictionary_rejects_legacy_text_key() {
    let error = canonical_yaml::overlay_from_str(
        r#"id: overlay.translation.da
kind: translation
translations:
  text:
    Denmark: Danmark
"#,
    )
    .expect_err("legacy text key is not accepted");

    assert!(error.to_string().contains("text"));
}

#[test]
fn parses_emits_and_formats_structured_image_fields() {
    let yaml = image_deck_yaml("field.flag: !image media.flag.finland");

    let deck = canonical_yaml::from_str(&yaml).expect("image field yaml parses");
    let note = &deck.notes[&sid("note.finland")];
    assert_eq!(
        note.fields[&sid("field.flag")].as_images().unwrap()[0].media_id,
        sid("media.flag.finland")
    );

    let emitted = canonical_yaml::to_string(&deck).expect("image field emits");
    assert!(emitted.contains("field.flag: !image media.flag.finland\n"));
    let reparsed = canonical_yaml::from_str(&emitted).expect("emitted image yaml parses");
    assert_eq!(reparsed, deck);

    let once = canonical_yaml::format_str(&yaml).expect("image field formats once");
    let twice = canonical_yaml::format_str(&once).expect("image field formats twice");
    assert_eq!(twice, once);
}

#[test]
fn image_field_sequence_canonicalizes_singletons_and_emits_block_sequences() {
    let singleton = canonical_yaml::format_str(&image_deck_yaml(
        "field.flag:\n        - !image media.flag.finland",
    ))
    .expect("singleton image sequence formats");
    assert!(singleton.contains("field.flag: !image media.flag.finland\n"));

    let sequence = canonical_yaml::format_str(&image_deck_yaml(
        "field.flag:\n        - !image media.flag.finland\n        - !image media.flag.finland.blur",
    ))
    .expect("image sequence formats");
    assert!(sequence.contains(
        "field.flag:\n        - !image media.flag.finland\n        - !image media.flag.finland.blur\n"
    ));
    let parsed = canonical_yaml::from_str(&sequence).expect("image sequence parses");
    assert_eq!(
        parsed.notes[&sid("note.finland")].fields[&sid("field.flag")]
            .as_images()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(canonical_yaml::format_str(&sequence).unwrap(), sequence);
}

#[test]
fn include_resolver_passes_image_tags_through_when_includes_are_present() {
    let dir = temp_fixture_dir("brain-brew-image-include");
    std::fs::write(dir.join("capital.txt"), "Helsinki").expect("write include");
    let source = image_deck_yaml("field.flag: !image media.flag.finland")
        .replace("description: ''", "description: !include capital.txt");

    let resolved =
        source_includes::resolve_file_includes(&source, &dir.join("deck.yaml"), &dir, &[])
            .expect("include resolver accepts !image alongside !include");

    assert!(resolved.contains("!image media.flag.finland"));
    let formatted =
        source_includes::format_preserving_file_includes(&source, canonical_yaml::format_str)
            .expect("include-preserving format accepts !image alongside !include");
    assert!(formatted.contains("field.flag: !image media.flag.finland\n"));
    let deck = canonical_yaml::from_str(&resolved).expect("resolved deck parses");
    assert!(
        deck.notes[&sid("note.finland")].fields[&sid("field.flag")]
            .as_images()
            .is_some()
    );
}

#[test]
fn top_level_media_include_resolves_to_the_same_deck_as_inline_media() {
    let dir = temp_fixture_dir("brain-brew-media-include-resolve");
    let inline = image_deck_yaml("field.flag: !image media.flag.finland");
    std::fs::write(dir.join("media.yaml"), image_media_map_yaml()).expect("write media include");
    let source = deck_with_media_include(&inline, "media.yaml");

    let resolved =
        source_includes::resolve_file_includes(&source, &dir.join("deck.yaml"), &dir, &[])
            .expect("media include resolves");

    let resolved_deck = canonical_yaml::from_str(&resolved).expect("resolved deck parses");
    let inline_deck = canonical_yaml::from_str(&inline).expect("inline deck parses");
    assert_eq!(resolved_deck, inline_deck);
}

#[test]
fn top_level_media_include_rejects_duplicate_ids_before_materializing() {
    let dir = temp_fixture_dir("brain-brew-media-include-duplicate");
    let media = image_media_map_yaml();
    std::fs::write(dir.join("media.yaml"), format!("{media}{media}"))
        .expect("write duplicate media include");
    let inline = image_deck_yaml("field.flag: !image media.flag.finland");
    let source = deck_with_media_include(&inline, "media.yaml");

    let error = source_includes::resolve_file_includes(&source, &dir.join("deck.yaml"), &dir, &[])
        .expect_err("duplicate included media ID is rejected");
    let message = error.to_string();

    assert!(message.contains("media.yaml"), "{message}");
    assert!(
        message.contains("duplicate key \"media.flag.finland\""),
        "{message}"
    );
    assert!(message.contains("media.flag.finland"), "{message}");
}

#[test]
fn format_preserving_file_includes_round_trips_media_and_scalar_includes() {
    let inline = image_deck_yaml("field.flag: !image media.flag.finland");
    let source = deck_with_media_include(&inline, "media.yaml")
        .replace("description: ''", "description: !include description.html");

    // This exercises the ADR-0016 §7 strip-and-restore path's load-bearing
    // assumption: canonical_yaml::to_string validates !image stable-ID syntax,
    // but does not require referenced media IDs to exist while media is stripped.
    // If validation grows that existence check, media includes must be spliced for parse.
    let formatted =
        source_includes::format_preserving_file_includes(&source, canonical_yaml::format_str)
            .expect("media include formats without materializing files");

    assert!(formatted.contains("description: !include description.html\n"));
    assert!(formatted.contains("media: !include media.yaml\n"));
    assert_eq!(
        source_includes::format_preserving_file_includes(&formatted, canonical_yaml::format_str)
            .expect("formatted media include formats idempotently"),
        formatted
    );
}

#[test]
fn include_under_media_entries_still_rejects_non_whitelisted_structural_positions() {
    let dir = temp_fixture_dir("brain-brew-media-include-reject-entry");
    std::fs::write(dir.join("path.txt"), "flags/fi.png").expect("write scalar include");
    let source = image_deck_yaml("field.flag: '<img src=\"fi.png\">'")
        .replace("path: flags/fi.png", "path: !include path.txt");

    let error = source_includes::resolve_file_includes(&source, &dir.join("deck.yaml"), &dir, &[])
        .expect_err("media entry includes are not whitelisted");

    assert!(
        error
            .to_string()
            .contains("is only valid for scalar content fields"),
        "unexpected error: {error}"
    );
}

#[test]
fn media_include_rejects_non_mapping_file_root_with_deck_and_include_paths() {
    let dir = temp_fixture_dir("brain-brew-media-include-reject-root");
    std::fs::write(dir.join("media.yaml"), "- not-a-map\n").expect("write bad media include");
    let inline = image_deck_yaml("field.flag: '<img src=\"fi.png\">'");
    let source = deck_with_media_include(&inline, "media.yaml");

    let error = source_includes::resolve_file_includes(&source, &dir.join("deck.yaml"), &dir, &[])
        .expect_err("media include root must be mapping");
    let message = error.to_string();

    assert!(
        message.contains("deck.yaml:media"),
        "unexpected error: {message}"
    );
    assert!(
        message.contains("media.yaml"),
        "unexpected error: {message}"
    );
    assert!(message.contains("mapping"), "unexpected error: {message}");
}

#[test]
fn media_include_rejects_yaml_tags_inside_included_file() {
    let dir = temp_fixture_dir("brain-brew-media-include-reject-tags");
    std::fs::write(
        dir.join("media.yaml"),
        "media.flag.finland:\n  path: !include path.txt\n  sha256: hash-fi\n",
    )
    .expect("write tagged media include");
    let inline = image_deck_yaml("field.flag: '<img src=\"fi.png\">'");
    let source = deck_with_media_include(&inline, "media.yaml");

    let error = source_includes::resolve_file_includes(&source, &dir.join("deck.yaml"), &dir, &[])
        .expect_err("media include tags are rejected");
    let message = error.to_string();

    assert!(
        message.contains("media.yaml:media.flag.finland.path"),
        "unexpected error: {message}"
    );
    assert!(
        message.contains("unsupported YAML tag"),
        "unexpected error: {message}"
    );
}

#[test]
fn media_include_missing_file_uses_existing_unreadable_error() {
    let dir = temp_fixture_dir("brain-brew-media-include-missing");
    let inline = image_deck_yaml("field.flag: '<img src=\"fi.png\">'");
    let source = deck_with_media_include(&inline, "missing-media.yaml");

    let error = source_includes::resolve_file_includes(&source, &dir.join("deck.yaml"), &dir, &[])
        .expect_err("missing media include fails");

    assert!(
        error.to_string().contains("could not be read"),
        "unexpected error: {error}"
    );
}

#[test]
fn parser_rejects_unknown_fields_in_canonical_yaml() {
    let yaml = EXPECTED_CANONICAL_YAML.replace(
        "name: Ultimate Geography\n",
        "name: Ultimate Geography\n  unsupported: true\n",
    );

    let error = canonical_yaml::from_str(&yaml).expect_err("unknown fields must fail");

    assert!(error.to_string().contains("unsupported"));
}

const EXPECTED_CANONICAL_YAML: &str = r#"deck:
  id: deck.ultimate-geography
  name: Ultimate Geography
  description: A geography deck fixture.
  adapter_ids: {}
note_types:
  note-type.country:
    name: Ultimate Geography Country
    field_order:
      - field.country
      - field.capital
      - field.flag
    fields:
      field.capital:
        name: Capital
      field.country:
        name: Country
      field.flag:
        name: Flag
    card_template_order:
      - template.country-to-capital
    card_templates:
      template.country-to-capital:
        name: Country - Capital
        question_format: '{{Country}}'
        answer_format: '{{FrontSide}}<hr id=answer>{{Capital}}'
        adapter_ids: {}
    styling: |
      .card { font-family: sans-serif; }
    adapter_ids:
      crowdanki:model_id: '1548959259107'
notes:
  note.finland:
    note_type_id: note-type.country
    fields:
      field.capital: Helsinki
      field.country: Finland
      field.flag: '<img src="fi.png">'
    tags:
      - Europe
      - Nordic
    adapter_ids:
      crowdanki:guid: ug-finland-guid
media:
  media.flag.finland:
    path: flags/fi.png
    sha256: 0123456789abcdef
tombstones: []
"#;

fn temp_fixture_dir(prefix: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "{prefix}-{}-{:?}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn deck_with_media_include(inline: &str, include_path: &str) -> String {
    inline.replace(
        "media:\n  media.flag.finland:\n    path: flags/fi.png\n    sha256: hash-fi\n  media.flag.finland.blur:\n    path: flags/fi-blur.png\n    sha256: hash-fi-blur\n",
        &format!("media: !include {include_path}\n"),
    )
}

fn image_media_map_yaml() -> &'static str {
    "media.flag.finland:\n  path: flags/fi.png\n  sha256: hash-fi\nmedia.flag.finland.blur:\n  path: flags/fi-blur.png\n  sha256: hash-fi-blur\n"
}

fn image_deck_yaml(flag_field_yaml: &str) -> String {
    format!(
        r#"deck:
  id: deck.image-fixture
  name: Image Fixture
  description: ''
  adapter_ids: {{}}
note_types:
  note-type.country:
    name: Country
    field_order:
      - field.country
      - field.capital
      - field.flag
    fields:
      field.country:
        name: Country
      field.capital:
        name: Capital
      field.flag:
        name: Flag
    card_template_order:
      - template.country
    card_templates:
      template.country:
        name: Country
        question_format: '{{{{Country}}}}'
        answer_format: '{{{{FrontSide}}}}<hr id=answer>{{{{Capital}}}}'
        adapter_ids: {{}}
    styling: ''
    adapter_ids: {{}}
notes:
  note.finland:
    note_type_id: note-type.country
    fields:
      field.country: Finland
      field.capital: Helsinki
      {flag_field_yaml}
    tags:
      - Europe
    adapter_ids: {{}}
media:
  media.flag.finland:
    path: flags/fi.png
    sha256: hash-fi
  media.flag.finland.blur:
    path: flags/fi-blur.png
    sha256: hash-fi-blur
tombstones: []
"#
    )
}

fn ug_style_deck() -> CanonicalDeck {
    let deck_adapter_ids = AdapterIds::new();
    let mut note_type_adapter_ids = AdapterIds::new();
    note_type_adapter_ids.insert("crowdanki:model_id", "1548959259107");

    let note_type = NoteType {
        id: sid("note-type.country"),
        name: "Ultimate Geography Country".to_owned(),
        variables: BTreeMap::new(),
        fields: vec![
            FieldDefinition {
                id: sid("field.country"),
                name: "Country".to_owned(),
                message_pattern: None,
            },
            FieldDefinition {
                id: sid("field.capital"),
                name: "Capital".to_owned(),
                message_pattern: None,
            },
            FieldDefinition {
                id: sid("field.flag"),
                name: "Flag".to_owned(),
                message_pattern: None,
            },
        ],
        card_templates: vec![CardTemplate {
            id: sid("template.country-to-capital"),
            name: "Country - Capital".to_owned(),
            variables: BTreeMap::new(),
            question_format: "{{Country}}".to_owned(),
            answer_format: "{{FrontSide}}<hr id=answer>{{Capital}}".to_owned(),
            adapter_ids: AdapterIds::new(),
        }],
        styling: ".card { font-family: sans-serif; }\n".to_owned(),
        adapter_ids: note_type_adapter_ids,
    };

    let mut note_adapter_ids = AdapterIds::new();
    note_adapter_ids.insert("crowdanki:guid", "ug-finland-guid");

    let note = Note {
        id: sid("note.finland"),
        note_type_id: sid("note-type.country"),
        variables: BTreeMap::new(),
        fields: BTreeMap::from([
            (sid("field.country"), "Finland".to_owned()),
            (sid("field.capital"), "Helsinki".to_owned()),
            (sid("field.flag"), "<img src=\"fi.png\">".to_owned()),
        ])
        .into(),
        tags: BTreeSet::from(["Europe".to_owned(), "Nordic".to_owned()]),
        adapter_ids: note_adapter_ids,
    };

    CanonicalDeck {
        id: sid("deck.ultimate-geography"),
        name: "Ultimate Geography".to_owned(),
        description: "A geography deck fixture.".to_owned(),
        variables: BTreeMap::new(),
        note_types: BTreeMap::from([(note_type.id.clone(), note_type)]),
        notes: BTreeMap::from([(note.id.clone(), note)]),
        media: BTreeMap::from([(
            sid("media.flag.finland"),
            MediaReference {
                id: sid("media.flag.finland"),
                path: "flags/fi.png".to_owned(),
                sha256: "0123456789abcdef".to_owned(),
            },
        )]),
        tombstones: Tombstones::default(),
        adapter_ids: deck_adapter_ids,
    }
}

fn sid(value: &str) -> StableId {
    StableId::new(value).expect("test stable id is valid")
}
