use std::collections::BTreeSet;

use crate::messages::validate_message_references;
use crate::*;

impl CanonicalDeck {
    /// Validate strict core invariants that do not require filesystem or format access.
    pub fn validate(&self) -> Result<(), ValidationReport> {
        let mut errors = Vec::new();
        let mut invalid_stable_id_paths = BTreeSet::new();

        for (id, note_type) in &self.note_types {
            push_invalid_stable_id_error(
                id,
                DeckPath::NoteType {
                    note_type_id: id.clone(),
                }
                .to_string(),
                &mut errors,
                &mut invalid_stable_id_paths,
            );
            push_invalid_stable_id_error(
                &note_type.id,
                DeckPath::NoteTypeId {
                    note_type_id: id.clone(),
                }
                .to_string(),
                &mut errors,
                &mut invalid_stable_id_paths,
            );
            for field in &note_type.fields {
                push_invalid_stable_id_error(
                    &field.id,
                    DeckPath::NoteTypeField {
                        note_type_id: id.clone(),
                        field_id: field.id.clone(),
                    }
                    .to_string(),
                    &mut errors,
                    &mut invalid_stable_id_paths,
                );
            }
            for template in &note_type.card_templates {
                push_invalid_stable_id_error(
                    &template.id,
                    DeckPath::NoteTypeCardTemplate {
                        note_type_id: id.clone(),
                        template_id: template.id.clone(),
                    }
                    .to_string(),
                    &mut errors,
                    &mut invalid_stable_id_paths,
                );
            }

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
            push_invalid_stable_id_error(
                id,
                DeckPath::Media {
                    media_id: id.clone(),
                }
                .to_string(),
                &mut errors,
                &mut invalid_stable_id_paths,
            );
            push_invalid_stable_id_error(
                &media.id,
                DeckPath::MediaId {
                    media_id: id.clone(),
                }
                .to_string(),
                &mut errors,
                &mut invalid_stable_id_paths,
            );

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
            push_invalid_stable_id_error(
                id,
                DeckPath::Note {
                    note_id: id.clone(),
                }
                .to_string(),
                &mut errors,
                &mut invalid_stable_id_paths,
            );
            push_invalid_stable_id_error(
                &note.id,
                DeckPath::NoteId {
                    note_id: id.clone(),
                }
                .to_string(),
                &mut errors,
                &mut invalid_stable_id_paths,
            );
            push_invalid_stable_id_error(
                &note.note_type_id,
                DeckPath::NoteNoteTypeId {
                    note_id: id.clone(),
                }
                .to_string(),
                &mut errors,
                &mut invalid_stable_id_paths,
            );
            for field_id in note.fields.keys() {
                push_invalid_stable_id_error(
                    field_id,
                    DeckPath::NoteField {
                        note_id: id.clone(),
                        field_id: field_id.clone(),
                    }
                    .to_string(),
                    &mut errors,
                    &mut invalid_stable_id_paths,
                );
            }
            for field_id in note.field_messages.keys() {
                push_invalid_stable_id_error(
                    field_id,
                    DeckPath::NoteFieldMessage {
                        note_id: id.clone(),
                        field_id: field_id.clone(),
                    }
                    .to_string(),
                    &mut errors,
                    &mut invalid_stable_id_paths,
                );
            }
            for (field_id, images) in &note.field_images {
                push_invalid_stable_id_error(
                    field_id,
                    DeckPath::NoteField {
                        note_id: id.clone(),
                        field_id: field_id.clone(),
                    }
                    .to_string(),
                    &mut errors,
                    &mut invalid_stable_id_paths,
                );
                for image in images {
                    push_invalid_stable_id_error(
                        &image.media_id,
                        DeckPath::NoteField {
                            note_id: id.clone(),
                            field_id: field_id.clone(),
                        }
                        .to_string(),
                        &mut errors,
                        &mut invalid_stable_id_paths,
                    );
                }
            }

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

            for (field_id, images) in &note.field_images {
                if images.is_empty() {
                    errors.push(ValidationError::new(
                        ValidationErrorKind::ConflictingFieldRepresentation,
                        DeckPath::NoteField {
                            note_id: id.clone(),
                            field_id: field_id.clone(),
                        }
                        .to_string(),
                        format!("structured image field {field_id} must not be empty"),
                    ));
                }
                if !expected_field_ids.contains(field_id) {
                    errors.push(ValidationError::new(
                        ValidationErrorKind::UnknownNoteField,
                        DeckPath::NoteField {
                            note_id: id.clone(),
                            field_id: field_id.clone(),
                        }
                        .to_string(),
                        format!(
                            "structured image field {field_id} is not defined by note type {}",
                            note.note_type_id
                        ),
                    ));
                }
            }

            for field_id in note
                .fields
                .keys()
                .chain(note.field_messages.keys())
                .chain(note.field_images.keys())
                .cloned()
                .collect::<BTreeSet<_>>()
            {
                let has_raw_value = note
                    .fields
                    .get(&field_id)
                    .is_some_and(|value| !value.is_empty());
                let has_message = note.field_messages.contains_key(&field_id);
                let has_images = note.field_images.contains_key(&field_id);
                if has_images && (has_raw_value || has_message) {
                    let mut representations = Vec::new();
                    if has_raw_value {
                        representations.push("raw value");
                    }
                    if has_message {
                        representations.push("structured message");
                    }
                    if has_images {
                        representations.push("structured images");
                    }
                    errors.push(ValidationError::new(
                        ValidationErrorKind::ConflictingFieldRepresentation,
                        DeckPath::NoteField {
                            note_id: id.clone(),
                            field_id: field_id.clone(),
                        }
                        .to_string(),
                        format!(
                            "conflicting field representations for {field_id}: {}",
                            representations.join(", ")
                        ),
                    ));
                }
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

        for id in &self.tombstones {
            push_invalid_stable_id_error(
                id,
                DeckPath::Tombstone { id: id.clone() }.to_string(),
                &mut errors,
                &mut invalid_stable_id_paths,
            );
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ValidationReport { errors })
        }
    }
}

fn push_invalid_stable_id_error(
    id: &StableId,
    path: String,
    errors: &mut Vec<ValidationError>,
    seen: &mut BTreeSet<(String, String)>,
) {
    let Some(reason) = invalid_stable_id_deck_path_reason(id.as_str()) else {
        return;
    };
    let id_text = id.as_str().to_owned();
    if !seen.insert((path.clone(), id_text.clone())) {
        return;
    }

    errors.push(ValidationError::new(
        ValidationErrorKind::InvalidStableId,
        path,
        format!("stable id {id_text} cannot be used in a DeckPath segment: {reason}"),
    ));
}

fn invalid_stable_id_deck_path_reason(id: &str) -> Option<String> {
    if id.contains("..") {
        return Some("contains reserved empty dotted segment '..'".to_owned());
    }

    for marker in RESERVED_CONTAINER_MARKERS {
        if id.contains(marker) {
            return Some(format!("contains reserved DeckPath marker {marker}"));
        }
    }

    for suffix in RESERVED_PROPERTY_SUFFIXES {
        if id.ends_with(suffix) {
            return Some(format!(
                "ends with reserved DeckPath property suffix {suffix}"
            ));
        }
    }

    None
}

const RESERVED_CONTAINER_MARKERS: &[&str] = &[
    ".fields.",
    ".card_templates.",
    ".variables.",
    ".adapter_ids.",
    ".tags.",
    ".message.",
];

const RESERVED_PROPERTY_SUFFIXES: &[&str] = &[
    ".id",
    ".name",
    ".styling",
    ".fields",
    ".card_templates",
    ".variables",
    ".adapter_ids",
    ".tags",
    ".note_type_id",
    ".message",
    ".path",
    ".sha256",
    ".question_format",
    ".answer_format",
];

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
