use brain_brew_formats::manifest;

#[test]
fn expands_manifest_target_dependencies_in_deterministic_order() {
    let manifest = manifest::from_str(
        r#"
base: deck.yaml
overlays:
  lang.de:
    file: overlays/languages/de.yaml
    kind: translation
  variant.extended.de:
    file: overlays/variants/extended.de.yaml
    kind: extension
    depends_on:
      - lang.de
  patch.capital:
    file: overlays/patches/capital.yaml
    kind: patch
targets:
  de-extended-patched:
    overlays:
      - variant.extended.de
      - patch.capital
"#,
    )
    .expect("manifest parses");

    let target = manifest
        .expand_target("de-extended-patched")
        .expect("target expands");

    assert_eq!(target.base, "deck.yaml");
    assert_eq!(target.extends, None);
    assert_eq!(
        target
            .overlays
            .iter()
            .map(|overlay| overlay.id.as_str())
            .collect::<Vec<_>>(),
        vec!["lang.de", "variant.extended.de", "patch.capital"]
    );
    assert_eq!(
        target
            .overlays
            .iter()
            .map(|overlay| overlay.file.as_str())
            .collect::<Vec<_>>(),
        vec![
            "overlays/languages/de.yaml",
            "overlays/variants/extended.de.yaml",
            "overlays/patches/capital.yaml"
        ]
    );
}

#[test]
fn parses_package_metadata_and_target_export_checks() {
    let manifest = manifest::from_str(
        r#"
package:
  id: anki-geo.ultimate-geography
  version: 0.1.0
  base_package: anki-geo.shared-geography
  compatible_base_versions:
    - '>=0.1,<0.2'
  depends_on:
    - anki-geo.shared-geography@0.1.0
base: deck.yaml
targets:
  en-standard:
    overlays: []
    exports:
      crowdanki:
        out: build/en-standard
        golden: goldens/en-standard/deck.json
        golden_allowlist:
          - $.deck_config_uuid
"#,
    )
    .expect("manifest parses");

    let package = manifest.package.expect("package metadata parsed");
    assert_eq!(package.id, "anki-geo.ultimate-geography");
    assert_eq!(package.version, "0.1.0");
    assert_eq!(
        package.base_package.as_deref(),
        Some("anki-geo.shared-geography")
    );
    assert_eq!(package.compatible_base_versions, vec![">=0.1, <0.2"]);
    assert_eq!(package.depends_on, vec!["anki-geo.shared-geography@0.1.0"]);

    let export = manifest.targets["en-standard"]
        .exports
        .crowdanki
        .as_ref()
        .expect("crowdanki export config parsed");
    assert_eq!(export.out.as_deref(), Some("build/en-standard"));
    assert_eq!(
        export.golden.as_deref(),
        Some("goldens/en-standard/deck.json")
    );
    assert_eq!(export.golden_allowlist, vec!["$.deck_config_uuid"]);
}

#[test]
fn parses_and_formats_safe_include_roots() {
    let formatted = manifest::format_str(
        r#"
include_roots: [../shared-content]
base: deck.yaml
targets:
  base:
    overlays: []
"#,
    )
    .expect("manifest formats");

    assert_eq!(
        formatted,
        r#"base: deck.yaml
include_roots:
  - ../shared-content
overlays: {}
targets:
  base:
    overlays: []
"#
    );
    let manifest = manifest::from_str(&formatted).expect("manifest parses");
    assert_eq!(manifest.include_roots, vec!["../shared-content"]);
}

#[test]
fn formatter_canonicalizes_manifest_yaml() {
    let formatted = manifest::format_str(
        r#"
targets:
  de-extended:
    overlays: [overlay.variant.extended.de]
overlays:
  overlay.variant.extended.de:
    depends_on: [overlay.translation.de]
    kind: extension
    file: overlays/variants/extended/de.yaml
  overlay.translation.de:
    file: overlays/languages/de.yaml
    kind: translation
base: deck.yaml
"#,
    )
    .expect("manifest formats");

    assert_eq!(
        formatted,
        r#"base: deck.yaml
overlays:
  overlay.translation.de:
    file: overlays/languages/de.yaml
    kind: translation
  overlay.variant.extended.de:
    file: overlays/variants/extended/de.yaml
    kind: extension
    depends_on:
      - overlay.translation.de
targets:
  de-extended:
    overlays:
      - overlay.variant.extended.de
"#
    );
}

#[test]
fn parses_and_formats_language_catalog_and_translation_profile() {
    let formatted = manifest::format_str(
        r#"
translation_profile:
  metadata_category_order: [deck-metadata, note-type-name, field-label]
  metadata_categories:
    - key: field-label
      label: Field labels
      paths: [note_types.*.fields.*.name]
    - key: deck-metadata
      label: Deck metadata
      paths: [deck.name, deck.description]
    - key: note-type-name
      label: Note type names
      paths: [note_types.*.name]
  metadata_exclude_paths: [deck.adapter_ids.*, notes.*.adapter_ids.*]
  metadata_paths: [note_types.*, deck.*, notes.*.tags.*]
  structural_fields: [field.map, field.flag]
languages:
  da:
    targets:
      extended: da-extended
      standard: da-standard
    primary_target: standard
    translation_overlays:
      hardcore: overlay.translation.hardcore.da
      base: overlay.translation.da
    display_name: Danish
  en:
    targets:
      extended: en-extended
      standard: en-standard
    primary_target: standard
    source: true
    display_name: English
base: deck.yaml
overlays:
  overlay.translation.hardcore.da:
    file: overlays/languages/hardcore/da.yaml
    kind: translation
  overlay.translation.da:
    file: overlays/languages/da.yaml
    kind: translation
targets:
  en-standard:
    overlays: []
  en-extended:
    overlays: []
  da-standard:
    overlays: [overlay.translation.da]
  da-extended:
    overlays: [overlay.translation.da, overlay.translation.hardcore.da]
"#,
    )
    .expect("manifest formats");

    assert_eq!(
        formatted,
        r#"base: deck.yaml
overlays:
  overlay.translation.da:
    file: overlays/languages/da.yaml
    kind: translation
  overlay.translation.hardcore.da:
    file: overlays/languages/hardcore/da.yaml
    kind: translation
targets:
  da-extended:
    overlays:
      - overlay.translation.da
      - overlay.translation.hardcore.da
  da-standard:
    overlays:
      - overlay.translation.da
  en-extended:
    overlays: []
  en-standard:
    overlays: []
languages:
  da:
    display_name: Danish
    translation_overlays:
      base: overlay.translation.da
      hardcore: overlay.translation.hardcore.da
    primary_target: standard
    targets:
      extended: da-extended
      standard: da-standard
  en:
    display_name: English
    source: true
    primary_target: standard
    targets:
      extended: en-extended
      standard: en-standard
translation_profile:
  structural_fields:
    - field.map
    - field.flag
  metadata_categories:
    - key: field-label
      label: Field labels
      paths:
        - 'note_types.*.fields.*.name'
    - key: deck-metadata
      label: Deck metadata
      paths:
        - deck.name
        - deck.description
    - key: note-type-name
      label: Note type names
      paths:
        - 'note_types.*.name'
  metadata_paths:
    - 'note_types.*'
    - 'deck.*'
    - 'notes.*.tags.*'
  metadata_exclude_paths:
    - 'deck.adapter_ids.*'
    - 'notes.*.adapter_ids.*'
  metadata_category_order:
    - deck-metadata
    - note-type-name
    - field-label
"#
    );

    let manifest = manifest::from_str(&formatted).expect("manifest parses");
    let english = &manifest.languages["en"];
    assert_eq!(english.display_name, "English");
    assert!(english.source);
    assert!(english.translation_overlays.is_empty());
    assert_eq!(english.primary_target, "standard");
    assert_eq!(english.targets["extended"], "en-extended");

    let danish = &manifest.languages["da"];
    assert_eq!(danish.display_name, "Danish");
    assert!(!danish.source);
    assert_eq!(
        danish.translation_overlays["base"],
        "overlay.translation.da"
    );
    assert_eq!(
        danish.translation_overlays["hardcore"],
        "overlay.translation.hardcore.da"
    );
    assert_eq!(danish.primary_target, "standard");
    assert_eq!(danish.targets["standard"], "da-standard");

    assert_eq!(
        manifest.translation_profile.structural_fields,
        vec!["field.map", "field.flag"]
    );
    assert_eq!(manifest.translation_profile.metadata_categories.len(), 3);
    assert_eq!(
        manifest.translation_profile.metadata_categories[0].key,
        "field-label"
    );
    assert_eq!(
        manifest.translation_profile.metadata_categories[0].label,
        "Field labels"
    );
    assert_eq!(
        manifest.translation_profile.metadata_categories[0].paths,
        vec!["note_types.*.fields.*.name"]
    );
    assert_eq!(
        manifest.translation_profile.metadata_paths,
        vec!["note_types.*", "deck.*", "notes.*.tags.*"]
    );
    assert_eq!(
        manifest.translation_profile.metadata_exclude_paths,
        vec!["deck.adapter_ids.*", "notes.*.adapter_ids.*"]
    );
    assert_eq!(
        manifest.translation_profile.metadata_category_order,
        vec!["deck-metadata", "note-type-name", "field-label"]
    );
}

#[test]
fn language_catalog_reports_missing_target_references() {
    let error = manifest::from_str(
        r#"
base: deck.yaml
targets: {}
languages:
  da:
    display_name: Danish
    primary_target: standard
    targets:
      standard: da-standard
"#,
    )
    .expect_err("missing language target reference is reported");

    assert_eq!(
        error.to_string(),
        "manifest language \"da\" target \"standard\" references missing build target \"da-standard\""
    );
}

#[test]
fn language_catalog_reports_missing_translation_overlay_references() {
    let error = manifest::from_str(
        r#"
base: deck.yaml
targets:
  da-standard:
    overlays: []
languages:
  da:
    display_name: Danish
    translation_overlays:
      base: overlay.translation.da
    primary_target: standard
    targets:
      standard: da-standard
"#,
    )
    .expect_err("missing language overlay reference is reported");

    assert_eq!(
        error.to_string(),
        "manifest language \"da\" translation overlay \"base\" references missing overlay \"overlay.translation.da\""
    );
}

#[test]
fn language_catalog_reports_non_translation_overlay_kind() {
    let error = manifest::from_str(
        r#"
base: deck.yaml
overlays:
  overlay.extension.hardcore:
    file: overlays/variants/hardcore.yaml
    kind: extension
targets:
  da-standard:
    overlays:
      - overlay.extension.hardcore
languages:
  da:
    display_name: Danish
    translation_overlays:
      hardcore: overlay.extension.hardcore
    primary_target: standard
    targets:
      standard: da-standard
"#,
    )
    .expect_err("translation overlay kind is validated when present");

    assert_eq!(
        error.to_string(),
        "manifest language \"da\" translation overlay \"hardcore\" references overlay \"overlay.extension.hardcore\" with kind \"extension\"; expected translation"
    );
}

#[test]
fn source_languages_reject_translation_overlays() {
    let error = manifest::from_str(
        r#"
base: deck.yaml
overlays:
  overlay.translation.en:
    file: overlays/languages/en.yaml
    kind: translation
targets:
  en-standard:
    overlays: []
languages:
  en:
    display_name: English
    source: true
    translation_overlays:
      base: overlay.translation.en
    primary_target: standard
    targets:
      standard: en-standard
"#,
    )
    .expect_err("source language translation overlays are rejected");

    assert_eq!(
        error.to_string(),
        "manifest source language \"en\" must not declare translation_overlays"
    );
}

#[test]
fn language_catalog_reports_missing_primary_target_label() {
    let error = manifest::from_str(
        r#"
base: deck.yaml
targets:
  da-standard:
    overlays: []
languages:
  da:
    display_name: Danish
    primary_target: extended
    targets:
      standard: da-standard
"#,
    )
    .expect_err("missing primary target label is reported");

    assert_eq!(
        error.to_string(),
        "manifest language \"da\" primary_target \"extended\" is not present in its targets map"
    );
}

#[test]
fn parses_and_formats_translation_coverage_policy() {
    let formatted = manifest::format_str(
        r#"
base: deck.yaml
targets:
  de-release:
    translation_coverage: strict
    overlays: []
  de-dev:
    translation_coverage: lenient
    overlays: []
"#,
    )
    .expect("manifest formats");

    assert_eq!(
        formatted,
        r#"base: deck.yaml
overlays: {}
targets:
  de-dev:
    overlays: []
  de-release:
    overlays: []
    translation_coverage: strict
"#
    );
    let manifest = manifest::from_str(&formatted).expect("manifest parses");
    assert_eq!(
        manifest.targets["de-release"].translation_coverage,
        manifest::TranslationCoveragePolicy::Strict
    );
    assert_eq!(
        manifest.targets["de-dev"].translation_coverage,
        manifest::TranslationCoveragePolicy::Lenient
    );
}

#[test]
fn parses_and_formats_target_extends_for_package_federation() {
    let formatted = manifest::format_str(
        r#"
base: deck.yaml
overlays:
  overlay.extension.america:
    file: overlays/america.yaml
    kind: extension
targets:
  en-america:
    overlays: [overlay.extension.america]
    extends: anki-geo.ultimate-geography:en-standard
"#,
    )
    .expect("manifest formats");

    assert_eq!(
        formatted,
        r#"base: deck.yaml
overlays:
  overlay.extension.america:
    file: overlays/america.yaml
    kind: extension
targets:
  en-america:
    extends: anki-geo.ultimate-geography:en-standard
    overlays:
      - overlay.extension.america
"#
    );
    let manifest = manifest::from_str(&formatted).expect("manifest parses");
    let target = manifest.expand_target("en-america").unwrap();
    assert_eq!(
        target.extends.as_deref(),
        Some("anki-geo.ultimate-geography:en-standard")
    );
}

#[test]
fn formatter_emits_package_metadata_and_target_exports() {
    let formatted = manifest::format_str(
        r#"
targets:
  en-standard:
    exports:
      crowdanki:
        golden_allowlist: ['$.deck_config_uuid']
        golden: goldens/en-standard/deck.json
        out: build/en-standard
    overlays: []
package:
  depends_on: [anki-geo.shared-geography@0.1.0]
  compatible_base_versions: ['>=0.1,<0.2']
  base_package: anki-geo.shared-geography
  version: 0.1.0
  id: anki-geo.ultimate-geography
base: deck.yaml
"#,
    )
    .expect("manifest formats");

    assert_eq!(
        formatted,
        r#"package:
  id: anki-geo.ultimate-geography
  version: 0.1.0
  base_package: anki-geo.shared-geography
  compatible_base_versions:
    - '>=0.1, <0.2'
  depends_on:
    - 'anki-geo.shared-geography@0.1.0'
base: deck.yaml
overlays: {}
targets:
  en-standard:
    overlays: []
    exports:
      crowdanki:
        out: build/en-standard
        golden: goldens/en-standard/deck.json
        golden_allowlist:
          - '$.deck_config_uuid'
"#
    );
}

#[test]
fn package_semver_is_validated_and_requirements_are_canonicalized() {
    let manifest = manifest::from_str(
        r#"package:
  id: example.extension
  version: 2.0.0-beta.1+build.7
  base_package: example.base
  compatible_base_versions:
    - '>=1.2.3-alpha.1,<2'
    - '=2.0.0-beta.1'
  depends_on:
    - example.base@1.5.0
base: deck.yaml
"#,
    )
    .expect("valid semantic versions parse");
    let package = manifest.package.unwrap();
    assert_eq!(package.version, "2.0.0-beta.1+build.7");
    assert_eq!(
        package.compatible_base_versions,
        vec![">=1.2.3-alpha.1, <2", "=2.0.0-beta.1"]
    );

    for (field, old, replacement) in [
        ("version", "version: 1.0.0", "version: latest"),
        (
            "depends_on",
            "depends_on: [example.base@1.2.3]",
            "depends_on: [example.base@1.2]",
        ),
        (
            "compatible_base_versions",
            "compatible_base_versions: ['>=1']",
            "compatible_base_versions: ['']",
        ),
    ] {
        let source = "package:\n  id: example.extension\n  version: 1.0.0\n  base_package: example.base\n  compatible_base_versions: ['>=1']\n  depends_on: [example.base@1.2.3]\nbase: deck.yaml\n"
            .replace(old, replacement);
        let error = manifest::from_str(&source).expect_err("invalid SemVer must fail");
        assert!(error.to_string().contains(field), "{field}: {error}");
    }

    for dependency in ["example.base", "example.base@>=1.0.0"] {
        let source = format!(
            "package:\n  id: example.extension\n  version: 1.0.0\n  depends_on: [{dependency}]\nbase: deck.yaml\n"
        );
        let error = manifest::from_str(&source).expect_err("dependencies require exact pins");
        assert!(error.to_string().contains("depends_on[0]"), "{error}");
        assert!(
            error.to_string().contains("exact dependency pin"),
            "{error}"
        );
    }
}

#[test]
fn package_base_compatibility_fields_must_be_declared_together() {
    for package_fields in [
        "  compatible_base_versions: ['>=1']\n",
        "  base_package: example.base\n",
        "  base_package: example.base\n  compatible_base_versions: ['>=1']\n  depends_on: [other@1.0.0]\n",
    ] {
        let source = format!(
            "package:\n  id: example.extension\n  version: 1.0.0\n{package_fields}base: deck.yaml\n"
        );
        let error = manifest::from_str(&source).expect_err("invalid base relation must fail");
        assert!(error.to_string().contains("base_package"), "{error}");
    }
}

#[test]
fn rejects_unknown_overlay_catalog_kinds_at_manifest_decode() {
    let error = manifest::from_str(
        "base: deck.yaml\noverlays:\n  overlay.bad:\n    file: bad.yaml\n    kind: typo\n",
    )
    .expect_err("unknown catalog kind must fail");
    assert!(error.to_string().contains("overlays.overlay.bad.kind"));
}

#[test]
fn rejects_manifest_unknown_fields() {
    let error = manifest::from_str(
        r#"
base: deck.yaml
unknown: true
"#,
    )
    .expect_err("unknown fields are rejected");

    assert!(error.to_string().contains("unknown field `unknown`"));
}

#[test]
fn rejects_unknown_fields_at_nested_manifest_levels() {
    let cases = [
        (
            "package",
            r#"
package:
  id: pkg
  version: 1
  unknown: true
base: deck.yaml
"#,
        ),
        (
            "overlay",
            r#"
base: deck.yaml
overlays:
  overlay.translation.da:
    file: overlays/da.yaml
    unknown: true
"#,
        ),
        (
            "target",
            r#"
base: deck.yaml
targets:
  da:
    overlays: []
    unknown: true
"#,
        ),
        (
            "exports",
            r#"
base: deck.yaml
targets:
  da:
    overlays: []
    exports:
      unknown: true
"#,
        ),
        (
            "crowdanki",
            r#"
base: deck.yaml
targets:
  da:
    overlays: []
    exports:
      crowdanki:
        unknown: true
"#,
        ),
        (
            "language",
            r#"
base: deck.yaml
targets:
  da:
    overlays: []
languages:
  da:
    display_name: Danish
    primary_target: standard
    targets:
      standard: da
    unknown: true
"#,
        ),
        (
            "translation_profile",
            r#"
base: deck.yaml
translation_profile:
  unknown: true
"#,
        ),
        (
            "metadata_category",
            r#"
base: deck.yaml
translation_profile:
  metadata_categories:
    - key: deck
      label: Deck
      unknown: true
"#,
        ),
    ];

    for (case, yaml) in cases {
        let error =
            manifest::from_str(yaml).expect_err(&format!("{case}: unknown nested field rejected"));
        assert!(
            error.to_string().contains("unknown field `unknown`"),
            "{case}: {error}"
        );
    }
}

#[test]
fn reports_manifest_shape_and_validation_errors() {
    let missing = manifest::from_str("targets: {}\n").expect_err("missing base is reported");
    assert!(missing.to_string().contains("missing field `base`"));

    let wrong_type = manifest::from_str("base: []\n").expect_err("wrong base type is reported");
    assert!(wrong_type.to_string().contains("invalid type"));

    let duplicate = manifest::from_str("base: one\nbase: two\n")
        .expect_err("duplicate keys are rejected by strict YAML parsing");
    assert!(duplicate.to_string().contains("duplicate"));

    let invalid_policy = manifest::from_str(
        r#"
base: deck.yaml
targets:
  release:
    overlays: []
    translation_coverage: aggressive
"#,
    )
    .expect_err("invalid translation coverage policy is rejected");
    assert_eq!(
        invalid_policy.to_string(),
        "invalid translation coverage policy \"aggressive\"; expected lenient or strict"
    );
}

#[test]
fn rejects_duplicate_manifest_dynamic_map_keys_with_schema_paths() {
    let cases = [
        (
            "overlay",
            r#"base: deck.yaml
overlays:
  overlay.patch.one:
    file: one.yaml
  overlay.patch.one:
    file: two.yaml
"#,
            "overlays.overlay.patch.one",
            "overlay.patch.one",
        ),
        (
            "target",
            r#"base: deck.yaml
targets:
  release:
    overlays: []
  release:
    overlays: []
"#,
            "targets.release",
            "release",
        ),
        (
            "language target label",
            r#"base: deck.yaml
languages:
  de:
    display_name: German
    primary_target: standard
    targets:
      standard: de-standard
      standard: de-release
"#,
            "languages.de.targets.standard",
            "standard",
        ),
    ];

    for (case, yaml, path, key) in cases {
        let error = manifest::from_str(yaml)
            .expect_err(&format!("{case}: duplicate dynamic key is rejected"));
        let message = error.to_string();
        assert!(
            message.contains(&format!("duplicate key {key:?}")),
            "{case}: {message}"
        );
        assert!(message.contains(path), "{case}: {message}");
        assert!(
            manifest::format_str(yaml).is_err(),
            "{case}: formatter must reject duplicate"
        );
    }
}

#[test]
fn validates_translation_profile_category_references() {
    let error = manifest::from_str(
        r#"
base: deck.yaml
translation_profile:
  metadata_categories:
    - key: deck
      label: Deck
      paths: [deck.name]
  metadata_category_order: [deck, missing]
"#,
    )
    .expect_err("unknown metadata category order references are rejected");

    assert_eq!(
        error.to_string(),
        "manifest translation profile metadata_category_order references unknown category key \"missing\""
    );
}

#[test]
fn formats_empty_crowdanki_export_stably() {
    let formatted = manifest::format_str(
        r#"
base: deck.yaml
targets:
  en:
    overlays: []
    exports:
      crowdanki: {}
"#,
    )
    .expect("manifest formats");

    assert_eq!(
        formatted,
        r#"base: deck.yaml
overlays: {}
targets:
  en:
    overlays: []
    exports:
      crowdanki: {}
"#
    );
    assert_eq!(manifest::format_str(&formatted).unwrap(), formatted);
}

#[test]
fn reports_missing_overlay_references() {
    let manifest = manifest::from_str(
        r#"
base: deck.yaml
targets:
  broken:
    overlays:
      - missing.overlay
"#,
    )
    .expect("manifest parses");

    let error = manifest
        .expand_target("broken")
        .expect_err("missing overlay is reported");

    assert_eq!(
        error.to_string(),
        "manifest overlay \"missing.overlay\" does not exist"
    );
}

#[test]
fn reports_overlay_dependency_cycles() {
    let manifest = manifest::from_str(
        r#"
base: deck.yaml
overlays:
  a:
    file: a.yaml
    depends_on: [b]
  b:
    file: b.yaml
    depends_on: [a]
targets:
  cyclic:
    overlays: [a]
"#,
    )
    .expect("manifest parses");

    let error = manifest
        .expand_target("cyclic")
        .expect_err("cycle is reported");

    assert_eq!(
        error.to_string(),
        "manifest overlay dependency cycle: a -> b -> a"
    );
}
