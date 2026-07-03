use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write as _};

use brain_brew_core::{
    AdapterIdChange, AdapterIds, CanonicalDeck, CardTemplate, CardTemplateChange, ChangeIntent,
    DeckChange, ExpectedBase, FieldChange, FieldDefinition, FieldDefinitionChange, InvalidStableId,
    MediaChange, MediaReference, MessageComponent, Note, NoteChange, NoteType, NoteTypeChange,
    Overlay, OverlayKind, PropertyChange, StableId, StaleTranslation, StructuredMessage, TagChange,
    TargetAdaptation, TranslationDictionary, ValidationReport,
};
use serde::Deserialize;

const UG_TARGET_ADDITION_REASON: &str = "target addition from upstream UG";

/// Parse a CanonicalDeck from strict canonical YAML.
pub fn from_str(input: &str) -> Result<CanonicalDeck, CanonicalYamlError> {
    let file: CanonicalDeckYaml = serde_yaml::from_str(input).map_err(CanonicalYamlError::Parse)?;
    let deck = file.into_deck()?;
    deck.validate().map_err(CanonicalYamlError::Validation)?;
    Ok(deck)
}

/// Parse a sparse overlay YAML file.
pub fn overlay_from_str(input: &str) -> Result<Overlay, CanonicalYamlError> {
    let file: OverlayYaml = serde_yaml::from_str(input).map_err(CanonicalYamlError::Parse)?;
    file.into_overlay()
}

/// Parse and re-emit a CanonicalDeck YAML file using deterministic formatting.
pub fn format_str(input: &str) -> Result<String, CanonicalYamlError> {
    let deck = from_str(input)?;
    to_string(&deck)
}

/// Parse and re-emit a sparse overlay YAML file using deterministic formatting.
pub fn overlay_format_str(input: &str) -> Result<String, CanonicalYamlError> {
    let overlay = overlay_from_str(input)?;
    Ok(overlay_to_string(&overlay))
}

/// Emit a CanonicalDeck as deterministic canonical YAML.
pub fn to_string(deck: &CanonicalDeck) -> Result<String, CanonicalYamlError> {
    deck.validate().map_err(CanonicalYamlError::Validation)?;

    let mut out = String::new();
    writeln!(out, "deck:").expect("writing to a string cannot fail");
    writeln!(out, "  id: {}", deck.id).expect("writing to a string cannot fail");
    writeln!(out, "  name: {}", yaml_scalar(&deck.name)).expect("writing to a string cannot fail");
    write_multiline_or_scalar(&mut out, "  ", "description", &deck.description);
    write_variables(&mut out, "  ", &deck.variables);
    write_adapter_ids(&mut out, "  ", &deck.adapter_ids);

    writeln!(out, "note_types:").expect("writing to a string cannot fail");
    for (id, note_type) in &deck.note_types {
        writeln!(out, "  {id}:").expect("writing to a string cannot fail");
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
            writeln!(out, "      {field_id}:").expect("writing to a string cannot fail");
            writeln!(out, "        name: {}", yaml_scalar(&field.name))
                .expect("writing to a string cannot fail");
        }
        writeln!(out, "    card_template_order:").expect("writing to a string cannot fail");
        for template in &note_type.card_templates {
            writeln!(out, "      - {}", template.id).expect("writing to a string cannot fail");
        }
        writeln!(out, "    card_templates:").expect("writing to a string cannot fail");
        let templates_by_id = note_type
            .card_templates
            .iter()
            .map(|template| (&template.id, template))
            .collect::<BTreeMap<_, _>>();
        for (template_id, template) in templates_by_id {
            writeln!(out, "      {template_id}:").expect("writing to a string cannot fail");
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

    writeln!(out, "notes:").expect("writing to a string cannot fail");
    for (id, note) in &deck.notes {
        writeln!(out, "  {id}:").expect("writing to a string cannot fail");
        writeln!(out, "    note_type_id: {}", note.note_type_id)
            .expect("writing to a string cannot fail");
        write_variables(&mut out, "    ", &note.variables);
        writeln!(out, "    fields:").expect("writing to a string cannot fail");
        for (field_id, value) in &note.fields {
            if let Some(message) = note.field_messages.get(field_id) {
                write_structured_message_field(&mut out, "      ", field_id, message);
            } else {
                writeln!(out, "      {field_id}: {}", yaml_scalar(value))
                    .expect("writing to a string cannot fail");
            }
        }
        writeln!(out, "    tags:").expect("writing to a string cannot fail");
        for tag in &note.tags {
            writeln!(out, "      - {}", yaml_scalar(tag)).expect("writing to a string cannot fail");
        }
        write_adapter_ids(&mut out, "    ", &note.adapter_ids);
    }

    writeln!(out, "media:").expect("writing to a string cannot fail");
    for (id, media) in &deck.media {
        writeln!(out, "  {id}:").expect("writing to a string cannot fail");
        writeln!(out, "    path: {}", yaml_scalar(&media.path))
            .expect("writing to a string cannot fail");
        writeln!(out, "    sha256: {}", yaml_scalar(&media.sha256))
            .expect("writing to a string cannot fail");
    }

    if deck.tombstones.is_empty() {
        writeln!(out, "tombstones: []").expect("writing to a string cannot fail");
    } else {
        writeln!(out, "tombstones:").expect("writing to a string cannot fail");
        for tombstone in &deck.tombstones {
            writeln!(out, "  - {tombstone}").expect("writing to a string cannot fail");
        }
    }

    Ok(out)
}

/// Emit a sparse overlay as deterministic YAML.
pub fn overlay_to_string(overlay: &Overlay) -> String {
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
        let (target_additions, target_adaptations) =
            split_target_additions_for_format(&translations.target_adaptations);
        let has_top_level_translation_data =
            !target_adaptations.is_empty() || !translations.stale_translations.is_empty();
        if !translation_dictionary_is_empty(translations)
            || !target_additions.is_empty()
            || !has_top_level_translation_data
        {
            write_translation_dictionary(&mut out, translations, &target_additions);
        }
        write_target_adaptations(&mut out, &target_adaptations);
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
            writeln!(out, "  {id}:").expect("writing to a string cannot fail");
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
                    writeln!(out, "      {field_id}:").expect("writing to a string cannot fail");
                    writeln!(
                        out,
                        "        intent: {}",
                        change_intent_name(field_change.intent)
                    )
                    .expect("writing to a string cannot fail");
                    if let Some(field) = &field_change.field {
                        writeln!(out, "        name: {}", yaml_scalar(&field.name))
                            .expect("writing to a string cannot fail");
                    }
                    if let Some(expected_base) = &field_change.expected_base {
                        write_expected_base(&mut out, "        ", expected_base);
                    }
                }
            }
            if !change.card_templates.is_empty() {
                writeln!(out, "    card_templates:").expect("writing to a string cannot fail");
                for (template_id, template_change) in &change.card_templates {
                    writeln!(out, "      {template_id}:").expect("writing to a string cannot fail");
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
            writeln!(out, "  {id}:").expect("writing to a string cannot fail");
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
                    writeln!(out, "      {}:", yaml_scalar(tag))
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
            writeln!(out, "  {id}:").expect("writing to a string cannot fail");
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

    out
}

#[derive(Default)]
struct FieldAdditionsForFormat {
    fields: BTreeMap<StableId, String>,
    values: BTreeMap<StableId, BTreeMap<StableId, String>>,
}

#[derive(Clone)]
enum FieldFillForFormat {
    Scalar(String),
    Message(StructuredMessage),
}

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
                let Some(value) = &field_change.value else {
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
                    .insert(field_id.clone(), value.clone());
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
    BTreeMap<StableId, BTreeMap<StableId, FieldFillForFormat>>,
    BTreeMap<StableId, NoteChange>,
) {
    let mut field_fills = BTreeMap::<StableId, BTreeMap<StableId, FieldFillForFormat>>::new();

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

fn field_fill_value(change: &FieldChange) -> Option<FieldFillForFormat> {
    if change.intent == ChangeIntent::Replace
        && matches!(change.expected_base, Some(ExpectedBase::Value(ref value)) if value.is_empty())
    {
        if let Some(message) = &change.message {
            Some(FieldFillForFormat::Message(message.clone()))
        } else {
            change
                .value
                .as_ref()
                .map(|value| FieldFillForFormat::Scalar(value.clone()))
        }
    } else {
        None
    }
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
        writeln!(out, "  {note_type_id}:").expect("writing to a string cannot fail");
        writeln!(out, "    fields:").expect("writing to a string cannot fail");
        for (field_id, name) in &additions.fields {
            writeln!(out, "      {field_id}: {}", yaml_scalar(name))
                .expect("writing to a string cannot fail");
        }
        if !additions.values.is_empty() {
            writeln!(out, "    values:").expect("writing to a string cannot fail");
            for (note_id, values) in &additions.values {
                writeln!(out, "      {note_id}:").expect("writing to a string cannot fail");
                for (field_id, value) in values {
                    writeln!(out, "        {field_id}: {}", yaml_scalar(value))
                        .expect("writing to a string cannot fail");
                }
            }
        }
    }
}

fn write_field_fills(
    out: &mut String,
    field_fills: &BTreeMap<StableId, BTreeMap<StableId, FieldFillForFormat>>,
) {
    writeln!(out, "field_fills:").expect("writing to a string cannot fail");
    for (note_id, fields) in field_fills {
        writeln!(out, "  {note_id}:").expect("writing to a string cannot fail");
        for (field_id, value) in fields {
            match value {
                FieldFillForFormat::Scalar(value) => {
                    writeln!(out, "    {field_id}: {}", yaml_scalar(value))
                        .expect("writing to a string cannot fail");
                }
                FieldFillForFormat::Message(message) => {
                    write_structured_message_field(out, "    ", field_id, message);
                }
            }
        }
    }
}

fn write_translation_dictionary(
    out: &mut String,
    translations: &TranslationDictionary,
    target_additions: &BTreeMap<String, String>,
) {
    if translation_dictionary_is_empty(translations) && target_additions.is_empty() {
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
    if !target_additions.is_empty() {
        writeln!(out, "  target_additions:").expect("writing to a string cannot fail");
        for (path, target) in target_additions {
            writeln!(out, "    {}: {}", yaml_scalar(path), yaml_scalar(target))
                .expect("writing to a string cannot fail");
        }
    }
    if !translations.variables.is_empty() {
        writeln!(out, "  variables:").expect("writing to a string cannot fail");
        for (variable_key, replacements) in &translations.variables {
            writeln!(out, "    {variable_key}:").expect("writing to a string cannot fail");
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
            writeln!(out, "    {adapter_key}:").expect("writing to a string cannot fail");
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

fn split_target_additions_for_format(
    target_adaptations: &BTreeMap<String, TargetAdaptation>,
) -> (BTreeMap<String, String>, BTreeMap<String, TargetAdaptation>) {
    let mut target_additions = BTreeMap::new();
    let mut remaining_target_adaptations = BTreeMap::new();
    for (path, adaptation) in target_adaptations {
        if adaptation.expected_source.is_empty()
            && adaptation.reason.as_deref() == Some(UG_TARGET_ADDITION_REASON)
        {
            target_additions.insert(path.clone(), adaptation.target.clone());
        } else {
            remaining_target_adaptations.insert(path.clone(), adaptation.clone());
        }
    }
    (target_additions, remaining_target_adaptations)
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
        writeln!(out, "  {}:", yaml_scalar(path)).expect("writing to a string cannot fail");
        writeln!(
            out,
            "    expected_source: {}",
            yaml_scalar(&adaptation.expected_source)
        )
        .expect("writing to a string cannot fail");
        writeln!(out, "    target: {}", yaml_scalar(&adaptation.target))
            .expect("writing to a string cannot fail");
        if let Some(reason) = &adaptation.reason {
            writeln!(out, "    reason: {}", yaml_scalar(reason))
                .expect("writing to a string cannot fail");
        }
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
        writeln!(out, "{indent}{}:", yaml_scalar(key)).expect("writing to a string cannot fail");
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
    writeln!(out, "{indent}{key}:").expect("writing to a string cannot fail");
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
        writeln!(out, "{indent}  {key}: {}", yaml_scalar(value))
            .expect("writing to a string cannot fail");
    }
}

fn write_property_change(out: &mut String, indent: &str, key: &str, change: &PropertyChange) {
    writeln!(out, "{indent}{key}:").expect("writing to a string cannot fail");
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
        writeln!(out, "{indent}  {key}:").expect("writing to a string cannot fail");
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
    writeln!(out, "{indent}{field_id}:").expect("writing to a string cannot fail");
    writeln!(
        out,
        "{indent}  intent: {}",
        change_intent_name(change.intent)
    )
    .expect("writing to a string cannot fail");
    if let Some(value) = &change.value {
        write_multiline_or_scalar(out, &format!("{indent}  "), "value", value);
    }
    if let Some(message) = &change.message {
        write_structured_message_value(out, &format!("{indent}  "), message);
    }
    if let Some(expected_base) = &change.expected_base {
        write_expected_base(out, &format!("{indent}  "), expected_base);
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
        writeln!(out, "{indent}  {field_id}:").expect("writing to a string cannot fail");
        writeln!(out, "{indent}    name: {}", yaml_scalar(&field.name))
            .expect("writing to a string cannot fail");
    }
    writeln!(out, "{indent}card_template_order:").expect("writing to a string cannot fail");
    for template in &note_type.card_templates {
        writeln!(out, "{indent}  - {}", template.id).expect("writing to a string cannot fail");
    }
    writeln!(out, "{indent}card_templates:").expect("writing to a string cannot fail");
    let templates_by_id = note_type
        .card_templates
        .iter()
        .map(|template| (&template.id, template))
        .collect::<BTreeMap<_, _>>();
    for (template_id, template) in templates_by_id {
        writeln!(out, "{indent}  {template_id}:").expect("writing to a string cannot fail");
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
        if let Some(message) = note.field_messages.get(field_id) {
            write_structured_message_field(out, &format!("{indent}  "), field_id, message);
        } else {
            writeln!(out, "{indent}  {field_id}: {}", yaml_scalar(value))
                .expect("writing to a string cannot fail");
        }
    }
    writeln!(out, "{indent}tags:").expect("writing to a string cannot fail");
    for tag in &note.tags {
        writeln!(out, "{indent}  - {}", yaml_scalar(tag)).expect("writing to a string cannot fail");
    }
    write_adapter_ids(out, indent, &note.adapter_ids);
}

fn write_structured_message_field(
    out: &mut String,
    indent: &str,
    field_id: &StableId,
    message: &StructuredMessage,
) {
    writeln!(out, "{indent}{field_id}:").expect("writing to a string cannot fail");
    write_structured_message_value(out, &format!("{indent}  "), message);
}

fn write_structured_message_value(out: &mut String, indent: &str, message: &StructuredMessage) {
    if let Some(format) = &message.format {
        writeln!(out, "{indent}format: {}", yaml_scalar(format))
            .expect("writing to a string cannot fail");
        writeln!(out, "{indent}variables:").expect("writing to a string cannot fail");
        for (name, component) in &message.variables {
            writeln!(out, "{indent}  {name}:").expect("writing to a string cannot fail");
            write_message_component(out, &format!("{indent}    "), component, false);
        }
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
        ExpectedBase::EntityPresent => {
            writeln!(out, "{indent}expected_base: entity_present")
                .expect("writing to a string cannot fail");
        }
        ExpectedBase::Value(value) => {
            writeln!(out, "{indent}expected_base:").expect("writing to a string cannot fail");
            write_multiline_or_scalar(out, &format!("{indent}  "), "value", value);
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

fn change_intent_name(intent: ChangeIntent) -> &'static str {
    match intent {
        ChangeIntent::Add => "add",
        ChangeIntent::Merge => "merge",
        ChangeIntent::Replace => "replace",
        ChangeIntent::Remove => "remove",
        ChangeIntent::Override => "override",
    }
}

fn write_multiline_or_scalar(out: &mut String, indent: &str, key: &str, value: &str) {
    if value.contains('\n') {
        let chomp = if value.ends_with('\n') { "|" } else { "|-" };
        writeln!(out, "{indent}{key}: {chomp}").expect("writing to a string cannot fail");
        for line in value.lines() {
            writeln!(out, "{indent}  {line}").expect("writing to a string cannot fail");
        }
    } else {
        writeln!(out, "{indent}{key}: {}", yaml_scalar(value))
            .expect("writing to a string cannot fail");
    }
}

fn yaml_scalar(value: &str) -> String {
    if can_emit_plain_scalar(value) {
        value.to_owned()
    } else {
        format!("'{}'", value.replace('\'', "''"))
    }
}

fn can_emit_plain_scalar(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with([
            ' ', '-', '?', ':', '@', '`', '&', '*', '!', '|', '>', '#', '{', '[', ',',
        ])
        && !value.ends_with(' ')
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '.' | ',' | '_' | '-' | '/'))
        && !value.chars().all(|ch| ch.is_ascii_digit())
        && !matches!(
            value,
            "true" | "false" | "True" | "False" | "TRUE" | "FALSE" | "null" | "Null" | "NULL"
        )
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
    MissingOrderedEntity { section: &'static str, id: String },
    UnorderedEntity { section: &'static str, id: String },
    Validation(ValidationReport),
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
            Self::MissingOrderedEntity { section, id } => {
                write!(f, "{section} order references missing entity {id}")
            }
            Self::UnorderedEntity { section, id } => {
                write!(f, "{section} entity {id} is missing from its order array")
            }
            Self::Validation(report) => write!(f, "canonical deck validation failed: {report}"),
        }
    }
}

impl std::error::Error for CanonicalYamlError {}

impl From<InvalidStableId> for CanonicalYamlError {
    fn from(error: InvalidStableId) -> Self {
        Self::StableId(error)
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
            translations
                .target_adaptations
                .insert(path, adaptation.into_target_adaptation());
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
    values: BTreeMap<String, BTreeMap<String, String>>,
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
                    field: Some(FieldDefinition { id: field_id, name }),
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
                note_change.fields.insert(
                    field_id,
                    FieldChange {
                        intent: ChangeIntent::Add,
                        value: Some(value),
                        message: None,
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
            let (value, message) = match value {
                FieldValueYaml::Scalar(value) => (Some(value), None),
                FieldValueYaml::Formatted(message) => {
                    (None, Some(message.into_structured_message()))
                }
                FieldValueYaml::Message(message) => (None, Some(message.into_structured_message())),
            };
            note_change.fields.insert(
                field_id,
                FieldChange {
                    intent: ChangeIntent::Replace,
                    value,
                    message,
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
    expected_source: String,
    target: String,
    #[serde(default)]
    reason: Option<String>,
}

impl TargetAdaptationYaml {
    fn into_target_adaptation(self) -> TargetAdaptation {
        TargetAdaptation {
            expected_source: self.expected_source,
            target: self.target,
            reason: self.reason,
        }
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
            .map(|(path, adaptation)| (path, adaptation.into_target_adaptation()))
            .collect::<BTreeMap<_, _>>();
        for (path, target) in self.target_additions {
            if target_adaptations
                .insert(
                    path.clone(),
                    TargetAdaptation {
                        expected_source: String::new(),
                        target,
                        reason: Some(UG_TARGET_ADDITION_REASON.to_owned()),
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
    expected_base: Option<ExpectedBaseYaml>,
}

impl FieldDefinitionChangeYaml {
    fn into_field_definition_change(
        self,
        id: StableId,
    ) -> Result<FieldDefinitionChange, CanonicalYamlError> {
        Ok(FieldDefinitionChange {
            intent: parse_change_intent(&self.intent)?,
            field: self.name.map(|name| FieldDefinition { id, name }),
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
    value: Option<String>,
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
        Ok(FieldChange {
            intent: parse_change_intent(&self.intent)?,
            value: self.value,
            message,
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

#[derive(Deserialize)]
#[serde(untagged)]
enum ExpectedBaseYaml {
    Marker(String),
    Value { value: String },
}

impl ExpectedBaseYaml {
    fn into_expected_base(self) -> Result<ExpectedBase, CanonicalYamlError> {
        match self {
            Self::Marker(marker) if marker == "entity_present" => Ok(ExpectedBase::EntityPresent),
            Self::Marker(marker) => Err(CanonicalYamlError::InvalidExpectedBase(marker)),
            Self::Value { value } => Ok(ExpectedBase::Value(value)),
        }
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
    tombstones: Vec<String>,
}

impl CanonicalDeckYaml {
    fn into_deck(self) -> Result<CanonicalDeck, CanonicalYamlError> {
        let deck = CanonicalDeck {
            id: sid(&self.deck.id)?,
            name: self.deck.name,
            description: self.deck.description,
            variables: self.deck.variables,
            note_types: self
                .note_types
                .into_iter()
                .map(|(id, note_type)| {
                    let stable_id = sid(&id)?;
                    Ok((stable_id.clone(), note_type.into_note_type(stable_id)?))
                })
                .collect::<Result<_, CanonicalYamlError>>()?,
            notes: self
                .notes
                .into_iter()
                .map(|(id, note)| {
                    let stable_id = sid(&id)?;
                    Ok((stable_id.clone(), note.into_note(stable_id)?))
                })
                .collect::<Result<_, CanonicalYamlError>>()?,
            media: self
                .media
                .into_iter()
                .map(|(id, media)| {
                    let stable_id = sid(&id)?;
                    Ok((stable_id.clone(), media.into_media(stable_id)))
                })
                .collect::<Result<_, CanonicalYamlError>>()?,
            tombstones: self
                .tombstones
                .into_iter()
                .map(|id| sid(&id))
                .collect::<Result<BTreeSet<_>, _>>()?,
            adapter_ids: adapter_ids_from_map(self.deck.adapter_ids),
        };
        deck.resolve_structured_messages()
            .map_err(CanonicalYamlError::Validation)
    }
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
        let mut fields = BTreeMap::new();
        let mut field_messages = BTreeMap::new();
        for (field_id, value) in self.fields {
            let field_id = sid(&field_id)?;
            match value {
                FieldValueYaml::Scalar(value) => {
                    fields.insert(field_id, value);
                }
                FieldValueYaml::Formatted(message) => {
                    fields.insert(field_id.clone(), String::new());
                    field_messages.insert(field_id, message.into_structured_message());
                }
                FieldValueYaml::Message(message) => {
                    fields.insert(field_id.clone(), String::new());
                    field_messages.insert(field_id, message.into_structured_message());
                }
            }
        }
        Ok(Note {
            id,
            note_type_id: sid(&self.note_type_id)?,
            variables: self.variables,
            fields,
            field_messages,
            tags: self.tags,
            adapter_ids: adapter_ids_from_map(self.adapter_ids),
        })
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum FieldValueYaml {
    Scalar(String),
    Formatted(FormattedMessageYaml),
    Message(ComponentMessageYaml),
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
