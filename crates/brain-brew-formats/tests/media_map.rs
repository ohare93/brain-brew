use brain_brew_formats::media_map;

#[test]
fn media_map_format_str_emits_root_mapping_with_inline_media_ordering_and_scalars() {
    let input = r#"media.flag.b:
  sha256: hash-b
  path: 'flags/key: value.png'
media.flag.a:
  path: flags/a.png
  sha256: 'true'
"#;

    let formatted = media_map::format_str(input).expect("media map formats");

    assert_eq!(
        formatted,
        "media.flag.a:\n  path: flags/a.png\n  sha256: 'true'\nmedia.flag.b:\n  path: 'flags/key: value.png'\n  sha256: hash-b\n"
    );
    assert_eq!(
        media_map::format_str(&formatted).expect("formatted media map is idempotent"),
        formatted
    );
}

#[test]
fn media_map_empty_file_formats_as_inline_empty_mapping() {
    let formatted = media_map::format_str("{}\n").expect("empty media map formats");

    assert_eq!(formatted, "{}\n");
    assert_eq!(media_map::format_str(&formatted).unwrap(), formatted);
    assert!(media_map::from_str(&formatted).unwrap().is_empty());
}

#[test]
fn media_map_preserves_duplicate_path_entries_by_media_id() {
    let input = r#"media.flag.primary:
  path: flags/shared.png
  sha256: hash-primary
media.flag.copy:
  path: flags/shared.png
  sha256: hash-copy
"#;

    let media = media_map::from_str(input).expect("duplicate paths are valid declarations");

    assert_eq!(media.len(), 2);
    assert_eq!(
        media
            .values()
            .filter(|entry| entry.path == "flags/shared.png")
            .count(),
        2
    );
    assert_eq!(
        media_map::format_str(input).unwrap(),
        "media.flag.copy:\n  path: flags/shared.png\n  sha256: hash-copy\nmedia.flag.primary:\n  path: flags/shared.png\n  sha256: hash-primary\n"
    );
}

#[test]
fn media_map_formats_unicode_companion_and_dependency_paths() {
    let input = r#"media.ug-hardcore-companion.bali-flag:
  sha256: hash-bali
  path: flags/üñîçødé-bali.png
media.ug-core.dependency-map:
  path: maps/dependency world.svg
  sha256: hash-map
"#;

    let formatted = media_map::format_str(input).expect("companion/dependency media map formats");

    assert_eq!(
        formatted,
        "media.ug-core.dependency-map:\n  path: maps/dependency world.svg\n  sha256: hash-map\nmedia.ug-hardcore-companion.bali-flag:\n  path: 'flags/üñîçødé-bali.png'\n  sha256: hash-bali\n"
    );
    assert_eq!(media_map::format_str(&formatted).unwrap(), formatted);
}

#[test]
fn media_map_rejects_duplicate_ids_with_schema_path() {
    let yaml = r#"media.flag.a:
  path: flags/a.png
  sha256: hash-a
media.flag.a:
  path: flags/b.png
  sha256: hash-b
"#;

    let error = media_map::from_str(yaml).expect_err("duplicate media ID is rejected");
    let message = error.to_string();
    assert!(
        message.contains("duplicate key \"media.flag.a\""),
        "{message}"
    );
    assert!(message.contains("media.flag.a"), "{message}");
    assert!(media_map::format_str(yaml).is_err());
}

#[test]
fn media_map_rejects_unknown_media_fields() {
    let error = media_map::format_str(
        "media.flag.a:\n  path: flags/a.png\n  sha256: hash-a\n  extra: nope\n",
    )
    .expect_err("unknown fields are denied");

    assert!(
        error.to_string().contains("extra"),
        "unexpected error: {error}"
    );
}
