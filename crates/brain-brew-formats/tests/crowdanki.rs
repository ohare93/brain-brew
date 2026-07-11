use std::collections::{BTreeMap, BTreeSet};

use brain_brew_core::{
    AdapterIds, CanonicalDeck, CardTemplate, FieldDefinition, FieldImageReference, FieldValue,
    MediaReference, MessageComponent, Note, NoteType, StableId, StructuredMessage,
    TombstoneAddress, TombstoneRecord, Tombstones,
};
use brain_brew_formats::{canonical_yaml, crowdanki};

fn import_approved(
    input: &str,
) -> Result<brain_brew_core::CanonicalDeck, crowdanki::CrowdAnkiError> {
    let plan = crowdanki::plan_import(input.as_bytes())?;
    crowdanki::apply_import_plan(input.as_bytes(), &plan, true)
}

#[test]
fn exports_deterministic_crowdanki_json_preserving_adapter_identities() {
    let export = crowdanki::export_deck(&ug_style_deck()).expect("deck exports");

    let actual_json: serde_json::Value = serde_json::from_str(&export.deck_json).unwrap();
    let expected_json: serde_json::Value = serde_json::from_str(EXPECTED_CROWDANKI_JSON).unwrap();
    assert_eq!(actual_json, expected_json);
    assert_eq!(
        export.deck_json,
        crowdanki::export_deck(&ug_style_deck())
            .expect("second export succeeds")
            .deck_json
    );
    assert!(export.omitted_tombstones.is_empty());
}

#[test]
fn import_export_round_trip_is_semantically_equal_when_suggested_ids_match_source() {
    let original = ug_style_deck();
    let export = crowdanki::export_deck(&original).expect("deck exports");

    let imported = import_approved(&export.deck_json).expect("exported CrowdAnki imports");
    let expected = crowdanki::project_deck_for_crowdanki_round_trip(&original)
        .expect("source projects to the named CrowdAnki profile");
    let actual = crowdanki::project_deck_for_crowdanki_round_trip(&imported)
        .expect("import projects to the named CrowdAnki profile");
    let diff = expected.semantic_diff(&actual);

    assert!(
        diff.is_empty(),
        "{} mismatch: {diff:#?}",
        crowdanki::CROWDANKI_ROUND_TRIP_PROFILE.name
    );
}

#[test]
fn export_is_byte_identical_for_structured_image_and_equivalent_raw_img_field() {
    let mut raw = ug_style_deck();
    raw.notes
        .get_mut(&sid("note.finland"))
        .unwrap()
        .fields
        .insert(sid("field.flag"), "<img src=\"flags/fi.png\" />".to_owned());
    let mut structured = ug_style_deck();
    let note = structured.notes.get_mut(&sid("note.finland")).unwrap();
    note.fields.insert(
        sid("field.flag"),
        FieldValue::Images(vec![FieldImageReference {
            media_id: sid("media.flags-fi-png"),
        }]),
    );

    let raw_json = crowdanki::export_deck(&raw)
        .expect("raw image field exports")
        .deck_json;
    let structured_json = crowdanki::export_deck(&structured)
        .expect("structured image field exports")
        .deck_json;

    assert_eq!(structured_json, raw_json);
}

#[test]
fn crowdanki_export_resolves_message_to_structured_image_dependency() {
    let mut deck = ug_style_deck();
    let note = deck.notes.get_mut(&sid("note.finland")).unwrap();
    note.fields.insert(
        sid("field.flag"),
        FieldValue::Images(vec![FieldImageReference {
            media_id: sid("media.flags-fi-png"),
        }]),
    );
    note.fields.insert(
        sid("field.capital"),
        FieldValue::Message(StructuredMessage {
            components: vec![MessageComponent::FieldRef(
                "notes.note.finland.fields.field.flag".to_owned(),
            )],
            format: None,
            variables: BTreeMap::new(),
        }),
    );

    let export = crowdanki::export_deck(&deck).expect("mixed field graph exports");
    let json: serde_json::Value = serde_json::from_str(&export.deck_json).unwrap();
    assert_eq!(
        json["notes"][0]["fields"][1],
        "<img src=\"flags/fi.png\" />"
    );
}

#[test]
fn import_emits_strict_image_fields_as_structured_references() {
    let mut deck_json = expected_crowdanki_json_value();
    deck_json["media_files"] = serde_json::json!(["flags/fi.png", "maps/fi.png"]);
    deck_json["notes"][0]["fields"] = serde_json::json!([
        "Finland",
        "<img src=\"flags/fi.png\" />",
        "<img src=\"flags/fi.png\" />\t\n <img src=\"maps/fi.png\" />"
    ]);

    let imported = import_approved(&deck_json.to_string()).expect("strict image fields import");
    let note = imported.notes.get(&sid("note.finland")).unwrap();

    assert_eq!(
        note.fields[&sid("field.capital")].as_images().unwrap(),
        &[FieldImageReference {
            media_id: sid("media.flags-fi-png"),
        }]
    );
    assert_eq!(
        note.fields[&sid("field.flag")].as_images().unwrap(),
        &[
            FieldImageReference {
                media_id: sid("media.flags-fi-png"),
            },
            FieldImageReference {
                media_id: sid("media.maps-fi-png"),
            },
        ]
    );

    let yaml = canonical_yaml::to_string(&imported).expect("imported deck emits YAML");
    assert!(
        yaml.contains("      field.capital: !image media.flags-fi-png\n"),
        "{yaml}"
    );
    assert!(
        yaml.contains(
            "      field.flag:\n        - !image media.flags-fi-png\n        - !image media.maps-fi-png\n"
        ),
        "{yaml}"
    );
}

#[test]
fn import_decodes_safe_rendered_image_url_back_to_original_media_filename() {
    let mut deck_json = expected_crowdanki_json_value();
    let original = "images/旗 & quote\" #1?.svg";
    deck_json["media_files"] = serde_json::json!([original]);
    deck_json["notes"][0]["fields"] = serde_json::json!([
        "Finland",
        "Helsinki",
        r#"<img src="images/%E6%97%97%20%26%20quote%22%20%231%3F.svg" />"#
    ]);

    let imported =
        import_approved(&deck_json.to_string()).expect("encoded strict image path imports");
    let note = imported.notes.get(&sid("note.finland")).unwrap();
    assert!(note.fields[&sid("field.flag")].as_images().is_some());
    assert_eq!(imported.media.values().next().unwrap().path, original);
}

#[test]
fn import_and_export_reject_unsafe_media_filenames() {
    let mut deck_json = expected_crowdanki_json_value();
    deck_json["media_files"] = serde_json::json!(["line\nfeed.png"]);
    let import_error =
        import_approved(&deck_json.to_string()).expect_err("unsafe CrowdAnki filename must fail");
    assert!(import_error.to_string().contains("control character"));

    let mut deck = ug_style_deck();
    deck.media.values_mut().next().unwrap().path = "https:payload.png".to_owned();
    let export_error = crowdanki::export_deck(&deck).expect_err("unsafe deck filename must fail");
    assert!(export_error.to_string().contains("URL scheme delimiter"));
}

#[test]
fn import_keeps_non_strict_or_ambiguous_image_html_as_raw_fields() {
    let cases = [
        ("Extra Attribute", "<img src=\"x.png\" alt=\"y\" />"),
        ("Single Quoted Src", "<img src='x.png' />"),
        ("Non Self Closing", "<img src=\"x.png\">"),
        ("Surrounding Text", "before <img src=\"x.png\" />"),
        ("Missing Declaration", "<img src=\"missing.png\" />"),
    ];
    let mut deck_json = expected_crowdanki_json_value();
    deck_json["media_files"] = serde_json::json!(["x.png"]);
    deck_json["notes"] = serde_json::Value::Array(
        cases
            .iter()
            .enumerate()
            .map(|(index, (label, value))| {
                serde_json::json!({
                    "__type__": "Note",
                    "data": "",
                    "fields": [label, format!("Capital {index}"), value],
                    "flags": 0,
                    "guid": format!("negative-{index}"),
                    "note_model_uuid": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
                    "tags": []
                })
            })
            .collect(),
    );

    let imported = import_approved(&deck_json.to_string())
        .expect("non-strict image fields import as raw HTML");
    let yaml = canonical_yaml::to_string(&imported).expect("imported deck emits YAML");

    assert!(!yaml.contains("!image"), "{yaml}");
    for (label, expected_value) in cases {
        let note_id = sid(&format!("note.{}", slug_for_test(label)));
        let note = imported.notes.get(&note_id).unwrap();
        assert_eq!(note.fields.get(&sid("field.flag")).unwrap(), expected_value);
        assert!(note.fields[&sid("field.flag")].as_scalar().is_some());
    }
    assert!(
        yaml.contains("field.flag: '<img src=\"x.png\" alt=\"y\" />'"),
        "{yaml}"
    );
}

#[test]
fn structured_image_fields_survive_crowdanki_export_import_round_trip() {
    let mut original = ug_style_deck();
    original.adapter_ids.insert(
        "crowdanki:deck_config_uuid",
        "deck.ultimate-geography:deck-config",
    );
    original
        .adapter_ids
        .insert("crowdanki:deck_config_name", "Ultimate Geography");
    original.media.insert(
        sid("media.maps-fi-png"),
        MediaReference {
            id: sid("media.maps-fi-png"),
            path: "maps/fi.png".to_owned(),
            sha256: String::new(),
        },
    );
    let note = original.notes.get_mut(&sid("note.finland")).unwrap();
    note.fields.insert(
        sid("field.capital"),
        FieldValue::Images(vec![FieldImageReference {
            media_id: sid("media.flags-fi-png"),
        }]),
    );
    note.fields.insert(
        sid("field.flag"),
        FieldValue::Images(vec![
            FieldImageReference {
                media_id: sid("media.flags-fi-png"),
            },
            FieldImageReference {
                media_id: sid("media.maps-fi-png"),
            },
        ]),
    );

    let export = crowdanki::export_deck(&original).expect("structured deck exports");
    let imported = import_approved(&export.deck_json).expect("exported CrowdAnki re-imports");
    let expected = crowdanki::project_deck_for_crowdanki_round_trip(&original)
        .expect("structured source projects");
    let actual = crowdanki::project_deck_for_crowdanki_round_trip(&imported)
        .expect("imported source projects");
    let diff = expected.semantic_diff(&actual);

    assert!(
        diff.is_empty(),
        "{} mismatch: {diff:#?}",
        crowdanki::CROWDANKI_ROUND_TRIP_PROFILE.name
    );
}

#[test]
fn import_preserves_crowdanki_adapter_identities() {
    let imported = import_approved(EXPECTED_CROWDANKI_JSON).expect("CrowdAnki imports");

    assert_eq!(
        imported.adapter_ids.get("crowdanki:uuid"),
        Some("43c5ba66-9a65-11e8-90c9-a0481cc15658")
    );
    assert_eq!(
        imported.adapter_ids.get("crowdanki:deck_config_uuid"),
        Some("deck.ultimate-geography:deck-config")
    );
    assert_eq!(
        imported.adapter_ids.get("crowdanki:deck_config_name"),
        Some("Ultimate Geography")
    );
    assert_eq!(
        imported
            .note_types
            .get(&sid("note-type.country"))
            .unwrap()
            .adapter_ids
            .get("crowdanki:uuid"),
        Some("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa")
    );
    assert_eq!(
        imported
            .notes
            .get(&sid("note.finland"))
            .unwrap()
            .adapter_ids
            .get("crowdanki:guid"),
        Some("ug-finland-guid")
    );
}

#[test]
fn imported_note_id_suggestions_are_unicode_safe_normalized_and_order_independent() {
    let identities = vec![
        crowdanki::ImportedNoteIdentity {
            first_field: "Москва".to_owned(),
            source_guid: "guid-cyrillic".to_owned(),
        },
        crowdanki::ImportedNoteIdentity {
            first_field: "日本".to_owned(),
            source_guid: "guid-cjk".to_owned(),
        },
        crowdanki::ImportedNoteIdentity {
            first_field: "العَرَبِيَّة".to_owned(),
            source_guid: "guid-rtl".to_owned(),
        },
        crowdanki::ImportedNoteIdentity {
            first_field: "Finland".to_owned(),
            source_guid: "guid-finland-a".to_owned(),
        },
        crowdanki::ImportedNoteIdentity {
            first_field: "Finland".to_owned(),
            source_guid: "guid-finland-b".to_owned(),
        },
        crowdanki::ImportedNoteIdentity {
            first_field: "".to_owned(),
            source_guid: "guid-blank".to_owned(),
        },
    ];

    let suggestions = crowdanki::suggest_imported_note_stable_ids(&identities)
        .expect("unique CrowdAnki GUIDs produce suggestions");
    assert!(suggestions.iter().all(|id| id.as_str() != "note.unnamed"));
    assert!(suggestions[0].as_str().starts_with("note.imported-"));
    assert!(suggestions[1].as_str().starts_with("note.imported-"));
    assert!(suggestions[2].as_str().starts_with("note.imported-"));
    assert!(suggestions[3].as_str().starts_with("note.finland-"));
    assert!(suggestions[4].as_str().starts_with("note.finland-"));
    assert!(suggestions[5].as_str().starts_with("note.imported-"));
    assert_ne!(suggestions[3], suggestions[4]);

    let original_by_guid = identities
        .iter()
        .zip(&suggestions)
        .map(|(identity, id)| (identity.source_guid.clone(), id.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut reversed = identities.clone();
    reversed.reverse();
    let reversed_by_guid = reversed
        .iter()
        .zip(
            crowdanki::suggest_imported_note_stable_ids(&reversed)
                .expect("permuted identities produce suggestions"),
        )
        .map(|(identity, id)| (identity.source_guid.clone(), id))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(original_by_guid, reversed_by_guid);

    let composed =
        crowdanki::suggest_imported_note_stable_ids(&[crowdanki::ImportedNoteIdentity {
            first_field: "Café".to_owned(),
            source_guid: "guid-normalized".to_owned(),
        }])
        .unwrap();
    let decomposed =
        crowdanki::suggest_imported_note_stable_ids(&[crowdanki::ImportedNoteIdentity {
            first_field: "Cafe\u{301}".to_owned(),
            source_guid: "guid-normalized".to_owned(),
        }])
        .unwrap();
    assert_eq!(composed, decomposed, "suggestions use NFC normalization");

    let duplicate_guid = crowdanki::suggest_imported_note_stable_ids(&[
        crowdanki::ImportedNoteIdentity {
            first_field: "One".to_owned(),
            source_guid: "duplicate-guid".to_owned(),
        },
        crowdanki::ImportedNoteIdentity {
            first_field: "Two".to_owned(),
            source_guid: "duplicate-guid".to_owned(),
        },
    ])
    .expect_err("source GUIDs are adapter identities and must remain unique");
    assert!(duplicate_guid.to_string().contains("share guid"));
}

#[test]
fn importing_duplicate_guids_reports_every_affected_note_index() {
    let mut deck_json = expected_crowdanki_json_value();
    let duplicate = deck_json["notes"][0].clone();
    deck_json["notes"] = serde_json::json!([
        duplicate.clone(),
        {"__type__": "Note", "data": "", "fields": ["Distinct", "Capital", ""], "flags": 0, "guid": "distinct-guid", "note_model_uuid": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa", "tags": []},
        duplicate.clone(),
        duplicate
    ]);

    let message = import_error_message(&deck_json, "duplicate GUIDs fail closed");
    assert!(
        message.contains("CrowdAnki GUID \"ug-finland-guid\" is duplicated"),
        "{message}"
    );
    for index in [0, 2, 3] {
        assert!(
            message.contains(&format!("$.notes[{index}].guid")),
            "{message}"
        );
    }
}

#[test]
fn importing_guid_identity_is_exact_and_opaque() {
    let mut deck_json = expected_crowdanki_json_value();
    let mut spaced = deck_json["notes"][0].clone();
    spaced["guid"] = serde_json::json!(" ug-finland-guid ");
    spaced["fields"][0] = serde_json::json!("Spaced");
    let mut decomposed = deck_json["notes"][0].clone();
    decomposed["guid"] = serde_json::json!("ug-finland-gui\u{301}d");
    decomposed["fields"][0] = serde_json::json!("Unicode");
    deck_json["notes"] = serde_json::json!([deck_json["notes"][0].clone(), spaced, decomposed]);

    let imported =
        import_approved(&deck_json.to_string()).expect("opaque GUID lookalikes remain distinct");
    assert_eq!(imported.notes.len(), 3);
}

#[test]
fn importing_empty_guid_fails_closed_at_its_source_location() {
    let mut deck_json = expected_crowdanki_json_value();
    deck_json["notes"][0]["guid"] = serde_json::json!("");

    let message = import_error_message(&deck_json, "empty GUID fails closed");
    assert!(message.contains("$.notes[0].guid"), "{message}");
    assert!(message.contains("must not be empty"), "{message}");
}

#[test]
fn importing_notes_with_repeated_first_fields_disambiguates_and_preserves_guids() {
    let mut deck_json = expected_crowdanki_json_value();
    deck_json["notes"] = serde_json::json!([
        {
            "__type__": "Note", "data": "", "fields": ["Repeated", "One", ""],
            "flags": 0, "guid": "repeated-guid-a",
            "note_model_uuid": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa", "tags": []
        },
        {
            "__type__": "Note", "data": "", "fields": ["Repeated", "Two", ""],
            "flags": 0, "guid": "repeated-guid-b",
            "note_model_uuid": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa", "tags": []
        },
        {
            "__type__": "Note", "data": "", "fields": ["", "Blank", ""],
            "flags": 0, "guid": "blank-guid",
            "note_model_uuid": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa", "tags": []
        }
    ]);

    let imported = import_approved(&deck_json.to_string())
        .expect("repeated and blank first fields are disambiguated");
    assert_eq!(imported.notes.len(), 3);
    assert!(
        imported
            .notes
            .keys()
            .all(|id| id.as_str() != "note.unnamed")
    );
    assert_eq!(
        imported
            .notes
            .values()
            .map(|note| note.adapter_ids.get("crowdanki:guid").unwrap().to_owned())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "blank-guid".to_owned(),
            "repeated-guid-a".to_owned(),
            "repeated-guid-b".to_owned(),
        ])
    );
}

#[test]
fn importing_notes_with_colliding_suggested_stable_ids_disambiguates() {
    let mut deck_json = expected_crowdanki_json_value();
    deck_json["notes"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "__type__": "Note",
            "data": "",
            "fields": [
                "Finland!",
                "Helsinki duplicate",
                "<img src=\"fi-duplicate.png\">"
            ],
            "flags": 0,
            "guid": "ug-finland-duplicate-guid",
            "note_model_uuid": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "tags": ["duplicate"]
        }));

    let imported = import_approved(&deck_json.to_string())
        .expect("colliding readable note IDs are disambiguated");
    assert_eq!(imported.notes.len(), 2);
    assert!(
        imported
            .notes
            .keys()
            .all(|id| id.as_str().starts_with("note.finland-"))
    );
    assert_eq!(
        imported
            .notes
            .values()
            .map(|note| note.adapter_ids.get("crowdanki:guid").unwrap())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["ug-finland-guid", "ug-finland-duplicate-guid"])
    );
}

#[test]
fn importing_note_models_with_colliding_suggested_stable_ids_fails_closed() {
    let mut deck_json = expected_crowdanki_json_value();
    let mut duplicate_model = deck_json["note_models"][0].clone();
    duplicate_model["crowdanki_uuid"] = serde_json::json!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
    duplicate_model["name"] = serde_json::json!("Country!");
    deck_json["note_models"]
        .as_array_mut()
        .unwrap()
        .push(duplicate_model);

    let error = import_approved(&deck_json.to_string())
        .expect_err("colliding note model stable IDs fail closed");
    let message = error.to_string();

    assert!(message.contains("unresolved collision"), "{message}");
    assert!(message.contains("$.note_models[0]"), "{message}");
}

#[test]
fn importing_note_models_with_duplicate_crowdanki_uuid_fails_closed() {
    let mut deck_json = expected_crowdanki_json_value();
    let mut duplicate_model = deck_json["note_models"][0].clone();
    duplicate_model["name"] = serde_json::json!("Country Duplicate UUID");
    deck_json["note_models"]
        .as_array_mut()
        .unwrap()
        .push(duplicate_model);

    let error = import_approved(&deck_json.to_string())
        .expect_err("duplicate note model UUIDs fail closed");
    let message = error.to_string();

    assert!(message.contains("share crowdanki_uuid"), "{message}");
    assert!(
        message.contains("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
        "{message}"
    );
    assert!(message.contains("Country"), "{message}");
    assert!(message.contains("Country Duplicate UUID"), "{message}");
    assert!(
        message.contains("generate a CrowdAnki import plan"),
        "{message}"
    );
}

#[test]
fn importing_media_files_with_colliding_suggested_stable_ids_fails_closed() {
    let mut deck_json = expected_crowdanki_json_value();
    deck_json["media_files"] = serde_json::json!(["foo/bar.png", "foo_bar.png"]);

    let message = import_error_message(&deck_json, "colliding media stable IDs fail closed");

    assert!(message.contains("unresolved collision"), "{message}");
    assert!(message.contains("$.media_files[0]"), "{message}");
}

#[test]
fn importing_exact_duplicate_media_files_fails_closed() {
    let mut deck_json = expected_crowdanki_json_value();
    deck_json["media_files"] = serde_json::json!(["flags/fi.png", "flags/fi.png"]);

    let message = import_error_message(&deck_json, "duplicate physical media paths fail closed");
    assert!(
        message.contains("duplicate CrowdAnki media path"),
        "{message}"
    );
    assert!(message.contains("$.media_files[0]"), "{message}");
    assert!(message.contains("$.media_files[1]"), "{message}");
}

#[test]
fn importing_child_decks_fails_closed() {
    let mut deck_json = expected_crowdanki_json_value();
    deck_json["children"] = serde_json::json!([{ "name": "Child" }]);

    assert_import_error_contains(
        &deck_json,
        "child decks",
        &["child decks", "not modeled yet"],
    );
}

#[test]
fn importing_non_default_deck_scheduling_header_fails_closed() {
    let mut deck_json = expected_crowdanki_json_value();
    deck_json["dyn"] = serde_json::json!(1);
    deck_json["extendNew"] = serde_json::json!(11);
    deck_json["extendRev"] = serde_json::json!(51);

    assert_import_error_contains(
        &deck_json,
        "non-default deck scheduling header",
        &[
            "non-default deck scheduling header",
            "dyn=1",
            "extendNew=11",
            "extendRev=51",
        ],
    );
}

#[test]
fn importing_cloze_note_model_type_fails_closed() {
    let mut deck_json = expected_crowdanki_json_value();
    deck_json["note_models"][0]["type"] = serde_json::json!(1);

    assert_import_error_contains(
        &deck_json,
        "cloze note model type",
        &["only standard note models are supported", "type 1"],
    );
}

#[test]
fn importing_note_model_req_or_vers_fails_closed() {
    let mut deck_json = expected_crowdanki_json_value();
    deck_json["note_models"][0]["req"] = serde_json::json!([[0, "all", [0]]]);
    deck_json["note_models"][0]["vers"] = serde_json::json!([1]);

    assert_import_error_contains(
        &deck_json,
        "note model req/vers",
        &[
            "note model Country",
            "non-default CrowdAnki options",
            "not modeled yet",
        ],
    );
}

#[test]
fn importing_note_model_sort_field_fails_closed() {
    let mut deck_json = expected_crowdanki_json_value();
    deck_json["note_models"][0]["sortf"] = serde_json::json!(1);

    assert_import_error_contains(
        &deck_json,
        "note model sort field",
        &["note model Country", "non-default CrowdAnki options"],
    );
}

#[test]
fn importing_note_model_latex_svg_fails_closed() {
    let mut deck_json = expected_crowdanki_json_value();
    deck_json["note_models"][0]["latexsvg"] = serde_json::json!(true);

    assert_import_error_contains(
        &deck_json,
        "note model latexsvg",
        &["note model Country", "non-default CrowdAnki options"],
    );
}

#[test]
fn importing_note_model_tags_fails_closed() {
    let mut deck_json = expected_crowdanki_json_value();
    deck_json["note_models"][0]["tags"] = serde_json::json!(["model-tag"]);

    assert_import_error_contains(
        &deck_json,
        "note model tags",
        &["note model Country", "non-default CrowdAnki options"],
    );
}

#[test]
fn importing_field_font_fails_closed() {
    let mut deck_json = expected_crowdanki_json_value();
    deck_json["note_models"][0]["flds"][0]["font"] = serde_json::json!("Helvetica");

    assert_import_error_contains(
        &deck_json,
        "field font",
        &["field Country", "non-default CrowdAnki options"],
    );
}

#[test]
fn importing_field_size_fails_closed() {
    let mut deck_json = expected_crowdanki_json_value();
    deck_json["note_models"][0]["flds"][0]["size"] = serde_json::json!(21);

    assert_import_error_contains(
        &deck_json,
        "field size",
        &["field Country", "non-default CrowdAnki options"],
    );
}

#[test]
fn importing_field_rtl_fails_closed() {
    let mut deck_json = expected_crowdanki_json_value();
    deck_json["note_models"][0]["flds"][0]["rtl"] = serde_json::json!(true);

    assert_import_error_contains(
        &deck_json,
        "field rtl",
        &["field Country", "non-default CrowdAnki options"],
    );
}

#[test]
fn importing_field_sticky_fails_closed() {
    let mut deck_json = expected_crowdanki_json_value();
    deck_json["note_models"][0]["flds"][0]["sticky"] = serde_json::json!(true);

    assert_import_error_contains(
        &deck_json,
        "field sticky",
        &["field Country", "non-default CrowdAnki options"],
    );
}

#[test]
fn importing_field_media_fails_closed() {
    let mut deck_json = expected_crowdanki_json_value();
    deck_json["note_models"][0]["flds"][0]["media"] = serde_json::json!(["x.png"]);

    assert_import_error_contains(
        &deck_json,
        "field media",
        &["field Country", "non-default CrowdAnki options"],
    );
}

#[test]
fn importing_field_ord_fails_closed() {
    let mut deck_json = expected_crowdanki_json_value();
    deck_json["note_models"][0]["flds"][0]["ord"] = serde_json::json!(1);

    assert_import_error_contains(
        &deck_json,
        "field ord",
        &["field Country", "non-default CrowdAnki options"],
    );
}

#[test]
fn importing_note_data_fails_closed() {
    let mut deck_json = expected_crowdanki_json_value();
    deck_json["notes"][0]["data"] = serde_json::json!("opaque");

    assert_import_error_contains(
        &deck_json,
        "note data",
        &["note ug-finland-guid", "non-default data/flags"],
    );
}

#[test]
fn importing_note_flags_fails_closed() {
    let mut deck_json = expected_crowdanki_json_value();
    deck_json["notes"][0]["flags"] = serde_json::json!(1);

    assert_import_error_contains(
        &deck_json,
        "note flags",
        &["note ug-finland-guid", "non-default data/flags"],
    );
}

#[test]
fn importing_note_field_count_mismatch_fails_closed() {
    let mut deck_json = expected_crowdanki_json_value();
    deck_json["notes"][0]["fields"] = serde_json::json!(["Finland", "Helsinki"]);

    assert_import_error_contains(
        &deck_json,
        "note field-count mismatch",
        &[
            "note ug-finland-guid has 2 fields",
            "note type note-type.country has 3 fields",
        ],
    );
}

#[test]
fn importing_note_with_unknown_note_model_uuid_fails_closed() {
    let mut deck_json = expected_crowdanki_json_value();
    deck_json["notes"][0]["note_model_uuid"] = serde_json::json!("missing-model-uuid");

    assert_import_error_contains(
        &deck_json,
        "unknown note_model_uuid",
        &[
            "note references missing note_model_uuid",
            "missing-model-uuid",
        ],
    );
}

#[test]
fn importing_unknown_fields_fails_closed_with_json_error() {
    let mut deck_json = expected_crowdanki_json_value();
    deck_json["unexpected"] = serde_json::json!(true);

    let message = import_approved(&deck_json.to_string())
        .expect_err("unknown fields fail closed")
        .to_string();

    assert!(message.contains("CrowdAnki JSON error"), "{message}");
    assert!(message.contains("unknown field"), "{message}");
    assert!(message.contains("unexpected"), "{message}");
}

#[test]
fn importing_nested_json_schema_errors_report_the_machine_path() {
    let mut deck_json = expected_crowdanki_json_value();
    deck_json["notes"][0]["unexpected"] = serde_json::json!(true);

    let message = import_approved(&deck_json.to_string())
        .expect_err("unknown nested fields fail closed")
        .to_string();

    assert!(message.contains("schema path $.notes[0]"), "{message}");
    assert!(message.contains("unexpected"), "{message}");
}

#[test]
fn importing_non_default_deck_configurations_fails_closed() {
    let mut deck_json = expected_crowdanki_json_value();
    deck_json["deck_configurations"][0]["new"]["perDay"] = serde_json::json!(999);

    assert_import_error_contains(
        &deck_json,
        "non-default deck configurations",
        &["non-default deck configurations", "not modeled yet"],
    );
}

#[test]
fn importing_malformed_json_returns_crowdanki_error() {
    let message = import_approved("{ not json")
        .expect_err("malformed JSON fails cleanly")
        .to_string();

    assert!(message.contains("CrowdAnki JSON error"), "{message}");
}

#[test]
fn importing_template_ordinals_must_be_zero_based_contiguous_and_in_array_order() {
    let mut deck_json = expected_crowdanki_json_value();
    let template = deck_json["note_models"][0]["tmpls"][0].clone();
    deck_json["note_models"][0]["tmpls"] = serde_json::Value::Array(
        [99, 1, 2, 3]
            .into_iter()
            .enumerate()
            .map(|(index, ord)| {
                let mut template = template.clone();
                template["name"] = serde_json::json!(format!("Template {index}"));
                template["ord"] = serde_json::json!(ord);
                template
            })
            .collect(),
    );

    let message = import_error_message(&deck_json, "out-of-order template ordinals fail closed");
    assert!(
        message.contains("$.note_models[0].tmpls[0].ord"),
        "{message}"
    );
    assert!(message.contains("found 99, expected 0"), "{message}");
}

#[test]
fn importing_template_ordinal_duplicate_gap_negative_and_overflow_fail_closed() {
    for (label, ordinals, needle) in [
        ("duplicate", vec![0, 0], "duplicate template ordinal 0"),
        ("gap", vec![0, 2], "found 2, expected 1"),
        ("negative", vec![-1], "must be non-negative"),
    ] {
        let mut deck_json = expected_crowdanki_json_value();
        let template = deck_json["note_models"][0]["tmpls"][0].clone();
        deck_json["note_models"][0]["tmpls"] = serde_json::Value::Array(
            ordinals
                .into_iter()
                .enumerate()
                .map(|(index, ord)| {
                    let mut template = template.clone();
                    template["name"] = serde_json::json!(format!("{label} {index}"));
                    template["ord"] = serde_json::json!(ord);
                    template
                })
                .collect(),
        );
        let message = import_error_message(&deck_json, &format!("{label} ordinal fails closed"));
        assert!(message.contains("$.note_models[0].tmpls"), "{message}");
        assert!(message.contains(needle), "{message}");
    }

    let mut overflow = expected_crowdanki_json_value();
    overflow["note_models"][0]["tmpls"][0]["ord"] = serde_json::json!(u64::MAX);
    let message = import_error_message(&overflow, "overflow ordinal fails closed");
    assert!(
        message.contains("schema path $.note_models[0].tmpls[0].ord"),
        "{message}"
    );
}

#[test]
fn import_export_preserves_valid_template_array_order_and_ordinals() {
    let mut deck_json = expected_crowdanki_json_value();
    let mut second = deck_json["note_models"][0]["tmpls"][0].clone();
    second["name"] = serde_json::json!("Second");
    second["ord"] = serde_json::json!(1);
    deck_json["note_models"][0]["tmpls"] =
        serde_json::json!([deck_json["note_models"][0]["tmpls"][0].clone(), second,]);

    let imported =
        import_approved(&deck_json.to_string()).expect("valid template ordering imports");
    let exported: serde_json::Value = serde_json::from_str(
        &crowdanki::export_deck(&imported)
            .expect("valid template ordering exports")
            .deck_json,
    )
    .unwrap();
    assert_eq!(
        exported["note_models"][0]["tmpls"][0]["name"],
        "Country - Capital"
    );
    assert_eq!(exported["note_models"][0]["tmpls"][0]["ord"], 0);
    assert_eq!(exported["note_models"][0]["tmpls"][1]["name"], "Second");
    assert_eq!(exported["note_models"][0]["tmpls"][1]["ord"], 1);
}

#[test]
fn exporting_duplicate_or_empty_effective_guids_fails_closed() {
    let mut duplicate = ug_style_deck();
    let mut note = duplicate.notes[&sid("note.finland")].clone();
    note.id = sid("note.sweden");
    duplicate.notes.insert(note.id.clone(), note);
    let duplicate_error = crowdanki::export_deck(&duplicate)
        .expect_err("duplicate effective GUIDs must not export")
        .to_string();
    assert!(
        duplicate_error.contains("notes.note.finland"),
        "{duplicate_error}"
    );
    assert!(
        duplicate_error.contains("notes.note.sweden"),
        "{duplicate_error}"
    );
    let projection_error = crowdanki::project_deck_for_crowdanki_round_trip(&duplicate)
        .expect_err("duplicate effective GUIDs must not round-trip")
        .to_string();
    assert!(
        projection_error.contains("notes.note.finland"),
        "{projection_error}"
    );
    assert!(
        projection_error.contains("notes.note.sweden"),
        "{projection_error}"
    );

    let mut empty = ug_style_deck();
    empty
        .notes
        .get_mut(&sid("note.finland"))
        .unwrap()
        .adapter_ids
        .insert("crowdanki:guid", "");
    let empty_error = crowdanki::export_deck(&empty)
        .expect_err("empty explicit GUID must not export")
        .to_string();
    assert!(empty_error.contains("must not be empty"), "{empty_error}");
}

#[test]
fn importing_template_bafmt_fails_closed() {
    let mut deck_json = expected_crowdanki_json_value();
    deck_json["note_models"][0]["tmpls"][0]["bafmt"] = serde_json::json!("browser answer");

    assert_import_error_contains(
        &deck_json,
        "template bafmt",
        &[
            "card template Country - Capital",
            "non-default browser options",
        ],
    );
}

#[test]
fn importing_template_bqfmt_fails_closed() {
    let mut deck_json = expected_crowdanki_json_value();
    deck_json["note_models"][0]["tmpls"][0]["bqfmt"] = serde_json::json!("browser question");

    assert_import_error_contains(
        &deck_json,
        "template bqfmt",
        &[
            "card template Country - Capital",
            "non-default browser options",
        ],
    );
}

#[test]
fn importing_template_deck_override_did_fails_closed() {
    let mut deck_json = expected_crowdanki_json_value();
    deck_json["note_models"][0]["tmpls"][0]["did"] = serde_json::json!(12345);

    assert_import_error_contains(
        &deck_json,
        "template did",
        &[
            "card template Country - Capital",
            "non-default browser options",
        ],
    );
}

#[test]
fn crowdanki_parity_comparator_matches_reordered_media_files_as_multiset() {
    let expected: serde_json::Value = serde_json::json!({
        "media_files": ["a.png", "b.png", "a.png"],
        "name": "Deck"
    });
    let actual: serde_json::Value = serde_json::json!({
        "media_files": ["b.png", "a.png", "a.png"],
        "name": "Deck"
    });

    crowdanki::compare_deck_json_values(
        &expected,
        &actual,
        &crowdanki::CrowdAnkiParityOptions::default(),
    )
    .expect("media_files may reorder without a parity difference, with duplicate counts preserved");
}

#[test]
fn crowdanki_parity_comparator_accepts_exact_match() {
    let expected: serde_json::Value = serde_json::json!({
        "name": "Deck",
        "notes": [{"guid": "abc", "fields": ["A"]}]
    });
    let actual = expected.clone();

    crowdanki::compare_deck_json_values(
        &expected,
        &actual,
        &crowdanki::CrowdAnkiParityOptions::default(),
    )
    .expect("exact JSON matches");
}

#[test]
fn crowdanki_parity_comparator_reports_json_paths() {
    let expected: serde_json::Value = serde_json::json!({
        "desc": "Expected description",
        "name": "Deck",
        "notes": [{"guid": "abc", "fields": ["A"]}]
    });
    let actual: serde_json::Value = serde_json::json!({
        "name": "Deck",
        "notes": [{"guid": "abc", "fields": ["B"]}],
        "deck_config_uuid": "legacy-default"
    });

    let report = crowdanki::compare_deck_json_values(
        &expected,
        &actual,
        &crowdanki::CrowdAnkiParityOptions::default(),
    )
    .expect_err("differences are reported");

    assert!(
        report
            .differences
            .iter()
            .any(|difference| difference.path == "$.deck_config_uuid")
    );
    assert!(
        report
            .differences
            .iter()
            .any(|difference| difference.path == "$.desc")
    );
    assert!(
        report
            .differences
            .iter()
            .any(|difference| difference.path == "$.notes[guid=\"abc\"].fields[0]")
    );
}

#[test]
fn crowdanki_parity_comparator_accepts_allowlisted_paths() {
    let expected: serde_json::Value = serde_json::json!({
        "name": "Deck",
        "notes": [{"guid": "abc", "fields": ["A"]}]
    });
    let actual: serde_json::Value = serde_json::json!({
        "name": "Deck",
        "notes": [{"guid": "abc", "fields": ["A"], "flags": 0}],
        "deck_config_uuid": "legacy-default"
    });
    let options = crowdanki::CrowdAnkiParityOptions {
        allowed_path_globs: BTreeSet::from([
            "$.deck_config_uuid".to_owned(),
            "$.notes[*].flags".to_owned(),
        ]),
    };

    crowdanki::compare_deck_json_values(&expected, &actual, &options)
        .expect("allowlisted JSON paths may differ");
}

#[test]
fn crowdanki_parity_comparator_rejects_reordered_template_ordinals() {
    let expected: serde_json::Value = serde_json::json!({
        "notes": [
            {"guid": "a", "fields": ["A"]},
            {"guid": "b", "fields": ["B"]}
        ],
        "note_models": [{
            "crowdanki_uuid": "model-1",
            "flds": [{"name": "Country"}, {"name": "Capital"}],
            "tmpls": [{"ord": 0, "name": "A"}, {"ord": 1, "name": "B"}]
        }]
    });
    let actual: serde_json::Value = serde_json::json!({
        "note_models": [{
            "crowdanki_uuid": "model-1",
            "tmpls": [{"ord": 1, "name": "B"}, {"ord": 0, "name": "A"}],
            "flds": [{"name": "Capital"}, {"name": "Country"}]
        }],
        "notes": [
            {"guid": "b", "fields": ["B"]},
            {"guid": "a", "fields": ["A"]}
        ]
    });

    let report = crowdanki::compare_deck_json_values(
        &expected,
        &actual,
        &crowdanki::CrowdAnkiParityOptions::default(),
    )
    .expect_err("template ordinal array order is identity and must not be normalized");
    assert!(
        report
            .differences
            .iter()
            .any(|difference| difference.path.ends_with(".tmpls[0].ord"))
    );
}

#[test]
fn crowdanki_parity_report_summarizes_repeated_defaults_and_serializes_to_json() {
    let expected: serde_json::Value = serde_json::json!({
        "notes": [
            {"guid": "a", "fields": ["A"]},
            {"guid": "b", "fields": ["B"]},
            {"guid": "c", "fields": ["C"]}
        ]
    });
    let actual: serde_json::Value = serde_json::json!({
        "notes": [
            {"guid": "a", "fields": ["A"], "flags": 0},
            {"guid": "b", "fields": ["B"], "flags": 0},
            {"guid": "c", "fields": ["C"], "flags": 0}
        ]
    });

    let report = crowdanki::compare_deck_json_values(
        &expected,
        &actual,
        &crowdanki::CrowdAnkiParityOptions::default(),
    )
    .expect_err("extra defaults are reported");
    let human = report.to_string();
    assert!(human.contains("Repeated differences"));
    assert!(human.contains("3 × $.notes[*].flags"));

    let json = serde_json::to_value(&report).expect("report serializes");
    assert_eq!(json["differences"][0]["kind"], "extra_actual");
    assert_eq!(json["differences"].as_array().unwrap().len(), 3);
}

#[test]
fn export_omits_tombstoned_notes_and_reports_their_stable_ids() {
    let mut deck = ug_style_deck();
    deck.tombstones
        .insert(TombstoneRecord::legacy(TombstoneAddress::Note {
            note_id: sid("note.finland"),
        }));

    let export = crowdanki::export_deck(&deck).expect("deck exports");

    assert_eq!(
        export.omitted_tombstones,
        vec![TombstoneAddress::Note {
            note_id: sid("note.finland"),
        }]
    );
    assert!(!export.deck_json.contains("ug-finland-guid"));
}

const EXPECTED_CROWDANKI_JSON: &str = r#"{
  "__type__": "Deck",
  "children": [],
  "crowdanki_uuid": "43c5ba66-9a65-11e8-90c9-a0481cc15658",
  "deck_config_uuid": "deck.ultimate-geography:deck-config",
  "deck_configurations": [
    {
      "__type__": "DeckConfig",
      "autoplay": false,
      "crowdanki_uuid": "deck.ultimate-geography:deck-config",
      "dyn": false,
      "lapse": {
        "delays": [
          10
        ],
        "leechAction": 0,
        "leechFails": 8,
        "minInt": 1,
        "mult": 0
      },
      "maxTaken": 60,
      "name": "Ultimate Geography",
      "new": {
        "bury": true,
        "delays": [
          1,
          10
        ],
        "initialFactor": 2500,
        "ints": [
          1,
          4,
          7
        ],
        "order": 0,
        "perDay": 15,
        "separate": true
      },
      "replayq": true,
      "rev": {
        "bury": true,
        "ease4": 1.3,
        "fuzz": 0.05,
        "ivlFct": 1,
        "maxIvl": 36500,
        "minSpace": 1,
        "perDay": 100
      },
      "timer": 0
    }
  ],
  "desc": "A geography deck fixture.",
  "dyn": 0,
  "extendNew": 10,
  "extendRev": 50,
  "media_files": [
    "flags/fi.png"
  ],
  "name": "Ultimate Geography",
  "note_models": [
    {
      "__type__": "NoteModel",
      "crowdanki_uuid": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
      "css": ".card { font-family: sans-serif; }\n",
      "flds": [
        {
          "font": "Arial",
          "media": [],
          "name": "Country",
          "ord": 0,
          "rtl": false,
          "size": 20,
          "sticky": false
        },
        {
          "font": "Arial",
          "media": [],
          "name": "Capital",
          "ord": 1,
          "rtl": false,
          "size": 20,
          "sticky": false
        },
        {
          "font": "Arial",
          "media": [],
          "name": "Flag",
          "ord": 2,
          "rtl": false,
          "size": 20,
          "sticky": false
        }
      ],
      "latexPost": "\\end{document}",
      "latexPre": "\\documentclass[12pt]{article}\n\\special{papersize=3in,5in}\n\\usepackage{amssymb,amsmath}\n\\pagestyle{empty}\n\\setlength{\\parindent}{0in}\n\\begin{document}\n",
      "latexsvg": false,
      "name": "Country",
      "req": [],
      "sortf": 0,
      "tags": [],
      "tmpls": [
        {
          "afmt": "{{FrontSide}}<hr id=answer>{{Capital}}",
          "bafmt": "",
          "bfont": "",
          "bqfmt": "",
          "bsize": 0,
          "did": null,
          "name": "Country - Capital",
          "ord": 0,
          "qfmt": "{{Country}}",
          "scratchPad": 0
        }
      ],
      "type": 0,
      "vers": []
    }
  ],
  "notes": [
    {
      "__type__": "Note",
      "data": "",
      "fields": [
        "Finland",
        "Helsinki",
        "<img src=\"fi.png\">"
      ],
      "flags": 0,
      "guid": "ug-finland-guid",
      "note_model_uuid": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
      "tags": [
        "Europe",
        "Nordic"
      ]
    }
  ]
}
"#;

fn ug_style_deck() -> CanonicalDeck {
    let mut deck_adapter_ids = AdapterIds::new();
    deck_adapter_ids.insert("crowdanki:uuid", "43c5ba66-9a65-11e8-90c9-a0481cc15658");

    let mut note_type_adapter_ids = AdapterIds::new();
    note_type_adapter_ids.insert("crowdanki:uuid", "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");

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
                id: sid("field.capital"),
                name: "Capital".to_owned(),
            },
            FieldDefinition {
                id: sid("field.flag"),
                name: "Flag".to_owned(),
            },
        ],
        card_templates: vec![CardTemplate {
            id: sid("template.country-capital"),
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
            sid("media.flags-fi-png"),
            MediaReference {
                id: sid("media.flags-fi-png"),
                path: "flags/fi.png".to_owned(),
                sha256: String::new(),
            },
        )]),
        tombstones: Tombstones::default(),
        adapter_ids: deck_adapter_ids,
    }
}

fn expected_crowdanki_json_value() -> serde_json::Value {
    serde_json::from_str(EXPECTED_CROWDANKI_JSON).expect("fixture JSON is valid")
}

fn import_error_message(deck_json: &serde_json::Value, label: &str) -> String {
    import_approved(&deck_json.to_string())
        .expect_err(label)
        .to_string()
}

fn assert_import_error_contains(deck_json: &serde_json::Value, label: &str, needles: &[&str]) {
    let message = import_error_message(deck_json, label);
    for needle in needles {
        assert!(message.contains(needle), "expected {needle:?} in {message}");
    }
}

fn sid(value: &str) -> StableId {
    StableId::new(value).expect("test stable id is valid")
}

fn slug_for_test(source: &str) -> String {
    source
        .chars()
        .flat_map(char::to_lowercase)
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
