use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write as _};

use brain_brew_core::{
    AdapterIdChange, AdapterIds, CanonicalDeck, CardTemplate, CardTemplateChange, ChangeIntent,
    DeckChange, DeckPath, EntityFingerprint, ExpectedBase, FieldChange, FieldDefinition,
    FieldDefinitionChange, FieldImageReference, FieldValue, InvalidStableId, ListMessageArgument,
    ListMessageItems, ListMessageParameter, ListMessagePattern, MediaChange, MediaReference,
    MessageComponent, Note, NoteChange, NoteType, NoteTypeChange, Overlay, OverlayKind,
    PropertyChange, RemovalProvenance, StableId, StaleTranslation, StructuredMessage, TagChange,
    TargetAdaptation, TargetAdaptationIntent, TargetAdaptationOwnership, TombstoneAddress,
    TombstoneRecord, Tombstones, TranslationDictionary, ValidationReport,
};
use serde::{Deserialize, Deserializer};
use serde_yaml::Value;

use crate::yaml_scalar::{
    is_emittable_key as is_emittable_yaml_key, key as yaml_key, scalar as yaml_scalar,
    write_multiline_or_scalar,
};

/// An actionable notice emitted when a legacy overlay is read compatibly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OverlayMigrationDiagnostic {
    pub path: String,
    pub message: String,
}

/// Identify legacy target-adaptation forms accepted by the compatibility reader.
///
/// Run `brainbrew fmt <overlay.yaml>` to emit the typed canonical form.
pub fn overlay_migration_diagnostics(
    input: &str,
) -> Result<Vec<OverlayMigrationDiagnostic>, CanonicalYamlError> {
    overlay_from_str(input)?;
    let value: Value = serde_yaml::from_str(input).map_err(CanonicalYamlError::Parse)?;
    let Some(root) = value.as_mapping() else {
        return Ok(Vec::new());
    };
    let mut diagnostics = Vec::new();
    let translations = yaml_mapping_value(root, "translations");
    if let Some(translations) = translations.and_then(Value::as_mapping) {
        if let Some(additions) =
            yaml_mapping_value(translations, "target_additions").and_then(Value::as_mapping)
        {
            for path in additions.keys().filter_map(Value::as_str) {
                diagnostics.push(OverlayMigrationDiagnostic {
                    path: format!("translations.target_additions.{path}"),
                    message: "legacy target_additions is accepted for migration; run brainbrew fmt to emit target_adaptations with intent, ownership, expected_source, and reason".to_owned(),
                });
            }
        }
        collect_legacy_target_adaptation_diagnostics(
            &mut diagnostics,
            translations,
            "translations.target_adaptations",
        );
    }
    collect_legacy_target_adaptation_diagnostics(&mut diagnostics, root, "target_adaptations");
    Ok(diagnostics)
}

fn yaml_mapping_value<'a>(mapping: &'a serde_yaml::Mapping, key: &str) -> Option<&'a Value> {
    mapping.get(Value::String(key.to_owned()))
}

fn collect_legacy_target_adaptation_diagnostics(
    diagnostics: &mut Vec<OverlayMigrationDiagnostic>,
    parent: &serde_yaml::Mapping,
    section: &str,
) {
    let Some(adaptations) =
        yaml_mapping_value(parent, section.rsplit('.').next().unwrap_or(section))
            .and_then(Value::as_mapping)
    else {
        return;
    };
    for (path, value) in adaptations {
        let Some(path) = path.as_str() else {
            continue;
        };
        let Some(adaptation) = value.as_mapping() else {
            continue;
        };
        if yaml_mapping_value(adaptation, "intent").is_none()
            && yaml_mapping_value(adaptation, "ownership").is_none()
        {
            diagnostics.push(OverlayMigrationDiagnostic {
                path: format!("{section}.{path}"),
                message: "legacy target adaptation is accepted for migration; run brainbrew fmt to add typed intent, ownership, and a reviewable reason".to_owned(),
            });
        }
    }
}

/// Parse a CanonicalDeck from strict canonical YAML.
pub fn from_str(input: &str) -> Result<CanonicalDeck, CanonicalYamlError> {
    crate::strict_yaml::reject_duplicate_keys(input).map_err(CanonicalYamlError::Parse)?;
    validate_canonical_field_value_unions(input)?;
    let file: CanonicalDeckYaml = serde_yaml::from_str(input).map_err(CanonicalYamlError::Parse)?;
    crate::strict_yaml::reject_unintended_scalars(
        input,
        crate::strict_yaml::ScalarPolicy::CanonicalDeck,
    )
    .map_err(CanonicalYamlError::Parse)?;
    let deck = file.into_deck()?;
    deck.validate().map_err(CanonicalYamlError::Validation)?;
    validate_deck_yaml_keys(&deck)?;
    Ok(deck)
}

/// Parse a sparse overlay YAML file.
pub fn overlay_from_str(input: &str) -> Result<Overlay, CanonicalYamlError> {
    crate::strict_yaml::reject_duplicate_keys(input).map_err(CanonicalYamlError::Parse)?;
    validate_overlay_unions(input)?;
    let file: OverlayYaml = serde_yaml::from_str(input).map_err(CanonicalYamlError::Parse)?;
    crate::strict_yaml::reject_unintended_scalars(input, crate::strict_yaml::ScalarPolicy::Overlay)
        .map_err(CanonicalYamlError::Parse)?;
    let overlay = file.into_overlay()?;
    validate_overlay_yaml_keys(&overlay)?;
    validate_translation_dictionary_invariants(&overlay)?;
    Ok(overlay)
}

/// Parse and re-emit a CanonicalDeck YAML file using deterministic formatting.
pub fn format_str(input: &str) -> Result<String, CanonicalYamlError> {
    let deck = from_str(input)?;
    to_string(&deck)
}

/// Parse and re-emit a sparse overlay YAML file using deterministic formatting.
pub fn overlay_format_str(input: &str) -> Result<String, CanonicalYamlError> {
    let overlay = overlay_from_str(input)?;
    overlay_to_string(&overlay)
}

/// Emit a CanonicalDeck as deterministic canonical YAML.
pub fn to_string(deck: &CanonicalDeck) -> Result<String, CanonicalYamlError> {
    deck.validate().map_err(CanonicalYamlError::Validation)?;
    validate_deck_yaml_keys(deck)?;

    let mut out = String::new();
    writeln!(out, "deck:").expect("writing to a string cannot fail");
    writeln!(out, "  id: {}", deck.id).expect("writing to a string cannot fail");
    writeln!(out, "  name: {}", yaml_scalar(&deck.name)).expect("writing to a string cannot fail");
    write_multiline_or_scalar(&mut out, "  ", "description", &deck.description);
    write_variables(&mut out, "  ", &deck.variables);
    write_adapter_ids(&mut out, "  ", &deck.adapter_ids);

    writeln!(out, "note_types:").expect("writing to a string cannot fail");
    for (id, note_type) in &deck.note_types {
        writeln!(out, "  {}:", emitted_key(id.as_str())).expect("writing to a string cannot fail");
        writeln!(out, "    name: {}", yaml_scalar(&note_type.name))
            .expect("writing to a string cannot fail");
        write_variables(&mut out, "    ", &note_type.variables);
        writeln!(out, "    field_order:").expect("writing to a string cannot fail");
        for field in &note_type.fields {
            writeln!(out, "      - {}", field.id).expect("writing to a string cannot fail");
        }
        writeln!(out, "    fields:").expect("writing to a string cannot fail");
        let fields_by_id = note_type
            .fields
            .iter()
            .map(|field| (&field.id, field))
            .collect::<BTreeMap<_, _>>();
        for (field_id, field) in fields_by_id {
            writeln!(out, "      {}:", emitted_key(field_id.as_str()))
                .expect("writing to a string cannot fail");
            writeln!(out, "        name: {}", yaml_scalar(&field.name))
                .expect("writing to a string cannot fail");
            if let Some(pattern) = &field.message_pattern {
                write_list_message_pattern(&mut out, "        ", pattern);
            }
        }
        if note_type.card_templates.is_empty() {
            writeln!(out, "    card_template_order: []").expect("writing to a string cannot fail");
            writeln!(out, "    card_templates: {{}}").expect("writing to a string cannot fail");
        } else {
            writeln!(out, "    card_template_order:").expect("writing to a string cannot fail");
            for template in &note_type.card_templates {
                writeln!(out, "      - {}", template.id).expect("writing to a string cannot fail");
            }
            writeln!(out, "    card_templates:").expect("writing to a string cannot fail");
        }
        let templates_by_id = note_type
            .card_templates
            .iter()
            .map(|template| (&template.id, template))
            .collect::<BTreeMap<_, _>>();
        for (template_id, template) in templates_by_id {
            writeln!(out, "      {}:", emitted_key(template_id.as_str()))
                .expect("writing to a string cannot fail");
            writeln!(out, "        name: {}", yaml_scalar(&template.name))
                .expect("writing to a string cannot fail");
            write_variables(&mut out, "        ", &template.variables);
            write_multiline_or_scalar(
                &mut out,
                "        ",
                "question_format",
                &template.question_format,
            );
            write_multiline_or_scalar(
                &mut out,
                "        ",
                "answer_format",
                &template.answer_format,
            );
            write_adapter_ids(&mut out, "        ", &template.adapter_ids);
        }
        write_multiline_or_scalar(&mut out, "    ", "styling", &note_type.styling);
        write_adapter_ids(&mut out, "    ", &note_type.adapter_ids);
    }

    if deck.notes.is_empty() {
        writeln!(out, "notes: {{}}").expect("writing to a string cannot fail");
    } else {
        writeln!(out, "notes:").expect("writing to a string cannot fail");
        for (id, note) in &deck.notes {
            writeln!(out, "  {}:", emitted_key(id.as_str()))
                .expect("writing to a string cannot fail");
            writeln!(out, "    note_type_id: {}", note.note_type_id)
                .expect("writing to a string cannot fail");
            write_variables(&mut out, "    ", &note.variables);
            writeln!(out, "    fields:").expect("writing to a string cannot fail");
            for (field_id, value) in &note.fields {
                match value {
                    FieldValue::Scalar(value) => writeln!(
                        out,
                        "      {}: {}",
                        emitted_key(field_id.as_str()),
                        yaml_scalar(value)
                    )
                    .expect("writing to a string cannot fail"),
                    FieldValue::Message(message) => {
                        write_structured_message_field(&mut out, "      ", field_id, message);
                    }
                    FieldValue::MessageItems(message) => {
                        write_list_message_field(&mut out, "      ", field_id, message);
                    }
                    FieldValue::Images(images) => {
                        write_image_field_value(&mut out, "      ", field_id, images);
                    }
                }
            }
            if note.tags.is_empty() {
                writeln!(out, "    tags: []").expect("writing to a string cannot fail");
            } else {
                writeln!(out, "    tags:").expect("writing to a string cannot fail");
                for tag in &note.tags {
                    writeln!(out, "      - {}", yaml_scalar(tag))
                        .expect("writing to a string cannot fail");
                }
            }
            write_adapter_ids(&mut out, "    ", &note.adapter_ids);
        }
    }

    if deck.media.is_empty() {
        writeln!(out, "media: {{}}").expect("writing to a string cannot fail");
    } else {
        writeln!(out, "media:").expect("writing to a string cannot fail");
        for (id, media) in &deck.media {
            writeln!(out, "  {}:", emitted_key(id.as_str()))
                .expect("writing to a string cannot fail");
            writeln!(out, "    path: {}", yaml_scalar(&media.path))
                .expect("writing to a string cannot fail");
            writeln!(out, "    sha256: {}", yaml_scalar(&media.sha256))
                .expect("writing to a string cannot fail");
        }
    }

    if deck.tombstones.is_empty() {
        writeln!(out, "tombstones: []").expect("writing to a string cannot fail");
    } else {
        writeln!(out, "tombstones:").expect("writing to a string cannot fail");
        for tombstone in deck.tombstones.iter() {
            writeln!(out, "  - kind: {}", tombstone.address.kind())
                .expect("writing to a string cannot fail");
            writeln!(out, "    path: {}", tombstone.address)
                .expect("writing to a string cannot fail");
            if let Some(provenance) = &tombstone.provenance {
                writeln!(out, "    removed_by: {}", provenance.overlay_id)
                    .expect("writing to a string cannot fail");
                writeln!(
                    out,
                    "    operation: {}",
                    change_intent_name(provenance.operation)
                )
                .expect("writing to a string cannot fail");
            }
        }
    }

    Ok(out)
}

/// Emit a sparse overlay as deterministic YAML.
pub fn overlay_to_string(overlay: &Overlay) -> Result<String, CanonicalYamlError> {
    validate_translation_dictionary_invariants(overlay)?;
    validate_overlay_yaml_keys(overlay)?;
    validate_overlay_representations(overlay)?;
    let mut out = String::new();
    writeln!(out, "id: {}", overlay.id).expect("writing to a string cannot fail");
    writeln!(out, "kind: {}", overlay_kind_name(overlay.kind))
        .expect("writing to a string cannot fail");

    let (field_additions, note_type_changes, note_changes) =
        split_field_additions_for_format(overlay);
    let (field_fills, note_changes) = if overlay.kind == OverlayKind::Translation {
        (BTreeMap::new(), note_changes)
    } else {
        split_field_fills_for_format(note_changes)
    };

    if let Some(translations) = &overlay.translations {
        let has_top_level_translation_data = !translations.target_adaptations.is_empty()
            || !translations.stale_translations.is_empty();
        if !translation_dictionary_is_empty(translations) || !has_top_level_translation_data {
            write_translation_dictionary(&mut out, translations);
        }
        write_target_adaptations(&mut out, &translations.target_adaptations);
        write_stale_translations(&mut out, &translations.stale_translations);
    }

    if !field_additions.is_empty() {
        write_field_additions(&mut out, &field_additions);
    }

    if !field_fills.is_empty() {
        write_field_fills(&mut out, &field_fills);
    }

    if let Some(deck_change) = &overlay.deck_change {
        writeln!(out, "deck:").expect("writing to a string cannot fail");
        if let Some(change) = &deck_change.name {
            write_property_change(&mut out, "  ", "name", change);
        }
        if let Some(change) = &deck_change.description {
            write_property_change(&mut out, "  ", "description", change);
        }
        write_property_changes(&mut out, "  ", "variables", &deck_change.variables);
        write_adapter_id_changes(&mut out, "  ", &deck_change.adapter_ids);
    }

    if !note_type_changes.is_empty() {
        writeln!(out, "note_types:").expect("writing to a string cannot fail");
        for (id, change) in &note_type_changes {
            writeln!(out, "  {}:", emitted_key(id.as_str()))
                .expect("writing to a string cannot fail");
            writeln!(out, "    intent: {}", change_intent_name(change.intent))
                .expect("writing to a string cannot fail");
            if let Some(expected_base) = &change.expected_base {
                write_expected_base(&mut out, "    ", expected_base);
            }
            if let Some(note_type) = &change.note_type {
                writeln!(out, "    note_type:").expect("writing to a string cannot fail");
                write_note_type_payload(&mut out, "      ", note_type);
            }
            if let Some(name) = &change.name {
                write_property_change(&mut out, "    ", "name", name);
            }
            write_property_changes(&mut out, "    ", "variables", &change.variables);
            if let Some(styling) = &change.styling {
                write_property_change(&mut out, "    ", "styling", styling);
            }
            if !change.fields.is_empty() {
                writeln!(out, "    fields:").expect("writing to a string cannot fail");
                for (field_id, field_change) in &change.fields {
                    writeln!(out, "      {}:", emitted_key(field_id.as_str()))
                        .expect("writing to a string cannot fail");
                    writeln!(
                        out,
                        "        intent: {}",
                        change_intent_name(field_change.intent)
                    )
                    .expect("writing to a string cannot fail");
                    if let Some(field) = &field_change.field {
                        writeln!(out, "        name: {}", yaml_scalar(&field.name))
                            .expect("writing to a string cannot fail");
                        if let Some(pattern) = &field.message_pattern {
                            write_list_message_pattern(&mut out, "        ", pattern);
                        }
                    }
                    if let Some(expected_base) = &field_change.expected_base {
                        write_expected_base(&mut out, "        ", expected_base);
                    }
                }
            }
            if !change.card_templates.is_empty() {
                writeln!(out, "    card_templates:").expect("writing to a string cannot fail");
                for (template_id, template_change) in &change.card_templates {
                    writeln!(out, "      {}:", emitted_key(template_id.as_str()))
                        .expect("writing to a string cannot fail");
                    writeln!(
                        out,
                        "        intent: {}",
                        change_intent_name(template_change.intent)
                    )
                    .expect("writing to a string cannot fail");
                    if let Some(expected_base) = &template_change.expected_base {
                        write_expected_base(&mut out, "        ", expected_base);
                    }
                    if let Some(insert_after) = &template_change.insert_after {
                        writeln!(out, "        insert_after: {insert_after}")
                            .expect("writing to a string cannot fail");
                    }
                    if let Some(template) = &template_change.template {
                        writeln!(out, "        template:")
                            .expect("writing to a string cannot fail");
                        write_card_template_payload(&mut out, "          ", template);
                    }
                    if let Some(name) = &template_change.name {
                        write_property_change(&mut out, "        ", "name", name);
                    }
                    write_property_changes(
                        &mut out,
                        "        ",
                        "variables",
                        &template_change.variables,
                    );
                    if let Some(question_format) = &template_change.question_format {
                        write_property_change(
                            &mut out,
                            "        ",
                            "question_format",
                            question_format,
                        );
                    }
                    if let Some(answer_format) = &template_change.answer_format {
                        write_property_change(&mut out, "        ", "answer_format", answer_format);
                    }
                    write_adapter_id_changes(&mut out, "        ", &template_change.adapter_ids);
                }
            }
            write_adapter_id_changes(&mut out, "    ", &change.adapter_ids);
        }
    }

    if !note_changes.is_empty() {
        writeln!(out, "notes:").expect("writing to a string cannot fail");
        for (id, change) in &note_changes {
            writeln!(out, "  {}:", emitted_key(id.as_str()))
                .expect("writing to a string cannot fail");
            writeln!(out, "    intent: {}", change_intent_name(change.intent))
                .expect("writing to a string cannot fail");
            if let Some(expected_base) = &change.expected_base {
                write_expected_base(&mut out, "    ", expected_base);
            }
            if let Some(note) = &change.note {
                writeln!(out, "    note:").expect("writing to a string cannot fail");
                write_note_payload(&mut out, "      ", note);
            }
            write_property_changes(&mut out, "    ", "variables", &change.variables);
            if !change.fields.is_empty() {
                writeln!(out, "    fields:").expect("writing to a string cannot fail");
                for (field_id, field_change) in &change.fields {
                    write_field_change(&mut out, "      ", field_id, field_change);
                }
            }
            if !change.tags.is_empty() {
                writeln!(out, "    tags:").expect("writing to a string cannot fail");
                for (tag, tag_change) in &change.tags {
                    writeln!(out, "      {}:", emitted_key(tag))
                        .expect("writing to a string cannot fail");
                    writeln!(
                        out,
                        "        intent: {}",
                        change_intent_name(tag_change.intent)
                    )
                    .expect("writing to a string cannot fail");
                    if let Some(expected_base) = &tag_change.expected_base {
                        write_expected_base(&mut out, "        ", expected_base);
                    }
                }
            }
            write_adapter_id_changes(&mut out, "    ", &change.adapter_ids);
        }
    }

    if !overlay.media_changes.is_empty() {
        writeln!(out, "media:").expect("writing to a string cannot fail");
        for (id, change) in &overlay.media_changes {
            writeln!(out, "  {}:", emitted_key(id.as_str()))
                .expect("writing to a string cannot fail");
            writeln!(out, "    intent: {}", change_intent_name(change.intent))
                .expect("writing to a string cannot fail");
            if let Some(media) = &change.media {
                writeln!(out, "    path: {}", yaml_scalar(&media.path))
                    .expect("writing to a string cannot fail");
                writeln!(out, "    sha256: {}", yaml_scalar(&media.sha256))
                    .expect("writing to a string cannot fail");
            }
            if let Some(expected_base) = &change.expected_base {
                write_expected_base(&mut out, "    ", expected_base);
            }
        }
    }

    Ok(out)
}

#[derive(Default)]
struct FieldAdditionsForFormat {
    fields: BTreeMap<StableId, String>,
    values: BTreeMap<StableId, BTreeMap<StableId, FieldValueForFormat>>,
}

type FieldValueForFormat = FieldValue;

fn split_field_additions_for_format(
    overlay: &Overlay,
) -> (
    BTreeMap<StableId, FieldAdditionsForFormat>,
    BTreeMap<StableId, NoteTypeChange>,
    BTreeMap<StableId, NoteChange>,
) {
    let mut field_additions = BTreeMap::<StableId, FieldAdditionsForFormat>::new();
    let mut note_type_changes = overlay.note_type_changes.clone();
    let mut note_changes = overlay.note_changes.clone();
    let mut field_to_note_type = BTreeMap::<StableId, StableId>::new();
    let mut ambiguous_fields = BTreeSet::<StableId>::new();

    for (note_type_id, change) in &overlay.note_type_changes {
        if change.intent != ChangeIntent::Merge {
            continue;
        }
        for (field_id, field_change) in &change.fields {
            if field_change.intent == ChangeIntent::Add
                && field_change.expected_base.is_none()
                && let Some(field) = &field_change.field
                && field.message_pattern.is_none()
            {
                field_additions
                    .entry(note_type_id.clone())
                    .or_default()
                    .fields
                    .insert(field_id.clone(), field.name.clone());
                if field_to_note_type
                    .insert(field_id.clone(), note_type_id.clone())
                    .is_some()
                {
                    ambiguous_fields.insert(field_id.clone());
                }
            }
        }
    }

    for field_id in ambiguous_fields {
        field_to_note_type.remove(&field_id);
    }

    for (note_id, change) in &overlay.note_changes {
        if change.intent != ChangeIntent::Merge {
            continue;
        }
        for (field_id, field_change) in &change.fields {
            if field_change.intent == ChangeIntent::Add && field_change.expected_base.is_none() {
                let Some(value) = field_addition_format_value(field_change) else {
                    continue;
                };
                let Some(note_type_id) = field_to_note_type.get(field_id) else {
                    continue;
                };
                field_additions
                    .entry(note_type_id.clone())
                    .or_default()
                    .values
                    .entry(note_id.clone())
                    .or_default()
                    .insert(field_id.clone(), value);
            }
        }
    }

    for (note_type_id, additions) in &field_additions {
        let Some(change) = note_type_changes.get_mut(note_type_id) else {
            continue;
        };
        for field_id in additions.fields.keys() {
            change.fields.remove(field_id);
        }
    }
    note_type_changes.retain(|_, change| !is_empty_note_type_merge_change(change));

    for additions in field_additions.values() {
        for (note_id, values) in &additions.values {
            let Some(change) = note_changes.get_mut(note_id) else {
                continue;
            };
            for field_id in values.keys() {
                change.fields.remove(field_id);
            }
        }
    }
    note_changes.retain(|_, change| !is_empty_note_merge_change(change));

    field_additions.retain(|_, additions| !additions.fields.is_empty());
    (field_additions, note_type_changes, note_changes)
}

fn split_field_fills_for_format(
    mut note_changes: BTreeMap<StableId, NoteChange>,
) -> (
    BTreeMap<StableId, BTreeMap<StableId, FieldValueForFormat>>,
    BTreeMap<StableId, NoteChange>,
) {
    let mut field_fills = BTreeMap::<StableId, BTreeMap<StableId, FieldValueForFormat>>::new();

    for (note_id, change) in &note_changes {
        if change.intent != ChangeIntent::Merge {
            continue;
        }
        for (field_id, field_change) in &change.fields {
            if let Some(value) = field_fill_value(field_change) {
                field_fills
                    .entry(note_id.clone())
                    .or_default()
                    .insert(field_id.clone(), value);
            }
        }
    }

    for (note_id, fields) in &field_fills {
        let Some(change) = note_changes.get_mut(note_id) else {
            continue;
        };
        for field_id in fields.keys() {
            change.fields.remove(field_id);
        }
    }
    note_changes.retain(|_, change| !is_empty_note_merge_change(change));

    (field_fills, note_changes)
}

fn field_fill_value(change: &FieldChange) -> Option<FieldValueForFormat> {
    if change.intent == ChangeIntent::Replace
        && matches!(change.expected_base, Some(ExpectedBase::Value(ref value)) if value.is_empty())
    {
        field_change_format_value(change)
    } else {
        None
    }
}

fn field_addition_format_value(change: &FieldChange) -> Option<FieldValueForFormat> {
    match change.value.as_ref()? {
        FieldValue::Scalar(value) => Some(FieldValue::Scalar(value.clone())),
        FieldValue::Images(images) => Some(FieldValue::Images(images.clone())),
        FieldValue::Message(_) => None,
        FieldValue::MessageItems(message) => Some(FieldValue::MessageItems(message.clone())),
    }
}

fn field_change_format_value(change: &FieldChange) -> Option<FieldValueForFormat> {
    change.value.clone()
}

fn is_empty_note_type_merge_change(change: &NoteTypeChange) -> bool {
    change.intent == ChangeIntent::Merge
        && change.note_type.is_none()
        && change.name.is_none()
        && change.variables.is_empty()
        && change.styling.is_none()
        && change.fields.is_empty()
        && change.card_templates.is_empty()
        && change.adapter_ids.is_empty()
        && change.expected_base.is_none()
}

fn is_empty_note_merge_change(change: &NoteChange) -> bool {
    change.intent == ChangeIntent::Merge
        && change.note.is_none()
        && change.variables.is_empty()
        && change.fields.is_empty()
        && change.tags.is_empty()
        && change.adapter_ids.is_empty()
        && change.expected_base.is_none()
}

fn write_field_additions(
    out: &mut String,
    field_additions: &BTreeMap<StableId, FieldAdditionsForFormat>,
) {
    writeln!(out, "field_additions:").expect("writing to a string cannot fail");
    for (note_type_id, additions) in field_additions {
        writeln!(out, "  {}:", emitted_key(note_type_id.as_str()))
            .expect("writing to a string cannot fail");
        writeln!(out, "    fields:").expect("writing to a string cannot fail");
        for (field_id, name) in &additions.fields {
            writeln!(
                out,
                "      {}: {}",
                emitted_key(field_id.as_str()),
                yaml_scalar(name)
            )
            .expect("writing to a string cannot fail");
        }
        if !additions.values.is_empty() {
            writeln!(out, "    values:").expect("writing to a string cannot fail");
            for (note_id, values) in &additions.values {
                writeln!(out, "      {}:", emitted_key(note_id.as_str()))
                    .expect("writing to a string cannot fail");
                for (field_id, value) in values {
                    write_field_value_for_format(out, "        ", field_id, value);
                }
            }
        }
    }
}

fn write_field_fills(
    out: &mut String,
    field_fills: &BTreeMap<StableId, BTreeMap<StableId, FieldValueForFormat>>,
) {
    writeln!(out, "field_fills:").expect("writing to a string cannot fail");
    for (note_id, fields) in field_fills {
        writeln!(out, "  {}:", emitted_key(note_id.as_str()))
            .expect("writing to a string cannot fail");
        for (field_id, value) in fields {
            write_field_value_for_format(out, "    ", field_id, value);
        }
    }
}

fn write_field_value_for_format(
    out: &mut String,
    indent: &str,
    field_id: &StableId,
    value: &FieldValueForFormat,
) {
    match value {
        FieldValueForFormat::Scalar(value) => {
            writeln!(
                out,
                "{indent}{}: {}",
                emitted_key(field_id.as_str()),
                yaml_scalar(value)
            )
            .expect("writing to a string cannot fail");
        }
        FieldValueForFormat::Message(message) => {
            write_structured_message_field(out, indent, field_id, message);
        }
        FieldValueForFormat::MessageItems(message) => {
            write_list_message_field(out, indent, field_id, message);
        }
        FieldValueForFormat::Images(images) => {
            write_image_field_value(out, indent, field_id, images);
        }
    }
}

fn validate_translation_dictionary_invariants(overlay: &Overlay) -> Result<(), CanonicalYamlError> {
    if let Some(translations) = &overlay.translations {
        translations
            .validate_mutation_invariants()
            .map_err(|error| CanonicalYamlError::InvalidTranslationDictionary(error.to_string()))?;
    }
    Ok(())
}

fn write_translation_dictionary(out: &mut String, translations: &TranslationDictionary) {
    if translation_dictionary_is_empty(translations) {
        writeln!(out, "translations: {{}}").expect("writing to a string cannot fail");
        return;
    }

    writeln!(out, "translations:").expect("writing to a string cannot fail");
    if translations.require_complete {
        writeln!(out, "  require_complete: true").expect("writing to a string cannot fail");
    }
    if !translations.ignore_paths.is_empty() {
        writeln!(out, "  ignore_paths:").expect("writing to a string cannot fail");
        for path in &translations.ignore_paths {
            writeln!(out, "    - {}", yaml_scalar(path)).expect("writing to a string cannot fail");
        }
    }
    if !translations.direct.is_empty() {
        writeln!(out, "  direct:").expect("writing to a string cannot fail");
        for (source, translated) in &translations.direct {
            writeln!(
                out,
                "    {}: {}",
                yaml_scalar(source),
                yaml_scalar(translated)
            )
            .expect("writing to a string cannot fail");
        }
    }
    if !translations.contextual.is_empty() {
        writeln!(out, "  contextual:").expect("writing to a string cannot fail");
        let nodes = contextual_format_tree(&translations.contextual);
        write_contextual_nodes(out, "    ", &nodes);
    }
    if !translations.no_change.is_empty() {
        write_no_change(out, &translations.no_change);
    }
    if !translations.variables.is_empty() {
        writeln!(out, "  variables:").expect("writing to a string cannot fail");
        for (variable_key, replacements) in &translations.variables {
            writeln!(out, "    {}:", emitted_key(variable_key))
                .expect("writing to a string cannot fail");
            for (source, translated) in replacements {
                writeln!(
                    out,
                    "      {}: {}",
                    yaml_scalar(source),
                    yaml_scalar(translated)
                )
                .expect("writing to a string cannot fail");
            }
        }
    }
    if !translations.adapter_ids.is_empty() {
        writeln!(out, "  adapter_ids:").expect("writing to a string cannot fail");
        for (adapter_key, replacements) in &translations.adapter_ids {
            writeln!(out, "    {}:", emitted_key(adapter_key))
                .expect("writing to a string cannot fail");
            for (source, translated) in replacements {
                writeln!(
                    out,
                    "      {}: {}",
                    yaml_scalar(source),
                    yaml_scalar(translated)
                )
                .expect("writing to a string cannot fail");
            }
        }
    }
}

fn write_target_adaptations(
    out: &mut String,
    target_adaptations: &BTreeMap<String, TargetAdaptation>,
) {
    if target_adaptations.is_empty() {
        return;
    }
    writeln!(out, "target_adaptations:").expect("writing to a string cannot fail");
    for (path, adaptation) in target_adaptations {
        writeln!(out, "  {}:", emitted_key(path)).expect("writing to a string cannot fail");
        writeln!(
            out,
            "    intent: {}",
            target_adaptation_intent_name(adaptation.intent)
        )
        .expect("writing to a string cannot fail");
        writeln!(
            out,
            "    ownership: {}",
            target_adaptation_ownership_name(adaptation.ownership)
        )
        .expect("writing to a string cannot fail");
        writeln!(
            out,
            "    expected_source: {}",
            yaml_scalar(&adaptation.expected_source)
        )
        .expect("writing to a string cannot fail");
        if !adaptation.is_deletion() {
            writeln!(out, "    target: {}", yaml_scalar(&adaptation.target))
                .expect("writing to a string cannot fail");
        }
        writeln!(out, "    reason: {}", yaml_scalar(&adaptation.reason))
            .expect("writing to a string cannot fail");
    }
}

fn write_stale_translations(out: &mut String, stale_translations: &[StaleTranslation]) {
    if stale_translations.is_empty() {
        return;
    }
    writeln!(out, "stale_translations:").expect("writing to a string cannot fail");
    let mut records = stale_translations.to_vec();
    records.sort_by(|left, right| {
        left.context
            .cmp(&right.context)
            .then_with(|| left.new_source.cmp(&right.new_source))
            .then_with(|| left.old_source.cmp(&right.old_source))
            .then_with(|| left.target.cmp(&right.target))
    });
    for record in &records {
        writeln!(out, "  - old_source: {}", yaml_scalar(&record.old_source))
            .expect("writing to a string cannot fail");
        writeln!(out, "    new_source: {}", yaml_scalar(&record.new_source))
            .expect("writing to a string cannot fail");
        writeln!(out, "    target: {}", yaml_scalar(&record.target))
            .expect("writing to a string cannot fail");
        if let Some(context) = &record.context {
            writeln!(out, "    context: {}", yaml_scalar(context))
                .expect("writing to a string cannot fail");
        }
    }
}

fn translation_dictionary_is_empty(translations: &TranslationDictionary) -> bool {
    !translations.require_complete
        && translations.ignore_paths.is_empty()
        && translations.direct.is_empty()
        && translations.contextual.is_empty()
        && translations.no_change.is_empty()
        && translations.variables.is_empty()
        && translations.adapter_ids.is_empty()
}

fn emitted_key(value: &str) -> String {
    yaml_key(value).expect("emitted YAML key was not prevalidated")
}

#[derive(Default)]
struct ContextualFormatNode {
    translations: BTreeMap<String, String>,
    children: BTreeMap<String, ContextualFormatNode>,
}

fn contextual_format_tree(
    contextual: &BTreeMap<String, BTreeMap<String, String>>,
) -> BTreeMap<String, ContextualFormatNode> {
    let mut nodes = BTreeMap::new();
    for (context_path, replacements) in contextual {
        match context_path.parse::<DeckPath>() {
            Ok(DeckPath::NoteTypeFieldMessagePatternItemFormat {
                note_type_id,
                field_id,
            }) => {
                insert_contextual_format_node(
                    &mut nodes,
                    &DeckPath::NoteTypeFieldMessagePattern {
                        note_type_id,
                        field_id,
                    }
                    .to_string(),
                    "item_format",
                    replacements,
                );
                continue;
            }
            Ok(DeckPath::NoteTypeFieldMessagePatternSeparator {
                note_type_id,
                field_id,
            }) => {
                insert_contextual_format_node(
                    &mut nodes,
                    &DeckPath::NoteTypeFieldMessagePattern {
                        note_type_id,
                        field_id,
                    }
                    .to_string(),
                    "separator",
                    replacements,
                );
                continue;
            }
            Ok(DeckPath::NoteFieldMessageSeparator { note_id, field_id }) => {
                insert_contextual_format_node(
                    &mut nodes,
                    &DeckPath::NoteFieldMessage { note_id, field_id }.to_string(),
                    "separator",
                    replacements,
                );
                continue;
            }
            _ => {}
        }
        if let Some(suffix) = context_path.strip_prefix("notes.note.") {
            insert_contextual_format_node(&mut nodes, "notes.note", suffix, replacements);
        } else if let Some(suffix) = context_path.strip_prefix("note_types.note-type.") {
            insert_contextual_format_node(&mut nodes, "note_types.note-type", suffix, replacements);
        } else {
            nodes
                .entry(context_path.clone())
                .or_default()
                .translations
                .extend(replacements.clone());
        }
    }
    nodes
}

fn insert_contextual_format_node(
    nodes: &mut BTreeMap<String, ContextualFormatNode>,
    group: &str,
    suffix: &str,
    replacements: &BTreeMap<String, String>,
) {
    nodes
        .entry(group.to_owned())
        .or_default()
        .children
        .entry(suffix.to_owned())
        .or_default()
        .translations
        .extend(replacements.clone());
}

fn write_contextual_nodes(
    out: &mut String,
    indent: &str,
    nodes: &BTreeMap<String, ContextualFormatNode>,
) {
    for (key, node) in nodes {
        writeln!(out, "{indent}{}:", emitted_key(key)).expect("writing to a string cannot fail");
        let child_indent = format!("{indent}  ");
        for (source, translated) in &node.translations {
            writeln!(
                out,
                "{}{}: {}",
                child_indent,
                yaml_scalar(source),
                yaml_scalar(translated)
            )
            .expect("writing to a string cannot fail");
        }
        write_contextual_nodes(out, &child_indent, &node.children);
    }
}

fn write_no_change(out: &mut String, no_change: &BTreeSet<String>) {
    writeln!(out, "  no_change:").expect("writing to a string cannot fail");
    for source in no_change {
        writeln!(out, "    - {}", yaml_scalar(source)).expect("writing to a string cannot fail");
    }
}

fn write_variables(out: &mut String, indent: &str, variables: &BTreeMap<String, String>) {
    if variables.is_empty() {
        return;
    }
    writeln!(out, "{indent}variables:").expect("writing to a string cannot fail");
    for (key, value) in variables {
        write_multiline_or_scalar(out, &format!("{indent}  "), key, value);
    }
}

fn write_property_changes(
    out: &mut String,
    indent: &str,
    key: &str,
    changes: &BTreeMap<String, PropertyChange>,
) {
    if changes.is_empty() {
        return;
    }
    writeln!(out, "{indent}{}:", emitted_key(key)).expect("writing to a string cannot fail");
    for (change_key, change) in changes {
        write_property_change(out, &format!("{indent}  "), change_key, change);
    }
}

fn write_adapter_ids(out: &mut String, indent: &str, adapter_ids: &AdapterIds) {
    if adapter_ids.is_empty() {
        writeln!(out, "{indent}adapter_ids: {{}}").expect("writing to a string cannot fail");
        return;
    }

    writeln!(out, "{indent}adapter_ids:").expect("writing to a string cannot fail");
    for (key, value) in adapter_ids.iter() {
        writeln!(
            out,
            "{indent}  {}: {}",
            emitted_key(key),
            yaml_scalar(value)
        )
        .expect("writing to a string cannot fail");
    }
}

fn write_property_change(out: &mut String, indent: &str, key: &str, change: &PropertyChange) {
    writeln!(out, "{indent}{}:", emitted_key(key)).expect("writing to a string cannot fail");
    writeln!(
        out,
        "{indent}  intent: {}",
        change_intent_name(change.intent)
    )
    .expect("writing to a string cannot fail");
    if let Some(value) = &change.value {
        write_multiline_or_scalar(out, &format!("{indent}  "), "value", value);
    }
    if let Some(expected_base) = &change.expected_base {
        write_expected_base(out, &format!("{indent}  "), expected_base);
    }
}

fn write_adapter_id_changes(
    out: &mut String,
    indent: &str,
    adapter_ids: &BTreeMap<String, AdapterIdChange>,
) {
    if adapter_ids.is_empty() {
        return;
    }
    writeln!(out, "{indent}adapter_ids:").expect("writing to a string cannot fail");
    for (key, change) in adapter_ids {
        writeln!(out, "{indent}  {}:", emitted_key(key)).expect("writing to a string cannot fail");
        writeln!(
            out,
            "{indent}    intent: {}",
            change_intent_name(change.intent)
        )
        .expect("writing to a string cannot fail");
        if let Some(value) = &change.value {
            write_multiline_or_scalar(out, &format!("{indent}    "), "value", value);
        }
        if let Some(expected_base) = &change.expected_base {
            write_expected_base(out, &format!("{indent}    "), expected_base);
        }
    }
}

fn write_field_change(out: &mut String, indent: &str, field_id: &StableId, change: &FieldChange) {
    writeln!(out, "{indent}{}:", emitted_key(field_id.as_str()))
        .expect("writing to a string cannot fail");
    writeln!(
        out,
        "{indent}  intent: {}",
        change_intent_name(change.intent)
    )
    .expect("writing to a string cannot fail");
    if let Some(value) = &change.value {
        match value {
            FieldValue::Scalar(value) => {
                write_multiline_or_scalar(out, &format!("{indent}  "), "value", value);
            }
            FieldValue::Images(images) => {
                write_image_change_value(out, &format!("{indent}  "), images);
            }
            FieldValue::Message(message) => {
                write_structured_message_value(out, &format!("{indent}  "), message);
            }
            FieldValue::MessageItems(message) => {
                writeln!(out, "{indent}  value:").expect("writing to a string cannot fail");
                write_list_message_items(out, &format!("{indent}    "), message);
            }
        }
    }
    if let Some(expected_base) = &change.expected_base {
        write_expected_base(out, &format!("{indent}  "), expected_base);
    }
}

fn write_image_change_value(out: &mut String, indent: &str, images: &[FieldImageReference]) {
    match images {
        [image] => {
            writeln!(
                out,
                "{indent}value: !image {}",
                yaml_scalar(image.media_id.as_str())
            )
            .expect("writing to a string cannot fail");
        }
        _ => {
            writeln!(out, "{indent}value:").expect("writing to a string cannot fail");
            for image in images {
                writeln!(
                    out,
                    "{indent}  - !image {}",
                    yaml_scalar(image.media_id.as_str())
                )
                .expect("writing to a string cannot fail");
            }
        }
    }
}

fn write_note_type_payload(out: &mut String, indent: &str, note_type: &NoteType) {
    writeln!(out, "{indent}name: {}", yaml_scalar(&note_type.name))
        .expect("writing to a string cannot fail");
    write_variables(out, indent, &note_type.variables);
    writeln!(out, "{indent}field_order:").expect("writing to a string cannot fail");
    for field in &note_type.fields {
        writeln!(out, "{indent}  - {}", field.id).expect("writing to a string cannot fail");
    }
    writeln!(out, "{indent}fields:").expect("writing to a string cannot fail");
    let fields_by_id = note_type
        .fields
        .iter()
        .map(|field| (&field.id, field))
        .collect::<BTreeMap<_, _>>();
    for (field_id, field) in fields_by_id {
        writeln!(out, "{indent}  {}:", emitted_key(field_id.as_str()))
            .expect("writing to a string cannot fail");
        writeln!(out, "{indent}    name: {}", yaml_scalar(&field.name))
            .expect("writing to a string cannot fail");
        if let Some(pattern) = &field.message_pattern {
            write_list_message_pattern(out, &format!("{indent}    "), pattern);
        }
    }
    if note_type.card_templates.is_empty() {
        writeln!(out, "{indent}card_template_order: []").expect("writing to a string cannot fail");
        writeln!(out, "{indent}card_templates: {{}}").expect("writing to a string cannot fail");
    } else {
        writeln!(out, "{indent}card_template_order:").expect("writing to a string cannot fail");
        for template in &note_type.card_templates {
            writeln!(out, "{indent}  - {}", template.id).expect("writing to a string cannot fail");
        }
        writeln!(out, "{indent}card_templates:").expect("writing to a string cannot fail");
    }
    let templates_by_id = note_type
        .card_templates
        .iter()
        .map(|template| (&template.id, template))
        .collect::<BTreeMap<_, _>>();
    for (template_id, template) in templates_by_id {
        writeln!(out, "{indent}  {}:", emitted_key(template_id.as_str()))
            .expect("writing to a string cannot fail");
        writeln!(out, "{indent}    name: {}", yaml_scalar(&template.name))
            .expect("writing to a string cannot fail");
        write_variables(out, &format!("{indent}    "), &template.variables);
        write_multiline_or_scalar(
            out,
            &format!("{indent}    "),
            "question_format",
            &template.question_format,
        );
        write_multiline_or_scalar(
            out,
            &format!("{indent}    "),
            "answer_format",
            &template.answer_format,
        );
        write_adapter_ids(out, &format!("{indent}    "), &template.adapter_ids);
    }
    write_multiline_or_scalar(out, indent, "styling", &note_type.styling);
    write_adapter_ids(out, indent, &note_type.adapter_ids);
}

fn write_note_payload(out: &mut String, indent: &str, note: &Note) {
    writeln!(out, "{indent}note_type_id: {}", note.note_type_id)
        .expect("writing to a string cannot fail");
    write_variables(out, indent, &note.variables);
    writeln!(out, "{indent}fields:").expect("writing to a string cannot fail");
    for (field_id, value) in &note.fields {
        match value {
            FieldValue::Scalar(value) => writeln!(
                out,
                "{indent}  {}: {}",
                emitted_key(field_id.as_str()),
                yaml_scalar(value)
            )
            .expect("writing to a string cannot fail"),
            FieldValue::Message(message) => {
                write_structured_message_field(out, &format!("{indent}  "), field_id, message);
            }
            FieldValue::MessageItems(message) => {
                write_list_message_field(out, &format!("{indent}  "), field_id, message);
            }
            FieldValue::Images(images) => {
                write_image_field_value(out, &format!("{indent}  "), field_id, images);
            }
        }
    }
    if note.tags.is_empty() {
        writeln!(out, "{indent}tags: []").expect("writing to a string cannot fail");
    } else {
        writeln!(out, "{indent}tags:").expect("writing to a string cannot fail");
        for tag in &note.tags {
            writeln!(out, "{indent}  - {}", yaml_scalar(tag))
                .expect("writing to a string cannot fail");
        }
    }
    write_adapter_ids(out, indent, &note.adapter_ids);
}

fn write_image_field_value(
    out: &mut String,
    indent: &str,
    field_id: &StableId,
    images: &[FieldImageReference],
) {
    match images {
        [image] => {
            writeln!(
                out,
                "{indent}{}: !image {}",
                emitted_key(field_id.as_str()),
                yaml_scalar(image.media_id.as_str())
            )
            .expect("writing to a string cannot fail");
        }
        _ => {
            writeln!(out, "{indent}{}:", emitted_key(field_id.as_str()))
                .expect("writing to a string cannot fail");
            for image in images {
                writeln!(
                    out,
                    "{indent}  - !image {}",
                    yaml_scalar(image.media_id.as_str())
                )
                .expect("writing to a string cannot fail");
            }
        }
    }
}

fn write_list_message_pattern(out: &mut String, indent: &str, pattern: &ListMessagePattern) {
    writeln!(out, "{indent}message_pattern:").expect("writing to a string cannot fail");
    writeln!(out, "{indent}  kind: list").expect("writing to a string cannot fail");
    writeln!(
        out,
        "{indent}  item_format: {}",
        yaml_scalar(&pattern.item_format)
    )
    .expect("writing to a string cannot fail");
    writeln!(
        out,
        "{indent}  separator: {}",
        yaml_scalar(&pattern.separator)
    )
    .expect("writing to a string cannot fail");
    writeln!(out, "{indent}  parameters:").expect("writing to a string cannot fail");
    for (name, parameter) in &pattern.parameters {
        writeln!(out, "{indent}    {}:", emitted_key(name))
            .expect("writing to a string cannot fail");
        match parameter {
            ListMessageParameter::NoteFieldRef { field_id } => {
                writeln!(out, "{indent}      type: note_field_ref")
                    .expect("writing to a string cannot fail");
                writeln!(out, "{indent}      field: {field_id}")
                    .expect("writing to a string cannot fail");
            }
            ListMessageParameter::Text => {
                writeln!(out, "{indent}      type: text").expect("writing to a string cannot fail");
            }
        }
    }
}

fn write_list_message_field(
    out: &mut String,
    indent: &str,
    field_id: &StableId,
    message: &ListMessageItems,
) {
    writeln!(out, "{indent}{}:", emitted_key(field_id.as_str()))
        .expect("writing to a string cannot fail");
    write_list_message_items(out, &format!("{indent}  "), message);
}

fn write_list_message_items(out: &mut String, indent: &str, message: &ListMessageItems) {
    writeln!(out, "{indent}items:").expect("writing to a string cannot fail");
    for item in &message.items {
        let mut parameters = item.iter();
        if let Some((name, value)) = parameters.next() {
            write_list_message_argument(
                out,
                &format!("{indent}  - "),
                &format!("{indent}      "),
                name,
                value,
            );
        }
        for (name, value) in parameters {
            write_list_message_argument(
                out,
                &format!("{indent}    "),
                &format!("{indent}      "),
                name,
                value,
            );
        }
    }
}

fn write_list_message_argument(
    out: &mut String,
    indent: &str,
    nested_indent: &str,
    name: &str,
    value: &ListMessageArgument,
) {
    match value {
        ListMessageArgument::Scalar(value) => {
            writeln!(out, "{indent}{}: {}", emitted_key(name), yaml_scalar(value))
                .expect("writing to a string cannot fail");
        }
        ListMessageArgument::Text(value) => {
            writeln!(out, "{indent}{}:", emitted_key(name))
                .expect("writing to a string cannot fail");
            writeln!(out, "{nested_indent}text: {}", yaml_scalar(value))
                .expect("writing to a string cannot fail");
        }
    }
}

fn write_structured_message_field(
    out: &mut String,
    indent: &str,
    field_id: &StableId,
    message: &StructuredMessage,
) {
    writeln!(out, "{indent}{}:", emitted_key(field_id.as_str()))
        .expect("writing to a string cannot fail");
    write_structured_message_value(out, &format!("{indent}  "), message);
}

fn write_structured_message_value(out: &mut String, indent: &str, message: &StructuredMessage) {
    if let Some(format) = &message.format {
        writeln!(out, "{indent}format: {}", yaml_scalar(format))
            .expect("writing to a string cannot fail");
        if message.variables.is_empty() {
            writeln!(out, "{indent}variables: {{}}").expect("writing to a string cannot fail");
        } else {
            writeln!(out, "{indent}variables:").expect("writing to a string cannot fail");
        }
        for (name, component) in &message.variables {
            writeln!(out, "{indent}  {}:", emitted_key(name))
                .expect("writing to a string cannot fail");
            write_message_component(out, &format!("{indent}    "), component, false);
        }
    } else if message.components.is_empty() {
        writeln!(out, "{indent}message: []").expect("writing to a string cannot fail");
    } else {
        writeln!(out, "{indent}message:").expect("writing to a string cannot fail");
        write_message_components(out, &format!("{indent}  "), message);
    }
}

fn write_message_components(out: &mut String, indent: &str, message: &StructuredMessage) {
    for component in &message.components {
        write_message_component(out, indent, component, true);
    }
}

fn write_message_component(
    out: &mut String,
    indent: &str,
    component: &MessageComponent,
    list: bool,
) {
    let prefix = if list { "- " } else { "" };
    match component {
        MessageComponent::Literal(value) => {
            writeln!(out, "{indent}{prefix}literal: {}", yaml_scalar(value))
                .expect("writing to a string cannot fail");
        }
        MessageComponent::Text(value) => {
            writeln!(out, "{indent}{prefix}text: {}", yaml_scalar(value))
                .expect("writing to a string cannot fail");
        }
        MessageComponent::FieldRef(reference) => {
            writeln!(out, "{indent}{prefix}ref: {}", yaml_scalar(reference))
                .expect("writing to a string cannot fail");
        }
    }
}

fn write_card_template_payload(out: &mut String, indent: &str, template: &CardTemplate) {
    writeln!(out, "{indent}name: {}", yaml_scalar(&template.name))
        .expect("writing to a string cannot fail");
    write_variables(out, indent, &template.variables);
    write_multiline_or_scalar(out, indent, "question_format", &template.question_format);
    write_multiline_or_scalar(out, indent, "answer_format", &template.answer_format);
    write_adapter_ids(out, indent, &template.adapter_ids);
}

fn write_expected_base(out: &mut String, indent: &str, expected_base: &ExpectedBase) {
    match expected_base {
        ExpectedBase::EntityFingerprint(fingerprint) => {
            writeln!(
                out,
                "{indent}expected_base:\n{indent}  fingerprint: {fingerprint}"
            )
            .expect("writing to a string cannot fail");
        }
        ExpectedBase::EntityPresent => {
            writeln!(out, "{indent}expected_base: entity_present")
                .expect("writing to a string cannot fail");
        }
        ExpectedBase::Value(value) => {
            writeln!(out, "{indent}expected_base:").expect("writing to a string cannot fail");
            write_multiline_or_scalar(out, &format!("{indent}  "), "value", value);
        }
        ExpectedBase::FieldValue(value) => {
            writeln!(out, "{indent}expected_base:").expect("writing to a string cannot fail");
            let value_key = StableId::new("value").expect("static stable ID");
            write_field_value_for_format(out, &format!("{indent}  "), &value_key, value);
        }
    }
}

fn overlay_kind_name(kind: OverlayKind) -> &'static str {
    match kind {
        OverlayKind::Translation => "translation",
        OverlayKind::Extension => "extension",
        OverlayKind::Patch => "patch",
        OverlayKind::Personal => "personal",
    }
}

fn target_adaptation_intent_name(intent: TargetAdaptationIntent) -> &'static str {
    match intent {
        TargetAdaptationIntent::Adapt => "adapt",
        TargetAdaptationIntent::Delete => "delete",
    }
}

fn target_adaptation_ownership_name(ownership: TargetAdaptationOwnership) -> &'static str {
    match ownership {
        TargetAdaptationOwnership::Translation => "translation",
        TargetAdaptationOwnership::Extension => "extension",
    }
}

fn change_intent_name(intent: ChangeIntent) -> &'static str {
    match intent {
        ChangeIntent::Add => "add",
        ChangeIntent::Merge => "merge",
        ChangeIntent::Replace => "replace",
        ChangeIntent::Remove => "remove",
        ChangeIntent::Override => "override",
    }
}

#[derive(Debug)]
pub enum CanonicalYamlError {
    Parse(serde_yaml::Error),
    StableId(InvalidStableId),
    InvalidOverlayKind(String),
    InvalidChangeIntent(String),
    InvalidExpectedBase(String),
    InvalidTranslationDictionary(String),
    InvalidFieldAddition(String),
    InvalidFieldFill(String),
    InvalidFieldValue(String),
    InvalidSchemaValue { path: String, message: String },
    MissingOrderedEntity { section: &'static str, id: String },
    UnorderedEntity { section: &'static str, id: String },
    Validation(ValidationReport),
    UnemittableYamlKey { section: &'static str, key: String },
}

impl fmt::Display for CanonicalYamlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "failed to parse canonical YAML: {error}"),
            Self::StableId(error) => write!(f, "{error}"),
            Self::InvalidOverlayKind(kind) => write!(f, "invalid overlay kind {kind:?}"),
            Self::InvalidChangeIntent(intent) => write!(f, "invalid change intent {intent:?}"),
            Self::InvalidExpectedBase(expected_base) => {
                write!(f, "invalid expected base {expected_base:?}")
            }
            Self::InvalidTranslationDictionary(message) => {
                write!(f, "invalid translation dictionary: {message}")
            }
            Self::InvalidFieldAddition(message) => write!(f, "invalid field addition: {message}"),
            Self::InvalidFieldFill(message) => write!(f, "invalid field fill: {message}"),
            Self::InvalidFieldValue(message) => write!(f, "invalid field value: {message}"),
            Self::InvalidSchemaValue { path, message } => {
                write!(f, "invalid value at schema path {path}: {message}")
            }
            Self::MissingOrderedEntity { section, id } => {
                write!(f, "{section} order references missing entity {id}")
            }
            Self::UnorderedEntity { section, id } => {
                write!(f, "{section} entity {id} is missing from its order array")
            }
            Self::Validation(report) => write!(f, "canonical deck validation failed: {report}"),
            Self::UnemittableYamlKey { section, key } => write!(
                f,
                "canonical YAML {section} key {key:?} cannot be emitted safely"
            ),
        }
    }
}

impl std::error::Error for CanonicalYamlError {}

impl From<InvalidStableId> for CanonicalYamlError {
    fn from(error: InvalidStableId) -> Self {
        Self::StableId(error)
    }
}

fn validate_canonical_field_value_unions(input: &str) -> Result<(), CanonicalYamlError> {
    let root: Value = serde_yaml::from_str(input).map_err(CanonicalYamlError::Parse)?;
    let Some(notes) = mapping_value(&root, "notes").and_then(Value::as_mapping) else {
        return Ok(());
    };
    for (note_id, note) in notes {
        let Some(note_id) = note_id.as_str() else {
            continue;
        };
        validate_note_field_values(note, &format!("notes.{note_id}"))?;
    }
    Ok(())
}

fn validate_overlay_unions(input: &str) -> Result<(), CanonicalYamlError> {
    let root: Value = serde_yaml::from_str(input).map_err(CanonicalYamlError::Parse)?;
    if let Some(notes) = mapping_value(&root, "notes").and_then(Value::as_mapping) {
        for (note_id, note_change) in notes {
            let Some(note_id) = note_id.as_str() else {
                continue;
            };
            let note_path = format!("notes.{note_id}");
            if let Some(note) = mapping_value(note_change, "note") {
                validate_note_field_values(note, &format!("{note_path}.note"))?;
            }
            if let Some(fields) = mapping_value(note_change, "fields").and_then(Value::as_mapping) {
                for (field_id, change) in fields {
                    let Some(field_id) = field_id.as_str() else {
                        continue;
                    };
                    validate_field_change(change, &format!("{note_path}.fields.{field_id}"))?;
                }
            }
        }
    }
    if let Some(media) = mapping_value(&root, "media").and_then(Value::as_mapping) {
        for (media_id, change) in media {
            let Some(media_id) = media_id.as_str() else {
                continue;
            };
            let Some(change) = change.as_mapping() else {
                continue;
            };
            let has_path = change.contains_key(Value::String("path".to_owned()));
            let has_sha256 = change.contains_key(Value::String("sha256".to_owned()));
            if has_path != has_sha256 {
                return schema_error(
                    format!("media.{media_id}"),
                    "path and sha256 must be provided together",
                );
            }
        }
    }
    if let Some(fills) = mapping_value(&root, "field_fills").and_then(Value::as_mapping) {
        for (note_id, fields) in fills {
            let (Some(note_id), Some(fields)) = (note_id.as_str(), fields.as_mapping()) else {
                continue;
            };
            for (field_id, value) in fields {
                let Some(field_id) = field_id.as_str() else {
                    continue;
                };
                validate_field_value(value, &format!("field_fills.{note_id}.{field_id}"), true)?;
            }
        }
    }
    if let Some(additions) = mapping_value(&root, "field_additions").and_then(Value::as_mapping) {
        for (note_type_id, addition) in additions {
            let (Some(note_type_id), Some(values)) = (
                note_type_id.as_str(),
                mapping_value(addition, "values").and_then(Value::as_mapping),
            ) else {
                continue;
            };
            for (note_id, fields) in values {
                let (Some(note_id), Some(fields)) = (note_id.as_str(), fields.as_mapping()) else {
                    continue;
                };
                for (field_id, value) in fields {
                    let Some(field_id) = field_id.as_str() else {
                        continue;
                    };
                    validate_field_value(
                        value,
                        &format!("field_additions.{note_type_id}.values.{note_id}.{field_id}"),
                        false,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn validate_note_field_values(note: &Value, note_path: &str) -> Result<(), CanonicalYamlError> {
    let Some(fields) = mapping_value(note, "fields").and_then(Value::as_mapping) else {
        return Ok(());
    };
    for (field_id, value) in fields {
        let Some(field_id) = field_id.as_str() else {
            continue;
        };
        validate_field_value(value, &format!("{note_path}.fields.{field_id}"), true)?;
    }
    Ok(())
}

fn validate_field_change(value: &Value, path: &str) -> Result<(), CanonicalYamlError> {
    let Some(change) = value.as_mapping() else {
        return Ok(());
    };
    let has_value = change.contains_key(Value::String("value".to_owned()));
    let has_message = change.contains_key(Value::String("message".to_owned()));
    let has_format = change.contains_key(Value::String("format".to_owned()));
    let has_variables = change.contains_key(Value::String("variables".to_owned()));
    let alternatives = usize::from(has_value) + usize::from(has_message) + usize::from(has_format);
    if alternatives > 1 {
        return schema_error(
            path.to_owned(),
            "value, message, and format are mutually exclusive field representations",
        );
    }
    if has_variables != has_format {
        return schema_error(
            path.to_owned(),
            "format and variables must be provided together",
        );
    }
    if let Some(field_value) = mapping_value(value, "value") {
        validate_field_value(field_value, &format!("{path}.value"), false)?;
    }
    if let Some(message) = mapping_value(value, "message") {
        validate_message_components(message, &format!("{path}.message"))?;
    }
    if let Some(variables) = mapping_value(value, "variables") {
        validate_message_variables(variables, &format!("{path}.variables"))?;
    }
    Ok(())
}

fn validate_field_value(
    value: &Value,
    path: &str,
    allow_messages: bool,
) -> Result<(), CanonicalYamlError> {
    match value {
        Value::String(_) => Ok(()),
        Value::Tagged(tagged) => validate_image_value(&tagged.tag.to_string(), &tagged.value, path),
        Value::Sequence(items) => {
            if items.is_empty() {
                return schema_error(path.to_owned(), "image sequence must not be empty");
            }
            for (index, item) in items.iter().enumerate() {
                let Value::Tagged(tagged) = item else {
                    return schema_error(
                        format!("{path}[{index}]"),
                        "image sequences may contain only !image tagged scalars",
                    );
                };
                validate_image_value(
                    &tagged.tag.to_string(),
                    &tagged.value,
                    &format!("{path}[{index}]"),
                )?;
            }
            Ok(())
        }
        Value::Mapping(mapping)
            if mapping.len() == 1 && mapping.contains_key(Value::String("items".to_owned())) =>
        {
            let Some(items) = mapping_value(value, "items").and_then(Value::as_sequence) else {
                return schema_error(
                    format!("{path}.items"),
                    "list message items must be a sequence",
                );
            };
            if items.is_empty() {
                return schema_error(
                    format!("{path}.items"),
                    "list message must contain at least one item",
                );
            }
            for (index, item) in items.iter().enumerate() {
                let Some(parameters) = item.as_mapping() else {
                    return schema_error(
                        format!("{path}.items[{index}]"),
                        "list message item must be a parameter mapping",
                    );
                };
                if parameters.is_empty() {
                    return schema_error(
                        format!("{path}.items[{index}]"),
                        "list message item parameters must not be empty",
                    );
                }
                for (name, argument) in parameters {
                    let name = name.as_str().unwrap_or("<non-string>");
                    let argument_path = format!("{path}.items[{index}].{name}");
                    match argument {
                        Value::String(_) => {}
                        Value::Mapping(explicit)
                            if explicit.len() == 1
                                && explicit
                                    .get(Value::String("text".to_owned()))
                                    .is_some_and(Value::is_string) => {}
                        Value::Mapping(_) => {
                            return schema_error(
                                argument_path,
                                "explicit list message argument must contain exactly one string `text` property",
                            );
                        }
                        _ => {
                            return schema_error(
                                argument_path,
                                "list message argument must be a scalar string or explicit `text` object",
                            );
                        }
                    }
                }
            }
            Ok(())
        }
        Value::Mapping(mapping) if allow_messages => {
            let has_format = mapping.contains_key(Value::String("format".to_owned()));
            let has_variables = mapping.contains_key(Value::String("variables".to_owned()));
            let has_message = mapping.contains_key(Value::String("message".to_owned()));
            if has_message && mapping.len() == 1 {
                return validate_message_components(
                    mapping_value(value, "message").expect("message key exists"),
                    &format!("{path}.message"),
                );
            }
            if has_format && has_variables && mapping.len() == 2 {
                return validate_message_variables(
                    mapping_value(value, "variables").expect("variables key exists"),
                    &format!("{path}.variables"),
                );
            }
            schema_error(
                path.to_owned(),
                "structured field value must contain exactly message, or exactly format and variables",
            )
        }
        _ => schema_error(
            path.to_owned(),
            if allow_messages {
                "field value must be a string, structured message, or !image reference"
            } else {
                "field value must be a string or !image reference"
            },
        ),
    }
}

fn validate_image_value(tag: &str, value: &Value, path: &str) -> Result<(), CanonicalYamlError> {
    if tag != "!image" && tag != "image" {
        return schema_error(path.to_owned(), format!("unsupported YAML tag {tag}"));
    }
    match value {
        Value::String(media_id) if !media_id.is_empty() => Ok(()),
        _ => schema_error(path.to_owned(), "!image value must be a non-empty string"),
    }
}

fn validate_message_components(value: &Value, path: &str) -> Result<(), CanonicalYamlError> {
    let Some(components) = value.as_sequence() else {
        return schema_error(path.to_owned(), "message must be a sequence");
    };
    for (index, component) in components.iter().enumerate() {
        validate_message_component(component, &format!("{path}[{index}]"))?;
    }
    Ok(())
}

fn validate_message_variables(value: &Value, path: &str) -> Result<(), CanonicalYamlError> {
    let Some(variables) = value.as_mapping() else {
        return schema_error(path.to_owned(), "message variables must be a mapping");
    };
    for (name, component) in variables {
        let Some(name) = name.as_str() else {
            continue;
        };
        validate_message_component(component, &format!("{path}.{name}"))?;
    }
    Ok(())
}

fn validate_message_component(value: &Value, path: &str) -> Result<(), CanonicalYamlError> {
    let Some(component) = value.as_mapping() else {
        return schema_error(path.to_owned(), "message component must be a mapping");
    };
    if component.len() != 1 {
        return schema_error(
            path.to_owned(),
            "message component must contain exactly one of literal, text, or ref",
        );
    }
    let (key, value) = component.iter().next().expect("one component entry");
    if !matches!(key.as_str(), Some("literal" | "text" | "ref")) || !value.is_string() {
        return schema_error(
            path.to_owned(),
            "message component must contain exactly one string literal, text, or ref",
        );
    }
    Ok(())
}

fn mapping_value<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value.as_mapping()?.get(Value::String(key.to_owned()))
}

fn schema_error<T>(path: String, message: impl Into<String>) -> Result<T, CanonicalYamlError> {
    Err(CanonicalYamlError::InvalidSchemaValue {
        path,
        message: message.into(),
    })
}

fn validate_deck_yaml_keys(deck: &CanonicalDeck) -> Result<(), CanonicalYamlError> {
    validate_string_keys("deck.variables", deck.variables.keys())?;
    validate_adapter_id_keys("deck.adapter_ids", &deck.adapter_ids)?;
    for note_type in deck.note_types.values() {
        validate_string_keys("note_type.variables", note_type.variables.keys())?;
        validate_adapter_id_keys("note_type.adapter_ids", &note_type.adapter_ids)?;
        for template in &note_type.card_templates {
            validate_string_keys("card_template.variables", template.variables.keys())?;
            validate_adapter_id_keys("card_template.adapter_ids", &template.adapter_ids)?;
        }
    }
    for note in deck.notes.values() {
        validate_note_representations(note, &format!("notes.{}", note.id))?;
        validate_string_keys("note.variables", note.variables.keys())?;
        validate_adapter_id_keys("note.adapter_ids", &note.adapter_ids)?;
        for value in note.fields.values() {
            if let FieldValue::Message(message) = value {
                validate_structured_message_keys(message)?;
            }
        }
    }
    Ok(())
}

fn validate_overlay_yaml_keys(overlay: &Overlay) -> Result<(), CanonicalYamlError> {
    if let Some(translations) = &overlay.translations {
        validate_translation_dictionary_keys(translations)?;
    }
    if let Some(deck_change) = &overlay.deck_change {
        validate_string_keys("deck.variables", deck_change.variables.keys())?;
        validate_string_keys("deck.adapter_ids", deck_change.adapter_ids.keys())?;
    }
    for change in overlay.note_type_changes.values() {
        if let Some(note_type) = &change.note_type {
            validate_deck_note_type_payload_keys(note_type)?;
        }
        validate_string_keys("note_type.variables", change.variables.keys())?;
        for template_change in change.card_templates.values() {
            if let Some(template) = &template_change.template {
                validate_string_keys("card_template.variables", template.variables.keys())?;
                validate_adapter_id_keys("card_template.adapter_ids", &template.adapter_ids)?;
            }
            validate_string_keys("card_template.variables", template_change.variables.keys())?;
            validate_string_keys(
                "card_template.adapter_ids",
                template_change.adapter_ids.keys(),
            )?;
        }
        validate_string_keys("note_type.adapter_ids", change.adapter_ids.keys())?;
    }
    for change in overlay.note_changes.values() {
        if let Some(note) = &change.note {
            validate_string_keys("note.variables", note.variables.keys())?;
            validate_adapter_id_keys("note.adapter_ids", &note.adapter_ids)?;
            for value in note.fields.values() {
                if let FieldValue::Message(message) = value {
                    validate_structured_message_keys(message)?;
                }
            }
        }
        validate_string_keys("note.variables", change.variables.keys())?;
        validate_string_keys("note.tags", change.tags.keys())?;
        validate_string_keys("note.adapter_ids", change.adapter_ids.keys())?;
        for field_change in change.fields.values() {
            if let Some(FieldValue::Message(message)) = &field_change.value {
                validate_structured_message_keys(message)?;
            }
        }
    }
    Ok(())
}

fn validate_overlay_representations(overlay: &Overlay) -> Result<(), CanonicalYamlError> {
    let reject = |expected_base: &Option<ExpectedBase>| {
        if matches!(expected_base, Some(ExpectedBase::EntityPresent)) {
            Err(CanonicalYamlError::InvalidExpectedBase(
                "entity_present is no longer accepted for destructive changes; use an exact typed value or generate an entity fingerprint with `brainbrew diff --as-overlay`"
                    .to_owned(),
            ))
        } else {
            Ok(())
        }
    };
    if let Some(change) = &overlay.deck_change {
        for property in change.name.iter().chain(change.description.iter()) {
            reject(&property.expected_base)?;
        }
        for property in change.variables.values() {
            reject(&property.expected_base)?;
        }
        for adapter in change.adapter_ids.values() {
            reject(&adapter.expected_base)?;
        }
    }
    for change in overlay.note_type_changes.values() {
        reject(&change.expected_base)?;
        for property in change
            .name
            .iter()
            .chain(change.styling.iter())
            .chain(change.variables.values())
        {
            reject(&property.expected_base)?;
        }
        for field in change.fields.values() {
            reject(&field.expected_base)?;
        }
        for template in change.card_templates.values() {
            reject(&template.expected_base)?;
            for property in template
                .name
                .iter()
                .chain(template.question_format.iter())
                .chain(template.answer_format.iter())
                .chain(template.variables.values())
            {
                reject(&property.expected_base)?;
            }
            for adapter in template.adapter_ids.values() {
                reject(&adapter.expected_base)?;
            }
        }
        for adapter in change.adapter_ids.values() {
            reject(&adapter.expected_base)?;
        }
    }
    for change in overlay.note_changes.values() {
        reject(&change.expected_base)?;
        for property in change.variables.values() {
            reject(&property.expected_base)?;
        }
        for field in change.fields.values() {
            reject(&field.expected_base)?;
        }
        for tag in change.tags.values() {
            reject(&tag.expected_base)?;
        }
        for adapter in change.adapter_ids.values() {
            reject(&adapter.expected_base)?;
        }
    }
    for change in overlay.media_changes.values() {
        reject(&change.expected_base)?;
    }

    for (note_id, change) in &overlay.note_changes {
        if let Some(note) = &change.note {
            validate_note_representations(note, &format!("notes.{note_id}.note"))?;
        }
        for (field_id, field_change) in &change.fields {
            let path = format!("notes.{note_id}.fields.{field_id}");
            if let Some(value) = &field_change.value {
                validate_field_value_representation(value, &path)?;
            }
        }
    }
    for (media_id, change) in &overlay.media_changes {
        if let Some(media) = &change.media
            && media.id != *media_id
        {
            return schema_error(
                format!("media.{media_id}"),
                format!("media payload id {} does not match its map key", media.id),
            );
        }
    }
    Ok(())
}

fn validate_note_representations(note: &Note, path: &str) -> Result<(), CanonicalYamlError> {
    for (field_id, value) in &note.fields {
        validate_field_value_representation(value, &format!("{path}.fields.{field_id}"))?;
    }
    Ok(())
}

fn validate_field_value_representation(
    value: &FieldValue,
    path: &str,
) -> Result<(), CanonicalYamlError> {
    match value {
        FieldValue::Scalar(_) => Ok(()),
        FieldValue::Images(images) if images.is_empty() => {
            schema_error(path.to_owned(), "image sequence must not be empty")
        }
        FieldValue::Images(_) => Ok(()),
        FieldValue::Message(message) => validate_structured_message_representation(message, path),
        FieldValue::MessageItems(message) if message.items.is_empty() => {
            Err(CanonicalYamlError::InvalidFieldValue(format!(
                "list message at {path} must contain at least one item"
            )))
        }
        FieldValue::MessageItems(_) => Ok(()),
    }
}

fn validate_structured_message_representation(
    message: &StructuredMessage,
    path: &str,
) -> Result<(), CanonicalYamlError> {
    message
        .validate_shape()
        .map_err(|error| CanonicalYamlError::InvalidSchemaValue {
            path: path.to_owned(),
            message: error.to_string(),
        })
}

fn validate_deck_note_type_payload_keys(note_type: &NoteType) -> Result<(), CanonicalYamlError> {
    validate_string_keys("note_type.variables", note_type.variables.keys())?;
    validate_adapter_id_keys("note_type.adapter_ids", &note_type.adapter_ids)?;
    for template in &note_type.card_templates {
        validate_string_keys("card_template.variables", template.variables.keys())?;
        validate_adapter_id_keys("card_template.adapter_ids", &template.adapter_ids)?;
    }
    Ok(())
}

fn validate_translation_dictionary_keys(
    translations: &TranslationDictionary,
) -> Result<(), CanonicalYamlError> {
    validate_string_keys("translations.contextual", translations.contextual.keys())?;
    validate_string_keys(
        "translations.target_adaptations",
        translations.target_adaptations.keys(),
    )?;
    validate_string_keys("translations.variables", translations.variables.keys())?;
    validate_string_keys("translations.adapter_ids", translations.adapter_ids.keys())?;
    Ok(())
}

fn validate_structured_message_keys(message: &StructuredMessage) -> Result<(), CanonicalYamlError> {
    validate_string_keys("message.variables", message.variables.keys())
}

fn validate_adapter_id_keys(
    section: &'static str,
    adapter_ids: &AdapterIds,
) -> Result<(), CanonicalYamlError> {
    validate_string_keys(section, adapter_ids.iter().map(|(key, _)| key))
}

fn validate_string_keys<K>(
    section: &'static str,
    keys: impl IntoIterator<Item = K>,
) -> Result<(), CanonicalYamlError>
where
    K: AsRef<str>,
{
    for key in keys {
        validate_yaml_key(section, key.as_ref())?;
    }
    Ok(())
}

fn validate_yaml_key(section: &'static str, key: &str) -> Result<(), CanonicalYamlError> {
    if is_emittable_yaml_key(key) {
        Ok(())
    } else {
        Err(CanonicalYamlError::UnemittableYamlKey {
            section,
            key: key.to_owned(),
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OverlayYaml {
    id: String,
    kind: String,
    #[serde(default)]
    translations: Option<TranslationDictionaryYaml>,
    #[serde(default)]
    target_adaptations: BTreeMap<String, TargetAdaptationYaml>,
    #[serde(default)]
    stale_translations: Vec<StaleTranslationYaml>,
    #[serde(default)]
    deck: Option<DeckChangeYaml>,
    #[serde(default)]
    field_additions: BTreeMap<String, FieldAdditionsYaml>,
    #[serde(default)]
    field_fills: BTreeMap<String, BTreeMap<String, FieldValueYaml>>,
    #[serde(default)]
    notes: BTreeMap<String, NoteChangeYaml>,
    #[serde(default)]
    note_types: BTreeMap<String, NoteTypeChangeYaml>,
    #[serde(default)]
    media: BTreeMap<String, MediaChangeYaml>,
}

impl OverlayYaml {
    fn into_overlay(self) -> Result<Overlay, CanonicalYamlError> {
        let mut note_changes = self
            .notes
            .into_iter()
            .map(|(id, change)| {
                let id = sid(&id)?;
                Ok((id.clone(), change.into_note_change(id)?))
            })
            .collect::<Result<BTreeMap<_, _>, CanonicalYamlError>>()?;
        let mut note_type_changes = self
            .note_types
            .into_iter()
            .map(|(id, change)| {
                let id = sid(&id)?;
                Ok((id.clone(), change.into_note_type_change(id)?))
            })
            .collect::<Result<BTreeMap<_, _>, CanonicalYamlError>>()?;

        for (note_type_id, additions) in self.field_additions {
            additions.apply(
                sid(&note_type_id)?,
                &mut note_type_changes,
                &mut note_changes,
            )?;
        }

        apply_field_fills(self.field_fills, &mut note_changes)?;

        let had_translations = self.translations.is_some();
        let mut translations = self
            .translations
            .map(TranslationDictionaryYaml::into_translation_dictionary)
            .transpose()?
            .unwrap_or_default();
        for (path, adaptation) in self.target_adaptations {
            if translations
                .target_adaptations
                .insert(path.clone(), adaptation.into_target_adaptation(&path)?)
                .is_some()
            {
                return Err(CanonicalYamlError::InvalidTranslationDictionary(format!(
                    "top-level target adaptation {path} duplicates translations target_adaptations entry"
                )));
            }
        }
        translations.stale_translations.extend(
            self.stale_translations
                .into_iter()
                .map(StaleTranslationYaml::into_stale_translation),
        );
        let translations = (had_translations
            || !translation_dictionary_is_empty(&translations)
            || !translations.target_adaptations.is_empty()
            || !translations.stale_translations.is_empty())
        .then_some(translations);

        Ok(Overlay {
            id: sid(&self.id)?,
            kind: parse_overlay_kind(&self.kind)?,
            translations,
            deck_change: self
                .deck
                .map(DeckChangeYaml::into_deck_change)
                .transpose()?,
            note_changes,
            note_type_changes,
            media_changes: self
                .media
                .into_iter()
                .map(|(id, change)| {
                    let id = sid(&id)?;
                    Ok((id.clone(), change.into_media_change(id)?))
                })
                .collect::<Result<_, CanonicalYamlError>>()?,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FieldAdditionsYaml {
    fields: BTreeMap<String, String>,
    #[serde(default)]
    values: BTreeMap<String, BTreeMap<String, FieldValueYaml>>,
}

impl FieldAdditionsYaml {
    fn apply(
        self,
        note_type_id: StableId,
        note_type_changes: &mut BTreeMap<StableId, NoteTypeChange>,
        note_changes: &mut BTreeMap<StableId, NoteChange>,
    ) -> Result<(), CanonicalYamlError> {
        if self.fields.is_empty() {
            return Err(CanonicalYamlError::InvalidFieldAddition(format!(
                "field_additions.{note_type_id}.fields must not be empty"
            )));
        }

        let mut declared_fields = BTreeSet::new();
        let note_type_change = note_type_changes
            .entry(note_type_id.clone())
            .or_insert_with(empty_note_type_merge_change);
        if note_type_change.intent != ChangeIntent::Merge {
            return Err(CanonicalYamlError::InvalidFieldAddition(format!(
                "field_additions.{note_type_id} can only merge into an existing note type"
            )));
        }

        for (field_id, name) in self.fields {
            let field_id = sid(&field_id)?;
            if !declared_fields.insert(field_id.clone()) {
                return Err(CanonicalYamlError::InvalidFieldAddition(format!(
                    "duplicate field_additions.{note_type_id}.fields.{field_id}"
                )));
            }
            if note_type_change.fields.contains_key(&field_id) {
                return Err(CanonicalYamlError::InvalidFieldAddition(format!(
                    "field_additions.{note_type_id}.fields.{field_id} conflicts with another field change"
                )));
            }
            note_type_change.fields.insert(
                field_id.clone(),
                FieldDefinitionChange {
                    intent: ChangeIntent::Add,
                    field: Some(FieldDefinition {
                        id: field_id,
                        name,
                        message_pattern: None,
                    }),
                    expected_base: None,
                },
            );
        }

        for (note_id, values) in self.values {
            let note_id = sid(&note_id)?;
            let note_change = note_changes
                .entry(note_id.clone())
                .or_insert_with(empty_note_merge_change);
            if note_change.intent != ChangeIntent::Merge {
                return Err(CanonicalYamlError::InvalidFieldAddition(format!(
                    "field_additions.{note_type_id}.values.{note_id} can only merge into an existing note"
                )));
            }
            for (field_id, value) in values {
                let field_id = sid(&field_id)?;
                if !declared_fields.contains(&field_id) {
                    return Err(CanonicalYamlError::InvalidFieldAddition(format!(
                        "field_additions.{note_type_id}.values.{note_id}.{field_id} has no declared field"
                    )));
                }
                if note_change.fields.contains_key(&field_id) {
                    return Err(CanonicalYamlError::InvalidFieldAddition(format!(
                        "field_additions.{note_type_id}.values.{note_id}.{field_id} conflicts with another field change"
                    )));
                }
                let value = match value {
                    FieldValueYaml::Scalar(value) => FieldValue::Scalar(value),
                    FieldValueYaml::Images(images) => FieldValue::images(images.into_images()?)
                        .map_err(|error| {
                            CanonicalYamlError::InvalidFieldAddition(error.to_string())
                        })?,
                    FieldValueYaml::Formatted(_)
                    | FieldValueYaml::Message(_)
                    | FieldValueYaml::MessageItems(_) => {
                        return Err(CanonicalYamlError::InvalidFieldAddition(format!(
                            "field_additions.{note_type_id}.values.{note_id}.{field_id} must be a scalar string or !image reference"
                        )));
                    }
                };
                note_change.fields.insert(
                    field_id,
                    FieldChange {
                        intent: ChangeIntent::Add,
                        value: Some(value),
                        expected_base: None,
                    },
                );
            }
        }

        Ok(())
    }
}

fn apply_field_fills(
    field_fills: BTreeMap<String, BTreeMap<String, FieldValueYaml>>,
    note_changes: &mut BTreeMap<StableId, NoteChange>,
) -> Result<(), CanonicalYamlError> {
    for (note_id, fields) in field_fills {
        let note_id = sid(&note_id)?;
        if fields.is_empty() {
            return Err(CanonicalYamlError::InvalidFieldFill(format!(
                "field_fills.{note_id} must not be empty"
            )));
        }
        let note_change = note_changes
            .entry(note_id.clone())
            .or_insert_with(empty_note_merge_change);
        if note_change.intent != ChangeIntent::Merge {
            return Err(CanonicalYamlError::InvalidFieldFill(format!(
                "field_fills.{note_id} can only merge into an existing note"
            )));
        }
        for (field_id, value) in fields {
            let field_id = sid(&field_id)?;
            if note_change.fields.contains_key(&field_id) {
                return Err(CanonicalYamlError::InvalidFieldFill(format!(
                    "field_fills.{note_id}.{field_id} conflicts with another field change"
                )));
            }
            let value = value.into_field_value()?;
            note_change.fields.insert(
                field_id,
                FieldChange {
                    intent: ChangeIntent::Replace,
                    value: Some(value),
                    expected_base: Some(ExpectedBase::Value(String::new())),
                },
            );
        }
    }

    Ok(())
}

fn empty_note_type_merge_change() -> NoteTypeChange {
    NoteTypeChange {
        intent: ChangeIntent::Merge,
        note_type: None,
        name: None,
        variables: BTreeMap::new(),
        styling: None,
        fields: BTreeMap::new(),
        card_templates: BTreeMap::new(),
        adapter_ids: BTreeMap::new(),
        expected_base: None,
    }
}

fn empty_note_merge_change() -> NoteChange {
    NoteChange {
        intent: ChangeIntent::Merge,
        note: None,
        variables: BTreeMap::new(),
        fields: BTreeMap::new(),
        tags: BTreeMap::new(),
        adapter_ids: BTreeMap::new(),
        expected_base: None,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TranslationDictionaryYaml {
    #[serde(default)]
    direct: BTreeMap<String, String>,
    #[serde(default)]
    contextual: BTreeMap<String, ContextualTranslationYaml>,
    #[serde(default)]
    no_change: BTreeSet<String>,
    #[serde(default)]
    target_adaptations: BTreeMap<String, TargetAdaptationYaml>,
    #[serde(default)]
    target_additions: BTreeMap<String, String>,
    #[serde(default)]
    stale_translations: Vec<StaleTranslationYaml>,
    #[serde(default)]
    variables: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(default)]
    adapter_ids: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(default)]
    require_complete: bool,
    #[serde(default)]
    ignore_paths: BTreeSet<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetAdaptationYaml {
    #[serde(default)]
    intent: Option<String>,
    #[serde(default)]
    ownership: Option<String>,
    expected_source: String,
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    reason: Option<String>,
}

impl TargetAdaptationYaml {
    fn into_target_adaptation(self, path: &str) -> Result<TargetAdaptation, CanonicalYamlError> {
        let typed_form = self.intent.is_some() || self.ownership.is_some();
        if typed_form
            && (self.intent.is_none() || self.ownership.is_none() || self.reason.is_none())
        {
            return Err(CanonicalYamlError::InvalidTranslationDictionary(format!(
                "target_adaptations.{path}: typed target adaptations require intent, ownership, and a non-blank reason; run brainbrew fmt on a legacy overlay before editing it"
            )));
        }
        if !typed_form && self.target.is_none() {
            return Err(CanonicalYamlError::InvalidTranslationDictionary(format!(
                "target_adaptations.{path}.target: legacy target adaptations require target text; use typed intent: delete for an intentional deletion"
            )));
        }
        let target = self.target.unwrap_or_default();
        let intent = self
            .intent
            .as_deref()
            .map(parse_target_adaptation_intent)
            .transpose()?
            .unwrap_or(if target.is_empty() {
                TargetAdaptationIntent::Delete
            } else {
                TargetAdaptationIntent::Adapt
            });
        let ownership = self
            .ownership
            .as_deref()
            .map(parse_target_adaptation_ownership)
            .transpose()?
            .unwrap_or(TargetAdaptationOwnership::Translation);
        Ok(TargetAdaptation {
            intent,
            ownership,
            expected_source: self.expected_source,
            target,
            reason: self.reason.unwrap_or_else(|| {
                "migrated legacy target adaptation; review and describe its target-language purpose"
                    .to_owned()
            }),
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StaleTranslationYaml {
    old_source: String,
    new_source: String,
    target: String,
    #[serde(default)]
    context: Option<String>,
}

impl StaleTranslationYaml {
    fn into_stale_translation(self) -> StaleTranslation {
        StaleTranslation {
            old_source: self.old_source,
            new_source: self.new_source,
            target: self.target,
            context: self.context,
        }
    }
}

impl TranslationDictionaryYaml {
    fn into_translation_dictionary(self) -> Result<TranslationDictionary, CanonicalYamlError> {
        let mut contextual = BTreeMap::new();
        flatten_contextual_translations(None, self.contextual, &mut contextual)?;
        let mut target_adaptations = self
            .target_adaptations
            .into_iter()
            .map(|(path, adaptation)| Ok((path.clone(), adaptation.into_target_adaptation(&path)?)))
            .collect::<Result<BTreeMap<_, _>, CanonicalYamlError>>()?;
        for (path, target) in self.target_additions {
            if target_adaptations
                .insert(
                    path.clone(),
                    TargetAdaptation {
                        intent: TargetAdaptationIntent::Adapt,
                        ownership: TargetAdaptationOwnership::Translation,
                        expected_source: String::new(),
                        target,
                        reason: "migrated from legacy translations.target_additions; review and describe its target-language purpose".to_owned(),
                    },
                )
                .is_some()
            {
                return Err(CanonicalYamlError::InvalidTranslationDictionary(format!(
                    "target addition {path} duplicates target_adaptations entry"
                )));
            }
        }

        Ok(TranslationDictionary {
            direct: self.direct,
            contextual,
            no_change: self.no_change,
            target_adaptations,
            stale_translations: self
                .stale_translations
                .into_iter()
                .map(StaleTranslationYaml::into_stale_translation)
                .collect(),
            variables: self.variables,
            adapter_ids: self.adapter_ids,
            require_complete: self.require_complete,
            ignore_paths: self.ignore_paths,
        })
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ContextualTranslationYaml {
    Translation(String),
    Nested(BTreeMap<String, ContextualTranslationYaml>),
}

fn flatten_contextual_translations(
    context_path: Option<String>,
    entries: BTreeMap<String, ContextualTranslationYaml>,
    contextual: &mut BTreeMap<String, BTreeMap<String, String>>,
) -> Result<(), CanonicalYamlError> {
    for (key, entry) in entries {
        match entry {
            ContextualTranslationYaml::Translation(translated) => {
                let Some(context_path) = &context_path else {
                    return Err(CanonicalYamlError::InvalidTranslationDictionary(format!(
                        "translations.contextual.{key} has no context path; use translations.direct for reusable translations"
                    )));
                };
                insert_contextual_translation(contextual, context_path, key, translated)?;
            }
            ContextualTranslationYaml::Nested(nested) => {
                let nested_context = context_path
                    .as_ref()
                    .map_or_else(|| key.clone(), |prefix| format!("{prefix}.{key}"));
                flatten_contextual_translations(Some(nested_context), nested, contextual)?;
            }
        }
    }
    Ok(())
}

fn insert_contextual_translation(
    contextual: &mut BTreeMap<String, BTreeMap<String, String>>,
    context_path: &str,
    source: String,
    translated: String,
) -> Result<(), CanonicalYamlError> {
    let replacements = contextual.entry(context_path.to_owned()).or_default();
    if let Some(existing) = replacements.get(&source) {
        if existing != &translated {
            return Err(CanonicalYamlError::InvalidTranslationDictionary(format!(
                "translations.contextual.{context_path}.{source} has conflicting translations"
            )));
        }
        return Ok(());
    }
    replacements.insert(source, translated);
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeckChangeYaml {
    #[serde(default)]
    name: Option<PropertyChangeYaml>,
    #[serde(default)]
    description: Option<PropertyChangeYaml>,
    #[serde(default)]
    variables: BTreeMap<String, PropertyChangeYaml>,
    #[serde(default)]
    adapter_ids: BTreeMap<String, AdapterIdChangeYaml>,
}

impl DeckChangeYaml {
    fn into_deck_change(self) -> Result<DeckChange, CanonicalYamlError> {
        Ok(DeckChange {
            name: self
                .name
                .map(PropertyChangeYaml::into_property_change)
                .transpose()?,
            description: self
                .description
                .map(PropertyChangeYaml::into_property_change)
                .transpose()?,
            variables: self
                .variables
                .into_iter()
                .map(|(key, change)| Ok((key, change.into_property_change()?)))
                .collect::<Result<_, CanonicalYamlError>>()?,
            adapter_ids: self
                .adapter_ids
                .into_iter()
                .map(|(key, change)| Ok((key, change.into_adapter_id_change()?)))
                .collect::<Result<_, CanonicalYamlError>>()?,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NoteTypeChangeYaml {
    intent: String,
    #[serde(default)]
    note_type: Option<NoteTypeYaml>,
    #[serde(default)]
    name: Option<PropertyChangeYaml>,
    #[serde(default)]
    variables: BTreeMap<String, PropertyChangeYaml>,
    #[serde(default)]
    styling: Option<PropertyChangeYaml>,
    #[serde(default)]
    fields: BTreeMap<String, FieldDefinitionChangeYaml>,
    #[serde(default)]
    card_templates: BTreeMap<String, CardTemplateChangeYaml>,
    #[serde(default)]
    adapter_ids: BTreeMap<String, AdapterIdChangeYaml>,
    #[serde(default)]
    expected_base: Option<ExpectedBaseYaml>,
}

impl NoteTypeChangeYaml {
    fn into_note_type_change(self, id: StableId) -> Result<NoteTypeChange, CanonicalYamlError> {
        Ok(NoteTypeChange {
            intent: parse_change_intent(&self.intent)?,
            note_type: self
                .note_type
                .map(|note_type| note_type.into_note_type(id.clone()))
                .transpose()?,
            name: self
                .name
                .map(PropertyChangeYaml::into_property_change)
                .transpose()?,
            variables: self
                .variables
                .into_iter()
                .map(|(key, change)| Ok((key, change.into_property_change()?)))
                .collect::<Result<_, CanonicalYamlError>>()?,
            styling: self
                .styling
                .map(PropertyChangeYaml::into_property_change)
                .transpose()?,
            fields: self
                .fields
                .into_iter()
                .map(|(id, change)| {
                    let id = sid(&id)?;
                    Ok((id.clone(), change.into_field_definition_change(id)?))
                })
                .collect::<Result<_, CanonicalYamlError>>()?,
            card_templates: self
                .card_templates
                .into_iter()
                .map(|(id, change)| {
                    let id = sid(&id)?;
                    Ok((id.clone(), change.into_card_template_change(id)?))
                })
                .collect::<Result<_, CanonicalYamlError>>()?,
            adapter_ids: self
                .adapter_ids
                .into_iter()
                .map(|(key, change)| Ok((key, change.into_adapter_id_change()?)))
                .collect::<Result<_, CanonicalYamlError>>()?,
            expected_base: self
                .expected_base
                .map(ExpectedBaseYaml::into_expected_base)
                .transpose()?,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CardTemplatePayloadYaml {
    name: String,
    #[serde(default)]
    variables: BTreeMap<String, String>,
    question_format: String,
    answer_format: String,
    #[serde(default)]
    adapter_ids: BTreeMap<String, String>,
}

impl CardTemplatePayloadYaml {
    fn into_card_template(self, id: StableId) -> Result<CardTemplate, CanonicalYamlError> {
        Ok(CardTemplate {
            id,
            name: self.name,
            variables: self.variables,
            question_format: self.question_format,
            answer_format: self.answer_format,
            adapter_ids: adapter_ids_from_map(self.adapter_ids),
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CardTemplateChangeYaml {
    intent: String,
    #[serde(default)]
    template: Option<CardTemplatePayloadYaml>,
    #[serde(default)]
    insert_after: Option<String>,
    #[serde(default)]
    name: Option<PropertyChangeYaml>,
    #[serde(default)]
    variables: BTreeMap<String, PropertyChangeYaml>,
    #[serde(default)]
    question_format: Option<PropertyChangeYaml>,
    #[serde(default)]
    answer_format: Option<PropertyChangeYaml>,
    #[serde(default)]
    adapter_ids: BTreeMap<String, AdapterIdChangeYaml>,
    #[serde(default)]
    expected_base: Option<ExpectedBaseYaml>,
}

impl CardTemplateChangeYaml {
    fn into_card_template_change(
        self,
        id: StableId,
    ) -> Result<CardTemplateChange, CanonicalYamlError> {
        Ok(CardTemplateChange {
            intent: parse_change_intent(&self.intent)?,
            template: self
                .template
                .map(|template| template.into_card_template(id))
                .transpose()?,
            insert_after: self.insert_after.map(|id| sid(&id)).transpose()?,
            name: self
                .name
                .map(PropertyChangeYaml::into_property_change)
                .transpose()?,
            variables: self
                .variables
                .into_iter()
                .map(|(key, change)| Ok((key, change.into_property_change()?)))
                .collect::<Result<_, CanonicalYamlError>>()?,
            question_format: self
                .question_format
                .map(PropertyChangeYaml::into_property_change)
                .transpose()?,
            answer_format: self
                .answer_format
                .map(PropertyChangeYaml::into_property_change)
                .transpose()?,
            adapter_ids: self
                .adapter_ids
                .into_iter()
                .map(|(key, change)| Ok((key, change.into_adapter_id_change()?)))
                .collect::<Result<_, CanonicalYamlError>>()?,
            expected_base: self
                .expected_base
                .map(ExpectedBaseYaml::into_expected_base)
                .transpose()?,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PropertyChangeYaml {
    intent: String,
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    expected_base: Option<ExpectedBaseYaml>,
}

impl PropertyChangeYaml {
    fn into_property_change(self) -> Result<PropertyChange, CanonicalYamlError> {
        Ok(PropertyChange {
            intent: parse_change_intent(&self.intent)?,
            value: self.value,
            expected_base: self
                .expected_base
                .map(ExpectedBaseYaml::into_expected_base)
                .transpose()?,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AdapterIdChangeYaml {
    intent: String,
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    expected_base: Option<ExpectedBaseYaml>,
}

impl AdapterIdChangeYaml {
    fn into_adapter_id_change(self) -> Result<AdapterIdChange, CanonicalYamlError> {
        Ok(AdapterIdChange {
            intent: parse_change_intent(&self.intent)?,
            value: self.value,
            expected_base: self
                .expected_base
                .map(ExpectedBaseYaml::into_expected_base)
                .transpose()?,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FieldDefinitionChangeYaml {
    intent: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    message_pattern: Option<MessagePatternYaml>,
    #[serde(default)]
    expected_base: Option<ExpectedBaseYaml>,
}

impl FieldDefinitionChangeYaml {
    fn into_field_definition_change(
        self,
        id: StableId,
    ) -> Result<FieldDefinitionChange, CanonicalYamlError> {
        let message_pattern = self
            .message_pattern
            .map(MessagePatternYaml::into_pattern)
            .transpose()?;
        let field = match (self.name, message_pattern) {
            (Some(name), message_pattern) => Some(FieldDefinition {
                id,
                name,
                message_pattern,
            }),
            (None, None) => None,
            (None, Some(_)) => {
                return Err(CanonicalYamlError::InvalidFieldValue(
                    "field-definition message_pattern requires `name` in a complete field payload"
                        .to_owned(),
                ));
            }
        };
        Ok(FieldDefinitionChange {
            intent: parse_change_intent(&self.intent)?,
            field,
            expected_base: self
                .expected_base
                .map(ExpectedBaseYaml::into_expected_base)
                .transpose()?,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NoteChangeYaml {
    intent: String,
    #[serde(default)]
    note: Option<NoteYaml>,
    #[serde(default)]
    variables: BTreeMap<String, PropertyChangeYaml>,
    #[serde(default)]
    fields: BTreeMap<String, FieldChangeYaml>,
    #[serde(default)]
    tags: BTreeMap<String, TagChangeYaml>,
    #[serde(default)]
    adapter_ids: BTreeMap<String, AdapterIdChangeYaml>,
    #[serde(default)]
    expected_base: Option<ExpectedBaseYaml>,
}

impl NoteChangeYaml {
    fn into_note_change(self, id: StableId) -> Result<NoteChange, CanonicalYamlError> {
        Ok(NoteChange {
            intent: parse_change_intent(&self.intent)?,
            note: self.note.map(|note| note.into_note(id)).transpose()?,
            variables: self
                .variables
                .into_iter()
                .map(|(key, change)| Ok((key, change.into_property_change()?)))
                .collect::<Result<_, CanonicalYamlError>>()?,
            fields: self
                .fields
                .into_iter()
                .map(|(id, change)| {
                    let id = sid(&id)?;
                    Ok((id, change.into_field_change()?))
                })
                .collect::<Result<_, CanonicalYamlError>>()?,
            tags: self
                .tags
                .into_iter()
                .map(|(tag, change)| Ok((tag, change.into_tag_change()?)))
                .collect::<Result<_, CanonicalYamlError>>()?,
            adapter_ids: self
                .adapter_ids
                .into_iter()
                .map(|(key, change)| Ok((key, change.into_adapter_id_change()?)))
                .collect::<Result<_, CanonicalYamlError>>()?,
            expected_base: self
                .expected_base
                .map(ExpectedBaseYaml::into_expected_base)
                .transpose()?,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FieldChangeYaml {
    intent: String,
    #[serde(default)]
    value: Option<FieldValueYaml>,
    #[serde(default)]
    message: Option<Vec<MessageComponentYaml>>,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    variables: BTreeMap<String, MessageComponentYaml>,
    #[serde(default)]
    expected_base: Option<ExpectedBaseYaml>,
}

impl FieldChangeYaml {
    fn into_field_change(self) -> Result<FieldChange, CanonicalYamlError> {
        let message = if let Some(format) = self.format {
            Some(
                FormattedMessageYaml {
                    format,
                    variables: self.variables,
                }
                .into_structured_message(),
            )
        } else {
            self.message
                .map(|message| ComponentMessageYaml { message }.into_structured_message())
        };
        let value = match (self.value, message) {
            (Some(_), Some(_)) => {
                return Err(CanonicalYamlError::InvalidFieldValue(
                    "field change must contain exactly one semantic value".to_owned(),
                ));
            }
            (Some(value), None) => Some(value.into_field_value()?),
            (None, Some(message)) => Some(
                FieldValue::message(message)
                    .map_err(|error| CanonicalYamlError::InvalidFieldValue(error.to_string()))?,
            ),
            (None, None) => None,
        };
        Ok(FieldChange {
            intent: parse_change_intent(&self.intent)?,
            value,
            expected_base: self
                .expected_base
                .map(ExpectedBaseYaml::into_expected_base)
                .transpose()?,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TagChangeYaml {
    intent: String,
    #[serde(default)]
    expected_base: Option<ExpectedBaseYaml>,
}

impl TagChangeYaml {
    fn into_tag_change(self) -> Result<TagChange, CanonicalYamlError> {
        Ok(TagChange {
            intent: parse_change_intent(&self.intent)?,
            expected_base: self
                .expected_base
                .map(ExpectedBaseYaml::into_expected_base)
                .transpose()?,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MediaChangeYaml {
    intent: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    sha256: Option<String>,
    #[serde(default)]
    expected_base: Option<ExpectedBaseYaml>,
}

impl MediaChangeYaml {
    fn into_media_change(self, id: StableId) -> Result<MediaChange, CanonicalYamlError> {
        let media = match (self.path, self.sha256) {
            (Some(path), Some(sha256)) => Some(MediaReference { id, path, sha256 }),
            _ => None,
        };
        Ok(MediaChange {
            intent: parse_change_intent(&self.intent)?,
            media,
            expected_base: self
                .expected_base
                .map(ExpectedBaseYaml::into_expected_base)
                .transpose()?,
        })
    }
}

enum ExpectedBaseYaml {
    Marker(String),
    Value { value: FieldValueYaml },
    Fingerprint(EntityFingerprint),
}

impl<'de> Deserialize<'de> for ExpectedBaseYaml {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        if let Value::String(marker) = value {
            return Ok(Self::Marker(marker));
        }
        let Value::Mapping(mut mapping) = value else {
            return Err(serde::de::Error::custom(
                "expected base must be a single fingerprint or value mapping",
            ));
        };
        if mapping.len() != 1 {
            return Err(serde::de::Error::custom(
                "expected base must contain exactly one value",
            ));
        }
        if let Some(value) = mapping.remove(Value::String("fingerprint".to_owned())) {
            let Value::String(value) = value else {
                return Err(serde::de::Error::custom(
                    "expected base fingerprint must be a canonical string",
                ));
            };
            return value
                .parse::<EntityFingerprint>()
                .map(Self::Fingerprint)
                .map_err(serde::de::Error::custom);
        }
        let Some(value) = mapping.remove(Value::String("value".to_owned())) else {
            return Err(serde::de::Error::custom(
                "expected base mapping must contain fingerprint or value",
            ));
        };
        field_value_from_yaml_value(value)
            .map(|value| Self::Value { value })
            .map_err(serde::de::Error::custom)
    }
}

impl ExpectedBaseYaml {
    fn into_expected_base(self) -> Result<ExpectedBase, CanonicalYamlError> {
        match self {
            Self::Marker(marker) if marker == "entity_present" => Err(
                CanonicalYamlError::InvalidExpectedBase(
                    "entity_present is no longer accepted for destructive changes; use an exact typed value or generate an entity fingerprint with `brainbrew diff --as-overlay`"
                        .to_owned(),
                ),
            ),
            Self::Marker(marker) => Err(CanonicalYamlError::InvalidExpectedBase(marker)),
            Self::Fingerprint(fingerprint) => Ok(ExpectedBase::EntityFingerprint(fingerprint)),
            Self::Value {
                value: FieldValueYaml::Scalar(value),
            } => Ok(ExpectedBase::Value(value)),
            Self::Value { value } => Ok(ExpectedBase::FieldValue(value.into_field_value()?)),
        }
    }
}

fn parse_target_adaptation_intent(
    intent: &str,
) -> Result<TargetAdaptationIntent, CanonicalYamlError> {
    match intent {
        "adapt" => Ok(TargetAdaptationIntent::Adapt),
        "delete" => Ok(TargetAdaptationIntent::Delete),
        _ => Err(CanonicalYamlError::InvalidTranslationDictionary(format!(
            "target adaptation intent must be adapt or delete, found {intent:?}"
        ))),
    }
}

fn parse_target_adaptation_ownership(
    ownership: &str,
) -> Result<TargetAdaptationOwnership, CanonicalYamlError> {
    match ownership {
        "translation" => Ok(TargetAdaptationOwnership::Translation),
        "extension" => Ok(TargetAdaptationOwnership::Extension),
        _ => Err(CanonicalYamlError::InvalidTranslationDictionary(format!(
            "target adaptation ownership must be translation or extension, found {ownership:?}"
        ))),
    }
}

fn parse_overlay_kind(kind: &str) -> Result<OverlayKind, CanonicalYamlError> {
    match kind {
        "translation" => Ok(OverlayKind::Translation),
        "extension" => Ok(OverlayKind::Extension),
        "patch" => Ok(OverlayKind::Patch),
        "personal" => Ok(OverlayKind::Personal),
        other => Err(CanonicalYamlError::InvalidOverlayKind(other.to_owned())),
    }
}

fn parse_change_intent(intent: &str) -> Result<ChangeIntent, CanonicalYamlError> {
    match intent {
        "add" => Ok(ChangeIntent::Add),
        "merge" => Ok(ChangeIntent::Merge),
        "replace" => Ok(ChangeIntent::Replace),
        "remove" => Ok(ChangeIntent::Remove),
        "override" => Ok(ChangeIntent::Override),
        other => Err(CanonicalYamlError::InvalidChangeIntent(other.to_owned())),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CanonicalDeckYaml {
    deck: DeckYaml,
    note_types: BTreeMap<String, NoteTypeYaml>,
    notes: BTreeMap<String, NoteYaml>,
    #[serde(default)]
    media: BTreeMap<String, MediaYaml>,
    #[serde(default)]
    tombstones: Vec<TombstoneYaml>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TombstoneYaml {
    Legacy(String),
    Typed(TypedTombstoneYaml),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TypedTombstoneYaml {
    kind: String,
    path: String,
    #[serde(default)]
    removed_by: Option<String>,
    #[serde(default)]
    operation: Option<String>,
}

impl CanonicalDeckYaml {
    fn into_deck(self) -> Result<CanonicalDeck, CanonicalYamlError> {
        let note_types = self
            .note_types
            .into_iter()
            .map(|(id, note_type)| {
                let stable_id = sid(&id)?;
                Ok((stable_id.clone(), note_type.into_note_type(stable_id)?))
            })
            .collect::<Result<BTreeMap<_, _>, CanonicalYamlError>>()?;
        let notes = self
            .notes
            .into_iter()
            .map(|(id, note)| {
                let stable_id = sid(&id)?;
                Ok((stable_id.clone(), note.into_note(stable_id)?))
            })
            .collect::<Result<BTreeMap<_, _>, CanonicalYamlError>>()?;
        let media = self
            .media
            .into_iter()
            .map(|(id, media)| {
                let stable_id = sid(&id)?;
                Ok((stable_id.clone(), media.into_media(stable_id)))
            })
            .collect::<Result<BTreeMap<_, _>, CanonicalYamlError>>()?;
        let tombstones = parse_tombstones(self.tombstones, &note_types, &notes, &media)?;
        Ok(CanonicalDeck {
            id: sid(&self.deck.id)?,
            name: self.deck.name,
            description: self.deck.description,
            variables: self.deck.variables,
            note_types,
            notes,
            media,
            tombstones,
            adapter_ids: adapter_ids_from_map(self.deck.adapter_ids),
        })
    }
}

fn parse_tombstones(
    values: Vec<TombstoneYaml>,
    note_types: &BTreeMap<StableId, NoteType>,
    notes: &BTreeMap<StableId, Note>,
    media: &BTreeMap<StableId, MediaReference>,
) -> Result<Tombstones, CanonicalYamlError> {
    let mut tombstones = Tombstones::default();
    for (index, value) in values.into_iter().enumerate() {
        let record = match value {
            TombstoneYaml::Legacy(value) => {
                let id = sid(&value)?;
                let mut matches = Vec::new();
                if note_types.contains_key(&id) {
                    matches.push(TombstoneAddress::NoteType {
                        note_type_id: id.clone(),
                    });
                }
                if notes.contains_key(&id) {
                    matches.push(TombstoneAddress::Note {
                        note_id: id.clone(),
                    });
                }
                if media.contains_key(&id) {
                    matches.push(TombstoneAddress::MediaReference {
                        media_id: id.clone(),
                    });
                }
                if matches.len() != 1 {
                    let detail = if matches.is_empty() {
                        "matches no retained top-level note, note type, or media identity"
                            .to_owned()
                    } else {
                        format!(
                            "matches multiple top-level kinds: {}",
                            matches
                                .iter()
                                .map(TombstoneAddress::kind)
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    };
                    return Err(CanonicalYamlError::InvalidSchemaValue {
                        path: format!("tombstones.{index}"),
                        message: format!(
                            "legacy bare tombstone {value:?} {detail}; replace it with an explicit typed record containing `kind` and full `path` (nested field/template ownership is never inferred)"
                        ),
                    });
                }
                TombstoneRecord::legacy(matches.pop().expect("one legacy match"))
            }
            TombstoneYaml::Typed(value) => {
                let path = value.path.parse::<DeckPath>().map_err(|error| {
                    CanonicalYamlError::InvalidSchemaValue {
                        path: format!("tombstones.{index}.path"),
                        message: error.to_string(),
                    }
                })?;
                let address = TombstoneAddress::try_from(path).map_err(|_| {
                    CanonicalYamlError::InvalidSchemaValue {
                        path: format!("tombstones.{index}.path"),
                        message: "path is not an exact removable entity/value address".to_owned(),
                    }
                })?;
                if value.kind != address.kind() {
                    return Err(CanonicalYamlError::InvalidSchemaValue {
                        path: format!("tombstones.{index}.kind"),
                        message: format!(
                            "kind {:?} does not match typed path kind {:?}",
                            value.kind,
                            address.kind()
                        ),
                    });
                }
                let provenance = match (value.removed_by, value.operation) {
                    (None, None) => None,
                    (Some(overlay_id), Some(operation)) => {
                        let operation = parse_change_intent(&operation)?;
                        if operation != ChangeIntent::Remove {
                            return Err(CanonicalYamlError::InvalidSchemaValue {
                                path: format!("tombstones.{index}.operation"),
                                message: "tombstone provenance operation must be `remove`"
                                    .to_owned(),
                            });
                        }
                        Some(RemovalProvenance {
                            overlay_id: sid(&overlay_id)?,
                            operation,
                        })
                    }
                    _ => {
                        return Err(CanonicalYamlError::InvalidSchemaValue {
                            path: format!("tombstones.{index}"),
                            message: "`removed_by` and `operation` must be supplied together"
                                .to_owned(),
                        });
                    }
                };
                TombstoneRecord {
                    address,
                    provenance,
                }
            }
        };
        if tombstones.insert(record).is_some() {
            return Err(CanonicalYamlError::InvalidSchemaValue {
                path: format!("tombstones.{index}"),
                message: "duplicate typed tombstone address".to_owned(),
            });
        }
    }
    Ok(tombstones)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeckYaml {
    id: String,
    name: String,
    description: String,
    #[serde(default)]
    variables: BTreeMap<String, String>,
    #[serde(default)]
    adapter_ids: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NoteTypeYaml {
    name: String,
    #[serde(default)]
    variables: BTreeMap<String, String>,
    field_order: Vec<String>,
    fields: BTreeMap<String, FieldYaml>,
    card_template_order: Vec<String>,
    card_templates: BTreeMap<String, CardTemplateYaml>,
    styling: String,
    #[serde(default)]
    adapter_ids: BTreeMap<String, String>,
}

impl NoteTypeYaml {
    fn into_note_type(self, id: StableId) -> Result<NoteType, CanonicalYamlError> {
        let fields = ordered_values("fields", self.fields, self.field_order, |id, field| {
            Ok(FieldDefinition {
                id,
                name: field.name,
                message_pattern: field
                    .message_pattern
                    .map(MessagePatternYaml::into_pattern)
                    .transpose()?,
            })
        })?;
        let card_templates = ordered_values(
            "card_templates",
            self.card_templates,
            self.card_template_order,
            |id, template| {
                Ok(CardTemplate {
                    id,
                    name: template.name,
                    variables: template.variables,
                    question_format: template.question_format,
                    answer_format: template.answer_format,
                    adapter_ids: adapter_ids_from_map(template.adapter_ids),
                })
            },
        )?;

        Ok(NoteType {
            id,
            name: self.name,
            variables: self.variables,
            fields,
            card_templates,
            styling: self.styling,
            adapter_ids: adapter_ids_from_map(self.adapter_ids),
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FieldYaml {
    name: String,
    #[serde(default)]
    message_pattern: Option<MessagePatternYaml>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MessagePatternYaml {
    kind: String,
    item_format: String,
    separator: String,
    parameters: BTreeMap<String, MessagePatternParameterYaml>,
}

impl MessagePatternYaml {
    fn into_pattern(self) -> Result<ListMessagePattern, CanonicalYamlError> {
        if self.kind != "list" {
            return Err(CanonicalYamlError::InvalidFieldValue(format!(
                "unsupported message_pattern kind {:?}; expected `list`",
                self.kind
            )));
        }
        let parameters = self
            .parameters
            .into_iter()
            .map(|(name, parameter)| Ok((name, parameter.into_parameter()?)))
            .collect::<Result<_, CanonicalYamlError>>()?;
        Ok(ListMessagePattern {
            item_format: self.item_format,
            separator: self.separator,
            parameters,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MessagePatternParameterYaml {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    field: Option<String>,
}

impl MessagePatternParameterYaml {
    fn into_parameter(self) -> Result<ListMessageParameter, CanonicalYamlError> {
        match (self.kind.as_str(), self.field) {
            ("text", None) => Ok(ListMessageParameter::Text),
            ("note_field_ref", Some(field_id)) => Ok(ListMessageParameter::NoteFieldRef {
                field_id: sid(&field_id)?,
            }),
            ("text", Some(_)) => Err(CanonicalYamlError::InvalidFieldValue(
                "text message_pattern parameter must not declare `field`".to_owned(),
            )),
            ("note_field_ref", None) => Err(CanonicalYamlError::InvalidFieldValue(
                "note_field_ref message_pattern parameter requires `field`".to_owned(),
            )),
            (kind, _) => Err(CanonicalYamlError::InvalidFieldValue(format!(
                "unsupported message_pattern parameter type {kind:?}"
            ))),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CardTemplateYaml {
    name: String,
    #[serde(default)]
    variables: BTreeMap<String, String>,
    question_format: String,
    answer_format: String,
    #[serde(default)]
    adapter_ids: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NoteYaml {
    note_type_id: String,
    #[serde(default)]
    variables: BTreeMap<String, String>,
    fields: BTreeMap<String, FieldValueYaml>,
    #[serde(default)]
    tags: BTreeSet<String>,
    #[serde(default)]
    adapter_ids: BTreeMap<String, String>,
}

impl NoteYaml {
    fn into_note(self, id: StableId) -> Result<Note, CanonicalYamlError> {
        let fields = self
            .fields
            .into_iter()
            .map(|(field_id, value)| Ok((sid(&field_id)?, value.into_field_value()?)))
            .collect::<Result<_, CanonicalYamlError>>()?;
        Ok(Note {
            id,
            note_type_id: sid(&self.note_type_id)?,
            variables: self.variables,
            fields,
            tags: self.tags,
            adapter_ids: adapter_ids_from_map(self.adapter_ids),
        })
    }
}

enum FieldValueYaml {
    Scalar(String),
    Images(ImageReferencesYaml),
    Formatted(FormattedMessageYaml),
    Message(ComponentMessageYaml),
    MessageItems(ListMessageItemsYaml),
}

impl FieldValueYaml {
    fn into_field_value(self) -> Result<FieldValue, CanonicalYamlError> {
        match self {
            Self::Scalar(value) => Ok(FieldValue::Scalar(value)),
            Self::Images(images) => FieldValue::images(images.into_images()?)
                .map_err(|error| CanonicalYamlError::InvalidFieldValue(error.to_string())),
            Self::Formatted(message) => FieldValue::message(message.into_structured_message())
                .map_err(|error| CanonicalYamlError::InvalidFieldValue(error.to_string())),
            Self::Message(message) => FieldValue::message(message.into_structured_message())
                .map_err(|error| CanonicalYamlError::InvalidFieldValue(error.to_string())),
            Self::MessageItems(message) => Ok(FieldValue::MessageItems(ListMessageItems::new(
                message
                    .items
                    .into_iter()
                    .map(|item| {
                        item.into_iter()
                            .map(|(name, argument)| (name, argument.into_argument()))
                            .collect()
                    })
                    .collect(),
            ))),
        }
    }
}

struct ImageReferencesYaml(Vec<String>);

impl ImageReferencesYaml {
    fn into_images(self) -> Result<Vec<FieldImageReference>, CanonicalYamlError> {
        self.0
            .into_iter()
            .map(|media_id| {
                Ok(FieldImageReference {
                    media_id: sid(&media_id)?,
                })
            })
            .collect()
    }
}

impl<'de> Deserialize<'de> for FieldValueYaml {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        field_value_from_yaml_value(value).map_err(serde::de::Error::custom)
    }
}

fn field_value_from_yaml_value(value: Value) -> Result<FieldValueYaml, String> {
    if let Some(media_id) = image_scalar_from_yaml_value(&value)? {
        return Ok(FieldValueYaml::Images(ImageReferencesYaml(vec![media_id])));
    }

    if let Value::Sequence(sequence) = &value {
        if sequence.is_empty() {
            return Err("!image field sequence must not be empty".to_owned());
        }
        let mut images = Vec::new();
        for item in sequence {
            let Some(media_id) = image_scalar_from_yaml_value(item)? else {
                return Err(
                    "field image sequences may contain only !image tagged scalars".to_owned(),
                );
            };
            images.push(media_id);
        }
        return Ok(FieldValueYaml::Images(ImageReferencesYaml(images)));
    }

    if matches!(value, Value::Tagged(_)) {
        return Err("unsupported YAML tag in field value".to_owned());
    }

    if let Ok(value) = serde_yaml::from_value::<String>(value.clone()) {
        return Ok(FieldValueYaml::Scalar(value));
    }
    if let Ok(message) = serde_yaml::from_value::<FormattedMessageYaml>(value.clone()) {
        return Ok(FieldValueYaml::Formatted(message));
    }
    if let Ok(message) = serde_yaml::from_value::<ComponentMessageYaml>(value.clone()) {
        return Ok(FieldValueYaml::Message(message));
    }
    if let Ok(message) = serde_yaml::from_value::<ListMessageItemsYaml>(value) {
        return Ok(FieldValueYaml::MessageItems(message));
    }
    Err(
        "field value must be a scalar string, structured message, list message items, or !image reference"
            .to_owned(),
    )
}

fn image_scalar_from_yaml_value(value: &Value) -> Result<Option<String>, String> {
    let Value::Tagged(tagged) = value else {
        return Ok(None);
    };
    if tagged.tag != "image" {
        return Err(format!(
            "unsupported YAML tag {} in field value",
            tagged.tag
        ));
    }
    let Value::String(media_id) = &tagged.value else {
        return Err("!image value must be a scalar string".to_owned());
    };
    if media_id.is_empty() {
        return Err("!image value must not be empty".to_owned());
    }
    Ok(Some(media_id.clone()))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListMessageItemsYaml {
    items: Vec<BTreeMap<String, ListMessageArgumentYaml>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ListMessageArgumentYaml {
    Scalar(String),
    ExplicitText(ExplicitListMessageTextYaml),
}

impl ListMessageArgumentYaml {
    fn into_argument(self) -> ListMessageArgument {
        match self {
            Self::Scalar(value) => ListMessageArgument::Scalar(value),
            Self::ExplicitText(value) => ListMessageArgument::Text(value.text),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExplicitListMessageTextYaml {
    text: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ComponentMessageYaml {
    message: Vec<MessageComponentYaml>,
}

impl ComponentMessageYaml {
    fn into_structured_message(self) -> StructuredMessage {
        StructuredMessage {
            components: self
                .message
                .into_iter()
                .map(MessageComponentYaml::into_component)
                .collect(),
            format: None,
            variables: BTreeMap::new(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FormattedMessageYaml {
    format: String,
    variables: BTreeMap<String, MessageComponentYaml>,
}

impl FormattedMessageYaml {
    fn into_structured_message(self) -> StructuredMessage {
        StructuredMessage {
            components: Vec::new(),
            format: Some(self.format),
            variables: self
                .variables
                .into_iter()
                .map(|(name, component)| (name, component.into_component()))
                .collect(),
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum MessageComponentYaml {
    Literal(LiteralComponentYaml),
    Text(TextComponentYaml),
    Reference(ReferenceComponentYaml),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LiteralComponentYaml {
    literal: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TextComponentYaml {
    text: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReferenceComponentYaml {
    #[serde(rename = "ref")]
    reference: String,
}

impl MessageComponentYaml {
    fn into_component(self) -> MessageComponent {
        match self {
            Self::Literal(component) => MessageComponent::Literal(component.literal),
            Self::Text(component) => MessageComponent::Text(component.text),
            Self::Reference(component) => MessageComponent::FieldRef(component.reference),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MediaYaml {
    path: String,
    sha256: String,
}

impl MediaYaml {
    fn into_media(self, id: StableId) -> MediaReference {
        MediaReference {
            id,
            path: self.path,
            sha256: self.sha256,
        }
    }
}

fn ordered_values<T, U>(
    section: &'static str,
    mut values: BTreeMap<String, T>,
    order: Vec<String>,
    convert: impl Fn(StableId, T) -> Result<U, CanonicalYamlError>,
) -> Result<Vec<U>, CanonicalYamlError> {
    let mut ordered = Vec::new();
    for id in order {
        let Some(value) = values.remove(&id) else {
            return Err(CanonicalYamlError::MissingOrderedEntity { section, id });
        };
        ordered.push(convert(sid(&id)?, value)?);
    }

    if let Some(id) = values.into_keys().next() {
        return Err(CanonicalYamlError::UnorderedEntity { section, id });
    }

    Ok(ordered)
}

fn adapter_ids_from_map(map: BTreeMap<String, String>) -> AdapterIds {
    let mut adapter_ids = AdapterIds::new();
    for (key, value) in map {
        adapter_ids.insert(key, value);
    }
    adapter_ids
}

fn sid(value: &str) -> Result<StableId, InvalidStableId> {
    StableId::new(value)
}
