//! Pure domain model and behavior for Brain Brew.
//!
//! This crate intentionally contains no file formats, filesystem access, terminal UI,
//! or command-line concerns. It owns the CanonicalDeck domain model, validation,
//! composition, and semantic diffing as they are introduced through TDD.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Name of the core crate.
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

impl CanonicalDeck {
    /// Apply an ordered overlay stack to this base deck.
    pub fn compose(&self, overlays: &[Overlay]) -> Result<Self, ComposeReport> {
        let mut resolved = self.clone();
        let mut errors = Vec::new();
        let mut changed_paths = BTreeMap::<String, StableId>::new();

        resolve_structured_messages_with_compose_errors(&mut resolved, &mut errors);

        for overlay in overlays {
            apply_overlay(&mut resolved, overlay, &mut changed_paths, &mut errors);
            resolve_structured_messages_with_compose_errors(&mut resolved, &mut errors);
        }

        if !errors.is_empty() {
            return Err(ComposeReport { errors });
        }

        resolved.validate().map_err(|report| ComposeReport {
            errors: report
                .errors
                .into_iter()
                .map(|error| {
                    ComposeError::new(
                        ComposeErrorKind::ValidationFailed,
                        error.path,
                        error.message,
                    )
                })
                .collect(),
        })?;

        Ok(resolved)
    }

    /// Compare this deck with another deck by stable IDs and deck entities.
    pub fn semantic_diff(&self, other: &Self) -> SemanticDiff {
        let mut changes = Vec::new();

        push_modified_if_changed(
            &mut changes,
            "deck.name".to_owned(),
            &self.name,
            &other.name,
        );
        push_modified_if_changed(
            &mut changes,
            "deck.description".to_owned(),
            &self.description,
            &other.description,
        );
        push_modified_if_changed(
            &mut changes,
            "deck.variables".to_owned(),
            &string_map_summary(&self.variables),
            &string_map_summary(&other.variables),
        );

        diff_note_types(&self.note_types, &other.note_types, &mut changes);
        diff_notes(&self.notes, &other.notes, &mut changes);
        diff_media(&self.media, &other.media, &mut changes);
        diff_tombstones(&self.tombstones, &other.tombstones, &mut changes);

        SemanticDiff { changes }
    }

    /// Render `${variable}` references in deck text using deck, note type, card template, and note scopes.
    pub fn render_variables(&self) -> Result<Self, VariableRenderReport> {
        render_deck_variables(self)
    }

    /// Resolve structured message fields into their final scalar field strings.
    pub fn resolve_structured_messages(&self) -> Result<Self, ValidationReport> {
        let mut resolved = self.clone();
        let mut errors = Vec::new();
        resolve_structured_messages_with_validation_errors(&mut resolved, &mut errors);
        if errors.is_empty() {
            Ok(resolved)
        } else {
            Err(ValidationReport { errors })
        }
    }

    /// Report translation coverage for one translation overlay without modifying this deck.
    pub fn translation_coverage(&self, overlay: &Overlay) -> Option<TranslationCoverageReport> {
        overlay
            .translations
            .as_ref()
            .map(|translations| translation_coverage_report(self, overlay, translations))
    }

    /// Build translator-facing note/field/card context for one coverage report.
    pub fn translation_context(
        &self,
        report: &TranslationCoverageReport,
    ) -> TranslationContextView {
        translation_context_view(self, report)
    }

    /// Validate strict core invariants that do not require filesystem or format access.
    pub fn validate(&self) -> Result<(), ValidationReport> {
        let mut errors = Vec::new();

        for (id, note_type) in &self.note_types {
            if &note_type.id != id {
                errors.push(ValidationError::new(
                    ValidationErrorKind::MismatchedEntityId,
                    format!("note_types.{id}.id"),
                    format!("note type stored under {id} contains id {}", note_type.id),
                ));
            }

            push_duplicate_id_errors(
                note_type.fields.iter().map(|field| &field.id),
                ValidationErrorKind::DuplicateFieldDefinition,
                |duplicate_id| format!("note_types.{id}.fields.{duplicate_id}"),
                &mut errors,
            );

            push_duplicate_id_errors(
                note_type.card_templates.iter().map(|template| &template.id),
                ValidationErrorKind::DuplicateCardTemplate,
                |duplicate_id| format!("note_types.{id}.card_templates.{duplicate_id}"),
                &mut errors,
            );
        }

        for (id, media) in &self.media {
            if &media.id != id {
                errors.push(ValidationError::new(
                    ValidationErrorKind::MismatchedEntityId,
                    format!("media.{id}.id"),
                    format!("media stored under {id} contains id {}", media.id),
                ));
            }
        }

        for (id, note) in &self.notes {
            if &note.id != id {
                errors.push(ValidationError::new(
                    ValidationErrorKind::MismatchedEntityId,
                    format!("notes.{id}.id"),
                    format!("note stored under {id} contains id {}", note.id),
                ));
            }

            let Some(note_type) = self.note_types.get(&note.note_type_id) else {
                errors.push(ValidationError::new(
                    ValidationErrorKind::MissingNoteType,
                    format!("notes.{id}.note_type_id"),
                    format!("note references missing note type {}", note.note_type_id),
                ));
                continue;
            };

            let expected_field_ids = note_type
                .fields
                .iter()
                .map(|field| field.id.clone())
                .collect::<BTreeSet<_>>();

            for field_id in note.fields.keys() {
                if !expected_field_ids.contains(field_id) {
                    errors.push(ValidationError::new(
                        ValidationErrorKind::UnknownNoteField,
                        format!("notes.{id}.fields.{field_id}"),
                        format!(
                            "note field {field_id} is not defined by note type {}",
                            note.note_type_id
                        ),
                    ));
                }
            }

            for (field_id, message) in &note.field_messages {
                if !expected_field_ids.contains(field_id) {
                    errors.push(ValidationError::new(
                        ValidationErrorKind::UnknownNoteField,
                        format!("notes.{id}.fields.{field_id}.message"),
                        format!(
                            "structured message field {field_id} is not defined by note type {}",
                            note.note_type_id
                        ),
                    ));
                }
                validate_message_references(self, id, field_id, message, &mut errors);
            }

            for field_id in expected_field_ids {
                if !note.fields.contains_key(&field_id) {
                    errors.push(ValidationError::new(
                        ValidationErrorKind::MissingNoteField,
                        format!("notes.{id}.fields.{field_id}"),
                        format!(
                            "note is missing field {field_id} defined by note type {}",
                            note.note_type_id
                        ),
                    ));
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ValidationReport { errors })
        }
    }
}

fn apply_overlay(
    resolved: &mut CanonicalDeck,
    overlay: &Overlay,
    changed_paths: &mut BTreeMap<String, StableId>,
    errors: &mut Vec<ComposeError>,
) {
    if let Some(translations) = &overlay.translations {
        apply_translation_dictionary(resolved, overlay, translations, changed_paths, errors);
    }

    if let Some(change) = &overlay.deck_change {
        apply_deck_change(resolved, overlay, change, changed_paths, errors);
    }

    let mut added_fields = Vec::new();
    for (note_type_id, change) in &overlay.note_type_changes {
        match change.intent {
            ChangeIntent::Add => apply_note_type_add(
                resolved,
                overlay,
                note_type_id,
                change,
                changed_paths,
                errors,
            ),
            ChangeIntent::Merge | ChangeIntent::Replace | ChangeIntent::Override => {
                added_fields.extend(apply_note_type_change(
                    resolved,
                    overlay,
                    note_type_id,
                    change,
                    changed_paths,
                    errors,
                ));
            }
            ChangeIntent::Remove => apply_note_type_remove(
                resolved,
                overlay,
                note_type_id,
                change,
                changed_paths,
                errors,
            ),
        }
    }

    for (note_id, change) in &overlay.note_changes {
        match change.intent {
            ChangeIntent::Add => {
                apply_note_add(resolved, overlay, note_id, change, changed_paths, errors)
            }
            ChangeIntent::Merge | ChangeIntent::Replace | ChangeIntent::Override => {
                apply_note_merge(resolved, overlay, note_id, change, changed_paths, errors);
            }
            ChangeIntent::Remove => {
                apply_note_remove(resolved, overlay, note_id, change, changed_paths, errors);
            }
        }
    }

    for (note_type_id, field_id) in added_fields {
        fill_added_field_blanks(resolved, &note_type_id, &field_id);
    }

    for (media_id, change) in &overlay.media_changes {
        apply_media_change(resolved, overlay, media_id, change, changed_paths, errors);
    }
}

fn fill_added_field_blanks(
    resolved: &mut CanonicalDeck,
    note_type_id: &StableId,
    field_id: &StableId,
) {
    for note in resolved
        .notes
        .values_mut()
        .filter(|note| &note.note_type_id == note_type_id)
    {
        note.fields.entry(field_id.clone()).or_default();
    }
}

fn validate_message_references(
    deck: &CanonicalDeck,
    note_id: &StableId,
    field_id: &StableId,
    message: &StructuredMessage,
    errors: &mut Vec<ValidationError>,
) {
    if let Some(format) = &message.format {
        if !message.components.is_empty() {
            errors.push(ValidationError::new(
                ValidationErrorKind::InvalidMessageReference,
                message_format_path(note_id, field_id),
                "structured message cannot mix an inline format with positional message components"
                    .to_owned(),
            ));
        }
        match parse_message_format(format) {
            Ok(parts) => {
                let referenced_variables = parts
                    .iter()
                    .filter_map(|part| match part {
                        MessageFormatPart::Literal(_) => None,
                        MessageFormatPart::Variable(variable) => Some(variable.clone()),
                    })
                    .collect::<BTreeSet<_>>();
                for variable in &referenced_variables {
                    if !message.variables.contains_key(variable) {
                        errors.push(ValidationError::new(
                            ValidationErrorKind::InvalidMessageReference,
                            message_format_path(note_id, field_id),
                            format!(
                                "structured message format references undefined variable {variable:?}"
                            ),
                        ));
                    }
                }
            }
            Err(message) => errors.push(ValidationError::new(
                ValidationErrorKind::InvalidMessageReference,
                message_format_path(note_id, field_id),
                message,
            )),
        }
        for (variable, component) in &message.variables {
            validate_message_component_reference(
                deck,
                component,
                message_variable_path(note_id, field_id, variable),
                errors,
            );
        }
        return;
    }

    if !message.variables.is_empty() {
        errors.push(ValidationError::new(
            ValidationErrorKind::InvalidMessageReference,
            format!("notes.{note_id}.fields.{field_id}.message"),
            "structured message variables require an inline format".to_owned(),
        ));
    }

    for (index, component) in message.components.iter().enumerate() {
        validate_message_component_reference(
            deck,
            component,
            message_component_path(note_id, field_id, index),
            errors,
        );
    }
}

fn validate_message_component_reference(
    deck: &CanonicalDeck,
    component: &MessageComponent,
    path: String,
    errors: &mut Vec<ValidationError>,
) {
    let MessageComponent::FieldRef(reference) = component else {
        return;
    };
    if field_value_at_path(deck, reference).is_none() {
        errors.push(ValidationError::new(
            ValidationErrorKind::InvalidMessageReference,
            path,
            format!(
                "structured message field reference {reference:?} does not resolve to a note field"
            ),
        ));
    }
}

fn resolve_structured_messages_with_validation_errors(
    deck: &mut CanonicalDeck,
    errors: &mut Vec<ValidationError>,
) {
    let snapshot = deck.clone();
    let mut resolved_fields = Vec::<(StableId, StableId, String)>::new();
    for (note_id, note) in &snapshot.notes {
        for (field_id, message) in &note.field_messages {
            match render_structured_message(&snapshot, message) {
                Ok(value) => resolved_fields.push((note_id.clone(), field_id.clone(), value)),
                Err(error) => errors.push(ValidationError::new(
                    ValidationErrorKind::InvalidMessageReference,
                    format!("notes.{note_id}.fields.{field_id}.message"),
                    error.message(),
                )),
            }
        }
    }
    for (note_id, field_id, value) in resolved_fields {
        if let Some(note) = deck.notes.get_mut(&note_id) {
            note.fields.insert(field_id, value);
        }
    }
}

fn resolve_structured_messages_with_compose_errors(
    deck: &mut CanonicalDeck,
    errors: &mut Vec<ComposeError>,
) {
    let snapshot = deck.clone();
    let mut resolved_fields = Vec::<(StableId, StableId, String)>::new();
    for (note_id, note) in &snapshot.notes {
        for (field_id, message) in &note.field_messages {
            match render_structured_message(&snapshot, message) {
                Ok(value) => resolved_fields.push((note_id.clone(), field_id.clone(), value)),
                Err(error) => errors.push(ComposeError::new(
                    ComposeErrorKind::ValidationFailed,
                    format!("notes.{note_id}.fields.{field_id}.message"),
                    error.message(),
                )),
            }
        }
    }
    for (note_id, field_id, value) in resolved_fields {
        if let Some(note) = deck.notes.get_mut(&note_id) {
            note.fields.insert(field_id, value);
        }
    }
}

fn render_structured_message(
    deck: &CanonicalDeck,
    message: &StructuredMessage,
) -> Result<String, StructuredMessageRenderError> {
    if let Some(format) = &message.format {
        let mut variables = BTreeMap::new();
        for (name, component) in &message.variables {
            variables.insert(name.clone(), render_message_component(deck, component)?);
        }
        return render_message_format(format, &variables)
            .map_err(StructuredMessageRenderError::Format);
    }

    let mut rendered = String::new();
    for component in &message.components {
        rendered.push_str(&render_message_component(deck, component)?);
    }
    Ok(rendered)
}

fn render_message_component(
    deck: &CanonicalDeck,
    component: &MessageComponent,
) -> Result<String, StructuredMessageRenderError> {
    match component {
        MessageComponent::Literal(value) | MessageComponent::Text(value) => Ok(value.clone()),
        MessageComponent::FieldRef(reference) => {
            let Some(value) = field_value_at_path(deck, reference) else {
                return Err(StructuredMessageRenderError::InvalidReference(
                    reference.clone(),
                ));
            };
            Ok(value.to_owned())
        }
    }
}

#[derive(Debug)]
enum StructuredMessageRenderError {
    InvalidReference(String),
    Format(String),
}

impl StructuredMessageRenderError {
    fn message(&self) -> String {
        match self {
            Self::InvalidReference(reference) => format!(
                "structured message field reference {reference:?} does not resolve to a note field"
            ),
            Self::Format(message) => message.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum MessageFormatPart {
    Literal(String),
    Variable(String),
}

fn render_message_format(
    format: &str,
    variables: &BTreeMap<String, String>,
) -> Result<String, String> {
    let parts = parse_message_format(format)?;
    let mut rendered = String::new();
    for part in parts {
        match part {
            MessageFormatPart::Literal(value) => rendered.push_str(&value),
            MessageFormatPart::Variable(variable) => {
                let Some(value) = variables.get(&variable) else {
                    return Err(format!(
                        "structured message format references undefined variable {variable:?}"
                    ));
                };
                rendered.push_str(value);
            }
        }
    }
    Ok(rendered)
}

fn parse_message_format(format: &str) -> Result<Vec<MessageFormatPart>, String> {
    let mut parts = Vec::new();
    let mut literal = String::new();
    let mut chars = format.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '{' => {
                if chars.peek() == Some(&'{') {
                    chars.next();
                    literal.push('{');
                    continue;
                }
                if !literal.is_empty() {
                    parts.push(MessageFormatPart::Literal(std::mem::take(&mut literal)));
                }
                let mut variable = String::new();
                let mut closed = false;
                for variable_ch in chars.by_ref() {
                    if variable_ch == '}' {
                        closed = true;
                        break;
                    }
                    variable.push(variable_ch);
                }
                if !closed {
                    return Err(
                        "structured message format has an unclosed variable placeholder".to_owned(),
                    );
                }
                if variable.is_empty() {
                    return Err(
                        "structured message format contains an empty variable placeholder"
                            .to_owned(),
                    );
                }
                parts.push(MessageFormatPart::Variable(variable));
            }
            '}' => {
                if chars.peek() == Some(&'}') {
                    chars.next();
                    literal.push('}');
                } else {
                    return Err(
                        "structured message format contains an unmatched closing brace".to_owned(),
                    );
                }
            }
            other => literal.push(other),
        }
    }
    if !literal.is_empty() {
        parts.push(MessageFormatPart::Literal(literal));
    }
    Ok(parts)
}

fn field_value_at_path<'a>(deck: &'a CanonicalDeck, path: &str) -> Option<&'a str> {
    let (note_id, field_id) = note_field_path_parts(path)?;
    deck.notes
        .get(&note_id)
        .and_then(|note| note.fields.get(&field_id))
        .map(String::as_str)
}

fn note_field_path_parts(path: &str) -> Option<(StableId, StableId)> {
    let rest = path.strip_prefix("notes.")?;
    let (note_id, field_id) = rest.split_once(".fields.")?;
    Some((StableId::new(note_id).ok()?, StableId::new(field_id).ok()?))
}

fn message_component_path(note_id: &StableId, field_id: &StableId, index: usize) -> String {
    format!("notes.{note_id}.fields.{field_id}.message.{index}")
}

fn message_format_path(note_id: &StableId, field_id: &StableId) -> String {
    format!("notes.{note_id}.fields.{field_id}.message.format")
}

fn message_variable_path(note_id: &StableId, field_id: &StableId, variable: &str) -> String {
    format!("notes.{note_id}.fields.{field_id}.message.variables.{variable}")
}

fn translation_coverage_report(
    deck: &CanonicalDeck,
    overlay: &Overlay,
    translations: &TranslationDictionary,
) -> TranslationCoverageReport {
    let mut builder = TranslationCoverageBuilder {
        deck,
        translations,
        entries: Vec::new(),
        seen_sources: BTreeSet::new(),
        seen_direct: BTreeSet::new(),
        seen_contextual: BTreeSet::new(),
        seen_no_change: BTreeSet::new(),
        seen_target_additions: BTreeSet::new(),
        seen_stale_records: BTreeSet::new(),
        seen_variables: BTreeSet::new(),
        seen_adapter_ids: BTreeSet::new(),
    };

    builder.record_string(&deck.name, "deck.name".to_owned(), None);
    builder.record_string(&deck.description, "deck.description".to_owned(), None);
    builder.record_variables(&deck.variables, "deck.variables");
    builder.record_adapter_ids(&deck.adapter_ids, "deck.adapter_ids");

    for (note_type_id, note_type) in &deck.note_types {
        builder.record_string(
            &note_type.name,
            format!("note_types.{note_type_id}.name"),
            None,
        );
        builder.record_variables(
            &note_type.variables,
            &format!("note_types.{note_type_id}.variables"),
        );
        for field in &note_type.fields {
            builder.record_string(
                &field.name,
                format!("note_types.{note_type_id}.fields.{}.name", field.id),
                None,
            );
        }
        for template in &note_type.card_templates {
            builder.record_string(
                &template.name,
                format!(
                    "note_types.{note_type_id}.card_templates.{}.name",
                    template.id
                ),
                None,
            );
            builder.record_variables(
                &template.variables,
                &format!(
                    "note_types.{note_type_id}.card_templates.{}.variables",
                    template.id
                ),
            );
            builder.record_adapter_ids(
                &template.adapter_ids,
                &format!(
                    "note_types.{note_type_id}.card_templates.{}.adapter_ids",
                    template.id
                ),
            );
        }
        builder.record_adapter_ids(
            &note_type.adapter_ids,
            &format!("note_types.{note_type_id}.adapter_ids"),
        );
    }

    for (note_id, note) in &deck.notes {
        builder.record_variables(&note.variables, &format!("notes.{note_id}.variables"));
        for (field_id, value) in &note.fields {
            let path = format!("notes.{note_id}.fields.{field_id}");
            if let Some(message) = note.field_messages.get(field_id) {
                builder.record_message(value, path, note_id, field_id, message);
            } else {
                builder.record_string(value, path, None);
            }
        }
        builder.record_tags(&note.tags, &format!("notes.{note_id}.tags"));
        builder.record_adapter_ids(&note.adapter_ids, &format!("notes.{note_id}.adapter_ids"));
    }

    builder.finish(overlay.id.clone())
}

struct TranslationCoverageBuilder<'a> {
    deck: &'a CanonicalDeck,
    translations: &'a TranslationDictionary,
    entries: Vec<TranslationCoverageEntry>,
    seen_sources: BTreeSet<String>,
    seen_direct: BTreeSet<String>,
    seen_contextual: BTreeSet<(String, String)>,
    seen_no_change: BTreeSet<String>,
    seen_target_additions: BTreeSet<String>,
    seen_stale_records: BTreeSet<usize>,
    seen_variables: BTreeSet<(String, String)>,
    seen_adapter_ids: BTreeSet<(String, String)>,
}

impl TranslationCoverageBuilder<'_> {
    fn record_variables(&mut self, variables: &BTreeMap<String, String>, path_prefix: &str) {
        for (key, value) in variables {
            self.record_string(value, format!("{path_prefix}.{key}"), Some(key));
        }
    }

    fn record_message(
        &mut self,
        resolved_value: &str,
        path: String,
        note_id: &StableId,
        field_id: &StableId,
        message: &StructuredMessage,
    ) {
        if self.translations.target_additions.contains_key(&path)
            || self.has_explicit_string_entry(resolved_value, &path, None)
        {
            self.record_string(resolved_value, path, None);
            return;
        }

        if resolved_value.is_empty() {
            return;
        }

        if is_ignored_translation_path(self.translations, &path) {
            self.entries.push(TranslationCoverageEntry {
                category: TranslationCoverageCategory::IgnoredSource,
                path,
                source: resolved_value.to_owned(),
                old_source: None,
                translated: None,
                context: None,
            });
            return;
        }

        if let Some(format) = &message.format {
            self.record_optional_string(format, message_format_path(note_id, field_id));
            for (variable, component) in &message.variables {
                self.record_message_component(
                    component,
                    message_variable_path(note_id, field_id, variable),
                );
            }
        } else {
            for (index, component) in message.components.iter().enumerate() {
                self.record_message_component(
                    component,
                    message_component_path(note_id, field_id, index),
                );
            }
        }
    }

    fn record_message_component(&mut self, component: &MessageComponent, path: String) {
        match component {
            MessageComponent::Literal(_) => {}
            MessageComponent::Text(value) => {
                self.record_string(value, path, None);
            }
            MessageComponent::FieldRef(reference) => {
                if let Some(value) = field_value_at_path(self.deck, reference) {
                    self.record_string(value, path, None);
                }
            }
        }
    }

    fn has_explicit_string_entry(
        &self,
        value: &str,
        path: &str,
        variable_key: Option<&str>,
    ) -> bool {
        if value.is_empty() {
            return false;
        }
        if let Some(variable_key) = variable_key
            && self
                .translations
                .variables
                .get(variable_key)
                .is_some_and(|replacements| replacements.contains_key(value))
        {
            return true;
        }
        self.translations.direct.contains_key(value)
            || self.translations.no_change.contains(value)
            || self
                .translations
                .contextual
                .iter()
                .any(|(context_path, replacements)| {
                    context_matches_path(context_path, path) && replacements.contains_key(value)
                })
            || matching_stale_record(self.translations, value, path).is_some()
    }

    fn record_string(&mut self, value: &str, path: String, variable_key: Option<&str>) {
        if let Some(addition) = self.translations.target_additions.get(&path) {
            self.seen_target_additions.insert(path.clone());
            let category = if value.is_empty() {
                TranslationCoverageCategory::TargetLanguageAddition
            } else {
                TranslationCoverageCategory::InvalidTargetAddition
            };
            self.entries.push(TranslationCoverageEntry {
                category,
                path,
                source: value.to_owned(),
                old_source: None,
                translated: Some(addition.clone()),
                context: None,
            });
            return;
        }

        if value.is_empty() {
            return;
        }

        if is_ignored_translation_path(self.translations, &path) {
            self.entries.push(TranslationCoverageEntry {
                category: TranslationCoverageCategory::IgnoredSource,
                path,
                source: value.to_owned(),
                old_source: None,
                translated: None,
                context: None,
            });
            return;
        }

        if let Some(variable_key) = variable_key
            && let Some(replacements) = self.translations.variables.get(variable_key)
            && let Some(translated) = replacements.get(value)
        {
            self.seen_variables
                .insert((variable_key.to_owned(), value.to_owned()));
            self.entries.push(TranslationCoverageEntry {
                category: TranslationCoverageCategory::VariableTranslation,
                path,
                source: value.to_owned(),
                old_source: None,
                translated: Some(translated.clone()),
                context: Some(variable_key.to_owned()),
            });
            return;
        }

        let source = value.to_owned();
        self.seen_sources.insert(source.clone());
        let direct_translation = self.translations.direct.get(value);
        if direct_translation.is_some() {
            self.seen_direct.insert(source.clone());
        }
        let no_change = self.translations.no_change.contains(value);

        let mut contextual_translation: Option<(&String, &String)> = None;
        for (context_path, replacements) in &self.translations.contextual {
            if context_matches_path(context_path, &path)
                && let Some(translated) = replacements.get(value)
            {
                self.seen_contextual
                    .insert((context_path.clone(), source.clone()));
                if contextual_translation
                    .as_ref()
                    .is_none_or(|(current_context, _)| context_path.len() > current_context.len())
                {
                    contextual_translation = Some((context_path, translated));
                }
            }
        }

        if let Some((context_path, translated)) = contextual_translation {
            self.entries.push(TranslationCoverageEntry {
                category: TranslationCoverageCategory::ContextualOverride,
                path,
                source,
                old_source: None,
                translated: Some(translated.clone()),
                context: Some(context_path.clone()),
            });
        } else if let Some(translated) = direct_translation {
            self.entries.push(TranslationCoverageEntry {
                category: TranslationCoverageCategory::DirectTranslation,
                path,
                source,
                old_source: None,
                translated: Some(translated.clone()),
                context: None,
            });
        } else if no_change {
            self.seen_no_change.insert(source.clone());
            self.entries.push(TranslationCoverageEntry {
                category: TranslationCoverageCategory::NoChange,
                path,
                source: source.clone(),
                old_source: None,
                translated: Some(source),
                context: None,
            });
        } else if let Some((index, record)) =
            matching_stale_record(self.translations, &source, &path)
        {
            self.seen_stale_records.insert(index);
            self.entries.push(TranslationCoverageEntry {
                category: TranslationCoverageCategory::StaleTranslationRecord,
                path,
                source: source.clone(),
                old_source: Some(record.old_source.clone()),
                translated: Some(record.target.clone()),
                context: record.context.clone(),
            });
        } else {
            self.entries.push(TranslationCoverageEntry {
                category: TranslationCoverageCategory::UntranslatedFallback,
                path,
                source: source.clone(),
                old_source: None,
                translated: Some(source),
                context: None,
            });
        }
    }

    fn record_optional_string(&mut self, value: &str, path: String) {
        if value.is_empty() || is_ignored_translation_path(self.translations, &path) {
            return;
        }

        let source = value.to_owned();
        self.seen_sources.insert(source.clone());
        let direct_translation = self.translations.direct.get(value);
        if direct_translation.is_some() {
            self.seen_direct.insert(source.clone());
        }
        let no_change = self.translations.no_change.contains(value);

        let mut contextual_translation: Option<(&String, &String)> = None;
        for (context_path, replacements) in &self.translations.contextual {
            if context_matches_path(context_path, &path)
                && let Some(translated) = replacements.get(value)
            {
                self.seen_contextual
                    .insert((context_path.clone(), source.clone()));
                if contextual_translation
                    .as_ref()
                    .is_none_or(|(current_context, _)| context_path.len() > current_context.len())
                {
                    contextual_translation = Some((context_path, translated));
                }
            }
        }

        if let Some((context_path, translated)) = contextual_translation {
            self.entries.push(TranslationCoverageEntry {
                category: TranslationCoverageCategory::ContextualOverride,
                path,
                source,
                old_source: None,
                translated: Some(translated.clone()),
                context: Some(context_path.clone()),
            });
        } else if let Some(translated) = direct_translation {
            self.entries.push(TranslationCoverageEntry {
                category: TranslationCoverageCategory::DirectTranslation,
                path,
                source,
                old_source: None,
                translated: Some(translated.clone()),
                context: None,
            });
        } else if no_change {
            self.seen_no_change.insert(source.clone());
            self.entries.push(TranslationCoverageEntry {
                category: TranslationCoverageCategory::NoChange,
                path,
                source: source.clone(),
                old_source: None,
                translated: Some(source),
                context: None,
            });
        } else if let Some((index, record)) =
            matching_stale_record(self.translations, &source, &path)
        {
            self.seen_stale_records.insert(index);
            self.entries.push(TranslationCoverageEntry {
                category: TranslationCoverageCategory::StaleTranslationRecord,
                path,
                source: source.clone(),
                old_source: Some(record.old_source.clone()),
                translated: Some(record.target.clone()),
                context: record.context.clone(),
            });
        }
    }

    fn record_tags(&mut self, tags: &BTreeSet<String>, path_prefix: &str) {
        for tag in tags {
            self.record_string(tag, format!("{path_prefix}.{tag}"), None);
        }
    }

    fn record_adapter_ids(&mut self, adapter_ids: &AdapterIds, path_prefix: &str) {
        for (key, value) in adapter_ids.iter() {
            let Some(replacements) = self.translations.adapter_ids.get(key) else {
                continue;
            };
            let Some(translated) = replacements.get(value) else {
                continue;
            };
            self.seen_adapter_ids
                .insert((key.to_owned(), value.to_owned()));
            self.entries.push(TranslationCoverageEntry {
                category: TranslationCoverageCategory::AdapterIdTranslation,
                path: format!("{path_prefix}.{key}"),
                source: value.to_owned(),
                old_source: None,
                translated: Some(translated.clone()),
                context: Some(key.to_owned()),
            });
        }
    }

    fn finish(mut self, overlay_id: StableId) -> TranslationCoverageReport {
        for (source, translated) in &self.translations.direct {
            if !self.seen_direct.contains(source) {
                self.entries.push(TranslationCoverageEntry {
                    category: TranslationCoverageCategory::StaleDirectKey,
                    path: format!("translations.direct.{source}"),
                    source: source.clone(),
                    old_source: None,
                    translated: Some(translated.clone()),
                    context: None,
                });
            }
        }
        for (context_path, replacements) in &self.translations.contextual {
            for (source, translated) in replacements {
                if !self
                    .seen_contextual
                    .contains(&(context_path.clone(), source.clone()))
                {
                    self.entries.push(TranslationCoverageEntry {
                        category: TranslationCoverageCategory::StaleContextualKey,
                        path: format!("translations.contextual.{context_path}.{source}"),
                        source: source.clone(),
                        old_source: None,
                        translated: Some(translated.clone()),
                        context: Some(context_path.clone()),
                    });
                }
            }
        }
        for source in &self.translations.no_change {
            if !self.seen_sources.contains(source) {
                self.entries.push(TranslationCoverageEntry {
                    category: TranslationCoverageCategory::StaleNoChangeKey,
                    path: format!("translations.no_change.{source}"),
                    source: source.clone(),
                    old_source: None,
                    translated: Some(source.clone()),
                    context: None,
                });
            }
        }
        for (path, translated) in &self.translations.target_additions {
            if !self.seen_target_additions.contains(path) {
                self.entries.push(TranslationCoverageEntry {
                    category: TranslationCoverageCategory::StaleTargetAddition,
                    path: format!("translations.target_additions.{path}"),
                    source: String::new(),
                    old_source: None,
                    translated: Some(translated.clone()),
                    context: Some(path.clone()),
                });
            }
        }
        for (index, record) in self.translations.stale_records.iter().enumerate() {
            if !self.seen_stale_records.contains(&index) {
                self.entries.push(TranslationCoverageEntry {
                    category: TranslationCoverageCategory::StaleTranslationRecord,
                    path: format!("translations.stale_records.{index}"),
                    source: record.new_source.clone(),
                    old_source: Some(record.old_source.clone()),
                    translated: Some(record.target.clone()),
                    context: record.context.clone(),
                });
            }
        }
        for (variable_key, replacements) in &self.translations.variables {
            for (source, translated) in replacements {
                if !self
                    .seen_variables
                    .contains(&(variable_key.clone(), source.clone()))
                {
                    self.entries.push(TranslationCoverageEntry {
                        category: TranslationCoverageCategory::StaleVariableKey,
                        path: format!("translations.variables.{variable_key}.{source}"),
                        source: source.clone(),
                        old_source: None,
                        translated: Some(translated.clone()),
                        context: Some(variable_key.clone()),
                    });
                }
            }
        }
        for (adapter_key, replacements) in &self.translations.adapter_ids {
            for (source, translated) in replacements {
                if !self
                    .seen_adapter_ids
                    .contains(&(adapter_key.clone(), source.clone()))
                {
                    self.entries.push(TranslationCoverageEntry {
                        category: TranslationCoverageCategory::StaleAdapterIdKey,
                        path: format!("translations.adapter_ids.{adapter_key}.{source}"),
                        source: source.clone(),
                        old_source: None,
                        translated: Some(translated.clone()),
                        context: Some(adapter_key.clone()),
                    });
                }
            }
        }

        self.entries.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then_with(|| left.category.as_str().cmp(right.category.as_str()))
                .then_with(|| left.source.cmp(&right.source))
        });

        TranslationCoverageReport {
            overlay_id,
            entries: self.entries,
        }
    }
}

fn translation_context_view(
    deck: &CanonicalDeck,
    report: &TranslationCoverageReport,
) -> TranslationContextView {
    let mut source_counts = BTreeMap::<String, usize>::new();
    for entry in &report.entries {
        if !entry.source.is_empty() {
            *source_counts.entry(entry.source.clone()).or_insert(0) += 1;
        }
    }

    let entries_by_path = report
        .entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let units = report
        .entries
        .iter()
        .map(|entry| translation_context_unit(deck, entry, &source_counts, &entries_by_path))
        .collect::<Vec<_>>();

    TranslationContextView {
        overlay_id: report.overlay_id.clone(),
        units,
    }
}

fn translation_context_unit(
    deck: &CanonicalDeck,
    entry: &TranslationCoverageEntry,
    source_counts: &BTreeMap<String, usize>,
    entries_by_path: &BTreeMap<&str, &TranslationCoverageEntry>,
) -> TranslationContextUnit {
    let note_id = note_id_from_translation_path(&entry.path);
    let note_type_id = note_id
        .as_ref()
        .and_then(|note_id| {
            deck.notes
                .get(note_id)
                .map(|note| note.note_type_id.clone())
        })
        .or_else(|| note_type_id_from_translation_path(&entry.path));
    let note_type = note_type_id
        .as_ref()
        .and_then(|note_type_id| deck.note_types.get(note_type_id));
    let field_id = field_id_from_translation_path(&entry.path, note_type);
    let note = note_id.as_ref().and_then(|note_id| deck.notes.get(note_id));
    let field_name = note_type
        .zip(field_id.as_ref())
        .and_then(|(note_type, field_id)| field_definition_name(note_type, field_id))
        .map(str::to_owned);
    let note_fields = note
        .zip(note_type)
        .zip(note_id.as_ref())
        .map(|((note, note_type), note_id)| {
            note_field_contexts(note, note_type, note_id, entries_by_path)
        })
        .unwrap_or_default();
    let message = note.zip(note_id.as_ref()).zip(field_id.as_ref()).and_then(
        |((note, note_id), field_id)| {
            message_context(deck, note, note_id, field_id, entries_by_path)
        },
    );
    let card_templates = note_type
        .zip(field_name.as_deref())
        .map(|(note_type, field_name)| card_contexts_for_field(note_type, field_name))
        .unwrap_or_default();

    TranslationContextUnit {
        category: entry.category,
        path: entry.path.clone(),
        source: entry.source.clone(),
        old_source: entry.old_source.clone(),
        translated: entry.translated.clone(),
        context: entry.context.clone(),
        note_id,
        note_type_id,
        field_id,
        field_name,
        note_fields,
        message,
        card_templates,
        source_occurrences: source_counts.get(&entry.source).copied().unwrap_or(0),
    }
}

fn note_id_from_translation_path(path: &str) -> Option<StableId> {
    let rest = path.strip_prefix("notes.")?;
    let end = [".fields.", ".tags.", ".variables.", ".adapter_ids."]
        .into_iter()
        .filter_map(|marker| rest.find(marker))
        .min()
        .unwrap_or(rest.len());
    StableId::new(rest[..end].to_owned()).ok()
}

fn note_type_id_from_translation_path(path: &str) -> Option<StableId> {
    let rest = path.strip_prefix("note_types.")?;
    let end = [
        ".fields.",
        ".card_templates.",
        ".variables.",
        ".adapter_ids.",
    ]
    .into_iter()
    .filter_map(|marker| rest.find(marker))
    .min()
    .unwrap_or(rest.len());
    StableId::new(rest[..end].to_owned()).ok()
}

fn field_id_from_translation_path(path: &str, note_type: Option<&NoteType>) -> Option<StableId> {
    let rest = path.split_once(".fields.")?.1;
    if let Some(note_type) = note_type {
        return note_type
            .fields
            .iter()
            .filter(|field| {
                rest == field.id.as_str() || rest.starts_with(&format!("{}.", field.id))
            })
            .max_by_key(|field| field.id.as_str().len())
            .map(|field| field.id.clone());
    }
    StableId::new(rest.strip_suffix(".name").unwrap_or(rest).to_owned()).ok()
}

fn field_definition_name<'a>(note_type: &'a NoteType, field_id: &StableId) -> Option<&'a str> {
    note_type
        .fields
        .iter()
        .find(|field| &field.id == field_id)
        .map(|field| field.name.as_str())
}

fn note_field_contexts(
    note: &Note,
    note_type: &NoteType,
    note_id: &StableId,
    entries_by_path: &BTreeMap<&str, &TranslationCoverageEntry>,
) -> Vec<TranslationNoteFieldContext> {
    note_type
        .fields
        .iter()
        .map(|field| {
            let source = note.fields.get(&field.id).cloned().unwrap_or_default();
            let path = format!("notes.{note_id}.fields.{}", field.id);
            let entry = entries_by_path.get(path.as_str()).copied();
            TranslationNoteFieldContext {
                field_id: field.id.clone(),
                field_name: field.name.clone(),
                source: source.clone(),
                translated: entry
                    .and_then(|entry| entry.translated.clone())
                    .unwrap_or(source),
                category: entry.map(|entry| entry.category),
            }
        })
        .collect()
}

fn message_context(
    deck: &CanonicalDeck,
    note: &Note,
    note_id: &StableId,
    field_id: &StableId,
    entries_by_path: &BTreeMap<&str, &TranslationCoverageEntry>,
) -> Option<TranslationMessageContext> {
    let message = note.field_messages.get(field_id)?;
    let resolved_source = note.fields.get(field_id).cloned().unwrap_or_default();
    let full_field_translation = entries_by_path
        .get(format!("notes.{note_id}.fields.{field_id}").as_str())
        .and_then(|entry| entry.translated.clone());

    if let Some(format) = &message.format {
        let format_path = message_format_path(note_id, field_id);
        let format_entry = entries_by_path.get(format_path.as_str()).copied();
        let translated_format = format_entry
            .and_then(|entry| entry.translated.clone())
            .unwrap_or_else(|| format.clone());
        let format_context = TranslationMessageComponentContext {
            index: 0,
            name: None,
            kind: MessageComponentKind::Format,
            path: format_path,
            source: format.clone(),
            translated: translated_format.clone(),
            reference: None,
            category: format_entry.map(|entry| entry.category),
        };
        let mut components = Vec::new();
        let mut translated_variables = BTreeMap::new();
        for (index, (name, component)) in message.variables.iter().enumerate() {
            let path = message_variable_path(note_id, field_id, name);
            let context = message_component_context(
                deck,
                component,
                index,
                Some(name.clone()),
                path,
                entries_by_path,
            );
            translated_variables.insert(name.clone(), context.translated.clone());
            components.push(context);
        }
        let translated = full_field_translation.unwrap_or_else(|| {
            render_message_format(&translated_format, &translated_variables).unwrap_or_else(|_| {
                components
                    .iter()
                    .map(|component| component.translated.as_str())
                    .collect::<String>()
            })
        });
        return Some(TranslationMessageContext {
            source: resolved_source,
            translated,
            format: Some(format_context),
            components,
        });
    }

    let mut components = Vec::new();
    for (index, component) in message.components.iter().enumerate() {
        components.push(message_component_context(
            deck,
            component,
            index,
            None,
            message_component_path(note_id, field_id, index),
            entries_by_path,
        ));
    }
    let translated = full_field_translation.unwrap_or_else(|| {
        components
            .iter()
            .map(|component| component.translated.as_str())
            .collect::<String>()
    });
    Some(TranslationMessageContext {
        source: resolved_source,
        translated,
        format: None,
        components,
    })
}

fn message_component_context(
    deck: &CanonicalDeck,
    component: &MessageComponent,
    index: usize,
    name: Option<String>,
    path: String,
    entries_by_path: &BTreeMap<&str, &TranslationCoverageEntry>,
) -> TranslationMessageComponentContext {
    let entry = entries_by_path.get(path.as_str()).copied();
    let (kind, source, reference) = match component {
        MessageComponent::Literal(value) => (MessageComponentKind::Literal, value.clone(), None),
        MessageComponent::Text(value) => (MessageComponentKind::Text, value.clone(), None),
        MessageComponent::FieldRef(reference) => (
            MessageComponentKind::FieldRef,
            field_value_at_path(deck, reference)
                .unwrap_or_default()
                .to_owned(),
            Some(reference.clone()),
        ),
    };
    let translated = entry
        .and_then(|entry| entry.translated.clone())
        .unwrap_or_else(|| source.clone());
    TranslationMessageComponentContext {
        index,
        name,
        kind,
        path,
        source,
        translated,
        reference,
        category: entry.map(|entry| entry.category),
    }
}

fn card_contexts_for_field(note_type: &NoteType, field_name: &str) -> Vec<TranslationCardContext> {
    note_type
        .card_templates
        .iter()
        .filter_map(|template| {
            let mut sides = BTreeSet::new();
            if template_uses_field(&template.question_format, field_name) {
                sides.insert(CardTemplateSide::Question);
            }
            if template_uses_field(&template.answer_format, field_name) {
                sides.insert(CardTemplateSide::Answer);
            }
            if sides.is_empty() {
                None
            } else {
                Some(TranslationCardContext {
                    template_id: template.id.clone(),
                    template_name: template.name.clone(),
                    sides,
                    question_format: template.question_format.clone(),
                    answer_format: template.answer_format.clone(),
                })
            }
        })
        .collect()
}

fn template_uses_field(template_text: &str, field_name: &str) -> bool {
    [
        format!("{{{{{field_name}}}}}"),
        format!("{{{{#{field_name}}}}}"),
        format!("{{{{/{field_name}}}}}"),
        format!("{{{{^{field_name}}}}}"),
        format!("{{{{type:{field_name}}}}}"),
    ]
    .iter()
    .any(|marker| template_text.contains(marker))
}

fn apply_translation_dictionary(
    resolved: &mut CanonicalDeck,
    overlay: &Overlay,
    translations: &TranslationDictionary,
    changed_paths: &mut BTreeMap<String, StableId>,
    errors: &mut Vec<ComposeError>,
) {
    let source_deck = resolved.clone();
    let mut seen_direct = BTreeSet::new();
    let mut seen_contextual = BTreeSet::new();
    let mut seen_target_additions = BTreeSet::new();
    let mut seen_variables = BTreeSet::new();
    let mut seen_adapter_ids = BTreeSet::new();
    let mut source_paths = BTreeMap::<String, BTreeSet<String>>::new();

    {
        let mut context = TranslationApplyContext {
            overlay,
            translations,
            seen_direct: &mut seen_direct,
            seen_contextual: &mut seen_contextual,
            seen_target_additions: &mut seen_target_additions,
            seen_variables: &mut seen_variables,
            seen_adapter_ids: &mut seen_adapter_ids,
            source_paths: &mut source_paths,
            changed_paths,
            errors,
        };
        context.translate_string(&mut resolved.name, "deck.name".to_owned(), None);
        context.translate_string(
            &mut resolved.description,
            "deck.description".to_owned(),
            None,
        );
        context.translate_variables(&mut resolved.variables, "deck.variables");
        context.translate_adapter_ids(&mut resolved.adapter_ids, "deck.adapter_ids");

        for (note_type_id, note_type) in &mut resolved.note_types {
            context.translate_string(
                &mut note_type.name,
                format!("note_types.{note_type_id}.name"),
                None,
            );
            context.translate_variables(
                &mut note_type.variables,
                &format!("note_types.{note_type_id}.variables"),
            );
            for field in &mut note_type.fields {
                context.translate_string(
                    &mut field.name,
                    format!("note_types.{note_type_id}.fields.{}.name", field.id),
                    None,
                );
            }
            for template in &mut note_type.card_templates {
                context.translate_string(
                    &mut template.name,
                    format!(
                        "note_types.{note_type_id}.card_templates.{}.name",
                        template.id
                    ),
                    None,
                );
                context.translate_variables(
                    &mut template.variables,
                    &format!(
                        "note_types.{note_type_id}.card_templates.{}.variables",
                        template.id
                    ),
                );
                context.translate_adapter_ids(
                    &mut template.adapter_ids,
                    &format!(
                        "note_types.{note_type_id}.card_templates.{}.adapter_ids",
                        template.id
                    ),
                );
            }
            context.translate_adapter_ids(
                &mut note_type.adapter_ids,
                &format!("note_types.{note_type_id}.adapter_ids"),
            );
        }

        for (note_id, note) in &mut resolved.notes {
            context.translate_variables(&mut note.variables, &format!("notes.{note_id}.variables"));
            let field_ids = note.fields.keys().cloned().collect::<Vec<_>>();
            for field_id in field_ids {
                let path = format!("notes.{note_id}.fields.{field_id}");
                if note.field_messages.contains_key(&field_id) {
                    let source = note.fields.get(&field_id).cloned().unwrap_or_default();
                    let full_override = {
                        let message = note
                            .field_messages
                            .get_mut(&field_id)
                            .expect("field message existence checked");
                        context.translate_message_field(
                            &source_deck,
                            &source,
                            path.clone(),
                            note_id,
                            &field_id,
                            message,
                        )
                    };
                    if let Some(translated) = full_override {
                        note.field_messages.remove(&field_id);
                        note.fields.insert(field_id, translated);
                    }
                } else if let Some(value) = note.fields.get_mut(&field_id) {
                    context.translate_string(value, path, None);
                }
            }
            context.translate_tags(&mut note.tags, &format!("notes.{note_id}.tags"));
            context.translate_adapter_ids(
                &mut note.adapter_ids,
                &format!("notes.{note_id}.adapter_ids"),
            );
        }
    }

    for source in translations.direct.keys() {
        if !seen_direct.contains(source) {
            errors.push(ComposeError::new(
                ComposeErrorKind::StaleTranslationEntry,
                format!("translations.direct.{source}"),
                format!(
                    "stale direct translation source {source:?} did not match any extracted non-empty source text; use translations.target_additions for intentionally blank source fields"
                ),
            ));
        }
    }
    for (context_path, replacements) in &translations.contextual {
        for (source, translated) in replacements {
            if !seen_contextual.contains(&(context_path.clone(), source.clone())) {
                errors.push(ComposeError::new(
                    ComposeErrorKind::StaleTranslationEntry,
                    format!("translations.contextual.{context_path}.{source}"),
                    format!(
                        "invalid contextual translation: source {source:?} did not match any extracted text under {context_path}; the source key may be stale, the context may be invalid, or a blank source field should use translations.target_additions"
                    ),
                ));
                continue;
            }
            if let Some(shorter_context) = shortest_safe_context(
                translations,
                &source_paths,
                context_path,
                source,
                translated,
            ) {
                errors.push(ComposeError::new(
                    ComposeErrorKind::ValidationFailed,
                    format!("translations.contextual.{context_path}.{source}"),
                    format!(
                        "contextual translation for source {source:?} is more specific than necessary; use context {shorter_context:?} instead of {context_path:?}"
                    ),
                ));
            }
        }
    }
    for source in &translations.no_change {
        if !source_paths.contains_key(source) {
            errors.push(ComposeError::new(
                ComposeErrorKind::StaleTranslationEntry,
                format!("translations.no_change.{source}"),
                format!(
                    "stale no-change source {source:?} did not match any extracted non-empty source text"
                ),
            ));
        }
    }
    for path in translations.target_additions.keys() {
        if !seen_target_additions.contains(path) {
            errors.push(ComposeError::new(
                ComposeErrorKind::MissingOverlayTarget,
                format!("translations.target_additions.{path}"),
                format!(
                    "target-language addition path {path} did not match any extracted field; use translations.direct or translations.contextual for non-blank source text"
                ),
            ));
        }
    }
    for (variable_key, replacements) in &translations.variables {
        for source in replacements.keys() {
            if !seen_variables.contains(&(variable_key.clone(), source.clone())) {
                errors.push(ComposeError::new(
                    ComposeErrorKind::StaleTranslationEntry,
                    format!("translations.variables.{variable_key}.{source}"),
                    format!(
                        "variable translation source {variable_key}={source:?} did not match any variable"
                    ),
                ));
            }
        }
    }
    for (adapter_key, replacements) in &translations.adapter_ids {
        for source in replacements.keys() {
            if !seen_adapter_ids.contains(&(adapter_key.clone(), source.clone())) {
                errors.push(ComposeError::new(
                    ComposeErrorKind::StaleTranslationEntry,
                    format!("translations.adapter_ids.{adapter_key}.{source}"),
                    format!(
                        "adapter id translation source {adapter_key}={source:?} did not match any adapter id"
                    ),
                ));
            }
        }
    }
}

struct TranslationApplyContext<'a, 'b> {
    overlay: &'a Overlay,
    translations: &'a TranslationDictionary,
    seen_direct: &'b mut BTreeSet<String>,
    seen_contextual: &'b mut BTreeSet<(String, String)>,
    seen_target_additions: &'b mut BTreeSet<String>,
    seen_variables: &'b mut BTreeSet<(String, String)>,
    seen_adapter_ids: &'b mut BTreeSet<(String, String)>,
    source_paths: &'b mut BTreeMap<String, BTreeSet<String>>,
    changed_paths: &'b mut BTreeMap<String, StableId>,
    errors: &'b mut Vec<ComposeError>,
}

impl TranslationApplyContext<'_, '_> {
    fn translate_variables(&mut self, variables: &mut BTreeMap<String, String>, path_prefix: &str) {
        for (key, value) in variables {
            self.translate_string(value, format!("{path_prefix}.{key}"), Some(key));
        }
    }

    fn translate_message_field(
        &mut self,
        source_deck: &CanonicalDeck,
        resolved_source: &str,
        path: String,
        note_id: &StableId,
        field_id: &StableId,
        message: &mut StructuredMessage,
    ) -> Option<String> {
        if self.translations.target_additions.contains_key(&path)
            || self.has_explicit_string_entry(resolved_source, &path, None)
        {
            let mut translated = resolved_source.to_owned();
            self.translate_string(&mut translated, path, None);
            return Some(translated);
        }

        if resolved_source.is_empty() || is_ignored_translation_path(self.translations, &path) {
            return None;
        }

        if let Some(format) = &mut message.format {
            self.translate_optional_string(format, message_format_path(note_id, field_id));
            for (variable, component) in &mut message.variables {
                self.translate_message_component(
                    source_deck,
                    component,
                    message_variable_path(note_id, field_id, variable),
                );
            }
        } else {
            for (index, component) in message.components.iter_mut().enumerate() {
                self.translate_message_component(
                    source_deck,
                    component,
                    message_component_path(note_id, field_id, index),
                );
            }
        }
        None
    }

    fn translate_message_component(
        &mut self,
        source_deck: &CanonicalDeck,
        component: &mut MessageComponent,
        path: String,
    ) {
        match component {
            MessageComponent::Literal(_) => {}
            MessageComponent::Text(value) => {
                self.translate_string(value, path, None);
            }
            MessageComponent::FieldRef(reference) => {
                let Some(source) = field_value_at_path(source_deck, reference) else {
                    return;
                };
                let mut translated = source.to_owned();
                self.translate_string(&mut translated, path, None);
                *component = MessageComponent::Literal(translated);
            }
        }
    }

    fn has_explicit_string_entry(
        &self,
        value: &str,
        path: &str,
        variable_key: Option<&str>,
    ) -> bool {
        if value.is_empty() {
            return false;
        }
        if let Some(variable_key) = variable_key
            && self
                .translations
                .variables
                .get(variable_key)
                .is_some_and(|replacements| replacements.contains_key(value))
        {
            return true;
        }
        self.translations.direct.contains_key(value)
            || self.translations.no_change.contains(value)
            || self
                .translations
                .contextual
                .iter()
                .any(|(context_path, replacements)| {
                    context_matches_path(context_path, path) && replacements.contains_key(value)
                })
            || matching_stale_record(self.translations, value, path).is_some()
    }

    fn translate_string(&mut self, value: &mut String, path: String, variable_key: Option<&str>) {
        if let Some(addition) = self.translations.target_additions.get(&path) {
            self.seen_target_additions.insert(path.clone());
            if !value.is_empty() {
                self.errors.push(ComposeError::new(
                    ComposeErrorKind::ExpectedBaseMismatch,
                    path,
                    format!(
                        "target-language addition expected blank source value, found {value:?}; use translations.direct or translations.contextual for non-blank source text"
                    ),
                ));
                return;
            }
            if !record_change_path(
                &path,
                self.overlay,
                ChangeIntent::Replace,
                self.changed_paths,
                self.errors,
            ) {
                return;
            }
            *value = addition.clone();
            return;
        }

        if value.is_empty() || is_ignored_translation_path(self.translations, &path) {
            return;
        }

        if let Some(variable_key) = variable_key
            && let Some(replacements) = self.translations.variables.get(variable_key)
            && let Some(translated) = replacements.get(value.as_str())
        {
            let source = value.clone();
            self.seen_variables
                .insert((variable_key.to_owned(), source));
            if value != translated {
                if !record_change_path(
                    &path,
                    self.overlay,
                    ChangeIntent::Replace,
                    self.changed_paths,
                    self.errors,
                ) {
                    return;
                }
                *value = translated.clone();
            }
            return;
        }

        let source = value.clone();
        self.source_paths
            .entry(source.clone())
            .or_default()
            .insert(path.clone());
        let direct_translation = self.translations.direct.get(source.as_str());
        if direct_translation.is_some() {
            self.seen_direct.insert(source.clone());
        }
        let no_change = self.translations.no_change.contains(source.as_str());

        let mut contextual_translation: Option<(&String, &String)> = None;
        for (context_path, replacements) in &self.translations.contextual {
            if context_matches_path(context_path, &path)
                && let Some(translated) = replacements.get(source.as_str())
            {
                self.seen_contextual
                    .insert((context_path.clone(), source.clone()));
                if contextual_translation
                    .as_ref()
                    .is_none_or(|(current_context, _)| context_path.len() > current_context.len())
                {
                    contextual_translation = Some((context_path, translated));
                }
            }
        }

        let translated = contextual_translation
            .map(|(_, translated)| translated)
            .or(direct_translation);
        if let Some(translated) = translated {
            if value != translated {
                if !record_change_path(
                    &path,
                    self.overlay,
                    ChangeIntent::Replace,
                    self.changed_paths,
                    self.errors,
                ) {
                    return;
                }
                *value = translated.clone();
            }
            return;
        }

        if no_change {
            return;
        }

        if let Some((_, record)) = matching_stale_record(self.translations, &source, &path) {
            if value != &record.target {
                if !record_change_path(
                    &path,
                    self.overlay,
                    ChangeIntent::Replace,
                    self.changed_paths,
                    self.errors,
                ) {
                    return;
                }
                *value = record.target.clone();
            }
            return;
        }

        if self.translations.require_complete {
            self.errors.push(ComposeError::new(
                ComposeErrorKind::MissingTranslation,
                path.clone(),
                format!(
                    "missing direct or contextual translation for {value:?} at {path}; add translations.direct, add translations.no_change for intentionally unchanged text, add a translations.contextual entry for path-specific text, or ignore the path"
                ),
            ));
        }
    }

    fn translate_optional_string(&mut self, value: &mut String, path: String) {
        if value.is_empty() || is_ignored_translation_path(self.translations, &path) {
            return;
        }

        let source = value.clone();
        self.source_paths
            .entry(source.clone())
            .or_default()
            .insert(path.clone());
        let direct_translation = self.translations.direct.get(source.as_str());
        if direct_translation.is_some() {
            self.seen_direct.insert(source.clone());
        }
        let mut contextual_translation: Option<(&String, &String)> = None;
        for (context_path, replacements) in &self.translations.contextual {
            if context_matches_path(context_path, &path)
                && let Some(translated) = replacements.get(source.as_str())
            {
                self.seen_contextual
                    .insert((context_path.clone(), source.clone()));
                if contextual_translation
                    .as_ref()
                    .is_none_or(|(current_context, _)| context_path.len() > current_context.len())
                {
                    contextual_translation = Some((context_path, translated));
                }
            }
        }

        let translated = contextual_translation
            .map(|(_, translated)| translated)
            .or(direct_translation);
        if let Some(translated) = translated {
            if value != translated {
                if !record_change_path(
                    &path,
                    self.overlay,
                    ChangeIntent::Replace,
                    self.changed_paths,
                    self.errors,
                ) {
                    return;
                }
                *value = translated.clone();
            }
            return;
        }

        if let Some((_, record)) = matching_stale_record(self.translations, &source, &path)
            && value != &record.target
        {
            if !record_change_path(
                &path,
                self.overlay,
                ChangeIntent::Replace,
                self.changed_paths,
                self.errors,
            ) {
                return;
            }
            *value = record.target.clone();
        }
    }

    fn translate_tags(&mut self, tags: &mut BTreeSet<String>, path_prefix: &str) {
        for tag in tags.iter().cloned().collect::<Vec<_>>() {
            let mut translated = tag.clone();
            self.translate_string(&mut translated, format!("{path_prefix}.{tag}"), None);
            if translated != tag {
                tags.remove(&tag);
                tags.insert(translated);
            }
        }
    }

    fn translate_adapter_ids(&mut self, adapter_ids: &mut AdapterIds, path_prefix: &str) {
        let current = adapter_ids
            .iter()
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect::<Vec<_>>();
        for (key, value) in current {
            let Some(replacements) = self.translations.adapter_ids.get(&key) else {
                continue;
            };
            let Some(translated) = replacements.get(&value) else {
                continue;
            };
            self.seen_adapter_ids.insert((key.clone(), value.clone()));
            if value != *translated {
                let path = format!("{path_prefix}.{key}");
                if !record_change_path(
                    &path,
                    self.overlay,
                    ChangeIntent::Replace,
                    self.changed_paths,
                    self.errors,
                ) {
                    continue;
                }
                adapter_ids.insert(key, translated.clone());
            }
        }
    }
}

fn shortest_safe_context(
    translations: &TranslationDictionary,
    source_paths: &BTreeMap<String, BTreeSet<String>>,
    context_path: &str,
    source: &str,
    translated: &str,
) -> Option<String> {
    context_parent_candidates(context_path)
        .into_iter()
        .filter(|candidate| {
            contextual_replacement_is_safe(
                translations,
                source_paths,
                candidate,
                source,
                translated,
            )
        })
        .min_by_key(|candidate| candidate.len())
}

fn contextual_replacement_is_safe(
    translations: &TranslationDictionary,
    source_paths: &BTreeMap<String, BTreeSet<String>>,
    candidate_context: &str,
    source: &str,
    translated: &str,
) -> bool {
    if translations
        .contextual
        .get(candidate_context)
        .and_then(|replacements| replacements.get(source))
        .is_some_and(|existing| existing != translated)
    {
        return false;
    }

    let Some(paths) = source_paths.get(source) else {
        return false;
    };
    paths
        .iter()
        .filter(|path| context_matches_path(candidate_context, path))
        .all(|path| {
            let current = direct_or_contextual_translation_for_path(translations, source, path);
            let candidate = direct_or_contextual_translation_for_path_with_candidate(
                translations,
                candidate_context,
                source,
                translated,
                path,
            );
            current == candidate
        })
}

fn direct_or_contextual_translation_for_path<'a>(
    translations: &'a TranslationDictionary,
    source: &str,
    path: &str,
) -> Option<&'a str> {
    translations
        .contextual
        .iter()
        .filter(|(context_path, replacements)| {
            context_matches_path(context_path, path) && replacements.contains_key(source)
        })
        .max_by_key(|(context_path, _)| context_path.len())
        .and_then(|(_, replacements)| replacements.get(source).map(String::as_str))
        .or_else(|| translations.direct.get(source).map(String::as_str))
}

fn direct_or_contextual_translation_for_path_with_candidate<'a>(
    translations: &'a TranslationDictionary,
    candidate_context: &'a str,
    source: &str,
    translated: &'a str,
    path: &str,
) -> Option<&'a str> {
    let best_contextual = translations
        .contextual
        .iter()
        .filter(|(context_path, replacements)| {
            context_matches_path(context_path, path) && replacements.contains_key(source)
        })
        .max_by_key(|(context_path, _)| context_path.len());

    if context_matches_path(candidate_context, path)
        && best_contextual
            .is_none_or(|(context_path, _)| candidate_context.len() > context_path.len())
    {
        return Some(translated);
    }

    best_contextual
        .and_then(|(_, replacements)| replacements.get(source).map(String::as_str))
        .or_else(|| translations.direct.get(source).map(String::as_str))
}

fn context_parent_candidates(context_path: &str) -> Vec<String> {
    if context_path.contains(".message.variables.") || context_path.ends_with(".message.format") {
        return Vec::new();
    }
    if let Some((note_context, _)) = context_path.split_once(".fields.")
        && note_context.starts_with("notes.")
    {
        return vec![note_context.to_owned()];
    }
    if let Some((note_context, _)) = context_path.split_once(".tags.")
        && note_context.starts_with("notes.")
    {
        return vec![note_context.to_owned()];
    }
    if let Some((note_context, _)) = context_path.split_once(".variables.")
        && note_context.starts_with("notes.")
    {
        return vec![note_context.to_owned()];
    }
    if let Some((note_type_context, _)) = context_path.split_once(".fields.")
        && note_type_context.starts_with("note_types.")
    {
        return vec![note_type_context.to_owned()];
    }
    if let Some((note_type_context, _)) = context_path.split_once(".card_templates.")
        && note_type_context.starts_with("note_types.")
    {
        return vec![note_type_context.to_owned()];
    }
    Vec::new()
}

fn context_matches_path(context_path: &str, path: &str) -> bool {
    path == context_path
        || path
            .strip_prefix(context_path)
            .is_some_and(|suffix| suffix.starts_with('.'))
}

fn matching_stale_record<'a>(
    translations: &'a TranslationDictionary,
    source: &str,
    path: &str,
) -> Option<(usize, &'a StaleTranslationRecord)> {
    translations
        .stale_records
        .iter()
        .enumerate()
        .filter(|(_, record)| {
            record.new_source == source
                && record
                    .context
                    .as_deref()
                    .is_none_or(|context| context_matches_path(context, path))
        })
        .max_by_key(|(_, record)| record.context.as_ref().map_or(0, String::len))
}

fn is_ignored_translation_path(translations: &TranslationDictionary, path: &str) -> bool {
    translations
        .ignore_paths
        .iter()
        .any(|pattern| glob_matches(pattern, path))
}

fn glob_matches(pattern: &str, value: &str) -> bool {
    fn matches_parts(pattern: &[u8], value: &[u8]) -> bool {
        match pattern.split_first() {
            None => value.is_empty(),
            Some((&b'*', rest)) => {
                matches_parts(rest, value)
                    || (!value.is_empty() && matches_parts(pattern, &value[1..]))
            }
            Some((&expected, rest)) => value.split_first().is_some_and(|(&actual, rest_value)| {
                actual == expected && matches_parts(rest, rest_value)
            }),
        }
    }

    matches_parts(pattern.as_bytes(), value.as_bytes())
}

fn apply_deck_change(
    resolved: &mut CanonicalDeck,
    overlay: &Overlay,
    change: &DeckChange,
    changed_paths: &mut BTreeMap<String, StableId>,
    errors: &mut Vec<ComposeError>,
) {
    if let Some(name) = &change.name {
        apply_string_property_change(
            &mut resolved.name,
            overlay,
            "deck.name".to_owned(),
            name,
            changed_paths,
            errors,
        );
    }
    if let Some(description) = &change.description {
        apply_string_property_change(
            &mut resolved.description,
            overlay,
            "deck.description".to_owned(),
            description,
            changed_paths,
            errors,
        );
    }
    apply_variable_changes(
        &mut resolved.variables,
        overlay,
        "deck.variables",
        &change.variables,
        changed_paths,
        errors,
    );
    for (key, adapter_change) in &change.adapter_ids {
        apply_adapter_id_change(
            &mut resolved.adapter_ids,
            overlay,
            format!("deck.adapter_ids.{key}"),
            key,
            adapter_change,
            changed_paths,
            errors,
        );
    }
}

fn apply_note_type_add(
    resolved: &mut CanonicalDeck,
    overlay: &Overlay,
    note_type_id: &StableId,
    change: &NoteTypeChange,
    changed_paths: &mut BTreeMap<String, StableId>,
    errors: &mut Vec<ComposeError>,
) {
    let path = format!("note_types.{note_type_id}");
    if resolved.note_types.contains_key(note_type_id) {
        errors.push(ComposeError::new(
            ComposeErrorKind::AlreadyExists,
            path,
            format!("note type {note_type_id} already exists"),
        ));
        return;
    }
    if !record_change_path(&path, overlay, change.intent, changed_paths, errors) {
        return;
    }
    let Some(note_type) = &change.note_type else {
        errors.push(ComposeError::new(
            ComposeErrorKind::MissingOverlayPayload,
            path,
            format!("add change for note type {note_type_id} must include a note_type"),
        ));
        return;
    };
    if &note_type.id != note_type_id {
        errors.push(ComposeError::new(
            ComposeErrorKind::ValidationFailed,
            path,
            format!(
                "note type payload id {} does not match target {note_type_id}",
                note_type.id
            ),
        ));
        return;
    }
    resolved
        .note_types
        .insert(note_type_id.clone(), note_type.clone());
}

fn apply_note_type_remove(
    resolved: &mut CanonicalDeck,
    overlay: &Overlay,
    note_type_id: &StableId,
    change: &NoteTypeChange,
    changed_paths: &mut BTreeMap<String, StableId>,
    errors: &mut Vec<ComposeError>,
) {
    let path = format!("note_types.{note_type_id}");
    if !record_change_path(&path, overlay, change.intent, changed_paths, errors) {
        return;
    }
    if !has_expected_base(&change.expected_base, path.clone(), errors) {
        return;
    }
    if !resolved.note_types.contains_key(note_type_id) {
        errors.push(ComposeError::new(
            ComposeErrorKind::MissingOverlayTarget,
            path,
            format!("note type {note_type_id} does not exist"),
        ));
        return;
    }
    if resolved
        .notes
        .values()
        .any(|note| &note.note_type_id == note_type_id)
    {
        errors.push(ComposeError::new(
            ComposeErrorKind::ValidationFailed,
            path,
            format!("cannot remove note type {note_type_id} while notes still reference it"),
        ));
        return;
    }
    if let Some(ExpectedBase::Value(expected_value)) = &change.expected_base {
        errors.push(ComposeError::new(
            ComposeErrorKind::ExpectedBaseMismatch,
            path,
            format!("note type removal expected an entity marker, not value {expected_value:?}"),
        ));
        return;
    }
    resolved.note_types.remove(note_type_id);
    resolved.tombstones.insert(note_type_id.clone());
}

fn apply_note_type_change(
    resolved: &mut CanonicalDeck,
    overlay: &Overlay,
    note_type_id: &StableId,
    change: &NoteTypeChange,
    changed_paths: &mut BTreeMap<String, StableId>,
    errors: &mut Vec<ComposeError>,
) -> Vec<(StableId, StableId)> {
    let mut added_fields = Vec::new();
    if requires_expected_base(change.intent)
        && !has_expected_base(
            &change.expected_base,
            format!("note_types.{note_type_id}"),
            errors,
        )
    {
        return added_fields;
    }

    let Some(note_type) = resolved.note_types.get_mut(note_type_id) else {
        errors.push(ComposeError::new(
            ComposeErrorKind::MissingOverlayTarget,
            format!("note_types.{note_type_id}"),
            format!("note type {note_type_id} does not exist"),
        ));
        return added_fields;
    };

    if let Some(name) = &change.name {
        apply_string_property_change(
            &mut note_type.name,
            overlay,
            format!("note_types.{note_type_id}.name"),
            name,
            changed_paths,
            errors,
        );
    }
    apply_variable_changes(
        &mut note_type.variables,
        overlay,
        &format!("note_types.{note_type_id}.variables"),
        &change.variables,
        changed_paths,
        errors,
    );
    if let Some(styling) = &change.styling {
        apply_string_property_change(
            &mut note_type.styling,
            overlay,
            format!("note_types.{note_type_id}.styling"),
            styling,
            changed_paths,
            errors,
        );
    }
    for (key, adapter_change) in &change.adapter_ids {
        apply_adapter_id_change(
            &mut note_type.adapter_ids,
            overlay,
            format!("note_types.{note_type_id}.adapter_ids.{key}"),
            key,
            adapter_change,
            changed_paths,
            errors,
        );
    }
    for (template_id, template_change) in &change.card_templates {
        apply_card_template_change(
            note_type,
            overlay,
            note_type_id,
            template_id,
            template_change,
            changed_paths,
            errors,
        );
    }

    for (field_id, field_change) in &change.fields {
        if apply_field_definition_change(
            note_type,
            overlay,
            note_type_id,
            field_id,
            field_change,
            changed_paths,
            errors,
        ) {
            added_fields.push((note_type_id.clone(), field_id.clone()));
        }
    }

    added_fields
}

fn apply_card_template_change(
    note_type: &mut NoteType,
    overlay: &Overlay,
    note_type_id: &StableId,
    template_id: &StableId,
    change: &CardTemplateChange,
    changed_paths: &mut BTreeMap<String, StableId>,
    errors: &mut Vec<ComposeError>,
) {
    let path = format!("note_types.{note_type_id}.card_templates.{template_id}");
    if requires_expected_base(change.intent)
        && !has_expected_base(&change.expected_base, path.clone(), errors)
    {
        return;
    }

    let existing_index = note_type
        .card_templates
        .iter()
        .position(|template| &template.id == template_id);

    match change.intent {
        ChangeIntent::Add if existing_index.is_some() => {
            errors.push(ComposeError::new(
                ComposeErrorKind::AlreadyExists,
                path,
                format!("card template {template_id} already exists on note type {note_type_id}"),
            ));
            return;
        }
        ChangeIntent::Add => {
            if !record_change_path(&path, overlay, change.intent, changed_paths, errors) {
                return;
            }
            let Some(template) = &change.template else {
                errors.push(ComposeError::new(
                    ComposeErrorKind::MissingOverlayPayload,
                    path,
                    format!("add change for card template {template_id} must include a template"),
                ));
                return;
            };
            if &template.id != template_id {
                errors.push(ComposeError::new(
                    ComposeErrorKind::ValidationFailed,
                    path,
                    format!(
                        "card template payload id {} does not match target {template_id}",
                        template.id
                    ),
                ));
                return;
            }
            let insert_index = match &change.insert_after {
                Some(after_id) => match note_type
                    .card_templates
                    .iter()
                    .position(|template| &template.id == after_id)
                {
                    Some(index) => index + 1,
                    None => {
                        errors.push(ComposeError::new(
                            ComposeErrorKind::MissingOverlayTarget,
                            path,
                            format!("insert_after template {after_id} does not exist"),
                        ));
                        return;
                    }
                },
                None => note_type.card_templates.len(),
            };
            note_type
                .card_templates
                .insert(insert_index, template.clone());
        }
        ChangeIntent::Remove => {
            if !record_change_path(&path, overlay, change.intent, changed_paths, errors) {
                return;
            }
            if let Some(index) = existing_index {
                note_type.card_templates.remove(index);
            } else {
                errors.push(ComposeError::new(
                    ComposeErrorKind::MissingOverlayTarget,
                    path,
                    format!(
                        "card template {template_id} does not exist on note type {note_type_id}"
                    ),
                ));
            }
            return;
        }
        ChangeIntent::Merge | ChangeIntent::Replace | ChangeIntent::Override => {
            if let Some(template) = &change.template {
                if !record_change_path(&path, overlay, change.intent, changed_paths, errors) {
                    return;
                }
                let Some(index) = existing_index else {
                    errors.push(ComposeError::new(
                        ComposeErrorKind::MissingOverlayTarget,
                        path,
                        format!(
                            "card template {template_id} does not exist on note type {note_type_id}"
                        ),
                    ));
                    return;
                };
                if &template.id != template_id {
                    errors.push(ComposeError::new(
                        ComposeErrorKind::ValidationFailed,
                        path,
                        format!(
                            "card template payload id {} does not match target {template_id}",
                            template.id
                        ),
                    ));
                    return;
                }
                note_type.card_templates[index] = template.clone();
            }
        }
    }

    let Some(template) = note_type
        .card_templates
        .iter_mut()
        .find(|template| &template.id == template_id)
    else {
        errors.push(ComposeError::new(
            ComposeErrorKind::MissingOverlayTarget,
            path,
            format!("card template {template_id} does not exist on note type {note_type_id}"),
        ));
        return;
    };

    if let Some(name) = &change.name {
        apply_string_property_change(
            &mut template.name,
            overlay,
            format!("note_types.{note_type_id}.card_templates.{template_id}.name"),
            name,
            changed_paths,
            errors,
        );
    }
    apply_variable_changes(
        &mut template.variables,
        overlay,
        &format!("note_types.{note_type_id}.card_templates.{template_id}.variables"),
        &change.variables,
        changed_paths,
        errors,
    );
    if let Some(question_format) = &change.question_format {
        apply_string_property_change(
            &mut template.question_format,
            overlay,
            format!("note_types.{note_type_id}.card_templates.{template_id}.question_format"),
            question_format,
            changed_paths,
            errors,
        );
    }
    if let Some(answer_format) = &change.answer_format {
        apply_string_property_change(
            &mut template.answer_format,
            overlay,
            format!("note_types.{note_type_id}.card_templates.{template_id}.answer_format"),
            answer_format,
            changed_paths,
            errors,
        );
    }
    for (key, adapter_change) in &change.adapter_ids {
        apply_adapter_id_change(
            &mut template.adapter_ids,
            overlay,
            format!("note_types.{note_type_id}.card_templates.{template_id}.adapter_ids.{key}"),
            key,
            adapter_change,
            changed_paths,
            errors,
        );
    }
}

fn apply_string_property_change(
    value: &mut String,
    overlay: &Overlay,
    path: String,
    change: &PropertyChange,
    changed_paths: &mut BTreeMap<String, StableId>,
    errors: &mut Vec<ComposeError>,
) {
    if !record_change_path(&path, overlay, change.intent, changed_paths, errors) {
        return;
    }

    if requires_expected_base(change.intent)
        && !has_expected_base(&change.expected_base, path.clone(), errors)
    {
        return;
    }

    if let Some(expected_base) = &change.expected_base {
        match expected_base {
            ExpectedBase::Value(expected_value) => {
                if value != expected_value {
                    errors.push(ComposeError::new(
                        ComposeErrorKind::ExpectedBaseMismatch,
                        path,
                        format!(
                            "expected base value {:?}, found {:?}",
                            expected_value, value
                        ),
                    ));
                    return;
                }
            }
            ExpectedBase::EntityPresent => {
                if value.is_empty() {
                    errors.push(ComposeError::new(
                        ComposeErrorKind::ExpectedBaseMismatch,
                        path,
                        "expected property to be present".to_owned(),
                    ));
                    return;
                }
            }
        }
    }

    match change.intent {
        ChangeIntent::Add if !value.is_empty() => {
            errors.push(ComposeError::new(
                ComposeErrorKind::AlreadyExists,
                path,
                "property already has a value".to_owned(),
            ));
        }
        ChangeIntent::Remove => value.clear(),
        ChangeIntent::Add
        | ChangeIntent::Merge
        | ChangeIntent::Replace
        | ChangeIntent::Override => {
            let Some(new_value) = &change.value else {
                errors.push(ComposeError::new(
                    ComposeErrorKind::MissingOverlayPayload,
                    path,
                    "property change must include a value".to_owned(),
                ));
                return;
            };
            *value = new_value.clone();
        }
    }
}

fn apply_variable_changes(
    variables: &mut BTreeMap<String, String>,
    overlay: &Overlay,
    path_prefix: &str,
    changes: &BTreeMap<String, PropertyChange>,
    changed_paths: &mut BTreeMap<String, StableId>,
    errors: &mut Vec<ComposeError>,
) {
    for (key, change) in changes {
        apply_map_string_property_change(
            variables,
            overlay,
            format!("{path_prefix}.{key}"),
            key,
            change,
            changed_paths,
            errors,
        );
    }
}

fn apply_map_string_property_change(
    values: &mut BTreeMap<String, String>,
    overlay: &Overlay,
    path: String,
    key: &str,
    change: &PropertyChange,
    changed_paths: &mut BTreeMap<String, StableId>,
    errors: &mut Vec<ComposeError>,
) {
    if !record_change_path(&path, overlay, change.intent, changed_paths, errors) {
        return;
    }

    if requires_expected_base(change.intent)
        && !has_expected_base(&change.expected_base, path.clone(), errors)
    {
        return;
    }

    if let Some(expected_base) = &change.expected_base {
        match expected_base {
            ExpectedBase::Value(expected_value) => {
                let current_value = values.get(key);
                if current_value != Some(expected_value) {
                    errors.push(ComposeError::new(
                        ComposeErrorKind::ExpectedBaseMismatch,
                        path,
                        format!(
                            "expected base value {:?}, found {:?}",
                            expected_value, current_value
                        ),
                    ));
                    return;
                }
            }
            ExpectedBase::EntityPresent => {
                if !values.contains_key(key) {
                    errors.push(ComposeError::new(
                        ComposeErrorKind::ExpectedBaseMismatch,
                        path,
                        format!("expected variable {key} to be present"),
                    ));
                    return;
                }
            }
        }
    }

    match change.intent {
        ChangeIntent::Add if values.contains_key(key) => {
            errors.push(ComposeError::new(
                ComposeErrorKind::AlreadyExists,
                path,
                format!("variable {key} already exists"),
            ));
        }
        ChangeIntent::Remove => {
            values.remove(key);
        }
        ChangeIntent::Add
        | ChangeIntent::Merge
        | ChangeIntent::Replace
        | ChangeIntent::Override => {
            let Some(value) = &change.value else {
                errors.push(ComposeError::new(
                    ComposeErrorKind::MissingOverlayPayload,
                    path,
                    format!("variable change for {key} must include a value"),
                ));
                return;
            };
            values.insert(key.to_owned(), value.clone());
        }
    }
}

fn apply_adapter_id_change(
    adapter_ids: &mut AdapterIds,
    overlay: &Overlay,
    path: String,
    key: &str,
    change: &AdapterIdChange,
    changed_paths: &mut BTreeMap<String, StableId>,
    errors: &mut Vec<ComposeError>,
) {
    if !record_change_path(&path, overlay, change.intent, changed_paths, errors) {
        return;
    }

    if requires_expected_base(change.intent)
        && !has_expected_base(&change.expected_base, path.clone(), errors)
    {
        return;
    }

    if let Some(expected_base) = &change.expected_base {
        match expected_base {
            ExpectedBase::Value(expected_value) => {
                let current_value = adapter_ids.get(key);
                if current_value != Some(expected_value.as_str()) {
                    errors.push(ComposeError::new(
                        ComposeErrorKind::ExpectedBaseMismatch,
                        path,
                        format!(
                            "expected base value {:?}, found {:?}",
                            expected_value, current_value
                        ),
                    ));
                    return;
                }
            }
            ExpectedBase::EntityPresent => {
                if !adapter_ids.contains_key(key) {
                    errors.push(ComposeError::new(
                        ComposeErrorKind::ExpectedBaseMismatch,
                        path,
                        format!("expected adapter id {key} to be present"),
                    ));
                    return;
                }
            }
        }
    }

    match change.intent {
        ChangeIntent::Add if adapter_ids.contains_key(key) => {
            errors.push(ComposeError::new(
                ComposeErrorKind::AlreadyExists,
                path,
                format!("adapter id {key} already exists"),
            ));
        }
        ChangeIntent::Remove => {
            adapter_ids.remove(key);
        }
        ChangeIntent::Add
        | ChangeIntent::Merge
        | ChangeIntent::Replace
        | ChangeIntent::Override => {
            let Some(value) = &change.value else {
                errors.push(ComposeError::new(
                    ComposeErrorKind::MissingOverlayPayload,
                    path,
                    format!("adapter id change for {key} must include a value"),
                ));
                return;
            };
            adapter_ids.insert(key.to_owned(), value.clone());
        }
    }
}

fn apply_field_definition_change(
    note_type: &mut NoteType,
    overlay: &Overlay,
    note_type_id: &StableId,
    field_id: &StableId,
    change: &FieldDefinitionChange,
    changed_paths: &mut BTreeMap<String, StableId>,
    errors: &mut Vec<ComposeError>,
) -> bool {
    let path = format!("note_types.{note_type_id}.fields.{field_id}");
    if !record_change_path(&path, overlay, change.intent, changed_paths, errors) {
        return false;
    }

    if requires_expected_base(change.intent)
        && !has_expected_base(&change.expected_base, path.clone(), errors)
    {
        return false;
    }

    let existing_index = note_type
        .fields
        .iter()
        .position(|field| &field.id == field_id);
    match change.intent {
        ChangeIntent::Add if existing_index.is_some() => {
            errors.push(ComposeError::new(
                ComposeErrorKind::AlreadyExists,
                path,
                format!("field {field_id} already exists on note type {note_type_id}"),
            ));
            false
        }
        ChangeIntent::Remove => {
            if let Some(index) = existing_index {
                note_type.fields.remove(index);
            } else {
                errors.push(ComposeError::new(
                    ComposeErrorKind::MissingOverlayTarget,
                    path,
                    format!("field {field_id} does not exist on note type {note_type_id}"),
                ));
            }
            false
        }
        ChangeIntent::Add
        | ChangeIntent::Merge
        | ChangeIntent::Replace
        | ChangeIntent::Override => {
            let Some(field) = &change.field else {
                errors.push(ComposeError::new(
                    ComposeErrorKind::MissingOverlayPayload,
                    path,
                    format!("field definition change for {field_id} must include a field"),
                ));
                return false;
            };
            if &field.id != field_id {
                errors.push(ComposeError::new(
                    ComposeErrorKind::ValidationFailed,
                    path,
                    format!(
                        "field payload id {} does not match target {field_id}",
                        field.id
                    ),
                ));
                return false;
            }
            if let Some(index) = existing_index {
                note_type.fields[index] = field.clone();
                false
            } else {
                note_type.fields.push(field.clone());
                change.intent == ChangeIntent::Add
            }
        }
    }
}

fn apply_note_add(
    resolved: &mut CanonicalDeck,
    overlay: &Overlay,
    note_id: &StableId,
    change: &NoteChange,
    changed_paths: &mut BTreeMap<String, StableId>,
    errors: &mut Vec<ComposeError>,
) {
    let path = format!("notes.{note_id}");
    if !record_change_path(&path, overlay, change.intent, changed_paths, errors) {
        return;
    }

    if resolved.notes.contains_key(note_id) && !resolved.tombstones.contains(note_id) {
        errors.push(ComposeError::new(
            ComposeErrorKind::AlreadyExists,
            path,
            format!("note {note_id} already exists"),
        ));
        return;
    }

    let Some(note) = &change.note else {
        errors.push(ComposeError::new(
            ComposeErrorKind::MissingOverlayPayload,
            path,
            format!("add change for note {note_id} must include a note payload"),
        ));
        return;
    };

    resolved.notes.insert(note_id.clone(), note.clone());
    resolved.tombstones.remove(note_id);
}

fn apply_note_merge(
    resolved: &mut CanonicalDeck,
    overlay: &Overlay,
    note_id: &StableId,
    change: &NoteChange,
    changed_paths: &mut BTreeMap<String, StableId>,
    errors: &mut Vec<ComposeError>,
) {
    if requires_expected_base(change.intent)
        && !has_expected_base(&change.expected_base, format!("notes.{note_id}"), errors)
    {
        return;
    }

    let Some(note) = resolved.notes.get_mut(note_id) else {
        errors.push(ComposeError::new(
            ComposeErrorKind::MissingOverlayTarget,
            format!("notes.{note_id}"),
            format!("note {note_id} does not exist"),
        ));
        return;
    };

    apply_variable_changes(
        &mut note.variables,
        overlay,
        &format!("notes.{note_id}.variables"),
        &change.variables,
        changed_paths,
        errors,
    );

    for (tag, tag_change) in &change.tags {
        apply_tag_change(
            note,
            overlay,
            note_id,
            tag,
            tag_change,
            changed_paths,
            errors,
        );
    }

    for (key, adapter_change) in &change.adapter_ids {
        apply_adapter_id_change(
            &mut note.adapter_ids,
            overlay,
            format!("notes.{note_id}.adapter_ids.{key}"),
            key,
            adapter_change,
            changed_paths,
            errors,
        );
    }

    for (field_id, field_change) in &change.fields {
        apply_field_change(
            note,
            overlay,
            note_id,
            field_id,
            field_change,
            changed_paths,
            errors,
        );
    }
}

fn apply_tag_change(
    note: &mut Note,
    overlay: &Overlay,
    note_id: &StableId,
    tag: &str,
    change: &TagChange,
    changed_paths: &mut BTreeMap<String, StableId>,
    errors: &mut Vec<ComposeError>,
) {
    let path = format!("notes.{note_id}.tags.{tag}");
    if !record_change_path(&path, overlay, change.intent, changed_paths, errors) {
        return;
    }

    if requires_expected_base(change.intent)
        && !has_expected_base(&change.expected_base, path.clone(), errors)
    {
        return;
    }

    if let Some(expected_base) = &change.expected_base {
        match expected_base {
            ExpectedBase::EntityPresent => {
                if !note.tags.contains(tag) {
                    errors.push(ComposeError::new(
                        ComposeErrorKind::ExpectedBaseMismatch,
                        path,
                        format!("expected tag {tag} to be present"),
                    ));
                    return;
                }
            }
            ExpectedBase::Value(expected_value) => {
                if expected_value != tag || !note.tags.contains(tag) {
                    errors.push(ComposeError::new(
                        ComposeErrorKind::ExpectedBaseMismatch,
                        path,
                        format!("expected tag value {:?} to be present", expected_value),
                    ));
                    return;
                }
            }
        }
    }

    match change.intent {
        ChangeIntent::Add if note.tags.contains(tag) => {
            errors.push(ComposeError::new(
                ComposeErrorKind::AlreadyExists,
                path,
                format!("tag {tag} already exists on note {note_id}"),
            ));
        }
        ChangeIntent::Remove => {
            note.tags.remove(tag);
        }
        ChangeIntent::Add
        | ChangeIntent::Merge
        | ChangeIntent::Replace
        | ChangeIntent::Override => {
            note.tags.insert(tag.to_owned());
        }
    }
}

fn apply_field_change(
    note: &mut Note,
    overlay: &Overlay,
    note_id: &StableId,
    field_id: &StableId,
    change: &FieldChange,
    changed_paths: &mut BTreeMap<String, StableId>,
    errors: &mut Vec<ComposeError>,
) {
    let path = format!("notes.{note_id}.fields.{field_id}");
    if !record_change_path(&path, overlay, change.intent, changed_paths, errors) {
        return;
    }

    if requires_expected_base(change.intent)
        && !has_expected_base(&change.expected_base, path.clone(), errors)
    {
        return;
    }

    if let Some(expected_base) = &change.expected_base {
        match expected_base {
            ExpectedBase::Value(expected_value) => {
                let current_value = note.fields.get(field_id).map(String::as_str);
                if current_value != Some(expected_value.as_str()) {
                    errors.push(ComposeError::new(
                        ComposeErrorKind::ExpectedBaseMismatch,
                        path,
                        format!(
                            "expected base value {:?}, found {:?}",
                            expected_value, current_value
                        ),
                    ));
                    return;
                }
            }
            ExpectedBase::EntityPresent => {
                if !note.fields.contains_key(field_id) {
                    errors.push(ComposeError::new(
                        ComposeErrorKind::ExpectedBaseMismatch,
                        path,
                        format!("expected field {field_id} to be present"),
                    ));
                    return;
                }
            }
        }
    }

    match change.intent {
        ChangeIntent::Add if note.fields.contains_key(field_id) => {
            errors.push(ComposeError::new(
                ComposeErrorKind::AlreadyExists,
                path,
                format!("field {field_id} already exists on note {note_id}"),
            ));
        }
        ChangeIntent::Remove => {
            note.fields.remove(field_id);
            note.field_messages.remove(field_id);
        }
        ChangeIntent::Add
        | ChangeIntent::Merge
        | ChangeIntent::Replace
        | ChangeIntent::Override => {
            if let Some(message) = &change.message {
                note.fields.insert(field_id.clone(), String::new());
                note.field_messages
                    .insert(field_id.clone(), message.clone());
                return;
            }
            let Some(value) = &change.value else {
                errors.push(ComposeError::new(
                    ComposeErrorKind::MissingOverlayPayload,
                    path,
                    format!("field change for {field_id} must include a value or message"),
                ));
                return;
            };
            note.fields.insert(field_id.clone(), value.clone());
            note.field_messages.remove(field_id);
        }
    }
}

fn apply_media_change(
    resolved: &mut CanonicalDeck,
    overlay: &Overlay,
    media_id: &StableId,
    change: &MediaChange,
    changed_paths: &mut BTreeMap<String, StableId>,
    errors: &mut Vec<ComposeError>,
) {
    let path = format!("media.{media_id}");
    if !record_change_path(&path, overlay, change.intent, changed_paths, errors) {
        return;
    }

    if requires_expected_base(change.intent)
        && !has_expected_base(&change.expected_base, path.clone(), errors)
    {
        return;
    }

    if let Some(expected_base) = &change.expected_base {
        match expected_base {
            ExpectedBase::EntityPresent => {
                if !resolved.media.contains_key(media_id) {
                    errors.push(ComposeError::new(
                        ComposeErrorKind::ExpectedBaseMismatch,
                        path,
                        format!("expected media reference {media_id} to be present"),
                    ));
                    return;
                }
            }
            ExpectedBase::Value(expected_value) => {
                let current_value = resolved.media.get(media_id).map(media_reference_summary);
                if current_value.as_deref() != Some(expected_value.as_str()) {
                    errors.push(ComposeError::new(
                        ComposeErrorKind::ExpectedBaseMismatch,
                        path,
                        format!(
                            "expected base value {:?}, found {:?}",
                            expected_value, current_value
                        ),
                    ));
                    return;
                }
            }
        }
    }

    match change.intent {
        ChangeIntent::Add if resolved.media.contains_key(media_id) => {
            errors.push(ComposeError::new(
                ComposeErrorKind::AlreadyExists,
                path,
                format!("media reference {media_id} already exists"),
            ));
        }
        ChangeIntent::Remove => {
            resolved.media.remove(media_id);
        }
        ChangeIntent::Add
        | ChangeIntent::Merge
        | ChangeIntent::Replace
        | ChangeIntent::Override => {
            let Some(media) = &change.media else {
                errors.push(ComposeError::new(
                    ComposeErrorKind::MissingOverlayPayload,
                    path,
                    format!("media change for {media_id} must include a media reference"),
                ));
                return;
            };
            if &media.id != media_id {
                errors.push(ComposeError::new(
                    ComposeErrorKind::ValidationFailed,
                    path,
                    format!(
                        "media payload id {} does not match target {media_id}",
                        media.id
                    ),
                ));
                return;
            }
            resolved.media.insert(media_id.clone(), media.clone());
        }
    }
}

fn media_reference_summary(media: &MediaReference) -> String {
    format!("path={};sha256={}", media.path, media.sha256)
}

fn apply_note_remove(
    resolved: &mut CanonicalDeck,
    overlay: &Overlay,
    note_id: &StableId,
    change: &NoteChange,
    changed_paths: &mut BTreeMap<String, StableId>,
    errors: &mut Vec<ComposeError>,
) {
    let path = format!("notes.{note_id}");
    if !record_change_path(&path, overlay, change.intent, changed_paths, errors) {
        return;
    }

    if !has_expected_base(&change.expected_base, path.clone(), errors) {
        return;
    }

    if !resolved.notes.contains_key(note_id) {
        errors.push(ComposeError::new(
            ComposeErrorKind::MissingOverlayTarget,
            path,
            format!("note {note_id} does not exist"),
        ));
        return;
    }

    if let Some(ExpectedBase::Value(expected_value)) = &change.expected_base {
        errors.push(ComposeError::new(
            ComposeErrorKind::ExpectedBaseMismatch,
            path,
            format!("note removal expected an entity marker, not value {expected_value:?}"),
        ));
        return;
    }

    resolved.tombstones.insert(note_id.clone());
}

fn record_change_path(
    path: &str,
    overlay: &Overlay,
    intent: ChangeIntent,
    changed_paths: &mut BTreeMap<String, StableId>,
    errors: &mut Vec<ComposeError>,
) -> bool {
    if let Some(previous_overlay_id) = changed_paths.get(path)
        && intent != ChangeIntent::Override
    {
        errors.push(ComposeError::new(
            ComposeErrorKind::Conflict,
            path.to_owned(),
            format!(
                "overlay {} conflicts with earlier overlay {} at {path}",
                overlay.id, previous_overlay_id
            ),
        ));
        return false;
    }

    changed_paths.insert(path.to_owned(), overlay.id.clone());
    true
}

fn has_expected_base(
    expected_base: &Option<ExpectedBase>,
    path: String,
    errors: &mut Vec<ComposeError>,
) -> bool {
    if expected_base.is_some() {
        true
    } else {
        errors.push(ComposeError::new(
            ComposeErrorKind::MissingExpectedBase,
            path,
            "destructive overlay change must declare an expected base".to_owned(),
        ));
        false
    }
}

fn requires_expected_base(intent: ChangeIntent) -> bool {
    matches!(
        intent,
        ChangeIntent::Replace | ChangeIntent::Remove | ChangeIntent::Override
    )
}

fn render_deck_variables(deck: &CanonicalDeck) -> Result<CanonicalDeck, VariableRenderReport> {
    let mut rendered = deck.clone();
    let mut errors = Vec::new();
    let deck_variables = rendered.variables.clone();

    render_string_with_variables(
        &mut rendered.name,
        "deck.name",
        &[&deck_variables],
        &mut errors,
    );
    render_string_with_variables(
        &mut rendered.description,
        "deck.description",
        &[&deck_variables],
        &mut errors,
    );

    for (note_type_id, note_type) in &mut rendered.note_types {
        let note_type_variables = note_type.variables.clone();
        render_string_with_variables(
            &mut note_type.name,
            &format!("note_types.{note_type_id}.name"),
            &[&note_type_variables, &deck_variables],
            &mut errors,
        );
        render_string_with_variables(
            &mut note_type.styling,
            &format!("note_types.{note_type_id}.styling"),
            &[&note_type_variables, &deck_variables],
            &mut errors,
        );
        for field in &mut note_type.fields {
            render_string_with_variables(
                &mut field.name,
                &format!("note_types.{note_type_id}.fields.{}.name", field.id),
                &[&note_type_variables, &deck_variables],
                &mut errors,
            );
        }
        for template in &mut note_type.card_templates {
            let template_variables = template.variables.clone();
            let scopes = [&template_variables, &note_type_variables, &deck_variables];
            render_string_with_variables(
                &mut template.name,
                &format!(
                    "note_types.{note_type_id}.card_templates.{}.name",
                    template.id
                ),
                &scopes,
                &mut errors,
            );
            render_string_with_variables(
                &mut template.question_format,
                &format!(
                    "note_types.{note_type_id}.card_templates.{}.question_format",
                    template.id
                ),
                &scopes,
                &mut errors,
            );
            render_string_with_variables(
                &mut template.answer_format,
                &format!(
                    "note_types.{note_type_id}.card_templates.{}.answer_format",
                    template.id
                ),
                &scopes,
                &mut errors,
            );
        }
    }

    for (note_id, note) in &mut rendered.notes {
        let note_variables = note.variables.clone();
        let note_type_variables = rendered
            .note_types
            .get(&note.note_type_id)
            .map(|note_type| &note_type.variables);
        for (field_id, value) in &mut note.fields {
            let path = format!("notes.{note_id}.fields.{field_id}");
            if let Some(note_type_variables) = note_type_variables {
                render_string_with_variables(
                    value,
                    &path,
                    &[&note_variables, note_type_variables, &deck_variables],
                    &mut errors,
                );
            } else {
                render_string_with_variables(
                    value,
                    &path,
                    &[&note_variables, &deck_variables],
                    &mut errors,
                );
            }
        }
        for (field_id, message) in &mut note.field_messages {
            let scopes = if let Some(note_type_variables) = note_type_variables {
                vec![&note_variables, note_type_variables, &deck_variables]
            } else {
                vec![&note_variables, &deck_variables]
            };
            render_message_variables(
                message,
                &format!("notes.{note_id}.fields.{field_id}.message"),
                &scopes,
                &mut errors,
            );
        }
    }

    if errors.is_empty() {
        let mut message_errors = Vec::new();
        resolve_structured_messages_with_validation_errors(&mut rendered, &mut message_errors);
        Ok(rendered)
    } else {
        Err(VariableRenderReport { errors })
    }
}

fn render_message_variables(
    message: &mut StructuredMessage,
    path: &str,
    scopes: &[&BTreeMap<String, String>],
    errors: &mut Vec<VariableRenderError>,
) {
    for (index, component) in message.components.iter_mut().enumerate() {
        match component {
            MessageComponent::Literal(value) | MessageComponent::Text(value) => {
                render_string_with_variables(value, &format!("{path}.{index}"), scopes, errors);
            }
            MessageComponent::FieldRef(_) => {}
        }
    }
}

fn render_string_with_variables(
    value: &mut String,
    path: &str,
    scopes: &[&BTreeMap<String, String>],
    errors: &mut Vec<VariableRenderError>,
) {
    let mut rendered = String::new();
    let mut remaining = value.as_str();
    while let Some(start) = remaining.find("${") {
        rendered.push_str(&remaining[..start]);
        let after_start = &remaining[start + 2..];
        let Some(end) = after_start.find('}') else {
            rendered.push_str(&remaining[start..]);
            *value = rendered;
            return;
        };
        let key = &after_start[..end];
        if let Some(replacement) = lookup_variable(scopes, key) {
            rendered.push_str(replacement);
        } else {
            errors.push(VariableRenderError {
                path: path.to_owned(),
                variable: key.to_owned(),
            });
            rendered.push_str(&remaining[start..start + end + 3]);
        }
        remaining = &after_start[end + 1..];
    }
    rendered.push_str(remaining);
    *value = rendered;
}

fn lookup_variable<'a>(scopes: &[&'a BTreeMap<String, String>], key: &str) -> Option<&'a str> {
    scopes
        .iter()
        .find_map(|scope| scope.get(key).map(String::as_str))
}

fn diff_note_types(
    left: &BTreeMap<StableId, NoteType>,
    right: &BTreeMap<StableId, NoteType>,
    changes: &mut Vec<SemanticChange>,
) {
    for id in left.keys() {
        if !right.contains_key(id) {
            changes.push(SemanticChange::removed(format!("note_types.{id}")));
        }
    }

    for (id, right_note_type) in right {
        let Some(left_note_type) = left.get(id) else {
            changes.push(SemanticChange::added(format!("note_types.{id}")));
            continue;
        };

        push_modified_if_changed(
            changes,
            format!("note_types.{id}.name"),
            &left_note_type.name,
            &right_note_type.name,
        );
        push_modified_if_changed(
            changes,
            format!("note_types.{id}.styling"),
            &left_note_type.styling,
            &right_note_type.styling,
        );
        push_modified_if_changed(
            changes,
            format!("note_types.{id}.variables"),
            &string_map_summary(&left_note_type.variables),
            &string_map_summary(&right_note_type.variables),
        );
        push_modified_if_changed(
            changes,
            format!("note_types.{id}.fields"),
            &field_summary(&left_note_type.fields),
            &field_summary(&right_note_type.fields),
        );
        push_modified_if_changed(
            changes,
            format!("note_types.{id}.card_templates"),
            &template_summary(&left_note_type.card_templates),
            &template_summary(&right_note_type.card_templates),
        );
        push_modified_if_changed(
            changes,
            format!("note_types.{id}.adapter_ids"),
            &adapter_ids_summary(&left_note_type.adapter_ids),
            &adapter_ids_summary(&right_note_type.adapter_ids),
        );
    }
}

fn diff_notes(
    left: &BTreeMap<StableId, Note>,
    right: &BTreeMap<StableId, Note>,
    changes: &mut Vec<SemanticChange>,
) {
    for id in left.keys() {
        if !right.contains_key(id) {
            changes.push(SemanticChange::removed(format!("notes.{id}")));
        }
    }

    for (id, right_note) in right {
        let Some(left_note) = left.get(id) else {
            changes.push(SemanticChange::added(format!("notes.{id}")));
            continue;
        };

        push_modified_if_changed(
            changes,
            format!("notes.{id}.note_type_id"),
            &left_note.note_type_id.to_string(),
            &right_note.note_type_id.to_string(),
        );
        push_modified_if_changed(
            changes,
            format!("notes.{id}.variables"),
            &string_map_summary(&left_note.variables),
            &string_map_summary(&right_note.variables),
        );
        diff_note_fields(id, &left_note.fields, &right_note.fields, changes);
        push_modified_if_changed(
            changes,
            format!("notes.{id}.tags"),
            &set_summary(&left_note.tags),
            &set_summary(&right_note.tags),
        );
        push_modified_if_changed(
            changes,
            format!("notes.{id}.adapter_ids"),
            &adapter_ids_summary(&left_note.adapter_ids),
            &adapter_ids_summary(&right_note.adapter_ids),
        );
    }
}

fn diff_note_fields(
    note_id: &StableId,
    left: &BTreeMap<StableId, String>,
    right: &BTreeMap<StableId, String>,
    changes: &mut Vec<SemanticChange>,
) {
    for field_id in left.keys() {
        if !right.contains_key(field_id) {
            changes.push(SemanticChange::new(
                SemanticChangeKind::Removed,
                format!("notes.{note_id}.fields.{field_id}"),
                left.get(field_id).cloned(),
                None,
            ));
        }
    }

    for (field_id, right_value) in right {
        let Some(left_value) = left.get(field_id) else {
            changes.push(SemanticChange::new(
                SemanticChangeKind::Added,
                format!("notes.{note_id}.fields.{field_id}"),
                None,
                Some(right_value.clone()),
            ));
            continue;
        };

        push_modified_if_changed(
            changes,
            format!("notes.{note_id}.fields.{field_id}"),
            left_value,
            right_value,
        );
    }
}

fn diff_media(
    left: &BTreeMap<StableId, MediaReference>,
    right: &BTreeMap<StableId, MediaReference>,
    changes: &mut Vec<SemanticChange>,
) {
    for id in left.keys() {
        if !right.contains_key(id) {
            changes.push(SemanticChange::removed(format!("media.{id}")));
        }
    }

    for (id, right_media) in right {
        let Some(left_media) = left.get(id) else {
            changes.push(SemanticChange::added(format!("media.{id}")));
            continue;
        };

        push_modified_if_changed(
            changes,
            format!("media.{id}.path"),
            &left_media.path,
            &right_media.path,
        );
        push_modified_if_changed(
            changes,
            format!("media.{id}.sha256"),
            &left_media.sha256,
            &right_media.sha256,
        );
    }
}

fn diff_tombstones(
    left: &BTreeSet<StableId>,
    right: &BTreeSet<StableId>,
    changes: &mut Vec<SemanticChange>,
) {
    for id in left {
        if !right.contains(id) {
            changes.push(SemanticChange::removed(format!("tombstones.{id}")));
        }
    }

    for id in right {
        if !left.contains(id) {
            changes.push(SemanticChange::new(
                SemanticChangeKind::Tombstoned,
                format!("tombstones.{id}"),
                None,
                Some(id.to_string()),
            ));
        }
    }
}

fn push_modified_if_changed(
    changes: &mut Vec<SemanticChange>,
    path: String,
    left: &str,
    right: &str,
) {
    if left != right {
        changes.push(SemanticChange::new(
            SemanticChangeKind::Modified,
            path,
            Some(left.to_owned()),
            Some(right.to_owned()),
        ));
    }
}

fn field_summary(fields: &[FieldDefinition]) -> String {
    fields
        .iter()
        .map(|field| format!("{}={}", field.id, field.name))
        .collect::<Vec<_>>()
        .join("|")
}

fn template_summary(templates: &[CardTemplate]) -> String {
    templates
        .iter()
        .map(|template| {
            format!(
                "{}={}:{}:{}:{}:{}",
                template.id,
                template.name,
                string_map_summary(&template.variables),
                template.question_format,
                template.answer_format,
                adapter_ids_summary(&template.adapter_ids)
            )
        })
        .collect::<Vec<_>>()
        .join("|")
}

fn set_summary(values: &BTreeSet<String>) -> String {
    values.iter().cloned().collect::<Vec<_>>().join("|")
}

fn string_map_summary(values: &BTreeMap<String, String>) -> String {
    values
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("|")
}

fn adapter_ids_summary(adapter_ids: &AdapterIds) -> String {
    adapter_ids
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("|")
}

fn push_duplicate_id_errors<'a>(
    ids: impl Iterator<Item = &'a StableId>,
    kind: ValidationErrorKind,
    path: impl Fn(&StableId) -> String,
    errors: &mut Vec<ValidationError>,
) {
    let mut seen = BTreeSet::new();
    for id in ids {
        if !seen.insert(id) {
            errors.push(ValidationError::new(
                kind,
                path(id),
                format!("duplicate stable id {id}"),
            ));
        }
    }
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
    /// Stable deck paths to fill only when the current source value is intentionally blank.
    pub target_additions: BTreeMap<String, String>,
    /// Persisted source-change review records that apply prior target text while warning.
    pub stale_records: Vec<StaleTranslationRecord>,
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
    /// Resolve one stale record into a normal direct or contextual translation entry.
    pub fn resolve_stale_record(
        &mut self,
        old_source: &str,
        new_source: &str,
        context: Option<&str>,
    ) -> Option<StaleTranslationRecord> {
        let position = self.stale_records.iter().position(|record| {
            record.old_source == old_source
                && record.new_source == new_source
                && record.context.as_deref() == context
        })?;
        let record = self.stale_records.remove(position);
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

/// Persisted translation review debt created when source text changes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaleTranslationRecord {
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
                TranslationCoverageCategory::StaleTranslationRecord
                    | TranslationCoverageCategory::StaleDirectKey
                    | TranslationCoverageCategory::StaleContextualKey
                    | TranslationCoverageCategory::StaleNoChangeKey
                    | TranslationCoverageCategory::StaleTargetAddition
                    | TranslationCoverageCategory::StaleVariableKey
                    | TranslationCoverageCategory::StaleAdapterIdKey
                    | TranslationCoverageCategory::InvalidTargetAddition
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
    ContextualOverride,
    NoChange,
    TargetLanguageAddition,
    VariableTranslation,
    AdapterIdTranslation,
    IgnoredSource,
    StaleTranslationRecord,
    UntranslatedFallback,
    StaleDirectKey,
    StaleContextualKey,
    StaleNoChangeKey,
    StaleTargetAddition,
    StaleVariableKey,
    StaleAdapterIdKey,
    InvalidTargetAddition,
}

impl TranslationCoverageCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DirectTranslation => "direct_translation",
            Self::ContextualOverride => "contextual_override",
            Self::NoChange => "no_change",
            Self::TargetLanguageAddition => "target_language_addition",
            Self::VariableTranslation => "variable_translation",
            Self::AdapterIdTranslation => "adapter_id_translation",
            Self::IgnoredSource => "ignored_source",
            Self::StaleTranslationRecord => "stale_translation_record",
            Self::UntranslatedFallback => "untranslated_fallback",
            Self::StaleDirectKey => "stale_direct_key",
            Self::StaleContextualKey => "stale_contextual_key",
            Self::StaleNoChangeKey => "stale_no_change_key",
            Self::StaleTargetAddition => "stale_target_addition",
            Self::StaleVariableKey => "stale_variable_key",
            Self::StaleAdapterIdKey => "stale_adapter_id_key",
            Self::InvalidTargetAddition => "invalid_target_addition",
        }
    }

    pub fn is_problem(self) -> bool {
        matches!(
            self,
            Self::UntranslatedFallback
                | Self::StaleTranslationRecord
                | Self::StaleDirectKey
                | Self::StaleContextualKey
                | Self::StaleNoChangeKey
                | Self::StaleTargetAddition
                | Self::StaleVariableKey
                | Self::StaleAdapterIdKey
                | Self::InvalidTargetAddition
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
}

impl fmt::Display for VariableRenderReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, error) in self.errors.iter().enumerate() {
            if index > 0 {
                writeln!(f)?;
            }
            write!(f, "{}: missing variable ${}", error.path, error.variable)?;
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
    fn new(kind: ComposeErrorKind, path: String, message: String) -> Self {
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
    fn new(
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

    fn added(path: String) -> Self {
        Self::new(SemanticChangeKind::Added, path, None, None)
    }

    fn removed(path: String) -> Self {
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

#[cfg(test)]
mod tests {
    use super::CRATE_NAME;

    #[test]
    fn exposes_core_crate_name() {
        assert_eq!(CRATE_NAME, "brain-brew-core");
    }
}
