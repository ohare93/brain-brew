use brain_brew_core::{ChangeIntent, ExpectedBase, FieldValue, OverlayKind, StableId};
use brain_brew_formats::canonical_yaml;

#[test]
fn formatter_canonicalizes_overlay_yaml() {
    let formatted = canonical_yaml::overlay_format_str(
        r#"kind: extension
id: overlay.extension.extended
note_types:
  note-type.country:
    card_templates:
      template.country-flag:
        template:
          answer_format: '{{Flag}}'
          question_format: '{{Country}}'
          name: Country - Flag
        insert_after: template.capital-country
        intent: add
    styling:
      value: |
        .card { font-family: serif; }
      intent: replace
      expected_base:
        value: |
          .card { font-family: sans-serif; }
    intent: merge
"#,
    )
    .expect("overlay formats");

    assert!(formatted.starts_with("id: overlay.extension.extended\nkind: extension\n"));
    assert!(formatted.contains("    styling:\n      intent: replace\n"));
    assert!(formatted.contains("      template.country-flag:\n        intent: add\n"));
    assert!(formatted.contains("        insert_after: template.capital-country\n"));
    canonical_yaml::overlay_from_str(&formatted).expect("formatted overlay parses");
}

#[test]
fn empty_translation_dictionary_round_trips_as_translation_overlay() {
    let formatted = canonical_yaml::overlay_format_str(
        r#"id: overlay.translation.nb
kind: translation
translations: {}
"#,
    )
    .expect("overlay formats");

    assert_eq!(
        formatted,
        "id: overlay.translation.nb\nkind: translation\ntranslations: {}\n"
    );
    let overlay = canonical_yaml::overlay_from_str(&formatted).expect("formatted overlay parses");
    assert!(overlay.translations.is_some());
}

#[test]
fn parses_sparse_overlay_yaml_with_field_expected_base() {
    let overlay = canonical_yaml::overlay_from_str(
        r#"id: overlay.patch.capital
kind: patch
notes:
  note.finland:
    intent: merge
    fields:
      field.capital:
        intent: replace
        value: Helsingfors
        expected_base:
          value: Helsinki
"#,
    )
    .expect("overlay parses");

    assert_eq!(overlay.id, sid("overlay.patch.capital"));
    assert_eq!(overlay.kind, OverlayKind::Patch);
    let note_change = overlay.note_changes.get(&sid("note.finland")).unwrap();
    assert_eq!(note_change.intent, ChangeIntent::Merge);
    let field_change = note_change.fields.get(&sid("field.capital")).unwrap();
    assert_eq!(field_change.intent, ChangeIntent::Replace);
    assert_eq!(
        field_change.value.as_ref().and_then(FieldValue::as_scalar),
        Some("Helsingfors")
    );
    assert_eq!(
        field_change.expected_base,
        Some(ExpectedBase::Value("Helsinki".to_owned()))
    );
}

#[test]
fn parses_note_type_field_addition_overlay() {
    let overlay = canonical_yaml::overlay_from_str(
        r#"id: overlay.extension.population
kind: extension
note_types:
  note-type.country:
    intent: merge
    fields:
      field.population:
        intent: add
        name: Population
notes: {}
"#,
    )
    .expect("overlay parses");

    assert_eq!(overlay.kind, OverlayKind::Extension);
    let note_type_change = overlay
        .note_type_changes
        .get(&sid("note-type.country"))
        .unwrap();
    let field_change = note_type_change
        .fields
        .get(&sid("field.population"))
        .unwrap();
    assert_eq!(field_change.intent, ChangeIntent::Add);
    assert_eq!(field_change.field.as_ref().unwrap().name, "Population");
}

#[test]
fn parses_note_type_addition_overlay_with_payload() {
    let formatted = canonical_yaml::overlay_format_str(
        r#"id: overlay.extension.regions
kind: extension
note_types:
  note-type.region:
    intent: add
    note_type:
      name: Region Geography
      field_order:
        - field.region
        - field.map
      fields:
        field.map:
          name: Map
        field.region:
          name: Region
      card_template_order:
        - template.region-map
      card_templates:
        template.region-map:
          name: Region - Map
          question_format: '{{Region}}'
          answer_format: '{{Map}}'
      styling: |
        .card { font-family: sans-serif; }
"#,
    )
    .expect("overlay formats");

    let overlay = canonical_yaml::overlay_from_str(&formatted).expect("formatted overlay parses");
    let change = overlay
        .note_type_changes
        .get(&sid("note-type.region"))
        .unwrap();
    assert_eq!(change.intent, ChangeIntent::Add);
    let note_type = change.note_type.as_ref().unwrap();
    assert_eq!(note_type.name, "Region Geography");
    assert_eq!(note_type.fields[0].id, sid("field.region"));
    assert_eq!(note_type.card_templates[0].id, sid("template.region-map"));
    assert!(formatted.contains("    note_type:\n      name: Region Geography\n"));
}

#[test]
fn parses_field_additions_shorthand_for_multiple_fields() {
    let overlay = canonical_yaml::overlay_from_str(
        r#"id: overlay.extension.population
kind: extension
field_additions:
  note-type.country:
    fields:
      field.population: Population
      field.area: Area
    values:
      note.australia:
        field.population: 25.0 million
        field.area: 7.69 million km²
      note.austria:
        field.population: 16.0 million
"#,
    )
    .expect("overlay parses");

    let note_type_change = overlay
        .note_type_changes
        .get(&sid("note-type.country"))
        .unwrap();
    assert_eq!(note_type_change.intent, ChangeIntent::Merge);
    assert_eq!(
        note_type_change
            .fields
            .get(&sid("field.population"))
            .unwrap()
            .field
            .as_ref()
            .unwrap()
            .name,
        "Population"
    );
    assert_eq!(
        note_type_change
            .fields
            .get(&sid("field.area"))
            .unwrap()
            .field
            .as_ref()
            .unwrap()
            .name,
        "Area"
    );

    let note_change = overlay.note_changes.get(&sid("note.australia")).unwrap();
    assert_eq!(note_change.intent, ChangeIntent::Merge);
    assert_eq!(
        note_change
            .fields
            .get(&sid("field.population"))
            .unwrap()
            .value
            .as_ref()
            .and_then(FieldValue::as_scalar),
        Some("25.0 million")
    );
    assert_eq!(
        note_change
            .fields
            .get(&sid("field.area"))
            .unwrap()
            .value
            .as_ref()
            .and_then(FieldValue::as_scalar),
        Some("7.69 million km²")
    );
}

#[test]
fn field_additions_shorthand_matches_verbose_overlay_semantics() {
    let base = canonical_yaml::from_str(
        r#"deck:
  id: deck.demo
  name: Demo
  description: ''
note_types:
  note-type.country:
    name: Country
    field_order:
      - field.country
    fields:
      field.country:
        name: Country
    card_template_order:
      - template.country
    card_templates:
      template.country:
        name: Country
        question_format: '{{Country}}'
        answer_format: '{{Country}}'
    styling: ''
notes:
  note.australia:
    note_type_id: note-type.country
    fields:
      field.country: Australia
    tags: []
  note.austria:
    note_type_id: note-type.country
    fields:
      field.country: Austria
    tags: []
media: {}
tombstones: []
"#,
    )
    .expect("base parses");
    let concise = canonical_yaml::overlay_from_str(
        r#"id: overlay.extension.population
kind: extension
field_additions:
  note-type.country:
    fields:
      field.population: Population
      field.area: Area
    values:
      note.australia:
        field.population: 25.0 million
        field.area: 7.69 million km²
      note.austria:
        field.population: 16.0 million
        field.area: 83,879 km²
"#,
    )
    .expect("concise overlay parses");
    let verbose = canonical_yaml::overlay_from_str(
        r#"id: overlay.extension.population
kind: extension
note_types:
  note-type.country:
    intent: merge
    fields:
      field.population:
        intent: add
        name: Population
      field.area:
        intent: add
        name: Area
notes:
  note.australia:
    intent: merge
    fields:
      field.population:
        intent: add
        value: 25.0 million
      field.area:
        intent: add
        value: 7.69 million km²
  note.austria:
    intent: merge
    fields:
      field.population:
        intent: add
        value: 16.0 million
      field.area:
        intent: add
        value: 83,879 km²
"#,
    )
    .expect("verbose overlay parses");

    assert_eq!(concise, verbose);
    let concise_deck = base.compose(&[concise]).expect("concise composes");
    let verbose_deck = base.compose(&[verbose]).expect("verbose composes");
    assert!(concise_deck.semantic_diff(&verbose_deck).is_empty());
}

#[test]
fn formatter_prefers_field_additions_shorthand() {
    let formatted = canonical_yaml::overlay_format_str(
        r#"id: overlay.extension.population
kind: extension
note_types:
  note-type.country:
    intent: merge
    fields:
      field.population:
        intent: add
        name: Population
notes:
  note.australia:
    intent: merge
    fields:
      field.population:
        intent: add
        value: 25.0 million
"#,
    )
    .expect("overlay formats");

    assert_eq!(
        formatted,
        "id: overlay.extension.population\nkind: extension\nfield_additions:\n  note-type.country:\n    fields:\n      field.population: Population\n    values:\n      note.australia:\n        field.population: 25.0 million\n"
    );
}

#[test]
fn parses_field_fills_shorthand_for_existing_blank_fields() {
    let overlay = canonical_yaml::overlay_from_str(
        r#"id: overlay.extension.hardcore.fills.en
kind: extension
field_fills:
  note.anguilla:
    field.capital: The Valley
    field.flag: '<img src="ug-flag-anguilla.svg" />'
"#,
    )
    .expect("overlay parses");

    let note_change = overlay.note_changes.get(&sid("note.anguilla")).unwrap();
    assert_eq!(note_change.intent, ChangeIntent::Merge);
    let capital = note_change.fields.get(&sid("field.capital")).unwrap();
    assert_eq!(capital.intent, ChangeIntent::Replace);
    assert_eq!(
        capital.value.as_ref().and_then(FieldValue::as_scalar),
        Some("The Valley")
    );
    assert_eq!(
        capital.expected_base,
        Some(ExpectedBase::Value(String::new()))
    );
}

#[test]
fn field_fills_shorthand_matches_verbose_overlay_semantics() {
    let concise = canonical_yaml::overlay_from_str(
        r#"id: overlay.extension.hardcore.fills.en
kind: extension
field_fills:
  note.anguilla:
    field.capital: The Valley
    field.flag: '<img src="ug-flag-anguilla.svg" />'
"#,
    )
    .expect("concise overlay parses");
    let verbose = canonical_yaml::overlay_from_str(
        r#"id: overlay.extension.hardcore.fills.en
kind: extension
notes:
  note.anguilla:
    intent: merge
    fields:
      field.capital:
        intent: replace
        value: The Valley
        expected_base:
          value: ''
      field.flag:
        intent: replace
        value: '<img src="ug-flag-anguilla.svg" />'
        expected_base:
          value: ''
"#,
    )
    .expect("verbose overlay parses");

    assert_eq!(concise, verbose);
}

#[test]
fn formatter_prefers_field_fills_shorthand() {
    let formatted = canonical_yaml::overlay_format_str(
        r#"id: overlay.extension.hardcore.fills.en
kind: extension
notes:
  note.anguilla:
    intent: merge
    fields:
      field.capital:
        intent: replace
        value: The Valley
        expected_base:
          value: ''
"#,
    )
    .expect("overlay formats");

    assert_eq!(
        formatted,
        "id: overlay.extension.hardcore.fills.en\nkind: extension\nfield_fills:\n  note.anguilla:\n    field.capital: The Valley\n"
    );
}

#[test]
fn parses_formats_and_round_trips_image_values_in_overlay_field_positions() {
    let overlay = canonical_yaml::overlay_from_str(
        r#"id: overlay.extension.images
kind: extension
field_additions:
  note-type.country:
    fields:
      field.map: Map
    values:
      note.finland:
        field.map: !image media.map.finland
field_fills:
  note.iceland:
    field.flag:
      - !image media.flag.iceland.blur
      - !image media.flag.iceland
notes:
  note.norway:
    intent: merge
    fields:
      field.flag:
        intent: replace
        value: !image media.flag.norway
        expected_base:
          value: old raw flag
"#,
    )
    .expect("overlay image values parse");

    assert_eq!(
        overlay.note_changes[&sid("note.finland")].fields[&sid("field.map")]
            .value
            .as_ref()
            .unwrap()
            .as_images()
            .unwrap()[0]
            .media_id,
        sid("media.map.finland")
    );
    assert_eq!(
        overlay.note_changes[&sid("note.iceland")].fields[&sid("field.flag")]
            .value
            .as_ref()
            .unwrap()
            .as_images()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        overlay.note_changes[&sid("note.norway")].fields[&sid("field.flag")]
            .value
            .as_ref()
            .unwrap()
            .as_images()
            .unwrap()[0]
            .media_id,
        sid("media.flag.norway")
    );

    let formatted = canonical_yaml::overlay_to_string(&overlay).expect("overlay emits");
    assert!(formatted.contains("field.map: !image media.map.finland\n"));
    assert!(formatted.contains(
        "field.flag:\n      - !image media.flag.iceland.blur\n      - !image media.flag.iceland\n"
    ));
    assert!(formatted.contains("value: !image media.flag.norway\n"));
    let reparsed = canonical_yaml::overlay_from_str(&formatted).expect("formatted overlay parses");
    assert_eq!(reparsed, overlay);
    assert_eq!(
        canonical_yaml::overlay_format_str(&formatted).expect("overlay formats twice"),
        formatted
    );
}

#[test]
fn formatter_orders_translation_dictionary_sections_deterministically() {
    let formatted = canonical_yaml::overlay_format_str(
        r#"id: overlay.translation.de
kind: translation
translations:
  adapter_ids:
    crowdanki:guid:
      old-guid: new-guid
  contextual:
    notes.note:
      georgia.fields.field.country:
        Georgia: Georgien
  no_change:
    - Canada
    - Andorra
  direct:
    Germany: Deutschland
target_adaptations:
  notes.note.anguilla.fields.field.capital:
    expected_source: ''
    target: The Valley
"#,
    )
    .expect("overlay formats");

    assert!(formatted.contains(
        "    notes.note:\n      georgia.fields.field.country:\n        Georgia: Georgien\n"
    ));
    assert!(formatted.contains("  no_change:\n    - Andorra\n    - Canada\n"));
    assert!(
        formatted.find("  direct:\n").unwrap() < formatted.find("  contextual:\n").unwrap(),
        "direct translations are emitted before contextual translations"
    );
    assert!(
        formatted.find("  contextual:\n").unwrap() < formatted.find("  no_change:\n").unwrap(),
        "contextual translations are emitted before no-change entries"
    );
    assert!(
        formatted.find("  no_change:\n").unwrap() < formatted.find("  adapter_ids:\n").unwrap(),
        "no-change entries are emitted before adapter_ids"
    );
    assert!(
        formatted.find("  adapter_ids:\n").unwrap()
            < formatted.find("target_adaptations:\n").unwrap(),
        "target adaptations are emitted after the translations dictionary"
    );
}

#[test]
fn rejects_duplicate_overlay_and_contextual_translation_keys_with_schema_paths() {
    let cases = [
        (
            "overlay note field",
            r#"id: overlay.patch.capital
kind: patch
notes:
  note.finland:
    intent: merge
    fields:
      field.capital:
        intent: replace
        value: Helsinki
      field.capital:
        intent: replace
        value: Helsingfors
"#,
            "notes.note.finland.fields.field.capital",
            "field.capital",
        ),
        (
            "contextual translation source",
            r#"id: overlay.translation.de
kind: translation
translations:
  contextual:
    notes.note.finland:
      Finland: Finnland
      Finland: Suomi
"#,
            "translations.contextual.notes.note.finland.Finland",
            "Finland",
        ),
    ];

    for (case, yaml, path, key) in cases {
        for error in [
            canonical_yaml::overlay_from_str(yaml)
                .expect_err(&format!("{case}: decoder rejects duplicate")),
            canonical_yaml::overlay_format_str(yaml)
                .expect_err(&format!("{case}: formatter rejects duplicate")),
        ] {
            let message = error.to_string();
            assert!(
                message.contains(&format!("duplicate key {key:?}")),
                "{case}: {message}"
            );
            assert!(message.contains(path), "{case}: {message}");
        }
    }
}

#[test]
fn parses_upstream_ug_target_additions_as_blank_source_adaptations() {
    let overlay = canonical_yaml::overlay_from_str(
        r#"id: overlay.translation.cs
kind: translation
translations:
  target_additions:
    notes.note.pacific-ocean.fields.field.country-info: 'Známý též jako Pacifik.'
"#,
    )
    .expect("overlay parses");

    let translations = overlay.translations.expect("translation dictionary");
    let adaptation =
        &translations.target_adaptations["notes.note.pacific-ocean.fields.field.country-info"];
    assert_eq!(adaptation.expected_source, "");
    assert_eq!(adaptation.target, "Známý též jako Pacifik.");
}

#[test]
fn rejects_duplicate_top_level_target_adaptation_paths() {
    let error = canonical_yaml::overlay_from_str(
        r#"id: overlay.translation.da
kind: translation
translations:
  target_adaptations:
    notes.note.finland.fields.field.capital:
      expected_source: Helsinki
      target: Helsingfors
target_adaptations:
  notes.note.finland.fields.field.capital:
    expected_source: Helsinki
    target: Helsingfors
"#,
    )
    .expect_err("duplicate target adaptation path is rejected");

    assert_eq!(
        error.to_string(),
        "invalid translation dictionary: top-level target adaptation notes.note.finland.fields.field.capital duplicates translations target_adaptations entry"
    );
}

#[test]
fn parses_metadata_and_adapter_id_overlay_changes() {
    let overlay = canonical_yaml::overlay_from_str(
        r#"id: overlay.translation.de
kind: translation
deck:
  name:
    intent: replace
    value: Ultimate Geography [DE]
    expected_base:
      value: Ultimate Geography
  adapter_ids:
    crowdanki:uuid:
      intent: replace
      value: de-deck-uuid
      expected_base:
        value: en-deck-uuid
note_types:
  note-type.country:
    intent: merge
    name:
      intent: replace
      value: Ultimate Geography [DE]
      expected_base:
        value: Ultimate Geography
    adapter_ids:
      crowdanki:model_id:
        intent: replace
        value: de-model-id
        expected_base:
          value: en-model-id
    card_templates:
      template.country-capital:
        intent: merge
        adapter_ids:
          crowdanki:ord:
            intent: add
            value: '0'
notes:
  note.finland:
    intent: merge
    adapter_ids:
      crowdanki:guid:
        intent: replace
        value: ug-finland-de-guid
        expected_base:
          value: ug-finland-guid
"#,
    )
    .expect("overlay parses");

    let deck_change = overlay.deck_change.as_ref().unwrap();
    assert_eq!(
        deck_change.name.as_ref().unwrap().value.as_deref(),
        Some("Ultimate Geography [DE]")
    );
    assert_eq!(
        deck_change
            .adapter_ids
            .get("crowdanki:uuid")
            .unwrap()
            .value
            .as_deref(),
        Some("de-deck-uuid")
    );

    let note_type_change = overlay
        .note_type_changes
        .get(&sid("note-type.country"))
        .unwrap();
    assert_eq!(
        note_type_change.name.as_ref().unwrap().value.as_deref(),
        Some("Ultimate Geography [DE]")
    );
    assert_eq!(
        note_type_change
            .adapter_ids
            .get("crowdanki:model_id")
            .unwrap()
            .value
            .as_deref(),
        Some("de-model-id")
    );
    assert_eq!(
        note_type_change
            .card_templates
            .get(&sid("template.country-capital"))
            .unwrap()
            .adapter_ids
            .get("crowdanki:ord")
            .unwrap()
            .value
            .as_deref(),
        Some("0")
    );

    let note_change = overlay.note_changes.get(&sid("note.finland")).unwrap();
    assert_eq!(
        note_change
            .adapter_ids
            .get("crowdanki:guid")
            .unwrap()
            .value
            .as_deref(),
        Some("ug-finland-de-guid")
    );
}

#[test]
fn parses_add_note_overlay_payload() {
    let overlay = canonical_yaml::overlay_from_str(
        r#"id: overlay.extension.sweden
kind: extension
notes:
  note.sweden:
    intent: add
    note:
      note_type_id: note-type.country
      fields:
        field.country: Sweden
        field.capital: Stockholm
      tags: [Europe, Nordic]
      adapter_ids:
        crowdanki:guid: ug-sweden-guid
"#,
    )
    .expect("overlay parses");

    let note_change = overlay.note_changes.get(&sid("note.sweden")).unwrap();
    let note = note_change.note.as_ref().unwrap();
    assert_eq!(note.id, sid("note.sweden"));
    assert_eq!(note.note_type_id, sid("note-type.country"));
    assert_eq!(note.fields.get(&sid("field.capital")).unwrap(), "Stockholm");
    assert_eq!(
        note.adapter_ids.get("crowdanki:guid"),
        Some("ug-sweden-guid")
    );
}

#[test]
fn parses_card_template_and_styling_overlay_changes() {
    let overlay = canonical_yaml::overlay_from_str(
        r#"id: overlay.extension.extended
kind: extension
note_types:
  note-type.country:
    intent: merge
    styling:
      intent: replace
      value: |
        .card { font-family: serif; }
      expected_base:
        value: |
          .card { font-family: sans-serif; }
    card_templates:
      template.country-flag:
        intent: add
        insert_after: template.capital-country
        template:
          name: Country - Flag
          question_format: '{{Country}}'
          answer_format: '{{Flag}}'
          adapter_ids: {}
      template.country-capital:
        intent: merge
        question_format:
          intent: replace
          value: '{{Land}}'
          expected_base:
            value: '{{Country}}'
"#,
    )
    .expect("overlay parses");

    let note_type_change = overlay
        .note_type_changes
        .get(&sid("note-type.country"))
        .unwrap();
    assert_eq!(
        note_type_change.styling.as_ref().unwrap().value.as_deref(),
        Some(".card { font-family: serif; }\n")
    );
    let add_template = note_type_change
        .card_templates
        .get(&sid("template.country-flag"))
        .unwrap();
    assert_eq!(
        add_template.insert_after,
        Some(sid("template.capital-country"))
    );
    assert_eq!(
        add_template.template.as_ref().unwrap().name,
        "Country - Flag"
    );
    let replace_template = note_type_change
        .card_templates
        .get(&sid("template.country-capital"))
        .unwrap();
    assert_eq!(
        replace_template
            .question_format
            .as_ref()
            .unwrap()
            .value
            .as_deref(),
        Some("{{Land}}")
    );
}

#[test]
fn structured_field_expected_bases_round_trip_atomically() {
    let source = r#"id: overlay.patch.image
kind: patch
notes:
  note.finland:
    intent: merge
    fields:
      field.flag:
        intent: replace
        value: !image media.flag.new
        expected_base:
          value:
            - !image media.flag.blur
            - !image media.flag.old
"#;
    let overlay =
        canonical_yaml::overlay_from_str(source).expect("structured expected base parses");
    let change = &overlay.note_changes[&sid("note.finland")].fields[&sid("field.flag")];
    let ExpectedBase::FieldValue(expected) = change.expected_base.as_ref().unwrap() else {
        panic!("expected semantic field base");
    };
    assert_eq!(expected.as_images().unwrap().len(), 2);

    let once = canonical_yaml::overlay_to_string(&overlay).expect("structured expected base emits");
    assert!(
        once.contains("expected_base:\n          value:\n            - !image media.flag.blur")
    );
    let twice =
        canonical_yaml::overlay_format_str(&once).expect("structured expected base reformats");
    assert_eq!(twice, once);
}

#[test]
fn parses_tag_and_media_overlay_changes() {
    let overlay = canonical_yaml::overlay_from_str(
        r#"id: overlay.extension.tags-media
kind: extension
notes:
  note.finland:
    intent: merge
    tags:
      UG::Nordic:
        intent: add
      Nordic:
        intent: remove
        expected_base:
          value: Nordic
media:
  media.flag.sweden:
    intent: add
    path: flags/se.png
    sha256: abcdef
"#,
    )
    .expect("overlay parses");

    let note_change = overlay.note_changes.get(&sid("note.finland")).unwrap();
    assert_eq!(
        note_change.tags.get("UG::Nordic").unwrap().intent,
        ChangeIntent::Add
    );
    assert_eq!(
        note_change.tags.get("Nordic").unwrap().expected_base,
        Some(ExpectedBase::Value("Nordic".to_owned()))
    );

    let media_change = overlay
        .media_changes
        .get(&sid("media.flag.sweden"))
        .unwrap();
    assert_eq!(media_change.intent, ChangeIntent::Add);
    assert_eq!(media_change.media.as_ref().unwrap().path, "flags/se.png");
}

#[test]
fn parses_remove_overlay_with_canonical_entity_fingerprint() {
    let overlay = canonical_yaml::overlay_from_str(
        r#"id: overlay.patch.remove-finland
kind: patch
notes:
  note.finland:
    intent: remove
    expected_base:
      fingerprint: sha256:v1:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
"#,
    )
    .expect("overlay parses");

    let note_change = overlay.note_changes.get(&sid("note.finland")).unwrap();
    assert_eq!(note_change.intent, ChangeIntent::Remove);
    assert!(matches!(
        note_change.expected_base,
        Some(ExpectedBase::EntityFingerprint(_))
    ));
}

#[test]
fn rejects_legacy_presence_only_expected_base_with_migration_help() {
    let error = canonical_yaml::overlay_from_str(
        "id: overlay.patch.remove-finland\nkind: patch\nnotes:\n  note.finland:\n    intent: remove\n    expected_base: entity_present\n",
    )
    .expect_err("presence-only expected bases fail closed");

    assert!(
        error
            .to_string()
            .contains("entity_present is no longer accepted")
    );
    assert!(error.to_string().contains("diff --as-overlay"));
}

#[test]
fn rejects_unknown_overlay_fields() {
    let error = canonical_yaml::overlay_from_str(
        r#"id: overlay.patch.capital
kind: patch
unsupported: true
notes: {}
"#,
    )
    .expect_err("unknown overlay fields must fail");

    assert!(error.to_string().contains("unsupported"));
}

fn sid(value: &str) -> StableId {
    StableId::new(value).expect("test stable id is valid")
}
