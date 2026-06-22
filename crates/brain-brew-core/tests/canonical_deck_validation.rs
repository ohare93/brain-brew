use std::collections::{BTreeMap, BTreeSet};

use brain_brew_core::{
    AdapterIds, CanonicalDeck, CardTemplate, FieldDefinition, MediaReference, Note, NoteType,
    StableId, ValidationErrorKind,
};

#[test]
fn complete_deck_preserves_anki_compatible_structure_and_adapter_identities() {
    let deck = ug_style_deck();

    assert!(deck.validate().is_ok());

    let note_type = deck
        .note_types
        .get(&sid("note-type.country"))
        .expect("note type exists");
    assert_eq!(note_type.fields[0].id, sid("field.country"));
    assert_eq!(note_type.fields[1].id, sid("field.capital"));
    assert_eq!(
        note_type.card_templates[0].id,
        sid("template.country-to-capital")
    );
    assert_eq!(note_type.card_templates[0].question_format, "{{Country}}");
    assert_eq!(
        note_type.card_templates[0].answer_format,
        "{{FrontSide}}<hr id=answer>{{Capital}}"
    );
    assert_eq!(note_type.styling, ".card { font-family: sans-serif; }\n");
    assert_eq!(
        note_type.adapter_ids.get("crowdanki:model_id"),
        Some("1548959259107")
    );

    let note = deck.notes.get(&sid("note.finland")).expect("note exists");
    assert_eq!(note.note_type_id, sid("note-type.country"));
    assert_eq!(
        note.fields.get(&sid("field.country")),
        Some(&"Finland".to_owned())
    );
    assert!(note.tags.contains("Europe"));
    assert_eq!(
        note.adapter_ids.get("crowdanki:guid"),
        Some("ug-finland-guid")
    );

    let flag = deck
        .media
        .get(&sid("media.flag.finland"))
        .expect("media exists");
    assert_eq!(flag.path, "flags/fi.png");
    assert_eq!(flag.sha256, "0123456789abcdef");
}

#[test]
fn validation_rejects_note_with_missing_note_type() {
    let mut deck = ug_style_deck();
    deck.notes
        .get_mut(&sid("note.finland"))
        .unwrap()
        .note_type_id = sid("note-type.missing");

    let report = deck
        .validate()
        .expect_err("missing note type must fail validation");

    assert!(report.has_kind(ValidationErrorKind::MissingNoteType));
    assert!(
        report
            .errors
            .iter()
            .any(|error| error.path == "notes.note.finland.note_type_id")
    );
}

#[test]
fn validation_rejects_note_fields_not_defined_by_its_note_type() {
    let mut deck = ug_style_deck();
    deck.notes
        .get_mut(&sid("note.finland"))
        .unwrap()
        .fields
        .insert(sid("field.population"), "5.6 million".to_owned());

    let report = deck
        .validate()
        .expect_err("unknown field must fail validation");

    assert!(report.has_kind(ValidationErrorKind::UnknownNoteField));
    assert!(
        report
            .errors
            .iter()
            .any(|error| error.path == "notes.note.finland.fields.field.population")
    );
}

#[test]
fn validation_rejects_note_missing_required_field() {
    let mut deck = ug_style_deck();
    deck.notes
        .get_mut(&sid("note.finland"))
        .unwrap()
        .fields
        .remove(&sid("field.capital"));

    let report = deck
        .validate()
        .expect_err("missing field must fail validation");

    assert!(report.has_kind(ValidationErrorKind::MissingNoteField));
    assert!(
        report
            .errors
            .iter()
            .any(|error| error.path == "notes.note.finland.fields.field.capital")
    );
}

fn ug_style_deck() -> CanonicalDeck {
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
        field_messages: BTreeMap::new(),
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
        adapter_ids: AdapterIds::new(),
    }
}

fn sid(value: &str) -> StableId {
    StableId::new(value).expect("test stable id is valid")
}
