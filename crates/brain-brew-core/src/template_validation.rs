use std::collections::BTreeSet;

use crate::{CardTemplate, DeckPath, StableId, ValidationError, ValidationErrorKind};

/// Validate the Anki/Mustache field tokens used by one final card template.
///
/// This deliberately recognizes only Anki field references: direct and triple-brace
/// references, conditional/inverted sections, and the `type:`, `cloze:`, `hint:`,
/// and `text:` filters. Other Mustache/Handlebars helpers and tokens are ignored so
/// helper syntax embedded in HTML or JavaScript is not mistaken for an Anki field.
pub(crate) fn validate_template_field_references(
    note_type_id: &StableId,
    template: &CardTemplate,
    field_names: &BTreeSet<String>,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    validate_template_side(
        note_type_id,
        template,
        "question",
        &template.question_format,
        field_names,
        &mut errors,
    );
    validate_template_side(
        note_type_id,
        template,
        "answer",
        &template.answer_format,
        field_names,
        &mut errors,
    );
    errors
}

fn validate_template_side(
    note_type_id: &StableId,
    template: &CardTemplate,
    side: &str,
    source: &str,
    field_names: &BTreeSet<String>,
    errors: &mut Vec<ValidationError>,
) {
    let path = match side {
        "question" => DeckPath::NoteTypeCardTemplateQuestionFormat {
            note_type_id: note_type_id.clone(),
            template_id: template.id.clone(),
        },
        "answer" => DeckPath::NoteTypeCardTemplateAnswerFormat {
            note_type_id: note_type_id.clone(),
            template_id: template.id.clone(),
        },
        _ => unreachable!("only question and answer template sides are validated"),
    }
    .to_string();
    let mut index = 0;
    let mut sections = Vec::<String>::new();
    while let Some(relative) = source[index..].find("{{") {
        let start = index + relative;
        let triple = source[start..].starts_with("{{{");
        let opener_len = if triple { 3 } else { 2 };
        let closer = if triple { "}}}" } else { "}}" };
        let body_start = start + opener_len;
        let Some(relative_end) = source[body_start..].find(closer) else {
            malformed(
                errors,
                &path,
                note_type_id,
                template,
                "unterminated Anki/Mustache field token",
            );
            break;
        };
        let end = body_start + relative_end;
        let token = source[body_start..end].trim();
        if triple && token.contains("{{") {
            malformed(
                errors,
                &path,
                note_type_id,
                template,
                "malformed triple-brace Anki field token",
            );
        } else {
            validate_token(
                token,
                &path,
                note_type_id,
                template,
                field_names,
                &mut sections,
                errors,
            );
        }
        index = end + closer.len();
    }
    for field in sections {
        malformed(
            errors,
            &path,
            note_type_id,
            template,
            &format!("unclosed Anki field section {field:?}"),
        );
    }
}

fn validate_token(
    token: &str,
    path: &str,
    note_type_id: &StableId,
    template: &CardTemplate,
    field_names: &BTreeSet<String>,
    sections: &mut Vec<String>,
    errors: &mut Vec<ValidationError>,
) {
    if token.is_empty() {
        return malformed(
            errors,
            path,
            note_type_id,
            template,
            "empty Anki field token",
        );
    }
    if matches!(token.as_bytes().first(), Some(b'!' | b'>' | b'=')) {
        return;
    }
    let (marker, body) = match token.as_bytes().first() {
        Some(b'#' | b'^' | b'/') => (&token[..1], token[1..].trim()),
        Some(b'&') => ("&", token[1..].trim()),
        _ => ("", token),
    };
    let reference = match field_reference(body, field_names) {
        TokenReference::Field(field) => field,
        TokenReference::Helper => return,
        TokenReference::Malformed(message) => {
            return malformed(errors, path, note_type_id, template, &message);
        }
    };
    if !field_names.contains(reference) {
        return unknown(errors, path, note_type_id, template, reference);
    }
    match marker {
        "#" | "^" => sections.push(reference.to_owned()),
        "/" => match sections.pop() {
            Some(open) if open == reference => {}
            Some(open) => malformed(
                errors,
                path,
                note_type_id,
                template,
                &format!(
                    "closing Anki field section {reference:?} does not match open section {open:?}"
                ),
            ),
            None => malformed(
                errors,
                path,
                note_type_id,
                template,
                &format!("closing Anki field section {reference:?} has no open section"),
            ),
        },
        _ => {}
    }
}

enum TokenReference<'a> {
    Field(&'a str),
    Helper,
    Malformed(String),
}

fn field_reference<'a>(body: &'a str, field_names: &BTreeSet<String>) -> TokenReference<'a> {
    if body == "." || is_builtin(body) || is_helper_name(body) {
        return TokenReference::Helper;
    }
    // Field names may contain spaces. Prefer an exact schema match before treating
    // whitespace-bearing expressions as non-field helpers.
    if field_names.contains(body) {
        return TokenReference::Field(body);
    }
    if let Some((filter, field)) = body.split_once(':') {
        if matches!(filter.trim(), "type" | "cloze" | "hint" | "text") {
            let field = field.trim();
            return if field.is_empty() {
                TokenReference::Malformed(format!("{filter}: requires an Anki field name"))
            } else {
                TokenReference::Field(field)
            };
        }
        return TokenReference::Helper;
    }
    if body.chars().any(char::is_whitespace) {
        return TokenReference::Helper;
    }
    TokenReference::Field(body)
}

fn is_builtin(token: &str) -> bool {
    matches!(
        token,
        "FrontSide" | "Tags" | "Card" | "Deck" | "Subdeck" | "Type" | "Flag" | "CardFlag"
    )
}

fn is_helper_name(token: &str) -> bool {
    matches!(token, "if" | "unless" | "each" | "with" | "else")
}

fn unknown(
    errors: &mut Vec<ValidationError>,
    path: &str,
    note_type_id: &StableId,
    template: &CardTemplate,
    reference: &str,
) {
    errors.push(ValidationError::new(
        ValidationErrorKind::UnknownTemplateField,
        path.to_owned(),
        format!(
            "unknown Anki field reference {reference:?} in template {} of note type {note_type_id}",
            template.id
        ),
    ));
}

fn malformed(
    errors: &mut Vec<ValidationError>,
    path: &str,
    note_type_id: &StableId,
    template: &CardTemplate,
    message: &str,
) {
    errors.push(ValidationError::new(
        ValidationErrorKind::MalformedTemplateReference,
        path.to_owned(),
        format!(
            "{message} in template {} of note type {note_type_id}",
            template.id
        ),
    ));
}
