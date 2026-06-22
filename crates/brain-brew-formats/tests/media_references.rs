use std::collections::{BTreeMap, BTreeSet};

use brain_brew_core::{
    AdapterIds, CanonicalDeck, CardTemplate, FieldDefinition, MediaReference, Note, NoteType,
    StableId,
};
use brain_brew_formats::media::{self, MediaValidationErrorKind};

#[test]
fn extracts_media_references_from_fields_and_templates() {
    let deck = media_deck();

    let paths = media::referenced_paths(&deck);

    assert!(paths.contains("flags/fi.png"));
    assert!(paths.contains("audio/fi.mp3"));
    assert!(paths.contains("maps/fi.svg"));
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
fn validation_reports_unused_media_references() {
    let mut deck = media_deck();
    deck.media.insert(
        sid("media.unused"),
        MediaReference {
            id: sid("media.unused"),
            path: "unused.png".to_owned(),
            sha256: "abc".to_owned(),
        },
    );

    let report = media::validate_references(&deck).expect_err("unused media must fail");

    assert!(report.has_kind(MediaValidationErrorKind::UnusedReference));
    assert!(report.errors.iter().any(|error| error.path == "unused.png"));
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
            },
            FieldDefinition {
                id: sid("field.flag"),
                name: "Flag".to_owned(),
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
        ]),
        field_messages: BTreeMap::new(),
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
                    sha256: "abc".to_owned(),
                },
            ),
            (
                sid("media.audio-fi-mp3"),
                MediaReference {
                    id: sid("media.audio-fi-mp3"),
                    path: "audio/fi.mp3".to_owned(),
                    sha256: "def".to_owned(),
                },
            ),
            (
                sid("media.maps-fi-svg"),
                MediaReference {
                    id: sid("media.maps-fi-svg"),
                    path: "maps/fi.svg".to_owned(),
                    sha256: "ghi".to_owned(),
                },
            ),
        ]),
        tombstones: BTreeSet::new(),
        adapter_ids: AdapterIds::new(),
    }
}

fn sid(value: &str) -> StableId {
    StableId::new(value).expect("test stable id is valid")
}
