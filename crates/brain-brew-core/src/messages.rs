use std::collections::{BTreeMap, BTreeSet};

use crate::*;

impl CanonicalDeck {
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

    /// Resolve one scalar/message field as semantic text without lowering images.
    pub fn field_text(&self, note_id: &StableId, field_id: &StableId) -> Option<String> {
        let path = DeckPath::NoteField {
            note_id: note_id.clone(),
            field_id: field_id.clone(),
        }
        .to_string();
        rendered_field_text_at_path(self, &path)
    }
}

pub(crate) fn validate_message_references(
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
            DeckPath::NoteFieldMessage {
                note_id: note_id.clone(),
                field_id: field_id.clone(),
            }
            .to_string(),
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

pub(crate) fn resolve_structured_messages_with_validation_errors(
    deck: &mut CanonicalDeck,
    errors: &mut Vec<ValidationError>,
) {
    resolve_structured_messages(deck, errors, |path, message| {
        ValidationError::new(ValidationErrorKind::InvalidMessageReference, path, message)
    });
}

fn resolve_structured_messages<E>(
    deck: &mut CanonicalDeck,
    errors: &mut Vec<E>,
    make_error: impl Fn(String, String) -> E,
) {
    let snapshot = deck.clone();
    let message_fields = snapshot
        .notes
        .iter()
        .flat_map(|(note_id, note)| {
            note.fields.iter().filter_map(move |(field_id, value)| {
                matches!(value, FieldValue::Message(_))
                    .then_some((note_id.clone(), field_id.clone()))
            })
        })
        .collect::<Vec<_>>();
    let mut memo = BTreeMap::<String, String>::new();
    let mut resolved_fields = Vec::<(StableId, StableId, String)>::new();
    for (note_id, field_id) in message_fields {
        let path = DeckPath::NoteField {
            note_id: note_id.clone(),
            field_id: field_id.clone(),
        }
        .to_string();
        match render_field_at_path(&snapshot, &path, &mut BTreeSet::new(), &mut memo) {
            Ok(value) => resolved_fields.push((note_id, field_id, value)),
            Err(error) => errors.push(make_error(path, error.message())),
        }
    }
    for (note_id, field_id, value) in resolved_fields {
        if let Some(note) = deck.notes.get_mut(&note_id) {
            note.fields.insert(field_id, FieldValue::Scalar(value));
        }
    }
}

fn render_field_at_path(
    deck: &CanonicalDeck,
    path: &str,
    visiting: &mut BTreeSet<String>,
    memo: &mut BTreeMap<String, String>,
) -> Result<String, StructuredMessageRenderError> {
    if let Some(value) = memo.get(path) {
        return Ok(value.clone());
    }
    if !visiting.insert(path.to_owned()) {
        return Err(StructuredMessageRenderError::Cycle(path.to_owned()));
    }
    let value = match field_value_at_path(deck, path) {
        Some(FieldValue::Scalar(value)) => Ok(value.clone()),
        Some(FieldValue::Message(message)) => {
            render_structured_message(deck, message, visiting, memo)
        }
        Some(FieldValue::Images(_)) => Err(StructuredMessageRenderError::NonTextReference(
            path.to_owned(),
        )),
        None => Err(StructuredMessageRenderError::InvalidReference(
            path.to_owned(),
        )),
    };
    visiting.remove(path);
    if let Ok(value) = &value {
        memo.insert(path.to_owned(), value.clone());
    }
    value
}

fn render_structured_message(
    deck: &CanonicalDeck,
    message: &StructuredMessage,
    visiting: &mut BTreeSet<String>,
    memo: &mut BTreeMap<String, String>,
) -> Result<String, StructuredMessageRenderError> {
    if let Some(format) = &message.format {
        let mut variables = BTreeMap::new();
        for (name, component) in &message.variables {
            variables.insert(
                name.clone(),
                render_message_component(deck, component, visiting, memo)?,
            );
        }
        return render_message_format(format, &variables)
            .map_err(StructuredMessageRenderError::Format);
    }

    let mut rendered = String::new();
    for component in &message.components {
        rendered.push_str(&render_message_component(deck, component, visiting, memo)?);
    }
    Ok(rendered)
}

fn render_message_component(
    deck: &CanonicalDeck,
    component: &MessageComponent,
    visiting: &mut BTreeSet<String>,
    memo: &mut BTreeMap<String, String>,
) -> Result<String, StructuredMessageRenderError> {
    match component {
        MessageComponent::Literal(value) | MessageComponent::Text(value) => Ok(value.clone()),
        MessageComponent::FieldRef(reference) => {
            render_field_at_path(deck, reference, visiting, memo)
        }
    }
}

#[derive(Debug)]
enum StructuredMessageRenderError {
    InvalidReference(String),
    NonTextReference(String),
    Cycle(String),
    Format(String),
}

impl StructuredMessageRenderError {
    fn message(&self) -> String {
        match self {
            Self::InvalidReference(reference) => format!(
                "structured message field reference {reference:?} does not resolve to a note field"
            ),
            Self::NonTextReference(reference) => format!(
                "structured message field reference {reference:?} resolves to structured images, not text"
            ),
            Self::Cycle(reference) => {
                format!("structured message field reference cycle includes {reference:?}")
            }
            Self::Format(message) => message.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum MessageFormatPart {
    Literal(String),
    Variable(String),
}

pub(crate) fn render_message_format(
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

pub(crate) fn field_value_at_path<'a>(
    deck: &'a CanonicalDeck,
    path: &str,
) -> Option<&'a FieldValue> {
    let DeckPath::NoteField { note_id, field_id } = path.parse().ok()? else {
        return None;
    };
    deck.notes
        .get(&note_id)
        .and_then(|note| note.fields.get(&field_id))
}

pub(crate) fn rendered_field_text_at_path(deck: &CanonicalDeck, path: &str) -> Option<String> {
    render_field_at_path(deck, path, &mut BTreeSet::new(), &mut BTreeMap::new()).ok()
}

pub(crate) fn message_component_path(
    note_id: &StableId,
    field_id: &StableId,
    index: usize,
) -> String {
    DeckPath::NoteFieldMessageComponent {
        note_id: note_id.clone(),
        field_id: field_id.clone(),
        index,
    }
    .to_string()
}

pub(crate) fn message_format_path(note_id: &StableId, field_id: &StableId) -> String {
    DeckPath::NoteFieldMessageFormat {
        note_id: note_id.clone(),
        field_id: field_id.clone(),
    }
    .to_string()
}

pub(crate) fn message_variable_path(
    note_id: &StableId,
    field_id: &StableId,
    variable: &str,
) -> String {
    DeckPath::NoteFieldMessageVariable {
        note_id: note_id.clone(),
        field_id: field_id.clone(),
        variable: variable.to_owned(),
    }
    .to_string()
}
