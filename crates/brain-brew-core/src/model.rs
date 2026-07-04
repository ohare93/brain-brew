use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str::FromStr;

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
/// `.message.`), or reserved property suffixes (`.id`, `.name`, `.styling`,
/// `.fields`, `.card_templates`, `.variables`, `.adapter_ids`, `.tags`,
/// `.note_type_id`, `.message`, `.path`, `.sha256`, `.question_format`,
/// `.answer_format`). Non-StableId map keys and tag strings are exempt from that
/// StableId-only invariant, so keys such as `note-type.name` remain legal after
/// the first reserved container split.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeckPath {
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
        id: StableId,
    },
}

impl fmt::Display for DeckPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
            Self::Tombstone { id } => write!(f, "tombstones.{id}"),
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
        return stable_id(rest).map(|id| DeckPath::Tombstone { id });
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
    pub tombstones: BTreeSet<StableId>,
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

impl TranslationDictionary {
    /// Resolve one stale translation into a normal direct or contextual translation entry.
    pub fn resolve_stale_translation(
        &mut self,
        old_source: &str,
        new_source: &str,
        context: Option<&str>,
    ) -> Option<StaleTranslation> {
        let position = self.stale_translations.iter().position(|record| {
            record.old_source == old_source
                && record.new_source == new_source
                && record.context.as_deref() == context
        })?;
        let record = self.stale_translations.remove(position);
        if let Some(context) = &record.context {
            self.contextual
                .entry(context.clone())
                .or_default()
                .insert(record.new_source.clone(), record.target.clone());
        } else {
            self.direct
                .insert(record.new_source.clone(), record.target.clone());
        }
        Some(record)
    }
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChangeIntent {
    Add,
    Merge,
    Replace,
    Remove,
    Override,
}

/// Base value or condition an overlay expects before applying a destructive change.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExpectedBase {
    Value(String),
    EntityPresent,
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
    pub value: Option<String>,
    pub message: Option<StructuredMessage>,
    pub images: Option<Vec<FieldImageReference>>,
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
    pub fields: BTreeMap<StableId, String>,
    pub field_messages: BTreeMap<StableId, StructuredMessage>,
    pub field_images: BTreeMap<StableId, Vec<FieldImageReference>>,
    pub tags: BTreeSet<String>,
    pub adapter_ids: AdapterIds,
}

/// A structured reference from a note field to a declared media asset by stable ID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldImageReference {
    pub media_id: StableId,
}

/// A field value assembled from reusable references, translatable fragments, and literal glue.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuredMessage {
    /// Positional components for simple messages.
    pub components: Vec<MessageComponent>,
    /// Optional inline format string using `{variable}` placeholders for named message variables.
    pub format: Option<String>,
    /// Named components referenced from `format` placeholders.
    pub variables: BTreeMap<String, MessageComponent>,
}

/// One component of a structured message field value.
#[derive(Clone, Debug, Eq, PartialEq)]
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
            write!(f, "{}: {}", error.path, error.message)?;
        }
        Ok(())
    }
}

impl std::error::Error for ComposeReport {}

/// One overlay composition error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComposeError {
    pub kind: ComposeErrorKind,
    pub path: String,
    pub message: String,
}

impl ComposeError {
    pub(crate) fn new(kind: ComposeErrorKind, path: String, message: String) -> Self {
        Self {
            kind,
            path,
            message,
        }
    }
}

/// Machine-readable overlay composition error category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComposeErrorKind {
    MissingExpectedBase,
    ExpectedBaseMismatch,
    Conflict,
    MissingOverlayTarget,
    AlreadyExists,
    MissingOverlayPayload,
    MissingTranslation,
    StaleTranslationEntry,
    ValidationFailed,
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

    pub(crate) fn added(path: String) -> Self {
        Self::new(SemanticChangeKind::Added, path, None, None)
    }

    pub(crate) fn removed(path: String) -> Self {
        Self::new(SemanticChangeKind::Removed, path, None, None)
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
            write!(f, "{}: {}", error.path, error.message)?;
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
    pub message: String,
}

impl ValidationError {
    pub fn new(kind: ValidationErrorKind, path: String, message: String) -> Self {
        Self {
            kind,
            path,
            message,
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
    InvalidStableId,
    ConflictingFieldRepresentation,
    UnknownMediaReference,
}
