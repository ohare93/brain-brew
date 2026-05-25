use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use brain_brew_formats::core::{
    AdapterIdChange, AdapterIds, CanonicalDeck, CardTemplateChange, ChangeIntent, DeckChange,
    ExpectedBase, FieldChange, FieldDefinition, FieldDefinitionChange, MediaChange, MediaReference,
    Note, NoteChange, NoteTypeChange, Overlay, OverlayKind, PropertyChange, StableId, TagChange,
    TranslationChange,
};
use brain_brew_formats::{canonical_yaml, crowdanki, manifest, media};

#[test]
fn ultimate_geography_fixture_manifest_composes_all_targets() {
    let root = fixture_root();
    let manifest = read_manifest(&root);
    let base_path = root.join(&manifest.base);
    let base_source = fs::read_to_string(&base_path).unwrap();
    assert_eq!(
        canonical_yaml::format_str(&base_source).unwrap(),
        base_source,
        "{} is not canonicalized",
        base_path.display()
    );

    let package = manifest
        .package
        .as_ref()
        .expect("fixture has package metadata");
    assert_eq!(package.id, "anki-geo.ultimate-geography");
    assert_eq!(package.version, "0.1.0");

    assert_eq!(manifest.targets.len(), 71);
    for target in manifest.targets.keys() {
        assert!(
            manifest.targets[target].exports.is_empty(),
            "{target} relies on the manifest-target default CrowdAnki output path"
        );

        let deck = compose_target(&root, &manifest, target);
        deck.validate()
            .unwrap_or_else(|error| panic!("{target} validates: {error}"));
        media::validate_references(&deck)
            .unwrap_or_else(|error| panic!("{target} media references validate: {error}"));
    }
}

#[test]
fn ultimate_geography_hardcore_extension_builds_on_main_deck_without_erasing_base_content() {
    let root = fixture_root();
    let manifest = read_manifest(&root);

    let english = compose_target(&root, &manifest, "en-hardcore-standard");
    assert_eq!(english.notes.len(), 336);
    assert!(english.notes.contains_key(&sid("note.pitcairn-islands")));
    assert_eq!(
        english.notes[&sid("note.anguilla")].fields[&sid("field.capital")],
        "The Valley"
    );
    assert_eq!(
        english.notes[&sid("note.anguilla")].fields[&sid("field.map")],
        "<img src=\"ug-map-anguilla.png\" />",
        "Hardcore fills extra fields without blanking the main deck's existing map card"
    );
    assert!(
        english.notes[&sid("note.anguilla")]
            .tags
            .contains("UG::Overlapping")
    );

    let german = compose_target(&root, &manifest, "de-hardcore-standard");
    assert_eq!(
        german.notes[&sid("note.pitcairn-islands")].fields[&sid("field.country")],
        "Pitcairninseln"
    );
    assert_eq!(
        german.notes[&sid("note.canary-islands")].fields[&sid("field.capital")],
        "Santa Cruz de Tenerife, Las Palmas de Gran Canaria"
    );

    let extended = compose_target(&root, &manifest, "de-hardcore-extended");
    assert_eq!(
        extended.note_types[&sid("note-type.ultimate-geography")]
            .card_templates
            .len(),
        6,
        "Hardcore composes with the shared Extended variant"
    );
}

#[test]
fn ultimate_geography_translation_overlays_use_dictionaries_not_template_copies() {
    let root = fixture_root();
    let manifest = read_manifest(&root);
    for overlay_ref in manifest
        .overlays
        .values()
        .filter(|overlay| overlay.kind.as_deref() == Some("translation"))
    {
        let overlay = canonical_yaml::overlay_from_str(
            &fs::read_to_string(root.join(&overlay_ref.file)).unwrap(),
        )
        .unwrap_or_else(|error| panic!("{} parses: {error}", overlay_ref.file));
        let translations = overlay
            .translations
            .as_ref()
            .unwrap_or_else(|| panic!("{} uses translation dictionary", overlay_ref.file));
        assert!(
            !translations.changes.is_empty()
                || !translations.additions.is_empty()
                || !translations.variables.is_empty()
                || !translations.adapter_ids.is_empty(),
            "{} has translation dictionary entries",
            overlay_ref.file
        );
        assert!(
            overlay.note_changes.is_empty(),
            "{} uses dictionary changes/additions instead of per-note field replacements",
            overlay_ref.file
        );
        assert!(
            translations.changes.values().all(|change| match change {
                TranslationChange::Global(_) => true,
                TranslationChange::AtPaths(paths) => paths
                    .keys()
                    .all(|path| path != "note_types.note-type.ultimate-geography.name"),
            }),
            "{} translates note type names through variables instead of path-scoped metadata changes",
            overlay_ref.file
        );
        assert!(
            overlay
                .note_type_changes
                .values()
                .all(|change| change.name.is_none()),
            "{} translates note type names through variables instead of path-scoped metadata changes",
            overlay_ref.file
        );
        assert!(
            overlay
                .note_type_changes
                .values()
                .all(|change| change.card_templates.is_empty()),
            "{} does not copy standard card template HTML",
            overlay_ref.file
        );
        if overlay_ref
            .file
            .starts_with("overlays/extensions/hardcore/translations/")
        {
            assert!(
                !overlay_ref.file.ends_with("/en.yaml"),
                "English Hardcore field content is not a translation overlay"
            );
            assert!(
                translations.additions.is_empty(),
                "{} uses field_fills for extension-owned blank field content instead of translations.additions",
                overlay_ref.file
            );
        }
    }

    for overlay_ref in manifest.overlays.values().filter(|overlay| {
        overlay
            .file
            .starts_with("overlays/extensions/hardcore/field-fills/")
    }) {
        assert_eq!(
            overlay_ref.kind.as_deref(),
            Some("extension"),
            "{} is extension content, not translation content",
            overlay_ref.file
        );
        let overlay = canonical_yaml::overlay_from_str(
            &fs::read_to_string(root.join(&overlay_ref.file)).unwrap(),
        )
        .unwrap_or_else(|error| panic!("{} parses: {error}", overlay_ref.file));
        assert_eq!(overlay.kind, OverlayKind::Extension);
        assert!(
            overlay.translations.is_none(),
            "{} has field_fills rather than a translation dictionary",
            overlay_ref.file
        );
        assert!(
            !overlay.note_changes.is_empty(),
            "{} lowers field_fills into checked note field changes",
            overlay_ref.file
        );
    }

    for overlay_ref in manifest
        .overlays
        .values()
        .filter(|overlay| overlay.file.starts_with("overlays/variants/extended/"))
    {
        let overlay = canonical_yaml::overlay_from_str(
            &fs::read_to_string(root.join(&overlay_ref.file)).unwrap(),
        )
        .unwrap_or_else(|error| panic!("{} parses: {error}", overlay_ref.file));
        assert!(
            overlay
                .note_type_changes
                .values()
                .all(|change| change.card_templates.is_empty()),
            "{} carries only language-specific extended metadata; shared card templates live in overlays/variants/extended.yaml",
            overlay_ref.file
        );
    }
}

#[test]
fn ultimate_geography_fixture_exports_match_release_oracle_semantics_when_available() {
    let oracle_root = ultimate_geography_release_oracle_root();
    if !oracle_root
        .join("Ultimate Geography [EN]/deck.json")
        .exists()
    {
        eprintln!(
            "skipping Ultimate Geography release parity check; {} is missing. Run `scripts/fetch_ug_release_oracle.py --tag v5.3` or set BRAINBREW_UG_CROWDANKI_ORACLE to a CrowdAnki oracle root.",
            oracle_root.display()
        );
        return;
    }

    let root = fixture_root();
    let manifest = read_manifest(&root);
    for target in manifest
        .targets
        .keys()
        .filter(|target| matches!(target_parts(target).1, "standard" | "extended"))
    {
        let deck = compose_target(&root, &manifest, target);
        let export = crowdanki::export_deck(&deck)
            .unwrap_or_else(|error| panic!("{target} exports to CrowdAnki: {error}"));
        let new: serde_json::Value = serde_json::from_str(&export.deck_json).unwrap();
        let old: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(release_oracle_deck_json_path(&oracle_root, target)).unwrap(),
        )
        .unwrap();

        assert_crowdanki_semantic_subset_eq(&old, &new, target);
    }
}

#[test]
fn ug_regression_deck_metadata_changes_flow_to_crowdanki_for_every_target() {
    assert_all_targets_export_exact_diffs("deck name", |target, deck, _json| {
        let new_name = format!("Regression Deck {target}");
        let mut deck_change = empty_deck_change();
        deck_change.name = Some(replace_property(new_name.clone(), deck.name.clone()));
        let mut overlay = empty_overlay(&format!("overlay.regression.deck-name.{target}"));
        overlay.deck_change = Some(deck_change);
        MutationExpectation::new(overlay, vec![expected_json("/name", new_name)])
    });

    assert_all_targets_export_exact_diffs("deck description", |target, deck, _json| {
        let new_description = format!("Regression description for {target}");
        let mut deck_change = empty_deck_change();
        deck_change.description = Some(replace_property(
            new_description.clone(),
            deck.description.clone(),
        ));
        let mut overlay = empty_overlay(&format!("overlay.regression.deck-description.{target}"));
        overlay.deck_change = Some(deck_change);
        MutationExpectation::new(overlay, vec![expected_json("/desc", new_description)])
    });

    assert_all_targets_export_exact_diffs("deck uuid", |target, deck, _json| {
        let current_uuid = deck
            .adapter_ids
            .get("crowdanki:uuid")
            .expect("UG fixture deck has CrowdAnki UUID")
            .to_owned();
        let new_uuid = "11111111-1111-1111-1111-111111111111".to_owned();
        let mut deck_change = empty_deck_change();
        deck_change.adapter_ids.insert(
            "crowdanki:uuid".to_owned(),
            replace_adapter_id(new_uuid.clone(), current_uuid),
        );
        let mut overlay = empty_overlay(&format!("overlay.regression.deck-uuid.{target}"));
        overlay.deck_change = Some(deck_change);
        MutationExpectation::new(overlay, vec![expected_json("/crowdanki_uuid", new_uuid)])
    });

    assert_all_targets_export_exact_diffs("deck config name", |target, deck, _json| {
        let current_name = deck
            .adapter_ids
            .get("crowdanki:deck_config_name")
            .expect("UG fixture deck has CrowdAnki deck config name")
            .to_owned();
        let new_name = format!("Regression Deck Config {target}");
        let mut deck_change = empty_deck_change();
        deck_change.adapter_ids.insert(
            "crowdanki:deck_config_name".to_owned(),
            replace_adapter_id(new_name.clone(), current_name),
        );
        let mut overlay = empty_overlay(&format!("overlay.regression.deck-config-name.{target}"));
        overlay.deck_change = Some(deck_change);
        MutationExpectation::new(
            overlay,
            vec![expected_json("/deck_configurations/0/name", new_name)],
        )
    });

    assert_all_targets_export_exact_diffs("deck config uuid", |target, deck, _json| {
        let current_uuid = deck
            .adapter_ids
            .get("crowdanki:deck_config_uuid")
            .expect("UG fixture deck has CrowdAnki deck config UUID")
            .to_owned();
        let new_uuid = "33333333-3333-3333-3333-333333333333".to_owned();
        let mut deck_change = empty_deck_change();
        deck_change.adapter_ids.insert(
            "crowdanki:deck_config_uuid".to_owned(),
            replace_adapter_id(new_uuid.clone(), current_uuid),
        );
        let mut overlay = empty_overlay(&format!("overlay.regression.deck-config-uuid.{target}"));
        overlay.deck_change = Some(deck_change);
        MutationExpectation::new(
            overlay,
            vec![
                expected_json("/deck_config_uuid", new_uuid.clone()),
                expected_json("/deck_configurations/0/crowdanki_uuid", new_uuid),
            ],
        )
    });
}

#[test]
fn ug_regression_note_type_changes_flow_to_crowdanki_for_every_target() {
    assert_all_targets_export_exact_diffs("note type name", |target, deck, _json| {
        let note_type = ug_note_type(deck);
        let new_name = format!("Regression Note Type {target}");
        let mut note_type_change = empty_note_type_change();
        note_type_change.name = Some(replace_property(new_name.clone(), note_type.name.clone()));
        let mut overlay = empty_overlay(&format!("overlay.regression.note-type-name.{target}"));
        overlay
            .note_type_changes
            .insert(note_type.id.clone(), note_type_change);
        MutationExpectation::new(
            overlay,
            vec![expected_json("/note_models/0/name", new_name)],
        )
    });

    assert_all_targets_export_exact_diffs(
        "note type variable rendered name",
        |target, deck, _json| {
            let note_type = ug_note_type(deck);
            let old_source_name = note_type
                .variables
                .get("note-type.name")
                .expect("UG note type has source name variable")
                .clone();
            let suffix = note_type
                .variables
                .get("variant.name-suffix")
                .map(String::as_str)
                .unwrap_or_default();
            let new_source_name = format!("Regression Variable Name {target}");
            let mut note_type_change = empty_note_type_change();
            note_type_change.variables.insert(
                "note-type.name".to_owned(),
                replace_property(new_source_name.clone(), old_source_name),
            );
            let mut overlay =
                empty_overlay(&format!("overlay.regression.note-type-variable.{target}"));
            overlay
                .note_type_changes
                .insert(note_type.id.clone(), note_type_change);
            MutationExpectation::new(
                overlay,
                vec![expected_json(
                    "/note_models/0/name",
                    format!("{new_source_name}{suffix}"),
                )],
            )
        },
    );

    assert_all_targets_export_exact_diffs(
        "note type variable rendered templates",
        |target, deck, baseline_json| {
            let note_type = ug_note_type(deck);
            let old_label = note_type
                .variables
                .get("label.capital")
                .expect("UG note type has capital label variable")
                .clone();
            let new_label = format!("Regression Capital Label {target}");
            let mut note_type_change = empty_note_type_change();
            note_type_change.variables.insert(
                "label.capital".to_owned(),
                replace_property(new_label.clone(), old_label.clone()),
            );
            let mut overlay = empty_overlay(&format!(
                "overlay.regression.note-type-template-variable.{target}"
            ));
            overlay
                .note_type_changes
                .insert(note_type.id.clone(), note_type_change);

            let mut expected = Vec::new();
            for (template_index, template) in baseline_json["note_models"][0]["tmpls"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
            {
                for property in ["qfmt", "afmt"] {
                    let current = template[property].as_str().unwrap();
                    let old_fragment = format!("<div class=\"type\">{old_label}</div>");
                    if current.contains(&old_fragment) {
                        let new_fragment = format!("<div class=\"type\">{new_label}</div>");
                        expected.push(expected_json(
                            &format!("/note_models/0/tmpls/{template_index}/{property}"),
                            current.replace(&old_fragment, &new_fragment),
                        ));
                    }
                }
            }
            MutationExpectation::new(overlay, expected)
        },
    );

    assert_all_targets_export_exact_diffs("note type styling", |target, deck, _json| {
        let note_type = ug_note_type(deck);
        let new_css = format!(".card {{ color: #123456; }} /* {target} */");
        let mut note_type_change = empty_note_type_change();
        note_type_change.styling =
            Some(replace_property(new_css.clone(), note_type.styling.clone()));
        let mut overlay = empty_overlay(&format!("overlay.regression.note-type-styling.{target}"));
        overlay
            .note_type_changes
            .insert(note_type.id.clone(), note_type_change);
        MutationExpectation::new(overlay, vec![expected_json("/note_models/0/css", new_css)])
    });

    assert_all_targets_export_exact_diffs("note type uuid", |target, deck, baseline_json| {
        let note_type = ug_note_type(deck);
        let current_uuid = note_type
            .adapter_ids
            .get("crowdanki:uuid")
            .expect("UG note type has CrowdAnki UUID")
            .to_owned();
        let new_uuid = "22222222-2222-2222-2222-222222222222".to_owned();
        let mut note_type_change = empty_note_type_change();
        note_type_change.adapter_ids.insert(
            "crowdanki:uuid".to_owned(),
            replace_adapter_id(new_uuid.clone(), current_uuid),
        );
        let mut overlay = empty_overlay(&format!("overlay.regression.note-type-uuid.{target}"));
        overlay
            .note_type_changes
            .insert(note_type.id.clone(), note_type_change);

        let mut expected = vec![expected_json(
            "/note_models/0/crowdanki_uuid",
            new_uuid.clone(),
        )];
        for note_index in 0..baseline_json["notes"].as_array().unwrap().len() {
            expected.push(expected_json(
                &format!("/notes/{note_index}/note_model_uuid"),
                new_uuid.clone(),
            ));
        }
        MutationExpectation::new(overlay, expected)
    });
}

#[test]
fn ug_regression_field_definition_changes_flow_to_crowdanki_for_every_target() {
    assert_all_targets_export_exact_diffs("field definition name", |target, deck, _json| {
        let note_type = ug_note_type(deck);
        let field_id = sid("field.capital");
        assert!(
            note_type.fields.iter().any(|field| field.id == field_id),
            "UG note type has capital field"
        );
        let new_name = format!("Regression Capital Field {target}");
        let mut note_type_change = empty_note_type_change();
        note_type_change.fields.insert(
            field_id.clone(),
            FieldDefinitionChange {
                intent: ChangeIntent::Override,
                field: Some(FieldDefinition {
                    id: field_id,
                    name: new_name.clone(),
                }),
                expected_base: Some(ExpectedBase::EntityPresent),
            },
        );
        let mut overlay = empty_overlay(&format!("overlay.regression.field-name.{target}"));
        overlay
            .note_type_changes
            .insert(note_type.id.clone(), note_type_change);
        MutationExpectation::new(
            overlay,
            vec![expected_json(
                &format!(
                    "/note_models/0/flds/{}/name",
                    field_index(deck, "field.capital")
                ),
                new_name,
            )],
        )
    });

    assert_all_targets_export_exact_diffs("field addition", |target, deck, baseline_json| {
        let note_type = ug_note_type(deck);
        let field_id = sid("field.zzz-regression");
        let field_name = format!("Regression Added Field {target}");
        let field_count = note_type.fields.len();
        let finland_guid = note_guid(deck, "note.finland");
        let finland_value = format!("Regression field value {target}");

        let mut note_type_change = empty_note_type_change();
        note_type_change.fields.insert(
            field_id.clone(),
            FieldDefinitionChange {
                intent: ChangeIntent::Add,
                field: Some(FieldDefinition {
                    id: field_id.clone(),
                    name: field_name.clone(),
                }),
                expected_base: None,
            },
        );

        let mut note_change = empty_note_change();
        note_change.fields.insert(
            field_id,
            FieldChange {
                intent: ChangeIntent::Add,
                value: Some(finland_value.clone()),
                expected_base: None,
            },
        );

        let mut overlay = empty_overlay(&format!("overlay.regression.field-addition.{target}"));
        overlay
            .note_type_changes
            .insert(note_type.id.clone(), note_type_change);
        overlay
            .note_changes
            .insert(sid("note.finland"), note_change);

        let mut expected = vec![ExpectedJsonValue {
            path: format!("/note_models/0/flds/{field_count}"),
            value: Some(serde_json::json!({
                "font": "Arial",
                "media": [],
                "name": field_name,
                "ord": field_count,
                "rtl": false,
                "size": 20,
                "sticky": false,
            })),
        }];
        for (note_index, note) in baseline_json["notes"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            let value = if note["guid"].as_str() == Some(finland_guid.as_str()) {
                finland_value.clone()
            } else {
                String::new()
            };
            expected.push(expected_json(
                &format!("/notes/{note_index}/fields/{field_count}"),
                value,
            ));
        }
        MutationExpectation::new(overlay, expected)
    });
}

#[test]
fn ug_regression_card_template_changes_flow_to_crowdanki_for_every_target() {
    assert_all_targets_export_exact_diffs("template name", |target, deck, _json| {
        let note_type = ug_note_type(deck);
        let template_id = sid("template.country-capital");
        let template = note_type
            .card_templates
            .iter()
            .find(|template| template.id == template_id)
            .expect("UG note type has Country - Capital template");
        let new_name = format!("Regression Template Name {target}");
        let mut template_change = empty_card_template_change();
        template_change.name = Some(replace_property(new_name.clone(), template.name.clone()));
        let mut note_type_change = empty_note_type_change();
        note_type_change
            .card_templates
            .insert(template_id, template_change);
        let mut overlay = empty_overlay(&format!("overlay.regression.template-name.{target}"));
        overlay
            .note_type_changes
            .insert(note_type.id.clone(), note_type_change);
        MutationExpectation::new(
            overlay,
            vec![expected_json(
                &format!(
                    "/note_models/0/tmpls/{}/name",
                    template_index(deck, "template.country-capital")
                ),
                new_name,
            )],
        )
    });

    assert_all_targets_export_exact_diffs("template question", |target, deck, _json| {
        let note_type = ug_note_type(deck);
        let template_id = sid("template.country-capital");
        let template = note_type
            .card_templates
            .iter()
            .find(|template| template.id == template_id)
            .expect("UG note type has Country - Capital template");
        let new_question = format!("<div>Regression question {target}</div>");
        let mut template_change = empty_card_template_change();
        template_change.question_format = Some(replace_property(
            new_question.clone(),
            template.question_format.clone(),
        ));
        let mut note_type_change = empty_note_type_change();
        note_type_change
            .card_templates
            .insert(template_id, template_change);
        let mut overlay = empty_overlay(&format!("overlay.regression.template-question.{target}"));
        overlay
            .note_type_changes
            .insert(note_type.id.clone(), note_type_change);
        MutationExpectation::new(
            overlay,
            vec![expected_json(
                &format!(
                    "/note_models/0/tmpls/{}/qfmt",
                    template_index(deck, "template.country-capital")
                ),
                new_question,
            )],
        )
    });

    assert_all_targets_export_exact_diffs("template answer", |target, deck, _json| {
        let note_type = ug_note_type(deck);
        let template_id = sid("template.country-capital");
        let template = note_type
            .card_templates
            .iter()
            .find(|template| template.id == template_id)
            .expect("UG note type has Country - Capital template");
        let new_answer = format!("<div>Regression answer {target}</div>");
        let mut template_change = empty_card_template_change();
        template_change.answer_format = Some(replace_property(
            new_answer.clone(),
            template.answer_format.clone(),
        ));
        let mut note_type_change = empty_note_type_change();
        note_type_change
            .card_templates
            .insert(template_id, template_change);
        let mut overlay = empty_overlay(&format!("overlay.regression.template-answer.{target}"));
        overlay
            .note_type_changes
            .insert(note_type.id.clone(), note_type_change);
        MutationExpectation::new(
            overlay,
            vec![expected_json(
                &format!(
                    "/note_models/0/tmpls/{}/afmt",
                    template_index(deck, "template.country-capital")
                ),
                new_answer,
            )],
        )
    });

    assert_all_targets_export_exact_diffs(
        "template variable rendered question",
        |target, deck, _json| {
            let note_type = ug_note_type(deck);
            let template_id = sid("template.country-capital");
            let template = note_type
                .card_templates
                .iter()
                .find(|template| template.id == template_id)
                .expect("UG note type has Country - Capital template");
            let rendered_value = format!("Regression template variable {target}");
            let mut template_change = empty_card_template_change();
            template_change.variables.insert(
                "regression.template".to_owned(),
                PropertyChange {
                    intent: ChangeIntent::Add,
                    value: Some(rendered_value.clone()),
                    expected_base: None,
                },
            );
            template_change.question_format = Some(replace_property(
                "${regression.template}".to_owned(),
                template.question_format.clone(),
            ));
            let mut note_type_change = empty_note_type_change();
            note_type_change
                .card_templates
                .insert(template_id, template_change);
            let mut overlay =
                empty_overlay(&format!("overlay.regression.template-variable.{target}"));
            overlay
                .note_type_changes
                .insert(note_type.id.clone(), note_type_change);
            MutationExpectation::new(
                overlay,
                vec![expected_json(
                    &format!(
                        "/note_models/0/tmpls/{}/qfmt",
                        template_index(deck, "template.country-capital")
                    ),
                    rendered_value,
                )],
            )
        },
    );

    assert_all_targets_export_exact_diffs("template addition", |target, deck, _json| {
        let note_type = ug_note_type(deck);
        let template_count = note_type.card_templates.len();
        let template_id = sid("template.zzz-regression");
        let mut template_change = empty_card_template_change();
        template_change.intent = ChangeIntent::Add;
        template_change.insert_after = note_type
            .card_templates
            .last()
            .map(|template| template.id.clone());
        template_change.template = Some(brain_brew_formats::core::CardTemplate {
            id: template_id.clone(),
            name: format!("Regression Added Template {target}"),
            variables: BTreeMap::new(),
            question_format: format!("Regression added question {target}"),
            answer_format: format!("Regression added answer {target}"),
            adapter_ids: AdapterIds::new(),
        });
        let expected_template = template_change.template.as_ref().unwrap().clone();
        let mut note_type_change = empty_note_type_change();
        note_type_change
            .card_templates
            .insert(template_id, template_change);
        let mut overlay = empty_overlay(&format!("overlay.regression.template-addition.{target}"));
        overlay
            .note_type_changes
            .insert(note_type.id.clone(), note_type_change);
        MutationExpectation::new(
            overlay,
            vec![ExpectedJsonValue {
                path: format!("/note_models/0/tmpls/{template_count}"),
                value: Some(serde_json::json!({
                    "afmt": expected_template.answer_format,
                    "bafmt": "",
                    "bfont": "",
                    "bqfmt": "",
                    "bsize": 0,
                    "did": null,
                    "name": expected_template.name,
                    "ord": template_count,
                    "qfmt": expected_template.question_format,
                    "scratchPad": 0,
                })),
            }],
        )
    });
}

#[test]
fn ug_regression_note_changes_flow_to_crowdanki_for_every_target() {
    assert_all_targets_export_exact_diffs("note field", |target, deck, _json| {
        let note_id = sid("note.finland");
        let field_id = sid("field.capital");
        let current_value = deck.notes[&note_id].fields[&field_id].clone();
        let new_value = format!("Regression capital {target}");
        let mut note_change = empty_note_change();
        note_change.fields.insert(
            field_id,
            FieldChange {
                intent: ChangeIntent::Override,
                value: Some(new_value.clone()),
                expected_base: Some(ExpectedBase::Value(current_value)),
            },
        );
        let mut overlay = empty_overlay(&format!("overlay.regression.note-field.{target}"));
        overlay.note_changes.insert(note_id, note_change);
        MutationExpectation::new(
            overlay,
            vec![expected_json(
                &note_field_json_path(deck, "note.finland", "field.capital"),
                new_value,
            )],
        )
    });

    assert_all_targets_export_exact_diffs("note variable rendered field", |target, deck, _json| {
        let note_id = sid("note.finland");
        let field_id = sid("field.country");
        let current_value = deck.notes[&note_id].fields[&field_id].clone();
        let rendered_value = format!("Regression rendered country {target}");
        let mut note_change = empty_note_change();
        note_change.variables.insert(
            "regression.country".to_owned(),
            PropertyChange {
                intent: ChangeIntent::Add,
                value: Some(rendered_value.clone()),
                expected_base: None,
            },
        );
        note_change.fields.insert(
            field_id,
            FieldChange {
                intent: ChangeIntent::Override,
                value: Some("${regression.country}".to_owned()),
                expected_base: Some(ExpectedBase::Value(current_value)),
            },
        );
        let mut overlay = empty_overlay(&format!("overlay.regression.note-variable.{target}"));
        overlay.note_changes.insert(note_id, note_change);
        MutationExpectation::new(
            overlay,
            vec![expected_json(
                &note_field_json_path(deck, "note.finland", "field.country"),
                rendered_value,
            )],
        )
    });

    assert_all_targets_export_exact_diffs("note tag", |target, deck, baseline_json| {
        let tag = "ZZZ::Regression".to_owned();
        let mut note_change = empty_note_change();
        note_change.tags.insert(
            tag.clone(),
            TagChange {
                intent: ChangeIntent::Add,
                expected_base: None,
            },
        );
        let mut overlay = empty_overlay(&format!("overlay.regression.note-tag.{target}"));
        overlay
            .note_changes
            .insert(sid("note.finland"), note_change);
        let note_index = note_json_index(baseline_json, &note_guid(deck, "note.finland"));
        let tag_index = baseline_json["notes"][note_index]["tags"]
            .as_array()
            .unwrap()
            .len();
        MutationExpectation::new(
            overlay,
            vec![expected_json(
                &format!("/notes/{note_index}/tags/{tag_index}"),
                tag,
            )],
        )
    });

    assert_all_targets_export_exact_diffs("note guid", |target, deck, baseline_json| {
        let note_id = sid("note.finland");
        let current_guid = note_guid(deck, "note.finland");
        let new_guid = format!("regression-guid-{target}");
        let mut note_change = empty_note_change();
        note_change.adapter_ids.insert(
            "crowdanki:guid".to_owned(),
            replace_adapter_id(new_guid.clone(), current_guid.clone()),
        );
        let mut overlay = empty_overlay(&format!("overlay.regression.note-guid.{target}"));
        overlay.note_changes.insert(note_id, note_change);
        let note_index = note_json_index(baseline_json, &current_guid);
        MutationExpectation::new(
            overlay,
            vec![expected_json(
                &format!("/notes/{note_index}/guid"),
                new_guid,
            )],
        )
    });

    assert_all_targets_export_exact_diffs("note removal", |target, deck, baseline_json| {
        let (note_id, note) = deck
            .notes
            .iter()
            .rev()
            .find(|(note_id, _)| !deck.tombstones.contains(note_id))
            .expect("UG fixture has at least one exported note");
        let note_index = baseline_json["notes"].as_array().unwrap().len() - 1;
        assert_eq!(
            baseline_json["notes"][note_index]["guid"].as_str(),
            note.adapter_ids.get("crowdanki:guid"),
            "test removes the final exported note so the exact CrowdAnki diff is one missing array item"
        );
        let mut note_change = empty_note_change();
        note_change.intent = ChangeIntent::Remove;
        note_change.expected_base = Some(ExpectedBase::EntityPresent);
        let mut overlay = empty_overlay(&format!("overlay.regression.note-removal.{target}"));
        overlay.note_changes.insert(note_id.clone(), note_change);
        MutationExpectation::new(
            overlay,
            vec![expected_absent(&format!("/notes/{note_index}"))],
        )
    });

    assert_all_targets_export_exact_diffs("note addition", |target, deck, baseline_json| {
        let note_type = ug_note_type(deck);
        let note_id = sid("note.zzz-regression");
        let guid = format!("regression-added-note-{target}");
        let mut adapter_ids = AdapterIds::new();
        adapter_ids.insert("crowdanki:guid", guid.clone());
        let fields = note_type
            .fields
            .iter()
            .map(|field| {
                (
                    field.id.clone(),
                    format!("Regression {} {target}", field.name),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let expected_fields = note_type
            .fields
            .iter()
            .map(|field| fields[&field.id].clone())
            .collect::<Vec<_>>();
        let note = Note {
            id: note_id.clone(),
            note_type_id: note_type.id.clone(),
            variables: BTreeMap::new(),
            fields,
            tags: BTreeSet::from(["ZZZ::Regression".to_owned()]),
            adapter_ids,
        };
        let mut note_change = empty_note_change();
        note_change.intent = ChangeIntent::Add;
        note_change.note = Some(note);
        let mut overlay = empty_overlay(&format!("overlay.regression.note-addition.{target}"));
        overlay.note_changes.insert(note_id, note_change);
        let note_index = baseline_json["notes"].as_array().unwrap().len();
        MutationExpectation::new(
            overlay,
            vec![ExpectedJsonValue {
                path: format!("/notes/{note_index}"),
                value: Some(serde_json::json!({
                    "__type__": "Note",
                    "data": "",
                    "fields": expected_fields,
                    "flags": 0,
                    "guid": guid,
                    "note_model_uuid": baseline_json["note_models"][0]["crowdanki_uuid"].as_str().unwrap(),
                    "tags": ["ZZZ::Regression"],
                })),
            }],
        )
    });
}

#[test]
fn ug_regression_media_changes_flow_to_crowdanki_for_every_target() {
    assert_all_targets_export_exact_diffs("media addition", |target, _deck, baseline_json| {
        let path = format!("zzzz-regression-{target}.css");
        let mut overlay = empty_overlay(&format!("overlay.regression.media-addition.{target}"));
        overlay.media_changes.insert(
            sid("media.zzzz-regression"),
            MediaChange {
                intent: ChangeIntent::Add,
                media: Some(MediaReference {
                    id: sid("media.zzzz-regression"),
                    path: path.clone(),
                    sha256: String::new(),
                }),
                expected_base: None,
            },
        );
        let media_index = baseline_json["media_files"].as_array().unwrap().len();
        MutationExpectation::new(
            overlay,
            vec![expected_json(&format!("/media_files/{media_index}"), path)],
        )
    });

    assert_all_targets_export_exact_diffs("media path override", |target, deck, baseline_json| {
        let (media_id, media) = deck.media.iter().next_back().expect("UG fixture has media");
        let media_index = baseline_json["media_files"].as_array().unwrap().len() - 1;
        assert_eq!(
            baseline_json["media_files"][media_index].as_str(),
            Some(media.path.as_str()),
            "test updates the final exported media path so the exact CrowdAnki diff is one array item"
        );
        let new_path = format!("zzzz-regression-override-{target}.css");
        let mut overlay = empty_overlay(&format!("overlay.regression.media-path.{target}"));
        overlay.media_changes.insert(
            media_id.clone(),
            MediaChange {
                intent: ChangeIntent::Override,
                media: Some(MediaReference {
                    id: media_id.clone(),
                    path: new_path.clone(),
                    sha256: media.sha256.clone(),
                }),
                expected_base: Some(ExpectedBase::EntityPresent),
            },
        );
        MutationExpectation::new(
            overlay,
            vec![expected_json(
                &format!("/media_files/{media_index}"),
                new_path,
            )],
        )
    });
}

fn compose_target(
    root: &Path,
    manifest: &manifest::FederatedDeckManifest,
    target: &str,
) -> CanonicalDeck {
    compose_target_with_extra_overlay(root, manifest, target, None)
}

fn compose_target_with_extra_overlay(
    root: &Path,
    manifest: &manifest::FederatedDeckManifest,
    target: &str,
    extra_overlay: Option<Overlay>,
) -> CanonicalDeck {
    let expanded = manifest
        .expand_target(target)
        .unwrap_or_else(|error| panic!("{target} expands: {error}"));
    let base = canonical_yaml::from_str(&fs::read_to_string(root.join(&expanded.base)).unwrap())
        .unwrap_or_else(|error| panic!("{target} base parses: {error}"));
    let mut overlays = expanded
        .overlays
        .iter()
        .map(|overlay| {
            canonical_yaml::overlay_from_str(&fs::read_to_string(root.join(&overlay.file)).unwrap())
                .unwrap_or_else(|error| panic!("{target} overlay {} parses: {error}", overlay.id))
        })
        .collect::<Vec<_>>();
    if let Some(extra_overlay) = extra_overlay {
        overlays.push(extra_overlay);
    }
    base.compose(&overlays)
        .unwrap_or_else(|error| panic!("{target} composes: {error}"))
}

struct MutationExpectation {
    overlay: Overlay,
    expected: Vec<ExpectedJsonValue>,
}

impl MutationExpectation {
    fn new(overlay: Overlay, expected: Vec<ExpectedJsonValue>) -> Self {
        Self { overlay, expected }
    }
}

struct ExpectedJsonValue {
    path: String,
    value: Option<serde_json::Value>,
}

fn assert_all_targets_export_exact_diffs(
    case_name: &str,
    build: impl Fn(&str, &CanonicalDeck, &serde_json::Value) -> MutationExpectation,
) {
    let root = fixture_root();
    let manifest = read_manifest(&root);
    for target in manifest.targets.keys() {
        let baseline_deck = compose_target(&root, &manifest, target);
        let baseline_json = exported_json(&baseline_deck);
        let expectation = build(target, &baseline_deck, &baseline_json);
        assert!(
            !expectation.expected.is_empty(),
            "{case_name} for {target} must expect at least one CrowdAnki difference"
        );

        let changed_deck =
            compose_target_with_extra_overlay(&root, &manifest, target, Some(expectation.overlay));
        let changed_json = exported_json(&changed_deck);
        let actual_paths = json_diff_paths(&baseline_json, &changed_json);
        let expected_paths = expectation
            .expected
            .iter()
            .map(|expected| expected.path.clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            actual_paths, expected_paths,
            "{case_name} for {target} changed unexpected CrowdAnki JSON paths"
        );
        for expected in expectation.expected {
            match expected.value {
                Some(value) => assert_eq!(
                    changed_json.pointer(&expected.path),
                    Some(&value),
                    "{case_name} for {target} expected CrowdAnki value at {}",
                    expected.path
                ),
                None => assert!(
                    changed_json.pointer(&expected.path).is_none(),
                    "{case_name} for {target} expected no CrowdAnki value at {}",
                    expected.path
                ),
            }
        }
    }
}

fn exported_json(deck: &CanonicalDeck) -> serde_json::Value {
    let export = crowdanki::export_deck(deck).expect("deck exports to CrowdAnki");
    serde_json::from_str(&export.deck_json).expect("CrowdAnki export is JSON")
}

fn json_diff_paths(left: &serde_json::Value, right: &serde_json::Value) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    collect_json_diff_paths(left, right, "", &mut paths);
    paths
}

fn collect_json_diff_paths(
    left: &serde_json::Value,
    right: &serde_json::Value,
    path: &str,
    paths: &mut BTreeSet<String>,
) {
    match (left, right) {
        (serde_json::Value::Object(left), serde_json::Value::Object(right)) => {
            let keys = left.keys().chain(right.keys()).collect::<BTreeSet<_>>();
            for key in keys {
                let child_path = format!("{path}/{}", json_pointer_token(key));
                match (left.get(key), right.get(key)) {
                    (Some(left), Some(right)) => {
                        collect_json_diff_paths(left, right, &child_path, paths)
                    }
                    _ => {
                        paths.insert(child_path);
                    }
                }
            }
        }
        (serde_json::Value::Array(left), serde_json::Value::Array(right)) => {
            for index in 0..left.len().max(right.len()) {
                let child_path = format!("{path}/{index}");
                match (left.get(index), right.get(index)) {
                    (Some(left), Some(right)) => {
                        collect_json_diff_paths(left, right, &child_path, paths)
                    }
                    _ => {
                        paths.insert(child_path);
                    }
                }
            }
        }
        _ if left == right => {}
        _ => {
            paths.insert(if path.is_empty() {
                "/".to_owned()
            } else {
                path.to_owned()
            });
        }
    }
}

fn json_pointer_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

fn expected_json(path: &str, value: impl Into<serde_json::Value>) -> ExpectedJsonValue {
    ExpectedJsonValue {
        path: path.to_owned(),
        value: Some(value.into()),
    }
}

fn expected_absent(path: &str) -> ExpectedJsonValue {
    ExpectedJsonValue {
        path: path.to_owned(),
        value: None,
    }
}

fn empty_overlay(id: &str) -> Overlay {
    Overlay {
        id: sid(id),
        kind: OverlayKind::Patch,
        translations: None,
        deck_change: None,
        note_changes: BTreeMap::new(),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    }
}

fn empty_deck_change() -> DeckChange {
    DeckChange {
        name: None,
        description: None,
        variables: BTreeMap::new(),
        adapter_ids: BTreeMap::new(),
    }
}

fn empty_note_type_change() -> NoteTypeChange {
    NoteTypeChange {
        intent: ChangeIntent::Merge,
        note_type: None,
        name: None,
        variables: BTreeMap::new(),
        styling: None,
        fields: BTreeMap::new(),
        card_templates: BTreeMap::new(),
        adapter_ids: BTreeMap::new(),
        expected_base: None,
    }
}

fn empty_card_template_change() -> CardTemplateChange {
    CardTemplateChange {
        intent: ChangeIntent::Merge,
        template: None,
        insert_after: None,
        name: None,
        variables: BTreeMap::new(),
        question_format: None,
        answer_format: None,
        adapter_ids: BTreeMap::new(),
        expected_base: None,
    }
}

fn empty_note_change() -> NoteChange {
    NoteChange {
        intent: ChangeIntent::Merge,
        note: None,
        variables: BTreeMap::new(),
        fields: BTreeMap::new(),
        tags: BTreeMap::new(),
        adapter_ids: BTreeMap::new(),
        expected_base: None,
    }
}

fn replace_property(value: String, expected_base: String) -> PropertyChange {
    PropertyChange {
        intent: ChangeIntent::Override,
        value: Some(value),
        expected_base: Some(ExpectedBase::Value(expected_base)),
    }
}

fn replace_adapter_id(value: String, expected_base: String) -> AdapterIdChange {
    AdapterIdChange {
        intent: ChangeIntent::Override,
        value: Some(value),
        expected_base: Some(ExpectedBase::Value(expected_base)),
    }
}

fn ug_note_type(deck: &CanonicalDeck) -> &brain_brew_formats::core::NoteType {
    deck.note_types
        .get(&sid("note-type.ultimate-geography"))
        .expect("UG fixture has ultimate geography note type")
}

fn field_index(deck: &CanonicalDeck, field_id: &str) -> usize {
    let field_id = sid(field_id);
    ug_note_type(deck)
        .fields
        .iter()
        .position(|field| field.id == field_id)
        .unwrap_or_else(|| panic!("field {field_id} exists"))
}

fn template_index(deck: &CanonicalDeck, template_id: &str) -> usize {
    let template_id = sid(template_id);
    ug_note_type(deck)
        .card_templates
        .iter()
        .position(|template| template.id == template_id)
        .unwrap_or_else(|| panic!("template {template_id} exists"))
}

fn note_guid(deck: &CanonicalDeck, note_id: &str) -> String {
    deck.notes[&sid(note_id)]
        .adapter_ids
        .get("crowdanki:guid")
        .unwrap_or_else(|| panic!("{note_id} has CrowdAnki guid"))
        .to_owned()
}

fn note_json_index(json: &serde_json::Value, guid: &str) -> usize {
    json["notes"]
        .as_array()
        .unwrap()
        .iter()
        .position(|note| note["guid"].as_str() == Some(guid))
        .unwrap_or_else(|| panic!("CrowdAnki note {guid} exists"))
}

fn note_field_json_path(deck: &CanonicalDeck, note_id: &str, field_id: &str) -> String {
    let json = exported_json(deck);
    let note_index = note_json_index(&json, &note_guid(deck, note_id));
    let field_index = field_index(deck, field_id);
    format!("/notes/{note_index}/fields/{field_index}")
}

fn read_manifest(root: &Path) -> manifest::FederatedDeckManifest {
    manifest::from_str(&fs::read_to_string(root.join("brainbrew.yaml")).unwrap())
        .expect("manifest parses")
}

fn release_oracle_deck_json_path(root: &Path, target: &str) -> PathBuf {
    let (language, variant) = target_parts(target);
    let suffix = if variant == "extended" {
        " [Extended]"
    } else {
        ""
    };
    root.join(format!("Ultimate Geography [{language}]{suffix}/deck.json"))
}

fn target_parts(target: &str) -> (&str, &str) {
    if let Some(variant) = target.strip_prefix("zh-tw-") {
        return ("ZH-TW", variant);
    }
    let (language, variant) = target.split_once('-').unwrap();
    (language.to_ascii_uppercase().leak(), variant)
}

fn assert_crowdanki_semantic_subset_eq(
    old: &serde_json::Value,
    new: &serde_json::Value,
    target: &str,
) {
    assert_eq!(new["name"], old["name"], "{target} deck name");
    assert_eq!(
        new["crowdanki_uuid"], old["crowdanki_uuid"],
        "{target} deck UUID"
    );
    assert_eq!(new["desc"], old["desc"], "{target} deck description");

    assert_eq!(
        string_set(new["media_files"].as_array().unwrap()),
        string_set(old["media_files"].as_array().unwrap()),
        "{target} media files"
    );

    let old_model = &old["note_models"][0];
    let new_model = &new["note_models"][0];
    assert_eq!(new_model["name"], old_model["name"], "{target} model name");
    assert_eq!(
        new_model["crowdanki_uuid"], old_model["crowdanki_uuid"],
        "{target} model UUID"
    );
    assert_eq!(new_model["css"], old_model["css"], "{target} CSS");
    assert_eq!(
        field_names(new_model),
        field_names(old_model),
        "{target} fields"
    );
    assert_eq!(
        templates_by_ord(new_model),
        templates_by_ord(old_model),
        "{target} templates"
    );

    let old_notes = notes_by_guid(old);
    let new_notes = notes_by_guid(new);
    assert_eq!(new_notes.len(), old_notes.len(), "{target} note count");
    for (guid, old_note) in old_notes {
        let new_note = new_notes
            .get(&guid)
            .unwrap_or_else(|| panic!("{target} missing note {guid}"));
        assert_eq!(
            new_note["note_model_uuid"], old_note["note_model_uuid"],
            "{target} note model differs for {guid}"
        );
        assert_eq!(
            new_note["fields"], old_note["fields"],
            "{target} fields differ for {guid}"
        );
        assert_eq!(
            string_set(new_note["tags"].as_array().unwrap()),
            string_set(old_note["tags"].as_array().unwrap()),
            "{target} tags differ for {guid}"
        );
    }
}

fn field_names(model: &serde_json::Value) -> Vec<String> {
    model["flds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field["name"].as_str().unwrap().to_owned())
        .collect()
}

fn templates_by_ord(model: &serde_json::Value) -> BTreeMap<i64, (String, String, String)> {
    model["tmpls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|template| {
            (
                template["ord"].as_i64().unwrap(),
                (
                    template["name"].as_str().unwrap().to_owned(),
                    template["qfmt"].as_str().unwrap().to_owned(),
                    template["afmt"].as_str().unwrap().to_owned(),
                ),
            )
        })
        .collect()
}

fn notes_by_guid(deck: &serde_json::Value) -> BTreeMap<String, &serde_json::Value> {
    deck["notes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|note| (note["guid"].as_str().unwrap().to_owned(), note))
        .collect()
}

fn string_set(values: &[serde_json::Value]) -> BTreeSet<String> {
    values
        .iter()
        .map(|value| value.as_str().unwrap().to_owned())
        .collect()
}

fn sid(value: &str) -> StableId {
    StableId::new(value).expect("test stable id is valid")
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/ultimate-geography")
}

fn ultimate_geography_release_oracle_root() -> PathBuf {
    std::env::var_os("BRAINBREW_UG_CROWDANKI_ORACLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../.cache/brainbrew/ug-release-oracle/v5.3/crowdanki")
        })
}
