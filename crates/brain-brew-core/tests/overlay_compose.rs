use std::collections::{BTreeMap, BTreeSet};

use brain_brew_core::{
    AdapterIdChange, AdapterIds, CanonicalDeck, CardTemplate, CardTemplateChange, ChangeIntent,
    ComposeErrorKind, DeckChange, ExpectedBase, FieldChange, FieldDefinition,
    FieldDefinitionChange, MediaChange, MediaReference, Note, NoteChange, NoteType, NoteTypeChange,
    Overlay, OverlayKind, PropertyChange, StableId, TagChange, TranslationChange,
    TranslationDictionary,
};

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
            .get(&sid("field.capital")),
        Some(&"Stockholm".to_owned())
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
                    ]),
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
            .get(&sid("field.capital")),
        Some(&"Helsingfors".to_owned())
    );
}

#[test]
fn ordered_overlay_stack_reports_conflicts_without_explicit_override() {
    let base = ug_style_deck();
    let first = overlay_replacing_capital(ChangeIntent::Merge, None, "Helsingfors");
    let second = overlay_replacing_capital(ChangeIntent::Merge, None, "Helsinki, Helsingfors");

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
    let first = overlay_replacing_capital(ChangeIntent::Merge, None, "Helsingfors");
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
            .get(&sid("field.capital")),
        Some(&"Helsinki / Helsingfors".to_owned())
    );
}

#[test]
fn translation_dictionary_reports_stale_entries() {
    let deck = ug_style_deck();
    let overlay = Overlay {
        id: sid("overlay.translation.da"),
        kind: OverlayKind::Translation,
        translations: Some(TranslationDictionary {
            changes: BTreeMap::from([(
                "Missing source".to_owned(),
                TranslationChange::Global("Mangler".to_owned()),
            )]),
            additions: BTreeMap::new(),
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
            .contains("translation source \"Missing source\" did not match")
    );
}

#[test]
fn translation_dictionary_can_scope_ambiguous_changes_by_path_and_add_blank_values() {
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
    base.notes.insert(sid("note.sweden"), sweden);
    let overlay = Overlay {
        id: sid("overlay.translation.da"),
        kind: OverlayKind::Translation,
        translations: Some(TranslationDictionary {
            changes: BTreeMap::from([(
                "Shared source".to_owned(),
                TranslationChange::AtPaths(BTreeMap::from([
                    (
                        "notes.note.finland.fields.field.country".to_owned(),
                        "Finsk kontekst".to_owned(),
                    ),
                    (
                        "notes.note.sweden.fields.field.country".to_owned(),
                        "Svensk kontekst".to_owned(),
                    ),
                ])),
            )]),
            additions: BTreeMap::from([(
                "notes.note.sweden.fields.field.capital".to_owned(),
                "Stockholm".to_owned(),
            )]),
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
}

#[test]
fn translation_dictionary_can_translate_tags() {
    let base = ug_style_deck();
    let overlay = Overlay {
        id: sid("overlay.translation.da"),
        kind: OverlayKind::Translation,
        translations: Some(TranslationDictionary {
            changes: BTreeMap::from([
                (
                    "Nordic".to_owned(),
                    TranslationChange::Global("UG::Nordic".to_owned()),
                ),
                (
                    "Europe".to_owned(),
                    TranslationChange::AtPaths(BTreeMap::from([(
                        "notes.note.finland.tags.Europe".to_owned(),
                        "UG::Europe".to_owned(),
                    )])),
                ),
            ]),
            additions: BTreeMap::new(),
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
fn translation_dictionary_addition_fails_when_base_is_not_blank() {
    let base = ug_style_deck();
    let overlay = Overlay {
        id: sid("overlay.translation.da"),
        kind: OverlayKind::Translation,
        translations: Some(TranslationDictionary {
            changes: BTreeMap::new(),
            additions: BTreeMap::from([(
                "notes.note.finland.fields.field.capital".to_owned(),
                "Helsinki translated".to_owned(),
            )]),
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
        .expect_err("addition expects blank base");

    assert!(report.has_kind(ComposeErrorKind::ExpectedBaseMismatch));
    assert!(report.errors.iter().any(|error| {
        error.path == "notes.note.finland.fields.field.capital"
            && error.message.contains("expected blank value")
    }));
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
                        value: Some("5.6 million".to_owned()),
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
            .get(&sid("field.population")),
        Some(&"5.6 million".to_owned())
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
            .get(&sid("field.region-code")),
        Some(&String::new())
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
                            expected_base: Some(ExpectedBase::EntityPresent),
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
                expected_base: Some(ExpectedBase::EntityPresent),
            },
        )]),
        media_changes: BTreeMap::new(),
    };

    let resolved = base.compose(&[overlay]).expect("remove composes");

    assert!(!resolved.note_types.contains_key(&sid("note-type.region")));
    assert!(resolved.tombstones.contains(&sid("note-type.region")));
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
                expected_base: Some(ExpectedBase::EntityPresent),
            },
        )]),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    };

    let resolved = base.compose(&[overlay]).expect("remove composes");

    assert!(resolved.notes.contains_key(&sid("note.finland")));
    assert!(resolved.tombstones.contains(&sid("note.finland")));
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
                        value: Some(value.to_owned()),
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
        ]),
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
        tombstones: BTreeSet::new(),
        adapter_ids: deck_adapter_ids,
    }
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
        ]),
        tags: BTreeSet::from(["Europe".to_owned(), "Nordic".to_owned()]),
        adapter_ids: AdapterIds::new(),
    }
}

fn sid(value: &str) -> StableId {
    StableId::new(value).expect("test stable id is valid")
}
