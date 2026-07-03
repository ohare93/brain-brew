use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

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
    pub tags: BTreeSet<String>,
    pub adapter_ids: AdapterIds,
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
}
