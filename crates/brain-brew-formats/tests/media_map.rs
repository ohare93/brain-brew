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
