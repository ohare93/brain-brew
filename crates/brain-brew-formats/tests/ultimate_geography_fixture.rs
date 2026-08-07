use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use brain_brew_formats::canonical_source_document::CanonicalSourceDocument;
use brain_brew_formats::core::{
    AdapterIdChange, AdapterIds, CanonicalDeck, CardTemplateChange, ChangeIntent, DeckChange,
    ExpectedBase, FieldChange, FieldDefinition, FieldDefinitionChange, MediaChange, MediaReference,
    Note, NoteChange, NoteTypeChange, Overlay, OverlayKind, PropertyChange, StableId, TagChange,
    TranslationDictionary, fingerprint_field_definition, fingerprint_media_reference,
    fingerprint_note,
};
use brain_brew_formats::source_document::{SourceFile, SourceProvenance};
use brain_brew_formats::{
    canonical_yaml, crowdanki, lockfile, manifest, media, media_map, note_type_map, source_includes,
};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

fn import_approved(input: &str) -> Result<CanonicalDeck, crowdanki::CrowdAnkiError> {
    let plan = crowdanki::plan_import(input.as_bytes())?;
    crowdanki::apply_import_plan(input.as_bytes(), &plan, true)
}

#[test]
fn ultimate_geography_fixture_uses_shared_structural_and_scalar_includes() {
    let root = fixture_root();
    let deck_source = fs::read_to_string(root.join("deck.yaml")).unwrap();
    let hardcore_source = fs::read_to_string(root.join("deck-hardcore.yaml")).unwrap();
    let note_types_source = fs::read_to_string(root.join("note-types.yaml")).unwrap();
    assert!(
        deck_source.contains("description: !include descriptions/ultimate-geography/en.html"),
        "deck description should live outside deck.yaml"
    );
    for (relative_path, source) in [
        ("deck.yaml", &deck_source),
        ("deck-hardcore.yaml", &hardcore_source),
    ] {
        assert_eq!(
            source
                .lines()
                .filter(|line| line.starts_with("note_types:"))
                .collect::<Vec<_>>(),
            ["note_types: !include note-types.yaml"],
            "{relative_path} should use exactly the shared note-type map"
        );
    }
    assert_eq!(
        note_types_source
            .lines()
            .filter(|line| {
                !line.chars().next().is_some_and(char::is_whitespace) && line.ends_with(':')
            })
            .collect::<Vec<_>>(),
        ["note-type.ultimate-geography:"],
        "the shared include should contain exactly the UG note type"
    );
    assert!(
        note_types_source.contains(
            "question_format: !include templates/ultimate-geography/capital-country/question.html"
        ),
        "standard template HTML should live outside note-types.yaml"
    );
    assert!(
        note_types_source.contains("styling: !include styles/ultimate-geography/card.css"),
        "note type CSS should live outside note-types.yaml"
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
fn hebrew_translation_overlay_sets_rtl_only_on_text_fields() {
    let root = fixture_root();
    let manifest = read_manifest(&root);
    let expected = [
        ("field.country", true),
        ("field.country-info", true),
        ("field.capital", true),
        ("field.capital-info", true),
        ("field.capital-hint", true),
        ("field.flag", false),
        ("field.flag-similarity", true),
        ("field.map", false),
    ];

    for target in manifest
        .targets
        .keys()
        .filter(|target| target.starts_with("he-"))
    {
        let deck = compose_target(&root, &manifest, target);
        let note_type = ug_note_type(&deck);
        for (field_id, rtl) in expected {
            assert_eq!(
                note_type
                    .fields
                    .iter()
                    .find(|field| field.id == sid(field_id))
                    .unwrap()
                    .rtl,
                rtl,
                "{target} {field_id} canonical RTL"
            );
        }

        let exported = exported_json(&deck);
        for (index, (_, rtl)) in expected.into_iter().enumerate() {
            assert_eq!(
                exported["note_models"][0]["flds"][index]["rtl"], rtl,
                "{target} field {index} CrowdAnki RTL"
            );
        }
    }
}

#[test]
fn hebrew_crowdanki_field_rtl_round_trips_exactly() {
    let input = fs::read_to_string(
        fixture_root()
            .with_file_name("ultimate-geography-expected")
            .join("crowdanki/he-standard/deck.json"),
    )
    .unwrap();
    let imported = import_approved(&input).expect("legacy Hebrew CrowdAnki imports");
    let actual = exported_json(&imported);
    let expected = [true, true, true, true, true, false, true, false];

    assert_eq!(
        imported
            .note_types
            .values()
            .next()
            .unwrap()
            .fields
            .iter()
            .map(|field| field.rtl)
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(
        actual["note_models"][0]["flds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|field| field["rtl"].as_bool().unwrap())
            .collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn ultimate_geography_fixture_guards_current_declarative_source_layout() {
    let root = fixture_root();
    let note_types_source = fs::read_to_string(root.join("note-types.yaml")).unwrap();
    assert!(
        note_types_source.contains(
            "field_order:\n    - field.country\n    - field.country-info\n    - field.capital\n    - field.capital-info\n    - field.capital-hint\n    - field.flag\n    - field.flag-similarity\n    - field.map"
        ),
        "the shared note type should preserve declared CrowdAnki field ordering"
    );
    assert!(
        note_types_source.contains(
            "card_template_order:\n    - template.country-capital\n    - template.capital-country\n    - template.flag-country\n    - template.map-country"
        ),
        "the shared note type should preserve declared CrowdAnki card-template ordering"
    );
    assert_eq!(note_types_source.matches("label.capital-hint:").count(), 1);
    assert!(!note_types_source.contains("label.capital-hint.question"));
    assert!(!note_types_source.contains("label.capital-hint.answer"));
    for template in [
        "templates/ultimate-geography/capital-country/question.html",
        "templates/ultimate-geography/capital-country/answer.html",
    ] {
        let source = fs::read_to_string(root.join(template)).unwrap();
        assert!(source.contains("${label.capital-hint}"));
        assert!(!source.contains("${label.capital-hint."));
    }

    let experimental =
        fs::read_to_string(root.join("overlays/variants/experimental.yaml")).unwrap();
    assert_eq!(
        experimental.matches("field.region-code:").count(),
        192,
        "Experimental should declare one field and only its 191 non-empty values"
    );
    assert!(
        !experimental.contains("field.region-code: ''"),
        "blank region-code values should remain implicit"
    );

    let metadata_languages = [
        "cs", "de", "es", "fr", "it", "nb", "nl", "pl", "pt", "ru", "sv", "zh",
    ];
    let main_manifest = fs::read_to_string(root.join("brainbrew.yaml")).unwrap();
    let companion_manifest = fs::read_to_string(root.join("brainbrew-hardcore.yaml")).unwrap();
    for language in metadata_languages {
        let relative = format!("overlays/languages/note-types/{language}.yaml");
        let source = fs::read_to_string(root.join(&relative)).unwrap();
        assert!(source.starts_with(&format!(
            "id: overlay.translation.note-type.{language}\nkind: translation\n"
        )));
        assert!(source.contains("note-type.name:"));
        for manifest_source in [&main_manifest, &companion_manifest] {
            assert!(manifest_source.contains(&format!("file: {relative}")));
        }
    }
    assert!(!main_manifest.contains("companion-note-type-translations"));
    assert!(!companion_manifest.contains("companion-note-type-translations"));

    let mut overlay_files = Vec::new();
    collect_yaml_files(&root.join("overlays"), &mut overlay_files);
    let context =
        "note_types.note-type.ultimate-geography.fields.field.flag-similarity.message_pattern:";
    let mut parameter_context_files = BTreeSet::new();
    let mut positional_country_contexts = Vec::new();
    for path in overlay_files {
        let source = fs::read_to_string(&path).unwrap();
        let relative = path
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let lines = source.lines().collect::<Vec<_>>();
        for (index, line) in lines.iter().enumerate() {
            if line.trim() == context {
                let has_parameters = lines[index + 1..]
                    .iter()
                    .take_while(|candidate| candidate.len() - candidate.trim_start().len() > 4)
                    .any(|candidate| candidate.trim() == "parameters:");
                if has_parameters {
                    parameter_context_files.insert(relative.clone());
                }
            }
            if line.contains(".fields.field.flag-similarity.message.items.")
                && line.trim_end().ends_with(".country:")
            {
                positional_country_contexts.push(format!(
                    "{relative}:{}:{}",
                    index + 1,
                    line.trim()
                ));
            }
        }
    }
    assert_eq!(
        parameter_context_files,
        BTreeSet::from([
            "overlays/extensions/hardcore/translations/zh.yaml".to_owned(),
            "overlays/languages/de.yaml".to_owned(),
            "overlays/languages/pt.yaml".to_owned(),
            "overlays/languages/zh-tw.yaml".to_owned(),
            "overlays/languages/zh.yaml".to_owned(),
        ]),
        "shared pattern-parameter contexts should replace positional country copies"
    );
    assert_eq!(
        positional_country_contexts,
        [
            "overlays/languages/cs.yaml:488:poland.fields.field.flag-similarity.message.items.1.country:"
        ],
        "only the reviewed Monaco/Monako positional exception should remain"
    );

    let main_profile = main_manifest
        .split_once("translation_profile:\n")
        .expect("main manifest has translation profile")
        .1;
    let companion_profile = companion_manifest
        .split_once("translation_profile:\n")
        .expect("companion manifest has translation profile")
        .1;
    assert_eq!(main_profile, companion_profile);
    for path in [
        "'note_types.*.fields.*.message_pattern.item_format'",
        "'note_types.*.fields.*.message_pattern.separator'",
    ] {
        assert_eq!(main_profile.matches(path).count(), 1);
    }
    assert!(!main_profile.contains("structured-message-format"));
    assert!(!main_profile.contains("'notes.*.fields.*.message.format'"));
}

#[test]
fn ultimate_geography_fixture_uses_list_patterns_for_flag_similarity() {
    let root = fixture_root();
    let note_types_source = fs::read_to_string(root.join("note-types.yaml")).unwrap();
    assert!(
        note_types_source.contains(
            "field.flag-similarity:\n      name: Flag similarity\n      message_pattern:"
        ),
        "the shared note type should declare the flag-similarity message pattern"
    );
    for relative_path in ["deck.yaml", "overlays/extensions/hardcore.yaml"] {
        let source = fs::read_to_string(root.join(relative_path)).unwrap();
        assert!(
            source.contains("field.flag-similarity:\n        - country:")
                || source.contains("field.flag-similarity:\n          - country:"),
            "{relative_path} should use direct list-message sequences for ordinary non-empty flag similarities"
        );
        assert!(
            !source.lines().any(|line| line.trim() == "items:"),
            "{relative_path} should not retain the legacy list-message items wrapper"
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
        if let Some(export) = &manifest.targets[target].exports.crowdanki
            && let Some(golden) = &export.golden
        {
            assert!(
                root.join(golden).is_file(),
                "{target} preserves its UG-owned referenced golden"
            );
        }

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
    assert_eq!(media.len(), 548, "media.yaml declares every UG media asset");

    let mut yaml_files = Vec::new();
    collect_yaml_files(&root, &mut yaml_files);
    let structured_image_references = yaml_files
        .iter()
        .map(|path| fs::read_to_string(path).unwrap().matches("!image ").count())
        .sum::<usize>();
    assert_eq!(structured_image_references, 604);
}

#[test]
fn ultimate_geography_media_attribution_inventory_is_exact() {
    let root = fixture_root();
    let supplement_root = root
        .parent()
        .expect("fixture has fixtures parent")
        .join("ultimate-geography-attribution/hardcore-geography");
    let media = read_real_media_assets(&root.join("media"));
    let ug = attribution_filenames(&root.join("sources.csv"), "UG sources.csv");
    let hardcore = attribution_filenames(
        &supplement_root.join("sources.csv"),
        "Hardcore Geography sources.csv",
    );
    let ug_notice = BTreeSet::from([
        "_ug-interactive_map_config.js".to_owned(),
        "_ug-interactive_map_init.js".to_owned(),
        "_ug-jsvectormap.js".to_owned(),
        "_ug-jsvectormap.min.css".to_owned(),
        "_ug-world.js".to_owned(),
    ]);

    assert_eq!(ug.len(), 548);
    assert_eq!(hardcore.len(), 56);
    assert!(
        ug.is_disjoint(&hardcore),
        "no image has ambiguous attribution"
    );
    assert!(ug.is_disjoint(&ug_notice));
    assert!(hardcore.is_disjoint(&ug_notice));
    assert_eq!(
        hardcore
            .iter()
            .filter(|filename| filename.starts_with("ug-flag-"))
            .count(),
        39
    );
    assert_eq!(
        hardcore
            .iter()
            .filter(|filename| filename.starts_with("ug-map-"))
            .count(),
        17
    );

    let image_media = media
        .keys()
        .filter(|filename| filename.ends_with(".png") || filename.ends_with(".svg"))
        .cloned()
        .collect::<BTreeSet<_>>();
    let attributed_images = ug.union(&hardcore).cloned().collect::<BTreeSet<_>>();
    assert_eq!(image_media.len(), 604);
    assert_eq!(attributed_images, image_media);
    let non_image_media = media
        .keys()
        .filter(|filename| !image_media.contains(*filename))
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(non_image_media, ug_notice);
    let all_attributed = attributed_images
        .union(&ug_notice)
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        all_attributed,
        media.keys().cloned().collect::<BTreeSet<_>>(),
        "every used media file has exactly one normalized attribution owner"
    );
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
        } else if path.file_name().and_then(|name| name.to_str()) == Some("note-types.yaml") {
            source_includes::format_preserving_file_includes(&source, note_type_map::format_str)
                .unwrap_or_else(|error| panic!("{} note-type map formats: {error}", path.display()))
        } else if matches!(
            path.file_name().and_then(|name| name.to_str()),
            Some("deck.yaml" | "deck-hardcore.yaml")
        ) {
            let provenance = SourceProvenance::new(path.display().to_string())
                .with_source_root(root.display().to_string());
            CanonicalSourceDocument::parse_with_includes(
                SourceFile::new(provenance, source.clone()),
                |request| {
                    let include_path = root.join(request.target());
                    let text = fs::read_to_string(&include_path)
                        .map_err(|error| format!("{}: {error}", include_path.display()))?;
                    Ok(SourceFile::new(
                        SourceProvenance::new(include_path.display().to_string())
                            .with_source_root(root.display().to_string()),
                        text,
                    ))
                },
            )
            .and_then(|document| document.emit())
            .map(|emission| emission.root().text().to_owned())
            .unwrap_or_else(|error| panic!("{} deck document formats: {error}", path.display()))
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
    assert_eq!(english.notes.len(), 366);
    assert!(english.notes.contains_key(&sid("note.pitcairn-islands")));
    assert_eq!(
        english.notes[&sid("note.strait-of-hormuz")].fields[&sid("field.country")],
        "Strait of Hormuz"
    );
    assert_eq!(note_guid(&english, "note.strait-of-malacca"), "K4>1fnrM;@");
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
fn ultimate_geography_fixture_matches_all_pinned_outputs_and_strict_real_media() {
    const UG_REVISION: &str = "54b32544a84d1746403ac8efaa3af0e2250ad4c0";
    const HARDCORE_REVISION: &str = "09ce7c3ba665eac6b0794d089a4e0bbafbfc0f46";
    const BRAINBREW_REVISION: &str = "68a828350de4bda46af85b5167bca807edd7d733";
    const SOURCE_SHA256: &str = "43645ea828c9295ba3984cf501d94e0ae7d045d9209798ed05cdbf3a12d73bb8";
    const MEDIA_SHA256: &str = "ad8bd371b4837d639d76f3a56a11fd7437d0ca0d31022ff5022fe5d5ce03e761";
    const GOLDENS_SHA256: &str = "6a2ec22f3a937e310e364d50eafb20a5fb73c17c27f6966ef07001c88d49704d";
    const ATTRIBUTION_SHA256: &str =
        "0f1d0c3c7b9d9465a7d0279050f54460ed4a31d5fa2703a140abe3da6e522151";
    const HARDCORE_ATTRIBUTION_SHA256: &str =
        "aada4219077e5fa77756701e537789c22b2baad29a961c456b3406f3e3629b06";
    const ATTRIBUTION_COVERAGE_SHA256: &str =
        "13a1d5c1d04a8eacaae3dd3c1c952483128b8f089779dc622f2014055b72351d";
    const GENERATOR_EXECUTABLE_SHA256: &str =
        "0a4963db7bf3e2e8ae019902e5aa98fabd165ba93687811db5ed7cbdd064421f";
    const GENERATOR_SOURCE_SHA256: &str =
        "53b7c7a31848035861115972881dfbd70e04ab27ddae11be88b945c7cabe7a27";
    const GENERATOR_IDENTITY_SHA256: &str =
        "f377b4d27dac34df9b09046cb139f00e1efba570f2592b9b635aa9694963ce9e";
    const EXPECTED_SHA256: &str =
        "267a3459fcc94bcff126c5c496b2c31dac3e72bc85fa7483b3beac2d2f152d2d";

    let root = fixture_root();
    let lock_path = root.with_file_name("ultimate-geography.lock.json");
    let lock: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&lock_path).expect("fixture lock reads"))
            .expect("fixture lock is JSON");
    assert_eq!(lock["schema_version"], 3);
    assert_eq!(
        lock["provenance"]["ultimate_geography"]["revision"],
        UG_REVISION
    );
    assert_eq!(
        lock["provenance"]["hardcore_geography"]["revision"],
        HARDCORE_REVISION
    );
    assert_eq!(
        lock["provenance"]["brain_brew"]["revision"],
        BRAINBREW_REVISION
    );
    assert_eq!(lock["provenance"]["brain_brew"]["version"], "1.0.0-alpha.3");

    let required_source_entries = BTreeSet::from([
        "LICENSE.md".to_owned(),
        "brainbrew-hardcore.yaml".to_owned(),
        "brainbrew.yaml".to_owned(),
        "deck-hardcore.yaml".to_owned(),
        "deck.yaml".to_owned(),
        "descriptions".to_owned(),
        "goldens".to_owned(),
        "media".to_owned(),
        "media.yaml".to_owned(),
        "note-types.yaml".to_owned(),
        "overlays".to_owned(),
        "sources.csv".to_owned(),
        "styles".to_owned(),
        "templates".to_owned(),
    ]);
    let actual_source_entries = fs::read_dir(&root)
        .expect("fixture source root reads")
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_source_entries, required_source_entries);

    let source_metadata = tree_metadata(&root);
    assert_eq!(source_metadata.file_count, 739);
    assert_eq!(source_metadata.byte_count, 20_033_438);
    assert_eq!(source_metadata.sha256, SOURCE_SHA256);
    assert_tree_lock(&lock["source"], &source_metadata, "source snapshot");
    assert_eq!(lock["source"]["sha256"], SOURCE_SHA256);

    let media_root = root.join("media");
    let media_metadata = tree_metadata(&media_root);
    assert_eq!(media_metadata.file_count, 609);
    assert_eq!(media_metadata.byte_count, 17_579_868);
    assert_eq!(media_metadata.sha256, MEDIA_SHA256);
    assert_tree_lock(&lock["source"]["media"], &media_metadata, "media snapshot");

    let goldens_metadata = tree_metadata(&root.join("goldens"));
    assert_eq!(goldens_metadata.file_count, 8);
    assert_eq!(goldens_metadata.byte_count, 1_321_860);
    assert_eq!(goldens_metadata.sha256, GOLDENS_SHA256);
    assert_tree_lock(
        &lock["source"]["goldens"],
        &goldens_metadata,
        "UG goldens snapshot",
    );

    let attribution = &lock["source"]["third_party_attribution"];
    assert_eq!(attribution["algorithm"], "sha256-path-length-content-v1");
    assert_eq!(attribution["file_count"], 2);
    assert_eq!(attribution["byte_count"], 58_618);
    assert_eq!(attribution["sha256"], ATTRIBUTION_SHA256);
    assert_eq!(
        attribution["paths"],
        serde_json::json!(["LICENSE.md", "sources.csv"])
    );
    assert_eq!(
        sha256_file(&root.join("LICENSE.md")),
        "e69b8954905c6a3ac9c104e1ea9d320da7deb1d867679d314dd61c32cf63df56"
    );
    assert_eq!(
        sha256_file(&root.join("sources.csv")),
        "ea4c77d1af88fc01e18de3d751235610a3af7f3c6916e2c0f09229b8597baabc"
    );

    let supplement_root = root
        .parent()
        .expect("fixture has fixtures parent")
        .join("ultimate-geography-attribution/hardcore-geography");
    let supplement_metadata = tree_metadata(&supplement_root);
    assert_eq!(supplement_metadata.file_count, 2);
    assert_eq!(supplement_metadata.byte_count, 6_126);
    assert_eq!(supplement_metadata.sha256, HARDCORE_ATTRIBUTION_SHA256);
    let supplement = &lock["attribution"]["supplements"]["hardcore_geography"];
    assert_tree_lock(
        supplement,
        &supplement_metadata,
        "Hardcore attribution supplement",
    );
    assert_eq!(
        supplement["root"],
        "ultimate-geography-attribution/hardcore-geography"
    );
    assert_eq!(supplement["provenance"]["revision"], HARDCORE_REVISION);
    assert_eq!(
        supplement["paths"],
        serde_json::json!(["README.md", "sources.csv"])
    );
    assert_eq!(
        sha256_file(&supplement_root.join("README.md")),
        "ea7da97156e5688e36b8c32eaaf2f5dd805620edf5c3face9ff4d7508fdb7e07"
    );
    assert_eq!(
        sha256_file(&supplement_root.join("sources.csv")),
        "aa3d9a96a0ae9dd15f6e108891b9a667c70ad9a97f64036045c13a5f4ecf204c"
    );
    let coverage = &lock["attribution"]["coverage"];
    assert_eq!(coverage["algorithm"], "sha256-normalized-filename-owner-v1");
    assert_eq!(
        coverage["filename_normalization"],
        "unicode-nfc-posix-basename-v1"
    );
    assert_eq!(coverage["media_file_count"], 609);
    assert_eq!(coverage["image_file_count"], 604);
    assert_eq!(
        coverage["ultimate_geography"]["sources_csv_file_count"],
        548
    );
    assert_eq!(
        coverage["ultimate_geography"]["license_notice_file_count"],
        5
    );
    assert_eq!(coverage["hardcore_geography"]["sources_csv_file_count"], 56);
    assert_eq!(coverage["hardcore_geography"]["flag_file_count"], 39);
    assert_eq!(coverage["hardcore_geography"]["map_file_count"], 17);
    assert_eq!(coverage["unattributed_file_count"], 0);
    assert_eq!(coverage["ambiguous_file_count"], 0);
    assert_eq!(coverage["sha256"], ATTRIBUTION_COVERAGE_SHA256);

    let expected_root = root
        .parent()
        .expect("fixture has fixtures parent")
        .join("ultimate-geography-expected/crowdanki");
    let manifest_contracts = [
        ("brainbrew.yaml", "main", 74_usize),
        ("brainbrew-hardcore.yaml", "companion", 26_usize),
    ];
    let source_records = lock["source"]["manifests"]
        .as_array()
        .expect("source manifests are an array");
    let expected_records = lock["expected"]["manifests"]
        .as_array()
        .expect("expected manifests are an array");
    assert_eq!(source_records, expected_records);
    assert_eq!(
        lock["expected"]["accepted_from_source_sha256"],
        SOURCE_SHA256
    );
    let generator = &lock["expected"]["generated_by"];
    assert_eq!(generator["revision"], BRAINBREW_REVISION);
    assert_eq!(
        generator["executable"]["sha256"],
        GENERATOR_EXECUTABLE_SHA256
    );
    assert_eq!(generator["executable"]["byte_count"], 15_777_528);
    assert_eq!(generator["source"]["sha256"], GENERATOR_SOURCE_SHA256);
    assert_eq!(generator["source"]["file_count"], 69);
    assert_eq!(generator["source"]["byte_count"], 2_985_380);
    assert_eq!(
        generator["build"]["cargo_lock_sha256"],
        "ea2858def2a0528b781d992930a8f6067e71b4baa8ef6bf6b298f3b44a120cd1"
    );
    assert_eq!(generator["identity"]["sha256"], GENERATOR_IDENTITY_SHA256);
    assert_eq!(lock["expected"]["file_count"], 100);
    assert_eq!(lock["expected"]["sha256"], EXPECTED_SHA256);

    let assets = read_real_media_assets(&media_root);
    assert_eq!(assets.len(), 609);
    let mut all_expected_targets = BTreeSet::new();
    let mut all_declared_media = BTreeSet::new();
    let mut compared = 0_usize;
    for (record_index, (manifest_file, role, target_count)) in
        manifest_contracts.into_iter().enumerate()
    {
        let manifest = read_manifest_file(&root, manifest_file);
        assert_eq!(manifest.targets.len(), target_count);
        let record = &source_records[record_index];
        assert_eq!(record["path"], manifest_file);
        assert_eq!(record["role"], role);
        assert_eq!(record["target_count"], target_count);
        let recorded_targets = record["targets"]
            .as_array()
            .expect("recorded targets are an array")
            .iter()
            .map(|target| target.as_str().unwrap().to_owned())
            .collect::<BTreeSet<_>>();
        let manifest_targets = manifest.targets.keys().cloned().collect::<BTreeSet<_>>();
        assert_eq!(recorded_targets, manifest_targets);

        for target in manifest.targets.keys() {
            assert!(
                all_expected_targets.insert(target.clone()),
                "target {target} occurs in only one manifest"
            );
            let expected_path = expected_root.join(target).join("deck.json");
            assert!(
                expected_path.is_file(),
                "{target} expected deck.json exists"
            );
            let expected: serde_json::Value = serde_json::from_str(
                &fs::read_to_string(&expected_path)
                    .unwrap_or_else(|error| panic!("{} reads: {error}", expected_path.display())),
            )
            .unwrap_or_else(|error| panic!("{} parses: {error}", expected_path.display()));

            let deck = compose_target(&root, &manifest, target);
            media::validate_declarations(&deck, media::MediaHashPolicy::Required)
                .unwrap_or_else(|error| panic!("{target} has strict media declarations: {error}"));
            media::validate_references(&deck)
                .unwrap_or_else(|error| panic!("{target} media references validate: {error}"));
            media::validate_hashes(&deck, &assets)
                .unwrap_or_else(|error| panic!("{target} real media bytes validate: {error}"));
            all_declared_media.extend(deck.media.values().map(|entry| entry.path.clone()));

            let actual = exported_json(&deck);
            assert_eq!(
                actual, expected,
                "{target} parsed CrowdAnki output drifted from its mandatory expected deck.json"
            );
            compared += 1;
        }
    }
    assert_eq!(compared, 100);
    assert_eq!(all_expected_targets.len(), 100);
    assert_eq!(
        all_declared_media,
        assets.keys().cloned().collect::<BTreeSet<_>>(),
        "the all-target strict verification covers every byte in the single vendored media tree"
    );
    assert_exact_expected_tree(&expected_root, &all_expected_targets);

    let expected_metadata = canonical_json_tree_metadata(&expected_root);
    assert_eq!(expected_metadata.file_count, 100);
    assert_eq!(expected_metadata.canonical_byte_count, 10_024_776);
    assert_eq!(expected_metadata.sha256, EXPECTED_SHA256);
    assert_eq!(
        lock["expected"]["algorithm"],
        "sha256-path-canonical-json-v1"
    );
    assert_eq!(
        lock["expected"]["canonical_byte_count"],
        expected_metadata.canonical_byte_count
    );
    assert_eq!(lock["expected"]["sha256"], expected_metadata.sha256);
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
    let cache = &*UG_REGRESSION_CACHE;
    for (target, baseline) in &cache.targets {
        let deck = &baseline.deck;
        let note_type = ug_note_type(deck);
        let field_id = sid("field.capital");
        let mut note_type_change = empty_note_type_change();
        note_type_change.fields.insert(
            field_id.clone(),
            FieldDefinitionChange {
                intent: ChangeIntent::Override,
                field: Some(FieldDefinition {
                    id: field_id.clone(),
                    name: format!("Regression Capital Field {target}"),
                    rtl: false,
                    message_pattern: None,
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
        let report = cache
            .compose_with_extra(target, overlay)
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
                    rtl: false,
                    message_pattern: None,
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

static UG_REGRESSION_CACHE: LazyLock<UgRegressionCache> = LazyLock::new(UgRegressionCache::load);

struct UgRegressionTarget {
    overlay_ids: Vec<String>,
    deck: CanonicalDeck,
    json: serde_json::Value,
}

struct UgRegressionCache {
    base: CanonicalDeck,
    overlays: BTreeMap<String, Overlay>,
    targets: BTreeMap<String, UgRegressionTarget>,
}

impl UgRegressionCache {
    fn load() -> Self {
        let root = fixture_root();
        let manifest = read_manifest(&root);
        let base_path = root.join(&manifest.base);
        let base = read_canonical_deck_file(&root, &base_path)
            .unwrap_or_else(|error| panic!("{} parses: {error}", manifest.base));
        let plans = manifest
            .targets
            .keys()
            .map(|target| {
                let overlay_ids = manifest
                    .expand_target(target)
                    .unwrap_or_else(|error| panic!("{target} expands: {error}"))
                    .overlays
                    .into_iter()
                    .map(|overlay| overlay.id)
                    .collect::<Vec<_>>();
                (target.clone(), overlay_ids)
            })
            .collect::<BTreeMap<_, _>>();
        let used_overlay_ids = plans.values().flatten().cloned().collect::<BTreeSet<_>>();
        let overlays = used_overlay_ids
            .into_iter()
            .map(|id| {
                let entry = manifest
                    .overlays
                    .get(&id)
                    .unwrap_or_else(|| panic!("expanded overlay {id} exists in manifest"));
                let path = root.join(&entry.file);
                let overlay = read_overlay_file(&root, &path)
                    .unwrap_or_else(|error| panic!("{} parses: {error}", entry.file));
                (id, overlay)
            })
            .collect::<BTreeMap<_, _>>();
        let targets = plans
            .into_iter()
            .map(|(target, overlay_ids)| {
                let stack = overlay_ids
                    .iter()
                    .map(|id| overlays.get(id).unwrap().clone())
                    .collect::<Vec<_>>();
                let deck = base
                    .compose(&stack)
                    .unwrap_or_else(|error| panic!("{target} composes: {error}"));
                let json = exported_json(&deck);
                (
                    target,
                    UgRegressionTarget {
                        overlay_ids,
                        deck,
                        json,
                    },
                )
            })
            .collect();
        Self {
            base,
            overlays,
            targets,
        }
    }

    fn compose_with_extra(
        &self,
        target: &str,
        extra: Overlay,
    ) -> Result<CanonicalDeck, brain_brew_formats::core::ComposeReport> {
        let target = self
            .targets
            .get(target)
            .unwrap_or_else(|| panic!("cached target {target} exists"));
        let mut stack = target
            .overlay_ids
            .iter()
            .map(|id| self.overlays.get(id).unwrap().clone())
            .collect::<Vec<_>>();
        stack.push(extra);
        self.base.compose(&stack)
    }
}

fn assert_all_targets_export_exact_diffs(
    case_name: &str,
    build: impl Fn(&str, &CanonicalDeck, &serde_json::Value) -> MutationExpectation,
) {
    let cache = &*UG_REGRESSION_CACHE;
    for (target, baseline) in &cache.targets {
        let expectation = build(target, &baseline.deck, &baseline.json);
        assert!(
            !expectation.expected.is_empty(),
            "{case_name} for {target} must expect at least one CrowdAnki difference"
        );

        let changed_deck = cache
            .compose_with_extra(target, expectation.overlay)
            .unwrap_or_else(|error| panic!("{case_name} for {target} composes: {error}"));
        let changed_json = exported_json(&changed_deck);
        let actual_paths = json_diff_paths(&baseline.json, &changed_json);
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

#[derive(Debug, Eq, PartialEq)]
struct TreeMetadata {
    file_count: usize,
    byte_count: u64,
    sha256: String,
}

#[derive(Debug, Eq, PartialEq)]
struct JsonTreeMetadata {
    file_count: usize,
    canonical_byte_count: usize,
    sha256: String,
}

fn tree_metadata(root: &Path) -> TreeMetadata {
    let mut files = Vec::new();
    collect_regular_files(root, root, &mut files);
    files.sort_by(|left, right| left.0.cmp(&right.0));
    let mut hasher = Sha256::new();
    hasher.update(b"brainbrew-tree-sha256-v1\0");
    let mut byte_count = 0_u64;
    for (relative, path) in &files {
        let bytes =
            fs::read(path).unwrap_or_else(|error| panic!("{} reads: {error}", path.display()));
        update_framed_digest(&mut hasher, relative, &bytes);
        byte_count += bytes.len() as u64;
    }
    TreeMetadata {
        file_count: files.len(),
        byte_count,
        sha256: format!("{:x}", hasher.finalize()),
    }
}

fn sha256_file(path: &Path) -> String {
    let bytes = fs::read(path).unwrap_or_else(|error| panic!("{} reads: {error}", path.display()));
    format!("{:x}", Sha256::digest(bytes))
}

fn canonical_json_tree_metadata(root: &Path) -> JsonTreeMetadata {
    let mut targets = fs::read_dir(root)
        .unwrap_or_else(|error| panic!("{} reads: {error}", root.display()))
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    targets.sort();
    let mut hasher = Sha256::new();
    hasher.update(b"brainbrew-json-tree-sha256-v1\0");
    let mut canonical_byte_count = 0_usize;
    for target_dir in &targets {
        let target = target_dir.file_name().unwrap().to_str().unwrap();
        let deck_path = target_dir.join("deck.json");
        let value: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&deck_path)
                .unwrap_or_else(|error| panic!("{} reads: {error}", deck_path.display())),
        )
        .unwrap_or_else(|error| panic!("{} parses: {error}", deck_path.display()));
        let canonical = serde_json::to_vec(&value).expect("expected JSON canonicalizes");
        update_framed_digest(&mut hasher, &format!("{target}/deck.json"), &canonical);
        canonical_byte_count += canonical.len();
    }
    JsonTreeMetadata {
        file_count: targets.len(),
        canonical_byte_count,
        sha256: format!("{:x}", hasher.finalize()),
    }
}

fn update_framed_digest(hasher: &mut Sha256, relative: &str, bytes: &[u8]) {
    hasher.update(relative.as_bytes());
    hasher.update(b"\0");
    hasher.update(bytes.len().to_string().as_bytes());
    hasher.update(b"\0");
    hasher.update(bytes);
}

fn collect_regular_files(root: &Path, dir: &Path, files: &mut Vec<(String, PathBuf)>) {
    for entry in
        fs::read_dir(dir).unwrap_or_else(|error| panic!("{} reads: {error}", dir.display()))
    {
        let path = entry.unwrap().path();
        let metadata = fs::symlink_metadata(&path)
            .unwrap_or_else(|error| panic!("{} metadata reads: {error}", path.display()));
        assert!(
            !metadata.file_type().is_symlink(),
            "fixture trees contain no symlinks: {}",
            path.display()
        );
        if metadata.is_dir() {
            collect_regular_files(root, &path, files);
        } else {
            assert!(
                metadata.is_file(),
                "fixture trees contain regular files only: {}",
                path.display()
            );
            files.push((
                path.strip_prefix(root)
                    .unwrap()
                    .to_str()
                    .expect("fixture paths are UTF-8")
                    .replace('\\', "/"),
                path,
            ));
        }
    }
}

fn assert_tree_lock(record: &serde_json::Value, actual: &TreeMetadata, label: &str) {
    assert_eq!(
        record["algorithm"], "sha256-path-length-content-v1",
        "{label} algorithm"
    );
    assert_eq!(
        record["file_count"], actual.file_count,
        "{label} file count"
    );
    assert_eq!(
        record["byte_count"], actual.byte_count,
        "{label} byte count"
    );
    assert_eq!(record["sha256"], actual.sha256, "{label} digest");
}

fn read_real_media_assets(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = Vec::new();
    collect_regular_files(root, root, &mut files);
    files
        .into_iter()
        .map(|(relative, path)| {
            let bytes =
                fs::read(&path).unwrap_or_else(|error| panic!("{} reads: {error}", path.display()));
            (relative, bytes)
        })
        .collect()
}

fn attribution_filenames(path: &Path, label: &str) -> BTreeSet<String> {
    let source = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{} reads: {error}", path.display()));
    let mut lines = source.lines();
    let header = lines
        .next()
        .unwrap_or_else(|| panic!("{label} has a header"));
    assert_eq!(
        parse_fixture_csv_row(header, label, 1),
        ["File", "Source", "License", "Modifications"],
        "{label} columns"
    );
    let mut filenames = BTreeSet::new();
    for (index, line) in lines.enumerate() {
        let row_number = index + 2;
        let fields = parse_fixture_csv_row(line, label, row_number);
        assert_eq!(fields.len(), 4, "{label} row {row_number} has four fields");
        assert!(
            !fields[1].is_empty(),
            "{label} row {row_number} has a source"
        );
        assert!(
            !fields[2].is_empty(),
            "{label} row {row_number} has a license"
        );
        let filename = &fields[0];
        assert_eq!(
            filename.trim(),
            filename,
            "{label} filename whitespace drift"
        );
        assert!(
            !filename.is_empty()
                && !filename.contains('/')
                && !filename.contains('\\')
                && filename != "."
                && filename != "..",
            "{label} row {row_number} File is a basename"
        );
        assert_eq!(
            filename.nfc().collect::<String>(),
            *filename,
            "{label} row {row_number} File is Unicode NFC"
        );
        assert!(
            filenames.insert(filename.clone()),
            "{label} repeats normalized filename {filename:?}"
        );
    }
    filenames
}

fn parse_fixture_csv_row(line: &str, label: &str, row_number: usize) -> Vec<String> {
    let mut fields = vec![String::new()];
    let mut chars = line.chars().peekable();
    let mut quoted = false;
    while let Some(character) = chars.next() {
        match character {
            '"' if quoted && chars.peek() == Some(&'"') => {
                fields.last_mut().unwrap().push('"');
                chars.next();
            }
            '"' if quoted => quoted = false,
            '"' if fields.last().unwrap().is_empty() => quoted = true,
            '"' => panic!("{label} row {row_number} has an unexpected quote"),
            ',' if !quoted => fields.push(String::new()),
            _ => fields.last_mut().unwrap().push(character),
        }
    }
    assert!(
        !quoted,
        "{label} row {row_number} has an unterminated quote"
    );
    fields
}

fn assert_exact_expected_tree(root: &Path, targets: &BTreeSet<String>) {
    let mut actual = BTreeSet::new();
    for entry in
        fs::read_dir(root).unwrap_or_else(|error| panic!("{} reads: {error}", root.display()))
    {
        let target_dir = entry.unwrap().path();
        let target_metadata = fs::symlink_metadata(&target_dir)
            .unwrap_or_else(|error| panic!("{} metadata reads: {error}", target_dir.display()));
        assert!(
            target_metadata.is_dir() && !target_metadata.file_type().is_symlink(),
            "expected root contains real target directories only"
        );
        let target = target_dir
            .file_name()
            .unwrap()
            .to_str()
            .expect("target name is UTF-8")
            .to_owned();
        assert!(actual.insert(target.clone()), "target directory is unique");
        let children = fs::read_dir(&target_dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        assert_eq!(children.len(), 1, "{target} contains one expected file");
        assert_eq!(
            children[0].file_name().and_then(|name| name.to_str()),
            Some("deck.json"),
            "{target} contains parsed deck.json only (never duplicated media)"
        );
        let deck_metadata = fs::symlink_metadata(&children[0])
            .unwrap_or_else(|error| panic!("{} metadata reads: {error}", children[0].display()));
        assert!(
            deck_metadata.is_file() && !deck_metadata.file_type().is_symlink(),
            "{target}/deck.json is a real regular file"
        );
    }
    assert_eq!(
        &actual, targets,
        "missing or extra expected target directory"
    );
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
