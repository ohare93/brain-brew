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
    assert_eq!(package.compatible_base_versions, vec![">=0.1,<0.2"]);
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
  optional_paths: [note_types.*, deck.*, notes.*.tags.*]
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
  optional_paths:
    - 'note_types.*'
    - 'deck.*'
    - 'notes.*.tags.*'
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
    assert_eq!(
        manifest.translation_profile.optional_paths,
        vec!["note_types.*", "deck.*", "notes.*.tags.*"]
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
  compatible_base_versions:
    - '>=0.1,<0.2'
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
