use std::collections::{BTreeMap, BTreeSet};

use brain_brew_core::{
    AdapterIds, CanonicalDeck, CardTemplate, FieldDefinition, MediaReference, Note, NoteType,
    StableId,
};
use brain_brew_formats::{canonical_yaml, crowdanki};

#[test]
fn emits_canonical_deck_yaml_with_explicit_order_arrays() {
    let yaml = canonical_yaml::to_string(&ug_style_deck()).expect("deck emits");

    assert_eq!(yaml, EXPECTED_CANONICAL_YAML);
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
fn formatter_canonicalizes_valid_yaml_bytes() {
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
  target_additions:
    notes.note.denmark.fields.field.country-info: Ekstra tekst.
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
        translations.target_additions["notes.note.denmark.fields.field.country-info"],
        "Ekstra tekst."
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
            },
            FieldDefinition {
                id: sid("field.capital"),
                name: "Capital".to_owned(),
            },
            FieldDefinition {
                id: sid("field.flag"),
                name: "Flag".to_owned(),
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
        ]),
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
        tombstones: BTreeSet::new(),
        adapter_ids: deck_adapter_ids,
    }
}

fn sid(value: &str) -> StableId {
    StableId::new(value).expect("test stable id is valid")
}
