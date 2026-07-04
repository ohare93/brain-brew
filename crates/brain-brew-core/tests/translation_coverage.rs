use std::collections::{BTreeMap, BTreeSet};

use brain_brew_core::{
    AdapterIds, CanonicalDeck, CardTemplate, FieldDefinition, MessageComponent, Note, NoteType,
    Overlay, OverlayKind, StableId, StaleTranslation, StructuredMessage,
    TranslationCoverageCategory, TranslationDictionary,
};

fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

#[test]
fn untranslated_structured_message_format_is_fallback_coverage_but_compose_succeeds() {
    let deck = structured_message_deck();
    let overlay = Overlay {
        id: sid("overlay.translation.da"),
        kind: OverlayKind::Translation,
        translations: Some(TranslationDictionary {
            direct: BTreeMap::from([("Helsinki".to_owned(), "Helsingfors".to_owned())]),
            contextual: BTreeMap::new(),
            no_change: BTreeSet::new(),
            target_adaptations: BTreeMap::new(),
            stale_translations: Vec::new(),
            variables: BTreeMap::new(),
            adapter_ids: BTreeMap::new(),
            require_complete: true,
            ignore_paths: BTreeSet::from([
                "deck.*".to_owned(),
                "note_types.*".to_owned(),
                "notes.*.fields.field.country".to_owned(),
                "notes.*.tags.*".to_owned(),
            ]),
        }),
        deck_change: None,
        note_changes: BTreeMap::new(),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    };

    let report = deck
        .translation_coverage(&overlay)
        .expect("translation overlay has coverage");
    assert!(report.entries.iter().any(|entry| {
        entry.category == TranslationCoverageCategory::UntranslatedFallback
            && entry.path == "notes.note.finland.fields.field.summary.message.format"
            && entry.source == "Capital: {capital}"
            && entry.translated.as_deref() == Some("Capital: {capital}")
    }));

    let resolved = deck.compose(&[overlay]).expect("compose does not fail");
    assert_eq!(
        resolved.notes[&sid("note.finland")].fields[&sid("field.summary")],
        "Capital: Helsingfors"
    );
}

#[test]
fn shadowed_stale_record_reports_rendered_translation_instead_of_dead_target() {
    let deck = structured_message_deck();
    let overlay = Overlay {
        id: sid("overlay.translation.da"),
        kind: OverlayKind::Translation,
        translations: Some(TranslationDictionary {
            direct: BTreeMap::from([(
                "Capital: Helsinki".to_owned(),
                "Hovedstad: Helsingfors".to_owned(),
            )]),
            contextual: BTreeMap::new(),
            no_change: BTreeSet::new(),
            target_adaptations: BTreeMap::new(),
            stale_translations: vec![StaleTranslation {
                old_source: "Old capital line".to_owned(),
                new_source: "Capital: Helsinki".to_owned(),
                target: "STALE TARGET THAT SHOULD NOT RENDER".to_owned(),
                context: None,
            }],
            variables: BTreeMap::new(),
            adapter_ids: BTreeMap::new(),
            require_complete: false,
            ignore_paths: BTreeSet::new(),
        }),
        deck_change: None,
        note_changes: BTreeMap::new(),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    };

    let report = deck
        .translation_coverage(&overlay)
        .expect("translation overlay has coverage");
    let stale = report
        .entries
        .iter()
        .find(|entry| entry.category == TranslationCoverageCategory::StaleTranslation)
        .expect("shadowed stale record remains visible");
    assert_eq!(stale.path, "translations.stale_translations.0");
    assert_eq!(stale.source, "Capital: Helsinki");
    assert_eq!(stale.translated.as_deref(), Some("Hovedstad: Helsingfors"));

    let resolved = deck.compose(&[overlay]).expect("compose uses direct entry");
    assert_eq!(
        resolved.notes[&sid("note.finland")].fields[&sid("field.summary")],
        "Hovedstad: Helsingfors"
    );
}

fn structured_message_deck() -> CanonicalDeck {
    let note_type = NoteType {
        id: sid("note-type.country"),
        name: "Country".to_owned(),
        variables: BTreeMap::new(),
        fields: vec![
            FieldDefinition {
                id: sid("field.country"),
                name: "Country".to_owned(),
            },
            FieldDefinition {
                id: sid("field.summary"),
                name: "Summary".to_owned(),
            },
        ],
        card_templates: vec![CardTemplate {
            id: sid("template.summary"),
            name: "Summary".to_owned(),
            variables: BTreeMap::new(),
            question_format: "{{Country}}".to_owned(),
            answer_format: "{{Summary}}".to_owned(),
            adapter_ids: AdapterIds::new(),
        }],
        styling: String::new(),
        adapter_ids: AdapterIds::new(),
    };
    let note = Note {
        id: sid("note.finland"),
        note_type_id: sid("note-type.country"),
        variables: BTreeMap::new(),
        fields: BTreeMap::from([
            (sid("field.country"), "Finland".to_owned()),
            (sid("field.summary"), "Capital: Helsinki".to_owned()),
        ]),
        field_messages: BTreeMap::from([(
            sid("field.summary"),
            StructuredMessage {
                components: Vec::new(),
                format: Some("Capital: {capital}".to_owned()),
                variables: BTreeMap::from([(
                    "capital".to_owned(),
                    MessageComponent::Text("Helsinki".to_owned()),
                )]),
            },
        )]),
        field_images: BTreeMap::new(),
        tags: BTreeSet::new(),
        adapter_ids: AdapterIds::new(),
    };

    CanonicalDeck {
        id: sid("deck.translation-coverage"),
        name: "Translation Coverage".to_owned(),
        description: String::new(),
        variables: BTreeMap::new(),
        note_types: BTreeMap::from([(note_type.id.clone(), note_type)]),
        notes: BTreeMap::from([(note.id.clone(), note)]),
        media: BTreeMap::new(),
        tombstones: BTreeSet::new(),
        adapter_ids: AdapterIds::new(),
    }
}
