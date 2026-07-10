use std::collections::{BTreeMap, BTreeSet};

use crate::messages::resolve_structured_messages_with_validation_errors;
use crate::translation::apply_translation_dictionary;
use crate::*;

impl CanonicalDeck {
    /// Apply an ordered overlay stack to this base deck.
    pub fn compose(&self, overlays: &[Overlay]) -> Result<Self, ComposeReport> {
        let mut resolved = self.clone();
        let mut errors = Vec::new();
        let mut changed_paths = BTreeMap::<String, StableId>::new();

        for overlay in overlays {
            apply_overlay(&mut resolved, overlay, &mut changed_paths, &mut errors);
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

    pub fn semantic_diff(&self, other: &Self) -> SemanticDiff {
        let mut changes = Vec::new();

        push_modified_if_changed(&mut changes, deck_name_path(), &self.name, &other.name);
        push_modified_if_changed(
            &mut changes,
            deck_description_path(),
            &self.description,
            &other.description,
        );
        push_modified_if_changed(
            &mut changes,
            deck_variables_path(),
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
            deck_name_path(),
            name,
            changed_paths,
            errors,
        );
    }
    if let Some(description) = &change.description {
        apply_string_property_change(
            &mut resolved.description,
            overlay,
            deck_description_path(),
            description,
            changed_paths,
            errors,
        );
    }
    apply_variable_changes(
        &mut resolved.variables,
        overlay,
        &deck_variables_path(),
        &change.variables,
        changed_paths,
        errors,
    );
    for (key, adapter_change) in &change.adapter_ids {
        apply_adapter_id_change(
            &mut resolved.adapter_ids,
            overlay,
            deck_adapter_id_path(key),
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
    let path = note_type_path(note_type_id);
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
    let path = note_type_path(note_type_id);
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
    if resolved.notes.iter().any(|(note_id, note)| {
        &note.note_type_id == note_type_id && !resolved.tombstones.contains(note_id)
    }) {
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
    let path = note_type_path(note_type_id);
    if change.note_type.is_some() {
        errors.push(ComposeError::new(
            ComposeErrorKind::ValidationFailed,
            path,
            "a full entity body is only valid with intent `add`; use add, or express edits as field/sub-changes".to_owned(),
        ));
        return added_fields;
    }
    if requires_expected_base(change.intent)
        && !has_expected_base(&change.expected_base, note_type_path(note_type_id), errors)
    {
        return added_fields;
    }

    let Some(note_type) = resolved.note_types.get_mut(note_type_id) else {
        errors.push(ComposeError::new(
            ComposeErrorKind::MissingOverlayTarget,
            note_type_path(note_type_id),
            format!("note type {note_type_id} does not exist"),
        ));
        return added_fields;
    };

    if let Some(name) = &change.name {
        apply_string_property_change(
            &mut note_type.name,
            overlay,
            note_type_name_path(note_type_id),
            name,
            changed_paths,
            errors,
        );
    }
    apply_variable_changes(
        &mut note_type.variables,
        overlay,
        &note_type_variables_path(note_type_id),
        &change.variables,
        changed_paths,
        errors,
    );
    if let Some(styling) = &change.styling {
        apply_string_property_change(
            &mut note_type.styling,
            overlay,
            note_type_styling_path(note_type_id),
            styling,
            changed_paths,
            errors,
        );
    }
    for (key, adapter_change) in &change.adapter_ids {
        apply_adapter_id_change(
            &mut note_type.adapter_ids,
            overlay,
            note_type_adapter_id_path(note_type_id, key),
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
    let path = card_template_path(note_type_id, template_id);
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
            card_template_name_path(note_type_id, template_id),
            name,
            changed_paths,
            errors,
        );
    }
    apply_variable_changes(
        &mut template.variables,
        overlay,
        &card_template_variables_path(note_type_id, template_id),
        &change.variables,
        changed_paths,
        errors,
    );
    if let Some(question_format) = &change.question_format {
        apply_string_property_change(
            &mut template.question_format,
            overlay,
            card_template_question_format_path(note_type_id, template_id),
            question_format,
            changed_paths,
            errors,
        );
    }
    if let Some(answer_format) = &change.answer_format {
        apply_string_property_change(
            &mut template.answer_format,
            overlay,
            card_template_answer_format_path(note_type_id, template_id),
            answer_format,
            changed_paths,
            errors,
        );
    }
    for (key, adapter_change) in &change.adapter_ids {
        apply_adapter_id_change(
            &mut template.adapter_ids,
            overlay,
            card_template_adapter_id_path(note_type_id, template_id, key),
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
            ExpectedBase::FieldValue(expected) => {
                errors.push(ComposeError::new(
                    ComposeErrorKind::ExpectedBaseMismatch,
                    path,
                    format!("semantic field expected base {expected:?} cannot apply to a scalar property"),
                ));
                return;
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
            variable_entry_path(path_prefix, key),
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
            ExpectedBase::FieldValue(expected) => {
                errors.push(ComposeError::new(
                    ComposeErrorKind::ExpectedBaseMismatch,
                    path,
                    format!("semantic field expected base {expected:?} cannot apply to a scalar property"),
                ));
                return;
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
            ExpectedBase::FieldValue(expected) => {
                errors.push(ComposeError::new(
                    ComposeErrorKind::ExpectedBaseMismatch,
                    path,
                    format!(
                        "semantic field expected base {expected:?} cannot apply to an adapter ID"
                    ),
                ));
                return;
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
    let path = field_definition_path(note_type_id, field_id);
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
            } else if change.intent == ChangeIntent::Add {
                note_type.fields.push(field.clone());
                true
            } else {
                errors.push(ComposeError::new(
                    ComposeErrorKind::MissingOverlayTarget,
                    path,
                    format!("field {field_id} does not exist on note type {note_type_id}"),
                ));
                false
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
    let path = note_path(note_id);
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
    if &note.id != note_id {
        errors.push(ComposeError::new(
            ComposeErrorKind::ValidationFailed,
            path,
            format!(
                "note payload id {} does not match target {note_id}",
                note.id
            ),
        ));
        return;
    }

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
    let path = note_path(note_id);
    if change.note.is_some() {
        errors.push(ComposeError::new(
            ComposeErrorKind::ValidationFailed,
            path,
            "a full entity body is only valid with intent `add`; use add, or express edits as field/sub-changes".to_owned(),
        ));
        return;
    }
    if requires_expected_base(change.intent)
        && !has_expected_base(&change.expected_base, note_path(note_id), errors)
    {
        return;
    }

    let Some(note) = resolved.notes.get_mut(note_id) else {
        errors.push(ComposeError::new(
            ComposeErrorKind::MissingOverlayTarget,
            note_path(note_id),
            format!("note {note_id} does not exist"),
        ));
        return;
    };

    apply_variable_changes(
        &mut note.variables,
        overlay,
        &note_variables_path(note_id),
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
            note_adapter_id_path(note_id, key),
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
    let path = note_tag_path(note_id, tag);
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
            ExpectedBase::FieldValue(expected) => {
                errors.push(ComposeError::new(
                    ComposeErrorKind::ExpectedBaseMismatch,
                    path,
                    format!("semantic field expected base {expected:?} cannot apply to a tag"),
                ));
                return;
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
    let path = note_field_path(note_id, field_id);
    if !record_change_path(&path, overlay, change.intent, changed_paths, errors) {
        return;
    }

    if requires_expected_base(change.intent)
        && !has_expected_base(&change.expected_base, path.clone(), errors)
    {
        return;
    }

    let current_value = note.fields.get(field_id);
    if let Some(expected_base) = &change.expected_base {
        let matches = match expected_base {
            ExpectedBase::Value(expected) => {
                current_value == Some(&FieldValue::Scalar(expected.clone()))
            }
            ExpectedBase::FieldValue(expected) => current_value == Some(expected),
            ExpectedBase::EntityPresent => current_value.is_some(),
        };
        if !matches {
            errors.push(ComposeError::new(
                ComposeErrorKind::ExpectedBaseMismatch,
                path,
                format!(
                    "expected base value {:?}, found {:?}",
                    expected_base, current_value
                ),
            ));
            return;
        }
    }

    if change.intent == ChangeIntent::Remove {
        if change.value.is_some() {
            errors.push(ComposeError::new(
                ComposeErrorKind::MissingOverlayPayload,
                path,
                format!("remove field change for {field_id} must not include a value"),
            ));
            return;
        }
        note.fields.remove(field_id);
        return;
    }

    let Some(replacement) = &change.value else {
        errors.push(ComposeError::new(
            ComposeErrorKind::MissingOverlayPayload,
            path,
            format!("field change for {field_id} must include one semantic value"),
        ));
        return;
    };

    match change.intent {
        ChangeIntent::Add if current_value.is_some_and(|value| !value.is_blank()) => {
            errors.push(ComposeError::new(
                ComposeErrorKind::AlreadyExists,
                path,
                format!("field {field_id} already has a non-blank value on note {note_id}"),
            ));
        }
        ChangeIntent::Merge if current_value.is_some_and(|value| !value.is_blank()) => {
            errors.push(ComposeError::new(
                ComposeErrorKind::ExpectedBaseMismatch,
                path,
                format!(
                    "field-level merge may only fill a blank scalar value; found {:?}",
                    current_value
                ),
            ));
        }
        ChangeIntent::Add
        | ChangeIntent::Merge
        | ChangeIntent::Replace
        | ChangeIntent::Override => {
            note.fields.insert(field_id.clone(), replacement.clone());
        }
        ChangeIntent::Remove => unreachable!("remove handled above"),
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
    let path = media_path(media_id);
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
            ExpectedBase::FieldValue(expected) => {
                errors.push(ComposeError::new(
                    ComposeErrorKind::ExpectedBaseMismatch,
                    path,
                    format!("semantic field expected base {expected:?} cannot apply to media"),
                ));
                return;
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

fn deck_name_path() -> String {
    DeckPath::DeckName.to_string()
}

fn deck_description_path() -> String {
    DeckPath::DeckDescription.to_string()
}

fn deck_variables_path() -> String {
    DeckPath::DeckVariables.to_string()
}

fn deck_adapter_id_path(key: &str) -> String {
    DeckPath::DeckAdapterId {
        key: key.to_owned(),
    }
    .to_string()
}

fn variable_entry_path(path_prefix: &str, key: &str) -> String {
    match path_prefix.parse().ok() {
        Some(DeckPath::DeckVariables) => DeckPath::DeckVariable {
            key: key.to_owned(),
        },
        Some(DeckPath::NoteTypeVariables { note_type_id }) => DeckPath::NoteTypeVariable {
            note_type_id,
            key: key.to_owned(),
        },
        Some(DeckPath::NoteTypeCardTemplateVariables {
            note_type_id,
            template_id,
        }) => DeckPath::NoteTypeCardTemplateVariable {
            note_type_id,
            template_id,
            key: key.to_owned(),
        },
        Some(DeckPath::NoteVariables { note_id }) => DeckPath::NoteVariable {
            note_id,
            key: key.to_owned(),
        },
        _ => panic!("unsupported variable deck path collection {path_prefix:?}"),
    }
    .to_string()
}

fn note_type_path(note_type_id: &StableId) -> String {
    DeckPath::NoteType {
        note_type_id: note_type_id.clone(),
    }
    .to_string()
}

fn note_type_name_path(note_type_id: &StableId) -> String {
    DeckPath::NoteTypeName {
        note_type_id: note_type_id.clone(),
    }
    .to_string()
}

fn note_type_variables_path(note_type_id: &StableId) -> String {
    DeckPath::NoteTypeVariables {
        note_type_id: note_type_id.clone(),
    }
    .to_string()
}

fn note_type_fields_path(note_type_id: &StableId) -> String {
    DeckPath::NoteTypeFields {
        note_type_id: note_type_id.clone(),
    }
    .to_string()
}

fn note_type_card_templates_path(note_type_id: &StableId) -> String {
    DeckPath::NoteTypeCardTemplates {
        note_type_id: note_type_id.clone(),
    }
    .to_string()
}

fn note_type_styling_path(note_type_id: &StableId) -> String {
    DeckPath::NoteTypeStyling {
        note_type_id: note_type_id.clone(),
    }
    .to_string()
}

fn note_type_adapter_ids_path(note_type_id: &StableId) -> String {
    DeckPath::NoteTypeAdapterIds {
        note_type_id: note_type_id.clone(),
    }
    .to_string()
}

fn note_type_adapter_id_path(note_type_id: &StableId, key: &str) -> String {
    DeckPath::NoteTypeAdapterId {
        note_type_id: note_type_id.clone(),
        key: key.to_owned(),
    }
    .to_string()
}

fn card_template_path(note_type_id: &StableId, template_id: &StableId) -> String {
    DeckPath::NoteTypeCardTemplate {
        note_type_id: note_type_id.clone(),
        template_id: template_id.clone(),
    }
    .to_string()
}

fn card_template_name_path(note_type_id: &StableId, template_id: &StableId) -> String {
    DeckPath::NoteTypeCardTemplateName {
        note_type_id: note_type_id.clone(),
        template_id: template_id.clone(),
    }
    .to_string()
}

fn card_template_variables_path(note_type_id: &StableId, template_id: &StableId) -> String {
    DeckPath::NoteTypeCardTemplateVariables {
        note_type_id: note_type_id.clone(),
        template_id: template_id.clone(),
    }
    .to_string()
}

fn card_template_question_format_path(note_type_id: &StableId, template_id: &StableId) -> String {
    DeckPath::NoteTypeCardTemplateQuestionFormat {
        note_type_id: note_type_id.clone(),
        template_id: template_id.clone(),
    }
    .to_string()
}

fn card_template_answer_format_path(note_type_id: &StableId, template_id: &StableId) -> String {
    DeckPath::NoteTypeCardTemplateAnswerFormat {
        note_type_id: note_type_id.clone(),
        template_id: template_id.clone(),
    }
    .to_string()
}

fn card_template_adapter_id_path(
    note_type_id: &StableId,
    template_id: &StableId,
    key: &str,
) -> String {
    DeckPath::NoteTypeCardTemplateAdapterId {
        note_type_id: note_type_id.clone(),
        template_id: template_id.clone(),
        key: key.to_owned(),
    }
    .to_string()
}

fn field_definition_path(note_type_id: &StableId, field_id: &StableId) -> String {
    DeckPath::NoteTypeField {
        note_type_id: note_type_id.clone(),
        field_id: field_id.clone(),
    }
    .to_string()
}

fn note_path(note_id: &StableId) -> String {
    DeckPath::Note {
        note_id: note_id.clone(),
    }
    .to_string()
}

fn note_note_type_id_path(note_id: &StableId) -> String {
    DeckPath::NoteNoteTypeId {
        note_id: note_id.clone(),
    }
    .to_string()
}

fn note_variables_path(note_id: &StableId) -> String {
    DeckPath::NoteVariables {
        note_id: note_id.clone(),
    }
    .to_string()
}

fn note_tags_path(note_id: &StableId) -> String {
    DeckPath::NoteTags {
        note_id: note_id.clone(),
    }
    .to_string()
}

fn note_tag_path(note_id: &StableId, tag: &str) -> String {
    DeckPath::NoteTag {
        note_id: note_id.clone(),
        tag: tag.to_owned(),
    }
    .to_string()
}

fn note_adapter_ids_path(note_id: &StableId) -> String {
    DeckPath::NoteAdapterIds {
        note_id: note_id.clone(),
    }
    .to_string()
}

fn note_adapter_id_path(note_id: &StableId, key: &str) -> String {
    DeckPath::NoteAdapterId {
        note_id: note_id.clone(),
        key: key.to_owned(),
    }
    .to_string()
}

fn note_field_path(note_id: &StableId, field_id: &StableId) -> String {
    DeckPath::NoteField {
        note_id: note_id.clone(),
        field_id: field_id.clone(),
    }
    .to_string()
}

fn note_field_message_path(note_id: &StableId, field_id: &StableId) -> String {
    DeckPath::NoteFieldMessage {
        note_id: note_id.clone(),
        field_id: field_id.clone(),
    }
    .to_string()
}

fn media_path(media_id: &StableId) -> String {
    DeckPath::Media {
        media_id: media_id.clone(),
    }
    .to_string()
}

fn media_path_path(media_id: &StableId) -> String {
    DeckPath::MediaPath {
        media_id: media_id.clone(),
    }
    .to_string()
}

fn media_sha256_path(media_id: &StableId) -> String {
    DeckPath::MediaSha256 {
        media_id: media_id.clone(),
    }
    .to_string()
}

fn tombstone_path(id: &StableId) -> String {
    DeckPath::Tombstone { id: id.clone() }.to_string()
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
    let path = note_path(note_id);
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

pub(crate) fn record_change_path(
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
    let mut image_errors = Vec::new();
    let deck_variables = rendered.variables.clone();
    let media_paths = rendered
        .media
        .iter()
        .map(|(id, media)| (id.clone(), media.path.clone()))
        .collect::<BTreeMap<_, _>>();

    render_string_with_variables(
        &mut rendered.name,
        &deck_name_path(),
        &[&deck_variables],
        &mut errors,
    );
    render_string_with_variables(
        &mut rendered.description,
        &deck_description_path(),
        &[&deck_variables],
        &mut errors,
    );

    for (note_type_id, note_type) in &mut rendered.note_types {
        let note_type_variables = note_type.variables.clone();
        render_string_with_variables(
            &mut note_type.name,
            &note_type_name_path(note_type_id),
            &[&note_type_variables, &deck_variables],
            &mut errors,
        );
        render_string_with_variables(
            &mut note_type.styling,
            &note_type_styling_path(note_type_id),
            &[&note_type_variables, &deck_variables],
            &mut errors,
        );
        for field in &mut note_type.fields {
            render_string_with_variables(
                &mut field.name,
                &DeckPath::NoteTypeFieldName {
                    note_type_id: note_type_id.clone(),
                    field_id: field.id.clone(),
                }
                .to_string(),
                &[&note_type_variables, &deck_variables],
                &mut errors,
            );
        }
        for template in &mut note_type.card_templates {
            let template_variables = template.variables.clone();
            let scopes = [&template_variables, &note_type_variables, &deck_variables];
            render_string_with_variables(
                &mut template.name,
                &card_template_name_path(note_type_id, &template.id),
                &scopes,
                &mut errors,
            );
            render_string_with_variables(
                &mut template.question_format,
                &card_template_question_format_path(note_type_id, &template.id),
                &scopes,
                &mut errors,
            );
            render_string_with_variables(
                &mut template.answer_format,
                &card_template_answer_format_path(note_type_id, &template.id),
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
            let path = note_field_path(note_id, field_id);
            let scopes = if let Some(note_type_variables) = note_type_variables {
                vec![&note_variables, note_type_variables, &deck_variables]
            } else {
                vec![&note_variables, &deck_variables]
            };
            match value {
                FieldValue::Scalar(value) => {
                    render_string_with_variables(value, &path, &scopes, &mut errors);
                }
                FieldValue::Message(message) => {
                    render_message_variables(
                        message,
                        &note_field_message_path(note_id, field_id),
                        &scopes,
                        &mut errors,
                    );
                }
                FieldValue::Images(_) => {}
            }
        }
        render_image_fields(note_id, note, &media_paths, &mut image_errors);
    }

    if errors.is_empty() {
        let mut validation_errors = image_errors;
        resolve_structured_messages_with_validation_errors(&mut rendered, &mut validation_errors);
        if validation_errors.is_empty() {
            Ok(rendered)
        } else {
            Err(VariableRenderReport {
                errors,
                validation_errors,
            })
        }
    } else {
        Err(VariableRenderReport {
            errors,
            validation_errors: Vec::new(),
        })
    }
}

fn render_image_fields(
    note_id: &StableId,
    note: &mut Note,
    media_paths: &BTreeMap<StableId, String>,
    errors: &mut Vec<ValidationError>,
) {
    let image_fields = note
        .fields
        .iter()
        .filter_map(|(field_id, value)| {
            value
                .as_images()
                .map(|images| (field_id.clone(), images.to_vec()))
        })
        .collect::<Vec<_>>();
    for (field_id, images) in image_fields {
        let path = note_field_path(note_id, &field_id);
        let mut rendered = String::new();
        let mut field_has_error = false;
        for image in images {
            let Some(media_path) = media_paths.get(&image.media_id) else {
                errors.push(ValidationError::new(
                    ValidationErrorKind::UnknownMediaReference,
                    path.clone(),
                    format!(
                        "unknown media id \"{}\" referenced in field {path}",
                        image.media_id
                    ),
                ));
                field_has_error = true;
                continue;
            };
            let encoded_path = encode_media_path_for_url(media_path);
            rendered.push_str("<img src=\"");
            rendered.push_str(&escape_html_attribute(&encoded_path));
            rendered.push_str("\" />");
        }
        if !field_has_error {
            note.fields
                .insert(field_id.clone(), FieldValue::Scalar(rendered));
        }
    }
}

fn encode_media_path_for_url(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for byte in path.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'.' | b'_' | b'~' | b'/') {
            encoded.push(char::from(*byte));
        } else {
            use std::fmt::Write as _;
            write!(&mut encoded, "%{byte:02X}").expect("writing to String cannot fail");
        }
    }
    encoded
}

fn escape_html_attribute(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn render_message_variables(
    message: &mut StructuredMessage,
    path: &str,
    scopes: &[&BTreeMap<String, String>],
    errors: &mut Vec<VariableRenderError>,
) {
    let DeckPath::NoteFieldMessage { note_id, field_id } = path.parse().expect("message path")
    else {
        panic!("unsupported message path {path:?}");
    };

    if let Some(format) = &mut message.format {
        render_string_with_variables(
            format,
            &DeckPath::NoteFieldMessageFormat {
                note_id: note_id.clone(),
                field_id: field_id.clone(),
            }
            .to_string(),
            scopes,
            errors,
        );
        for (variable, component) in &mut message.variables {
            render_message_component_variables(
                component,
                &DeckPath::NoteFieldMessageVariable {
                    note_id: note_id.clone(),
                    field_id: field_id.clone(),
                    variable: variable.clone(),
                }
                .to_string(),
                scopes,
                errors,
            );
        }
        return;
    }

    for (index, component) in message.components.iter_mut().enumerate() {
        render_message_component_variables(
            component,
            &DeckPath::NoteFieldMessageComponent {
                note_id: note_id.clone(),
                field_id: field_id.clone(),
                index,
            }
            .to_string(),
            scopes,
            errors,
        );
    }
}

fn render_message_component_variables(
    component: &mut MessageComponent,
    path: &str,
    scopes: &[&BTreeMap<String, String>],
    errors: &mut Vec<VariableRenderError>,
) {
    match component {
        MessageComponent::Literal(value) | MessageComponent::Text(value) => {
            render_string_with_variables(value, path, scopes, errors);
        }
        MessageComponent::FieldRef(reference) => {
            render_string_with_variables(reference, path, scopes, errors);
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
            changes.push(SemanticChange::removed(note_type_path(id)));
        }
    }

    for (id, right_note_type) in right {
        let Some(left_note_type) = left.get(id) else {
            changes.push(SemanticChange::added(note_type_path(id)));
            continue;
        };

        push_modified_if_changed(
            changes,
            note_type_name_path(id),
            &left_note_type.name,
            &right_note_type.name,
        );
        push_modified_if_changed(
            changes,
            note_type_styling_path(id),
            &left_note_type.styling,
            &right_note_type.styling,
        );
        push_modified_if_changed(
            changes,
            note_type_variables_path(id),
            &string_map_summary(&left_note_type.variables),
            &string_map_summary(&right_note_type.variables),
        );
        push_modified_if_changed(
            changes,
            note_type_fields_path(id),
            &field_summary(&left_note_type.fields),
            &field_summary(&right_note_type.fields),
        );
        push_modified_if_changed(
            changes,
            note_type_card_templates_path(id),
            &template_summary(&left_note_type.card_templates),
            &template_summary(&right_note_type.card_templates),
        );
        push_modified_if_changed(
            changes,
            note_type_adapter_ids_path(id),
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
            changes.push(SemanticChange::removed(note_path(id)));
        }
    }

    for (id, right_note) in right {
        let Some(left_note) = left.get(id) else {
            changes.push(SemanticChange::added(note_path(id)));
            continue;
        };

        push_modified_if_changed(
            changes,
            note_note_type_id_path(id),
            &left_note.note_type_id.to_string(),
            &right_note.note_type_id.to_string(),
        );
        push_modified_if_changed(
            changes,
            note_variables_path(id),
            &string_map_summary(&left_note.variables),
            &string_map_summary(&right_note.variables),
        );
        diff_note_fields(id, &left_note.fields, &right_note.fields, changes);
        push_modified_if_changed(
            changes,
            note_tags_path(id),
            &set_summary(&left_note.tags),
            &set_summary(&right_note.tags),
        );
        push_modified_if_changed(
            changes,
            note_adapter_ids_path(id),
            &adapter_ids_summary(&left_note.adapter_ids),
            &adapter_ids_summary(&right_note.adapter_ids),
        );
    }
}

fn diff_note_fields(
    note_id: &StableId,
    left: &BTreeMap<StableId, FieldValue>,
    right: &BTreeMap<StableId, FieldValue>,
    changes: &mut Vec<SemanticChange>,
) {
    for field_id in left.keys() {
        if !right.contains_key(field_id) {
            changes.push(SemanticChange::new(
                SemanticChangeKind::Removed,
                note_field_path(note_id, field_id),
                left.get(field_id).map(field_value_summary),
                None,
            ));
        }
    }

    for (field_id, right_value) in right {
        let Some(left_value) = left.get(field_id) else {
            changes.push(SemanticChange::new(
                SemanticChangeKind::Added,
                note_field_path(note_id, field_id),
                None,
                Some(field_value_summary(right_value)),
            ));
            continue;
        };

        push_modified_if_changed(
            changes,
            note_field_path(note_id, field_id),
            &field_value_summary(left_value),
            &field_value_summary(right_value),
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
            changes.push(SemanticChange::removed(media_path(id)));
        }
    }

    for (id, right_media) in right {
        let Some(left_media) = left.get(id) else {
            changes.push(SemanticChange::added(media_path(id)));
            continue;
        };

        push_modified_if_changed(
            changes,
            media_path_path(id),
            &left_media.path,
            &right_media.path,
        );
        push_modified_if_changed(
            changes,
            media_sha256_path(id),
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
            changes.push(SemanticChange::removed(tombstone_path(id)));
        }
    }

    for id in right {
        if !left.contains(id) {
            changes.push(SemanticChange::new(
                SemanticChangeKind::Tombstoned,
                tombstone_path(id),
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

fn field_value_summary(value: &FieldValue) -> String {
    match value {
        FieldValue::Scalar(value) => value.clone(),
        FieldValue::Images(images) => format!("images:{images:?}"),
        FieldValue::Message(message) => format!("message:{message:?}"),
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
