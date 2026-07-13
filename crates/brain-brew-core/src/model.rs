use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;

use crate::fingerprint::EntityFingerprint;

pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// Human-readable identity for a deck entity inside a CanonicalDeck.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StableId(String);

impl StableId {
    /// Create a stable ID after checking the conservative CanonicalDeck ID syntax.
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidStableId> {
        let value = value.into();
        let is_valid = !value.is_empty()
            && value
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | ':'));

        if is_valid {
            Ok(Self(value))
        } else {
            Err(InvalidStableId { value })
        }
    }

    /// Borrow the stable ID as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StableId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Error returned when text is not a valid stable ID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvalidStableId {
    value: String,
}

impl InvalidStableId {
    /// The rejected stable ID text.
    pub fn value(&self) -> &str {
        &self.value
    }
}

impl fmt::Display for InvalidStableId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid stable id {:?}", self.value)
    }
}

impl std::error::Error for InvalidStableId {}

/// A stable, deck-internal address rendered in the existing dotted on-disk syntax.
///
/// Dotted StableIds are intentionally preserved unescaped because current canonical
/// fixtures already use IDs such as `note.finland` and `field.capital`. The parser
/// treats the reserved grammar markers (`.fields.`, `.card_templates.`, and known
/// property suffixes) as separators while keeping ordinary dots inside IDs. To keep
/// this printer/parser boundary injective for StableId path segments, deck validation
/// rejects StableIds containing `..`, reserved container marker substrings
/// (`.fields.`, `.card_templates.`, `.variables.`, `.adapter_ids.`, `.tags.`,
/// `.images.`, `.message.`), or reserved property suffixes (`.id`, `.name`, `.styling`,
/// `.fields`, `.card_templates`, `.variables`, `.adapter_ids`, `.tags`, `.images`,
/// `.note_type_id`, `.message`, `.path`, `.sha256`, `.question_format`,
/// `.answer_format`). Non-StableId map keys and tag strings are exempt from that
/// StableId-only invariant, so keys such as `note-type.name` remain legal after
/// the first reserved container split.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeckPath {
    DeckId,
    DeckName,
    DeckDescription,
    DeckVariables,
    DeckVariable {
        key: String,
    },
    DeckAdapterIds,
    DeckAdapterId {
        key: String,
    },
    NoteType {
        note_type_id: StableId,
    },
    NoteTypeId {
        note_type_id: StableId,
    },
    NoteTypeName {
        note_type_id: StableId,
    },
    NoteTypeVariables {
        note_type_id: StableId,
    },
    NoteTypeVariable {
        note_type_id: StableId,
        key: String,
    },
    NoteTypeStyling {
        note_type_id: StableId,
    },
    NoteTypeFields {
        note_type_id: StableId,
    },
    NoteTypeField {
        note_type_id: StableId,
        field_id: StableId,
    },
    NoteTypeFieldId {
        note_type_id: StableId,
        field_id: StableId,
    },
    NoteTypeFieldName {
        note_type_id: StableId,
        field_id: StableId,
    },
    NoteTypeCardTemplates {
        note_type_id: StableId,
    },
    NoteTypeCardTemplate {
        note_type_id: StableId,
        template_id: StableId,
    },
    NoteTypeCardTemplateId {
        note_type_id: StableId,
        template_id: StableId,
    },
    NoteTypeCardTemplateName {
        note_type_id: StableId,
        template_id: StableId,
    },
    NoteTypeCardTemplateVariables {
        note_type_id: StableId,
        template_id: StableId,
    },
    NoteTypeCardTemplateVariable {
        note_type_id: StableId,
        template_id: StableId,
        key: String,
    },
    NoteTypeCardTemplateQuestionFormat {
        note_type_id: StableId,
        template_id: StableId,
    },
    NoteTypeCardTemplateAnswerFormat {
        note_type_id: StableId,
        template_id: StableId,
    },
    NoteTypeCardTemplateAdapterIds {
        note_type_id: StableId,
        template_id: StableId,
    },
    NoteTypeCardTemplateAdapterId {
        note_type_id: StableId,
        template_id: StableId,
        key: String,
    },
    NoteTypeAdapterIds {
        note_type_id: StableId,
    },
    NoteTypeAdapterId {
        note_type_id: StableId,
        key: String,
    },
    Note {
        note_id: StableId,
    },
    NoteId {
        note_id: StableId,
    },
    NoteNoteTypeId {
        note_id: StableId,
    },
    NoteVariables {
        note_id: StableId,
    },
    NoteVariable {
        note_id: StableId,
        key: String,
    },
    NoteField {
        note_id: StableId,
        field_id: StableId,
    },
    NoteFieldImage {
        note_id: StableId,
        field_id: StableId,
        index: usize,
    },
    NoteFieldMessage {
        note_id: StableId,
        field_id: StableId,
    },
    NoteFieldMessageComponent {
        note_id: StableId,
        field_id: StableId,
        index: usize,
    },
    NoteFieldMessageFormat {
        note_id: StableId,
        field_id: StableId,
    },
    NoteFieldMessageVariable {
        note_id: StableId,
        field_id: StableId,
        variable: String,
    },
    NoteTags {
        note_id: StableId,
    },
    NoteTag {
        note_id: StableId,
        tag: String,
    },
    NoteAdapterIds {
        note_id: StableId,
    },
    NoteAdapterId {
        note_id: StableId,
        key: String,
    },
    Media {
        media_id: StableId,
    },
    MediaId {
        media_id: StableId,
    },
    MediaPath {
        media_id: StableId,
    },
    MediaSha256 {
        media_id: StableId,
    },
    Tombstone {
        address: TombstoneAddress,
    },
}

impl fmt::Display for DeckPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeckId => f.write_str("deck.id"),
            Self::DeckName => f.write_str("deck.name"),
            Self::DeckDescription => f.write_str("deck.description"),
            Self::DeckVariables => f.write_str("deck.variables"),
            Self::DeckVariable { key } => write!(f, "deck.variables.{key}"),
            Self::DeckAdapterIds => f.write_str("deck.adapter_ids"),
            Self::DeckAdapterId { key } => write!(f, "deck.adapter_ids.{key}"),
            Self::NoteType { note_type_id } => write!(f, "note_types.{note_type_id}"),
            Self::NoteTypeId { note_type_id } => write!(f, "note_types.{note_type_id}.id"),
            Self::NoteTypeName { note_type_id } => write!(f, "note_types.{note_type_id}.name"),
            Self::NoteTypeVariables { note_type_id } => {
                write!(f, "note_types.{note_type_id}.variables")
            }
            Self::NoteTypeVariable { note_type_id, key } => {
                write!(f, "note_types.{note_type_id}.variables.{key}")
            }
            Self::NoteTypeStyling { note_type_id } => {
                write!(f, "note_types.{note_type_id}.styling")
            }
            Self::NoteTypeFields { note_type_id } => write!(f, "note_types.{note_type_id}.fields"),
            Self::NoteTypeField {
                note_type_id,
                field_id,
            } => write!(f, "note_types.{note_type_id}.fields.{field_id}"),
            Self::NoteTypeFieldId {
                note_type_id,
                field_id,
            } => write!(f, "note_types.{note_type_id}.fields.{field_id}.id"),
            Self::NoteTypeFieldName {
                note_type_id,
                field_id,
            } => write!(f, "note_types.{note_type_id}.fields.{field_id}.name"),
            Self::NoteTypeCardTemplates { note_type_id } => {
                write!(f, "note_types.{note_type_id}.card_templates")
            }
            Self::NoteTypeCardTemplate {
                note_type_id,
                template_id,
            } => write!(f, "note_types.{note_type_id}.card_templates.{template_id}"),
            Self::NoteTypeCardTemplateId {
                note_type_id,
                template_id,
            } => write!(
                f,
                "note_types.{note_type_id}.card_templates.{template_id}.id"
            ),
            Self::NoteTypeCardTemplateName {
                note_type_id,
                template_id,
            } => write!(
                f,
                "note_types.{note_type_id}.card_templates.{template_id}.name"
            ),
            Self::NoteTypeCardTemplateVariables {
                note_type_id,
                template_id,
            } => write!(
                f,
                "note_types.{note_type_id}.card_templates.{template_id}.variables"
            ),
            Self::NoteTypeCardTemplateVariable {
                note_type_id,
                template_id,
                key,
            } => write!(
                f,
                "note_types.{note_type_id}.card_templates.{template_id}.variables.{key}"
            ),
            Self::NoteTypeCardTemplateQuestionFormat {
                note_type_id,
                template_id,
            } => write!(
                f,
                "note_types.{note_type_id}.card_templates.{template_id}.question_format"
            ),
            Self::NoteTypeCardTemplateAnswerFormat {
                note_type_id,
                template_id,
            } => write!(
                f,
                "note_types.{note_type_id}.card_templates.{template_id}.answer_format"
            ),
            Self::NoteTypeCardTemplateAdapterIds {
                note_type_id,
                template_id,
            } => write!(
                f,
                "note_types.{note_type_id}.card_templates.{template_id}.adapter_ids"
            ),
            Self::NoteTypeCardTemplateAdapterId {
                note_type_id,
                template_id,
                key,
            } => write!(
                f,
                "note_types.{note_type_id}.card_templates.{template_id}.adapter_ids.{key}"
            ),
            Self::NoteTypeAdapterIds { note_type_id } => {
                write!(f, "note_types.{note_type_id}.adapter_ids")
            }
            Self::NoteTypeAdapterId { note_type_id, key } => {
                write!(f, "note_types.{note_type_id}.adapter_ids.{key}")
            }
            Self::Note { note_id } => write!(f, "notes.{note_id}"),
            Self::NoteId { note_id } => write!(f, "notes.{note_id}.id"),
            Self::NoteNoteTypeId { note_id } => write!(f, "notes.{note_id}.note_type_id"),
            Self::NoteVariables { note_id } => write!(f, "notes.{note_id}.variables"),
            Self::NoteVariable { note_id, key } => write!(f, "notes.{note_id}.variables.{key}"),
            Self::NoteField { note_id, field_id } => write!(f, "notes.{note_id}.fields.{field_id}"),
            Self::NoteFieldImage {
                note_id,
                field_id,
                index,
            } => write!(f, "notes.{note_id}.fields.{field_id}.images.{index}"),
            Self::NoteFieldMessage { note_id, field_id } => {
                write!(f, "notes.{note_id}.fields.{field_id}.message")
            }
            Self::NoteFieldMessageComponent {
                note_id,
                field_id,
                index,
            } => write!(f, "notes.{note_id}.fields.{field_id}.message.{index}"),
            Self::NoteFieldMessageFormat { note_id, field_id } => {
                write!(f, "notes.{note_id}.fields.{field_id}.message.format")
            }
            Self::NoteFieldMessageVariable {
                note_id,
                field_id,
                variable,
            } => write!(
                f,
                "notes.{note_id}.fields.{field_id}.message.variables.{variable}"
            ),
            Self::NoteTags { note_id } => write!(f, "notes.{note_id}.tags"),
            Self::NoteTag { note_id, tag } => write!(f, "notes.{note_id}.tags.{tag}"),
            Self::NoteAdapterIds { note_id } => write!(f, "notes.{note_id}.adapter_ids"),
            Self::NoteAdapterId { note_id, key } => write!(f, "notes.{note_id}.adapter_ids.{key}"),
            Self::Media { media_id } => write!(f, "media.{media_id}"),
            Self::MediaId { media_id } => write!(f, "media.{media_id}.id"),
            Self::MediaPath { media_id } => write!(f, "media.{media_id}.path"),
            Self::MediaSha256 { media_id } => write!(f, "media.{media_id}.sha256"),
            Self::Tombstone { address } => write!(f, "tombstones.{address}"),
        }
    }
}

impl FromStr for DeckPath {
    type Err = InvalidDeckPath;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        parse_deck_path(value).ok_or_else(|| InvalidDeckPath {
            value: value.to_owned(),
        })
    }
}

/// Error returned when text is not a valid DeckPath.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvalidDeckPath {
    value: String,
}

impl InvalidDeckPath {
    pub fn value(&self) -> &str {
        &self.value
    }
}

impl fmt::Display for InvalidDeckPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid deck path {:?}", self.value)
    }
}

impl std::error::Error for InvalidDeckPath {}

fn parse_deck_path(value: &str) -> Option<DeckPath> {
    match value {
        "deck.id" => return Some(DeckPath::DeckId),
        "deck.name" => return Some(DeckPath::DeckName),
        "deck.description" => return Some(DeckPath::DeckDescription),
        "deck.variables" => return Some(DeckPath::DeckVariables),
        "deck.adapter_ids" => return Some(DeckPath::DeckAdapterIds),
        _ => {}
    }

    if let Some(key) = value.strip_prefix("deck.variables.") {
        return non_empty_string(key).map(|key| DeckPath::DeckVariable { key });
    }
    if let Some(key) = value.strip_prefix("deck.adapter_ids.") {
        return non_empty_string(key).map(|key| DeckPath::DeckAdapterId { key });
    }
    if let Some(rest) = value.strip_prefix("note_types.") {
        return parse_note_type_path(rest);
    }
    if let Some(rest) = value.strip_prefix("notes.") {
        return parse_note_path(rest);
    }
    if let Some(rest) = value.strip_prefix("media.") {
        return parse_media_path(rest);
    }
    if let Some(rest) = value.strip_prefix("tombstones.") {
        let path = parse_deck_path(rest)?;
        return TombstoneAddress::try_from(path)
            .ok()
            .map(|address| DeckPath::Tombstone { address });
    }
    None
}

fn parse_note_type_path(rest: &str) -> Option<DeckPath> {
    if let Some((note_type_text, template_rest)) = rest.split_once(".card_templates.") {
        let note_type_id = stable_id(note_type_text)?;
        return parse_note_type_template_path(note_type_id, template_rest);
    }
    if let Some(note_type_text) = rest.strip_suffix(".card_templates") {
        return stable_id(note_type_text)
            .map(|note_type_id| DeckPath::NoteTypeCardTemplates { note_type_id });
    }
    if let Some((note_type_text, field_rest)) = rest.split_once(".fields.") {
        let note_type_id = stable_id(note_type_text)?;
        return parse_note_type_field_path(note_type_id, field_rest);
    }
    if let Some(note_type_text) = rest.strip_suffix(".fields") {
        return stable_id(note_type_text)
            .map(|note_type_id| DeckPath::NoteTypeFields { note_type_id });
    }
    if let Some((note_type_text, key)) = rest.split_once(".variables.") {
        let note_type_id = stable_id(note_type_text)?;
        return non_empty_string(key).map(|key| DeckPath::NoteTypeVariable { note_type_id, key });
    }
    if let Some(note_type_text) = rest.strip_suffix(".variables") {
        return stable_id(note_type_text)
            .map(|note_type_id| DeckPath::NoteTypeVariables { note_type_id });
    }
    if let Some((note_type_text, key)) = rest.split_once(".adapter_ids.") {
        let note_type_id = stable_id(note_type_text)?;
        return non_empty_string(key).map(|key| DeckPath::NoteTypeAdapterId { note_type_id, key });
    }
    if let Some(note_type_text) = rest.strip_suffix(".adapter_ids") {
        return stable_id(note_type_text)
            .map(|note_type_id| DeckPath::NoteTypeAdapterIds { note_type_id });
    }
    if let Some(note_type_text) = rest.strip_suffix(".styling") {
        return stable_id(note_type_text)
            .map(|note_type_id| DeckPath::NoteTypeStyling { note_type_id });
    }
    if let Some(note_type_text) = rest.strip_suffix(".name") {
        return stable_id(note_type_text)
            .map(|note_type_id| DeckPath::NoteTypeName { note_type_id });
    }
    if let Some(note_type_text) = rest.strip_suffix(".id") {
        return stable_id(note_type_text).map(|note_type_id| DeckPath::NoteTypeId { note_type_id });
    }
    stable_id(rest).map(|note_type_id| DeckPath::NoteType { note_type_id })
}

fn parse_note_type_field_path(note_type_id: StableId, rest: &str) -> Option<DeckPath> {
    if let Some(field_text) = rest.strip_suffix(".id") {
        return stable_id(field_text).map(|field_id| DeckPath::NoteTypeFieldId {
            note_type_id,
            field_id,
        });
    }
    if let Some(field_text) = rest.strip_suffix(".name") {
        return stable_id(field_text).map(|field_id| DeckPath::NoteTypeFieldName {
            note_type_id,
            field_id,
        });
    }
    stable_id(rest).map(|field_id| DeckPath::NoteTypeField {
        note_type_id,
        field_id,
    })
}

fn parse_note_type_template_path(note_type_id: StableId, rest: &str) -> Option<DeckPath> {
    if let Some(template_text) = rest.strip_suffix(".id") {
        let template_id = stable_id(template_text)?;
        return Some(DeckPath::NoteTypeCardTemplateId {
            note_type_id,
            template_id,
        });
    }
    if let Some((template_text, key)) = rest.split_once(".variables.") {
        let template_id = stable_id(template_text)?;
        return non_empty_string(key).map(|key| DeckPath::NoteTypeCardTemplateVariable {
            note_type_id,
            template_id,
            key,
        });
    }
    if let Some(template_text) = rest.strip_suffix(".variables") {
        let template_id = stable_id(template_text)?;
        return Some(DeckPath::NoteTypeCardTemplateVariables {
            note_type_id,
            template_id,
        });
    }
    if let Some((template_text, key)) = rest.split_once(".adapter_ids.") {
        let template_id = stable_id(template_text)?;
        return non_empty_string(key).map(|key| DeckPath::NoteTypeCardTemplateAdapterId {
            note_type_id,
            template_id,
            key,
        });
    }
    if let Some(template_text) = rest.strip_suffix(".adapter_ids") {
        let template_id = stable_id(template_text)?;
        return Some(DeckPath::NoteTypeCardTemplateAdapterIds {
            note_type_id,
            template_id,
        });
    }
    if let Some(template_text) = rest.strip_suffix(".question_format") {
        let template_id = stable_id(template_text)?;
        return Some(DeckPath::NoteTypeCardTemplateQuestionFormat {
            note_type_id,
            template_id,
        });
    }
    if let Some(template_text) = rest.strip_suffix(".answer_format") {
        let template_id = stable_id(template_text)?;
        return Some(DeckPath::NoteTypeCardTemplateAnswerFormat {
            note_type_id,
            template_id,
        });
    }
    if let Some(template_text) = rest.strip_suffix(".name") {
        let template_id = stable_id(template_text)?;
        return Some(DeckPath::NoteTypeCardTemplateName {
            note_type_id,
            template_id,
        });
    }
    stable_id(rest).map(|template_id| DeckPath::NoteTypeCardTemplate {
        note_type_id,
        template_id,
    })
}

fn parse_note_path(rest: &str) -> Option<DeckPath> {
    if let Some((note_text, field_rest)) = rest.split_once(".fields.") {
        let note_id = stable_id(note_text)?;
        return parse_note_field_path(note_id, field_rest);
    }
    if rest.ends_with(".fields") {
        return None;
    }
    if let Some((note_text, key)) = rest.split_once(".variables.") {
        let note_id = stable_id(note_text)?;
        return non_empty_string(key).map(|key| DeckPath::NoteVariable { note_id, key });
    }
    if let Some(note_text) = rest.strip_suffix(".variables") {
        return stable_id(note_text).map(|note_id| DeckPath::NoteVariables { note_id });
    }
    if let Some((note_text, tag)) = rest.split_once(".tags.") {
        let note_id = stable_id(note_text)?;
        return non_empty_string(tag).map(|tag| DeckPath::NoteTag { note_id, tag });
    }
    if let Some(note_text) = rest.strip_suffix(".tags") {
        return stable_id(note_text).map(|note_id| DeckPath::NoteTags { note_id });
    }
    if let Some((note_text, key)) = rest.split_once(".adapter_ids.") {
        let note_id = stable_id(note_text)?;
        return non_empty_string(key).map(|key| DeckPath::NoteAdapterId { note_id, key });
    }
    if let Some(note_text) = rest.strip_suffix(".adapter_ids") {
        return stable_id(note_text).map(|note_id| DeckPath::NoteAdapterIds { note_id });
    }
    if let Some(note_text) = rest.strip_suffix(".note_type_id") {
        return stable_id(note_text).map(|note_id| DeckPath::NoteNoteTypeId { note_id });
    }
    if let Some(note_text) = rest.strip_suffix(".id") {
        return stable_id(note_text).map(|note_id| DeckPath::NoteId { note_id });
    }
    stable_id(rest).map(|note_id| DeckPath::Note { note_id })
}

fn parse_note_field_path(note_id: StableId, rest: &str) -> Option<DeckPath> {
    if let Some((field_text, index_text)) = rest.rsplit_once(".images.") {
        let field_id = stable_id(field_text)?;
        let index = index_text.parse::<usize>().ok()?;
        return Some(DeckPath::NoteFieldImage {
            note_id,
            field_id,
            index,
        });
    }
    if let Some((field_text, variable)) = rest.split_once(".message.variables.") {
        let field_id = stable_id(field_text)?;
        return non_empty_string(variable).map(|variable| DeckPath::NoteFieldMessageVariable {
            note_id,
            field_id,
            variable,
        });
    }
    if let Some(field_text) = rest.strip_suffix(".message.format") {
        let field_id = stable_id(field_text)?;
        return Some(DeckPath::NoteFieldMessageFormat { note_id, field_id });
    }
    if let Some((field_text, index_text)) = rest.rsplit_once(".message.") {
        let field_id = stable_id(field_text)?;
        let index = index_text.parse::<usize>().ok()?;
        return Some(DeckPath::NoteFieldMessageComponent {
            note_id,
            field_id,
            index,
        });
    }
    if let Some(field_text) = rest.strip_suffix(".message") {
        let field_id = stable_id(field_text)?;
        return Some(DeckPath::NoteFieldMessage { note_id, field_id });
    }
    stable_id(rest).map(|field_id| DeckPath::NoteField { note_id, field_id })
}

fn parse_media_path(rest: &str) -> Option<DeckPath> {
    if let Some(media_text) = rest.strip_suffix(".sha256") {
        return stable_id(media_text).map(|media_id| DeckPath::MediaSha256 { media_id });
    }
    if let Some(media_text) = rest.strip_suffix(".path") {
        return stable_id(media_text).map(|media_id| DeckPath::MediaPath { media_id });
    }
    if let Some(media_text) = rest.strip_suffix(".id") {
        return stable_id(media_text).map(|media_id| DeckPath::MediaId { media_id });
    }
    stable_id(rest).map(|media_id| DeckPath::Media { media_id })
}

fn stable_id(value: &str) -> Option<StableId> {
    if value.contains("..") {
        return None;
    }
    StableId::new(value.to_owned()).ok()
}

fn non_empty_string(value: &str) -> Option<String> {
    (!value.is_empty() && !value.contains("..")).then(|| value.to_owned())
}

/// Adapter-specific identities keyed by adapter namespace.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdapterIds(BTreeMap<String, String>);

impl AdapterIds {
    /// Create an empty adapter ID collection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert an adapter identity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> Option<String> {
        self.0.insert(key.into(), value.into())
    }

    /// Look up an adapter identity by namespace key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    /// Returns true when an adapter identity exists for the namespace key.
    pub fn contains_key(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    /// Remove an adapter identity by namespace key.
    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.0.remove(key)
    }

    /// Iterate adapter identities in deterministic key order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
    }

    /// Returns true when no adapter identities are present.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// An exact, typed address whose removal identity must not be reused.
///
/// Every variant contains its complete parent scope, so invalid partial nested
/// addresses cannot be represented. New removable paths must add an explicit
/// variant here before they can create a tombstone.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TombstoneAddress {
    DeckName,
    DeckDescription,
    DeckVariable {
        key: String,
    },
    DeckAdapterId {
        key: String,
    },
    NoteType {
        note_type_id: StableId,
    },
    NoteTypeName {
        note_type_id: StableId,
    },
    NoteTypeVariable {
        note_type_id: StableId,
        key: String,
    },
    NoteTypeStyling {
        note_type_id: StableId,
    },
    NoteTypeAdapterId {
        note_type_id: StableId,
        key: String,
    },
    FieldDefinition {
        note_type_id: StableId,
        field_id: StableId,
    },
    CardTemplate {
        note_type_id: StableId,
        template_id: StableId,
    },
    CardTemplateName {
        note_type_id: StableId,
        template_id: StableId,
    },
    CardTemplateVariable {
        note_type_id: StableId,
        template_id: StableId,
        key: String,
    },
    CardTemplateQuestionFormat {
        note_type_id: StableId,
        template_id: StableId,
    },
    CardTemplateAnswerFormat {
        note_type_id: StableId,
        template_id: StableId,
    },
    CardTemplateAdapterId {
        note_type_id: StableId,
        template_id: StableId,
        key: String,
    },
    Note {
        note_id: StableId,
    },
    NoteVariable {
        note_id: StableId,
        key: String,
    },
    NoteField {
        note_id: StableId,
        field_id: StableId,
    },
    NoteTag {
        note_id: StableId,
        tag: String,
    },
    NoteAdapterId {
        note_id: StableId,
        key: String,
    },
    MediaReference {
        media_id: StableId,
    },
}

impl TombstoneAddress {
    /// The canonical entity/value kind used by YAML and diagnostics.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::DeckName => "deck_name",
            Self::DeckDescription => "deck_description",
            Self::DeckVariable { .. } => "deck_variable",
            Self::DeckAdapterId { .. } => "deck_adapter_id",
            Self::NoteType { .. } => "note_type",
            Self::NoteTypeName { .. } => "note_type_name",
            Self::NoteTypeVariable { .. } => "note_type_variable",
            Self::NoteTypeStyling { .. } => "note_type_styling",
            Self::NoteTypeAdapterId { .. } => "note_type_adapter_id",
            Self::FieldDefinition { .. } => "field_definition",
            Self::CardTemplate { .. } => "card_template",
            Self::CardTemplateName { .. } => "card_template_name",
            Self::CardTemplateVariable { .. } => "card_template_variable",
            Self::CardTemplateQuestionFormat { .. } => "card_template_question_format",
            Self::CardTemplateAnswerFormat { .. } => "card_template_answer_format",
            Self::CardTemplateAdapterId { .. } => "card_template_adapter_id",
            Self::Note { .. } => "note",
            Self::NoteVariable { .. } => "note_variable",
            Self::NoteField { .. } => "note_field",
            Self::NoteTag { .. } => "note_tag",
            Self::NoteAdapterId { .. } => "note_adapter_id",
            Self::MediaReference { .. } => "media_reference",
        }
    }

    /// Convert to the canonical deck path for this exact address.
    pub fn deck_path(&self) -> DeckPath {
        match self {
            Self::DeckName => DeckPath::DeckName,
            Self::DeckDescription => DeckPath::DeckDescription,
            Self::DeckVariable { key } => DeckPath::DeckVariable { key: key.clone() },
            Self::DeckAdapterId { key } => DeckPath::DeckAdapterId { key: key.clone() },
            Self::NoteType { note_type_id } => DeckPath::NoteType {
                note_type_id: note_type_id.clone(),
            },
            Self::NoteTypeName { note_type_id } => DeckPath::NoteTypeName {
                note_type_id: note_type_id.clone(),
            },
            Self::NoteTypeVariable { note_type_id, key } => DeckPath::NoteTypeVariable {
                note_type_id: note_type_id.clone(),
                key: key.clone(),
            },
            Self::NoteTypeStyling { note_type_id } => DeckPath::NoteTypeStyling {
                note_type_id: note_type_id.clone(),
            },
            Self::NoteTypeAdapterId { note_type_id, key } => DeckPath::NoteTypeAdapterId {
                note_type_id: note_type_id.clone(),
                key: key.clone(),
            },
            Self::FieldDefinition {
                note_type_id,
                field_id,
            } => DeckPath::NoteTypeField {
                note_type_id: note_type_id.clone(),
                field_id: field_id.clone(),
            },
            Self::CardTemplate {
                note_type_id,
                template_id,
            } => DeckPath::NoteTypeCardTemplate {
                note_type_id: note_type_id.clone(),
                template_id: template_id.clone(),
            },
            Self::CardTemplateName {
                note_type_id,
                template_id,
            } => DeckPath::NoteTypeCardTemplateName {
                note_type_id: note_type_id.clone(),
                template_id: template_id.clone(),
            },
            Self::CardTemplateVariable {
                note_type_id,
                template_id,
                key,
            } => DeckPath::NoteTypeCardTemplateVariable {
                note_type_id: note_type_id.clone(),
                template_id: template_id.clone(),
                key: key.clone(),
            },
            Self::CardTemplateQuestionFormat {
                note_type_id,
                template_id,
            } => DeckPath::NoteTypeCardTemplateQuestionFormat {
                note_type_id: note_type_id.clone(),
                template_id: template_id.clone(),
            },
            Self::CardTemplateAnswerFormat {
                note_type_id,
                template_id,
            } => DeckPath::NoteTypeCardTemplateAnswerFormat {
                note_type_id: note_type_id.clone(),
                template_id: template_id.clone(),
            },
            Self::CardTemplateAdapterId {
                note_type_id,
                template_id,
                key,
            } => DeckPath::NoteTypeCardTemplateAdapterId {
                note_type_id: note_type_id.clone(),
                template_id: template_id.clone(),
                key: key.clone(),
            },
            Self::Note { note_id } => DeckPath::Note {
                note_id: note_id.clone(),
            },
            Self::NoteVariable { note_id, key } => DeckPath::NoteVariable {
                note_id: note_id.clone(),
                key: key.clone(),
            },
            Self::NoteField { note_id, field_id } => DeckPath::NoteField {
                note_id: note_id.clone(),
                field_id: field_id.clone(),
            },
            Self::NoteTag { note_id, tag } => DeckPath::NoteTag {
                note_id: note_id.clone(),
                tag: tag.clone(),
            },
            Self::NoteAdapterId { note_id, key } => DeckPath::NoteAdapterId {
                note_id: note_id.clone(),
                key: key.clone(),
            },
            Self::MediaReference { media_id } => DeckPath::Media {
                media_id: media_id.clone(),
            },
        }
    }

    /// True when `self` is the same address or is structurally contained by `ancestor`.
    pub fn is_same_or_descendant_of(&self, ancestor: &Self) -> bool {
        if self == ancestor {
            return true;
        }
        match (self, ancestor) {
            (
                Self::NoteTypeName {
                    note_type_id: child,
                }
                | Self::NoteTypeVariable {
                    note_type_id: child,
                    ..
                }
                | Self::NoteTypeStyling {
                    note_type_id: child,
                }
                | Self::NoteTypeAdapterId {
                    note_type_id: child,
                    ..
                }
                | Self::FieldDefinition {
                    note_type_id: child,
                    ..
                }
                | Self::CardTemplate {
                    note_type_id: child,
                    ..
                }
                | Self::CardTemplateName {
                    note_type_id: child,
                    ..
                }
                | Self::CardTemplateVariable {
                    note_type_id: child,
                    ..
                }
                | Self::CardTemplateQuestionFormat {
                    note_type_id: child,
                    ..
                }
                | Self::CardTemplateAnswerFormat {
                    note_type_id: child,
                    ..
                }
                | Self::CardTemplateAdapterId {
                    note_type_id: child,
                    ..
                },
                Self::NoteType {
                    note_type_id: parent,
                },
            ) => child == parent,
            (
                Self::CardTemplateName {
                    note_type_id: child_type,
                    template_id: child_template,
                }
                | Self::CardTemplateVariable {
                    note_type_id: child_type,
                    template_id: child_template,
                    ..
                }
                | Self::CardTemplateQuestionFormat {
                    note_type_id: child_type,
                    template_id: child_template,
                }
                | Self::CardTemplateAnswerFormat {
                    note_type_id: child_type,
                    template_id: child_template,
                }
                | Self::CardTemplateAdapterId {
                    note_type_id: child_type,
                    template_id: child_template,
                    ..
                },
                Self::CardTemplate {
                    note_type_id: parent_type,
                    template_id: parent_template,
                },
            ) => child_type == parent_type && child_template == parent_template,
            (
                Self::NoteVariable { note_id: child, .. }
                | Self::NoteField { note_id: child, .. }
                | Self::NoteTag { note_id: child, .. }
                | Self::NoteAdapterId { note_id: child, .. },
                Self::Note { note_id: parent },
            ) => child == parent,
            _ => false,
        }
    }
}

impl fmt::Display for TombstoneAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.deck_path().fmt(f)
    }
}

impl TryFrom<DeckPath> for TombstoneAddress {
    type Error = InvalidDeckPath;

    fn try_from(path: DeckPath) -> Result<Self, Self::Error> {
        let original = path.to_string();
        let address = match path {
            DeckPath::DeckName => Self::DeckName,
            DeckPath::DeckDescription => Self::DeckDescription,
            DeckPath::DeckVariable { key } => Self::DeckVariable { key },
            DeckPath::DeckAdapterId { key } => Self::DeckAdapterId { key },
            DeckPath::NoteType { note_type_id } => Self::NoteType { note_type_id },
            DeckPath::NoteTypeName { note_type_id } => Self::NoteTypeName { note_type_id },
            DeckPath::NoteTypeVariable { note_type_id, key } => {
                Self::NoteTypeVariable { note_type_id, key }
            }
            DeckPath::NoteTypeStyling { note_type_id } => Self::NoteTypeStyling { note_type_id },
            DeckPath::NoteTypeAdapterId { note_type_id, key } => {
                Self::NoteTypeAdapterId { note_type_id, key }
            }
            DeckPath::NoteTypeField {
                note_type_id,
                field_id,
            } => Self::FieldDefinition {
                note_type_id,
                field_id,
            },
            DeckPath::NoteTypeCardTemplate {
                note_type_id,
                template_id,
            } => Self::CardTemplate {
                note_type_id,
                template_id,
            },
            DeckPath::NoteTypeCardTemplateName {
                note_type_id,
                template_id,
            } => Self::CardTemplateName {
                note_type_id,
                template_id,
            },
            DeckPath::NoteTypeCardTemplateVariable {
                note_type_id,
                template_id,
                key,
            } => Self::CardTemplateVariable {
                note_type_id,
                template_id,
                key,
            },
            DeckPath::NoteTypeCardTemplateQuestionFormat {
                note_type_id,
                template_id,
            } => Self::CardTemplateQuestionFormat {
                note_type_id,
                template_id,
            },
            DeckPath::NoteTypeCardTemplateAnswerFormat {
                note_type_id,
                template_id,
            } => Self::CardTemplateAnswerFormat {
                note_type_id,
                template_id,
            },
            DeckPath::NoteTypeCardTemplateAdapterId {
                note_type_id,
                template_id,
                key,
            } => Self::CardTemplateAdapterId {
                note_type_id,
                template_id,
                key,
            },
            DeckPath::Note { note_id } => Self::Note { note_id },
            DeckPath::NoteVariable { note_id, key } => Self::NoteVariable { note_id, key },
            DeckPath::NoteField { note_id, field_id } => Self::NoteField { note_id, field_id },
            DeckPath::NoteTag { note_id, tag } => Self::NoteTag { note_id, tag },
            DeckPath::NoteAdapterId { note_id, key } => Self::NoteAdapterId { note_id, key },
            DeckPath::Media { media_id } => Self::MediaReference { media_id },
            _ => return Err(InvalidDeckPath { value: original }),
        };
        Ok(address)
    }
}

/// Provenance for a removal produced during ordered overlay composition.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RemovalProvenance {
    pub overlay_id: StableId,
    pub operation: ChangeIntent,
}

/// One typed removal record. Legacy canonical records have no provenance;
/// composition-created records always identify the removing overlay and operation.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TombstoneRecord {
    pub address: TombstoneAddress,
    pub provenance: Option<RemovalProvenance>,
}

impl TombstoneRecord {
    pub fn removed_by(address: TombstoneAddress, overlay_id: StableId) -> Self {
        Self {
            address,
            provenance: Some(RemovalProvenance {
                overlay_id,
                operation: ChangeIntent::Remove,
            }),
        }
    }

    pub fn legacy(address: TombstoneAddress) -> Self {
        Self {
            address,
            provenance: None,
        }
    }
}

/// Address-keyed tombstones with deterministic ordering and unique exact addresses.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Tombstones(BTreeMap<TombstoneAddress, TombstoneRecord>);

impl Tombstones {
    pub fn insert(&mut self, record: TombstoneRecord) -> Option<TombstoneRecord> {
        self.0.insert(record.address.clone(), record)
    }

    pub fn get(&self, address: &TombstoneAddress) -> Option<&TombstoneRecord> {
        self.0.get(address)
    }

    pub fn blocking(&self, address: &TombstoneAddress) -> Option<&TombstoneRecord> {
        self.0
            .values()
            .find(|record| address.is_same_or_descendant_of(&record.address))
    }

    pub fn contains_address(&self, address: &TombstoneAddress) -> bool {
        self.0.contains_key(address)
    }

    pub fn iter(&self) -> impl Iterator<Item = &TombstoneRecord> {
        self.0.values()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn clear(&mut self) {
        self.0.clear();
    }
}

/// The format-independent representation of a deck's content and structure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalDeck {
    pub id: StableId,
    pub name: String,
    pub description: String,
    pub variables: BTreeMap<String, String>,
    pub note_types: BTreeMap<StableId, NoteType>,
    pub notes: BTreeMap<StableId, Note>,
    pub media: BTreeMap<StableId, MediaReference>,
    pub tombstones: Tombstones,
    pub adapter_ids: AdapterIds,
}

/// A sparse CanonicalDeck-shaped fragment applied to a base deck.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Overlay {
    pub id: StableId,
    pub kind: OverlayKind,
    pub translations: Option<TranslationDictionary>,
    pub deck_change: Option<DeckChange>,
    pub note_changes: BTreeMap<StableId, NoteChange>,
    pub note_type_changes: BTreeMap<StableId, NoteTypeChange>,
    pub media_changes: BTreeMap<StableId, MediaChange>,
}

/// Translation dictionary applied by a translation overlay.
///
/// The fields remain public for compatibility with the canonical domain model.
/// New mutations must use the validated transactional methods on this type;
/// core composition and every format decode/emit boundary call
/// `validate_mutation_invariants` before accepting these compatibility fields.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TranslationDictionary {
    /// Direct reusable replacements keyed by exact non-empty source text.
    pub direct: BTreeMap<String, String>,
    /// Contextual replacements keyed by stable deck path prefix and exact source text.
    pub contextual: BTreeMap<String, BTreeMap<String, String>>,
    /// Exact non-empty source strings reviewed as intentionally unchanged.
    pub no_change: BTreeSet<String>,
    /// Path-scoped target-language adaptations that may intentionally diverge from source wording.
    pub target_adaptations: BTreeMap<String, TargetAdaptation>,
    /// Persisted source-change review records that apply prior target text while warning.
    pub stale_translations: Vec<StaleTranslation>,
    /// Variable-specific source text to translated text replacements by variable key.
    pub variables: BTreeMap<String, BTreeMap<String, String>>,
    /// Adapter-specific source ID to translated ID replacements by adapter namespace.
    pub adapter_ids: BTreeMap<String, BTreeMap<String, String>>,
    /// When true, every extracted translatable string must be translated or ignored.
    pub require_complete: bool,
    /// Glob-style paths ignored by complete-coverage checks.
    pub ignore_paths: BTreeSet<String>,
}

/// Intentional path-scoped target-language wording that may diverge from the source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetAdaptation {
    pub expected_source: String,
    pub target: String,
    pub reason: Option<String>,
}

/// Persisted translation review debt created when source text changes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaleTranslation {
    pub old_source: String,
    pub new_source: String,
    pub target: String,
    pub context: Option<String>,
}

/// Non-mutating coverage report for one translation overlay applied to a source deck.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationCoverageReport {
    pub overlay_id: StableId,
    pub entries: Vec<TranslationCoverageEntry>,
}

impl TranslationCoverageReport {
    /// Entries that indicate untranslated fallback output or stale/invalid dictionary keys.
    pub fn problem_entries(&self) -> impl Iterator<Item = &TranslationCoverageEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.category.is_problem())
    }

    /// Returns true when at least one extracted source string would fall back untranslated.
    pub fn has_untranslated_fallbacks(&self) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.category == TranslationCoverageCategory::UntranslatedFallback)
    }

    /// Returns true when the dictionary contains stale or invalid entries.
    pub fn has_stale_or_invalid_entries(&self) -> bool {
        self.entries.iter().any(|entry| {
            matches!(
                entry.category,
                TranslationCoverageCategory::StaleTranslation
                    | TranslationCoverageCategory::StaleDirectKey
                    | TranslationCoverageCategory::StaleContextualKey
                    | TranslationCoverageCategory::StaleNoChangeKey
                    | TranslationCoverageCategory::StaleTargetAdaptation
                    | TranslationCoverageCategory::StaleVariableKey
                    | TranslationCoverageCategory::StaleAdapterIdKey
                    | TranslationCoverageCategory::InvalidTargetAdaptation
                    | TranslationCoverageCategory::StructuralFieldNameTranslation
            )
        })
    }
}

/// One source string, dictionary entry, or fallback in a translation coverage report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationCoverageEntry {
    pub category: TranslationCoverageCategory,
    pub path: String,
    pub source: String,
    pub old_source: Option<String>,
    pub translated: Option<String>,
    pub context: Option<String>,
}

/// Translator-facing contextual view for one translation coverage report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationContextView {
    pub overlay_id: StableId,
    pub units: Vec<TranslationContextUnit>,
}

/// One translatable unit annotated with note, field, card template, and status context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationContextUnit {
    pub category: TranslationCoverageCategory,
    pub path: String,
    pub source: String,
    pub old_source: Option<String>,
    pub translated: Option<String>,
    pub context: Option<String>,
    pub note_id: Option<StableId>,
    pub note_type_id: Option<StableId>,
    pub field_id: Option<StableId>,
    pub field_name: Option<String>,
    pub note_fields: Vec<TranslationNoteFieldContext>,
    pub message: Option<TranslationMessageContext>,
    pub card_templates: Vec<TranslationCardContext>,
    pub source_occurrences: usize,
}

/// Structured-message context for a note field shown around a translatable unit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationMessageContext {
    pub source: String,
    pub translated: String,
    pub format: Option<TranslationMessageComponentContext>,
    pub components: Vec<TranslationMessageComponentContext>,
}

/// One component of a structured message shown in translator context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationMessageComponentContext {
    pub index: usize,
    pub name: Option<String>,
    pub kind: MessageComponentKind,
    pub path: String,
    pub source: String,
    pub translated: String,
    pub reference: Option<String>,
    pub category: Option<TranslationCoverageCategory>,
}

/// Sibling note-field context shown around a translatable unit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationNoteFieldContext {
    pub field_id: StableId,
    pub field_name: String,
    pub source: String,
    pub translated: String,
    pub category: Option<TranslationCoverageCategory>,
}

/// Card/template context in which a note field can be seen by a translator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationCardContext {
    pub template_id: StableId,
    pub template_name: String,
    pub sides: BTreeSet<CardTemplateSide>,
    /// Near-rendered card front template snippet where this string may appear.
    pub question_format: String,
    /// Near-rendered card back template snippet where this string may appear.
    pub answer_format: String,
}

/// Card side where a translated field appears.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CardTemplateSide {
    Question,
    Answer,
}

impl CardTemplateSide {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Question => "question",
            Self::Answer => "answer",
        }
    }
}

/// Maintainer-facing classification for translation coverage entries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranslationCoverageCategory {
    DirectTranslation,
    ContextualTranslation,
    NoChange,
    TargetAdaptation,
    VariableTranslation,
    AdapterIdTranslation,
    IgnoredSource,
    StaleTranslation,
    UntranslatedFallback,
    StaleDirectKey,
    StaleContextualKey,
    StaleNoChangeKey,
    StaleTargetAdaptation,
    StaleVariableKey,
    StaleAdapterIdKey,
    InvalidTargetAdaptation,
    StructuralFieldNameTranslation,
}

impl TranslationCoverageCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DirectTranslation => "direct_translation",
            Self::ContextualTranslation => "contextual_translation",
            Self::NoChange => "no_change",
            Self::TargetAdaptation => "target_adaptation",
            Self::VariableTranslation => "variable_translation",
            Self::AdapterIdTranslation => "adapter_id_translation",
            Self::IgnoredSource => "ignored_source",
            Self::StaleTranslation => "stale_translation",
            Self::UntranslatedFallback => "untranslated_fallback",
            Self::StaleDirectKey => "stale_direct_key",
            Self::StaleContextualKey => "stale_contextual_key",
            Self::StaleNoChangeKey => "stale_no_change_key",
            Self::StaleTargetAdaptation => "stale_target_adaptation",
            Self::StaleVariableKey => "stale_variable_key",
            Self::StaleAdapterIdKey => "stale_adapter_id_key",
            Self::InvalidTargetAdaptation => "invalid_target_adaptation",
            Self::StructuralFieldNameTranslation => "structural_field_name_translation",
        }
    }

    pub fn is_problem(self) -> bool {
        matches!(
            self,
            Self::UntranslatedFallback
                | Self::StaleTranslation
                | Self::StaleDirectKey
                | Self::StaleContextualKey
                | Self::StaleNoChangeKey
                | Self::StaleTargetAdaptation
                | Self::StaleVariableKey
                | Self::StaleAdapterIdKey
                | Self::InvalidTargetAdaptation
                | Self::StructuralFieldNameTranslation
        )
    }
}

/// Maintainer-facing category for an overlay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OverlayKind {
    Translation,
    Extension,
    Patch,
    Personal,
}

/// The declared meaning of an overlay change.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ChangeIntent {
    Add,
    Merge,
    Replace,
    Remove,
    Override,
}

impl ChangeIntent {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Merge => "merge",
            Self::Replace => "replace",
            Self::Remove => "remove",
            Self::Override => "override",
        }
    }
}

/// Exact prior state an overlay expects before applying a destructive change.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExpectedBase {
    /// A scalar property value. For note fields this matches only a scalar field value.
    Value(String),
    /// A structured note-field value expected atomically.
    FieldValue(FieldValue),
    /// A stable fingerprint for a complete entity replacement, override, or removal.
    EntityFingerprint(EntityFingerprint),
    /// Legacy presence-only marker retained for migration diagnostics. It never authorizes a
    /// destructive operation; canonical YAML authors must replace it with an exact value or a
    /// tooling-generated entity fingerprint.
    EntityPresent,
}

/// Complete entity families protected by canonical fingerprints.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntityKind {
    NoteType,
    FieldDefinition,
    CardTemplate,
    Note,
    MediaReference,
}

impl EntityKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NoteType => "note_type",
            Self::FieldDefinition => "field_definition",
            Self::CardTemplate => "card_template",
            Self::Note => "note",
            Self::MediaReference => "media_reference",
        }
    }
}

/// Sparse change for deck-level metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeckChange {
    pub name: Option<PropertyChange>,
    pub description: Option<PropertyChange>,
    pub variables: BTreeMap<String, PropertyChange>,
    pub adapter_ids: BTreeMap<String, AdapterIdChange>,
}

/// Sparse change for a scalar string property.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PropertyChange {
    pub intent: ChangeIntent,
    pub value: Option<String>,
    pub expected_base: Option<ExpectedBase>,
}

/// Sparse change for one adapter identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterIdChange {
    pub intent: ChangeIntent,
    pub value: Option<String>,
    pub expected_base: Option<ExpectedBase>,
}

/// Sparse change for one note type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoteTypeChange {
    pub intent: ChangeIntent,
    pub note_type: Option<NoteType>,
    pub name: Option<PropertyChange>,
    pub variables: BTreeMap<String, PropertyChange>,
    pub styling: Option<PropertyChange>,
    pub fields: BTreeMap<StableId, FieldDefinitionChange>,
    pub card_templates: BTreeMap<StableId, CardTemplateChange>,
    pub adapter_ids: BTreeMap<String, AdapterIdChange>,
    pub expected_base: Option<ExpectedBase>,
}

/// Sparse change for one card template.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CardTemplateChange {
    pub intent: ChangeIntent,
    pub template: Option<CardTemplate>,
    pub insert_after: Option<StableId>,
    pub name: Option<PropertyChange>,
    pub variables: BTreeMap<String, PropertyChange>,
    pub question_format: Option<PropertyChange>,
    pub answer_format: Option<PropertyChange>,
    pub adapter_ids: BTreeMap<String, AdapterIdChange>,
    pub expected_base: Option<ExpectedBase>,
}

/// Sparse change for one note type field definition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldDefinitionChange {
    pub intent: ChangeIntent,
    pub field: Option<FieldDefinition>,
    pub expected_base: Option<ExpectedBase>,
}

/// Sparse change for one note.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoteChange {
    pub intent: ChangeIntent,
    pub note: Option<Note>,
    pub variables: BTreeMap<String, PropertyChange>,
    pub fields: BTreeMap<StableId, FieldChange>,
    pub tags: BTreeMap<String, TagChange>,
    pub adapter_ids: BTreeMap<String, AdapterIdChange>,
    pub expected_base: Option<ExpectedBase>,
}

/// Sparse change for one note tag.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TagChange {
    pub intent: ChangeIntent,
    pub expected_base: Option<ExpectedBase>,
}

/// Sparse change for one media reference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaChange {
    pub intent: ChangeIntent,
    pub media: Option<MediaReference>,
    pub expected_base: Option<ExpectedBase>,
}

/// Sparse change for one note field value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldChange {
    pub intent: ChangeIntent,
    /// The complete semantic value. `None` is valid only for removal.
    pub value: Option<FieldValue>,
    pub expected_base: Option<ExpectedBase>,
}

/// An Anki-compatible note type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoteType {
    pub id: StableId,
    pub name: String,
    pub variables: BTreeMap<String, String>,
    pub fields: Vec<FieldDefinition>,
    pub card_templates: Vec<CardTemplate>,
    pub styling: String,
    pub adapter_ids: AdapterIds,
}

/// A field declared by a note type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldDefinition {
    pub id: StableId,
    pub name: String,
}

/// Raw Anki-compatible card template text plus identity metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CardTemplate {
    pub id: StableId,
    pub name: String,
    pub variables: BTreeMap<String, String>,
    pub question_format: String,
    pub answer_format: String,
    pub adapter_ids: AdapterIds,
}

/// A note belonging to a note type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Note {
    pub id: StableId,
    pub note_type_id: StableId,
    pub variables: BTreeMap<String, String>,
    /// Exactly one semantic representation for each declared note field.
    pub fields: FieldMap,
    pub tags: BTreeSet<String>,
    pub adapter_ids: AdapterIds,
}

/// A note's single map of semantic field values.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FieldMap(BTreeMap<StableId, FieldValue>);

impl FieldMap {
    pub fn insert(
        &mut self,
        field_id: StableId,
        value: impl Into<FieldValue>,
    ) -> Option<FieldValue> {
        self.0.insert(field_id, value.into())
    }
}

impl Deref for FieldMap {
    type Target = BTreeMap<StableId, FieldValue>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for FieldMap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<BTreeMap<StableId, FieldValue>> for FieldMap {
    fn from(fields: BTreeMap<StableId, FieldValue>) -> Self {
        Self(fields)
    }
}

impl From<BTreeMap<StableId, String>> for FieldMap {
    fn from(fields: BTreeMap<StableId, String>) -> Self {
        Self(
            fields
                .into_iter()
                .map(|(field_id, value)| (field_id, FieldValue::Scalar(value)))
                .collect(),
        )
    }
}

impl FromIterator<(StableId, FieldValue)> for FieldMap {
    fn from_iter<T: IntoIterator<Item = (StableId, FieldValue)>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl<'a> IntoIterator for &'a FieldMap {
    type Item = (&'a StableId, &'a FieldValue);
    type IntoIter = std::collections::btree_map::Iter<'a, StableId, FieldValue>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a mut FieldMap {
    type Item = (&'a StableId, &'a mut FieldValue);
    type IntoIter = std::collections::btree_map::IterMut<'a, StableId, FieldValue>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

/// One semantic note-field value.
///
/// Equality, ordering, hashing, and debug output include the representation, so a
/// scalar containing rendered image HTML is intentionally distinct from structured
/// image references that lower to the same adapter bytes.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FieldValue {
    Scalar(String),
    Images(Vec<FieldImageReference>),
    Message(StructuredMessage),
}

impl FieldValue {
    pub fn scalar(value: impl Into<String>) -> Self {
        Self::Scalar(value.into())
    }

    pub fn images(images: Vec<FieldImageReference>) -> Result<Self, InvalidFieldValue> {
        if images.is_empty() {
            Err(InvalidFieldValue::EmptyImageSequence)
        } else {
            Ok(Self::Images(images))
        }
    }

    pub fn message(message: StructuredMessage) -> Result<Self, InvalidFieldValue> {
        message.validate_shape()?;
        Ok(Self::Message(message))
    }

    pub fn as_scalar(&self) -> Option<&str> {
        match self {
            Self::Scalar(value) => Some(value),
            Self::Images(_) | Self::Message(_) => None,
        }
    }

    pub fn as_scalar_mut(&mut self) -> Option<&mut String> {
        match self {
            Self::Scalar(value) => Some(value),
            Self::Images(_) | Self::Message(_) => None,
        }
    }

    pub fn as_images(&self) -> Option<&[FieldImageReference]> {
        match self {
            Self::Images(images) => Some(images),
            Self::Scalar(_) | Self::Message(_) => None,
        }
    }

    pub fn as_message(&self) -> Option<&StructuredMessage> {
        match self {
            Self::Message(message) => Some(message),
            Self::Scalar(_) | Self::Images(_) => None,
        }
    }

    pub fn as_message_mut(&mut self) -> Option<&mut StructuredMessage> {
        match self {
            Self::Message(message) => Some(message),
            Self::Scalar(_) | Self::Images(_) => None,
        }
    }

    /// Only the intentional empty scalar is blank/fillable.
    pub fn is_blank(&self) -> bool {
        matches!(self, Self::Scalar(value) if value.is_empty())
    }
}

impl Default for FieldValue {
    fn default() -> Self {
        Self::Scalar(String::new())
    }
}

impl From<String> for FieldValue {
    fn from(value: String) -> Self {
        Self::Scalar(value)
    }
}

impl From<&str> for FieldValue {
    fn from(value: &str) -> Self {
        Self::Scalar(value.to_owned())
    }
}

impl PartialEq<str> for FieldValue {
    fn eq(&self, other: &str) -> bool {
        self.as_scalar() == Some(other)
    }
}

impl PartialEq<&str> for FieldValue {
    fn eq(&self, other: &&str) -> bool {
        self == *other
    }
}

impl PartialEq<String> for FieldValue {
    fn eq(&self, other: &String) -> bool {
        self == other.as_str()
    }
}

/// A malformed semantic field value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InvalidFieldValue {
    EmptyImageSequence,
    InvalidMessage(String),
}

impl fmt::Display for InvalidFieldValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyImageSequence => f.write_str("structured image field must not be empty"),
            Self::InvalidMessage(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for InvalidFieldValue {}

/// A structured reference from a note field to a declared media asset by stable ID.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FieldImageReference {
    pub media_id: StableId,
}

/// A field value assembled from reusable references, translatable fragments, and literal glue.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StructuredMessage {
    /// Positional components for simple messages.
    pub components: Vec<MessageComponent>,
    /// Optional inline format string using `{variable}` placeholders for named message variables.
    pub format: Option<String>,
    /// Named components referenced from `format` placeholders.
    pub variables: BTreeMap<String, MessageComponent>,
}

impl StructuredMessage {
    pub fn validate_shape(&self) -> Result<(), InvalidFieldValue> {
        match (
            &self.format,
            self.components.is_empty(),
            self.variables.is_empty(),
        ) {
            (Some(_), false, _) => Err(InvalidFieldValue::InvalidMessage(
                "structured message cannot mix an inline format with positional message components"
                    .to_owned(),
            )),
            (None, _, false) => Err(InvalidFieldValue::InvalidMessage(
                "structured message variables require an inline format".to_owned(),
            )),
            (None, true, true) => Err(InvalidFieldValue::InvalidMessage(
                "structured message must contain at least one component".to_owned(),
            )),
            (Some(format), true, true) if format.is_empty() => {
                Err(InvalidFieldValue::InvalidMessage(
                    "structured message format must not be empty".to_owned(),
                ))
            }
            _ => Ok(()),
        }
    }
}

/// One component of a structured message field value.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MessageComponent {
    /// Non-translatable glue such as punctuation, spaces, or markup.
    Literal(String),
    /// A translatable fragment extracted independently for translation coverage.
    Text(String),
    /// A reference to another note field path, such as `notes.note.iceland.fields.field.country`.
    FieldRef(String),
}

/// Stable component kind for reports and translator context UIs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageComponentKind {
    Format,
    Literal,
    Text,
    FieldRef,
}

impl MessageComponentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Format => "format",
            Self::Literal => "literal",
            Self::Text => "text",
            Self::FieldRef => "field_ref",
        }
    }
}

/// A reference to an external media asset.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaReference {
    pub id: StableId,
    pub path: String,
    pub sha256: String,
}

/// A failed attempt to render source variables.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariableRenderReport {
    pub errors: Vec<VariableRenderError>,
    pub validation_errors: Vec<ValidationError>,
}

impl VariableRenderReport {
    /// Project every render and validation failure without parsing display text.
    pub fn diagnostics(&self) -> Vec<DomainDiagnostic> {
        self.errors
            .iter()
            .map(|error| DomainDiagnostic {
                code: "missing_variable",
                category: DiagnosticCategory::Validation,
                path: error.path.parse().ok(),
                address: error.path.clone(),
                overlay_id: None,
                source_id: None,
                intent: None,
                entity_kind: None,
                expected: None,
                actual: None,
                first_conflict_participant: None,
                current_conflict_participant: None,
                original_removal: None,
                field_graph_error: None,
                children: Vec::new(),
                message: format!("missing variable ${}", error.variable),
            })
            .chain(
                self.validation_errors
                    .iter()
                    .map(ValidationError::diagnostic),
            )
            .collect()
    }
}

impl fmt::Display for VariableRenderReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut wrote_error = false;
        for error in &self.errors {
            if wrote_error {
                writeln!(f)?;
            }
            write!(f, "{}: missing variable ${}", error.path, error.variable)?;
            wrote_error = true;
        }
        for error in &self.validation_errors {
            if wrote_error {
                writeln!(f)?;
            }
            write!(f, "{}: {}", error.path, error.message)?;
            wrote_error = true;
        }
        Ok(())
    }
}

impl std::error::Error for VariableRenderReport {}

/// One variable rendering failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariableRenderError {
    pub path: String,
    pub variable: String,
}

/// A failed attempt to compose an overlay stack.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComposeReport {
    pub errors: Vec<ComposeError>,
}

impl ComposeReport {
    /// Returns true when the report contains at least one error of the given kind.
    pub fn has_kind(&self, kind: ComposeErrorKind) -> bool {
        self.errors.iter().any(|error| error.kind == kind)
    }
}

impl fmt::Display for ComposeReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, error) in self.errors.iter().enumerate() {
            if index > 0 {
                writeln!(f)?;
            }
            write!(f, "{}", error.diagnostic())?;
        }
        Ok(())
    }
}

impl std::error::Error for ComposeReport {}

/// Typed expected or actual state attached to a composition precondition diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ComposePrecondition {
    Fingerprint(EntityFingerprint),
    Value(String),
    FieldValue(FieldValue),
    Missing,
}

/// Stable top-level classification shared by every domain diagnostic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticCategory {
    Validation,
    Precondition,
    Conflict,
    Overlay,
    Translation,
    Tombstone,
}

impl DiagnosticCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Validation => "validation",
            Self::Precondition => "precondition",
            Self::Conflict => "conflict",
            Self::Overlay => "overlay",
            Self::Translation => "translation",
            Self::Tombstone => "tombstone",
        }
    }
}

/// Canonical, format-independent projection consumed by CLI and HTTP adapters.
///
/// `message` is supplemental human text. Consumers branch on `code`, `category`,
/// and the typed metadata instead of parsing it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DomainDiagnostic {
    pub code: &'static str,
    pub category: DiagnosticCategory,
    pub path: Option<DeckPath>,
    /// Canonical textual address, retained for non-DeckPath domains such as dictionary entries.
    pub address: String,
    pub overlay_id: Option<StableId>,
    pub source_id: Option<StableId>,
    pub intent: Option<ChangeIntent>,
    pub entity_kind: Option<EntityKind>,
    pub expected: Option<ComposePrecondition>,
    pub actual: Option<ComposePrecondition>,
    pub first_conflict_participant: Option<StableId>,
    pub current_conflict_participant: Option<StableId>,
    pub original_removal: Option<TombstoneRecord>,
    pub field_graph_error: Option<FieldGraphError>,
    pub children: Vec<DomainDiagnostic>,
    pub message: String,
}

impl fmt::Display for DomainDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let path = self
            .path
            .as_ref()
            .map(ToString::to_string)
            .filter(|path| !path.is_empty())
            .unwrap_or_else(|| self.address.clone());
        write!(f, "{path}: {}", self.message)?;
        for child in &self.children {
            write!(f, "\n  {child}")?;
        }
        Ok(())
    }
}

/// One overlay composition error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComposeError {
    pub kind: ComposeErrorKind,
    pub path: String,
    pub deck_path: Option<DeckPath>,
    pub entity_kind: Option<EntityKind>,
    pub intent: Option<ChangeIntent>,
    pub overlay_id: Option<StableId>,
    /// Canonical source identity for source/final-validation diagnostics.
    pub source_id: Option<StableId>,
    pub expected: Option<ComposePrecondition>,
    pub actual: Option<ComposePrecondition>,
    /// Original removal record when a later overlay attempts to reuse its address.
    pub original_removal: Option<TombstoneRecord>,
    /// Structured graph diagnostics when composition fails during field dependency planning.
    pub field_graph_error: Option<FieldGraphError>,
    /// First and current participants for a federation conflict.
    pub first_conflict_participant: Option<StableId>,
    pub current_conflict_participant: Option<StableId>,
    /// Original validation variants retained by final composition validation.
    pub validation_errors: Vec<ValidationError>,
    pub message: String,
}

impl ComposeError {
    pub(crate) fn new(kind: ComposeErrorKind, path: String, message: String) -> Self {
        let deck_path = path.parse().ok();
        Self {
            kind,
            path,
            deck_path,
            entity_kind: None,
            intent: None,
            overlay_id: None,
            source_id: None,
            expected: None,
            actual: None,
            original_removal: None,
            field_graph_error: None,
            first_conflict_participant: None,
            current_conflict_participant: None,
            validation_errors: Vec::new(),
            message,
        }
    }

    pub(crate) fn precondition(
        kind: ComposeErrorKind,
        path: String,
        intent: ChangeIntent,
        overlay_id: StableId,
        expected: Option<ComposePrecondition>,
        actual: ComposePrecondition,
        message: String,
    ) -> Self {
        let mut error = Self::new(kind, path, message);
        error.intent = Some(intent);
        error.overlay_id = Some(overlay_id);
        error.expected = expected;
        error.actual = Some(actual);
        error
    }

    pub(crate) fn with_entity_kind(mut self, entity_kind: EntityKind) -> Self {
        self.entity_kind = Some(entity_kind);
        self
    }

    /// Project this error into the canonical machine-readable diagnostic model.
    pub fn diagnostic(&self) -> DomainDiagnostic {
        DomainDiagnostic {
            code: self.kind.code(),
            category: self.kind.category(),
            path: self.deck_path.clone(),
            address: self.path.clone(),
            overlay_id: self.overlay_id.clone(),
            source_id: self.source_id.clone(),
            intent: self.intent,
            entity_kind: self.entity_kind,
            expected: self.expected.clone(),
            actual: self.actual.clone(),
            first_conflict_participant: self.first_conflict_participant.clone(),
            current_conflict_participant: self.current_conflict_participant.clone(),
            original_removal: self.original_removal.clone(),
            field_graph_error: self.field_graph_error.clone(),
            children: self
                .validation_errors
                .iter()
                .map(ValidationError::diagnostic)
                .collect(),
            message: self.message.clone(),
        }
    }
}

/// Machine-readable overlay composition error category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComposeErrorKind {
    MissingExpectedBase,
    InvalidExpectedBase,
    ExpectedBaseMismatch,
    Conflict,
    MissingOverlayTarget,
    AlreadyExists,
    MissingOverlayPayload,
    MissingTranslation,
    StaleTranslationEntry,
    ValidationFailed,
    TombstonedAddressReuse,
}

impl ComposeErrorKind {
    pub fn code(self) -> &'static str {
        match self {
            Self::MissingExpectedBase => "missing_expected_base",
            Self::InvalidExpectedBase => "invalid_expected_base",
            Self::ExpectedBaseMismatch => "expected_base_mismatch",
            Self::Conflict => "overlay_conflict",
            Self::MissingOverlayTarget => "missing_overlay_target",
            Self::AlreadyExists => "entity_already_exists",
            Self::MissingOverlayPayload => "missing_overlay_payload",
            Self::MissingTranslation => "missing_translation",
            Self::StaleTranslationEntry => "stale_translation_entry",
            Self::ValidationFailed => "validation_failed",
            Self::TombstonedAddressReuse => "tombstoned_address_reuse",
        }
    }

    pub fn category(self) -> DiagnosticCategory {
        match self {
            Self::MissingExpectedBase | Self::InvalidExpectedBase | Self::ExpectedBaseMismatch => {
                DiagnosticCategory::Precondition
            }
            Self::Conflict => DiagnosticCategory::Conflict,
            Self::MissingOverlayTarget | Self::AlreadyExists | Self::MissingOverlayPayload => {
                DiagnosticCategory::Overlay
            }
            Self::MissingTranslation | Self::StaleTranslationEntry => {
                DiagnosticCategory::Translation
            }
            Self::ValidationFailed => DiagnosticCategory::Validation,
            Self::TombstonedAddressReuse => DiagnosticCategory::Tombstone,
        }
    }
}

/// A semantic comparison between two CanonicalDeck values.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SemanticDiff {
    pub changes: Vec<SemanticChange>,
}

impl SemanticDiff {
    /// Returns true when no deck entity differs semantically.
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Returns true when a change with this kind and stable path exists.
    pub fn has_change(&self, kind: SemanticChangeKind, path: &str) -> bool {
        self.changes
            .iter()
            .any(|change| change.kind == kind && change.path == path)
    }
}

/// One semantic change at a stable deck path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticChange {
    pub kind: SemanticChangeKind,
    pub path: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

impl SemanticChange {
    pub(crate) fn new(
        kind: SemanticChangeKind,
        path: String,
        before: Option<String>,
        after: Option<String>,
    ) -> Self {
        Self {
            kind,
            path,
            before,
            after,
        }
    }
}

/// Machine-readable semantic change category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticChangeKind {
    Added,
    Removed,
    Modified,
    Tombstoned,
}

/// The semantic representation of a note-field graph node.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FieldValueKind {
    Scalar,
    Images,
    Message,
}

impl FieldValueKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Scalar => "scalar",
            Self::Images => "images",
            Self::Message => "message",
        }
    }
}

/// Machine-readable structured-message dependency failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FieldGraphErrorKind {
    InvalidReference,
    MissingNote,
    MissingFieldDefinition,
    MissingFieldValue,
    TombstonedDependency,
    InvalidTargetRepresentation,
    Cycle,
    InvalidMessage,
}

/// One path-rich failure produced while planning or resolving note field dependencies.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct FieldGraphError {
    pub kind: FieldGraphErrorKind,
    pub note_id: StableId,
    pub field_id: StableId,
    pub consuming_path: String,
    pub dependency: Option<String>,
    pub representation: Option<FieldValueKind>,
    /// Canonical closed field path. Empty except for [`FieldGraphErrorKind::Cycle`].
    pub cycle: Vec<String>,
    pub message: String,
}

impl FieldGraphError {
    /// Project this graph issue without flattening its dependency metadata.
    pub fn diagnostic(&self) -> DomainDiagnostic {
        let code = match self.kind {
            FieldGraphErrorKind::Cycle => "message_dependency_cycle",
            FieldGraphErrorKind::InvalidTargetRepresentation => {
                "invalid_message_target_representation"
            }
            FieldGraphErrorKind::InvalidReference
            | FieldGraphErrorKind::MissingNote
            | FieldGraphErrorKind::MissingFieldDefinition
            | FieldGraphErrorKind::MissingFieldValue
            | FieldGraphErrorKind::TombstonedDependency
            | FieldGraphErrorKind::InvalidMessage => "invalid_message_reference",
        };
        DomainDiagnostic {
            code,
            category: DiagnosticCategory::Validation,
            path: self.consuming_path.parse().ok(),
            address: self.consuming_path.clone(),
            overlay_id: None,
            source_id: None,
            intent: None,
            entity_kind: None,
            expected: None,
            actual: None,
            first_conflict_participant: None,
            current_conflict_participant: None,
            original_removal: None,
            field_graph_error: Some(self.clone()),
            children: Vec::new(),
            message: self.message.clone(),
        }
    }
}

/// A deterministic field graph planning or resolution failure report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldGraphReport {
    pub errors: Vec<FieldGraphError>,
}

impl FieldGraphReport {
    pub fn has_kind(&self, kind: FieldGraphErrorKind) -> bool {
        self.errors.iter().any(|error| error.kind == kind)
    }
}

impl fmt::Display for FieldGraphReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, error) in self.errors.iter().enumerate() {
            if index > 0 {
                writeln!(f)?;
            }
            write!(f, "{}: {}", error.consuming_path, error.message)?;
        }
        Ok(())
    }
}

impl std::error::Error for FieldGraphReport {}

/// Scalar values resolved in deterministic dependency-first order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFieldGraph {
    pub(crate) values: HashMap<String, String>,
    pub(crate) order: Vec<String>,
}

impl ResolvedFieldGraph {
    pub fn get(&self, path: &str) -> Option<&str> {
        self.values.get(path).map(String::as_str)
    }

    pub fn order(&self) -> &[String] {
        &self.order
    }
}

/// A strict validation failure report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationReport {
    pub errors: Vec<ValidationError>,
}

impl ValidationReport {
    /// Returns true when the report contains at least one error of the given kind.
    pub fn has_kind(&self, kind: ValidationErrorKind) -> bool {
        self.errors.iter().any(|error| error.kind == kind)
    }
}

impl fmt::Display for ValidationReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, error) in self.errors.iter().enumerate() {
            if index > 0 {
                writeln!(f)?;
            }
            write!(f, "{}", error.diagnostic())?;
        }
        Ok(())
    }
}

impl std::error::Error for ValidationReport {}

/// One strict validation failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationError {
    pub kind: ValidationErrorKind,
    pub path: String,
    pub deck_path: Option<DeckPath>,
    pub message: String,
    /// Structured graph diagnostics when this validation failure came from a field dependency.
    pub field_graph_error: Option<FieldGraphError>,
}

impl ValidationError {
    pub fn new(kind: ValidationErrorKind, path: String, message: String) -> Self {
        let deck_path = path.parse().ok();
        Self {
            kind,
            path,
            deck_path,
            message,
            field_graph_error: None,
        }
    }

    pub(crate) fn from_field_graph(error: FieldGraphError) -> Self {
        let kind = match error.kind {
            FieldGraphErrorKind::Cycle => ValidationErrorKind::MessageDependencyCycle,
            FieldGraphErrorKind::InvalidTargetRepresentation => {
                ValidationErrorKind::InvalidMessageTargetRepresentation
            }
            FieldGraphErrorKind::InvalidReference
            | FieldGraphErrorKind::MissingNote
            | FieldGraphErrorKind::MissingFieldDefinition
            | FieldGraphErrorKind::MissingFieldValue
            | FieldGraphErrorKind::TombstonedDependency
            | FieldGraphErrorKind::InvalidMessage => ValidationErrorKind::InvalidMessageReference,
        };
        let path = error.consuming_path.clone();
        Self {
            kind,
            deck_path: path.parse().ok(),
            path,
            message: error.message.clone(),
            field_graph_error: Some(error),
        }
    }

    /// Project this issue into the canonical machine-readable diagnostic model.
    pub fn diagnostic(&self) -> DomainDiagnostic {
        DomainDiagnostic {
            code: self.kind.code(),
            category: DiagnosticCategory::Validation,
            path: self.deck_path.clone(),
            address: self.path.clone(),
            overlay_id: None,
            source_id: None,
            intent: None,
            entity_kind: None,
            expected: None,
            actual: None,
            first_conflict_participant: None,
            current_conflict_participant: None,
            original_removal: None,
            field_graph_error: self.field_graph_error.clone(),
            children: Vec::new(),
            message: self.message.clone(),
        }
    }
}

/// Machine-readable validation error category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationErrorKind {
    MissingNoteType,
    UnknownNoteField,
    MissingNoteField,
    MismatchedEntityId,
    DuplicateFieldDefinition,
    DuplicateCardTemplate,
    InvalidMessageReference,
    InvalidMessageTargetRepresentation,
    MessageDependencyCycle,
    InvalidStableId,
    ConflictingFieldRepresentation,
    UnknownMediaReference,
    UnknownTemplateField,
    MalformedTemplateReference,
}

impl ValidationErrorKind {
    pub fn code(self) -> &'static str {
        match self {
            Self::MissingNoteType => "missing_note_type",
            Self::UnknownNoteField => "unknown_note_field",
            Self::MissingNoteField => "missing_note_field",
            Self::MismatchedEntityId => "mismatched_entity_id",
            Self::DuplicateFieldDefinition => "duplicate_field_definition",
            Self::DuplicateCardTemplate => "duplicate_card_template",
            Self::InvalidMessageReference => "invalid_message_reference",
            Self::InvalidMessageTargetRepresentation => "invalid_message_target_representation",
            Self::MessageDependencyCycle => "message_dependency_cycle",
            Self::InvalidStableId => "invalid_stable_id",
            Self::ConflictingFieldRepresentation => "conflicting_field_representation",
            Self::UnknownMediaReference => "unknown_media_reference",
            Self::UnknownTemplateField => "unknown_template_field",
            Self::MalformedTemplateReference => "malformed_template_reference",
        }
    }

    pub fn category(self) -> DiagnosticCategory {
        DiagnosticCategory::Validation
    }
}
