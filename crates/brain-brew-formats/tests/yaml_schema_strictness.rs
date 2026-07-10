use brain_brew_formats::{canonical_yaml, lockfile, manifest, media_map};

const NON_STRING_SCALARS: &[(&str, &str)] = &[
    ("boolean", "true"),
    ("null", "null"),
    ("integer", "123"),
    ("float", "1.5"),
    ("hex integer", "0x10"),
];

#[test]
fn rejects_non_string_scalars_at_canonical_string_positions() {
    for (case, scalar) in NON_STRING_SCALARS {
        let canonical = format!(
            "deck:\n  id: deck.strict\n  name: Strict\n  description: {scalar}\nnote_types: {{}}\nnotes: {{}}\n"
        );
        assert_rejected_by_both(
            case,
            &canonical,
            "deck.description",
            canonical_yaml::from_str,
            canonical_yaml::format_str,
        );

        let overlay = format!("id: {scalar}\nkind: patch\n");
        assert_rejected_by_both(
            case,
            &overlay,
            "id",
            canonical_yaml::overlay_from_str,
            canonical_yaml::overlay_format_str,
        );

        let manifest_yaml = format!("base: {scalar}\n");
        assert_rejected_by_both(
            case,
            &manifest_yaml,
            "base",
            manifest::from_str,
            manifest::format_str,
        );

        let lock_yaml = format!(
            "version: 2\npackages:\n  package.strict:\n    manifest: {scalar}\n    package:\n      version: '1'\n    original:\n      type: path\n      path: .\n    locked:\n      type: path\n      path: .\n      nar_hash: 'sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA='\n"
        );
        assert_rejected_by_both(
            case,
            &lock_yaml,
            "packages.package.strict.manifest",
            lockfile::from_str,
            lockfile::format_str,
        );

        let media_yaml = format!("media.strict:\n  path: {scalar}\n  sha256: hash\n");
        assert_rejected_by_both(
            case,
            &media_yaml,
            "media.strict.path",
            media_map::from_str,
            media_map::format_str,
        );
    }
}

#[test]
fn preserves_intentionally_typed_yaml_scalars_and_quoted_lookalikes() {
    manifest::from_str(
        "base: '123'\nlanguages:\n  en:\n    display_name: 'true'\n    source: true\n    primary_target: standard\n    targets:\n      standard: standard\ntargets:\n  standard:\n    overlays: []\n",
    )
    .expect("manifest bool remains typed while quoted lookalikes remain strings");

    canonical_yaml::overlay_from_str(
        "id: overlay.strict\nkind: translation\ntranslations:\n  require_complete: true\n  direct:\n    'null': '123'\n",
    )
    .expect("translation completeness bool remains typed");

    lockfile::from_str("version: 2\npackages: {}\n")
        .expect("lock format version remains an integer");
}

#[test]
fn rejects_every_conflicting_or_orphaned_overlay_field_representation() {
    let cases = [
        (
            "scalar and positional message",
            "        value: Helsinki\n        message:\n          - literal: city\n",
        ),
        (
            "image and formatted message",
            "        value: !image media.flag.fi\n        format: '{city}'\n        variables:\n          city:\n            literal: Helsinki\n",
        ),
        (
            "positional and formatted message",
            "        message:\n          - literal: city\n        format: '{city}'\n        variables:\n          city:\n            literal: Helsinki\n",
        ),
        (
            "orphan message variables",
            "        variables:\n          city:\n            literal: Helsinki\n",
        ),
        (
            "formatted message without variables map",
            "        format: '{city}'\n",
        ),
    ];

    for (case, payload) in cases {
        let yaml = overlay_with_field_payload(payload);
        assert_rejected_by_both(
            case,
            &yaml,
            "notes.note.fi.fields.field.capital",
            canonical_yaml::overlay_from_str,
            canonical_yaml::overlay_format_str,
        );
    }
}

#[test]
fn rejects_partial_overlay_media_references() {
    for (case, payload) in [
        ("path without hash", "    path: flags/fi.svg\n"),
        ("hash without path", "    sha256: abc123\n"),
    ] {
        let yaml = format!(
            "id: overlay.media\nkind: extension\nmedia:\n  media.flag.fi:\n    intent: add\n{payload}"
        );
        assert_rejected_by_both(
            case,
            &yaml,
            "media.media.flag.fi",
            canonical_yaml::overlay_from_str,
            canonical_yaml::overlay_format_str,
        );
    }
}

#[test]
fn rejects_malformed_base_field_value_unions() {
    let cases = [
        (
            "formatted and positional message",
            "      field.value:\n        format: '{value}'\n        variables: {}\n        message: []\n",
        ),
        (
            "formatted message without variables",
            "      field.value:\n        format: '{value}'\n",
        ),
        (
            "message component with two variants",
            "      field.value:\n        message:\n          - literal: value\n            text: translated\n",
        ),
        (
            "mixed image sequence",
            "      field.value:\n        - !image media.one\n        - plain-string\n",
        ),
        ("empty image payload", "      field.value: !image\n"),
    ];

    for (case, field) in cases {
        let yaml = canonical_with_field(field);
        assert_rejected_by_both(
            case,
            &yaml,
            "notes.note.strict.fields.field.value",
            canonical_yaml::from_str,
            canonical_yaml::format_str,
        );
    }
}

#[test]
fn rejects_incomplete_translation_adaptations_without_discarding_context() {
    let yaml = "id: overlay.translation.strict\nkind: translation\ntarget_adaptations:\n  notes.note.strict.fields.field.value:\n    expected_source: old\n";
    assert_rejected_by_both(
        "missing target",
        yaml,
        "target_adaptations.notes.note.strict.fields.field.value",
        canonical_yaml::overlay_from_str,
        canonical_yaml::overlay_format_str,
    );
}

fn overlay_with_field_payload(payload: &str) -> String {
    format!(
        "id: overlay.strict\nkind: patch\nnotes:\n  note.fi:\n    intent: merge\n    fields:\n      field.capital:\n        intent: replace\n{payload}"
    )
}

fn canonical_with_field(field: &str) -> String {
    format!(
        "deck:\n  id: deck.strict\n  name: Strict\n  description: Strict schema\nnote_types:\n  note-type.strict:\n    name: Strict\n    field_order: [field.value]\n    fields:\n      field.value:\n        name: Value\n    card_template_order: []\n    card_templates: {{}}\n    styling: ''\nnotes:\n  note.strict:\n    note_type_id: note-type.strict\n    fields:\n{field}    tags: []\n"
    )
}

fn assert_rejected_by_both<T, U, E: std::fmt::Display>(
    case: &str,
    yaml: &str,
    path: &str,
    parse: impl Fn(&str) -> Result<T, E>,
    format: impl Fn(&str) -> Result<U, E>,
) {
    for message in [
        parse(yaml)
            .err()
            .unwrap_or_else(|| panic!("{case}: decoder unexpectedly accepted YAML"))
            .to_string(),
        format(yaml)
            .err()
            .unwrap_or_else(|| panic!("{case}: formatter unexpectedly accepted YAML"))
            .to_string(),
    ] {
        assert!(
            message.contains(path),
            "{case}: missing {path:?} in {message:?}"
        );
    }
}
