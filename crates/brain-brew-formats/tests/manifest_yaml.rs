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
