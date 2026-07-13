use std::collections::{BTreeMap, BTreeSet};

use brain_brew_core::{
    AdapterIdChange, AdapterIds, CanonicalDeck, CardTemplate, CardTemplateChange, CardTemplateSide,
    ChangeIntent, ComposeErrorKind, DeckChange, ExpectedBase, FieldChange, FieldDefinition,
    FieldDefinitionChange, FieldImageReference, FieldValue, MediaChange, MediaReference,
    MessageComponent, Note, NoteChange, NoteType, NoteTypeChange, Overlay, OverlayKind,
    PropertyChange, StableId, StaleTranslation, StructuredMessage, TagChange, TargetAdaptation,
    TombstoneAddress, Tombstones, TranslationCoverageCategory, TranslationDictionary,
    ValidationErrorKind, fingerprint_card_template, fingerprint_field_definition,
    fingerprint_media_reference, fingerprint_note, fingerprint_note_type,
};

fn target_adaptation(
    expected_source: impl Into<String>,
    target: impl Into<String>,
) -> TargetAdaptation {
    TargetAdaptation {
        expected_source: expected_source.into(),
        target: target.into(),
        reason: None,
    }
}

#[test]
fn add_overlay_adds_a_new_note_to_the_resolved_deck() {
    let base = ug_style_deck();
    let overlay = Overlay {
        id: sid("overlay.extension.nordics"),
        kind: OverlayKind::Extension,
        translations: None,
        deck_change: None,
        note_changes: BTreeMap::from([(
            sid("note.sweden"),
            NoteChange {
                intent: ChangeIntent::Add,
                note: Some(sweden_note()),
                variables: BTreeMap::new(),
                fields: BTreeMap::new(),
                tags: BTreeMap::new(),
                adapter_ids: BTreeMap::new(),
                expected_base: None,
            },
        )]),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    };

    let resolved = base.compose(&[overlay]).expect("overlay composes");

    assert!(resolved.notes.contains_key(&sid("note.finland")));
    assert_eq!(
        resolved
            .notes
            .get(&sid("note.sweden"))
            .unwrap()
            .fields
            .get(&sid("field.capital"))
            .and_then(FieldValue::as_scalar),
        Some("Stockholm")
    );
}

#[test]
fn extension_overlay_can_add_a_note_type_and_notes_using_it() {
    let base = ug_style_deck();
    let region_note_type = NoteType {
        id: sid("note-type.region"),
        name: "Geography Region".to_owned(),
        variables: BTreeMap::from([("label.region".to_owned(), "Region".to_owned())]),
        fields: vec![
            FieldDefinition {
                id: sid("field.region"),
                name: "Region".to_owned(),
            },
            FieldDefinition {
                id: sid("field.map"),
                name: "Map".to_owned(),
            },
        ],
        card_templates: vec![CardTemplate {
            id: sid("template.region-map"),
            name: "Region - Map".to_owned(),
            variables: BTreeMap::new(),
            question_format: "{{Region}}".to_owned(),
            answer_format: "{{Map}}".to_owned(),
            adapter_ids: AdapterIds::new(),
        }],
        styling: ".card { font-family: sans-serif; }\n".to_owned(),
        adapter_ids: AdapterIds::new(),
    };
    let overlay = Overlay {
        id: sid("overlay.extension.regions"),
        kind: OverlayKind::Extension,
        translations: None,
        deck_change: None,
        note_type_changes: BTreeMap::from([(
            sid("note-type.region"),
            NoteTypeChange {
                intent: ChangeIntent::Add,
                note_type: Some(region_note_type),
                name: None,
                variables: BTreeMap::new(),
                styling: None,
                fields: BTreeMap::new(),
                card_templates: BTreeMap::new(),
                adapter_ids: BTreeMap::new(),
                expected_base: None,
            },
        )]),
        note_changes: BTreeMap::from([(
            sid("note.europe"),
            NoteChange {
                intent: ChangeIntent::Add,
                note: Some(Note {
                    id: sid("note.europe"),
                    note_type_id: sid("note-type.region"),
                    variables: BTreeMap::new(),
                    fields: BTreeMap::from([
                        (sid("field.region"), "Europe".to_owned()),
                        (sid("field.map"), "<img src=\"europe.png\" />".to_owned()),
                    ])
                    .into(),
                    tags: BTreeSet::from(["UG::Europe".to_owned()]),
                    adapter_ids: AdapterIds::new(),
                }),
                variables: BTreeMap::new(),
                fields: BTreeMap::new(),
                tags: BTreeMap::new(),
                adapter_ids: BTreeMap::new(),
                expected_base: None,
            },
        )]),
        media_changes: BTreeMap::new(),
    };

    let resolved = base.compose(&[overlay]).expect("extension composes");

    assert!(resolved.note_types.contains_key(&sid("note-type.region")));
    assert_eq!(
        resolved.notes[&sid("note.europe")].note_type_id,
        sid("note-type.region")
    );
}

#[test]
fn replace_field_requires_expected_base() {
    let base = ug_style_deck();
    let overlay = overlay_replacing_capital(ChangeIntent::Replace, None, "Helsingfors");

    let report = base
        .compose(&[overlay])
        .expect_err("replace without expected base must fail");

    assert!(report.has_kind(ComposeErrorKind::MissingExpectedBase));
    assert!(
        report
            .errors
            .iter()
            .any(|error| error.path == "notes.note.finland.fields.field.capital")
    );
}

#[test]
fn replace_field_succeeds_when_expected_base_matches_current_value() {
    let base = ug_style_deck();
    let overlay = overlay_replacing_capital(
        ChangeIntent::Replace,
        Some(ExpectedBase::Value("Helsinki".to_owned())),
        "Helsingfors",
    );

    let resolved = base.compose(&[overlay]).expect("expected base matches");

    assert_eq!(
        resolved
            .notes
            .get(&sid("note.finland"))
            .unwrap()
            .fields
            .get(&sid("field.capital"))
            .and_then(FieldValue::as_scalar),
        Some("Helsingfors")
    );
}

#[test]
fn ordered_overlay_stack_reports_conflicts_without_explicit_override() {
    let base = ug_style_deck();
    let first = overlay_replacing_capital(
        ChangeIntent::Replace,
        Some(ExpectedBase::Value("Helsinki".to_owned())),
        "Helsingfors",
    );
    let second = overlay_replacing_capital(
        ChangeIntent::Replace,
        Some(ExpectedBase::Value("Helsingfors".to_owned())),
        "Helsinki, Helsingfors",
    );

    let report = base
        .compose(&[first, second])
        .expect_err("two non-override changes to the same path conflict");

    assert!(report.has_kind(ComposeErrorKind::Conflict));
    assert!(
        report
            .errors
            .iter()
            .any(|error| error.path == "notes.note.finland.fields.field.capital")
    );
}

#[test]
fn later_override_can_intentionally_replace_an_earlier_overlay_change() {
    let base = ug_style_deck();
    let first = overlay_replacing_capital(
        ChangeIntent::Replace,
        Some(ExpectedBase::Value("Helsinki".to_owned())),
        "Helsingfors",
    );
    let second = overlay_replacing_capital(
        ChangeIntent::Override,
        Some(ExpectedBase::Value("Helsingfors".to_owned())),
        "Helsinki / Helsingfors",
    );

    let resolved = base
        .compose(&[first, second])
        .expect("override resolves conflict explicitly");

    assert_eq!(
        resolved
            .notes
            .get(&sid("note.finland"))
            .unwrap()
            .fields
            .get(&sid("field.capital"))
            .and_then(FieldValue::as_scalar),
        Some("Helsinki / Helsingfors")
    );
}

#[test]
fn translation_dictionary_reports_stale_direct_entries() {
    let deck = ug_style_deck();
    let overlay = Overlay {
        id: sid("overlay.translation.da"),
        kind: OverlayKind::Translation,
        translations: Some(TranslationDictionary {
            direct: BTreeMap::from([("Missing source".to_owned(), "Mangler".to_owned())]),
            contextual: BTreeMap::new(),
            no_change: BTreeSet::new(),
            target_adaptations: BTreeMap::new(),
            stale_translations: Vec::new(),
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

    let report = deck.compose(&[overlay]).expect_err("stale entry fails");

    assert!(report.has_kind(ComposeErrorKind::StaleTranslationEntry));
    assert!(
        report.errors[0]
            .message
            .contains("stale direct translation source \"Missing source\"")
    );
}

#[test]
fn translation_dictionary_distinguishes_direct_contextual_and_target_adaptations() {
    let mut base = ug_style_deck();
    base.notes
        .get_mut(&sid("note.finland"))
        .unwrap()
        .fields
        .insert(sid("field.country"), "Shared source".to_owned());
    let mut sweden = sweden_note();
    sweden
        .fields
        .insert(sid("field.country"), "Shared source".to_owned());
    sweden.fields.insert(sid("field.capital"), String::new());
    sweden
        .fields
        .insert(sid("field.flag"), "Shared source".to_owned());
    base.notes.insert(sid("note.sweden"), sweden);
    let overlay = Overlay {
        id: sid("overlay.translation.da"),
        kind: OverlayKind::Translation,
        translations: Some(TranslationDictionary {
            direct: BTreeMap::from([
                ("Helsinki".to_owned(), "Helsingfors".to_owned()),
                ("Shared source".to_owned(), "Direkte standard".to_owned()),
            ]),
            contextual: BTreeMap::from([
                (
                    "notes.note.finland".to_owned(),
                    BTreeMap::from([("Shared source".to_owned(), "Finsk kontekst".to_owned())]),
                ),
                (
                    "notes.note.sweden.fields.field.country".to_owned(),
                    BTreeMap::from([("Shared source".to_owned(), "Svensk kontekst".to_owned())]),
                ),
            ]),
            no_change: BTreeSet::new(),
            target_adaptations: BTreeMap::from([(
                "notes.note.sweden.fields.field.capital".to_owned(),
                target_adaptation("", "Stockholm"),
            )]),
            stale_translations: Vec::new(),
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

    let resolved = base.compose(&[overlay]).expect("translations compose");

    assert_eq!(
        resolved.notes[&sid("note.finland")].fields[&sid("field.capital")],
        "Helsingfors"
    );
    assert_eq!(
        resolved.notes[&sid("note.finland")].fields[&sid("field.country")],
        "Finsk kontekst"
    );
    assert_eq!(
        resolved.notes[&sid("note.sweden")].fields[&sid("field.country")],
        "Svensk kontekst"
    );
    assert_eq!(
        resolved.notes[&sid("note.sweden")].fields[&sid("field.capital")],
        "Stockholm"
    );
    assert_eq!(
        resolved.notes[&sid("note.sweden")].fields[&sid("field.flag")],
        "Direkte standard"
    );
}

#[test]
fn stale_translation_applies_target_text_and_reports_review_debt() {
    let mut base = ug_style_deck();
    base.notes
        .get_mut(&sid("note.finland"))
        .unwrap()
        .fields
        .insert(sid("field.capital"), "Helsinki City".to_owned());
    let overlay = Overlay {
        id: sid("overlay.translation.da"),
        kind: OverlayKind::Translation,
        translations: Some(TranslationDictionary {
            direct: BTreeMap::from([("Finland".to_owned(), "Finland".to_owned())]),
            contextual: BTreeMap::new(),
            no_change: BTreeSet::new(),
            target_adaptations: BTreeMap::new(),
            stale_translations: vec![StaleTranslation {
                old_source: "Helsinki".to_owned(),
                new_source: "Helsinki City".to_owned(),
                target: "Helsingfors".to_owned(),
                context: None,
            }],
            variables: BTreeMap::new(),
            adapter_ids: BTreeMap::new(),
            require_complete: true,
            ignore_paths: BTreeSet::from([
                "deck.*".to_owned(),
                "note_types.*".to_owned(),
                "notes.*.fields.field.flag".to_owned(),
                "notes.*.tags.*".to_owned(),
            ]),
        }),
        deck_change: None,
        note_changes: BTreeMap::new(),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    };

    let report = base
        .translation_coverage(&overlay)
        .expect("translation overlay has coverage");
    assert!(report.entries.iter().any(|entry| {
        entry.category == TranslationCoverageCategory::StaleTranslation
            && entry.path == "notes.note.finland.fields.field.capital"
            && entry.old_source.as_deref() == Some("Helsinki")
            && entry.source == "Helsinki City"
            && entry.translated.as_deref() == Some("Helsingfors")
    }));
    assert!(!report.entries.iter().any(|entry| {
        entry.category == TranslationCoverageCategory::UntranslatedFallback
            && entry.path == "notes.note.finland.fields.field.capital"
    }));

    let resolved = base
        .compose(&[overlay])
        .expect("stale translations compose as warnings");
    assert_eq!(
        resolved.notes[&sid("note.finland")].fields[&sid("field.capital")],
        "Helsingfors"
    );
}

#[test]
fn contextual_stale_translation_only_applies_under_its_context() {
    let mut base = ug_style_deck();
    base.notes
        .get_mut(&sid("note.finland"))
        .unwrap()
        .fields
        .insert(sid("field.capital"), "Capital city".to_owned());
    let mut sweden = sweden_note();
    sweden
        .fields
        .insert(sid("field.capital"), "Capital city".to_owned());
    base.notes.insert(sid("note.sweden"), sweden);
    let overlay = Overlay {
        id: sid("overlay.translation.da"),
        kind: OverlayKind::Translation,
        translations: Some(TranslationDictionary {
            direct: BTreeMap::new(),
            contextual: BTreeMap::new(),
            no_change: BTreeSet::new(),
            target_adaptations: BTreeMap::new(),
            stale_translations: vec![StaleTranslation {
                old_source: "Helsinki".to_owned(),
                new_source: "Capital city".to_owned(),
                target: "Helsingfors".to_owned(),
                context: Some("notes.note.finland".to_owned()),
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

    let report = base
        .translation_coverage(&overlay)
        .expect("translation overlay has coverage");
    assert!(report.entries.iter().any(|entry| {
        entry.category == TranslationCoverageCategory::StaleTranslation
            && entry.context.as_deref() == Some("notes.note.finland")
            && entry.path == "notes.note.finland.fields.field.capital"
    }));
    assert!(report.entries.iter().any(|entry| {
        entry.category == TranslationCoverageCategory::UntranslatedFallback
            && entry.path == "notes.note.sweden.fields.field.capital"
    }));

    let resolved = base
        .compose(&[overlay])
        .expect("contextual stale translation composes");
    assert_eq!(
        resolved.notes[&sid("note.finland")].fields[&sid("field.capital")],
        "Helsingfors"
    );
    assert_eq!(
        resolved.notes[&sid("note.sweden")].fields[&sid("field.capital")],
        "Capital city"
    );
}

#[test]
fn resolving_stale_translations_migrates_to_normal_dictionary_entries() {
    let mut translations = TranslationDictionary {
        direct: BTreeMap::new(),
        contextual: BTreeMap::new(),
        no_change: BTreeSet::new(),
        target_adaptations: BTreeMap::new(),
        stale_translations: vec![
            StaleTranslation {
                old_source: "Helsinki".to_owned(),
                new_source: "Helsinki City".to_owned(),
                target: "Helsingfors".to_owned(),
                context: None,
            },
            StaleTranslation {
                old_source: "Capital".to_owned(),
                new_source: "Capital city".to_owned(),
                target: "Hovedstad".to_owned(),
                context: Some("notes.note.finland".to_owned()),
            },
        ],
        variables: BTreeMap::new(),
        adapter_ids: BTreeMap::new(),
        require_complete: false,
        ignore_paths: BTreeSet::new(),
    };

    let direct = translations
        .resolve_stale_translation_decision("Helsinki", "Helsinki City", None, None)
        .expect("direct stale translation resolves");
    assert_eq!(direct.target, "Helsingfors");
    assert_eq!(translations.direct["Helsinki City"], "Helsingfors");

    translations
        .resolve_stale_translation_decision(
            "Capital",
            "Capital city",
            Some("notes.note.finland"),
            None,
        )
        .expect("contextual stale translation resolves");
    assert_eq!(
        translations.contextual["notes.note.finland"]["Capital city"],
        "Hovedstad"
    );
    assert!(translations.stale_translations.is_empty());
}

#[test]
fn translation_dictionary_can_translate_tags() {
    let base = ug_style_deck();
    let overlay = Overlay {
        id: sid("overlay.translation.da"),
        kind: OverlayKind::Translation,
        translations: Some(TranslationDictionary {
            direct: BTreeMap::from([("Nordic".to_owned(), "UG::Nordic".to_owned())]),
            contextual: BTreeMap::from([(
                "notes.note.finland".to_owned(),
                BTreeMap::from([("Europe".to_owned(), "UG::Europe".to_owned())]),
            )]),
            no_change: BTreeSet::new(),
            target_adaptations: BTreeMap::new(),
            stale_translations: Vec::new(),
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

    let resolved = base.compose(&[overlay]).expect("tag translations compose");
    let note = &resolved.notes[&sid("note.finland")];

    assert!(note.tags.contains("UG::Europe"));
    assert!(note.tags.contains("UG::Nordic"));
    assert!(!note.tags.contains("Europe"));
    assert!(!note.tags.contains("Nordic"));
}

#[test]
fn translation_dictionary_keeps_field_identifiers_structural_and_translates_display_variables() {
    let mut base = ug_style_deck();
    base.note_types
        .get_mut(&sid("note-type.country"))
        .unwrap()
        .variables
        .insert("label.capital".to_owned(), "Capital".to_owned());
    let overlay = Overlay {
        id: sid("overlay.translation.da"),
        kind: OverlayKind::Translation,
        translations: Some(TranslationDictionary {
            variables: BTreeMap::from([(
                "label.capital".to_owned(),
                BTreeMap::from([("Capital".to_owned(), "Hovedstad".to_owned())]),
            )]),
            ..TranslationDictionary::default()
        }),
        deck_change: None,
        note_changes: BTreeMap::new(),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    };

    let resolved = base
        .compose(std::slice::from_ref(&overlay))
        .expect("translation composes");
    let note_type = &resolved.note_types[&sid("note-type.country")];
    assert_eq!(
        note_type
            .fields
            .iter()
            .find(|field| field.id == sid("field.capital"))
            .unwrap()
            .name,
        "Capital"
    );
    assert_eq!(note_type.variables["label.capital"], "Hovedstad");

    let coverage = base
        .translation_coverage(&overlay)
        .expect("coverage reports");
    assert!(
        coverage.entries.iter().all(|entry| {
            entry.path != "note_types.note-type.country.fields.field.capital.name"
        })
    );
}

#[test]
fn translation_dictionary_no_change_counts_as_reviewed_without_modifying_output() {
    let base = ug_style_deck();
    let overlay = Overlay {
        id: sid("overlay.translation.da"),
        kind: OverlayKind::Translation,
        translations: Some(TranslationDictionary {
            direct: BTreeMap::new(),
            contextual: BTreeMap::new(),
            no_change: BTreeSet::from(["Finland".to_owned(), "Helsinki".to_owned()]),
            target_adaptations: BTreeMap::new(),
            stale_translations: Vec::new(),
            variables: BTreeMap::new(),
            adapter_ids: BTreeMap::new(),
            require_complete: true,
            ignore_paths: BTreeSet::from([
                "deck.*".to_owned(),
                "note_types.*".to_owned(),
                "notes.*.fields.field.flag".to_owned(),
                "notes.*.tags.*".to_owned(),
            ]),
        }),
        deck_change: None,
        note_changes: BTreeMap::new(),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    };

    let resolved = base
        .compose(std::slice::from_ref(&overlay))
        .expect("no-change covers strict translation checks");
    assert_eq!(resolved, base, "no-change must not modify composed output");

    let report = base
        .translation_coverage(&overlay)
        .expect("translation overlay has coverage");
    assert!(!report.has_untranslated_fallbacks());
    assert!(report.entries.iter().any(|entry| {
        entry.category == TranslationCoverageCategory::NoChange
            && entry.path == "notes.note.finland.fields.field.country"
            && entry.source == "Finland"
    }));
    assert!(report.entries.iter().any(|entry| {
        entry.category == TranslationCoverageCategory::NoChange
            && entry.path == "notes.note.finland.fields.field.capital"
            && entry.source == "Helsinki"
    }));
}

#[test]
fn translation_context_identifies_note_field_card_and_duplicate_status() {
    let mut base = ug_style_deck();
    base.notes.insert(sid("note.sweden"), sweden_note());
    let overlay = Overlay {
        id: sid("overlay.translation.da"),
        kind: OverlayKind::Translation,
        translations: Some(TranslationDictionary {
            direct: BTreeMap::from([("Europe".to_owned(), "Europa".to_owned())]),
            contextual: BTreeMap::new(),
            no_change: BTreeSet::new(),
            target_adaptations: BTreeMap::new(),
            stale_translations: Vec::new(),
            variables: BTreeMap::new(),
            adapter_ids: BTreeMap::new(),
            require_complete: false,
            ignore_paths: BTreeSet::from([
                "deck.*".to_owned(),
                "note_types.*".to_owned(),
                "notes.*.fields.field.flag".to_owned(),
            ]),
        }),
        deck_change: None,
        note_changes: BTreeMap::new(),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    };

    let report = base
        .translation_coverage(&overlay)
        .expect("translation overlay has coverage");
    let context = base
        .translation_context(&report)
        .expect("translation context field graph resolves");
    let finland = context
        .units
        .iter()
        .find(|unit| unit.path == "notes.note.finland.fields.field.country")
        .expect("Finland field has context");

    assert_eq!(
        finland.category,
        TranslationCoverageCategory::UntranslatedFallback
    );
    assert_eq!(finland.note_id.as_ref().unwrap().as_str(), "note.finland");
    assert_eq!(
        finland.note_type_id.as_ref().unwrap().as_str(),
        "note-type.country"
    );
    assert_eq!(finland.field_id.as_ref().unwrap().as_str(), "field.country");
    assert_eq!(finland.field_name.as_deref(), Some("Country"));
    assert_eq!(finland.note_fields.len(), 3);
    assert_eq!(finland.note_fields[0].field_id.as_str(), "field.country");
    assert_eq!(finland.note_fields[0].source, "Finland");
    assert_eq!(finland.note_fields[0].translated, "Finland");
    assert_eq!(finland.source_occurrences, 1);
    assert!(finland.card_templates.iter().any(|card| {
        card.template_id.as_str() == "template.country-to-capital"
            && card.sides.contains(&CardTemplateSide::Question)
    }));

    let europe_units = context
        .units
        .iter()
        .filter(|unit| unit.source == "Europe")
        .collect::<Vec<_>>();
    assert_eq!(europe_units.len(), 2);
    assert!(europe_units.iter().all(|unit| unit.source_occurrences == 2));
}

#[test]
fn translation_dictionary_reports_stale_no_change_entries() {
    let base = ug_style_deck();
    let overlay = Overlay {
        id: sid("overlay.translation.da"),
        kind: OverlayKind::Translation,
        translations: Some(TranslationDictionary {
            direct: BTreeMap::new(),
            contextual: BTreeMap::new(),
            no_change: BTreeSet::from([
                "Removed source".to_owned(),
                "Removed contextual".to_owned(),
            ]),
            target_adaptations: BTreeMap::new(),
            stale_translations: Vec::new(),
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

    let report = base
        .translation_coverage(&overlay)
        .expect("translation overlay has coverage");
    assert!(report.has_stale_or_invalid_entries());
    assert!(report.entries.iter().any(|entry| {
        entry.category == TranslationCoverageCategory::StaleNoChangeKey
            && entry.path == "translations.no_change.Removed source"
    }));
    assert!(report.entries.iter().any(|entry| {
        entry.category == TranslationCoverageCategory::StaleNoChangeKey
            && entry.path == "translations.no_change.Removed contextual"
    }));

    let report = base
        .compose(&[overlay])
        .expect_err("stale no-change entries fail");
    assert!(report.has_kind(ComposeErrorKind::StaleTranslationEntry));
}

#[test]
fn translation_dictionary_reports_missing_direct_translation_when_complete() {
    let base = ug_style_deck();
    let overlay = Overlay {
        id: sid("overlay.translation.da"),
        kind: OverlayKind::Translation,
        translations: Some(TranslationDictionary {
            direct: BTreeMap::new(),
            contextual: BTreeMap::new(),
            no_change: BTreeSet::new(),
            target_adaptations: BTreeMap::new(),
            stale_translations: Vec::new(),
            variables: BTreeMap::new(),
            adapter_ids: BTreeMap::new(),
            require_complete: true,
            ignore_paths: BTreeSet::from(["deck.*".to_owned(), "note_types.*".to_owned()]),
        }),
        deck_change: None,
        note_changes: BTreeMap::new(),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    };

    let report = base
        .compose(&[overlay])
        .expect_err("complete translations require entries");

    assert!(report.has_kind(ComposeErrorKind::MissingTranslation));
    assert!(report.errors.iter().any(|error| {
        error.path == "notes.note.finland.fields.field.capital"
            && error
                .message
                .contains("missing direct or contextual translation for \"Helsinki\"")
    }));
}

#[test]
fn translation_dictionary_allows_field_level_contextual_translation() {
    let base = ug_style_deck();
    let overlay = Overlay {
        id: sid("overlay.translation.da"),
        kind: OverlayKind::Translation,
        translations: Some(TranslationDictionary {
            direct: BTreeMap::new(),
            contextual: BTreeMap::from([(
                "notes.note.finland.fields.field.capital".to_owned(),
                BTreeMap::from([("Helsinki".to_owned(), "Helsingfors".to_owned())]),
            )]),
            no_change: BTreeSet::new(),
            target_adaptations: BTreeMap::new(),
            stale_translations: Vec::new(),
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

    let deck = base.compose(&[overlay]).expect(
        "field-level contextual translations are allowed for workbench source-string edits",
    );

    assert_eq!(
        deck.notes
            .get(&sid("note.finland"))
            .unwrap()
            .fields
            .get(&sid("field.capital"))
            .and_then(FieldValue::as_scalar),
        Some("Helsingfors")
    );
}

#[test]
fn target_adaptation_can_intentionally_replace_matching_nonblank_source() {
    let base = ug_style_deck();
    let overlay = Overlay {
        id: sid("overlay.translation.zh"),
        kind: OverlayKind::Translation,
        translations: Some(TranslationDictionary {
            direct: BTreeMap::new(),
            contextual: BTreeMap::new(),
            no_change: BTreeSet::new(),
            target_adaptations: BTreeMap::from([(
                "notes.note.finland.fields.field.capital".to_owned(),
                target_adaptation("Helsinki", "Helsinki, target-language wording"),
            )]),
            stale_translations: Vec::new(),
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

    let resolved = base
        .compose(&[overlay])
        .expect("target adaptation composes");

    assert_eq!(
        resolved.notes[&sid("note.finland")].fields[&sid("field.capital")],
        "Helsinki, target-language wording"
    );
}

#[test]
fn noop_target_adaptation_reserves_change_path_for_later_overlay_conflicts() {
    let base = ug_style_deck();
    let adaptation = Overlay {
        id: sid("overlay.translation.zh"),
        kind: OverlayKind::Translation,
        translations: Some(TranslationDictionary {
            direct: BTreeMap::new(),
            contextual: BTreeMap::new(),
            no_change: BTreeSet::new(),
            target_adaptations: BTreeMap::from([(
                "notes.note.finland.fields.field.capital".to_owned(),
                target_adaptation("Helsinki", "Helsinki"),
            )]),
            stale_translations: Vec::new(),
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
    let later_patch = overlay_replacing_capital(ChangeIntent::Merge, None, "Helsinki City");

    let report = base
        .compose(&[adaptation, later_patch])
        .expect_err("no-op target adaptation still reserves the path");

    assert!(report.has_kind(ComposeErrorKind::Conflict));
    let conflict = report
        .errors
        .iter()
        .find(|error| error.kind == ComposeErrorKind::Conflict)
        .expect("typed conflict exists");
    assert_eq!(conflict.path, "notes.note.finland.fields.field.capital");
    assert_eq!(
        conflict.first_conflict_participant.as_ref(),
        Some(&sid("overlay.translation.zh"))
    );
    assert_eq!(
        conflict.current_conflict_participant.as_ref(),
        Some(&sid("overlay.patch.capital"))
    );
    assert_eq!(
        conflict.overlay_id.as_ref(),
        conflict.current_conflict_participant.as_ref()
    );
}

#[test]
fn translation_dictionary_target_adaptation_fails_when_expected_source_mismatches() {
    let base = ug_style_deck();
    let overlay = Overlay {
        id: sid("overlay.translation.da"),
        kind: OverlayKind::Translation,
        translations: Some(TranslationDictionary {
            direct: BTreeMap::new(),
            contextual: BTreeMap::new(),
            no_change: BTreeSet::new(),
            target_adaptations: BTreeMap::from([(
                "notes.note.finland.fields.field.capital".to_owned(),
                target_adaptation("", "Helsinki translated"),
            )]),
            stale_translations: Vec::new(),
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

    let report = base
        .compose(&[overlay])
        .expect_err("target adaptation expected source should match");

    assert!(report.has_kind(ComposeErrorKind::ExpectedBaseMismatch));
    assert!(report.errors.iter().any(|error| {
        error.path == "notes.note.finland.fields.field.capital"
            && error.message.contains("target adaptation expected source")
    }));
}

#[test]
fn translation_coverage_reports_new_note_and_new_field_fallbacks() {
    let mut base = ug_style_deck();
    let mut sweden = sweden_note();
    sweden
        .fields
        .insert(sid("field.population"), "10 million".to_owned());
    base.notes.insert(sid("note.sweden"), sweden);
    base.note_types
        .get_mut(&sid("note-type.country"))
        .unwrap()
        .fields
        .push(FieldDefinition {
            id: sid("field.population"),
            name: "Population".to_owned(),
        });
    let overlay = Overlay {
        id: sid("overlay.translation.da"),
        kind: OverlayKind::Translation,
        translations: Some(TranslationDictionary {
            direct: BTreeMap::from([
                ("Finland".to_owned(), "Finland".to_owned()),
                ("Helsinki".to_owned(), "Helsingfors".to_owned()),
            ]),
            contextual: BTreeMap::new(),
            no_change: BTreeSet::new(),
            target_adaptations: BTreeMap::new(),
            stale_translations: Vec::new(),
            variables: BTreeMap::new(),
            adapter_ids: BTreeMap::new(),
            require_complete: false,
            ignore_paths: BTreeSet::from([
                "deck.*".to_owned(),
                "note_types.*".to_owned(),
                "notes.*.fields.field.flag".to_owned(),
                "notes.*.tags.*".to_owned(),
            ]),
        }),
        deck_change: None,
        note_changes: BTreeMap::new(),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    };

    let report = base
        .translation_coverage(&overlay)
        .expect("translation overlay has coverage");

    assert!(report.has_untranslated_fallbacks());
    assert!(report.entries.iter().any(|entry| {
        entry.category == TranslationCoverageCategory::UntranslatedFallback
            && entry.path == "notes.note.sweden.fields.field.country"
            && entry.source == "Sweden"
    }));
    assert!(report.entries.iter().any(|entry| {
        entry.category == TranslationCoverageCategory::UntranslatedFallback
            && entry.path == "notes.note.sweden.fields.field.population"
            && entry.source == "10 million"
    }));
}

#[test]
fn translation_coverage_reports_stale_changed_or_removed_source_keys() {
    let mut base = ug_style_deck();
    base.notes
        .get_mut(&sid("note.finland"))
        .unwrap()
        .fields
        .insert(sid("field.capital"), "Helsinki City".to_owned());
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
            require_complete: false,
            ignore_paths: BTreeSet::new(),
        }),
        deck_change: None,
        note_changes: BTreeMap::new(),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    };

    let report = base
        .translation_coverage(&overlay)
        .expect("translation overlay has coverage");

    assert!(report.has_stale_or_invalid_entries());
    assert!(report.entries.iter().any(|entry| {
        entry.category == TranslationCoverageCategory::StaleDirectKey
            && entry.path == "translations.direct.Helsinki"
    }));
    assert!(report.entries.iter().any(|entry| {
        entry.category == TranslationCoverageCategory::UntranslatedFallback
            && entry.path == "notes.note.finland.fields.field.capital"
            && entry.source == "Helsinki City"
    }));
}

#[test]
fn translation_coverage_reports_path_specific_overrides_and_additions() {
    let mut base = ug_style_deck();
    base.notes
        .get_mut(&sid("note.finland"))
        .unwrap()
        .fields
        .insert(sid("field.flag"), String::new());
    let overlay = Overlay {
        id: sid("overlay.translation.da"),
        kind: OverlayKind::Translation,
        translations: Some(TranslationDictionary {
            direct: BTreeMap::from([("Finland".to_owned(), "Finland".to_owned())]),
            contextual: BTreeMap::from([(
                "notes.note.finland".to_owned(),
                BTreeMap::from([("Helsinki".to_owned(), "Helsingfors".to_owned())]),
            )]),
            no_change: BTreeSet::new(),
            target_adaptations: BTreeMap::from([(
                "notes.note.finland.fields.field.flag".to_owned(),
                target_adaptation("", r#"<img src=\"fi-da.png\">"#),
            )]),
            stale_translations: Vec::new(),
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

    let report = base
        .translation_coverage(&overlay)
        .expect("translation overlay has coverage");

    assert!(report.entries.iter().any(|entry| {
        entry.category == TranslationCoverageCategory::ContextualTranslation
            && entry.context.as_deref() == Some("notes.note.finland")
            && entry.path == "notes.note.finland.fields.field.capital"
    }));
    assert!(report.entries.iter().any(|entry| {
        entry.category == TranslationCoverageCategory::TargetAdaptation
            && entry.path == "notes.note.finland.fields.field.flag"
    }));
}

#[test]
fn render_variables_reports_structured_message_resolution_errors_after_substitution() {
    let deck = ug_style_deck_with_variable_field_ref_message("missing");

    let report = deck
        .render_variables()
        .expect_err("unresolvable structured message refs must fail variable rendering");

    let output = report.to_string();
    assert!(output.contains("notes.note.finland.fields.field.summary"));
    assert!(output.contains("notes.note.missing.fields.field.country"));
    assert!(output.contains("names missing note note.missing"));
}

#[test]
fn canonical_validation_rejects_structured_message_reference_cycles() {
    let mut deck = ug_style_deck();
    let note = deck.notes.get_mut(&sid("note.finland")).unwrap();
    note.fields.insert(
        sid("field.country"),
        FieldValue::Message(StructuredMessage {
            components: vec![MessageComponent::FieldRef(
                "notes.note.finland.fields.field.capital".to_owned(),
            )],
            format: None,
            variables: BTreeMap::new(),
        }),
    );
    note.fields.insert(
        sid("field.capital"),
        FieldValue::Message(StructuredMessage {
            components: vec![MessageComponent::FieldRef(
                "notes.note.finland.fields.field.country".to_owned(),
            )],
            format: None,
            variables: BTreeMap::new(),
        }),
    );

    let report = deck
        .validate()
        .expect_err("message cycle must fail validation");
    assert!(report.has_kind(ValidationErrorKind::MessageDependencyCycle));
    assert!(report.errors.iter().any(|error| {
        error
            .field_graph_error
            .as_ref()
            .is_some_and(|details| details.cycle.first() == details.cycle.last())
    }));
}

#[test]
fn render_variables_resolves_structured_message_field_refs_after_correct_substitution() {
    let deck = ug_style_deck_with_variable_field_ref_message("iceland");

    let rendered = deck
        .render_variables()
        .expect("correct structured message refs render after variable substitution");

    assert_eq!(
        rendered.notes[&sid("note.finland")].fields[&sid("field.summary")],
        "Iceland"
    );
}

#[test]
fn render_variables_lowers_structured_images_to_exact_img_html() {
    let mut deck = ug_style_deck();
    let note = deck.notes.get_mut(&sid("note.finland")).unwrap();
    note.fields.insert(
        sid("field.flag"),
        FieldValue::Images(vec![FieldImageReference {
            media_id: sid("media.flag.finland"),
        }]),
    );

    let rendered = deck
        .render_variables()
        .expect("declared structured image renders");

    let rendered_note = &rendered.notes[&sid("note.finland")];
    assert_eq!(
        rendered_note.fields[&sid("field.flag")],
        "<img src=\"flags/fi.png\" />"
    );
    assert!(matches!(
        rendered_note.fields[&sid("field.flag")],
        FieldValue::Scalar(_)
    ));
}

#[test]
fn render_variables_url_encodes_and_html_escapes_structured_image_paths() {
    let mut deck = ug_style_deck();
    deck.media.get_mut(&sid("media.flag.finland")).unwrap().path =
        "flags/旗 & quote\" #1?.svg".to_owned();
    let note = deck.notes.get_mut(&sid("note.finland")).unwrap();
    note.fields.insert(
        sid("field.flag"),
        FieldValue::Images(vec![FieldImageReference {
            media_id: sid("media.flag.finland"),
        }]),
    );

    let rendered = deck
        .render_variables()
        .expect("hostile filename renders safely");

    assert_eq!(
        rendered.notes[&sid("note.finland")].fields[&sid("field.flag")],
        "<img src=\"flags/%E6%97%97%20%26%20quote%22%20%231%3F.svg\" />"
    );
    assert_eq!(
        rendered.media[&sid("media.flag.finland")].path,
        "flags/旗 & quote\" #1?.svg"
    );
}

#[test]
fn render_variables_lowers_multi_image_fields_without_separators() {
    let mut deck = ug_style_deck();
    deck.media.insert(
        sid("media.flag.finland.blur"),
        MediaReference {
            id: sid("media.flag.finland.blur"),
            path: "flags/fi-blur.png".to_owned(),
            sha256: "abcdef".to_owned(),
        },
    );
    let note = deck.notes.get_mut(&sid("note.finland")).unwrap();
    note.fields.insert(
        sid("field.flag"),
        FieldValue::Images(vec![
            FieldImageReference {
                media_id: sid("media.flag.finland.blur"),
            },
            FieldImageReference {
                media_id: sid("media.flag.finland"),
            },
        ]),
    );

    let rendered = deck
        .render_variables()
        .expect("declared structured images render");

    assert_eq!(
        rendered.notes[&sid("note.finland")].fields[&sid("field.flag")],
        "<img src=\"flags/fi-blur.png\" /><img src=\"flags/fi.png\" />"
    );
}

#[test]
fn render_variables_reports_unknown_structured_image_media_id_with_field_path() {
    let mut deck = ug_style_deck();
    let note = deck.notes.get_mut(&sid("note.finland")).unwrap();
    note.fields.insert(
        sid("field.flag"),
        FieldValue::Images(vec![FieldImageReference {
            media_id: sid("media.flag.missing"),
        }]),
    );

    let report = deck
        .render_variables()
        .expect_err("unknown structured image id must fail rendering");

    let output = report.to_string();
    assert!(output.contains("unknown media id \"media.flag.missing\""));
    assert!(output.contains("notes.note.finland.fields.field.flag"));
}

#[test]
fn translation_dictionary_resolves_structured_message_components() {
    let base = ug_style_deck_with_flag_similarity_message();
    let overlay = Overlay {
        id: sid("overlay.translation.nb"),
        kind: OverlayKind::Translation,
        translations: Some(TranslationDictionary {
            direct: BTreeMap::from([
                ("Iceland".to_owned(), "Island".to_owned()),
                ("Norway".to_owned(), "Norge".to_owned()),
                (
                    "red background with a blue cross".to_owned(),
                    "rød bakgrunn med blått kors".to_owned(),
                ),
            ]),
            contextual: BTreeMap::from([(
                "notes.note.finland".to_owned(),
                BTreeMap::from([(
                    "blue background with a white cross".to_owned(),
                    "blå bakgrunn med hvitt kors".to_owned(),
                )]),
            )]),
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
                "notes.*.fields.field.capital".to_owned(),
                "notes.*.fields.field.flag".to_owned(),
                "notes.*.tags.*".to_owned(),
            ]),
        }),
        deck_change: None,
        note_changes: BTreeMap::new(),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    };

    let report = base
        .translation_coverage(&overlay)
        .expect("translation overlay has coverage");
    assert!(report.entries.iter().any(|entry| {
        entry.category == TranslationCoverageCategory::DirectTranslation
            && entry.path
                == "notes.note.finland.fields.field.flag-similarity.message.variables.country_1"
            && entry.source == "Iceland"
            && entry.translated.as_deref() == Some("Island")
    }));
    assert!(report.entries.iter().any(|entry| {
        entry.category == TranslationCoverageCategory::ContextualTranslation
            && entry.path
                == "notes.note.finland.fields.field.flag-similarity.message.variables.description_1"
            && entry.source == "blue background with a white cross"
            && entry.translated.as_deref() == Some("blå bakgrunn med hvitt kors")
    }));

    let resolved = base.compose(&[overlay]).expect("translations compose");

    assert_eq!(
        resolved
            .field_text(&sid("note.finland"), &sid("field.flag-similarity"))
            .expect("structured message resolves"),
        "Island (blå bakgrunn med hvitt kors), Norge (rød bakgrunn med blått kors)"
    );
}

#[test]
fn translation_dictionary_no_change_wins_over_stale_record_for_structured_message_format() {
    let base = ug_style_deck_with_flag_similarity_message();
    let format_source = "{country_1} ({description_1}), {country_2} ({description_2})";
    let overlay = Overlay {
        id: sid("overlay.translation.nb"),
        kind: OverlayKind::Translation,
        translations: Some(TranslationDictionary {
            direct: BTreeMap::new(),
            contextual: BTreeMap::new(),
            no_change: BTreeSet::from([format_source.to_owned()]),
            target_adaptations: BTreeMap::new(),
            stale_translations: vec![StaleTranslation {
                old_source: "old structured message format".to_owned(),
                new_source: format_source.to_owned(),
                target: "STALE {country_1}".to_owned(),
                context: Some("notes.note.finland.fields.field.flag-similarity.message".to_owned()),
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

    let report = base
        .translation_coverage(&overlay)
        .expect("translation overlay has coverage");
    assert!(report.entries.iter().any(|entry| {
        entry.category == TranslationCoverageCategory::NoChange
            && entry.path == "notes.note.finland.fields.field.flag-similarity.message.format"
            && entry.source == format_source
    }));
    assert!(!report.entries.iter().any(|entry| {
        entry.category == TranslationCoverageCategory::StaleTranslation
            && entry.path == "notes.note.finland.fields.field.flag-similarity.message.format"
    }));

    let resolved = base.compose(&[overlay]).expect("translations compose");

    assert_eq!(
        resolved
            .field_text(&sid("note.finland"), &sid("field.flag-similarity"))
            .expect("structured message resolves"),
        "Iceland (blue background with a white cross), Norway (red background with a blue cross)"
    );
}

#[test]
fn translation_dictionary_can_translate_structured_message_format() {
    let base = ug_style_deck_with_flag_similarity_message();
    let format_source = "{country_1} ({description_1}), {country_2} ({description_2})";
    let overlay = Overlay {
        id: sid("overlay.translation.zh"),
        kind: OverlayKind::Translation,
        translations: Some(TranslationDictionary {
            direct: BTreeMap::from([
                ("Iceland".to_owned(), "冰岛".to_owned()),
                ("Norway".to_owned(), "挪威".to_owned()),
                (
                    format_source.to_owned(),
                    "{country_1}({description_1})、{country_2}({description_2})".to_owned(),
                ),
                (
                    "blue background with a white cross".to_owned(),
                    "蓝底白十字".to_owned(),
                ),
                (
                    "red background with a blue cross".to_owned(),
                    "红底蓝十字".to_owned(),
                ),
            ]),
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
                "notes.*.fields.field.capital".to_owned(),
                "notes.*.fields.field.flag".to_owned(),
                "notes.*.tags.*".to_owned(),
            ]),
        }),
        deck_change: None,
        note_changes: BTreeMap::new(),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    };

    let report = base
        .translation_coverage(&overlay)
        .expect("translation overlay has coverage");
    assert!(report.entries.iter().any(|entry| {
        entry.category == TranslationCoverageCategory::DirectTranslation
            && entry.path == "notes.note.finland.fields.field.flag-similarity.message.format"
            && entry.source == format_source
    }));

    let resolved = base.compose(&[overlay]).expect("translations compose");

    assert_eq!(
        resolved
            .field_text(&sid("note.finland"), &sid("field.flag-similarity"))
            .expect("structured message resolves"),
        "冰岛(蓝底白十字)、挪威(红底蓝十字)"
    );
}

#[test]
fn translation_dictionary_can_override_full_structured_message_when_needed() {
    let base = ug_style_deck_with_flag_similarity_message();
    let full_source =
        "Iceland (blue background with a white cross), Norway (red background with a blue cross)";
    let overlay = Overlay {
        id: sid("overlay.translation.nb"),
        kind: OverlayKind::Translation,
        translations: Some(TranslationDictionary {
            direct: BTreeMap::new(),
            contextual: BTreeMap::from([(
                "notes.note.finland".to_owned(),
                BTreeMap::from([(
                    full_source.to_owned(),
                    "Særskilt full oversettelse".to_owned(),
                )]),
            )]),
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
                "notes.*.fields.field.capital".to_owned(),
                "notes.*.fields.field.flag".to_owned(),
                "notes.*.tags.*".to_owned(),
            ]),
        }),
        deck_change: None,
        note_changes: BTreeMap::new(),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    };

    let resolved = base.compose(&[overlay]).expect("full override composes");

    assert_eq!(
        resolved.notes[&sid("note.finland")].fields[&sid("field.flag-similarity")],
        "Særskilt full oversettelse"
    );
    assert!(matches!(
        resolved.notes[&sid("note.finland")].fields[&sid("field.flag-similarity")],
        FieldValue::Scalar(_)
    ));
}

#[test]
fn translation_dictionary_reports_structured_message_missing_and_stale_components() {
    let base = ug_style_deck_with_flag_similarity_message();
    let overlay = Overlay {
        id: sid("overlay.translation.nb"),
        kind: OverlayKind::Translation,
        translations: Some(TranslationDictionary {
            direct: BTreeMap::from([
                ("Iceland".to_owned(), "Island".to_owned()),
                ("Norway".to_owned(), "Norge".to_owned()),
                (
                    "stale qualifier".to_owned(),
                    "foreldet kvalifikator".to_owned(),
                ),
            ]),
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
                "notes.*.fields.field.capital".to_owned(),
                "notes.*.fields.field.flag".to_owned(),
                "notes.*.tags.*".to_owned(),
            ]),
        }),
        deck_change: None,
        note_changes: BTreeMap::new(),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    };

    let report = base
        .translation_coverage(&overlay)
        .expect("translation overlay has coverage");
    assert!(report.entries.iter().any(|entry| {
        entry.category == TranslationCoverageCategory::UntranslatedFallback
            && entry.path
                == "notes.note.finland.fields.field.flag-similarity.message.variables.description_1"
            && entry.source == "blue background with a white cross"
    }));
    assert!(report.entries.iter().any(|entry| {
        entry.category == TranslationCoverageCategory::StaleDirectKey
            && entry.path == "translations.direct.stale qualifier"
    }));

    let report = base
        .compose(&[overlay])
        .expect_err("strict structured message components fail");
    assert!(report.errors.iter().any(|error| {
        error.kind == ComposeErrorKind::MissingTranslation
            && error.path
                == "notes.note.finland.fields.field.flag-similarity.message.variables.description_1"
            && error
                .message
                .contains("missing direct or contextual translation")
    }));
    assert!(report.errors.iter().any(|error| {
        error.kind == ComposeErrorKind::StaleTranslationEntry
            && error.path == "translations.direct.stale qualifier"
    }));
}

#[test]
fn field_image_changes_replace_raw_fields_and_raw_changes_clear_images() {
    let mut base = ug_style_deck();
    base.notes
        .get_mut(&sid("note.finland"))
        .unwrap()
        .fields
        .insert(sid("field.flag"), "raw flag html".to_owned());

    let to_image = Overlay {
        id: sid("overlay.patch.flag-image"),
        kind: OverlayKind::Patch,
        translations: None,
        deck_change: None,
        note_changes: BTreeMap::from([(
            sid("note.finland"),
            NoteChange {
                intent: ChangeIntent::Merge,
                note: None,
                variables: BTreeMap::new(),
                fields: BTreeMap::from([(
                    sid("field.flag"),
                    FieldChange {
                        intent: ChangeIntent::Replace,
                        value: Some(FieldValue::Images(vec![FieldImageReference {
                            media_id: sid("media.flag.finland"),
                        }])),
                        expected_base: Some(ExpectedBase::Value("raw flag html".to_owned())),
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

    let imaged = base.compose(&[to_image]).expect("image field composes");
    let note = &imaged.notes[&sid("note.finland")];
    assert_eq!(
        note.fields[&sid("field.flag")].as_images().unwrap()[0].media_id,
        sid("media.flag.finland")
    );

    let to_raw = Overlay {
        id: sid("overlay.patch.flag-raw"),
        kind: OverlayKind::Patch,
        translations: None,
        deck_change: None,
        note_changes: BTreeMap::from([(
            sid("note.finland"),
            NoteChange {
                intent: ChangeIntent::Merge,
                note: None,
                variables: BTreeMap::new(),
                fields: BTreeMap::from([(
                    sid("field.flag"),
                    FieldChange {
                        intent: ChangeIntent::Replace,
                        value: Some(("replacement raw html".to_owned()).into()),
                        expected_base: Some(ExpectedBase::FieldValue(FieldValue::Images(vec![
                            FieldImageReference {
                                media_id: sid("media.flag.finland"),
                            },
                        ]))),
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

    let raw = imaged.compose(&[to_raw]).expect("raw replacement composes");
    let note = &raw.notes[&sid("note.finland")];
    assert_eq!(note.fields[&sid("field.flag")], "replacement raw html");
}

#[test]
fn semantic_field_intent_matrix_is_representation_aware() {
    let values = [
        ("blank", FieldValue::scalar("")),
        ("scalar", FieldValue::scalar("existing")),
        (
            "image",
            FieldValue::Images(vec![FieldImageReference {
                media_id: sid("media.flag.finland"),
            }]),
        ),
        (
            "message",
            FieldValue::Message(StructuredMessage {
                components: vec![MessageComponent::Text("existing message".to_owned())],
                format: None,
                variables: BTreeMap::new(),
            }),
        ),
    ];
    let intents = [
        ChangeIntent::Add,
        ChangeIntent::Merge,
        ChangeIntent::Replace,
        ChangeIntent::Override,
        ChangeIntent::Remove,
    ];

    for (name, existing) in &values {
        for intent in intents {
            let mut base = ug_style_deck();
            base.notes
                .get_mut(&sid("note.finland"))
                .unwrap()
                .fields
                .insert(sid("field.flag"), existing.clone());
            let expected_base = match intent {
                ChangeIntent::Replace | ChangeIntent::Override | ChangeIntent::Remove => {
                    Some(ExpectedBase::FieldValue(existing.clone()))
                }
                ChangeIntent::Add | ChangeIntent::Merge => None,
            };
            let change = FieldChange {
                intent,
                value: (intent != ChangeIntent::Remove).then(|| FieldValue::scalar("replacement")),
                expected_base,
            };
            let result = base.compose(&[overlay_with_field_change(
                "overlay.matrix",
                sid("field.flag"),
                change,
            )]);
            let should_succeed = match intent {
                ChangeIntent::Add | ChangeIntent::Merge => existing.is_blank(),
                ChangeIntent::Replace | ChangeIntent::Override => true,
                ChangeIntent::Remove => false,
            };
            assert_eq!(
                result.is_ok(),
                should_succeed,
                "intent {intent:?} against {name}"
            );
        }
    }

    for intent in intents {
        let mut base = ug_style_deck();
        base.notes
            .get_mut(&sid("note.finland"))
            .unwrap()
            .fields
            .remove(&sid("field.flag"));
        let change = FieldChange {
            intent,
            value: (intent != ChangeIntent::Remove).then(|| FieldValue::scalar("replacement")),
            expected_base: matches!(
                intent,
                ChangeIntent::Replace | ChangeIntent::Override | ChangeIntent::Remove
            )
            .then_some(ExpectedBase::EntityPresent),
        };
        let result = base.compose(&[overlay_with_field_change(
            "overlay.matrix.missing",
            sid("field.flag"),
            change,
        )]);
        assert_eq!(
            result.is_ok(),
            matches!(intent, ChangeIntent::Add | ChangeIntent::Merge),
            "intent {intent:?} against missing field"
        );
    }

    for existing in values.into_iter().skip(2).map(|(_, value)| value) {
        let mut base = ug_style_deck();
        base.notes
            .get_mut(&sid("note.finland"))
            .unwrap()
            .fields
            .insert(sid("field.flag"), existing);
        let report = base
            .compose(&[overlay_with_field_change(
                "overlay.matrix.wrong-scalar-base",
                sid("field.flag"),
                FieldChange {
                    intent: ChangeIntent::Replace,
                    value: Some(FieldValue::scalar("replacement")),
                    expected_base: Some(ExpectedBase::Value(String::new())),
                },
            )])
            .expect_err("empty scalar expected base must not match structured values");
        assert!(report.has_kind(ComposeErrorKind::ExpectedBaseMismatch));
    }
}

#[test]
fn extension_overlay_can_add_a_note_type_field_and_backfill_note_values() {
    let base = ug_style_deck();
    let overlay = Overlay {
        id: sid("overlay.extension.population"),
        kind: OverlayKind::Extension,
        translations: None,
        deck_change: None,
        note_type_changes: BTreeMap::from([(
            sid("note-type.country"),
            NoteTypeChange {
                intent: ChangeIntent::Merge,
                note_type: None,
                name: None,
                variables: BTreeMap::new(),
                styling: None,
                fields: BTreeMap::from([(
                    sid("field.population"),
                    FieldDefinitionChange {
                        intent: ChangeIntent::Add,
                        field: Some(FieldDefinition {
                            id: sid("field.population"),
                            name: "Population".to_owned(),
                        }),
                        expected_base: None,
                    },
                )]),
                card_templates: BTreeMap::new(),
                adapter_ids: BTreeMap::new(),
                expected_base: None,
            },
        )]),
        note_changes: BTreeMap::from([(
            sid("note.finland"),
            NoteChange {
                intent: ChangeIntent::Merge,
                note: None,
                variables: BTreeMap::new(),
                fields: BTreeMap::from([(
                    sid("field.population"),
                    FieldChange {
                        intent: ChangeIntent::Add,
                        value: Some(("5.6 million".to_owned()).into()),
                        expected_base: None,
                    },
                )]),
                tags: BTreeMap::new(),
                adapter_ids: BTreeMap::new(),
                expected_base: None,
            },
        )]),
        media_changes: BTreeMap::new(),
    };

    let resolved = base.compose(&[overlay]).expect("extension composes");

    let note_type = resolved.note_types.get(&sid("note-type.country")).unwrap();
    assert_eq!(note_type.fields.last().unwrap().id, sid("field.population"));
    assert_eq!(
        resolved
            .notes
            .get(&sid("note.finland"))
            .unwrap()
            .fields
            .get(&sid("field.population"))
            .and_then(FieldValue::as_scalar),
        Some("5.6 million")
    );
}

#[test]
fn extension_overlay_adds_blank_values_for_new_fields_without_explicit_values() {
    let base = ug_style_deck();
    let overlay = Overlay {
        id: sid("overlay.extension.region-code"),
        kind: OverlayKind::Extension,
        translations: None,
        deck_change: None,
        note_type_changes: BTreeMap::from([(
            sid("note-type.country"),
            NoteTypeChange {
                intent: ChangeIntent::Merge,
                note_type: None,
                name: None,
                variables: BTreeMap::new(),
                styling: None,
                fields: BTreeMap::from([(
                    sid("field.region-code"),
                    FieldDefinitionChange {
                        intent: ChangeIntent::Add,
                        field: Some(FieldDefinition {
                            id: sid("field.region-code"),
                            name: "Region code".to_owned(),
                        }),
                        expected_base: None,
                    },
                )]),
                card_templates: BTreeMap::new(),
                adapter_ids: BTreeMap::new(),
                expected_base: None,
            },
        )]),
        note_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    };

    let resolved = base.compose(&[overlay]).expect("extension composes");

    assert_eq!(
        resolved.notes[&sid("note.finland")]
            .fields
            .get(&sid("field.region-code"))
            .and_then(FieldValue::as_scalar),
        Some("")
    );
    resolved
        .validate()
        .expect("new fields default to blank values on existing notes");
}

#[test]
fn metadata_overlay_can_replace_names_and_adapter_identities() {
    let base = ug_style_deck();
    let overlay = Overlay {
        id: sid("overlay.translation.de"),
        kind: OverlayKind::Translation,
        translations: None,
        deck_change: Some(DeckChange {
            name: Some(PropertyChange {
                intent: ChangeIntent::Replace,
                value: Some("Ultimate Geography [DE]".to_owned()),
                expected_base: Some(ExpectedBase::Value("Ultimate Geography".to_owned())),
            }),
            description: None,
            variables: BTreeMap::new(),
            adapter_ids: BTreeMap::from([(
                "crowdanki:uuid".to_owned(),
                AdapterIdChange {
                    intent: ChangeIntent::Replace,
                    value: Some("de-deck-uuid".to_owned()),
                    expected_base: Some(ExpectedBase::Value(
                        "43c5ba66-9a65-11e8-90c9-a0481cc15658".to_owned(),
                    )),
                },
            )]),
        }),
        note_type_changes: BTreeMap::from([(
            sid("note-type.country"),
            NoteTypeChange {
                intent: ChangeIntent::Merge,
                note_type: None,
                name: Some(PropertyChange {
                    intent: ChangeIntent::Replace,
                    value: Some("Ultimate Geography [DE]".to_owned()),
                    expected_base: Some(ExpectedBase::Value(
                        "Ultimate Geography Country".to_owned(),
                    )),
                }),
                variables: BTreeMap::new(),
                styling: None,
                fields: BTreeMap::new(),
                card_templates: BTreeMap::from([(
                    sid("template.country-to-capital"),
                    CardTemplateChange {
                        intent: ChangeIntent::Merge,
                        template: None,
                        insert_after: None,
                        name: None,
                        variables: BTreeMap::new(),
                        question_format: None,
                        answer_format: None,
                        adapter_ids: BTreeMap::from([(
                            "crowdanki:ord".to_owned(),
                            AdapterIdChange {
                                intent: ChangeIntent::Add,
                                value: Some("0".to_owned()),
                                expected_base: None,
                            },
                        )]),
                        expected_base: None,
                    },
                )]),
                adapter_ids: BTreeMap::from([(
                    "crowdanki:model_id".to_owned(),
                    AdapterIdChange {
                        intent: ChangeIntent::Replace,
                        value: Some("de-model-id".to_owned()),
                        expected_base: Some(ExpectedBase::Value("1548959259107".to_owned())),
                    },
                )]),
                expected_base: None,
            },
        )]),
        note_changes: BTreeMap::from([(
            sid("note.finland"),
            NoteChange {
                intent: ChangeIntent::Merge,
                note: None,
                variables: BTreeMap::new(),
                fields: BTreeMap::new(),
                tags: BTreeMap::new(),
                adapter_ids: BTreeMap::from([(
                    "crowdanki:guid".to_owned(),
                    AdapterIdChange {
                        intent: ChangeIntent::Replace,
                        value: Some("ug-finland-de-guid".to_owned()),
                        expected_base: Some(ExpectedBase::Value("ug-finland-guid".to_owned())),
                    },
                )]),
                expected_base: None,
            },
        )]),
        media_changes: BTreeMap::new(),
    };

    let resolved = base.compose(&[overlay]).expect("metadata overlay composes");

    assert_eq!(resolved.name, "Ultimate Geography [DE]");
    assert_eq!(
        resolved.adapter_ids.get("crowdanki:uuid"),
        Some("de-deck-uuid")
    );
    let note_type = resolved.note_types.get(&sid("note-type.country")).unwrap();
    assert_eq!(note_type.name, "Ultimate Geography [DE]");
    assert_eq!(
        note_type.adapter_ids.get("crowdanki:model_id"),
        Some("de-model-id")
    );
    assert_eq!(
        note_type.card_templates[0].adapter_ids.get("crowdanki:ord"),
        Some("0")
    );
    assert_eq!(
        resolved
            .notes
            .get(&sid("note.finland"))
            .unwrap()
            .adapter_ids
            .get("crowdanki:guid"),
        Some("ug-finland-de-guid")
    );
}

#[test]
fn overlay_can_add_card_templates_in_order_and_replace_styling() {
    let base = ug_style_deck();
    let overlay = Overlay {
        id: sid("overlay.extension.extended"),
        kind: OverlayKind::Extension,
        translations: None,
        deck_change: None,
        note_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
        note_type_changes: BTreeMap::from([(
            sid("note-type.country"),
            NoteTypeChange {
                intent: ChangeIntent::Merge,
                note_type: None,
                name: None,
                variables: BTreeMap::new(),
                styling: Some(PropertyChange {
                    intent: ChangeIntent::Replace,
                    value: Some(".card { font-family: serif; }\n".to_owned()),
                    expected_base: Some(ExpectedBase::Value(
                        ".card { font-family: sans-serif; }\n".to_owned(),
                    )),
                }),
                fields: BTreeMap::new(),
                card_templates: BTreeMap::from([(
                    sid("template.country-to-flag"),
                    CardTemplateChange {
                        intent: ChangeIntent::Add,
                        template: Some(CardTemplate {
                            id: sid("template.country-to-flag"),
                            name: "Country - Flag".to_owned(),
                            variables: BTreeMap::new(),
                            question_format: "{{Country}}".to_owned(),
                            answer_format: "{{Flag}}".to_owned(),
                            adapter_ids: AdapterIds::new(),
                        }),
                        insert_after: Some(sid("template.country-to-capital")),
                        name: None,
                        variables: BTreeMap::new(),
                        question_format: None,
                        answer_format: None,
                        adapter_ids: BTreeMap::new(),
                        expected_base: None,
                    },
                )]),
                adapter_ids: BTreeMap::new(),
                expected_base: None,
            },
        )]),
    };

    let resolved = base.compose(&[overlay]).expect("template overlay composes");
    let note_type = resolved.note_types.get(&sid("note-type.country")).unwrap();
    assert_eq!(note_type.styling, ".card { font-family: serif; }\n");
    assert_eq!(
        note_type
            .card_templates
            .iter()
            .map(|template| template.id.clone())
            .collect::<Vec<_>>(),
        vec![
            sid("template.country-to-capital"),
            sid("template.country-to-flag")
        ]
    );
    assert_eq!(note_type.card_templates[1].answer_format, "{{Flag}}");
}

#[test]
fn overlay_can_change_note_tags_and_media_references() {
    let base = ug_style_deck();
    let overlay = Overlay {
        id: sid("overlay.extension.media-tags"),
        kind: OverlayKind::Extension,
        translations: None,
        deck_change: None,
        note_type_changes: BTreeMap::new(),
        note_changes: BTreeMap::from([(
            sid("note.finland"),
            NoteChange {
                intent: ChangeIntent::Merge,
                note: None,
                variables: BTreeMap::new(),
                fields: BTreeMap::new(),
                tags: BTreeMap::from([
                    (
                        "UG::Nordic".to_owned(),
                        TagChange {
                            intent: ChangeIntent::Add,
                            expected_base: None,
                        },
                    ),
                    (
                        "Nordic".to_owned(),
                        TagChange {
                            intent: ChangeIntent::Remove,
                            expected_base: Some(ExpectedBase::Value("Nordic".to_owned())),
                        },
                    ),
                ]),
                adapter_ids: BTreeMap::new(),
                expected_base: None,
            },
        )]),
        media_changes: BTreeMap::from([(
            sid("media.flag.sweden"),
            MediaChange {
                intent: ChangeIntent::Add,
                media: Some(MediaReference {
                    id: sid("media.flag.sweden"),
                    path: "flags/se.png".to_owned(),
                    sha256: "abcdef".to_owned(),
                }),
                expected_base: None,
            },
        )]),
    };

    let resolved = base
        .compose(&[overlay])
        .expect("tag/media overlay composes");
    let note = resolved.notes.get(&sid("note.finland")).unwrap();
    assert!(note.tags.contains("UG::Nordic"));
    assert!(!note.tags.contains("Nordic"));
    assert_eq!(
        resolved.media.get(&sid("media.flag.sweden")).unwrap().path,
        "flags/se.png"
    );
}

#[test]
fn remove_overlay_can_tombstone_an_unused_note_type() {
    let mut base = ug_style_deck();
    base.note_types.insert(
        sid("note-type.region"),
        NoteType {
            id: sid("note-type.region"),
            name: "Region".to_owned(),
            variables: BTreeMap::new(),
            fields: vec![FieldDefinition {
                id: sid("field.region"),
                name: "Region".to_owned(),
            }],
            card_templates: vec![CardTemplate {
                id: sid("template.region"),
                name: "Region".to_owned(),
                variables: BTreeMap::new(),
                question_format: "{{Region}}".to_owned(),
                answer_format: "{{Region}}".to_owned(),
                adapter_ids: AdapterIds::new(),
            }],
            styling: ".card {}\n".to_owned(),
            adapter_ids: AdapterIds::new(),
        },
    );
    let overlay = Overlay {
        id: sid("overlay.patch.remove-region-type"),
        kind: OverlayKind::Patch,
        translations: None,
        deck_change: None,
        note_changes: BTreeMap::new(),
        note_type_changes: BTreeMap::from([(
            sid("note-type.region"),
            NoteTypeChange {
                intent: ChangeIntent::Remove,
                note_type: None,
                name: None,
                variables: BTreeMap::new(),
                styling: None,
                fields: BTreeMap::new(),
                card_templates: BTreeMap::new(),
                adapter_ids: BTreeMap::new(),
                expected_base: Some(ExpectedBase::EntityFingerprint(fingerprint_note_type(
                    &base.note_types[&sid("note-type.region")],
                ))),
            },
        )]),
        media_changes: BTreeMap::new(),
    };

    let resolved = base.compose(&[overlay]).expect("remove composes");

    assert!(!resolved.note_types.contains_key(&sid("note-type.region")));
    assert!(
        resolved
            .tombstones
            .contains_address(&TombstoneAddress::NoteType {
                note_type_id: sid("note-type.region"),
            })
    );
}

#[test]
fn remove_overlay_records_a_tombstone_without_erasing_the_entity_from_resolved_deck() {
    let base = ug_style_deck();
    let overlay = Overlay {
        id: sid("overlay.patch.remove-finland"),
        kind: OverlayKind::Patch,
        translations: None,
        deck_change: None,
        note_changes: BTreeMap::from([(
            sid("note.finland"),
            NoteChange {
                intent: ChangeIntent::Remove,
                note: None,
                variables: BTreeMap::new(),
                fields: BTreeMap::new(),
                tags: BTreeMap::new(),
                adapter_ids: BTreeMap::new(),
                expected_base: Some(ExpectedBase::EntityFingerprint(fingerprint_note(
                    &base.notes[&sid("note.finland")],
                ))),
            },
        )]),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    };

    let resolved = base.compose(&[overlay]).expect("remove composes");

    assert!(resolved.notes.contains_key(&sid("note.finland")));
    assert!(
        resolved
            .tombstones
            .contains_address(&TombstoneAddress::Note {
                note_id: sid("note.finland"),
            })
    );
}

#[test]
fn exact_tombstone_reintroduction_fails_with_original_removal_provenance() {
    let base = ug_style_deck();
    let remove = Overlay {
        id: sid("overlay.patch.remove-finland"),
        kind: OverlayKind::Patch,
        translations: None,
        deck_change: None,
        note_changes: BTreeMap::from([(
            sid("note.finland"),
            NoteChange {
                intent: ChangeIntent::Remove,
                note: None,
                variables: BTreeMap::new(),
                fields: BTreeMap::new(),
                tags: BTreeMap::new(),
                adapter_ids: BTreeMap::new(),
                expected_base: Some(ExpectedBase::EntityFingerprint(fingerprint_note(
                    &base.notes[&sid("note.finland")],
                ))),
            },
        )]),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    };
    let reintroduce = Overlay {
        id: sid("overlay.extension.reintroduce-finland"),
        kind: OverlayKind::Extension,
        translations: None,
        deck_change: None,
        note_changes: BTreeMap::from([(
            sid("note.finland"),
            NoteChange {
                intent: ChangeIntent::Add,
                note: Some(base.notes[&sid("note.finland")].clone()),
                variables: BTreeMap::new(),
                fields: BTreeMap::new(),
                tags: BTreeMap::new(),
                adapter_ids: BTreeMap::new(),
                expected_base: None,
            },
        )]),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    };

    let report = base
        .compose(&[remove, reintroduce])
        .expect_err("an exact removed address cannot be reused");
    let error = report
        .errors
        .iter()
        .find(|error| error.kind == ComposeErrorKind::TombstonedAddressReuse)
        .expect("structured tombstone reuse error");
    assert_eq!(error.path, "notes.note.finland");
    assert_eq!(error.intent, Some(ChangeIntent::Add));
    assert_eq!(
        error.overlay_id.as_ref(),
        Some(&sid("overlay.extension.reintroduce-finland"))
    );
    let removal = error.original_removal.as_ref().expect("removal provenance");
    assert_eq!(removal.address.kind(), "note");
    assert_eq!(
        removal.provenance.as_ref().unwrap().overlay_id,
        sid("overlay.patch.remove-finland")
    );
}

#[test]
fn identical_stable_ids_in_different_kinds_do_not_alias() {
    let mut base = ug_style_deck();
    let shared_id = sid("entity.shared");
    base.note_types.insert(
        shared_id.clone(),
        NoteType {
            id: shared_id.clone(),
            name: "Unused shared ID type".to_owned(),
            variables: BTreeMap::new(),
            fields: Vec::new(),
            card_templates: Vec::new(),
            styling: String::new(),
            adapter_ids: AdapterIds::new(),
        },
    );
    base.media.insert(
        shared_id.clone(),
        MediaReference {
            id: shared_id.clone(),
            path: "shared.bin".to_owned(),
            sha256: "shared".to_owned(),
        },
    );
    let overlay = Overlay {
        id: sid("overlay.patch.remove-shared-type"),
        kind: OverlayKind::Patch,
        translations: None,
        deck_change: None,
        note_changes: BTreeMap::new(),
        note_type_changes: BTreeMap::from([(
            shared_id.clone(),
            NoteTypeChange {
                intent: ChangeIntent::Remove,
                note_type: None,
                name: None,
                variables: BTreeMap::new(),
                styling: None,
                fields: BTreeMap::new(),
                card_templates: BTreeMap::new(),
                adapter_ids: BTreeMap::new(),
                expected_base: Some(ExpectedBase::EntityFingerprint(fingerprint_note_type(
                    &base.note_types[&shared_id],
                ))),
            },
        )]),
        media_changes: BTreeMap::new(),
    };

    let resolved = base.compose(&[overlay]).expect("typed removal composes");
    assert!(resolved.media.contains_key(&shared_id));
    assert!(
        resolved
            .tombstones
            .contains_address(&TombstoneAddress::NoteType {
                note_type_id: shared_id.clone(),
            })
    );
    assert!(
        !resolved
            .tombstones
            .contains_address(&TombstoneAddress::MediaReference {
                media_id: shared_id,
            })
    );
}

#[test]
fn identical_nested_ids_under_different_parents_do_not_alias() {
    let mut base = ug_style_deck();
    let mut second = base.note_types[&sid("note-type.country")].clone();
    second.id = sid("note-type.region");
    second.name = "Region".to_owned();
    base.note_types.insert(second.id.clone(), second);
    base.notes.clear();
    base.tombstones
        .insert(brain_brew_core::TombstoneRecord::legacy(
            TombstoneAddress::FieldDefinition {
                note_type_id: sid("note-type.country"),
                field_id: sid("field.flag"),
            },
        ));
    let overlay = Overlay {
        id: sid("overlay.patch.rename-region-flag"),
        kind: OverlayKind::Patch,
        translations: None,
        deck_change: None,
        note_changes: BTreeMap::new(),
        note_type_changes: BTreeMap::from([(
            sid("note-type.region"),
            NoteTypeChange {
                intent: ChangeIntent::Merge,
                note_type: None,
                name: None,
                variables: BTreeMap::new(),
                styling: None,
                fields: BTreeMap::from([(
                    sid("field.flag"),
                    FieldDefinitionChange {
                        intent: ChangeIntent::Replace,
                        field: Some(FieldDefinition {
                            id: sid("field.flag"),
                            name: "Region flag".to_owned(),
                        }),
                        expected_base: Some(ExpectedBase::EntityFingerprint(
                            fingerprint_field_definition(
                                &base.note_types[&sid("note-type.region")].fields[2],
                            ),
                        )),
                    },
                )]),
                card_templates: BTreeMap::new(),
                adapter_ids: BTreeMap::new(),
                expected_base: None,
            },
        )]),
        media_changes: BTreeMap::new(),
    };

    let resolved = base
        .compose(&[overlay])
        .expect("different parent remains independent");
    assert_eq!(
        resolved.note_types[&sid("note-type.region")].fields[2].name,
        "Region flag"
    );
}

#[test]
fn tombstoned_notes_do_not_block_note_type_removal_and_validate_accepts_result() {
    let base = ug_style_deck();
    let tombstone_note = Overlay {
        id: sid("overlay.patch.tombstone-finland"),
        kind: OverlayKind::Patch,
        translations: None,
        deck_change: None,
        note_changes: BTreeMap::from([(
            sid("note.finland"),
            NoteChange {
                intent: ChangeIntent::Remove,
                note: None,
                variables: BTreeMap::new(),
                fields: BTreeMap::new(),
                tags: BTreeMap::new(),
                adapter_ids: BTreeMap::new(),
                expected_base: Some(ExpectedBase::EntityFingerprint(fingerprint_note(
                    &base.notes[&sid("note.finland")],
                ))),
            },
        )]),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    };
    let remove_type = Overlay {
        id: sid("overlay.patch.remove-country-type"),
        kind: OverlayKind::Patch,
        translations: None,
        deck_change: None,
        note_changes: BTreeMap::new(),
        note_type_changes: BTreeMap::from([(
            sid("note-type.country"),
            NoteTypeChange {
                intent: ChangeIntent::Remove,
                note_type: None,
                name: None,
                variables: BTreeMap::new(),
                styling: None,
                fields: BTreeMap::new(),
                card_templates: BTreeMap::new(),
                adapter_ids: BTreeMap::new(),
                expected_base: Some(ExpectedBase::EntityFingerprint(fingerprint_note_type(
                    &base.note_types[&sid("note-type.country")],
                ))),
            },
        )]),
        media_changes: BTreeMap::new(),
    };

    let resolved = base
        .compose(&[tombstone_note, remove_type])
        .expect("tombstoned references do not block note-type removal");

    assert!(!resolved.note_types.contains_key(&sid("note-type.country")));
    assert!(resolved.notes.contains_key(&sid("note.finland")));
    assert!(
        resolved
            .tombstones
            .contains_address(&TombstoneAddress::Note {
                note_id: sid("note.finland"),
            })
    );
    resolved
        .validate()
        .expect("tombstoned notes may retain a now-missing note type");
}

#[test]
fn non_tombstoned_note_still_blocks_note_type_removal_and_validation() {
    let base = ug_style_deck();
    let report = base
        .compose(&[Overlay {
            id: sid("overlay.patch.remove-country-type"),
            kind: OverlayKind::Patch,
            translations: None,
            deck_change: None,
            note_changes: BTreeMap::new(),
            note_type_changes: BTreeMap::from([(
                sid("note-type.country"),
                NoteTypeChange {
                    intent: ChangeIntent::Remove,
                    note_type: None,
                    name: None,
                    variables: BTreeMap::new(),
                    styling: None,
                    fields: BTreeMap::new(),
                    card_templates: BTreeMap::new(),
                    adapter_ids: BTreeMap::new(),
                    expected_base: Some(ExpectedBase::EntityFingerprint(fingerprint_note_type(
                        &base.note_types[&sid("note-type.country")],
                    ))),
                },
            )]),
            media_changes: BTreeMap::new(),
        }])
        .expect_err("live note references still block note-type removal");

    assert!(report.has_kind(ComposeErrorKind::ValidationFailed));
    assert!(report.errors.iter().any(|error| {
        error
            .message
            .contains("cannot remove note type note-type.country while notes still reference it")
    }));

    let mut invalid = ug_style_deck();
    invalid.note_types.remove(&sid("note-type.country"));
    let validation = invalid
        .validate()
        .expect_err("live missing note type reference fails validation");
    assert!(validation.has_kind(ValidationErrorKind::MissingNoteType));
}

#[test]
fn final_validation_preserves_original_typed_children_and_source_attribution() {
    let mut invalid = ug_style_deck();
    invalid.note_types.remove(&sid("note-type.country"));

    let report = invalid
        .compose(&[])
        .expect_err("final validation rejects the invalid canonical source");
    assert_eq!(report.errors.len(), 1);
    let error = &report.errors[0];
    assert_eq!(error.kind, ComposeErrorKind::ValidationFailed);
    assert_eq!(error.source_id.as_ref(), Some(&invalid.id));
    assert!(error.validation_errors.iter().any(|issue| {
        issue.kind == ValidationErrorKind::MissingNoteType
            && issue.deck_path.as_ref().map(ToString::to_string).as_deref()
                == Some("notes.note.finland.note_type_id")
    }));
    let diagnostic = error.diagnostic();
    assert!(
        diagnostic
            .children
            .iter()
            .any(|child| child.code == "missing_note_type")
    );
}

#[test]
fn complete_note_replace_and_override_require_the_current_fingerprint() {
    for intent in [ChangeIntent::Replace, ChangeIntent::Override] {
        let base = ug_style_deck();
        let mut replacement = base.notes[&sid("note.finland")].clone();
        replacement
            .fields
            .insert(sid("field.capital"), "Helsingfors");
        let overlay = Overlay {
            id: sid("overlay.patch.full-note-body"),
            kind: OverlayKind::Patch,
            translations: None,
            deck_change: None,
            note_changes: BTreeMap::from([(
                sid("note.finland"),
                NoteChange {
                    intent,
                    note: Some(replacement),
                    variables: BTreeMap::new(),
                    fields: BTreeMap::new(),
                    tags: BTreeMap::new(),
                    adapter_ids: BTreeMap::new(),
                    expected_base: Some(ExpectedBase::EntityFingerprint(fingerprint_note(
                        &base.notes[&sid("note.finland")],
                    ))),
                },
            )]),
            note_type_changes: BTreeMap::new(),
            media_changes: BTreeMap::new(),
        };

        let resolved = base
            .compose(&[overlay])
            .expect("matching note fingerprint applies");
        assert_eq!(
            resolved.notes[&sid("note.finland")].fields[&sid("field.capital")],
            "Helsingfors"
        );
    }
}

#[test]
fn complete_note_type_replace_and_override_require_the_current_fingerprint() {
    for intent in [ChangeIntent::Replace, ChangeIntent::Override] {
        let base = ug_style_deck();
        let mut replacement = base.note_types[&sid("note-type.country")].clone();
        replacement.name = "Countries updated".to_owned();
        let overlay = Overlay {
            id: sid("overlay.patch.full-note-type-body"),
            kind: OverlayKind::Patch,
            translations: None,
            deck_change: None,
            note_changes: BTreeMap::new(),
            note_type_changes: BTreeMap::from([(
                sid("note-type.country"),
                NoteTypeChange {
                    intent,
                    note_type: Some(replacement),
                    name: None,
                    variables: BTreeMap::new(),
                    styling: None,
                    fields: BTreeMap::new(),
                    card_templates: BTreeMap::new(),
                    adapter_ids: BTreeMap::new(),
                    expected_base: Some(ExpectedBase::EntityFingerprint(fingerprint_note_type(
                        &base.note_types[&sid("note-type.country")],
                    ))),
                },
            )]),
            media_changes: BTreeMap::new(),
        };

        let resolved = base
            .compose(&[overlay])
            .expect("matching note-type fingerprint applies");
        assert_eq!(
            resolved.note_types[&sid("note-type.country")].name,
            "Countries updated"
        );
    }
}

#[test]
fn field_level_merge_only_fills_blank_values() {
    let mut base = ug_style_deck();
    base.notes
        .get_mut(&sid("note.finland"))
        .unwrap()
        .fields
        .insert(sid("field.flag"), String::new());

    let resolved = base
        .compose(&[overlay_with_field_change(
            "overlay.patch.fill-flag",
            sid("field.flag"),
            FieldChange {
                intent: ChangeIntent::Merge,
                value: Some(("<img src=\"fi-new.png\">".to_owned()).into()),
                expected_base: None,
            },
        )])
        .expect("merge fills blank field");

    assert_eq!(
        resolved.notes[&sid("note.finland")].fields[&sid("field.flag")],
        "<img src=\"fi-new.png\">"
    );
}

#[test]
fn field_level_merge_rejects_non_blank_values_and_replace_with_expected_base_overwrites() {
    let report = ug_style_deck()
        .compose(&[overlay_with_field_change(
            "overlay.patch.merge-capital",
            sid("field.capital"),
            FieldChange {
                intent: ChangeIntent::Merge,
                value: Some(("Helsingfors".to_owned()).into()),
                expected_base: None,
            },
        )])
        .expect_err("merge must not overwrite non-blank field");

    assert!(report.has_kind(ComposeErrorKind::ExpectedBaseMismatch));
    assert!(report.errors.iter().any(|error| {
        error.path == "notes.note.finland.fields.field.capital"
            && error.message.contains("may only fill a blank scalar value")
    }));

    let resolved = ug_style_deck()
        .compose(&[overlay_with_field_change(
            "overlay.patch.replace-capital",
            sid("field.capital"),
            FieldChange {
                intent: ChangeIntent::Replace,
                value: Some(("Helsingfors".to_owned()).into()),
                expected_base: Some(ExpectedBase::Value("Helsinki".to_owned())),
            },
        )])
        .expect("replace with expected_base can overwrite deliberately");

    assert_eq!(
        resolved.notes[&sid("note.finland")].fields[&sid("field.capital")],
        "Helsingfors"
    );
}

#[test]
fn field_level_merge_rejects_non_blank_for_structured_messages_and_images() {
    for change in [
        FieldChange {
            intent: ChangeIntent::Merge,
            value: Some(FieldValue::Message(StructuredMessage {
                components: vec![MessageComponent::Text("structured".to_owned())],
                format: None,
                variables: BTreeMap::new(),
            })),
            expected_base: None,
        },
        FieldChange {
            intent: ChangeIntent::Merge,
            value: Some(FieldValue::Images(vec![FieldImageReference {
                media_id: sid("media.flag.finland"),
            }])),
            expected_base: None,
        },
    ] {
        let report = ug_style_deck()
            .compose(&[overlay_with_field_change(
                "overlay.patch.merge-structured-nonblank",
                sid("field.flag"),
                change,
            )])
            .expect_err("merge payloads cannot overwrite non-blank field values");

        assert!(report.has_kind(ComposeErrorKind::ExpectedBaseMismatch));
    }
}

#[test]
fn missing_field_definition_rejects_merge_replace_and_override() {
    for intent in [
        ChangeIntent::Merge,
        ChangeIntent::Replace,
        ChangeIntent::Override,
    ] {
        let overlay = Overlay {
            id: sid("overlay.patch.missing-field-definition"),
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
                    fields: BTreeMap::from([(
                        sid("field.population"),
                        FieldDefinitionChange {
                            intent,
                            field: Some(FieldDefinition {
                                id: sid("field.population"),
                                name: "Population".to_owned(),
                            }),
                            expected_base: Some(ExpectedBase::EntityFingerprint(
                                fingerprint_field_definition(
                                    &ug_style_deck().note_types[&sid("note-type.country")].fields
                                        [0],
                                ),
                            )),
                        },
                    )]),
                    card_templates: BTreeMap::new(),
                    adapter_ids: BTreeMap::new(),
                    expected_base: None,
                },
            )]),
            media_changes: BTreeMap::new(),
        };

        let report = ug_style_deck()
            .compose(&[overlay])
            .expect_err("non-add field definition changes require an existing target");

        assert!(report.has_kind(ComposeErrorKind::ExpectedBaseMismatch));
        assert!(
            report.errors.iter().any(|error| {
                error.path == "note_types.note-type.country.fields.field.population"
            })
        );
    }
}

#[test]
fn field_add_fills_blank_but_rejects_non_blank_existing_values() {
    let mut base = ug_style_deck();
    base.notes
        .get_mut(&sid("note.finland"))
        .unwrap()
        .fields
        .insert(sid("field.flag"), String::new());

    let filled = base
        .compose(&[overlay_with_field_change(
            "overlay.patch.add-blank-flag",
            sid("field.flag"),
            FieldChange {
                intent: ChangeIntent::Add,
                value: Some(("filled flag".to_owned()).into()),
                expected_base: None,
            },
        )])
        .expect("add fills an existing blank field value");
    assert_eq!(
        filled.notes[&sid("note.finland")].fields[&sid("field.flag")],
        "filled flag"
    );

    let report = ug_style_deck()
        .compose(&[overlay_with_field_change(
            "overlay.patch.add-nonblank-capital",
            sid("field.capital"),
            FieldChange {
                intent: ChangeIntent::Add,
                value: Some(("Helsingfors".to_owned()).into()),
                expected_base: None,
            },
        )])
        .expect_err("add cannot overwrite non-blank field value");

    assert!(report.has_kind(ComposeErrorKind::AlreadyExists));
}

#[test]
fn note_add_rejects_payload_id_mismatch() {
    let report = ug_style_deck()
        .compose(&[Overlay {
            id: sid("overlay.extension.bad-note-payload"),
            kind: OverlayKind::Extension,
            translations: None,
            deck_change: None,
            note_changes: BTreeMap::from([(
                sid("note.denmark"),
                NoteChange {
                    intent: ChangeIntent::Add,
                    note: Some(sweden_note()),
                    variables: BTreeMap::new(),
                    fields: BTreeMap::new(),
                    tags: BTreeMap::new(),
                    adapter_ids: BTreeMap::new(),
                    expected_base: None,
                },
            )]),
            note_type_changes: BTreeMap::new(),
            media_changes: BTreeMap::new(),
        }])
        .expect_err("mismatched note payload id fails validation");

    assert!(report.has_kind(ComposeErrorKind::ValidationFailed));
    assert!(report.errors.iter().any(|error| {
        error
            .message
            .contains("note payload id note.sweden does not match target note.denmark")
    }));
}

#[test]
fn destructive_operation_matrix_covers_core_change_families() {
    let mut base = ug_style_deck();
    base.variables
        .insert("deck.locale".to_owned(), "en".to_owned());

    let field_definition = &base.note_types[&sid("note-type.country")].fields[2];
    let mut remove_field_definition = field_definition_overlay(
        ChangeIntent::Remove,
        None,
        Some(ExpectedBase::EntityFingerprint(
            fingerprint_field_definition(field_definition),
        )),
    );
    remove_field_definition.note_changes = BTreeMap::from([(
        sid("note.finland"),
        NoteChange {
            intent: ChangeIntent::Merge,
            note: None,
            variables: BTreeMap::new(),
            fields: BTreeMap::from([(
                sid("field.flag"),
                FieldChange {
                    intent: ChangeIntent::Remove,
                    value: None,
                    expected_base: Some(ExpectedBase::FieldValue(
                        base.notes[&sid("note.finland")].fields[&sid("field.flag")].clone(),
                    )),
                },
            )]),
            tags: BTreeMap::new(),
            adapter_ids: BTreeMap::new(),
            expected_base: None,
        },
    )]);
    let field_removed = base
        .compose(&[remove_field_definition])
        .expect("field definition remove composes when note values are removed too");
    assert!(
        !field_removed.note_types[&sid("note-type.country")]
            .fields
            .iter()
            .any(|field| field.id == sid("field.flag"))
    );
    assert!(
        field_removed
            .tombstones
            .contains_address(&TombstoneAddress::FieldDefinition {
                note_type_id: sid("note-type.country"),
                field_id: sid("field.flag"),
            })
    );
    assert!(
        field_removed
            .tombstones
            .contains_address(&TombstoneAddress::NoteField {
                note_id: sid("note.finland"),
                field_id: sid("field.flag"),
            })
    );

    let field_replaced = base
        .compose(&[field_definition_overlay(
            ChangeIntent::Replace,
            Some(FieldDefinition {
                id: sid("field.flag"),
                name: "Flag image".to_owned(),
            }),
            Some(ExpectedBase::EntityFingerprint(
                fingerprint_field_definition(field_definition),
            )),
        )])
        .expect("field definition replace composes");
    assert_eq!(
        field_replaced.note_types[&sid("note-type.country")].fields[2].name,
        "Flag image"
    );

    let template_removed = base
        .compose(&[card_template_overlay(
            ChangeIntent::Remove,
            None,
            Some(ExpectedBase::EntityFingerprint(fingerprint_card_template(
                &base.note_types[&sid("note-type.country")].card_templates[0],
            ))),
        )])
        .expect("card template remove composes");
    assert!(
        template_removed.note_types[&sid("note-type.country")]
            .card_templates
            .is_empty()
    );
    assert!(
        template_removed
            .tombstones
            .contains_address(&TombstoneAddress::CardTemplate {
                note_type_id: sid("note-type.country"),
                template_id: sid("template.country-to-capital"),
            })
    );

    let template_replaced = base
        .compose(&[card_template_overlay(
            ChangeIntent::Replace,
            Some(CardTemplate {
                id: sid("template.country-to-capital"),
                name: "Country - Capital updated".to_owned(),
                variables: BTreeMap::new(),
                question_format: "{{Country}}?".to_owned(),
                answer_format: "{{Capital}}".to_owned(),
                adapter_ids: AdapterIds::new(),
            }),
            Some(ExpectedBase::EntityFingerprint(fingerprint_card_template(
                &base.note_types[&sid("note-type.country")].card_templates[0],
            ))),
        )])
        .expect("card template replace composes");
    assert_eq!(
        template_replaced.note_types[&sid("note-type.country")].card_templates[0].name,
        "Country - Capital updated"
    );

    let media_removed = base
        .compose(&[media_overlay(
            ChangeIntent::Remove,
            None,
            Some(ExpectedBase::EntityFingerprint(
                fingerprint_media_reference(&base.media[&sid("media.flag.finland")]),
            )),
        )])
        .expect("media remove composes");
    assert!(!media_removed.media.contains_key(&sid("media.flag.finland")));
    assert!(
        media_removed
            .tombstones
            .contains_address(&TombstoneAddress::MediaReference {
                media_id: sid("media.flag.finland"),
            })
    );

    let media_replaced = base
        .compose(&[media_overlay(
            ChangeIntent::Replace,
            Some(MediaReference {
                id: sid("media.flag.finland"),
                path: "flags/fi-new.png".to_owned(),
                sha256: "fedcba".to_owned(),
            }),
            Some(ExpectedBase::EntityFingerprint(
                fingerprint_media_reference(&base.media[&sid("media.flag.finland")]),
            )),
        )])
        .expect("media replace composes");
    assert_eq!(
        media_replaced.media[&sid("media.flag.finland")].path,
        "flags/fi-new.png"
    );

    let name_removed = base
        .compose(&[deck_name_overlay(
            ChangeIntent::Remove,
            None,
            Some(ExpectedBase::Value("Ultimate Geography".to_owned())),
        )])
        .expect("string property remove composes");
    assert!(name_removed.name.is_empty());
    assert!(
        name_removed
            .tombstones
            .contains_address(&TombstoneAddress::DeckName)
    );

    let name_replaced = base
        .compose(&[deck_name_overlay(
            ChangeIntent::Replace,
            Some("Ultimate Geography v2"),
            Some(ExpectedBase::Value("Ultimate Geography".to_owned())),
        )])
        .expect("string property replace composes");
    assert_eq!(name_replaced.name, "Ultimate Geography v2");

    let variable_removed = base
        .compose(&[deck_variable_overlay(
            ChangeIntent::Remove,
            None,
            Some(ExpectedBase::Value("en".to_owned())),
        )])
        .expect("variable remove composes");
    assert!(!variable_removed.variables.contains_key("deck.locale"));
    assert!(
        variable_removed
            .tombstones
            .contains_address(&TombstoneAddress::DeckVariable {
                key: "deck.locale".to_owned(),
            })
    );

    let variable_replaced = base
        .compose(&[deck_variable_overlay(
            ChangeIntent::Replace,
            Some("fi"),
            Some(ExpectedBase::Value("en".to_owned())),
        )])
        .expect("variable replace composes");
    assert_eq!(variable_replaced.variables["deck.locale"], "fi");
}

#[test]
fn entity_precondition_matrix_rejects_missing_stale_presence_only_and_ordered_overrides() {
    let base = ug_style_deck();
    let media_id = sid("media.flag.finland");
    let original = &base.media[&media_id];
    let original_fingerprint = fingerprint_media_reference(original);
    let replacement = MediaReference {
        id: media_id.clone(),
        path: "flags/fi-v2.png".to_owned(),
        sha256: "v2".to_owned(),
    };

    let mut matching = media_overlay(
        ChangeIntent::Replace,
        Some(replacement.clone()),
        Some(ExpectedBase::EntityFingerprint(
            original_fingerprint.clone(),
        )),
    );
    matching.id = sid("overlay.patch.media.first");
    let resolved = base
        .compose(&[matching.clone()])
        .expect("matching fingerprint applies");
    assert_eq!(resolved.media[&media_id], replacement);

    let mut wrong_present = media_overlay(
        ChangeIntent::Replace,
        Some(replacement.clone()),
        Some(ExpectedBase::EntityFingerprint(
            fingerprint_media_reference(&MediaReference {
                id: media_id.clone(),
                path: "flags/wrong.png".to_owned(),
                sha256: "wrong".to_owned(),
            }),
        )),
    );
    wrong_present.id = sid("overlay.patch.media.wrong");
    let report = base
        .compose(&[wrong_present])
        .expect_err("wrong-but-present fingerprint fails");
    let error = report
        .errors
        .iter()
        .find(|error| error.kind == ComposeErrorKind::ExpectedBaseMismatch)
        .unwrap();
    assert_eq!(
        error.deck_path.as_ref().unwrap().to_string(),
        "media.media.flag.finland"
    );
    assert_eq!(error.entity_kind.unwrap().as_str(), "media_reference");
    assert_eq!(error.intent, Some(ChangeIntent::Replace));
    assert_eq!(
        error.overlay_id.as_ref().unwrap(),
        &sid("overlay.patch.media.wrong")
    );
    assert!(error.expected.is_some() && error.actual.is_some());

    let report = base
        .compose(&[media_overlay(
            ChangeIntent::Replace,
            Some(replacement.clone()),
            Some(ExpectedBase::EntityFingerprint(fingerprint_note(
                &base.notes[&sid("note.finland")],
            ))),
        )])
        .expect_err("fingerprint from the wrong entity kind cannot match");
    assert!(report.has_kind(ComposeErrorKind::ExpectedBaseMismatch));

    let mut missing = base.clone();
    missing.media.remove(&media_id);
    let report = missing
        .compose(&[matching.clone()])
        .expect_err("missing current entity fails the fingerprint precondition");
    assert!(report.has_kind(ComposeErrorKind::ExpectedBaseMismatch));
    assert!(matches!(
        report.errors[0].actual,
        Some(brain_brew_core::ComposePrecondition::Missing)
    ));

    let report = base
        .compose(&[media_overlay(
            ChangeIntent::Remove,
            None,
            Some(ExpectedBase::EntityPresent),
        )])
        .expect_err("legacy presence-only baseline fails closed");
    assert!(report.has_kind(ComposeErrorKind::InvalidExpectedBase));

    let mut valid_override = media_overlay(
        ChangeIntent::Override,
        Some(MediaReference {
            id: media_id.clone(),
            path: "flags/fi-v3-valid.png".to_owned(),
            sha256: "v3-valid".to_owned(),
        }),
        Some(ExpectedBase::EntityFingerprint(
            fingerprint_media_reference(&replacement),
        )),
    );
    valid_override.id = sid("overlay.patch.media.valid-override");
    let resolved = base
        .compose(&[matching.clone(), valid_override])
        .expect("override resolves provenance after its current-state fingerprint matches");
    assert_eq!(resolved.media[&media_id].path, "flags/fi-v3-valid.png");

    let mut stale_replace = media_overlay(
        ChangeIntent::Replace,
        Some(MediaReference {
            id: media_id.clone(),
            path: "flags/fi-concurrent.png".to_owned(),
            sha256: "concurrent".to_owned(),
        }),
        Some(ExpectedBase::EntityFingerprint(
            original_fingerprint.clone(),
        )),
    );
    stale_replace.id = sid("overlay.patch.media.concurrent");
    let report = base
        .compose(&[matching.clone(), stale_replace])
        .expect_err("concurrent ordered overlay checks the mutated current entity");
    assert!(report.has_kind(ComposeErrorKind::ExpectedBaseMismatch));
    assert!(!report.has_kind(ComposeErrorKind::Conflict));

    let mut stale_override = media_overlay(
        ChangeIntent::Override,
        Some(MediaReference {
            id: media_id,
            path: "flags/fi-v3.png".to_owned(),
            sha256: "v3".to_owned(),
        }),
        Some(ExpectedBase::EntityFingerprint(original_fingerprint)),
    );
    stale_override.id = sid("overlay.patch.media.stale-override");
    let report = base
        .compose(&[matching, stale_override])
        .expect_err("override cannot bypass stale current state");
    assert!(report.has_kind(ComposeErrorKind::ExpectedBaseMismatch));
    assert!(!report.has_kind(ComposeErrorKind::Conflict));
    assert_eq!(
        report.errors[0].overlay_id.as_ref().unwrap(),
        &sid("overlay.patch.media.stale-override")
    );
}

#[test]
fn merge_and_override_variants_compose_where_supported() {
    let mut base = ug_style_deck();
    base.variables
        .insert("deck.locale".to_owned(), "en".to_owned());

    let field_merged = base
        .compose(&[field_definition_overlay(
            ChangeIntent::Merge,
            Some(FieldDefinition {
                id: sid("field.flag"),
                name: "Flag merged".to_owned(),
            }),
            Some(ExpectedBase::EntityFingerprint(
                fingerprint_field_definition(&base.note_types[&sid("note-type.country")].fields[2]),
            )),
        )])
        .expect("field definition merge composes on existing field");
    assert_eq!(
        field_merged.note_types[&sid("note-type.country")].fields[2].name,
        "Flag merged"
    );

    let template_merged = base
        .compose(&[card_template_overlay(
            ChangeIntent::Merge,
            Some(CardTemplate {
                id: sid("template.country-to-capital"),
                name: "Country - Capital merged".to_owned(),
                variables: BTreeMap::new(),
                question_format: "{{Country}}?".to_owned(),
                answer_format: "{{Capital}}".to_owned(),
                adapter_ids: AdapterIds::new(),
            }),
            Some(ExpectedBase::EntityFingerprint(fingerprint_card_template(
                &base.note_types[&sid("note-type.country")].card_templates[0],
            ))),
        )])
        .expect("card template merge composes");
    assert_eq!(
        template_merged.note_types[&sid("note-type.country")].card_templates[0].name,
        "Country - Capital merged"
    );

    let media_merged = base
        .compose(&[media_overlay(
            ChangeIntent::Merge,
            Some(MediaReference {
                id: sid("media.flag.finland"),
                path: "flags/fi-merge.png".to_owned(),
                sha256: "merge".to_owned(),
            }),
            Some(ExpectedBase::EntityFingerprint(
                fingerprint_media_reference(&base.media[&sid("media.flag.finland")]),
            )),
        )])
        .expect("media merge composes");
    assert_eq!(
        media_merged.media[&sid("media.flag.finland")].path,
        "flags/fi-merge.png"
    );

    let name_merged = base
        .compose(&[deck_name_overlay(
            ChangeIntent::Merge,
            Some("Merged name"),
            None,
        )])
        .expect("string property merge composes");
    assert_eq!(name_merged.name, "Merged name");

    let variable_merged = base
        .compose(&[deck_variable_overlay(ChangeIntent::Merge, Some("sv"), None)])
        .expect("variable merge composes");
    assert_eq!(variable_merged.variables["deck.locale"], "sv");

    let overridden = base
        .compose(&[
            deck_name_overlay(ChangeIntent::Merge, Some("First name"), None),
            deck_name_overlay(
                ChangeIntent::Override,
                Some("Override name"),
                Some(ExpectedBase::Value("First name".to_owned())),
            ),
        ])
        .expect("override chain composes");
    assert_eq!(overridden.name, "Override name");
}

#[test]
fn add_guards_cover_already_exists_and_payload_id_mismatch_paths() {
    let already_exists_cases = [
        already_existing_note_type_overlay(),
        already_existing_note_overlay(),
        already_existing_card_template_overlay(),
        already_existing_field_definition_overlay(),
        already_existing_media_overlay(),
    ];
    for overlay in already_exists_cases {
        let report = ug_style_deck()
            .compose(&[overlay])
            .expect_err("duplicate add fails");
        assert!(report.has_kind(ComposeErrorKind::AlreadyExists));
    }

    let payload_mismatch_cases = [
        mismatched_note_type_payload_overlay(),
        mismatched_note_payload_overlay(),
        mismatched_card_template_payload_overlay(),
        mismatched_field_definition_payload_overlay(),
        mismatched_media_payload_overlay(),
    ];
    for overlay in payload_mismatch_cases {
        let report = ug_style_deck()
            .compose(&[overlay])
            .expect_err("payload id mismatch fails");
        assert!(report.has_kind(ComposeErrorKind::ValidationFailed));
    }
}

#[test]
fn empty_stack_returns_the_base_deck_unchanged() {
    let base = ug_style_deck();

    let resolved = base.compose(&[]).expect("empty overlay stack composes");

    assert_eq!(resolved, base);
}

fn overlay_replacing_capital(
    intent: ChangeIntent,
    expected_base: Option<ExpectedBase>,
    value: &str,
) -> Overlay {
    Overlay {
        id: sid("overlay.patch.capital"),
        kind: OverlayKind::Patch,
        translations: None,
        deck_change: None,
        note_changes: BTreeMap::from([(
            sid("note.finland"),
            NoteChange {
                intent: ChangeIntent::Merge,
                note: None,
                variables: BTreeMap::new(),
                fields: BTreeMap::from([(
                    sid("field.capital"),
                    FieldChange {
                        intent,
                        value: Some((value.to_owned()).into()),
                        expected_base,
                    },
                )]),
                tags: BTreeMap::new(),
                adapter_ids: BTreeMap::new(),
                expected_base: None,
            },
        )]),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    }
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
        ])
        .into(),
        tags: BTreeSet::from(["Europe".to_owned(), "Nordic".to_owned()]),
        adapter_ids: note_adapter_ids,
    };

    let mut deck_adapter_ids = AdapterIds::new();
    deck_adapter_ids.insert("crowdanki:uuid", "43c5ba66-9a65-11e8-90c9-a0481cc15658");

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

fn ug_style_deck_with_variable_field_ref_message(target_note: &str) -> CanonicalDeck {
    let mut deck = ug_style_deck();
    deck.note_types
        .get_mut(&sid("note-type.country"))
        .unwrap()
        .fields
        .push(FieldDefinition {
            id: sid("field.summary"),
            name: "Summary".to_owned(),
        });
    deck.notes.insert(
        sid("note.iceland"),
        Note {
            id: sid("note.iceland"),
            note_type_id: sid("note-type.country"),
            variables: BTreeMap::new(),
            fields: BTreeMap::from([
                (sid("field.country"), "Iceland".to_owned()),
                (sid("field.capital"), "Reykjavik".to_owned()),
                (sid("field.flag"), "<img src=\"is.png\">".to_owned()),
                (sid("field.summary"), String::new()),
            ])
            .into(),
            tags: BTreeSet::from(["Europe".to_owned(), "Nordic".to_owned()]),
            adapter_ids: AdapterIds::new(),
        },
    );
    let finland = deck.notes.get_mut(&sid("note.finland")).unwrap();
    finland
        .variables
        .insert("target.note".to_owned(), target_note.to_owned());
    finland.fields.insert(
        sid("field.summary"),
        FieldValue::Message(StructuredMessage {
            components: vec![MessageComponent::FieldRef(
                "notes.note.${target.note}.fields.field.country".to_owned(),
            )],
            format: None,
            variables: BTreeMap::new(),
        }),
    );
    deck
}

fn ug_style_deck_with_flag_similarity_message() -> CanonicalDeck {
    let mut deck = ug_style_deck();
    deck.note_types
        .get_mut(&sid("note-type.country"))
        .unwrap()
        .fields
        .push(FieldDefinition {
            id: sid("field.flag-similarity"),
            name: "Flag similarity".to_owned(),
        });
    deck.notes.insert(
        sid("note.iceland"),
        Note {
            id: sid("note.iceland"),
            note_type_id: sid("note-type.country"),
            variables: BTreeMap::new(),
            fields: BTreeMap::from([
                (sid("field.country"), "Iceland".to_owned()),
                (sid("field.capital"), "Reykjavik".to_owned()),
                (sid("field.flag"), "<img src=\"is.png\">".to_owned()),
                (sid("field.flag-similarity"), String::new()),
            ])
            .into(),
            tags: BTreeSet::from(["Europe".to_owned(), "Nordic".to_owned()]),
            adapter_ids: AdapterIds::new(),
        },
    );
    deck.notes.insert(
        sid("note.norway"),
        Note {
            id: sid("note.norway"),
            note_type_id: sid("note-type.country"),
            variables: BTreeMap::new(),
            fields: BTreeMap::from([
                (sid("field.country"), "Norway".to_owned()),
                (sid("field.capital"), "Oslo".to_owned()),
                (sid("field.flag"), "<img src=\"no.png\">".to_owned()),
                (sid("field.flag-similarity"), String::new()),
            ])
            .into(),
            tags: BTreeSet::from(["Europe".to_owned(), "Nordic".to_owned()]),
            adapter_ids: AdapterIds::new(),
        },
    );
    let finland = deck.notes.get_mut(&sid("note.finland")).unwrap();
    finland.fields.insert(
        sid("field.flag-similarity"),
        FieldValue::Message(StructuredMessage {
            components: Vec::new(),
            format: Some("{country_1} ({description_1}), {country_2} ({description_2})".to_owned()),
            variables: BTreeMap::from([
                (
                    "country_1".to_owned(),
                    MessageComponent::FieldRef(
                        "notes.note.iceland.fields.field.country".to_owned(),
                    ),
                ),
                (
                    "description_1".to_owned(),
                    MessageComponent::Text("blue background with a white cross".to_owned()),
                ),
                (
                    "country_2".to_owned(),
                    MessageComponent::FieldRef("notes.note.norway.fields.field.country".to_owned()),
                ),
                (
                    "description_2".to_owned(),
                    MessageComponent::Text("red background with a blue cross".to_owned()),
                ),
            ]),
        }),
    );
    deck
}

fn sweden_note() -> Note {
    Note {
        id: sid("note.sweden"),
        note_type_id: sid("note-type.country"),
        variables: BTreeMap::new(),
        fields: BTreeMap::from([
            (sid("field.country"), "Sweden".to_owned()),
            (sid("field.capital"), "Stockholm".to_owned()),
            (sid("field.flag"), "<img src=\"se.png\">".to_owned()),
        ])
        .into(),
        tags: BTreeSet::from(["Europe".to_owned(), "Nordic".to_owned()]),
        adapter_ids: AdapterIds::new(),
    }
}

fn overlay_with_field_change(id: &str, field_id: StableId, change: FieldChange) -> Overlay {
    Overlay {
        id: sid(id),
        kind: OverlayKind::Patch,
        translations: None,
        deck_change: None,
        note_changes: BTreeMap::from([(
            sid("note.finland"),
            NoteChange {
                intent: ChangeIntent::Merge,
                note: None,
                variables: BTreeMap::new(),
                fields: BTreeMap::from([(field_id, change)]),
                tags: BTreeMap::new(),
                adapter_ids: BTreeMap::new(),
                expected_base: None,
            },
        )]),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    }
}

fn field_definition_overlay(
    intent: ChangeIntent,
    field: Option<FieldDefinition>,
    expected_base: Option<ExpectedBase>,
) -> Overlay {
    Overlay {
        id: sid("overlay.patch.field-definition"),
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
                fields: BTreeMap::from([(
                    sid("field.flag"),
                    FieldDefinitionChange {
                        intent,
                        field,
                        expected_base,
                    },
                )]),
                card_templates: BTreeMap::new(),
                adapter_ids: BTreeMap::new(),
                expected_base: None,
            },
        )]),
        media_changes: BTreeMap::new(),
    }
}

fn card_template_overlay(
    intent: ChangeIntent,
    template: Option<CardTemplate>,
    expected_base: Option<ExpectedBase>,
) -> Overlay {
    Overlay {
        id: sid("overlay.patch.card-template"),
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
                    sid("template.country-to-capital"),
                    CardTemplateChange {
                        intent,
                        template,
                        insert_after: None,
                        name: None,
                        variables: BTreeMap::new(),
                        question_format: None,
                        answer_format: None,
                        adapter_ids: BTreeMap::new(),
                        expected_base,
                    },
                )]),
                adapter_ids: BTreeMap::new(),
                expected_base: None,
            },
        )]),
        media_changes: BTreeMap::new(),
    }
}

fn media_overlay(
    intent: ChangeIntent,
    media: Option<MediaReference>,
    expected_base: Option<ExpectedBase>,
) -> Overlay {
    Overlay {
        id: sid("overlay.patch.media"),
        kind: OverlayKind::Patch,
        translations: None,
        deck_change: None,
        note_changes: BTreeMap::new(),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::from([(
            sid("media.flag.finland"),
            MediaChange {
                intent,
                media,
                expected_base,
            },
        )]),
    }
}

fn deck_name_overlay(
    intent: ChangeIntent,
    value: Option<&str>,
    expected_base: Option<ExpectedBase>,
) -> Overlay {
    Overlay {
        id: sid("overlay.patch.deck-name"),
        kind: OverlayKind::Patch,
        translations: None,
        deck_change: Some(DeckChange {
            name: Some(PropertyChange {
                intent,
                value: value.map(str::to_owned),
                expected_base,
            }),
            description: None,
            variables: BTreeMap::new(),
            adapter_ids: BTreeMap::new(),
        }),
        note_changes: BTreeMap::new(),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    }
}

fn deck_variable_overlay(
    intent: ChangeIntent,
    value: Option<&str>,
    expected_base: Option<ExpectedBase>,
) -> Overlay {
    Overlay {
        id: sid("overlay.patch.deck-variable"),
        kind: OverlayKind::Patch,
        translations: None,
        deck_change: Some(DeckChange {
            name: None,
            description: None,
            variables: BTreeMap::from([(
                "deck.locale".to_owned(),
                PropertyChange {
                    intent,
                    value: value.map(str::to_owned),
                    expected_base,
                },
            )]),
            adapter_ids: BTreeMap::new(),
        }),
        note_changes: BTreeMap::new(),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    }
}

fn already_existing_note_type_overlay() -> Overlay {
    let note_type = ug_style_deck().note_types[&sid("note-type.country")].clone();
    Overlay {
        id: sid("overlay.extension.duplicate-note-type"),
        kind: OverlayKind::Extension,
        translations: None,
        deck_change: None,
        note_changes: BTreeMap::new(),
        note_type_changes: BTreeMap::from([(
            sid("note-type.country"),
            NoteTypeChange {
                intent: ChangeIntent::Add,
                note_type: Some(note_type),
                name: None,
                variables: BTreeMap::new(),
                styling: None,
                fields: BTreeMap::new(),
                card_templates: BTreeMap::new(),
                adapter_ids: BTreeMap::new(),
                expected_base: None,
            },
        )]),
        media_changes: BTreeMap::new(),
    }
}

fn already_existing_note_overlay() -> Overlay {
    Overlay {
        id: sid("overlay.extension.duplicate-note"),
        kind: OverlayKind::Extension,
        translations: None,
        deck_change: None,
        note_changes: BTreeMap::from([(
            sid("note.finland"),
            NoteChange {
                intent: ChangeIntent::Add,
                note: Some(ug_style_deck().notes[&sid("note.finland")].clone()),
                variables: BTreeMap::new(),
                fields: BTreeMap::new(),
                tags: BTreeMap::new(),
                adapter_ids: BTreeMap::new(),
                expected_base: None,
            },
        )]),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    }
}

fn already_existing_card_template_overlay() -> Overlay {
    card_template_add_overlay(
        sid("template.country-to-capital"),
        CardTemplate {
            id: sid("template.country-to-capital"),
            name: "Duplicate".to_owned(),
            variables: BTreeMap::new(),
            question_format: "{{Country}}".to_owned(),
            answer_format: "{{Capital}}".to_owned(),
            adapter_ids: AdapterIds::new(),
        },
    )
}

fn already_existing_field_definition_overlay() -> Overlay {
    field_definition_add_overlay(
        sid("field.flag"),
        FieldDefinition {
            id: sid("field.flag"),
            name: "Flag duplicate".to_owned(),
        },
    )
}

fn already_existing_media_overlay() -> Overlay {
    Overlay {
        id: sid("overlay.extension.duplicate-media"),
        kind: OverlayKind::Extension,
        translations: None,
        deck_change: None,
        note_changes: BTreeMap::new(),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::from([(
            sid("media.flag.finland"),
            MediaChange {
                intent: ChangeIntent::Add,
                media: Some(MediaReference {
                    id: sid("media.flag.finland"),
                    path: "flags/fi.png".to_owned(),
                    sha256: "0123456789abcdef".to_owned(),
                }),
                expected_base: None,
            },
        )]),
    }
}

fn mismatched_note_type_payload_overlay() -> Overlay {
    let mut note_type = ug_style_deck().note_types[&sid("note-type.country")].clone();
    note_type.id = sid("note-type.other");
    Overlay {
        id: sid("overlay.extension.bad-note-type-payload"),
        kind: OverlayKind::Extension,
        translations: None,
        deck_change: None,
        note_changes: BTreeMap::new(),
        note_type_changes: BTreeMap::from([(
            sid("note-type.region"),
            NoteTypeChange {
                intent: ChangeIntent::Add,
                note_type: Some(note_type),
                name: None,
                variables: BTreeMap::new(),
                styling: None,
                fields: BTreeMap::new(),
                card_templates: BTreeMap::new(),
                adapter_ids: BTreeMap::new(),
                expected_base: None,
            },
        )]),
        media_changes: BTreeMap::new(),
    }
}

fn mismatched_note_payload_overlay() -> Overlay {
    Overlay {
        id: sid("overlay.extension.bad-note-payload"),
        kind: OverlayKind::Extension,
        translations: None,
        deck_change: None,
        note_changes: BTreeMap::from([(
            sid("note.denmark"),
            NoteChange {
                intent: ChangeIntent::Add,
                note: Some(sweden_note()),
                variables: BTreeMap::new(),
                fields: BTreeMap::new(),
                tags: BTreeMap::new(),
                adapter_ids: BTreeMap::new(),
                expected_base: None,
            },
        )]),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    }
}

fn mismatched_card_template_payload_overlay() -> Overlay {
    card_template_add_overlay(
        sid("template.country-to-flag"),
        CardTemplate {
            id: sid("template.other"),
            name: "Country - Flag".to_owned(),
            variables: BTreeMap::new(),
            question_format: "{{Country}}".to_owned(),
            answer_format: "{{Flag}}".to_owned(),
            adapter_ids: AdapterIds::new(),
        },
    )
}

fn mismatched_field_definition_payload_overlay() -> Overlay {
    field_definition_add_overlay(
        sid("field.population"),
        FieldDefinition {
            id: sid("field.other"),
            name: "Population".to_owned(),
        },
    )
}

fn mismatched_media_payload_overlay() -> Overlay {
    Overlay {
        id: sid("overlay.extension.bad-media-payload"),
        kind: OverlayKind::Extension,
        translations: None,
        deck_change: None,
        note_changes: BTreeMap::new(),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::from([(
            sid("media.flag.sweden"),
            MediaChange {
                intent: ChangeIntent::Add,
                media: Some(MediaReference {
                    id: sid("media.flag.other"),
                    path: "flags/se.png".to_owned(),
                    sha256: "abcdef".to_owned(),
                }),
                expected_base: None,
            },
        )]),
    }
}

fn card_template_add_overlay(template_id: StableId, template: CardTemplate) -> Overlay {
    Overlay {
        id: sid("overlay.extension.card-template-add"),
        kind: OverlayKind::Extension,
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
                    template_id,
                    CardTemplateChange {
                        intent: ChangeIntent::Add,
                        template: Some(template),
                        insert_after: None,
                        name: None,
                        variables: BTreeMap::new(),
                        question_format: None,
                        answer_format: None,
                        adapter_ids: BTreeMap::new(),
                        expected_base: None,
                    },
                )]),
                adapter_ids: BTreeMap::new(),
                expected_base: None,
            },
        )]),
        media_changes: BTreeMap::new(),
    }
}

fn field_definition_add_overlay(field_id: StableId, field: FieldDefinition) -> Overlay {
    Overlay {
        id: sid("overlay.extension.field-definition-add"),
        kind: OverlayKind::Extension,
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
                fields: BTreeMap::from([(
                    field_id,
                    FieldDefinitionChange {
                        intent: ChangeIntent::Add,
                        field: Some(field),
                        expected_base: None,
                    },
                )]),
                card_templates: BTreeMap::new(),
                adapter_ids: BTreeMap::new(),
                expected_base: None,
            },
        )]),
        media_changes: BTreeMap::new(),
    }
}

fn sid(value: &str) -> StableId {
    StableId::new(value).expect("test stable id is valid")
}
