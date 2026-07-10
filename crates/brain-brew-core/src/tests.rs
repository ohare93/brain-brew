use std::str::FromStr;

use super::{CRATE_NAME, DeckPath, StableId, glob_matches};

fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

#[test]
fn exposes_core_crate_name() {
    assert_eq!(CRATE_NAME, "brain-brew-core");
}

#[test]
fn deck_path_display_and_parse_round_trip_current_grammar() {
    let note_type_id = sid("note-type.country");
    let note_id = sid("note.finland");
    let field_id = sid("field.capital");
    let template_id = sid("template.card-1");
    let media_id = sid("media.flag.finland");

    let cases = [
        (DeckPath::DeckName, "deck.name"),
        (DeckPath::DeckDescription, "deck.description"),
        (DeckPath::DeckVariables, "deck.variables"),
        (
            DeckPath::DeckVariable {
                key: "source.language".to_owned(),
            },
            "deck.variables.source.language",
        ),
        (DeckPath::DeckAdapterIds, "deck.adapter_ids"),
        (
            DeckPath::DeckAdapterId {
                key: "crowdanki_uuid".to_owned(),
            },
            "deck.adapter_ids.crowdanki_uuid",
        ),
        (
            DeckPath::NoteType {
                note_type_id: note_type_id.clone(),
            },
            "note_types.note-type.country",
        ),
        (
            DeckPath::NoteTypeId {
                note_type_id: note_type_id.clone(),
            },
            "note_types.note-type.country.id",
        ),
        (
            DeckPath::NoteTypeName {
                note_type_id: note_type_id.clone(),
            },
            "note_types.note-type.country.name",
        ),
        (
            DeckPath::NoteTypeVariables {
                note_type_id: note_type_id.clone(),
            },
            "note_types.note-type.country.variables",
        ),
        (
            DeckPath::NoteTypeVariable {
                note_type_id: note_type_id.clone(),
                key: "card.prompt".to_owned(),
            },
            "note_types.note-type.country.variables.card.prompt",
        ),
        (
            DeckPath::NoteTypeStyling {
                note_type_id: note_type_id.clone(),
            },
            "note_types.note-type.country.styling",
        ),
        (
            DeckPath::NoteTypeFields {
                note_type_id: note_type_id.clone(),
            },
            "note_types.note-type.country.fields",
        ),
        (
            DeckPath::NoteTypeField {
                note_type_id: note_type_id.clone(),
                field_id: field_id.clone(),
            },
            "note_types.note-type.country.fields.field.capital",
        ),
        (
            DeckPath::NoteTypeFieldName {
                note_type_id: note_type_id.clone(),
                field_id: field_id.clone(),
            },
            "note_types.note-type.country.fields.field.capital.name",
        ),
        (
            DeckPath::NoteTypeCardTemplates {
                note_type_id: note_type_id.clone(),
            },
            "note_types.note-type.country.card_templates",
        ),
        (
            DeckPath::NoteTypeCardTemplate {
                note_type_id: note_type_id.clone(),
                template_id: template_id.clone(),
            },
            "note_types.note-type.country.card_templates.template.card-1",
        ),
        (
            DeckPath::NoteTypeCardTemplateName {
                note_type_id: note_type_id.clone(),
                template_id: template_id.clone(),
            },
            "note_types.note-type.country.card_templates.template.card-1.name",
        ),
        (
            DeckPath::NoteTypeCardTemplateVariables {
                note_type_id: note_type_id.clone(),
                template_id: template_id.clone(),
            },
            "note_types.note-type.country.card_templates.template.card-1.variables",
        ),
        (
            DeckPath::NoteTypeCardTemplateVariable {
                note_type_id: note_type_id.clone(),
                template_id: template_id.clone(),
                key: "side.label".to_owned(),
            },
            "note_types.note-type.country.card_templates.template.card-1.variables.side.label",
        ),
        (
            DeckPath::NoteTypeCardTemplateQuestionFormat {
                note_type_id: note_type_id.clone(),
                template_id: template_id.clone(),
            },
            "note_types.note-type.country.card_templates.template.card-1.question_format",
        ),
        (
            DeckPath::NoteTypeCardTemplateAnswerFormat {
                note_type_id: note_type_id.clone(),
                template_id: template_id.clone(),
            },
            "note_types.note-type.country.card_templates.template.card-1.answer_format",
        ),
        (
            DeckPath::NoteTypeCardTemplateAdapterIds {
                note_type_id: note_type_id.clone(),
                template_id: template_id.clone(),
            },
            "note_types.note-type.country.card_templates.template.card-1.adapter_ids",
        ),
        (
            DeckPath::NoteTypeCardTemplateAdapterId {
                note_type_id: note_type_id.clone(),
                template_id: template_id.clone(),
                key: "crowdanki_uuid".to_owned(),
            },
            "note_types.note-type.country.card_templates.template.card-1.adapter_ids.crowdanki_uuid",
        ),
        (
            DeckPath::NoteTypeAdapterIds {
                note_type_id: note_type_id.clone(),
            },
            "note_types.note-type.country.adapter_ids",
        ),
        (
            DeckPath::NoteTypeAdapterId {
                note_type_id: note_type_id.clone(),
                key: "crowdanki_uuid".to_owned(),
            },
            "note_types.note-type.country.adapter_ids.crowdanki_uuid",
        ),
        (
            DeckPath::Note {
                note_id: note_id.clone(),
            },
            "notes.note.finland",
        ),
        (
            DeckPath::NoteId {
                note_id: note_id.clone(),
            },
            "notes.note.finland.id",
        ),
        (
            DeckPath::NoteNoteTypeId {
                note_id: note_id.clone(),
            },
            "notes.note.finland.note_type_id",
        ),
        (
            DeckPath::NoteVariables {
                note_id: note_id.clone(),
            },
            "notes.note.finland.variables",
        ),
        (
            DeckPath::NoteVariable {
                note_id: note_id.clone(),
                key: "hint.short".to_owned(),
            },
            "notes.note.finland.variables.hint.short",
        ),
        (
            DeckPath::NoteField {
                note_id: note_id.clone(),
                field_id: field_id.clone(),
            },
            "notes.note.finland.fields.field.capital",
        ),
        (
            DeckPath::NoteFieldMessage {
                note_id: note_id.clone(),
                field_id: field_id.clone(),
            },
            "notes.note.finland.fields.field.capital.message",
        ),
        (
            DeckPath::NoteFieldMessageComponent {
                note_id: note_id.clone(),
                field_id: field_id.clone(),
                index: 2,
            },
            "notes.note.finland.fields.field.capital.message.2",
        ),
        (
            DeckPath::NoteFieldMessageFormat {
                note_id: note_id.clone(),
                field_id: field_id.clone(),
            },
            "notes.note.finland.fields.field.capital.message.format",
        ),
        (
            DeckPath::NoteFieldMessageVariable {
                note_id: note_id.clone(),
                field_id: field_id.clone(),
                variable: "country.name".to_owned(),
            },
            "notes.note.finland.fields.field.capital.message.variables.country.name",
        ),
        (
            DeckPath::NoteTags {
                note_id: note_id.clone(),
            },
            "notes.note.finland.tags",
        ),
        (
            DeckPath::NoteTag {
                note_id: note_id.clone(),
                tag: "northern-europe".to_owned(),
            },
            "notes.note.finland.tags.northern-europe",
        ),
        (
            DeckPath::NoteAdapterIds {
                note_id: note_id.clone(),
            },
            "notes.note.finland.adapter_ids",
        ),
        (
            DeckPath::NoteAdapterId {
                note_id: note_id.clone(),
                key: "crowdanki_uuid".to_owned(),
            },
            "notes.note.finland.adapter_ids.crowdanki_uuid",
        ),
        (
            DeckPath::Media {
                media_id: media_id.clone(),
            },
            "media.media.flag.finland",
        ),
        (
            DeckPath::MediaId {
                media_id: media_id.clone(),
            },
            "media.media.flag.finland.id",
        ),
        (
            DeckPath::MediaPath {
                media_id: media_id.clone(),
            },
            "media.media.flag.finland.path",
        ),
        (
            DeckPath::MediaSha256 {
                media_id: media_id.clone(),
            },
            "media.media.flag.finland.sha256",
        ),
        (
            DeckPath::Tombstone {
                address: super::TombstoneAddress::Note {
                    note_id: note_id.clone(),
                },
            },
            "tombstones.notes.note.finland",
        ),
    ];

    for (path, text) in cases {
        assert_eq!(path.to_string(), text);
        assert_eq!(DeckPath::from_str(text).unwrap(), path);
    }
}

#[test]
fn deck_path_rejects_hostile_shapes() {
    for text in [
        "",
        "deck",
        "deck.unknown",
        "notes",
        "notes.",
        "notes..fields.field.capital",
        "notes.note.finland.fields",
        "notes.note.finland.fields.",
        "notes.note.finland.fields.field.capital.message.not-a-number",
        "notes.note.finland.fields.field.capital.message.2.extra",
        "note_types",
        "note_types.",
        "note_types.note-type.country.fields.",
        "note_types.note-type.country.card_templates.",
        "media",
        "media.",
        "tombstones",
        "tombstones.",
    ] {
        assert!(DeckPath::from_str(text).is_err(), "{text:?} should reject");
    }
}

#[test]
fn deck_path_documents_unescaped_dot_id_decision() {
    // The on-disk dotted syntax is already stable and existing fixtures use dotted StableIds
    // such as `note.finland` and `field.capital`. DeckPath therefore keeps those IDs
    // unescaped and treats only reserved grammar markers (for example `.fields.`) as
    // separators, preserving byte-identical serialized paths.
    let path = DeckPath::NoteField {
        note_id: sid("note.with.dots"),
        field_id: sid("field.with.dots"),
    };

    assert_eq!(
        path.to_string(),
        "notes.note.with.dots.fields.field.with.dots"
    );
    assert_eq!(DeckPath::from_str(&path.to_string()).unwrap(), path);
}

#[test]
fn deck_path_parses_non_stable_id_keys_by_first_container_split() {
    let path = DeckPath::NoteTypeVariable {
        note_type_id: sid("note-type.country"),
        key: "note-type.name".to_owned(),
    };

    assert_eq!(
        path.to_string(),
        "note_types.note-type.country.variables.note-type.name"
    );
    assert_eq!(DeckPath::from_str(&path.to_string()).unwrap(), path);
}

#[test]
fn glob_matches_table_driven_cases() {
    let cases = [
        ("literal-only match", "abc", "abc", true),
        ("literal-only mismatch", "abc", "ab", false),
        (
            "star at start matches prefix",
            "*world",
            "hello world",
            true,
        ),
        ("star at start can be empty", "*world", "world", true),
        ("star in middle matches run", "he*ld", "hello world", true),
        ("star in middle can be empty", "he*llo", "hello", true),
        ("star at end matches suffix", "hello*", "hello world", true),
        ("star at end can be empty", "hello*", "hello", true),
        (
            "multiple stars match ordered literals",
            "a*b*c",
            "axbyc",
            true,
        ),
        ("multiple stars can be empty", "a**b", "ab", true),
        (
            "multiple stars do not reorder literals",
            "a*b*c",
            "acb",
            false,
        ),
        (
            "multiple stars require repeated literals",
            "*a*a",
            "a",
            false,
        ),
        ("empty pattern matches empty input", "", "", true),
        ("empty pattern rejects non-empty input", "", "a", false),
        ("star matches empty input", "*", "", true),
        ("literal rejects empty input", "a", "", false),
        ("empty input with trailing star", "a*", "", false),
    ];

    for (case, pattern, value, expected) in cases {
        assert_eq!(glob_matches(pattern, value), expected, "{case}");
    }
}
