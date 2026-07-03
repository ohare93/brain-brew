use std::collections::BTreeSet;

use crate::messages::validate_message_references;
use crate::*;

impl CanonicalDeck {
    /// Validate strict core invariants that do not require filesystem or format access.
    pub fn validate(&self) -> Result<(), ValidationReport> {
        let mut errors = Vec::new();

        for (id, note_type) in &self.note_types {
            if &note_type.id != id {
                errors.push(ValidationError::new(
                    ValidationErrorKind::MismatchedEntityId,
                    DeckPath::NoteTypeId {
                        note_type_id: id.clone(),
                    }
                    .to_string(),
                    format!("note type stored under {id} contains id {}", note_type.id),
                ));
            }

            push_duplicate_id_errors(
                note_type.fields.iter().map(|field| &field.id),
                ValidationErrorKind::DuplicateFieldDefinition,
                |duplicate_id| {
                    DeckPath::NoteTypeField {
                        note_type_id: id.clone(),
                        field_id: duplicate_id.clone(),
                    }
                    .to_string()
                },
                &mut errors,
            );

            push_duplicate_id_errors(
                note_type.card_templates.iter().map(|template| &template.id),
                ValidationErrorKind::DuplicateCardTemplate,
                |duplicate_id| {
                    DeckPath::NoteTypeCardTemplate {
                        note_type_id: id.clone(),
                        template_id: duplicate_id.clone(),
                    }
                    .to_string()
                },
                &mut errors,
            );
        }

        for (id, media) in &self.media {
            if &media.id != id {
                errors.push(ValidationError::new(
                    ValidationErrorKind::MismatchedEntityId,
                    DeckPath::MediaId {
                        media_id: id.clone(),
                    }
                    .to_string(),
                    format!("media stored under {id} contains id {}", media.id),
                ));
            }
        }

        for (id, note) in &self.notes {
            if &note.id != id {
                errors.push(ValidationError::new(
                    ValidationErrorKind::MismatchedEntityId,
                    DeckPath::NoteId {
                        note_id: id.clone(),
                    }
                    .to_string(),
                    format!("note stored under {id} contains id {}", note.id),
                ));
            }

            let Some(note_type) = self.note_types.get(&note.note_type_id) else {
                errors.push(ValidationError::new(
                    ValidationErrorKind::MissingNoteType,
                    DeckPath::NoteNoteTypeId {
                        note_id: id.clone(),
                    }
                    .to_string(),
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
                        DeckPath::NoteField {
                            note_id: id.clone(),
                            field_id: field_id.clone(),
                        }
                        .to_string(),
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
                        DeckPath::NoteFieldMessage {
                            note_id: id.clone(),
                            field_id: field_id.clone(),
                        }
                        .to_string(),
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
                        DeckPath::NoteField {
                            note_id: id.clone(),
                            field_id: field_id.clone(),
                        }
                        .to_string(),
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
