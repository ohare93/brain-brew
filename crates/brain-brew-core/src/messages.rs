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

pub(crate) fn resolve_structured_messages_with_validation_errors(
    deck: &mut CanonicalDeck,
    errors: &mut Vec<ValidationError>,
) {
    resolve_structured_messages(deck, errors, |path, message| {
        ValidationError::new(ValidationErrorKind::InvalidMessageReference, path, message)
    });
}

pub(crate) fn resolve_structured_messages_with_compose_errors(
    deck: &mut CanonicalDeck,
    errors: &mut Vec<ComposeError>,
) {
    resolve_structured_messages(deck, errors, |path, message| {
        ComposeError::new(ComposeErrorKind::ValidationFailed, path, message)
    });
}

fn resolve_structured_messages<E>(
    deck: &mut CanonicalDeck,
    errors: &mut Vec<E>,
    make_error: impl Fn(String, String) -> E,
) {
    let snapshot = deck.clone();
    let mut resolved_fields = Vec::<(StableId, StableId, String)>::new();
    for (note_id, note) in &snapshot.notes {
        for (field_id, message) in &note.field_messages {
            match render_structured_message(&snapshot, message) {
                Ok(value) => resolved_fields.push((note_id.clone(), field_id.clone(), value)),
                Err(error) => errors.push(make_error(
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

pub(crate) fn field_value_at_path<'a>(deck: &'a CanonicalDeck, path: &str) -> Option<&'a str> {
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

pub(crate) fn message_component_path(
    note_id: &StableId,
    field_id: &StableId,
    index: usize,
) -> String {
    format!("notes.{note_id}.fields.{field_id}.message.{index}")
}

pub(crate) fn message_format_path(note_id: &StableId, field_id: &StableId) -> String {
    format!("notes.{note_id}.fields.{field_id}.message.format")
}

pub(crate) fn message_variable_path(
    note_id: &StableId,
    field_id: &StableId,
    variable: &str,
) -> String {
    format!("notes.{note_id}.fields.{field_id}.message.variables.{variable}")
}
