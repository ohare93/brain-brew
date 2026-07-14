use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use brain_brew_formats::core::{
    AdapterIdChange, AdapterIds, CanonicalDeck, CardTemplateChange, ChangeIntent, DeckChange,
    ExpectedBase, FieldChange, FieldDefinition, FieldDefinitionChange, MediaChange, MediaReference,
    Note, NoteChange, NoteTypeChange, Overlay, OverlayKind, PropertyChange, StableId, TagChange,
    TranslationDictionary, fingerprint_field_definition, fingerprint_media_reference,
    fingerprint_note,
};
use brain_brew_formats::{
    canonical_yaml, crowdanki, lockfile, manifest, media, media_map, source_includes,
};

fn import_approved(input: &str) -> Result<CanonicalDeck, crowdanki::CrowdAnkiError> {
    let plan = crowdanki::plan_import(input.as_bytes())?;
    crowdanki::apply_import_plan(input.as_bytes(), &plan, true)
}

#[test]
fn ultimate_geography_fixture_uses_file_includes_for_large_source_text() {
    let root = fixture_root();
    let deck_source = fs::read_to_string(root.join("deck.yaml")).unwrap();
    assert!(
        deck_source.contains("description: !include descriptions/ultimate-geography/en.html"),
        "deck description should live outside deck.yaml"
    );
    assert!(
        deck_source.contains(
            "question_format: !include templates/ultimate-geography/capital-country/question.html"
        ),
        "standard template HTML should live outside deck.yaml"
    );
    assert!(
        deck_source.contains("styling: !include styles/ultimate-geography/card.css"),
        "note type CSS should live outside deck.yaml"
    );
    assert!(
        root.join("descriptions/ultimate-geography/en.html")
            .exists()
    );
    assert!(
        root.join("templates/ultimate-geography/country-capital/answer.html")
            .exists()
    );
    assert!(root.join("styles/ultimate-geography/card.css").exists());

    let extended_source = fs::read_to_string(root.join("overlays/variants/extended.yaml")).unwrap();
    assert!(
        extended_source.contains(
            "question_format: !include templates/ultimate-geography/country-flag/question.html"
        ),
        "shared Extended template HTML should live outside overlay YAML"
    );
    assert!(
        root.join("templates/ultimate-geography/country-map/answer.html")
            .exists()
    );

    let experimental_source =
        fs::read_to_string(root.join("overlays/variants/experimental.yaml")).unwrap();
    assert!(
        experimental_source.contains(
            "value: !include templates/ultimate-geography/experimental/country-map-question.html"
        ),
        "Experimental interactive map template HTML should live outside overlay YAML"
    );
    assert!(
        root.join("templates/ultimate-geography/experimental/country-map-answer.html")
            .exists()
    );

    for language in ["es", "he", "sv", "zh", "zh-tw"] {
        let overlay_source =
            fs::read_to_string(root.join(format!("overlays/languages/{language}.yaml"))).unwrap();
        assert!(
            overlay_source.contains(&format!(
                "value: !include descriptions/ultimate-geography/{language}.html"
            )),
            "{language} deck description should live outside translation overlay YAML"
        );
    }
    let sv_extended_source =
        fs::read_to_string(root.join("overlays/variants/extended/sv.yaml")).unwrap();
    assert!(
        sv_extended_source.contains("value: !include descriptions/ultimate-geography/en.html"),
        "Swedish Extended metadata override should reuse the shared English description include"
    );
}

#[test]
fn ultimate_geography_fixture_uses_structured_messages_for_flag_similarity() {
    let root = fixture_root();
    for relative_path in ["deck.yaml", "overlays/extensions/hardcore.yaml"] {
        let source = fs::read_to_string(root.join(relative_path)).unwrap();
        assert!(
            source.contains("field.flag-similarity:\n        format:")
                || source.contains("field.flag-similarity:\n          format:"),
            "{relative_path} should use inline formatted structured messages for non-empty flag similarity fields"
        );
        assert!(
            !source.lines().any(
                |line| line.trim_start().starts_with("field.flag-similarity: '")
                    && line.trim() != "field.flag-similarity: ''"
            ),
            "{relative_path} should not keep long scalar flag similarity strings"
        );
    }

    let manifest = read_manifest(&root);
    let flag_similarity_sources: BTreeSet<_> = ["en-standard", "en-hardcore-standard"]
        .into_iter()
        .flat_map(|target| {
            let deck = compose_target(&root, &manifest, target);
            let field_id = sid("field.flag-similarity");
            deck.notes
                .iter()
                .filter(|(_, note)| note.fields.contains_key(&field_id))
                .map(|(note_id, _)| {
                    deck.field_text(note_id, &field_id)
                        .expect("UG structured message graph resolves")
                })
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
        })
        .collect();

    for overlay_ref in manifest
        .overlays
        .values()
        .filter(|overlay| overlay.kind.as_deref() == Some("translation"))
    {
        let overlay_path = root.join(&overlay_ref.file);
        let overlay = read_overlay_file(&root, &overlay_path)
            .unwrap_or_else(|error| panic!("{} parses: {error}", overlay_ref.file));
        let translations = overlay
            .translations
            .as_ref()
            .unwrap_or_else(|| panic!("{} uses translation dictionary", overlay_ref.file));
        for source in &flag_similarity_sources {
            assert!(
                !translations.direct.contains_key(source),
                "{} should translate structured flag-similarity components, not duplicate the full composite source key {source:?}",
                overlay_ref.file
            );
        }
    }
}

#[test]
fn ultimate_geography_fixture_manifest_composes_all_targets() {
    let root = fixture_root();
    let manifest = read_manifest(&root);
    let base_path = root.join(&manifest.base);
    let base = read_canonical_deck_file(&root, &base_path)
        .unwrap_or_else(|error| panic!("{} resolves and parses: {error}", base_path.display()));
    let formatted = canonical_yaml::to_string(&base).expect("resolved base emits canonical YAML");
    assert!(
        canonical_yaml::from_str(&formatted)
            .expect("formatted resolved base parses")
            .semantic_diff(&base)
            .is_empty(),
        "{} resolves to deterministic canonical content",
        base_path.display()
    );

    let package = manifest
        .package
        .as_ref()
        .expect("fixture has package metadata");
    assert_eq!(package.id, "anki-geo.ultimate-geography");
    assert_eq!(package.version, "0.1.0");

    assert_eq!(manifest.targets.len(), 74);
    assert_eq!(manifest.languages.len(), 16);
    assert!(manifest.languages.contains_key("he"));
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
        assert_crowdanki_round_trip_profile(&deck, target);
    }
}

#[test]
fn ultimate_geography_main_and_hardcore_stack_coverage_is_deterministic_and_excludes_ids() {
    let root = fixture_root();
    let manifest = read_manifest(&root);

    for target in ["de-standard", "de-hardcore-standard"] {
        let expanded = manifest.expand_target(target).expect("target expands");
        let base = read_canonical_deck_file(&root, &root.join(&expanded.base))
            .expect("target base parses");
        let overlays = expanded
            .overlays
            .iter()
            .map(|entry| read_overlay_file(&root, &root.join(&entry.file)).expect("overlay parses"))
            .collect::<Vec<_>>();
        let first = base
            .translation_stack_coverage(&overlays)
            .expect("target stack composes for coverage");
        let second = base
            .translation_stack_coverage(&overlays)
            .expect("target stack repeats deterministically");

        assert_eq!(first, second, "{target} coverage order is deterministic");
        assert_eq!(
            first.target_stack,
            overlays
                .iter()
                .map(|overlay| overlay.id.clone())
                .collect::<Vec<_>>(),
            "{target} retains its exact expanded stack"
        );
        assert!(
            first
                .entries
                .iter()
                .all(|entry| !entry.source_path.contains("adapter_ids")),
            "{target} must not inflate normal coverage with adapter identifiers"
        );
    }
}

#[test]
fn ultimate_geography_fixture_formatting_is_byte_idempotent() {
    let root = fixture_root();

    for manifest_file in ["brainbrew.yaml", "brainbrew-hardcore.yaml"] {
        let source = fs::read_to_string(root.join(manifest_file)).unwrap();
        let once = manifest::format_str(&source)
            .unwrap_or_else(|error| panic!("{manifest_file} formats: {error}"));
        let twice = manifest::format_str(&once)
            .unwrap_or_else(|error| panic!("{manifest_file} formats twice: {error}"));
        assert_eq!(twice, once, "{manifest_file} formatting is byte-idempotent");
    }

    for deck_file in ["deck.yaml", "deck-hardcore.yaml"] {
        let deck_path = root.join(deck_file);
        let deck = read_canonical_deck_file(&root, &deck_path)
            .unwrap_or_else(|error| panic!("{deck_file} resolves and parses: {error}"));
        let once = canonical_yaml::to_string(&deck)
            .unwrap_or_else(|error| panic!("{deck_file} emits: {error}"));
        let twice = canonical_yaml::format_str(&once)
            .unwrap_or_else(|error| panic!("{deck_file} formats twice: {error}"));
        assert_eq!(twice, once, "{deck_file} formatting is byte-idempotent");
    }

    let media_source = fs::read_to_string(root.join("media.yaml")).unwrap();
    let media_once = media_map::format_str(&media_source).expect("media.yaml formats");
    let media_twice = media_map::format_str(&media_once).expect("media.yaml formats twice");
    assert_eq!(
        media_twice, media_once,
        "media.yaml formatting is byte-idempotent"
    );

    let mut overlay_files = Vec::new();
    collect_yaml_files(&root.join("overlays"), &mut overlay_files);
    overlay_files.sort();
    for overlay_path in overlay_files {
        let overlay = read_overlay_file(&root, &overlay_path).unwrap_or_else(|error| {
            panic!("{} resolves and parses: {error}", overlay_path.display())
        });
        let once = canonical_yaml::overlay_to_string(&overlay)
            .unwrap_or_else(|error| panic!("{} emits: {error}", overlay_path.display()));
        let twice = canonical_yaml::overlay_format_str(&once)
            .unwrap_or_else(|error| panic!("{} formats twice: {error}", overlay_path.display()));
        assert_eq!(
            twice,
            once,
            "{} formatting is byte-idempotent",
            overlay_path.display()
        );
    }

    let lock_source = r#"
packages:
  anki-geo.ultimate-geography:
    locked:
      nar_hash: sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=
      rev: ccf150a1b21e0000000000000000000000000000
      url: https://github.com/anki-geo/ultimate-geography.git
      type: git
    original:
      ref: main
      url: https://github.com/anki-geo/ultimate-geography.git
      type: git
    package:
      version: 0.1.0
    manifest: brainbrew.yaml
version: 2
"#;
    let lock_once = lockfile::format_str(lock_source).expect("fixture-style lock formats");
    let lock_twice = lockfile::format_str(&lock_once).expect("fixture-style lock formats twice");
    assert_eq!(
        lock_twice, lock_once,
        "lockfile formatting is byte-idempotent"
    );
}

#[test]
fn ultimate_geography_media_map_declares_expected_entry_count() {
    let root = fixture_root();
    let source = fs::read_to_string(root.join("media.yaml")).unwrap();
    let media = media_map::from_str(&source).expect("media.yaml parses as media map");
    assert_eq!(media.len(), 546, "media.yaml declares every UG media asset");
}

#[test]
fn ultimate_geography_hoisted_media_map_matches_inline_media_block() {
    let root = fixture_root();
    let deck_path = root.join("deck.yaml");
    let deck_source = fs::read_to_string(&deck_path).unwrap();
    let media_source = fs::read_to_string(root.join("media.yaml")).unwrap();
    let include_line = "media: !include media.yaml\n";
    assert!(
        deck_source.contains(include_line),
        "deck.yaml should keep media as a structural include"
    );

    let inline_media = media_source
        .lines()
        .map(|line| format!("  {line}\n"))
        .collect::<String>();
    let inline_source = deck_source.replace(include_line, &format!("media:\n{inline_media}"));

    let resolved_include =
        source_includes::resolve_file_includes(&deck_source, &deck_path, &root, &[])
            .expect("hoisted deck media include resolves");
    let resolved_inline =
        source_includes::resolve_file_includes(&inline_source, &deck_path, &root, &[])
            .expect("textually re-inlined deck resolves");
    let hoisted = canonical_yaml::from_str(&resolved_include).expect("hoisted deck parses");
    let inlined = canonical_yaml::from_str(&resolved_inline).expect("re-inlined deck parses");
    assert_eq!(
        hoisted, inlined,
        "hoisted media map should be equivalent to the original inline media block"
    );
}

#[test]
fn ultimate_geography_fixture_yaml_sources_are_checked_in_canonical() {
    let root = fixture_root();
    let mut yaml_files = Vec::new();
    collect_yaml_files(&root, &mut yaml_files);
    yaml_files.sort();

    for path in yaml_files {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{} is readable: {error}", path.display()));
        let formatted = if path.file_name().and_then(|name| name.to_str()) == Some("brainbrew.yaml")
            || path.file_name().and_then(|name| name.to_str()) == Some("brainbrew-hardcore.yaml")
        {
            manifest::format_str(&source)
                .unwrap_or_else(|error| panic!("{} manifest formats: {error}", path.display()))
        } else if path.file_name().and_then(|name| name.to_str()) == Some("media.yaml") {
            media_map::format_str(&source)
                .unwrap_or_else(|error| panic!("{} media map formats: {error}", path.display()))
        } else if path.starts_with(root.join("overlays")) {
            source_includes::format_preserving_file_includes(
                &source,
                canonical_yaml::overlay_format_str,
            )
            .unwrap_or_else(|error| panic!("{} overlay formats: {error}", path.display()))
        } else {
            source_includes::format_preserving_file_includes(&source, canonical_yaml::format_str)
                .unwrap_or_else(|error| panic!("{} deck formats: {error}", path.display()))
        };
        assert_eq!(
            formatted,
            source,
            "{} is not canonical; run brainbrew fmt on the fixture source",
            path.strip_prefix(&root).unwrap_or(&path).display()
        );
    }
}

#[test]
fn ultimate_geography_hardcore_companion_manifest_composes_all_targets() {
    let root = fixture_root();
    let manifest = read_manifest_file(&root, "brainbrew-hardcore.yaml");

    let package = manifest
        .package
        .as_ref()
        .expect("hardcore fixture has package metadata");
    assert_eq!(package.id, "anki-geo.hardcore-geography");
    assert_eq!(manifest.base, "deck-hardcore.yaml");
    assert_eq!(manifest.targets.len(), 26);
    assert_eq!(manifest.languages.len(), 13);

    for target in manifest.targets.keys() {
        let deck = compose_target(&root, &manifest, target);
        deck.validate()
            .unwrap_or_else(|error| panic!("{target} validates: {error}"));
        media::validate_references(&deck)
            .unwrap_or_else(|error| panic!("{target} media references validate: {error}"));
        assert_crowdanki_round_trip_profile(&deck, target);
    }

    let english = compose_target(&root, &manifest, "en-hardcore-companion-standard");
    assert!(english.notes.contains_key(&sid("note.pitcairn-islands")));
    assert_eq!(
        english.name, "Hardcore Geography",
        "the standalone hardcore manifest uses its own deck source"
    );
}

#[test]
fn ultimate_geography_hardcore_extension_builds_on_main_deck_without_erasing_base_content() {
    let root = fixture_root();
    let manifest = read_manifest(&root);

    let english = compose_target(&root, &manifest, "en-hardcore-standard");
    assert_eq!(english.notes.len(), 364);
    assert!(english.notes.contains_key(&sid("note.pitcairn-islands")));
    assert_eq!(
        english.notes[&sid("note.hardcore-anguilla")].fields[&sid("field.capital")],
        "The Valley"
    );
    let anguilla_map_images = english.notes[&sid("note.anguilla")].fields[&sid("field.map")]
        .as_images()
        .unwrap();
    assert_eq!(
        anguilla_map_images
            .iter()
            .map(|image| image.media_id.clone())
            .collect::<Vec<_>>(),
        vec![sid("media.ug-map-anguilla-png")],
        "Hardcore companion notes no longer overwrite the main deck's existing structured map card"
    );
    assert!(
        english.notes[&sid("note.hardcore-anguilla")]
            .tags
            .contains("UG::Overlapping")
    );

    let german = compose_target(&root, &manifest, "de-hardcore-standard");
    assert_eq!(
        german.notes[&sid("note.pitcairn-islands")].fields[&sid("field.country")],
        "Pitcairninseln"
    );
    assert_eq!(
        german.notes[&sid("note.hardcore-canary-islands")].fields[&sid("field.capital")],
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
        let overlay_path = root.join(&overlay_ref.file);
        let overlay_source = fs::read_to_string(&overlay_path)
            .unwrap_or_else(|error| panic!("{} reads: {error}", overlay_ref.file));
        assert!(
            !overlay_source.contains("\n  changes:\n")
                && !overlay_source.contains("\n  additions:\n")
                && !overlay_source.contains("\n  path_overrides:\n"),
            "{} uses direct/contextual/target_adaptations rather than old names",
            overlay_ref.file
        );
        let overlay = read_overlay_file(&root, &overlay_path)
            .unwrap_or_else(|error| panic!("{} parses: {error}", overlay_ref.file));
        let translations = overlay
            .translations
            .as_ref()
            .unwrap_or_else(|| panic!("{} uses translation dictionary", overlay_ref.file));
        assert!(
            !translations.direct.is_empty()
                || !translations.contextual.is_empty()
                || !translations.target_adaptations.is_empty()
                || !translations.variables.is_empty()
                || !translations.adapter_ids.is_empty(),
            "{} has translation dictionary entries",
            overlay_ref.file
        );
        assert!(
            overlay.note_changes.is_empty(),
            "{} uses dictionary direct/contextual/target_adaptations instead of per-note field replacements",
            overlay_ref.file
        );
        assert!(
            !translations
                .contextual
                .contains_key("note_types.note-type.ultimate-geography.name"),
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
                translations.target_adaptations.is_empty(),
                "{} uses field_fills for extension-owned blank field content instead of target_adaptations",
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
        let overlay_path = root.join(&overlay_ref.file);
        let overlay = read_overlay_file(&root, &overlay_path)
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
        let overlay_path = root.join(&overlay_ref.file);
        let overlay = read_overlay_file(&root, &overlay_path)
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

        let old_deck = import_approved(&old.to_string())
            .unwrap_or_else(|error| panic!("{target} release oracle imports: {error}"));
        let new_deck = import_approved(&new.to_string())
            .unwrap_or_else(|error| panic!("{target} generated export imports: {error}"));
        let old_projection = crowdanki::project_deck_for_crowdanki_round_trip(&old_deck)
            .unwrap_or_else(|error| panic!("{target} release oracle projects: {error}"));
        let new_projection = crowdanki::project_deck_for_crowdanki_round_trip(&new_deck)
            .unwrap_or_else(|error| panic!("{target} generated export projects: {error}"));
        let diff = old_projection.semantic_diff(&new_projection);
        assert!(
            diff.is_empty(),
            "{target} {} mismatch: {diff:#?}",
            crowdanki::CROWDANKI_ROUND_TRIP_PROFILE.name
        );
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
    let root = fixture_root();
    let manifest = read_manifest(&root);
    for target in manifest.targets.keys() {
        let deck = compose_target(&root, &manifest, target);
        let note_type = ug_note_type(&deck);
        let field_id = sid("field.capital");
        let mut note_type_change = empty_note_type_change();
        note_type_change.fields.insert(
            field_id.clone(),
            FieldDefinitionChange {
                intent: ChangeIntent::Override,
                field: Some(FieldDefinition {
                    id: field_id.clone(),
                    name: format!("Regression Capital Field {target}"),
                }),
                expected_base: Some(ExpectedBase::EntityFingerprint(
                    fingerprint_field_definition(
                        note_type
                            .fields
                            .iter()
                            .find(|field| field.id == field_id)
                            .unwrap(),
                    ),
                )),
            },
        );
        let mut overlay = empty_overlay(&format!("overlay.regression.field-name.{target}"));
        overlay
            .note_type_changes
            .insert(note_type.id.clone(), note_type_change);
        let report = deck
            .compose(&[overlay])
            .expect_err("renaming an Anki field without its template references fails safely");
        assert!(
            report
                .errors
                .iter()
                .flat_map(|error| &error.validation_errors)
                .any(|error| {
                    error.kind
                        == brain_brew_formats::core::ValidationErrorKind::UnknownTemplateField
                }),
            "{target} reports its stale Anki field reference: {report}"
        );
    }

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
                value: Some((finland_value.clone()).into()),
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
                value: Some((new_value.clone()).into()),
                expected_base: Some(ExpectedBase::FieldValue(current_value)),
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
                value: Some(("${regression.country}".to_owned()).into()),
                expected_base: Some(ExpectedBase::FieldValue(current_value)),
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
            .find(|(note_id, _)| {
                deck.tombstones
                    .blocking(&brain_brew_core::TombstoneAddress::Note {
                        note_id: (*note_id).clone(),
                    })
                    .is_none()
            })
            .expect("UG fixture has at least one exported note");
        let note_index = baseline_json["notes"].as_array().unwrap().len() - 1;
        assert_eq!(
            baseline_json["notes"][note_index]["guid"].as_str(),
            note.adapter_ids.get("crowdanki:guid"),
            "test removes the final exported note so the exact CrowdAnki diff is one missing array item"
        );
        let mut note_change = empty_note_change();
        note_change.intent = ChangeIntent::Remove;
        note_change.expected_base = Some(ExpectedBase::EntityFingerprint(fingerprint_note(note)));
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
            fields: fields.into(),
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
fn ug_regression_translation_overlay_changes_flow_to_resolved_and_crowdanki_output() {
    let root = fixture_root();
    let manifest = read_manifest(&root);
    let target = "en-standard";
    let baseline_deck = compose_target(&root, &manifest, target);
    let baseline_json = exported_json(&baseline_deck);
    let note_id = sid("note.finland");
    let field_id = sid("field.capital");
    let current_value = baseline_deck.notes[&note_id].fields[&field_id]
        .as_scalar()
        .unwrap()
        .to_owned();
    let translated_value = "Regression translated capital".to_owned();
    let expected_export_path =
        note_field_json_path(&baseline_deck, "note.finland", "field.capital");

    let mut overlay = empty_overlay("overlay.regression.translation.en-standard");
    overlay.kind = OverlayKind::Translation;
    overlay.translations = Some(TranslationDictionary {
        direct: BTreeMap::new(),
        contextual: BTreeMap::from([(
            "notes.note.finland".to_owned(),
            BTreeMap::from([(current_value, translated_value.clone())]),
        )]),
        no_change: BTreeSet::new(),
        target_adaptations: BTreeMap::new(),
        stale_translations: Vec::new(),
        variables: BTreeMap::new(),
        adapter_ids: BTreeMap::new(),
        require_complete: false,
        ignore_paths: BTreeSet::new(),
    });

    let changed_deck = compose_target_with_extra_overlay(&root, &manifest, target, Some(overlay));
    let resolved_diff = baseline_deck.semantic_diff(&changed_deck);
    assert_eq!(resolved_diff.changes.len(), 1);
    assert_eq!(
        resolved_diff.changes[0].path,
        "notes.note.finland.fields.field.capital"
    );
    assert_eq!(resolved_diff.changes[0].before.as_deref(), Some("Helsinki"));
    assert_eq!(
        resolved_diff.changes[0].after.as_deref(),
        Some(translated_value.as_str())
    );

    let changed_json = exported_json(&changed_deck);
    assert_eq!(
        json_diff_paths(&baseline_json, &changed_json),
        BTreeSet::from([expected_export_path.clone()]),
        "translation overlay should change exactly one CrowdAnki JSON path"
    );
    assert_eq!(
        changed_json.pointer(&expected_export_path),
        Some(&serde_json::json!(translated_value))
    );
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
                expected_base: Some(ExpectedBase::EntityFingerprint(
                    fingerprint_media_reference(media),
                )),
            },
        );
        let mut expected = vec![expected_json(
            &format!("/media_files/{media_index}"),
            new_path.clone(),
        )];
        for path in json_string_paths_containing(baseline_json, &media.path) {
            if !path.starts_with("/notes/") {
                continue;
            }
            let original = baseline_json
                .pointer(&path)
                .and_then(serde_json::Value::as_str)
                .expect("path came from a JSON string value");
            expected.push(expected_json(
                &path,
                original.replace(&media.path, &new_path),
            ));
        }
        MutationExpectation::new(overlay, expected)
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
    let base_path = root.join(&expanded.base);
    let base = read_canonical_deck_file(root, &base_path)
        .unwrap_or_else(|error| panic!("{target} base parses: {error}"));
    let mut overlays = expanded
        .overlays
        .iter()
        .map(|overlay| {
            let overlay_path = root.join(&overlay.file);
            read_overlay_file(root, &overlay_path)
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

fn assert_crowdanki_round_trip_profile(deck: &CanonicalDeck, target: &str) {
    let export = crowdanki::export_deck(deck)
        .unwrap_or_else(|error| panic!("{target} exports for round-trip parity: {error}"));
    let imported = match import_approved(&export.deck_json) {
        Ok(imported) => imported,
        Err(error)
            if error
                .to_string()
                .contains("both derive suggested stable ID") =>
        {
            assert!(
                crowdanki::CROWDANKI_ROUND_TRIP_PROFILE.losses.contains(
                    &crowdanki::CrowdAnkiRoundTripLoss::StableIdsAreRegeneratedFromAdapterContent
                ),
                "{target} collision must be an explicitly declared profile loss"
            );
            eprintln!(
                "{target}: explicit {} stable-ID loss: {error}",
                crowdanki::CROWDANKI_ROUND_TRIP_PROFILE.name
            );
            return;
        }
        Err(error) => panic!("{target} export re-imports: {error}"),
    };
    let expected = crowdanki::project_deck_for_crowdanki_round_trip(deck)
        .unwrap_or_else(|error| panic!("{target} source projects: {error}"));
    let actual = crowdanki::project_deck_for_crowdanki_round_trip(&imported)
        .unwrap_or_else(|error| panic!("{target} import projects: {error}"));
    let diff = expected.semantic_diff(&actual);
    assert!(
        diff.is_empty(),
        "{target} {} mismatch: {diff:#?}",
        crowdanki::CROWDANKI_ROUND_TRIP_PROFILE.name
    );
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

fn json_string_paths_containing(value: &serde_json::Value, needle: &str) -> Vec<String> {
    let mut paths = Vec::new();
    collect_json_string_paths_containing(value, needle, "", &mut paths);
    paths
}

fn collect_json_string_paths_containing(
    value: &serde_json::Value,
    needle: &str,
    path: &str,
    paths: &mut Vec<String>,
) {
    match value {
        serde_json::Value::String(text) if text.contains(needle) => {
            paths.push(if path.is_empty() {
                "/".to_owned()
            } else {
                path.to_owned()
            });
        }
        serde_json::Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                collect_json_string_paths_containing(
                    item,
                    needle,
                    &format!("{path}/{index}"),
                    paths,
                );
            }
        }
        serde_json::Value::Object(map) => {
            for (key, item) in map {
                collect_json_string_paths_containing(
                    item,
                    needle,
                    &format!("{path}/{}", json_pointer_token(key)),
                    paths,
                );
            }
        }
        _ => {}
    }
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
    read_manifest_file(root, "brainbrew.yaml")
}

fn read_manifest_file(root: &Path, manifest_file: &str) -> manifest::FederatedDeckManifest {
    manifest::from_str(&fs::read_to_string(root.join(manifest_file)).unwrap())
        .unwrap_or_else(|error| panic!("{manifest_file} parses: {error}"))
}

fn read_canonical_deck_file(root: &Path, path: &Path) -> Result<CanonicalDeck, String> {
    let source = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let resolved = source_includes::resolve_file_includes(&source, path, root, &[])
        .map_err(|error| error.to_string())?;
    canonical_yaml::from_str(&resolved).map_err(|error| error.to_string())
}

fn read_overlay_file(root: &Path, path: &Path) -> Result<Overlay, String> {
    let source = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let resolved = source_includes::resolve_file_includes(&source, path, root, &[])
        .map_err(|error| error.to_string())?;
    canonical_yaml::overlay_from_str(&resolved).map_err(|error| error.to_string())
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

fn sid(value: &str) -> StableId {
    StableId::new(value).expect("test stable id is valid")
}

fn collect_yaml_files(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in
        fs::read_dir(dir).unwrap_or_else(|error| panic!("{} is readable: {error}", dir.display()))
    {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_yaml_files(&path, files);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("yaml") {
            files.push(path);
        }
    }
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
