use std::collections::BTreeMap;

use crate::messages::lower_images_from_deck;
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

        resolved.validate().map_err(|report| {
            let first = report.errors.first();
            let mut error = ComposeError::new(
                ComposeErrorKind::ValidationFailed,
                first.map(|issue| issue.path.clone()).unwrap_or_default(),
                format!(
                    "composed deck failed final validation with {} issue{}",
                    report.errors.len(),
                    if report.errors.len() == 1 { "" } else { "s" }
                ),
            );
            error.source_id = Some(self.id.clone());
            error.validation_errors = report.errors;
            ComposeReport {
                errors: vec![error],
            }
        })?;

        Ok(resolved)
    }

    /// Compare every canonical property using exact typed semantics.
    ///
    /// Map/set insertion order is normalized by their domain collections. Explicit field,
    /// template, image, and message-component sequence order remains semantic.
    pub fn semantic_diff(&self, other: &Self) -> SemanticDiff {
        crate::semantic_diff::diff(self, other)
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
    let mutations = overlay_mutation_addresses(overlay);
    let mut blocked = false;
    for (address, intent) in &mutations {
        if let Some(record) = resolved.tombstones.blocking(address) {
            let path = address.to_string();
            let removal = record
                .provenance
                .as_ref()
                .map(|provenance| {
                    format!(
                        " by overlay {} with {}",
                        provenance.overlay_id,
                        provenance.operation.as_str()
                    )
                })
                .unwrap_or_else(|| " in legacy canonical source".to_owned());
            let mut error = ComposeError::new(
                ComposeErrorKind::TombstonedAddressReuse,
                path.clone(),
                format!(
                    "overlay {} cannot {} {path}; typed address {} was removed{removal} and removal provenance cannot be erased",
                    overlay.id,
                    intent.as_str(),
                    record.address
                ),
            );
            error.intent = Some(*intent);
            error.overlay_id = Some(overlay.id.clone());
            error.original_removal = Some(record.clone());
            errors.push(error);
            blocked = true;
        }
    }
    if blocked {
        return;
    }

    let errors_before = errors.len();
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

    for error in &mut errors[errors_before..] {
        if error.overlay_id.is_none() {
            error.overlay_id = Some(overlay.id.clone());
        }
    }

    if errors.len() == errors_before {
        for (address, intent) in mutations {
            if intent == ChangeIntent::Remove {
                resolved
                    .tombstones
                    .insert(TombstoneRecord::removed_by(address, overlay.id.clone()));
            }
        }
    }
}

fn overlay_mutation_addresses(overlay: &Overlay) -> Vec<(TombstoneAddress, ChangeIntent)> {
    let mut addresses = Vec::new();
    if let Some(change) = &overlay.deck_change {
        if let Some(item) = &change.name {
            addresses.push((TombstoneAddress::DeckName, item.intent));
        }
        if let Some(item) = &change.description {
            addresses.push((TombstoneAddress::DeckDescription, item.intent));
        }
        addresses.extend(change.variables.iter().map(|(key, item)| {
            (
                TombstoneAddress::DeckVariable { key: key.clone() },
                item.intent,
            )
        }));
        addresses.extend(change.adapter_ids.iter().map(|(key, item)| {
            (
                TombstoneAddress::DeckAdapterId { key: key.clone() },
                item.intent,
            )
        }));
    }
    for (note_type_id, change) in &overlay.note_type_changes {
        let parent = TombstoneAddress::NoteType {
            note_type_id: note_type_id.clone(),
        };
        if matches!(change.intent, ChangeIntent::Add | ChangeIntent::Remove)
            || change.note_type.is_some()
        {
            addresses.push((parent, change.intent));
            continue;
        }
        if let Some(item) = &change.name {
            addresses.push((
                TombstoneAddress::NoteTypeName {
                    note_type_id: note_type_id.clone(),
                },
                item.intent,
            ));
        }
        if let Some(item) = &change.styling {
            addresses.push((
                TombstoneAddress::NoteTypeStyling {
                    note_type_id: note_type_id.clone(),
                },
                item.intent,
            ));
        }
        addresses.extend(change.variables.iter().map(|(key, item)| {
            (
                TombstoneAddress::NoteTypeVariable {
                    note_type_id: note_type_id.clone(),
                    key: key.clone(),
                },
                item.intent,
            )
        }));
        addresses.extend(change.adapter_ids.iter().map(|(key, item)| {
            (
                TombstoneAddress::NoteTypeAdapterId {
                    note_type_id: note_type_id.clone(),
                    key: key.clone(),
                },
                item.intent,
            )
        }));
        for (field_id, item) in &change.fields {
            addresses.push((
                TombstoneAddress::FieldDefinition {
                    note_type_id: note_type_id.clone(),
                    field_id: field_id.clone(),
                },
                item.intent,
            ));
        }
        for (template_id, item) in &change.card_templates {
            let template = TombstoneAddress::CardTemplate {
                note_type_id: note_type_id.clone(),
                template_id: template_id.clone(),
            };
            if matches!(item.intent, ChangeIntent::Add | ChangeIntent::Remove)
                || item.template.is_some()
            {
                addresses.push((template, item.intent));
                continue;
            }
            if let Some(change) = &item.name {
                addresses.push((
                    TombstoneAddress::CardTemplateName {
                        note_type_id: note_type_id.clone(),
                        template_id: template_id.clone(),
                    },
                    change.intent,
                ));
            }
            if let Some(change) = &item.question_format {
                addresses.push((
                    TombstoneAddress::CardTemplateQuestionFormat {
                        note_type_id: note_type_id.clone(),
                        template_id: template_id.clone(),
                    },
                    change.intent,
                ));
            }
            if let Some(change) = &item.answer_format {
                addresses.push((
                    TombstoneAddress::CardTemplateAnswerFormat {
                        note_type_id: note_type_id.clone(),
                        template_id: template_id.clone(),
                    },
                    change.intent,
                ));
            }
            addresses.extend(item.variables.iter().map(|(key, change)| {
                (
                    TombstoneAddress::CardTemplateVariable {
                        note_type_id: note_type_id.clone(),
                        template_id: template_id.clone(),
                        key: key.clone(),
                    },
                    change.intent,
                )
            }));
            addresses.extend(item.adapter_ids.iter().map(|(key, change)| {
                (
                    TombstoneAddress::CardTemplateAdapterId {
                        note_type_id: note_type_id.clone(),
                        template_id: template_id.clone(),
                        key: key.clone(),
                    },
                    change.intent,
                )
            }));
        }
    }
    for (note_id, change) in &overlay.note_changes {
        let parent = TombstoneAddress::Note {
            note_id: note_id.clone(),
        };
        if matches!(change.intent, ChangeIntent::Add | ChangeIntent::Remove)
            || change.note.is_some()
        {
            addresses.push((parent, change.intent));
            continue;
        }
        addresses.extend(change.variables.iter().map(|(key, item)| {
            (
                TombstoneAddress::NoteVariable {
                    note_id: note_id.clone(),
                    key: key.clone(),
                },
                item.intent,
            )
        }));
        addresses.extend(change.fields.iter().map(|(field_id, item)| {
            (
                TombstoneAddress::NoteField {
                    note_id: note_id.clone(),
                    field_id: field_id.clone(),
                },
                item.intent,
            )
        }));
        addresses.extend(change.tags.iter().map(|(tag, item)| {
            (
                TombstoneAddress::NoteTag {
                    note_id: note_id.clone(),
                    tag: tag.clone(),
                },
                item.intent,
            )
        }));
        addresses.extend(change.adapter_ids.iter().map(|(key, item)| {
            (
                TombstoneAddress::NoteAdapterId {
                    note_id: note_id.clone(),
                    key: key.clone(),
                },
                item.intent,
            )
        }));
    }
    addresses.extend(overlay.media_changes.iter().map(|(media_id, item)| {
        (
            TombstoneAddress::MediaReference {
                media_id: media_id.clone(),
            },
            item.intent,
        )
    }));
    addresses
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
    let actual = resolved
        .note_types
        .get(note_type_id)
        .map(fingerprint_note_type);
    if !check_entity_expected_base(
        &change.expected_base,
        actual,
        EntityKind::NoteType,
        &path,
        overlay,
        change.intent,
        errors,
    ) {
        return;
    }
    if !record_change_path(&path, overlay, change.intent, changed_paths, errors) {
        return;
    }
    if resolved.notes.iter().any(|(note_id, note)| {
        &note.note_type_id == note_type_id && is_active_note(resolved, note_id)
    }) {
        errors.push(ComposeError::new(
            ComposeErrorKind::ValidationFailed,
            path,
            format!("cannot remove note type {note_type_id} while notes still reference it"),
        ));
        return;
    }
    resolved.note_types.remove(note_type_id);
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
    if let Some(replacement) = &change.note_type {
        if change.intent == ChangeIntent::Merge {
            errors.push(ComposeError::new(
                ComposeErrorKind::ValidationFailed,
                path,
                "a complete note type requires intent `replace` or `override`; use sparse sub-changes for merge"
                    .to_owned(),
            ));
            return added_fields;
        }
        let actual = resolved
            .note_types
            .get(note_type_id)
            .map(fingerprint_note_type);
        if !check_entity_expected_base(
            &change.expected_base,
            actual,
            EntityKind::NoteType,
            &path,
            overlay,
            change.intent,
            errors,
        ) {
            return added_fields;
        }
        if &replacement.id != note_type_id {
            errors.push(ComposeError::new(
                ComposeErrorKind::ValidationFailed,
                path,
                format!(
                    "note type payload id {} does not match target {note_type_id}",
                    replacement.id
                ),
            ));
            return added_fields;
        }
        if !record_change_path(&path, overlay, change.intent, changed_paths, errors) {
            return added_fields;
        }
        resolved
            .note_types
            .insert(note_type_id.clone(), replacement.clone());
        return added_fields;
    }
    if requires_expected_base(change.intent) {
        let actual = resolved
            .note_types
            .get(note_type_id)
            .map(fingerprint_note_type);
        if !check_entity_expected_base(
            &change.expected_base,
            actual,
            EntityKind::NoteType,
            &path,
            overlay,
            change.intent,
            errors,
        ) {
            return added_fields;
        }
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
    let existing_index = note_type
        .card_templates
        .iter()
        .position(|template| &template.id == template_id);
    let complete_or_destructive = change.intent != ChangeIntent::Add
        && (change.template.is_some() || change.intent == ChangeIntent::Remove);
    if complete_or_destructive {
        let actual =
            existing_index.map(|index| fingerprint_card_template(&note_type.card_templates[index]));
        if !check_entity_expected_base(
            &change.expected_base,
            actual,
            EntityKind::CardTemplate,
            &path,
            overlay,
            change.intent,
            errors,
        ) {
            return;
        }
    }

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
                    errors.push(ComposeError::precondition(
                        ComposeErrorKind::ExpectedBaseMismatch,
                        path,
                        change.intent,
                        overlay.id.clone(),
                        Some(ComposePrecondition::Value(expected_value.clone())),
                        ComposePrecondition::Value(value.clone()),
                        format!(
                            "expected base value {:?}, found {:?}",
                            expected_value, value
                        ),
                    ));
                    return;
                }
            }
            ExpectedBase::EntityPresent => {
                errors.push(legacy_presence_expected_base_error(path));
                return;
            }
            ExpectedBase::EntityFingerprint(fingerprint) => {
                errors.push(fingerprint_on_sparse_value_error(path, fingerprint));
                return;
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
                    errors.push(ComposeError::precondition(
                        ComposeErrorKind::ExpectedBaseMismatch,
                        path,
                        change.intent,
                        overlay.id.clone(),
                        Some(ComposePrecondition::Value(expected_value.clone())),
                        current_value
                            .cloned()
                            .map(ComposePrecondition::Value)
                            .unwrap_or(ComposePrecondition::Missing),
                        format!(
                            "expected base value {:?}, found {:?}",
                            expected_value, current_value
                        ),
                    ));
                    return;
                }
            }
            ExpectedBase::EntityPresent => {
                errors.push(legacy_presence_expected_base_error(path));
                return;
            }
            ExpectedBase::EntityFingerprint(fingerprint) => {
                errors.push(fingerprint_on_sparse_value_error(path, fingerprint));
                return;
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
                    errors.push(ComposeError::precondition(
                        ComposeErrorKind::ExpectedBaseMismatch,
                        path,
                        change.intent,
                        overlay.id.clone(),
                        Some(ComposePrecondition::Value(expected_value.clone())),
                        current_value
                            .map(|value| ComposePrecondition::Value(value.to_owned()))
                            .unwrap_or(ComposePrecondition::Missing),
                        format!(
                            "expected base value {:?}, found {:?}",
                            expected_value, current_value
                        ),
                    ));
                    return;
                }
            }
            ExpectedBase::EntityPresent => {
                errors.push(legacy_presence_expected_base_error(path));
                return;
            }
            ExpectedBase::EntityFingerprint(fingerprint) => {
                errors.push(fingerprint_on_sparse_value_error(path, fingerprint));
                return;
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
    let existing_index = note_type
        .fields
        .iter()
        .position(|field| &field.id == field_id);
    if change.intent != ChangeIntent::Add {
        let actual =
            existing_index.map(|index| fingerprint_field_definition(&note_type.fields[index]));
        if !check_entity_expected_base(
            &change.expected_base,
            actual,
            EntityKind::FieldDefinition,
            &path,
            overlay,
            change.intent,
            errors,
        ) {
            return false;
        }
    }
    if !record_change_path(&path, overlay, change.intent, changed_paths, errors) {
        return false;
    }
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

    if resolved.notes.contains_key(note_id) && is_active_note(resolved, note_id) {
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
    if let Some(replacement) = &change.note {
        if change.intent == ChangeIntent::Merge {
            errors.push(ComposeError::new(
                ComposeErrorKind::ValidationFailed,
                path,
                "a complete note requires intent `replace` or `override`; use sparse sub-changes for merge"
                    .to_owned(),
            ));
            return;
        }
        let actual = live_note(resolved, note_id).map(fingerprint_note);
        if !check_entity_expected_base(
            &change.expected_base,
            actual,
            EntityKind::Note,
            &path,
            overlay,
            change.intent,
            errors,
        ) {
            return;
        }
        if &replacement.id != note_id {
            errors.push(ComposeError::new(
                ComposeErrorKind::ValidationFailed,
                path,
                format!(
                    "note payload id {} does not match target {note_id}",
                    replacement.id
                ),
            ));
            return;
        }
        if !record_change_path(&path, overlay, change.intent, changed_paths, errors) {
            return;
        }
        resolved.notes.insert(note_id.clone(), replacement.clone());
        return;
    }
    if requires_expected_base(change.intent) {
        let actual = live_note(resolved, note_id).map(fingerprint_note);
        if !check_entity_expected_base(
            &change.expected_base,
            actual,
            EntityKind::Note,
            &path,
            overlay,
            change.intent,
            errors,
        ) {
            return;
        }
    }

    if !is_active_note(resolved, note_id) {
        errors.push(ComposeError::new(
            ComposeErrorKind::MissingOverlayTarget,
            note_path(note_id),
            format!("note {note_id} is removed"),
        ));
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
                errors.push(legacy_presence_expected_base_error(path));
                return;
            }
            ExpectedBase::EntityFingerprint(fingerprint) => {
                errors.push(fingerprint_on_sparse_value_error(path, fingerprint));
                return;
            }
            ExpectedBase::Value(expected_value) => {
                if expected_value != tag || !note.tags.contains(tag) {
                    errors.push(ComposeError::precondition(
                        ComposeErrorKind::ExpectedBaseMismatch,
                        path,
                        change.intent,
                        overlay.id.clone(),
                        Some(ComposePrecondition::Value(expected_value.clone())),
                        if note.tags.contains(tag) {
                            ComposePrecondition::Value(tag.to_owned())
                        } else {
                            ComposePrecondition::Missing
                        },
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
            ExpectedBase::EntityPresent => {
                errors.push(legacy_presence_expected_base_error(path));
                return;
            }
            ExpectedBase::EntityFingerprint(fingerprint) => {
                errors.push(fingerprint_on_sparse_value_error(path, fingerprint));
                return;
            }
        };
        if !matches {
            let expected = match expected_base {
                ExpectedBase::Value(value) => ComposePrecondition::Value(value.clone()),
                ExpectedBase::FieldValue(value) => ComposePrecondition::FieldValue(value.clone()),
                ExpectedBase::EntityPresent | ExpectedBase::EntityFingerprint(_) => {
                    unreachable!("invalid sparse expected bases returned above")
                }
            };
            errors.push(ComposeError::precondition(
                ComposeErrorKind::ExpectedBaseMismatch,
                path,
                change.intent,
                overlay.id.clone(),
                Some(expected),
                current_value
                    .cloned()
                    .map(ComposePrecondition::FieldValue)
                    .unwrap_or(ComposePrecondition::Missing),
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
    if change.intent != ChangeIntent::Add {
        let actual = resolved
            .media
            .get(media_id)
            .map(fingerprint_media_reference);
        if !check_entity_expected_base(
            &change.expected_base,
            actual,
            EntityKind::MediaReference,
            &path,
            overlay,
            change.intent,
            errors,
        ) {
            return;
        }
    }
    if !record_change_path(&path, overlay, change.intent, changed_paths, errors) {
        return;
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

fn note_type_styling_path(note_type_id: &StableId) -> String {
    DeckPath::NoteTypeStyling {
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

fn note_variables_path(note_id: &StableId) -> String {
    DeckPath::NoteVariables {
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

fn apply_note_remove(
    resolved: &mut CanonicalDeck,
    overlay: &Overlay,
    note_id: &StableId,
    change: &NoteChange,
    changed_paths: &mut BTreeMap<String, StableId>,
    errors: &mut Vec<ComposeError>,
) {
    let path = note_path(note_id);
    let actual = live_note(resolved, note_id).map(fingerprint_note);
    if !check_entity_expected_base(
        &change.expected_base,
        actual,
        EntityKind::Note,
        &path,
        overlay,
        change.intent,
        errors,
    ) {
        return;
    }
    record_change_path(&path, overlay, change.intent, changed_paths, errors);
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
        let mut error = ComposeError::new(
            ComposeErrorKind::Conflict,
            path.to_owned(),
            format!(
                "overlay {} conflicts with earlier overlay {} at {path}",
                overlay.id, previous_overlay_id
            ),
        );
        error.overlay_id = Some(overlay.id.clone());
        error.first_conflict_participant = Some(previous_overlay_id.clone());
        error.current_conflict_participant = Some(overlay.id.clone());
        errors.push(error);
        return false;
    }

    changed_paths.insert(path.to_owned(), overlay.id.clone());
    true
}

fn note_address(note_id: &StableId) -> TombstoneAddress {
    TombstoneAddress::Note {
        note_id: note_id.clone(),
    }
}

fn is_active_note(resolved: &CanonicalDeck, note_id: &StableId) -> bool {
    resolved
        .tombstones
        .blocking(&note_address(note_id))
        .is_none()
}

fn live_note<'a>(resolved: &'a CanonicalDeck, note_id: &StableId) -> Option<&'a Note> {
    is_active_note(resolved, note_id)
        .then(|| resolved.notes.get(note_id))
        .flatten()
}

fn check_entity_expected_base(
    expected_base: &Option<ExpectedBase>,
    actual: Option<EntityFingerprint>,
    entity_kind: EntityKind,
    path: &str,
    overlay: &Overlay,
    intent: ChangeIntent,
    errors: &mut Vec<ComposeError>,
) -> bool {
    let actual_state = actual
        .clone()
        .map(ComposePrecondition::Fingerprint)
        .unwrap_or(ComposePrecondition::Missing);
    let Some(expected_base) = expected_base else {
        errors.push(
            ComposeError::precondition(
                ComposeErrorKind::MissingExpectedBase,
                path.to_owned(),
                intent,
                overlay.id.clone(),
                None,
                actual_state,
                format!(
                    "complete {} {intent:?} must declare an entity fingerprint",
                    entity_kind.as_str()
                ),
            )
            .with_entity_kind(entity_kind),
        );
        return false;
    };
    let ExpectedBase::EntityFingerprint(expected) = expected_base else {
        let expected_state = match expected_base {
            ExpectedBase::Value(value) => ComposePrecondition::Value(value.clone()),
            ExpectedBase::FieldValue(value) => ComposePrecondition::FieldValue(value.clone()),
            ExpectedBase::EntityPresent => ComposePrecondition::Value("entity_present".to_owned()),
            ExpectedBase::EntityFingerprint(_) => unreachable!(),
        };
        errors.push(
            ComposeError::precondition(
                ComposeErrorKind::InvalidExpectedBase,
                path.to_owned(),
                intent,
                overlay.id.clone(),
                Some(expected_state),
                actual_state,
                "presence-only and value baselines cannot authorize complete entity changes; regenerate the overlay with `brainbrew diff --as-overlay` to obtain a fingerprint".to_owned(),
            )
            .with_entity_kind(entity_kind),
        );
        return false;
    };
    if actual.as_ref() != Some(expected) {
        errors.push(
            ComposeError::precondition(
                ComposeErrorKind::ExpectedBaseMismatch,
                path.to_owned(),
                intent,
                overlay.id.clone(),
                Some(ComposePrecondition::Fingerprint(expected.clone())),
                actual_state,
                format!(
                    "{} fingerprint precondition failed: expected {}, found {}",
                    entity_kind.as_str(),
                    expected,
                    actual
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| "missing".to_owned())
                ),
            )
            .with_entity_kind(entity_kind),
        );
        return false;
    }
    true
}

fn legacy_presence_expected_base_error(path: String) -> ComposeError {
    ComposeError::new(
        ComposeErrorKind::InvalidExpectedBase,
        path,
        "presence-only expected_base is no longer accepted for destructive changes; use the exact prior value, or regenerate complete entity changes with `brainbrew diff --as-overlay`"
            .to_owned(),
    )
}

fn fingerprint_on_sparse_value_error(
    path: String,
    fingerprint: &EntityFingerprint,
) -> ComposeError {
    ComposeError::new(
        ComposeErrorKind::InvalidExpectedBase,
        path,
        format!(
            "entity fingerprint {fingerprint} cannot guard a sparse property; use its exact typed prior value"
        ),
    )
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
    let tombstones = rendered.tombstones.clone();

    if tombstones.blocking(&TombstoneAddress::DeckName).is_none() {
        render_string_with_variables(
            &mut rendered.name,
            &deck_name_path(),
            &[&deck_variables],
            &mut errors,
        );
    }
    if tombstones
        .blocking(&TombstoneAddress::DeckDescription)
        .is_none()
    {
        render_string_with_variables(
            &mut rendered.description,
            &deck_description_path(),
            &[&deck_variables],
            &mut errors,
        );
    }

    for (note_type_id, note_type) in &mut rendered.note_types {
        if tombstones
            .blocking(&TombstoneAddress::NoteType {
                note_type_id: note_type_id.clone(),
            })
            .is_some()
        {
            continue;
        }
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
            if tombstones
                .blocking(&TombstoneAddress::FieldDefinition {
                    note_type_id: note_type_id.clone(),
                    field_id: field.id.clone(),
                })
                .is_some()
            {
                continue;
            }
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
            if let Some(pattern) = &mut field.message_pattern {
                render_string_with_variables(
                    &mut pattern.item_format,
                    &DeckPath::NoteTypeFieldMessagePatternItemFormat {
                        note_type_id: note_type_id.clone(),
                        field_id: field.id.clone(),
                    }
                    .to_string(),
                    &[&note_type_variables, &deck_variables],
                    &mut errors,
                );
                render_string_with_variables(
                    &mut pattern.separator,
                    &DeckPath::NoteTypeFieldMessagePatternSeparator {
                        note_type_id: note_type_id.clone(),
                        field_id: field.id.clone(),
                    }
                    .to_string(),
                    &[&note_type_variables, &deck_variables],
                    &mut errors,
                );
            }
        }
        for template in &mut note_type.card_templates {
            if tombstones
                .blocking(&TombstoneAddress::CardTemplate {
                    note_type_id: note_type_id.clone(),
                    template_id: template.id.clone(),
                })
                .is_some()
            {
                continue;
            }
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
        if tombstones
            .blocking(&TombstoneAddress::Note {
                note_id: note_id.clone(),
            })
            .is_some()
        {
            continue;
        }
        let note_variables = note.variables.clone();
        let note_type_variables = rendered
            .note_types
            .get(&note.note_type_id)
            .map(|note_type| &note_type.variables);
        for (field_id, value) in &mut note.fields {
            if tombstones
                .blocking(&TombstoneAddress::NoteField {
                    note_id: note_id.clone(),
                    field_id: field_id.clone(),
                })
                .is_some()
            {
                continue;
            }
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
                FieldValue::MessageItems(message) => {
                    for (index, item) in message.items.iter_mut().enumerate() {
                        for (parameter, argument) in item {
                            render_string_with_variables(
                                argument.as_str_mut(),
                                &DeckPath::NoteFieldMessageItemParameter {
                                    note_id: note_id.clone(),
                                    field_id: field_id.clone(),
                                    index,
                                    parameter: parameter.clone(),
                                }
                                .to_string(),
                                &scopes,
                                &mut errors,
                            );
                        }
                    }
                    if let Some(separator) = &mut message.separator_override {
                        render_string_with_variables(
                            separator,
                            &DeckPath::NoteFieldMessageSeparator {
                                note_id: note_id.clone(),
                                field_id: field_id.clone(),
                            }
                            .to_string(),
                            &scopes,
                            &mut errors,
                        );
                    }
                }
                FieldValue::Images(_) => {}
            }
        }
    }

    if !errors.is_empty() {
        return Err(VariableRenderReport {
            errors,
            validation_errors: Vec::new(),
        });
    }

    match rendered.resolve_field_graph(|note_id, field_id, images| {
        lower_images_from_deck(&rendered, note_id, field_id, images)
    }) {
        Ok(graph) => {
            for (path, value) in graph.values {
                let DeckPath::NoteField { note_id, field_id } = path
                    .parse()
                    .expect("planned graph values always have canonical note-field paths")
                else {
                    unreachable!("planned graph values are note fields")
                };
                if let Some(note) = rendered.notes.get_mut(&note_id) {
                    note.fields.insert(field_id, FieldValue::Scalar(value));
                }
            }
            Ok(rendered)
        }
        Err(report) => Err(VariableRenderReport {
            errors,
            validation_errors: report
                .errors
                .into_iter()
                .map(ValidationError::from_field_graph)
                .collect(),
        }),
    }
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
