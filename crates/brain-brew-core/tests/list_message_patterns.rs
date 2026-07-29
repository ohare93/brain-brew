use std::collections::{BTreeMap, BTreeSet};

use brain_brew_core::{
    AdapterIds, CanonicalDeck, ChangeIntent, DeckPath, ExpectedBase, FieldChange, FieldDefinition,
    FieldMap, FieldValue, ListMessageArgument, ListMessageItems, ListMessageParameter,
    ListMessagePattern, Note, NoteChange, NoteType, Overlay, OverlayKind, SemanticChangeKind,
    StableId, StaleTranslation, Tombstones, TranslationCoverageCategory, TranslationDictionary,
    fingerprint_note, fingerprint_note_type,
};

#[test]
fn list_message_pattern_renders_one_and_many_ordered_items() {
    let deck = pattern_deck();

    assert_eq!(
        deck.field_text(&sid("note.andorra"), &sid("field.flag-similarity"))
            .unwrap(),
        "Moldova (wider, coat of arms with eagle)"
    );
    assert_eq!(
        deck.field_text(&sid("note.yemen"), &sid("field.flag-similarity"))
            .unwrap(),
        "Egypt (with emblem), Iraq (with text)"
    );
    assert!(deck.notes[&sid("note.moldova")].fields[&sid("field.flag-similarity")].is_blank());
}

#[test]
fn overlay_can_fill_a_blank_pattern_field_with_list_items() {
    let deck = pattern_deck();
    let overlay = Overlay {
        id: sid("overlay.extension.similarity"),
        kind: OverlayKind::Extension,
        translations: None,
        deck_change: None,
        note_changes: BTreeMap::from([(
            sid("note.moldova"),
            NoteChange {
                intent: ChangeIntent::Merge,
                note: None,
                variables: BTreeMap::new(),
                fields: BTreeMap::from([(
                    sid("field.flag-similarity"),
                    FieldChange {
                        intent: ChangeIntent::Replace,
                        value: Some(list_message(&[("note.egypt", "with emblem")])),
                        expected_base: Some(ExpectedBase::Value(String::new())),
                    },
                )]),
                tags: BTreeMap::new(),
                adapter_ids: BTreeMap::new(),
                expected_base: None,
            },
        )]),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    };

    let resolved = deck.compose(&[overlay]).unwrap();
    assert_eq!(
        resolved
            .field_text(&sid("note.moldova"), &sid("field.flag-similarity"))
            .unwrap(),
        "Egypt (with emblem)"
    );
}

#[test]
fn list_message_pattern_translates_shared_glue_text_refs_and_usage_separator() {
    let deck = pattern_deck();
    let translations = TranslationDictionary {
        direct: BTreeMap::from([
            ("Egypt".to_owned(), "Egito".to_owned()),
            ("Iraq".to_owned(), "Iraque".to_owned()),
            ("with emblem".to_owned(), "com emblema".to_owned()),
            ("with text".to_owned(), "com texto".to_owned()),
        ]),
        contextual: BTreeMap::from([
            (
                "note_types.note-type.country.fields.field.flag-similarity.message_pattern.item_format"
                    .to_owned(),
                BTreeMap::from([(
                    "{country} ({description})".to_owned(),
                    "{country}（{description}）".to_owned(),
                )]),
            ),
            (
                "note_types.note-type.country.fields.field.flag-similarity.message_pattern.separator"
                    .to_owned(),
                BTreeMap::from([(", ".to_owned(), "、".to_owned())]),
            ),
            (
                "notes.note.yemen.fields.field.flag-similarity.message.separator".to_owned(),
                BTreeMap::from([(", ".to_owned(), ", e ".to_owned())]),
            ),
        ]),
        ..TranslationDictionary::default()
    };
    let overlay = translation_overlay(translations);
    let coverage = deck.translation_coverage(&overlay).unwrap();
    assert_eq!(
        coverage
            .entries
            .iter()
            .filter(|entry| entry.path.ends_with("message_pattern.item_format"))
            .count(),
        1
    );
    assert_eq!(
        coverage
            .entries
            .iter()
            .filter(|entry| entry.path.ends_with("message_pattern.separator"))
            .count(),
        1
    );
    assert!(coverage.entries.iter().any(|entry| {
        entry.path == "notes.note.yemen.fields.field.flag-similarity.message.separator"
    }));
    assert!(coverage.entries.iter().any(|entry| {
        entry.path == "notes.note.yemen.fields.field.flag-similarity.message.items.0.description"
    }));
    let context = deck.translation_context(&coverage).unwrap();
    assert!(context.units.iter().any(|unit| {
        unit.path == "notes.note.yemen.fields.field.flag-similarity.message.items.0.description"
            && unit.message.as_ref().is_some_and(|message| {
                message
                    .format
                    .as_ref()
                    .is_some_and(|format| format.source == "{country} ({description})")
                    && message
                        .components
                        .iter()
                        .any(|component| component.name.as_deref() == Some("items.0.description"))
            })
    }));

    let resolved = deck.compose(&[overlay]).unwrap();

    assert_eq!(
        resolved
            .field_text(&sid("note.yemen"), &sid("field.flag-similarity"))
            .unwrap(),
        "Egito（com emblema）, e Iraque（com texto）"
    );
    assert!(matches!(
        resolved.notes[&sid("note.yemen")].fields[&sid("field.flag-similarity")],
        FieldValue::Scalar(_)
    ));
}

#[test]
fn pattern_parameter_context_translates_all_items_with_consuming_and_direct_precedence() {
    let deck = pattern_deck();
    let country_parameter = "note_types.note-type.country.fields.field.flag-similarity.message_pattern.parameters.country";
    let description_parameter = "note_types.note-type.country.fields.field.flag-similarity.message_pattern.parameters.description";
    let translations = TranslationDictionary {
        direct: BTreeMap::from([
            ("Moldova".to_owned(), "Moldávia".to_owned()),
            ("Egypt".to_owned(), "Egito direto".to_owned()),
            ("Iraq".to_owned(), "Iraque direto".to_owned()),
            (
                "wider, coat of arms with eagle".to_owned(),
                "mais larga, brasão com águia".to_owned(),
            ),
            ("with emblem".to_owned(), "com emblema direto".to_owned()),
            ("with text".to_owned(), "com texto direto".to_owned()),
        ]),
        contextual: BTreeMap::from([
            (
                country_parameter.to_owned(),
                BTreeMap::from([
                    ("Egypt".to_owned(), "Egito do parâmetro".to_owned()),
                    ("Iraq".to_owned(), "Iraq".to_owned()),
                ]),
            ),
            (
                description_parameter.to_owned(),
                BTreeMap::from([
                    ("with emblem".to_owned(), "emblema do parâmetro".to_owned()),
                    ("with text".to_owned(), "texto do parâmetro".to_owned()),
                ]),
            ),
            (
                "notes.note.yemen.fields.field.flag-similarity.message.items.0.country".to_owned(),
                BTreeMap::from([("Egypt".to_owned(), "Egito exato".to_owned())]),
            ),
            (
                "notes.note.yemen.fields.field.flag-similarity.message.items.1.description"
                    .to_owned(),
                BTreeMap::from([("with text".to_owned(), "texto exato".to_owned())]),
            ),
        ]),
        ..TranslationDictionary::default()
    };
    let overlay = translation_overlay(translations);

    let coverage = deck.translation_coverage(&overlay).unwrap();
    let item_0_country = coverage
        .entries
        .iter()
        .find(|entry| {
            entry.path == "notes.note.yemen.fields.field.flag-similarity.message.items.0.country"
        })
        .unwrap();
    assert_eq!(
        item_0_country.category,
        TranslationCoverageCategory::ContextualTranslation
    );
    assert_eq!(
        item_0_country.context.as_deref(),
        Some(item_0_country.path.as_str())
    );
    let item_1_country = coverage
        .entries
        .iter()
        .find(|entry| {
            entry.path == "notes.note.yemen.fields.field.flag-similarity.message.items.1.country"
        })
        .unwrap();
    assert_eq!(item_1_country.context.as_deref(), Some(country_parameter));
    assert_eq!(item_1_country.translated.as_deref(), Some("Iraq"));
    let item_0_description = coverage
        .entries
        .iter()
        .find(|entry| {
            entry.path
                == "notes.note.yemen.fields.field.flag-similarity.message.items.0.description"
        })
        .unwrap();
    assert_eq!(
        item_0_description.context.as_deref(),
        Some(description_parameter)
    );

    let resolved = deck.compose(&[overlay]).unwrap();
    assert_eq!(
        resolved
            .field_text(&sid("note.yemen"), &sid("field.flag-similarity"))
            .unwrap(),
        "Egito exato (emblema do parâmetro), Iraq (texto exato)"
    );
    assert_eq!(
        resolved
            .field_text(&sid("note.andorra"), &sid("field.flag-similarity"))
            .unwrap(),
        "Moldávia (mais larga, brasão com águia)"
    );
}

#[test]
fn exact_item_stale_text_wins_over_live_parameter_context_and_is_attributed_to_the_item() {
    let deck = pattern_deck();
    let item_path = "notes.note.yemen.fields.field.flag-similarity.message.items.0.description";
    let parameter_path = "note_types.note-type.country.fields.field.flag-similarity.message_pattern.parameters.description";
    let overlay = translation_overlay(TranslationDictionary {
        contextual: BTreeMap::from([(
            parameter_path.to_owned(),
            BTreeMap::from([("with emblem".to_owned(), "live parameter".to_owned())]),
        )]),
        stale_translations: vec![StaleTranslation {
            old_source: "with an emblem".to_owned(),
            new_source: "with emblem".to_owned(),
            target: "exact stale".to_owned(),
            context: Some(item_path.to_owned()),
        }],
        ..TranslationDictionary::default()
    });

    let coverage = deck.translation_coverage(&overlay).unwrap();
    let entry = coverage
        .entries
        .iter()
        .find(|entry| entry.path == item_path)
        .unwrap();
    assert_eq!(
        entry.category,
        TranslationCoverageCategory::StaleTranslation
    );
    assert_eq!(entry.old_source.as_deref(), Some("with an emblem"));
    assert_eq!(entry.translated.as_deref(), Some("exact stale"));
    assert_eq!(entry.context.as_deref(), Some(item_path));

    let resolved = deck.compose(&[overlay]).unwrap();
    assert_eq!(
        resolved
            .field_text(&sid("note.yemen"), &sid("field.flag-similarity"))
            .unwrap(),
        "Egypt (exact stale), Iraq (with text)"
    );
}

#[test]
fn parameter_stale_note_field_ref_wins_over_direct_and_materializes_while_stale() {
    let deck = pattern_deck();
    let item_path = "notes.note.yemen.fields.field.flag-similarity.message.items.0.country";
    let parameter_path = "note_types.note-type.country.fields.field.flag-similarity.message_pattern.parameters.country";
    let overlay = translation_overlay(TranslationDictionary {
        direct: BTreeMap::from([("Egypt".to_owned(), "direct target".to_owned())]),
        stale_translations: vec![StaleTranslation {
            old_source: "Arab Republic of Egypt".to_owned(),
            new_source: "Egypt".to_owned(),
            target: "parameter stale".to_owned(),
            context: Some(parameter_path.to_owned()),
        }],
        ..TranslationDictionary::default()
    });

    let coverage = deck.translation_coverage(&overlay).unwrap();
    let entry = coverage
        .entries
        .iter()
        .find(|entry| entry.path == item_path)
        .unwrap();
    assert_eq!(
        entry.category,
        TranslationCoverageCategory::StaleTranslation
    );
    assert_eq!(entry.translated.as_deref(), Some("parameter stale"));
    assert_eq!(entry.context.as_deref(), Some(parameter_path));

    let resolved = deck.compose(&[overlay]).unwrap();
    assert_eq!(
        resolved
            .field_text(&sid("note.egypt"), &sid("field.country"))
            .unwrap(),
        "direct target"
    );
    assert_eq!(
        resolved
            .field_text(&sid("note.yemen"), &sid("field.flag-similarity"))
            .unwrap(),
        "parameter stale (with emblem), Iraq (with text)"
    );
}

#[test]
fn parameter_context_wins_over_broad_consuming_ancestors_without_cross_role_leakage() {
    let mut deck = pattern_deck();
    let FieldValue::MessageItems(message) = deck
        .notes
        .get_mut(&sid("note.yemen"))
        .unwrap()
        .fields
        .get_mut(&sid("field.flag-similarity"))
        .unwrap()
    else {
        panic!("expected list message")
    };
    message.items[0].insert(
        "description".to_owned(),
        ListMessageArgument::Scalar("Egypt".to_owned()),
    );
    let country_parameter = "note_types.note-type.country.fields.field.flag-similarity.message_pattern.parameters.country";
    let description_parameter = "note_types.note-type.country.fields.field.flag-similarity.message_pattern.parameters.description";
    let overlay = translation_overlay(TranslationDictionary {
        contextual: BTreeMap::from([
            (
                "notes.note.yemen".to_owned(),
                BTreeMap::from([("Egypt".to_owned(), "broad note".to_owned())]),
            ),
            (
                "notes.note.yemen.fields.field.flag-similarity".to_owned(),
                BTreeMap::from([("Egypt".to_owned(), "broad field".to_owned())]),
            ),
            (
                country_parameter.to_owned(),
                BTreeMap::from([("Egypt".to_owned(), "country role".to_owned())]),
            ),
            (
                description_parameter.to_owned(),
                BTreeMap::from([("Egypt".to_owned(), "description role".to_owned())]),
            ),
        ]),
        ..TranslationDictionary::default()
    });

    let resolved = deck.compose(&[overlay]).unwrap();
    assert_eq!(
        resolved
            .field_text(&sid("note.yemen"), &sid("field.flag-similarity"))
            .unwrap(),
        "country role (description role), Iraq (with text)"
    );
}

#[test]
fn parameter_context_is_scoped_to_its_pattern_field() {
    let mut deck = pattern_deck();
    let second_field_id = sid("field.other-similarity");
    let mut second_pattern = country_pattern().unwrap();
    second_pattern.item_format = "other {description}: {country}".to_owned();
    deck.note_types
        .get_mut(&sid("note-type.country"))
        .unwrap()
        .fields
        .push(FieldDefinition {
            id: second_field_id.clone(),
            name: "Other similarity".to_owned(),
            message_pattern: Some(second_pattern),
        });
    for note in deck.notes.values_mut() {
        note.fields
            .insert(second_field_id.clone(), FieldValue::Scalar(String::new()));
    }
    deck.notes
        .get_mut(&sid("note.andorra"))
        .unwrap()
        .fields
        .insert(
            second_field_id.clone(),
            list_message(&[("note.moldova", "shared text")]),
        );
    let FieldValue::MessageItems(first_message) = deck
        .notes
        .get_mut(&sid("note.andorra"))
        .unwrap()
        .fields
        .get_mut(&sid("field.flag-similarity"))
        .unwrap()
    else {
        panic!("expected list message")
    };
    first_message.items[0].insert(
        "description".to_owned(),
        ListMessageArgument::Scalar("shared text".to_owned()),
    );
    let overlay = translation_overlay(TranslationDictionary {
        direct: BTreeMap::from([("shared text".to_owned(), "direct second field".to_owned())]),
        contextual: BTreeMap::from([(
            "note_types.note-type.country.fields.field.flag-similarity.message_pattern.parameters.description"
                .to_owned(),
            BTreeMap::from([("shared text".to_owned(), "first field only".to_owned())]),
        )]),
        ..TranslationDictionary::default()
    });

    let resolved = deck.compose(&[overlay]).unwrap();
    assert_eq!(
        resolved
            .field_text(&sid("note.andorra"), &sid("field.flag-similarity"))
            .unwrap(),
        "Moldova (first field only)"
    );
    assert_eq!(
        resolved
            .field_text(&sid("note.andorra"), &second_field_id)
            .unwrap(),
        "other direct second field: Moldova"
    );
}

#[test]
fn unused_pattern_parameter_context_is_reported_as_stale_and_invalid() {
    let deck = pattern_deck();
    let parameter_path = "note_types.note-type.country.fields.field.flag-similarity.message_pattern.parameters.country";
    let overlay = translation_overlay(TranslationDictionary {
        contextual: BTreeMap::from([(
            parameter_path.to_owned(),
            BTreeMap::from([("Missing country".to_owned(), "País ausente".to_owned())]),
        )]),
        ..TranslationDictionary::default()
    });

    let coverage = deck.translation_coverage(&overlay).unwrap();
    assert!(coverage.entries.iter().any(|entry| {
        entry.category == TranslationCoverageCategory::StaleContextualKey
            && entry.context.as_deref() == Some(parameter_path)
            && entry.source == "Missing country"
    }));
    let errors = deck.compose(&[overlay]).unwrap_err();
    assert!(errors.errors.iter().any(|error| {
        error.path.contains(parameter_path)
            && error
                .message
                .contains("invalid contextual translation: source \"Missing country\"")
    }));
}

#[test]
fn pattern_parameter_context_is_a_typed_deck_path() {
    let path = "note_types.note-type.country.fields.field.flag-similarity.message_pattern.parameters.country";
    assert_eq!(
        path.parse::<DeckPath>().unwrap(),
        DeckPath::NoteTypeFieldMessagePatternParameter {
            note_type_id: sid("note-type.country"),
            field_id: sid("field.flag-similarity"),
            parameter: "country".to_owned(),
        }
    );
}

#[test]
fn consuming_contextual_ref_translation_wins_over_conflicting_direct_translation() {
    let deck = pattern_deck();
    let translations = TranslationDictionary {
        direct: BTreeMap::from([("Moldova".to_owned(), "Moldavien".to_owned())]),
        contextual: BTreeMap::from([(
            "notes.note.andorra.fields.field.flag-similarity.message.items.0.country".to_owned(),
            BTreeMap::from([("Moldova".to_owned(), "Moldova".to_owned())]),
        )]),
        ..TranslationDictionary::default()
    };

    let resolved = deck.compose(&[translation_overlay(translations)]).unwrap();

    assert_eq!(
        resolved
            .field_text(&sid("note.moldova"), &sid("field.country"))
            .unwrap(),
        "Moldavien"
    );
    assert_eq!(
        resolved
            .field_text(&sid("note.andorra"), &sid("field.flag-similarity"))
            .unwrap(),
        "Moldova (wider, coat of arms with eagle)"
    );
}

#[test]
fn explicit_text_argument_for_ref_parameter_is_translated_without_a_dependency() {
    let mut deck = pattern_deck();
    deck.notes
        .get_mut(&sid("note.andorra"))
        .unwrap()
        .fields
        .insert(
            sid("field.flag-similarity"),
            FieldValue::MessageItems(ListMessageItems::new(vec![BTreeMap::from([
                (
                    "country".to_owned(),
                    ListMessageArgument::Text("Sierra Leone".to_owned()),
                ),
                (
                    "description".to_owned(),
                    ListMessageArgument::Scalar("slightly lighter blue".to_owned()),
                ),
            ])])),
        );
    let translations = TranslationDictionary {
        direct: BTreeMap::from([(
            "slightly lighter blue".to_owned(),
            "azul ligeramente más claro".to_owned(),
        )]),
        contextual: BTreeMap::from([(
            "note_types.note-type.country.fields.field.flag-similarity.message_pattern.parameters.country"
                .to_owned(),
            BTreeMap::from([("Sierra Leone".to_owned(), "Sierra Leona".to_owned())]),
        )]),
        ..TranslationDictionary::default()
    };

    deck.validate()
        .expect("explicit text has no missing-note dependency");
    let resolved = deck.compose(&[translation_overlay(translations)]).unwrap();
    assert_eq!(
        resolved
            .field_text(&sid("note.andorra"), &sid("field.flag-similarity"))
            .unwrap(),
        "Sierra Leona (azul ligeramente más claro)"
    );
}

#[test]
fn list_message_pattern_rejects_empty_items_missing_arguments_and_missing_references() {
    let mut deck = pattern_deck();
    let FieldValue::MessageItems(message) = deck
        .notes
        .get_mut(&sid("note.andorra"))
        .unwrap()
        .fields
        .get_mut(&sid("field.flag-similarity"))
        .unwrap()
    else {
        panic!("expected list message")
    };
    message.items.clear();
    let report = deck.validate().unwrap_err();
    assert!(
        report
            .errors
            .iter()
            .any(|error| error.message.contains("must contain at least one item"))
    );

    let mut deck = pattern_deck();
    let FieldValue::MessageItems(message) = deck
        .notes
        .get_mut(&sid("note.andorra"))
        .unwrap()
        .fields
        .get_mut(&sid("field.flag-similarity"))
        .unwrap()
    else {
        panic!("expected list message")
    };
    message.items[0].remove("description");
    let report = deck.validate().unwrap_err();
    assert!(report.errors.iter().any(|error| {
        error
            .message
            .contains("must exactly match declared parameters")
            && error.path.contains("note.andorra")
    }));

    let mut deck = pattern_deck();
    let FieldValue::MessageItems(message) = deck
        .notes
        .get_mut(&sid("note.andorra"))
        .unwrap()
        .fields
        .get_mut(&sid("field.flag-similarity"))
        .unwrap()
    else {
        panic!("expected list message")
    };
    message.items[0].insert(
        "country".to_owned(),
        ListMessageArgument::Scalar("note.missing".to_owned()),
    );
    let report = deck.validate().unwrap_err();
    assert!(report.errors.iter().any(|error| {
        error.message.contains("missing note note.missing")
            && error.path.ends_with("message.items.0.country")
    }));

    let mut deck = pattern_deck();
    let FieldValue::MessageItems(message) = deck
        .notes
        .get_mut(&sid("note.andorra"))
        .unwrap()
        .fields
        .get_mut(&sid("field.flag-similarity"))
        .unwrap()
    else {
        panic!("expected list message")
    };
    message.items[0].insert(
        "description".to_owned(),
        ListMessageArgument::Text("redundant explicit text".to_owned()),
    );
    let report = deck.validate().unwrap_err();
    assert!(report.errors.iter().any(|error| {
        error.path.ends_with("message.items.0.description")
            && error
                .message
                .contains("explicit `text` is only valid as an escape hatch")
    }));
}

#[test]
fn list_message_patterns_and_ordered_items_participate_in_fingerprints_and_diff() {
    let deck = pattern_deck();
    let mut changed_pattern = deck.clone();
    changed_pattern
        .note_types
        .get_mut(&sid("note-type.country"))
        .unwrap()
        .fields
        .iter_mut()
        .find(|field| field.id == sid("field.flag-similarity"))
        .unwrap()
        .message_pattern
        .as_mut()
        .unwrap()
        .separator = " / ".to_owned();
    assert_ne!(
        fingerprint_note_type(&deck.note_types[&sid("note-type.country")]),
        fingerprint_note_type(&changed_pattern.note_types[&sid("note-type.country")])
    );
    assert!(deck.semantic_diff(&changed_pattern).has_change(
        SemanticChangeKind::Modified,
        "note_types.note-type.country.fields.field.flag-similarity.message_pattern"
    ));

    let mut reordered = deck.clone();
    let FieldValue::MessageItems(message) = reordered
        .notes
        .get_mut(&sid("note.yemen"))
        .unwrap()
        .fields
        .get_mut(&sid("field.flag-similarity"))
        .unwrap()
    else {
        panic!("expected list message")
    };
    message.items.reverse();
    assert_ne!(
        fingerprint_note(&deck.notes[&sid("note.yemen")]),
        fingerprint_note(&reordered.notes[&sid("note.yemen")])
    );
    assert!(deck.semantic_diff(&reordered).has_change(
        SemanticChangeKind::Modified,
        "notes.note.yemen.fields.field.flag-similarity.message"
    ));

    let mut explicit_text = deck.clone();
    let FieldValue::MessageItems(message) = explicit_text
        .notes
        .get_mut(&sid("note.andorra"))
        .unwrap()
        .fields
        .get_mut(&sid("field.flag-similarity"))
        .unwrap()
    else {
        panic!("expected list message")
    };
    message.items[0].insert(
        "country".to_owned(),
        ListMessageArgument::Text("note.moldova".to_owned()),
    );
    assert_ne!(
        fingerprint_note(&deck.notes[&sid("note.andorra")]),
        fingerprint_note(&explicit_text.notes[&sid("note.andorra")]),
        "explicit text and concise typed references are distinct semantic arguments"
    );
}

#[test]
fn list_message_pattern_declaration_rejects_placeholder_mismatch_without_an_invocation() {
    let mut deck = pattern_deck();
    let pattern = deck
        .note_types
        .get_mut(&sid("note-type.country"))
        .unwrap()
        .fields
        .iter_mut()
        .find(|field| field.id == sid("field.flag-similarity"))
        .unwrap()
        .message_pattern
        .as_mut()
        .unwrap();
    pattern.item_format = "{country}".to_owned();
    for note in deck.notes.values_mut() {
        note.fields.insert(
            sid("field.flag-similarity"),
            FieldValue::Scalar(String::new()),
        );
    }

    let report = deck.validate().unwrap_err();
    assert!(report.errors.iter().any(|error| {
        error
            .path
            .ends_with("field.flag-similarity.message_pattern")
            && error
                .message
                .contains("must exactly match declared parameters")
    }));
}

#[test]
fn list_message_pattern_participates_in_cycle_detection() {
    let mut deck = pattern_deck();
    let country_field = deck
        .note_types
        .get_mut(&sid("note-type.country"))
        .unwrap()
        .fields
        .iter_mut()
        .find(|field| field.id == sid("field.country"))
        .unwrap();
    country_field.message_pattern = country_pattern();
    deck.notes
        .get_mut(&sid("note.egypt"))
        .unwrap()
        .fields
        .insert(
            sid("field.country"),
            list_message(&[("note.iraq", "points onward")]),
        );
    deck.notes
        .get_mut(&sid("note.iraq"))
        .unwrap()
        .fields
        .insert(
            sid("field.country"),
            list_message(&[("note.egypt", "points back")]),
        );

    let report = deck.validate().unwrap_err();
    assert!(
        report
            .errors
            .iter()
            .any(|error| error.message.contains("dependency cycle"))
    );
}

fn pattern_deck() -> CanonicalDeck {
    let fields = vec![
        FieldDefinition {
            id: sid("field.country"),
            name: "Country".to_owned(),
            message_pattern: None,
        },
        FieldDefinition {
            id: sid("field.flag-similarity"),
            name: "Flag similarity".to_owned(),
            message_pattern: country_pattern(),
        },
    ];
    let notes = [
        ("note.moldova", "Moldova", FieldValue::Scalar(String::new())),
        ("note.egypt", "Egypt", FieldValue::Scalar(String::new())),
        ("note.iraq", "Iraq", FieldValue::Scalar(String::new())),
        (
            "note.andorra",
            "Andorra",
            list_message(&[("note.moldova", "wider, coat of arms with eagle")]),
        ),
        (
            "note.yemen",
            "Yemen",
            list_message(&[("note.egypt", "with emblem"), ("note.iraq", "with text")]),
        ),
    ]
    .into_iter()
    .map(|(id, country, similarity)| {
        (
            sid(id),
            Note {
                id: sid(id),
                note_type_id: sid("note-type.country"),
                variables: BTreeMap::new(),
                fields: FieldMap::from_iter([
                    (sid("field.country"), FieldValue::Scalar(country.to_owned())),
                    (sid("field.flag-similarity"), similarity),
                ]),
                tags: BTreeSet::new(),
                adapter_ids: AdapterIds::new(),
            },
        )
    })
    .collect();
    CanonicalDeck {
        id: sid("deck.pattern"),
        name: "Pattern".to_owned(),
        description: String::new(),
        variables: BTreeMap::new(),
        note_types: BTreeMap::from([(
            sid("note-type.country"),
            NoteType {
                id: sid("note-type.country"),
                name: "Country".to_owned(),
                variables: BTreeMap::new(),
                fields,
                card_templates: Vec::new(),
                styling: String::new(),
                adapter_ids: AdapterIds::new(),
            },
        )]),
        notes,
        media: BTreeMap::new(),
        tombstones: Tombstones::default(),
        adapter_ids: AdapterIds::new(),
    }
}

fn country_pattern() -> Option<ListMessagePattern> {
    Some(ListMessagePattern {
        item_format: "{country} ({description})".to_owned(),
        separator: ", ".to_owned(),
        parameters: BTreeMap::from([
            (
                "country".to_owned(),
                ListMessageParameter::NoteFieldRef {
                    field_id: sid("field.country"),
                },
            ),
            ("description".to_owned(), ListMessageParameter::Text),
        ]),
    })
}

fn list_message(items: &[(&str, &str)]) -> FieldValue {
    FieldValue::MessageItems(ListMessageItems::new(
        items
            .iter()
            .map(|(country, description)| {
                BTreeMap::from([
                    (
                        "country".to_owned(),
                        ListMessageArgument::Scalar((*country).to_owned()),
                    ),
                    (
                        "description".to_owned(),
                        ListMessageArgument::Scalar((*description).to_owned()),
                    ),
                ])
            })
            .collect(),
    ))
}

fn translation_overlay(translations: TranslationDictionary) -> Overlay {
    Overlay {
        id: sid("overlay.translation.pt"),
        kind: OverlayKind::Translation,
        translations: Some(translations),
        deck_change: None,
        note_changes: BTreeMap::new(),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    }
}

fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}
