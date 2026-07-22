use std::collections::{BTreeMap, BTreeSet};

use brain_brew_core::{
    AdapterIds, CanonicalDeck, CardTemplate, CardTemplateChange, ChangeIntent, ExpectedBase,
    FieldDefinition, FieldValue, Note, NoteType, NoteTypeChange, Overlay, OverlayKind,
    PropertyChange, StableId, Tombstones, ValidationErrorKind,
};

fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

fn deck(question_format: &str, answer_format: &str) -> CanonicalDeck {
    let note_type = NoteType {
        id: sid("note-type.country"),
        name: "Country".to_owned(),
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
        ],
        card_templates: vec![CardTemplate {
            id: sid("template.country"),
            name: "Country".to_owned(),
            variables: BTreeMap::new(),
            question_format: question_format.to_owned(),
            answer_format: answer_format.to_owned(),
            adapter_ids: AdapterIds::new(),
        }],
        styling: String::new(),
        adapter_ids: AdapterIds::new(),
    };
    let note = Note {
        id: sid("note.finland"),
        note_type_id: note_type.id.clone(),
        variables: BTreeMap::new(),
        fields: BTreeMap::from([
            (sid("field.country"), FieldValue::scalar("Finland")),
            (sid("field.capital"), FieldValue::scalar("Helsinki")),
        ])
        .into(),
        tags: BTreeSet::new(),
        adapter_ids: AdapterIds::new(),
    };
    CanonicalDeck {
        id: sid("deck.template-validation"),
        name: "Template validation".to_owned(),
        description: String::new(),
        variables: BTreeMap::new(),
        note_types: BTreeMap::from([(note_type.id.clone(), note_type)]),
        notes: BTreeMap::from([(note.id.clone(), note)]),
        media: BTreeMap::new(),
        tombstones: Tombstones::default(),
        adapter_ids: AdapterIds::new(),
    }
}

#[test]
fn validates_direct_type_conditional_inverted_and_repeated_field_references() {
    let deck = deck(
        "{{Country}}{{type:Capital}}{{#Capital}}{{Capital}}{{/Capital}}{{^Capital}}{{Country}}{{/Capital}}{{#Country}}{{.}}{{/Country}}",
        "{{FrontSide}} {{hint:Capital}} {{cloze:Country}} {{helper argument}}",
    );

    deck.validate()
        .expect("supported Anki field references are valid");
}

#[test]
fn rejects_unknown_field_with_typed_template_context() {
    let report = deck("{{Country}}", "{{Missing}}")
        .validate()
        .expect_err("unknown field fails");
    let error = report
        .errors
        .iter()
        .find(|error| error.kind == ValidationErrorKind::UnknownTemplateField)
        .expect("typed unknown-field diagnostic");
    assert_eq!(
        error.path,
        "note_types.note-type.country.card_templates.template.country.answer_format"
    );
    assert!(error.message.contains("note type note-type.country"));
    assert!(error.message.contains("template template.country"));
    assert!(error.message.contains("Missing"));
}

#[test]
fn rejects_malformed_relevant_section_deterministically() {
    let report = deck("{{#Capital}}{{Country}}", "")
        .validate()
        .expect_err("unclosed section fails");
    let error = report
        .errors
        .iter()
        .find(|error| error.kind == ValidationErrorKind::MalformedTemplateReference)
        .expect("typed malformed-reference diagnostic");
    assert_eq!(
        error.path,
        "note_types.note-type.country.card_templates.template.country.question_format"
    );
    assert!(
        error
            .message
            .contains("unclosed Anki field section \"Capital\"")
    );
}

#[test]
fn overlay_template_change_is_validated_against_the_final_schema() {
    let overlay = Overlay {
        id: sid("overlay.stale-template"),
        kind: OverlayKind::Patch,
        translations: None,
        deck_change: None,
        note_changes: BTreeMap::new(),
        note_type_changes: BTreeMap::from([(
            sid("note-type.country"),
            NoteTypeChange {
                intent: ChangeIntent::Merge,
                note_type: None,
                name: None,
                variables: BTreeMap::new(),
                styling: None,
                fields: BTreeMap::new(),
                card_templates: BTreeMap::from([(
                    sid("template.country"),
                    CardTemplateChange {
                        intent: ChangeIntent::Merge,
                        template: None,
                        insert_after: None,
                        name: None,
                        variables: BTreeMap::new(),
                        question_format: None,
                        answer_format: Some(PropertyChange {
                            intent: ChangeIntent::Replace,
                            value: Some("{{Hauptstadt}}".to_owned()),
                            expected_base: Some(ExpectedBase::Value("{{Capital}}".to_owned())),
                        }),
                        adapter_ids: BTreeMap::new(),
                        expected_base: None,
                    },
                )]),
                adapter_ids: BTreeMap::new(),
                expected_base: None,
            },
        )]),
        media_changes: BTreeMap::new(),
    };

    let report = deck("{{Country}}", "{{Capital}}")
        .compose(&[overlay])
        .expect_err("final overlay schema drift fails composition");
    assert!(
        report.errors[0]
            .validation_errors
            .iter()
            .any(|error| error.kind == ValidationErrorKind::UnknownTemplateField)
    );
}

#[test]
fn final_rendered_template_is_revalidated_against_schema() {
    let mut deck = deck("${template.field}", "{{Capital}}");
    deck.note_types
        .get_mut(&sid("note-type.country"))
        .unwrap()
        .card_templates[0]
        .variables
        .insert("template.field".to_owned(), "{{Hauptstadt}}".to_owned());

    deck.validate()
        .expect("unrendered variable has no field token");
    let rendered = deck.render_variables().expect("variables render");
    let report = rendered.validate().expect_err("rendered stale field fails");
    assert!(report.has_kind(ValidationErrorKind::UnknownTemplateField));
}
