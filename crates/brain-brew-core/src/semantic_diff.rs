//! Exact, deterministic semantic comparison for canonical decks.
//!
//! Equality is typed: no presentation serialization participates in deciding whether
//! values differ. Maps and sets use their key/value semantics; field definitions,
//! card templates, image references, and message components preserve sequence order.

use std::collections::{BTreeMap, BTreeSet};

use crate::*;

pub(crate) fn diff(left: &CanonicalDeck, right: &CanonicalDeck) -> SemanticDiff {
    let CanonicalDeck {
        id: left_id,
        name: left_name,
        description: left_description,
        variables: left_variables,
        note_types: left_note_types,
        notes: left_notes,
        media: left_media,
        tombstones: left_tombstones,
        adapter_ids: left_adapter_ids,
    } = left;
    let CanonicalDeck {
        id: right_id,
        name: right_name,
        description: right_description,
        variables: right_variables,
        note_types: right_note_types,
        notes: right_notes,
        media: right_media,
        tombstones: right_tombstones,
        adapter_ids: right_adapter_ids,
    } = right;

    let mut changes = Vec::new();
    modified(
        &mut changes,
        DeckPath::DeckId.to_string(),
        left_id,
        right_id,
        ToString::to_string,
    );
    modified_string(
        &mut changes,
        DeckPath::DeckName.to_string(),
        left_name,
        right_name,
    );
    modified_string(
        &mut changes,
        DeckPath::DeckDescription.to_string(),
        left_description,
        right_description,
    );
    diff_string_map(&mut changes, left_variables, right_variables, |key| {
        DeckPath::DeckVariable { key }.to_string()
    });
    diff_adapter_ids(&mut changes, left_adapter_ids, right_adapter_ids, |key| {
        DeckPath::DeckAdapterId { key }.to_string()
    });
    diff_note_types(&mut changes, left_note_types, right_note_types);
    diff_notes(&mut changes, left_notes, right_notes);
    diff_media(&mut changes, left_media, right_media);
    diff_tombstones(&mut changes, left_tombstones, right_tombstones);

    changes.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| kind_rank(left.kind).cmp(&kind_rank(right.kind)))
            .then_with(|| left.before.cmp(&right.before))
            .then_with(|| left.after.cmp(&right.after))
    });
    SemanticDiff { changes }
}

fn diff_note_types(
    changes: &mut Vec<SemanticChange>,
    left: &BTreeMap<StableId, NoteType>,
    right: &BTreeMap<StableId, NoteType>,
) {
    diff_entity_map(
        changes,
        left,
        right,
        |id| DeckPath::NoteType { note_type_id: id }.to_string(),
        note_type_summary,
        diff_note_type,
    );
}

fn diff_note_type(
    changes: &mut Vec<SemanticChange>,
    key: &StableId,
    left: &NoteType,
    right: &NoteType,
) {
    let NoteType {
        id: left_id,
        name: left_name,
        variables: left_variables,
        fields: left_fields,
        card_templates: left_templates,
        styling: left_styling,
        adapter_ids: left_adapter_ids,
    } = left;
    let NoteType {
        id: right_id,
        name: right_name,
        variables: right_variables,
        fields: right_fields,
        card_templates: right_templates,
        styling: right_styling,
        adapter_ids: right_adapter_ids,
    } = right;

    modified(
        changes,
        DeckPath::NoteTypeId {
            note_type_id: key.clone(),
        }
        .to_string(),
        left_id,
        right_id,
        ToString::to_string,
    );
    modified_string(
        changes,
        DeckPath::NoteTypeName {
            note_type_id: key.clone(),
        }
        .to_string(),
        left_name,
        right_name,
    );
    diff_string_map(changes, left_variables, right_variables, |map_key| {
        DeckPath::NoteTypeVariable {
            note_type_id: key.clone(),
            key: map_key,
        }
        .to_string()
    });
    diff_field_definitions(changes, key, left_fields, right_fields);
    diff_card_templates(changes, key, left_templates, right_templates);
    modified_string(
        changes,
        DeckPath::NoteTypeStyling {
            note_type_id: key.clone(),
        }
        .to_string(),
        left_styling,
        right_styling,
    );
    diff_adapter_ids(changes, left_adapter_ids, right_adapter_ids, |map_key| {
        DeckPath::NoteTypeAdapterId {
            note_type_id: key.clone(),
            key: map_key,
        }
        .to_string()
    });
}

fn diff_field_definitions(
    changes: &mut Vec<SemanticChange>,
    note_type_id: &StableId,
    left: &[FieldDefinition],
    right: &[FieldDefinition],
) {
    diff_ordered_entities(
        changes,
        DeckPath::NoteTypeFields {
            note_type_id: note_type_id.clone(),
        }
        .to_string(),
        left,
        right,
        |field| &field.id,
        field_definition_summary,
        |id| {
            DeckPath::NoteTypeField {
                note_type_id: note_type_id.clone(),
                field_id: id,
            }
            .to_string()
        },
        |changes, key, left, right| {
            let FieldDefinition {
                id: left_id,
                name: left_name,
            } = left;
            let FieldDefinition {
                id: right_id,
                name: right_name,
            } = right;
            modified(
                changes,
                DeckPath::NoteTypeFieldId {
                    note_type_id: note_type_id.clone(),
                    field_id: key.clone(),
                }
                .to_string(),
                left_id,
                right_id,
                ToString::to_string,
            );
            modified_string(
                changes,
                DeckPath::NoteTypeFieldName {
                    note_type_id: note_type_id.clone(),
                    field_id: key.clone(),
                }
                .to_string(),
                left_name,
                right_name,
            );
        },
    );
}

fn diff_card_templates(
    changes: &mut Vec<SemanticChange>,
    note_type_id: &StableId,
    left: &[CardTemplate],
    right: &[CardTemplate],
) {
    diff_ordered_entities(
        changes,
        DeckPath::NoteTypeCardTemplates {
            note_type_id: note_type_id.clone(),
        }
        .to_string(),
        left,
        right,
        |template| &template.id,
        card_template_summary,
        |id| {
            DeckPath::NoteTypeCardTemplate {
                note_type_id: note_type_id.clone(),
                template_id: id,
            }
            .to_string()
        },
        |changes, key, left, right| diff_card_template(changes, note_type_id, key, left, right),
    );
}

fn diff_card_template(
    changes: &mut Vec<SemanticChange>,
    note_type_id: &StableId,
    key: &StableId,
    left: &CardTemplate,
    right: &CardTemplate,
) {
    let CardTemplate {
        id: left_id,
        name: left_name,
        variables: left_variables,
        question_format: left_question,
        answer_format: left_answer,
        adapter_ids: left_adapter_ids,
    } = left;
    let CardTemplate {
        id: right_id,
        name: right_name,
        variables: right_variables,
        question_format: right_question,
        answer_format: right_answer,
        adapter_ids: right_adapter_ids,
    } = right;

    modified(
        changes,
        DeckPath::NoteTypeCardTemplateId {
            note_type_id: note_type_id.clone(),
            template_id: key.clone(),
        }
        .to_string(),
        left_id,
        right_id,
        ToString::to_string,
    );
    modified_string(
        changes,
        DeckPath::NoteTypeCardTemplateName {
            note_type_id: note_type_id.clone(),
            template_id: key.clone(),
        }
        .to_string(),
        left_name,
        right_name,
    );
    diff_string_map(changes, left_variables, right_variables, |map_key| {
        DeckPath::NoteTypeCardTemplateVariable {
            note_type_id: note_type_id.clone(),
            template_id: key.clone(),
            key: map_key,
        }
        .to_string()
    });
    modified_string(
        changes,
        DeckPath::NoteTypeCardTemplateQuestionFormat {
            note_type_id: note_type_id.clone(),
            template_id: key.clone(),
        }
        .to_string(),
        left_question,
        right_question,
    );
    modified_string(
        changes,
        DeckPath::NoteTypeCardTemplateAnswerFormat {
            note_type_id: note_type_id.clone(),
            template_id: key.clone(),
        }
        .to_string(),
        left_answer,
        right_answer,
    );
    diff_adapter_ids(changes, left_adapter_ids, right_adapter_ids, |map_key| {
        DeckPath::NoteTypeCardTemplateAdapterId {
            note_type_id: note_type_id.clone(),
            template_id: key.clone(),
            key: map_key,
        }
        .to_string()
    });
}

fn diff_notes(
    changes: &mut Vec<SemanticChange>,
    left: &BTreeMap<StableId, Note>,
    right: &BTreeMap<StableId, Note>,
) {
    diff_entity_map(
        changes,
        left,
        right,
        |id| DeckPath::Note { note_id: id }.to_string(),
        note_summary,
        diff_note,
    );
}

fn diff_note(changes: &mut Vec<SemanticChange>, key: &StableId, left: &Note, right: &Note) {
    let Note {
        id: left_id,
        note_type_id: left_note_type_id,
        variables: left_variables,
        fields: left_fields,
        tags: left_tags,
        adapter_ids: left_adapter_ids,
    } = left;
    let Note {
        id: right_id,
        note_type_id: right_note_type_id,
        variables: right_variables,
        fields: right_fields,
        tags: right_tags,
        adapter_ids: right_adapter_ids,
    } = right;

    modified(
        changes,
        DeckPath::NoteId {
            note_id: key.clone(),
        }
        .to_string(),
        left_id,
        right_id,
        ToString::to_string,
    );
    modified(
        changes,
        DeckPath::NoteNoteTypeId {
            note_id: key.clone(),
        }
        .to_string(),
        left_note_type_id,
        right_note_type_id,
        ToString::to_string,
    );
    diff_string_map(changes, left_variables, right_variables, |map_key| {
        DeckPath::NoteVariable {
            note_id: key.clone(),
            key: map_key,
        }
        .to_string()
    });
    diff_field_map(changes, key, left_fields, right_fields);
    diff_string_set(changes, left_tags, right_tags, |tag| {
        DeckPath::NoteTag {
            note_id: key.clone(),
            tag,
        }
        .to_string()
    });
    diff_adapter_ids(changes, left_adapter_ids, right_adapter_ids, |map_key| {
        DeckPath::NoteAdapterId {
            note_id: key.clone(),
            key: map_key,
        }
        .to_string()
    });
}

fn diff_field_map(
    changes: &mut Vec<SemanticChange>,
    note_id: &StableId,
    left: &FieldMap,
    right: &FieldMap,
) {
    diff_entity_map(
        changes,
        left,
        right,
        |field_id| {
            DeckPath::NoteField {
                note_id: note_id.clone(),
                field_id,
            }
            .to_string()
        },
        field_value_summary,
        |changes, field_id, left, right| diff_field_value(changes, note_id, field_id, left, right),
    );
}

fn diff_field_value(
    changes: &mut Vec<SemanticChange>,
    note_id: &StableId,
    field_id: &StableId,
    left: &FieldValue,
    right: &FieldValue,
) {
    match (left, right) {
        (FieldValue::Scalar(left), FieldValue::Scalar(right)) => modified_string(
            changes,
            DeckPath::NoteField {
                note_id: note_id.clone(),
                field_id: field_id.clone(),
            }
            .to_string(),
            left,
            right,
        ),
        (FieldValue::Images(left), FieldValue::Images(right)) => {
            diff_images(changes, note_id, field_id, left, right)
        }
        (FieldValue::Message(left), FieldValue::Message(right)) => {
            diff_message(changes, note_id, field_id, left, right)
        }
        (
            FieldValue::Scalar(_) | FieldValue::Images(_) | FieldValue::Message(_),
            FieldValue::Scalar(_) | FieldValue::Images(_) | FieldValue::Message(_),
        ) => modified_string(
            changes,
            DeckPath::NoteField {
                note_id: note_id.clone(),
                field_id: field_id.clone(),
            }
            .to_string(),
            &field_value_summary(left),
            &field_value_summary(right),
        ),
    }
}

fn diff_images(
    changes: &mut Vec<SemanticChange>,
    note_id: &StableId,
    field_id: &StableId,
    left: &[FieldImageReference],
    right: &[FieldImageReference],
) {
    let common = left.len().min(right.len());
    for index in 0..common {
        let FieldImageReference {
            media_id: left_media_id,
        } = &left[index];
        let FieldImageReference {
            media_id: right_media_id,
        } = &right[index];
        modified(
            changes,
            DeckPath::NoteFieldImage {
                note_id: note_id.clone(),
                field_id: field_id.clone(),
                index,
            }
            .to_string(),
            left_media_id,
            right_media_id,
            ToString::to_string,
        );
    }
    for (index, image) in left.iter().enumerate().skip(common) {
        removed(
            changes,
            DeckPath::NoteFieldImage {
                note_id: note_id.clone(),
                field_id: field_id.clone(),
                index,
            }
            .to_string(),
            image.media_id.to_string(),
        );
    }
    for (index, image) in right.iter().enumerate().skip(common) {
        added(
            changes,
            DeckPath::NoteFieldImage {
                note_id: note_id.clone(),
                field_id: field_id.clone(),
                index,
            }
            .to_string(),
            image.media_id.to_string(),
        );
    }
}

fn diff_message(
    changes: &mut Vec<SemanticChange>,
    note_id: &StableId,
    field_id: &StableId,
    left: &StructuredMessage,
    right: &StructuredMessage,
) {
    let StructuredMessage {
        components: left_components,
        format: left_format,
        variables: left_variables,
    } = left;
    let StructuredMessage {
        components: right_components,
        format: right_format,
        variables: right_variables,
    } = right;

    diff_optional_string(
        changes,
        DeckPath::NoteFieldMessageFormat {
            note_id: note_id.clone(),
            field_id: field_id.clone(),
        }
        .to_string(),
        left_format.as_ref(),
        right_format.as_ref(),
    );
    diff_component_sequence(changes, left_components, right_components, |index| {
        DeckPath::NoteFieldMessageComponent {
            note_id: note_id.clone(),
            field_id: field_id.clone(),
            index,
        }
        .to_string()
    });
    diff_component_map(changes, left_variables, right_variables, |variable| {
        DeckPath::NoteFieldMessageVariable {
            note_id: note_id.clone(),
            field_id: field_id.clone(),
            variable,
        }
        .to_string()
    });
}

fn diff_component_sequence(
    changes: &mut Vec<SemanticChange>,
    left: &[MessageComponent],
    right: &[MessageComponent],
    path: impl Fn(usize) -> String,
) {
    let common = left.len().min(right.len());
    for index in 0..common {
        modified(
            changes,
            path(index),
            &left[index],
            &right[index],
            message_component_summary,
        );
    }
    for (index, component) in left.iter().enumerate().skip(common) {
        removed(changes, path(index), message_component_summary(component));
    }
    for (index, component) in right.iter().enumerate().skip(common) {
        added(changes, path(index), message_component_summary(component));
    }
}

fn diff_component_map(
    changes: &mut Vec<SemanticChange>,
    left: &BTreeMap<String, MessageComponent>,
    right: &BTreeMap<String, MessageComponent>,
    path: impl Fn(String) -> String,
) {
    for (key, left_value) in left {
        match right.get(key) {
            Some(right_value) => modified(
                changes,
                path(key.clone()),
                left_value,
                right_value,
                message_component_summary,
            ),
            None => removed(
                changes,
                path(key.clone()),
                message_component_summary(left_value),
            ),
        }
    }
    for (key, right_value) in right {
        if !left.contains_key(key) {
            added(
                changes,
                path(key.clone()),
                message_component_summary(right_value),
            );
        }
    }
}

fn diff_media(
    changes: &mut Vec<SemanticChange>,
    left: &BTreeMap<StableId, MediaReference>,
    right: &BTreeMap<StableId, MediaReference>,
) {
    diff_entity_map(
        changes,
        left,
        right,
        |id| DeckPath::Media { media_id: id }.to_string(),
        media_summary,
        |changes, key, left, right| {
            let MediaReference {
                id: left_id,
                path: left_path,
                sha256: left_sha256,
            } = left;
            let MediaReference {
                id: right_id,
                path: right_path,
                sha256: right_sha256,
            } = right;
            modified(
                changes,
                DeckPath::MediaId {
                    media_id: key.clone(),
                }
                .to_string(),
                left_id,
                right_id,
                ToString::to_string,
            );
            modified_string(
                changes,
                DeckPath::MediaPath {
                    media_id: key.clone(),
                }
                .to_string(),
                left_path,
                right_path,
            );
            modified_string(
                changes,
                DeckPath::MediaSha256 {
                    media_id: key.clone(),
                }
                .to_string(),
                left_sha256,
                right_sha256,
            );
        },
    );
}

fn diff_tombstones(changes: &mut Vec<SemanticChange>, left: &Tombstones, right: &Tombstones) {
    for left_record in left.iter() {
        let TombstoneRecord {
            address,
            provenance: left_provenance,
        } = left_record;
        let path = DeckPath::Tombstone {
            address: address.clone(),
        }
        .to_string();
        match right.get(address) {
            Some(right_record) => {
                let TombstoneRecord {
                    address: right_address,
                    provenance: right_provenance,
                } = right_record;
                debug_assert_eq!(address, right_address);
                modified(
                    changes,
                    path,
                    left_provenance,
                    right_provenance,
                    provenance_summary,
                );
            }
            None => removed(changes, path, tombstone_summary(left_record)),
        }
    }
    for right_record in right.iter() {
        let TombstoneRecord {
            address,
            provenance: _,
        } = right_record;
        if !left.contains_address(address) {
            changes.push(SemanticChange::new(
                SemanticChangeKind::Tombstoned,
                DeckPath::Tombstone {
                    address: address.clone(),
                }
                .to_string(),
                None,
                Some(tombstone_summary(right_record)),
            ));
        }
    }
}

fn diff_entity_map<K, V>(
    changes: &mut Vec<SemanticChange>,
    left: &BTreeMap<K, V>,
    right: &BTreeMap<K, V>,
    path: impl Fn(K) -> String,
    summary: impl Fn(&V) -> String,
    mut compare: impl FnMut(&mut Vec<SemanticChange>, &K, &V, &V),
) where
    K: Clone + Ord,
{
    for (key, left_value) in left {
        match right.get(key) {
            Some(right_value) => compare(changes, key, left_value, right_value),
            None => removed(changes, path(key.clone()), summary(left_value)),
        }
    }
    for (key, right_value) in right {
        if !left.contains_key(key) {
            added(changes, path(key.clone()), summary(right_value));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn diff_ordered_entities<V: PartialEq>(
    changes: &mut Vec<SemanticChange>,
    collection_path: String,
    left: &[V],
    right: &[V],
    id: impl Fn(&V) -> &StableId,
    summary: impl Fn(&V) -> String,
    path: impl Fn(StableId) -> String,
    mut compare: impl FnMut(&mut Vec<SemanticChange>, &StableId, &V, &V),
) {
    let left_ids = left.iter().map(&id).cloned().collect::<Vec<_>>();
    let right_ids = right.iter().map(&id).cloned().collect::<Vec<_>>();
    let left_unique = left_ids.iter().collect::<BTreeSet<_>>().len() == left_ids.len();
    let right_unique = right_ids.iter().collect::<BTreeSet<_>>().len() == right_ids.len();

    if left_unique && right_unique {
        let left_by_id = left
            .iter()
            .map(|value| (id(value), value))
            .collect::<BTreeMap<_, _>>();
        let right_by_id = right
            .iter()
            .map(|value| (id(value), value))
            .collect::<BTreeMap<_, _>>();
        if left_by_id.keys().eq(right_by_id.keys()) && left_ids != right_ids {
            modified_string(
                changes,
                collection_path,
                &id_sequence_summary(&left_ids),
                &id_sequence_summary(&right_ids),
            );
        }
        for (entity_id, left_value) in &left_by_id {
            match right_by_id.get(entity_id) {
                Some(right_value) => compare(changes, entity_id, left_value, right_value),
                None => removed(changes, path((*entity_id).clone()), summary(left_value)),
            }
        }
        for (entity_id, right_value) in &right_by_id {
            if !left_by_id.contains_key(entity_id) {
                added(changes, path((*entity_id).clone()), summary(right_value));
            }
        }
    } else if left != right {
        modified_string(
            changes,
            collection_path,
            &sequence_summary(left, &summary),
            &sequence_summary(right, &summary),
        );
    }
}

fn diff_string_map(
    changes: &mut Vec<SemanticChange>,
    left: &BTreeMap<String, String>,
    right: &BTreeMap<String, String>,
    path: impl Fn(String) -> String,
) {
    for (key, left_value) in left {
        match right.get(key) {
            Some(right_value) => {
                modified_string(changes, path(key.clone()), left_value, right_value)
            }
            None => removed(changes, path(key.clone()), left_value.clone()),
        }
    }
    for (key, right_value) in right {
        if !left.contains_key(key) {
            added(changes, path(key.clone()), right_value.clone());
        }
    }
}

fn diff_adapter_ids(
    changes: &mut Vec<SemanticChange>,
    left: &AdapterIds,
    right: &AdapterIds,
    path: impl Fn(String) -> String,
) {
    let left = left.iter().collect::<BTreeMap<_, _>>();
    let right = right.iter().collect::<BTreeMap<_, _>>();
    for (key, left_value) in &left {
        match right.get(key) {
            Some(right_value) => {
                modified_string(changes, path((*key).to_owned()), left_value, right_value)
            }
            None => removed(changes, path((*key).to_owned()), (*left_value).to_owned()),
        }
    }
    for (key, right_value) in &right {
        if !left.contains_key(key) {
            added(changes, path((*key).to_owned()), (*right_value).to_owned());
        }
    }
}

fn diff_string_set(
    changes: &mut Vec<SemanticChange>,
    left: &BTreeSet<String>,
    right: &BTreeSet<String>,
    path: impl Fn(String) -> String,
) {
    for value in left.difference(right) {
        removed(changes, path(value.clone()), value.clone());
    }
    for value in right.difference(left) {
        added(changes, path(value.clone()), value.clone());
    }
}

fn diff_optional_string(
    changes: &mut Vec<SemanticChange>,
    path: String,
    left: Option<&String>,
    right: Option<&String>,
) {
    match (left, right) {
        (Some(left), Some(right)) => modified_string(changes, path, left, right),
        (Some(left), None) => removed(changes, path, left.clone()),
        (None, Some(right)) => added(changes, path, right.clone()),
        (None, None) => {}
    }
}

fn modified<T: PartialEq>(
    changes: &mut Vec<SemanticChange>,
    path: String,
    left: &T,
    right: &T,
    summary: impl Fn(&T) -> String,
) {
    if left != right {
        changes.push(SemanticChange::new(
            SemanticChangeKind::Modified,
            path,
            Some(summary(left)),
            Some(summary(right)),
        ));
    }
}

fn modified_string(changes: &mut Vec<SemanticChange>, path: String, left: &str, right: &str) {
    if left != right {
        changes.push(SemanticChange::new(
            SemanticChangeKind::Modified,
            path,
            Some(left.to_owned()),
            Some(right.to_owned()),
        ));
    }
}

fn added(changes: &mut Vec<SemanticChange>, path: String, after: String) {
    changes.push(SemanticChange::new(
        SemanticChangeKind::Added,
        path,
        None,
        Some(after),
    ));
}

fn removed(changes: &mut Vec<SemanticChange>, path: String, before: String) {
    changes.push(SemanticChange::new(
        SemanticChangeKind::Removed,
        path,
        Some(before),
        None,
    ));
}

fn kind_rank(kind: SemanticChangeKind) -> u8 {
    match kind {
        SemanticChangeKind::Removed => 0,
        SemanticChangeKind::Modified => 1,
        SemanticChangeKind::Added => 2,
        SemanticChangeKind::Tombstoned => 3,
    }
}

fn field_definition_summary(field: &FieldDefinition) -> String {
    let FieldDefinition { id, name } = field;
    format!("field(id={},name={})", quote(id.as_str()), quote(name))
}

fn card_template_summary(template: &CardTemplate) -> String {
    let CardTemplate {
        id,
        name,
        variables,
        question_format,
        answer_format,
        adapter_ids,
    } = template;
    format!(
        "template(id={},name={},variables={},question_format={},answer_format={},adapter_ids={})",
        quote(id.as_str()),
        quote(name),
        string_map_summary(variables),
        quote(question_format),
        quote(answer_format),
        adapter_ids_summary(adapter_ids)
    )
}

fn note_type_summary(note_type: &NoteType) -> String {
    let NoteType {
        id,
        name,
        variables,
        fields,
        card_templates,
        styling,
        adapter_ids,
    } = note_type;
    format!(
        "note_type(id={},name={},variables={},fields={},card_templates={},styling={},adapter_ids={})",
        quote(id.as_str()),
        quote(name),
        string_map_summary(variables),
        sequence_summary(fields, &field_definition_summary),
        sequence_summary(card_templates, &card_template_summary),
        quote(styling),
        adapter_ids_summary(adapter_ids)
    )
}

fn note_summary(note: &Note) -> String {
    let Note {
        id,
        note_type_id,
        variables,
        fields,
        tags,
        adapter_ids,
    } = note;
    let fields = fields
        .iter()
        .map(|(field_id, value)| {
            format!(
                "{}:{}",
                quote(field_id.as_str()),
                field_value_summary(value)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "note(id={},note_type_id={},variables={},fields={{{}}},tags={},adapter_ids={})",
        quote(id.as_str()),
        quote(note_type_id.as_str()),
        string_map_summary(variables),
        fields,
        string_set_summary(tags),
        adapter_ids_summary(adapter_ids)
    )
}

fn field_value_summary(value: &FieldValue) -> String {
    match value {
        FieldValue::Scalar(value) => format!("scalar({})", quote(value)),
        FieldValue::Images(images) => {
            let values = images
                .iter()
                .map(|image| {
                    let FieldImageReference { media_id } = image;
                    quote(media_id.as_str())
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("images[{values}]")
        }
        FieldValue::Message(message) => message_summary(message),
    }
}

fn message_summary(message: &StructuredMessage) -> String {
    let StructuredMessage {
        components,
        format,
        variables,
    } = message;
    let components = sequence_summary(components, &message_component_summary);
    let format = format
        .as_ref()
        .map(|value| quote(value))
        .unwrap_or_else(|| "none".to_owned());
    let variables = variables
        .iter()
        .map(|(key, value)| format!("{}:{}", quote(key), message_component_summary(value)))
        .collect::<Vec<_>>()
        .join(",");
    format!("message(components={components},format={format},variables={{{variables}}})")
}

fn message_component_summary(component: &MessageComponent) -> String {
    match component {
        MessageComponent::Literal(value) => format!("literal({})", quote(value)),
        MessageComponent::Text(value) => format!("text({})", quote(value)),
        MessageComponent::FieldRef(value) => format!("field_ref({})", quote(value)),
    }
}

fn media_summary(media: &MediaReference) -> String {
    let MediaReference { id, path, sha256 } = media;
    format!(
        "media(id={},path={},sha256={})",
        quote(id.as_str()),
        quote(path),
        quote(sha256)
    )
}

fn tombstone_summary(record: &TombstoneRecord) -> String {
    let TombstoneRecord {
        address,
        provenance,
    } = record;
    format!(
        "tombstone(kind={},path={},provenance={})",
        address.kind(),
        quote(&address.to_string()),
        provenance_summary(provenance)
    )
}

fn provenance_summary(provenance: &Option<RemovalProvenance>) -> String {
    match provenance {
        Some(provenance) => {
            let RemovalProvenance {
                overlay_id,
                operation,
            } = provenance;
            format!(
                "removed_by(overlay_id={},operation={})",
                quote(overlay_id.as_str()),
                operation.as_str()
            )
        }
        None => "legacy".to_owned(),
    }
}

fn string_map_summary(values: &BTreeMap<String, String>) -> String {
    let values = values
        .iter()
        .map(|(key, value)| format!("{}:{}", quote(key), quote(value)))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{values}}}")
}

fn string_set_summary(values: &BTreeSet<String>) -> String {
    let values = values
        .iter()
        .map(|value| quote(value))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{values}}}")
}

fn adapter_ids_summary(values: &AdapterIds) -> String {
    let values = values
        .iter()
        .map(|(key, value)| format!("{}:{}", quote(key), quote(value)))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{values}}}")
}

fn id_sequence_summary(values: &[StableId]) -> String {
    let values = values
        .iter()
        .map(|value| quote(value.as_str()))
        .collect::<Vec<_>>()
        .join(",");
    format!("[{values}]")
}

fn sequence_summary<V>(values: &[V], summary: &impl Fn(&V) -> String) -> String {
    format!(
        "[{}]",
        values.iter().map(summary).collect::<Vec<_>>().join(",")
    )
}

fn quote(value: &str) -> String {
    let mut result = String::with_capacity(value.len() + 2);
    result.push('"');
    for ch in value.chars() {
        match ch {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            ch if ch.is_control() => {
                use std::fmt::Write;
                write!(result, "\\u{{{:x}}}", ch as u32).expect("writing to String cannot fail");
            }
            ch => result.push(ch),
        }
    }
    result.push('"');
    result
}
