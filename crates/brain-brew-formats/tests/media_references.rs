use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use brain_brew_core::{
    AdapterIds, CanonicalDeck, CardTemplate, FieldDefinition, FieldImageReference, FieldValue,
    MediaReference, Note, NoteType, StableId, Tombstones,
};
use brain_brew_formats::{
    canonical_yaml,
    media::{self, MediaHashPolicy, MediaValidationErrorKind},
    source_includes,
};

#[test]
fn extracts_media_references_from_fields_and_templates() {
    let deck = media_deck();

    let paths = media::referenced_paths(&deck);

    assert!(paths.contains("flags/fi.png"));
    assert!(paths.contains("audio/fi.mp3"));
    assert!(paths.contains("maps/fi.svg"));
}

#[test]
fn extractor_finds_rendered_field_img_and_sound_references() {
    let paths = media::extract_media_references_from_rendered_field(
        r#"<img src="flags/fi.png"><img src='maps/fi.svg'>[sound:audio/fi.mp3]"#,
    );

    assert_eq!(
        paths,
        BTreeSet::from([
            "audio/fi.mp3".to_owned(),
            "flags/fi.png".to_owned(),
            "maps/fi.svg".to_owned(),
        ])
    );
}

#[test]
fn extracts_multiple_img_sources_from_one_field() {
    let mut deck = media_deck();
    deck.notes
        .get_mut(&sid("note.finland"))
        .unwrap()
        .fields
        .insert(
            sid("field.flag"),
            "<img src=\"flags/fi-blur.png\" /><img src=\"flags/fi.png\" />".to_owned(),
        );

    let paths = media::referenced_paths(&deck);

    assert!(paths.contains("flags/fi-blur.png"));
    assert!(paths.contains("flags/fi.png"));
}

#[test]
fn referenced_paths_resolves_structured_image_ids_to_declared_paths() {
    let mut deck = media_deck();
    let note = deck.notes.get_mut(&sid("note.finland")).unwrap();
    note.fields.insert(
        sid("field.flag"),
        FieldValue::Images(vec![FieldImageReference {
            media_id: sid("media.flags-fi-png"),
        }]),
    );

    let paths = media::referenced_paths(&deck);

    assert!(paths.contains("flags/fi.png"));
}

#[test]
fn validation_reports_unknown_structured_image_media_id_with_field_path() {
    let mut deck = media_deck();
    let note = deck.notes.get_mut(&sid("note.finland")).unwrap();
    note.fields.insert(
        sid("field.flag"),
        FieldValue::Images(vec![FieldImageReference {
            media_id: sid("media.unknown"),
        }]),
    );

    let report = media::validate_references(&deck).expect_err("unknown media id must fail");

    assert!(report.has_kind(MediaValidationErrorKind::UnknownMediaId));
    assert!(report.errors.iter().any(|error| {
        error.message.contains("unknown media id `media.unknown`")
            && error
                .message
                .contains("field `notes.note.finland.fields.field.flag`")
    }));
}

#[test]
fn extracts_css_url_media_from_templates_and_styling() {
    let mut deck = media_deck();
    let note_type = deck.note_types.get_mut(&sid("note-type.country")).unwrap();
    note_type.styling = "@import url(\"css/maps.css\");".to_owned();
    note_type.card_templates[0].question_format =
        "<link rel=\"stylesheet\" href=\"css/interactive.css\"><a href=\"https://example.com\">source</a><a href=\"mailto:geo@example.com\">mail</a><a href=\"#answer\">answer</a>{{Country}}".to_owned();
    note_type.card_templates[0].answer_format =
        "<style>@import url('css/review.css');</style>{{Flag}}".to_owned();

    let paths = media::referenced_paths(&deck);

    assert!(paths.contains("css/maps.css"));
    assert!(paths.contains("css/interactive.css"));
    assert!(paths.contains("css/review.css"));
    assert!(!paths.contains("https://example.com"));
    assert!(!paths.contains("mailto:geo@example.com"));
    assert!(!paths.contains("#answer"));
}

#[test]
fn validation_accepts_declared_media_references() {
    let deck = media_deck();

    assert!(media::validate_references(&deck).is_ok());
}

#[test]
fn reference_scanner_decodes_rendered_url_and_html_attribute_paths() {
    let paths = media::extract_media_references_from_rendered_field(
        r#"<img SRC = "flags/flag%20%26%20map%20%231%3F.svg" /><link href = "styles/a&amp;b.css"><style>.x { background: URL('地図/%E6%97%A5%E6%9C%AC.svg'); }</style>"#,
    );

    assert_eq!(
        paths,
        BTreeSet::from([
            "flags/flag & map #1?.svg".to_owned(),
            "styles/a&b.css".to_owned(),
            "地図/日本.svg".to_owned(),
        ])
    );
}

#[test]
fn declaration_validation_requires_canonical_hashes_only_in_release_mode() {
    let mut deck = media_deck();
    deck.media
        .get_mut(&sid("media.flags-fi-png"))
        .unwrap()
        .sha256 = String::new();
    deck.media
        .get_mut(&sid("media.audio-fi-mp3"))
        .unwrap()
        .sha256 = "ABCDEF".repeat(10) + "ABCD";

    let reference_only = media::validate_declarations(&deck, MediaHashPolicy::Optional)
        .expect_err("a present noncanonical hash still fails");
    assert!(reference_only.has_kind(MediaValidationErrorKind::InvalidHash));

    deck.media
        .get_mut(&sid("media.audio-fi-mp3"))
        .unwrap()
        .sha256 = media::sha256_hex(b"audio");
    assert!(media::validate_declarations(&deck, MediaHashPolicy::Optional).is_ok());

    let strict = media::validate_declarations(&deck, MediaHashPolicy::Required)
        .expect_err("release mode requires every hash");
    assert!(strict.has_kind(MediaValidationErrorKind::EmptyHash));
}

#[test]
fn conflicting_declaration_output_paths_fail_even_without_asset_bytes() {
    let mut deck = media_deck();
    deck.media.insert(
        sid("media.collision"),
        MediaReference {
            id: sid("media.collision"),
            path: "flags/fi.png".to_owned(),
            sha256: media::sha256_hex(b"different bytes"),
        },
    );

    let report = media::validate_references(&deck).expect_err("output collision must fail");
    assert!(report.has_kind(MediaValidationErrorKind::PathCollision));
}

#[test]
fn unsafe_or_ambiguous_media_paths_fail_reference_validation() {
    for path in [
        "line\nfeed.png",
        "https:payload.png",
        r"flags\escape.png",
        "bidi\u{202e}.png",
    ] {
        let mut deck = media_deck();
        deck.media.get_mut(&sid("media.flags-fi-png")).unwrap().path = path.to_owned();
        let report = media::validate_references(&deck).expect_err("unsafe path must fail");
        assert!(
            report.has_kind(MediaValidationErrorKind::UnsafePath),
            "{path:?}: {report}"
        );
    }
}

#[test]
fn malformed_encoded_raw_reference_fails_instead_of_becoming_undeclared_noise() {
    let mut deck = media_deck();
    deck.notes
        .get_mut(&sid("note.finland"))
        .unwrap()
        .fields
        .insert(
            sid("field.flag"),
            r#"<img src="flags/%ZZ.png" />"#.to_owned(),
        );

    let report = media::validate_references(&deck).expect_err("bad URL encoding must fail");
    assert!(report.has_kind(MediaValidationErrorKind::InvalidReferenceEncoding));
}

#[test]
fn media_validation_follows_structural_media_include() {
    let dir = temp_fixture_dir("media-validation-follows-include");
    fs::write(
        dir.join("media.yaml"),
        "media.flags-fi-png:\n  path: flags/fi.png\n  sha256: 14873f4faae48052921f9272d948a369f775b2406e57a9b8d55fb94452b73948\n",
    )
    .unwrap();
    let source = minimal_deck_with_media_include("!image media.flags-fi-png");
    let resolved =
        source_includes::resolve_file_includes(&source, &dir.join("deck.yaml"), &dir, &[])
            .expect("media include resolves into deck source");
    let deck = canonical_yaml::from_str(&resolved).expect("resolved deck parses");
    let assets = BTreeMap::from([("flags/fi.png".to_owned(), b"flag-bytes".to_vec())]);

    assert!(media::validate_references(&deck).is_ok());
    assert!(media::validate_hashes(&deck, &assets).is_ok());
}

#[test]
fn include_preserving_formatter_restores_minimal_media_include_idempotently() {
    let source = minimal_deck_with_media_include("'<img src=\"flags/fi.png\" />'");

    let formatted =
        source_includes::format_preserving_file_includes(&source, canonical_yaml::format_str)
            .expect("media include formats");

    assert!(
        formatted.contains("media: !include media.yaml\n"),
        "{formatted}"
    );
    assert_eq!(
        source_includes::format_preserving_file_includes(&formatted, canonical_yaml::format_str)
            .expect("formatted media include is idempotent"),
        formatted
    );
}

#[test]
fn validation_reports_missing_media_reference_paths() {
    let mut deck = media_deck();
    deck.media.remove(&sid("media.flags-fi-png"));

    let report = media::validate_references(&deck).expect_err("missing media must fail");

    assert!(report.has_kind(MediaValidationErrorKind::MissingReference));
    assert!(
        report
            .errors
            .iter()
            .any(|error| error.path == "flags/fi.png")
    );
}

#[test]
fn validates_media_asset_hashes_from_supplied_bytes() {
    let mut deck = media_deck();
    deck.media
        .get_mut(&sid("media.flags-fi-png"))
        .unwrap()
        .sha256 = media::sha256_hex(b"flag-bytes");
    deck.media
        .get_mut(&sid("media.audio-fi-mp3"))
        .unwrap()
        .sha256 = media::sha256_hex(b"audio-bytes");
    deck.media
        .get_mut(&sid("media.maps-fi-svg"))
        .unwrap()
        .sha256 = media::sha256_hex(b"map-bytes");
    let assets = BTreeMap::from([
        ("flags/fi.png".to_owned(), b"flag-bytes".to_vec()),
        ("audio/fi.mp3".to_owned(), b"audio-bytes".to_vec()),
        ("maps/fi.svg".to_owned(), b"map-bytes".to_vec()),
    ]);

    assert!(media::validate_hashes(&deck, &assets).is_ok());
}

#[test]
fn reports_media_hash_mismatches() {
    let mut deck = media_deck();
    deck.media
        .get_mut(&sid("media.flags-fi-png"))
        .unwrap()
        .sha256 = media::sha256_hex(b"expected");
    let assets = BTreeMap::from([("flags/fi.png".to_owned(), b"actual".to_vec())]);

    let report = media::validate_hashes(&deck, &assets).expect_err("hash mismatch must fail");

    assert!(report.has_kind(MediaValidationErrorKind::HashMismatch));
    assert!(
        report
            .errors
            .iter()
            .any(|error| error.path == "flags/fi.png")
    );
}

#[test]
fn validation_warns_about_unused_media_references_without_failing() {
    let mut deck = media_deck();
    deck.media.insert(
        sid("media.unused"),
        MediaReference {
            id: sid("media.unused"),
            path: "unused.png".to_owned(),
            sha256: media::sha256_hex(b"unused"),
        },
    );

    let report = media::reference_report(&deck);

    assert!(media::validate_references(&deck).is_ok());
    assert!(report.has_warning_kind(MediaValidationErrorKind::UnusedReference));
    assert!(
        report
            .warnings
            .iter()
            .any(|error| error.path == "unused.png")
    );
}

#[test]
fn reports_empty_media_hashes() {
    let mut deck = media_deck();
    deck.media
        .get_mut(&sid("media.flags-fi-png"))
        .unwrap()
        .sha256 = String::new();
    let assets = BTreeMap::from([("flags/fi.png".to_owned(), b"actual".to_vec())]);

    let report = media::validate_hashes(&deck, &assets).expect_err("empty hash must fail");

    assert!(report.has_kind(MediaValidationErrorKind::EmptyHash));
    assert!(
        report
            .errors
            .iter()
            .any(|error| error.path == "flags/fi.png"
                && error.message.contains("media.flags-fi-png"))
    );
}

fn minimal_deck_with_media_include(flag_field: &str) -> String {
    format!(
        r#"deck:
  id: deck.media-include
  name: Media Include
  description: ''
  adapter_ids: {{}}
note_types:
  note-type.country:
    name: Country
    field_order:
      - field.flag
    fields:
      field.flag:
        name: Flag
    card_template_order:
      - template.country-flag
    card_templates:
      template.country-flag:
        name: Country - Flag
        question_format: '{{{{Flag}}}}'
        answer_format: '{{{{FrontSide}}}}'
        adapter_ids: {{}}
    styling: ''
    adapter_ids: {{}}
notes:
  note.finland:
    note_type_id: note-type.country
    fields:
      field.flag: {flag_field}
    tags:
      - Media
    adapter_ids: {{}}
media: !include media.yaml
tombstones: []
"#
    )
}

fn temp_fixture_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}-{unique}"));
    fs::create_dir_all(&path).unwrap();
    path
}

fn media_deck() -> CanonicalDeck {
    let note_type = NoteType {
        id: sid("note-type.country"),
        name: "Country".to_owned(),
        variables: BTreeMap::new(),
        fields: vec![
            FieldDefinition {
                id: sid("field.country"),
                name: "Country".to_owned(),
                rtl: false,
                message_pattern: None,
            },
            FieldDefinition {
                id: sid("field.flag"),
                name: "Flag".to_owned(),
                rtl: false,
                message_pattern: None,
            },
        ],
        card_templates: vec![CardTemplate {
            id: sid("template.country-flag"),
            name: "Country - Flag".to_owned(),
            variables: BTreeMap::new(),
            question_format: "{{Country}} <img src=\"maps/fi.svg\">".to_owned(),
            answer_format: "{{Flag}} [sound:audio/fi.mp3]".to_owned(),
            adapter_ids: AdapterIds::new(),
        }],
        styling: String::new(),
        adapter_ids: AdapterIds::new(),
    };

    let note = Note {
        id: sid("note.finland"),
        note_type_id: sid("note-type.country"),
        variables: BTreeMap::new(),
        fields: BTreeMap::from([
            (sid("field.country"), "Finland".to_owned()),
            (sid("field.flag"), "<img src=\"flags/fi.png\">".to_owned()),
        ])
        .into(),
        tags: BTreeSet::new(),
        adapter_ids: AdapterIds::new(),
    };

    CanonicalDeck {
        id: sid("deck.media"),
        name: "Media".to_owned(),
        description: String::new(),
        variables: BTreeMap::new(),
        note_types: BTreeMap::from([(note_type.id.clone(), note_type)]),
        notes: BTreeMap::from([(note.id.clone(), note)]),
        media: BTreeMap::from([
            (
                sid("media.flags-fi-png"),
                MediaReference {
                    id: sid("media.flags-fi-png"),
                    path: "flags/fi.png".to_owned(),
                    sha256: media::sha256_hex(b"flag-bytes"),
                },
            ),
            (
                sid("media.audio-fi-mp3"),
                MediaReference {
                    id: sid("media.audio-fi-mp3"),
                    path: "audio/fi.mp3".to_owned(),
                    sha256: media::sha256_hex(b"audio-bytes"),
                },
            ),
            (
                sid("media.maps-fi-svg"),
                MediaReference {
                    id: sid("media.maps-fi-svg"),
                    path: "maps/fi.svg".to_owned(),
                    sha256: media::sha256_hex(b"map-bytes"),
                },
            ),
        ]),
        tombstones: Tombstones::default(),
        adapter_ids: AdapterIds::new(),
    }
}

fn sid(value: &str) -> StableId {
    StableId::new(value).expect("test stable id is valid")
}
