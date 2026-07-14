use std::collections::{BTreeMap, BTreeSet};

use crate::compose::record_change_path;
use crate::messages::{
    lower_images_from_deck, message_component_path, message_format_path, message_variable_path,
    render_message_format,
};
use crate::*;

impl CanonicalDeck {
    /// Report translation coverage for one translation overlay without modifying this deck.
    pub fn translation_coverage(
        &self,
        overlay: &Overlay,
    ) -> Result<TranslationCoverageReport, FieldGraphReport> {
        let Some(translations) = overlay.translations.as_ref() else {
            return Ok(TranslationCoverageReport {
                overlay_id: overlay.id.clone(),
                entries: Vec::new(),
            });
        };
        translation_coverage_report(self, overlay, translations)
    }

    /// Resolve translation coverage across an ordered stack without format or filesystem input.
    ///
    /// A unit keeps the overlay that introduced its current source text, while its
    /// completeness is evaluated only after every overlay has been applied.
    pub fn translation_stack_coverage(
        &self,
        overlays: &[Overlay],
    ) -> Result<TranslationStackCoverageReport, ComposeReport> {
        let target_stack = overlays
            .iter()
            .map(|overlay| overlay.id.clone())
            .collect::<Vec<_>>();
        let mut current = self.clone();
        let mut snapshot =
            translation_unit_snapshot(&current).map_err(stack_coverage_graph_error)?;
        let mut states = snapshot
            .iter()
            .map(|(path, source)| {
                (
                    path.clone(),
                    StackTranslationUnit {
                        owner: TranslationUnitOwner::Base,
                        source_text: source.clone(),
                        old_source_text: None,
                        status: TranslationStackCoverageStatus::UntranslatedFallback,
                        resolved_by: None,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut deleted = Vec::new();

        for overlay in overlays {
            if overlay.translations.is_some() {
                let report = current
                    .translation_coverage(overlay)
                    .map_err(stack_coverage_graph_error)?;
                for entry in report.entries {
                    let Some(status) = stack_status(entry.category) else {
                        continue;
                    };
                    if let Some(unit) = states.get_mut(&entry.path) {
                        set_stack_resolution(unit, status, entry.old_source, &overlay.id);
                    } else if status == TranslationStackCoverageStatus::Stale {
                        // A shadowed stale record has a dictionary path rather than a
                        // unit path. It remains review debt for the active source unit.
                        for (path, unit) in &mut states {
                            if unit.source_text == entry.source
                                && entry
                                    .context
                                    .as_deref()
                                    .is_none_or(|context| context_matches_path(context, path))
                            {
                                set_stack_resolution(
                                    unit,
                                    status,
                                    entry.old_source.clone(),
                                    &overlay.id,
                                );
                            }
                        }
                    }
                }
            }

            current = current.compose(std::slice::from_ref(overlay))?;
            let next_snapshot =
                translation_unit_snapshot(&current).map_err(stack_coverage_graph_error)?;

            // Dictionary application changes target text, never source ownership. All
            // other overlays own source units that they add or replace.
            if overlay.translations.is_none() {
                for (path, source) in &next_snapshot {
                    if snapshot.get(path) != Some(source) {
                        states.insert(
                            path.clone(),
                            StackTranslationUnit {
                                owner: TranslationUnitOwner::Overlay(overlay.id.clone()),
                                source_text: source.clone(),
                                old_source_text: None,
                                status: TranslationStackCoverageStatus::UntranslatedFallback,
                                resolved_by: None,
                            },
                        );
                    }
                }
                for path in snapshot
                    .keys()
                    .filter(|path| !next_snapshot.contains_key(*path))
                {
                    if let Some(unit) = states.remove(path) {
                        deleted.push(TranslationStackCoverageEntry {
                            status: TranslationStackCoverageStatus::Deleted,
                            owner: unit.owner,
                            source_path: path.clone(),
                            source_text: unit.source_text,
                            old_source_text: unit.old_source_text,
                            translated_text: None,
                            resolved_by: Some(overlay.id.clone()),
                            target_stack: target_stack.clone(),
                        });
                    }
                }
            }
            snapshot = next_snapshot;
        }

        let mut entries = states
            .into_iter()
            .filter_map(|(path, unit)| {
                snapshot
                    .get(&path)
                    .map(|target| TranslationStackCoverageEntry {
                        status: unit.status,
                        owner: unit.owner,
                        source_path: path,
                        source_text: unit.source_text,
                        old_source_text: unit.old_source_text,
                        translated_text: Some(target.clone()),
                        resolved_by: unit.resolved_by,
                        target_stack: target_stack.clone(),
                    })
            })
            .collect::<Vec<_>>();
        entries.append(&mut deleted);
        entries.sort_by(|left, right| {
            left.source_path
                .cmp(&right.source_path)
                .then_with(|| left.status.cmp(&right.status))
                .then_with(|| left.source_text.cmp(&right.source_text))
        });
        Ok(TranslationStackCoverageReport {
            target_stack,
            entries,
        })
    }

    /// Build translator-facing note/field/card context for one coverage report.
    pub fn translation_context(
        &self,
        report: &TranslationCoverageReport,
    ) -> Result<TranslationContextView, FieldGraphReport> {
        translation_context_view(self, report)
    }
}

#[derive(Clone)]
struct StackTranslationUnit {
    owner: TranslationUnitOwner,
    source_text: String,
    old_source_text: Option<String>,
    status: TranslationStackCoverageStatus,
    resolved_by: Option<StableId>,
}

fn set_stack_resolution(
    unit: &mut StackTranslationUnit,
    status: TranslationStackCoverageStatus,
    old_source_text: Option<String>,
    overlay_id: &StableId,
) {
    if status != TranslationStackCoverageStatus::UntranslatedFallback
        || unit.status == TranslationStackCoverageStatus::UntranslatedFallback
    {
        unit.status = status;
        unit.old_source_text = old_source_text;
        unit.resolved_by = Some(overlay_id.clone());
    }
}

fn translation_unit_snapshot(
    deck: &CanonicalDeck,
) -> Result<BTreeMap<String, String>, FieldGraphReport> {
    let overlay = Overlay {
        id: StableId::new("overlay.translation.coverage-snapshot")
            .expect("static coverage overlay id is valid"),
        kind: OverlayKind::Translation,
        translations: Some(TranslationDictionary::default()),
        deck_change: None,
        note_changes: BTreeMap::new(),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    };
    Ok(deck
        .translation_coverage(&overlay)?
        .entries
        .into_iter()
        .filter(|entry| {
            matches!(
                entry.category,
                TranslationCoverageCategory::UntranslatedFallback
                    | TranslationCoverageCategory::IgnoredSource
            )
        })
        .map(|entry| (entry.path, entry.source))
        .collect())
}

fn stack_coverage_graph_error(report: FieldGraphReport) -> ComposeReport {
    let error = report.errors.first();
    ComposeReport {
        errors: vec![ComposeError::new(
            ComposeErrorKind::ValidationFailed,
            error
                .map(|entry| entry.consuming_path.clone())
                .unwrap_or_default(),
            format!("translation stack coverage could not resolve source fields: {report}"),
        )],
    }
}

fn stack_status(category: TranslationCoverageCategory) -> Option<TranslationStackCoverageStatus> {
    match category {
        TranslationCoverageCategory::UntranslatedFallback => {
            Some(TranslationStackCoverageStatus::UntranslatedFallback)
        }
        TranslationCoverageCategory::DirectTranslation => {
            Some(TranslationStackCoverageStatus::Direct)
        }
        TranslationCoverageCategory::ContextualTranslation => {
            Some(TranslationStackCoverageStatus::Contextual)
        }
        TranslationCoverageCategory::NoChange => Some(TranslationStackCoverageStatus::NoChange),
        TranslationCoverageCategory::StaleTranslation => {
            Some(TranslationStackCoverageStatus::Stale)
        }
        TranslationCoverageCategory::TargetAdaptation => {
            Some(TranslationStackCoverageStatus::Adaptation)
        }
        TranslationCoverageCategory::VariableTranslation => {
            Some(TranslationStackCoverageStatus::Variable)
        }
        TranslationCoverageCategory::IgnoredSource => {
            Some(TranslationStackCoverageStatus::HiddenExcluded)
        }
        _ => None,
    }
}

fn translation_coverage_report(
    deck: &CanonicalDeck,
    overlay: &Overlay,
    translations: &TranslationDictionary,
) -> Result<TranslationCoverageReport, FieldGraphReport> {
    let resolved = deck.resolve_field_graph(|note_id, field_id, images| {
        lower_images_from_deck(deck, note_id, field_id, images)
    })?;
    let mut builder = TranslationCoverageBuilder {
        resolved: &resolved,
        translations,
        entries: Vec::new(),
        seen_sources: BTreeSet::new(),
        seen_direct: BTreeSet::new(),
        seen_contextual: BTreeSet::new(),
        seen_no_change: BTreeSet::new(),
        seen_target_adaptations: BTreeSet::new(),
        seen_stale_translations: BTreeSet::new(),
        seen_variables: BTreeSet::new(),
        seen_adapter_ids: BTreeSet::new(),
        source_paths: BTreeMap::new(),
        structural_field_paths: structural_field_paths(deck),
    };

    if deck
        .tombstones
        .blocking(&TombstoneAddress::DeckName)
        .is_none()
    {
        builder.record_string(&deck.name, DeckPath::DeckName.to_string(), None);
    }
    if deck
        .tombstones
        .blocking(&TombstoneAddress::DeckDescription)
        .is_none()
    {
        builder.record_string(
            &deck.description,
            DeckPath::DeckDescription.to_string(),
            None,
        );
    }
    builder.record_variables(&deck.variables, &DeckPath::DeckVariables.to_string());
    builder.record_adapter_ids(&deck.adapter_ids, &DeckPath::DeckAdapterIds.to_string());

    for (note_type_id, note_type) in &deck.note_types {
        if deck
            .tombstones
            .blocking(&TombstoneAddress::NoteType {
                note_type_id: note_type_id.clone(),
            })
            .is_some()
        {
            continue;
        }
        builder.record_string(
            &note_type.name,
            DeckPath::NoteTypeName {
                note_type_id: note_type_id.clone(),
            }
            .to_string(),
            None,
        );
        builder.record_variables(
            &note_type.variables,
            &DeckPath::NoteTypeVariables {
                note_type_id: note_type_id.clone(),
            }
            .to_string(),
        );
        // Anki field-definition names are structural identifiers. Localized
        // display labels belong in source variables, not translation dictionaries.
        for template in &note_type.card_templates {
            if deck
                .tombstones
                .blocking(&TombstoneAddress::CardTemplate {
                    note_type_id: note_type_id.clone(),
                    template_id: template.id.clone(),
                })
                .is_some()
            {
                continue;
            }
            builder.record_string(
                &template.name,
                DeckPath::NoteTypeCardTemplateName {
                    note_type_id: note_type_id.clone(),
                    template_id: template.id.clone(),
                }
                .to_string(),
                None,
            );
            builder.record_variables(
                &template.variables,
                &DeckPath::NoteTypeCardTemplateVariables {
                    note_type_id: note_type_id.clone(),
                    template_id: template.id.clone(),
                }
                .to_string(),
            );
            builder.record_adapter_ids(
                &template.adapter_ids,
                &DeckPath::NoteTypeCardTemplateAdapterIds {
                    note_type_id: note_type_id.clone(),
                    template_id: template.id.clone(),
                }
                .to_string(),
            );
        }
        builder.record_adapter_ids(
            &note_type.adapter_ids,
            &DeckPath::NoteTypeAdapterIds {
                note_type_id: note_type_id.clone(),
            }
            .to_string(),
        );
    }

    for (note_id, note) in &deck.notes {
        if deck
            .tombstones
            .blocking(&TombstoneAddress::Note {
                note_id: note_id.clone(),
            })
            .is_some()
        {
            continue;
        }
        builder.record_variables(
            &note.variables,
            &DeckPath::NoteVariables {
                note_id: note_id.clone(),
            }
            .to_string(),
        );
        for (field_id, value) in &note.fields {
            if deck
                .tombstones
                .blocking(&TombstoneAddress::NoteField {
                    note_id: note_id.clone(),
                    field_id: field_id.clone(),
                })
                .is_some()
            {
                continue;
            }
            let path = DeckPath::NoteField {
                note_id: note_id.clone(),
                field_id: field_id.clone(),
            }
            .to_string();
            match value {
                FieldValue::Scalar(value) => builder.record_string(value, path, None),
                FieldValue::Message(message) => {
                    let rendered = resolved
                        .get(&path)
                        .expect("every planned message has a resolved value");
                    builder.record_message(rendered, path, note_id, field_id, message);
                }
                FieldValue::Images(_) => {}
            }
        }
        builder.record_tags(
            &note.tags,
            &DeckPath::NoteTags {
                note_id: note_id.clone(),
            }
            .to_string(),
        );
        builder.record_adapter_ids(
            &note.adapter_ids,
            &DeckPath::NoteAdapterIds {
                note_id: note_id.clone(),
            }
            .to_string(),
        );
    }

    Ok(builder.finish(overlay.id.clone()))
}

fn structural_field_paths(deck: &CanonicalDeck) -> BTreeMap<String, BTreeSet<String>> {
    let mut paths = BTreeMap::new();
    for (note_type_id, note_type) in &deck.note_types {
        if deck
            .tombstones
            .blocking(&TombstoneAddress::NoteType {
                note_type_id: note_type_id.clone(),
            })
            .is_some()
        {
            continue;
        }
        for field in &note_type.fields {
            if deck
                .tombstones
                .blocking(&TombstoneAddress::FieldDefinition {
                    note_type_id: note_type_id.clone(),
                    field_id: field.id.clone(),
                })
                .is_none()
            {
                paths
                    .entry(field.name.clone())
                    .or_insert_with(BTreeSet::new)
                    .insert(
                        DeckPath::NoteTypeFieldName {
                            note_type_id: note_type_id.clone(),
                            field_id: field.id.clone(),
                        }
                        .to_string(),
                    );
            }
        }
    }
    paths
}

fn is_structural_field_name_path(path: &str) -> bool {
    matches!(path.parse(), Ok(DeckPath::NoteTypeFieldName { .. }))
}

struct TranslationCoverageBuilder<'a> {
    resolved: &'a ResolvedFieldGraph,
    translations: &'a TranslationDictionary,
    entries: Vec<TranslationCoverageEntry>,
    seen_sources: BTreeSet<String>,
    seen_direct: BTreeSet<String>,
    seen_contextual: BTreeSet<(String, String)>,
    seen_no_change: BTreeSet<String>,
    seen_target_adaptations: BTreeSet<String>,
    seen_stale_translations: BTreeSet<usize>,
    seen_variables: BTreeSet<(String, String)>,
    seen_adapter_ids: BTreeSet<(String, String)>,
    source_paths: BTreeMap<String, BTreeSet<String>>,
    structural_field_paths: BTreeMap<String, BTreeSet<String>>,
}

#[derive(Clone, Copy)]
struct TranslationResolveOptions<'a> {
    path: &'a str,
    variable_key: Option<&'a str>,
    include_target_adaptation: bool,
    include_variable: bool,
    include_ignored: bool,
}

#[derive(Clone, Copy)]
struct ContextualTranslationMatch<'a> {
    context_path: &'a str,
    translated: &'a str,
}

#[derive(Clone, Copy)]
struct TranslationRecordOptions {
    include_target_adaptation: bool,
    include_variable: bool,
    record_missing: bool,
    record_ignored: bool,
}

enum TranslationOutcome<'a> {
    TargetAdaptation {
        adaptation: &'a TargetAdaptation,
    },
    Variable {
        translated: &'a str,
    },
    Direct {
        translated: &'a str,
    },
    Contextual {
        translated: &'a str,
        context_path: &'a str,
        direct: Option<&'a str>,
        matches: Vec<ContextualTranslationMatch<'a>>,
    },
    NoChange,
    Stale {
        index: usize,
        record: &'a StaleTranslation,
    },
    Missing,
    Ignored,
    Empty,
}

impl<'a> TranslationOutcome<'a> {
    fn direct_translation(&self) -> Option<&'a str> {
        match self {
            Self::Direct { translated } => Some(translated),
            Self::Contextual { direct, .. } => *direct,
            _ => None,
        }
    }

    fn contextual_matches(&self) -> &[ContextualTranslationMatch<'a>] {
        match self {
            Self::Contextual { matches, .. } => matches,
            _ => &[],
        }
    }
}

fn resolve_translation<'a>(
    translations: &'a TranslationDictionary,
    source: &str,
    options: TranslationResolveOptions<'_>,
) -> TranslationOutcome<'a> {
    if options.include_target_adaptation
        && let Some(adaptation) = translations.target_adaptations.get(options.path)
    {
        return TranslationOutcome::TargetAdaptation { adaptation };
    }

    if source.is_empty() {
        return TranslationOutcome::Empty;
    }

    if options.include_ignored && is_ignored_translation_path(translations, options.path) {
        return TranslationOutcome::Ignored;
    }

    if options.include_variable
        && let Some(variable_key) = options.variable_key
        && let Some(replacements) = translations.variables.get(variable_key)
        && let Some(translated) = replacements.get(source)
    {
        return TranslationOutcome::Variable { translated };
    }

    let direct = translations.direct.get(source).map(String::as_str);
    let mut contextual_matches = Vec::new();
    let mut contextual_translation: Option<ContextualTranslationMatch<'a>> = None;
    for (context_path, replacements) in &translations.contextual {
        if context_matches_path(context_path, options.path)
            && let Some(translated) = replacements.get(source)
        {
            let candidate = ContextualTranslationMatch {
                context_path,
                translated,
            };
            contextual_matches.push(candidate);
            if contextual_translation
                .as_ref()
                .is_none_or(|current| context_path.len() > current.context_path.len())
            {
                contextual_translation = Some(candidate);
            }
        }
    }

    if let Some(contextual_translation) = contextual_translation {
        return TranslationOutcome::Contextual {
            translated: contextual_translation.translated,
            context_path: contextual_translation.context_path,
            direct,
            matches: contextual_matches,
        };
    }

    if let Some(translated) = direct {
        return TranslationOutcome::Direct { translated };
    }

    if translations.no_change.contains(source) {
        return TranslationOutcome::NoChange;
    }

    if let Some((index, record)) = matching_stale_translation(translations, source, options.path) {
        return TranslationOutcome::Stale { index, record };
    }

    TranslationOutcome::Missing
}

fn has_explicit_string_entry(
    translations: &TranslationDictionary,
    value: &str,
    path: &str,
    variable_key: Option<&str>,
) -> bool {
    !matches!(
        resolve_translation(
            translations,
            value,
            TranslationResolveOptions {
                path,
                variable_key,
                include_target_adaptation: false,
                include_variable: true,
                include_ignored: false,
            },
        ),
        TranslationOutcome::Empty | TranslationOutcome::Ignored | TranslationOutcome::Missing
    )
}

impl TranslationCoverageBuilder<'_> {
    fn record_variables(&mut self, variables: &BTreeMap<String, String>, path_prefix: &str) {
        for (key, value) in variables {
            self.record_string(value, map_entry_path(path_prefix, key), Some(key));
        }
    }

    fn record_message(
        &mut self,
        resolved_value: &str,
        path: String,
        note_id: &StableId,
        field_id: &StableId,
        message: &StructuredMessage,
    ) {
        if self.translations.target_adaptations.contains_key(&path)
            || has_explicit_string_entry(self.translations, resolved_value, &path, None)
        {
            self.record_string(resolved_value, path, None);
            return;
        }

        if resolved_value.is_empty() {
            return;
        }

        if is_ignored_translation_path(self.translations, &path) {
            self.entries.push(TranslationCoverageEntry {
                category: TranslationCoverageCategory::IgnoredSource,
                path,
                source: resolved_value.to_owned(),
                old_source: None,
                translated: None,
                context: None,
            });
            return;
        }

        if let Some(format) = &message.format {
            self.record_string_without_fallback(format, message_format_path(note_id, field_id));
            for (variable, component) in &message.variables {
                self.record_message_component(
                    component,
                    message_variable_path(note_id, field_id, variable),
                );
            }
        } else {
            for (index, component) in message.components.iter().enumerate() {
                self.record_message_component(
                    component,
                    message_component_path(note_id, field_id, index),
                );
            }
        }
    }

    fn record_message_component(&mut self, component: &MessageComponent, path: String) {
        match component {
            MessageComponent::Literal(_) => {}
            MessageComponent::Text(value) => {
                self.record_string(value, path, None);
            }
            MessageComponent::FieldRef(reference) => {
                let value = self
                    .resolved
                    .get(reference)
                    .expect("message references were validated by the shared field graph");
                self.record_string(value, path, None);
            }
        }
    }

    fn record_string(&mut self, value: &str, path: String, variable_key: Option<&str>) {
        self.record_string_with_options(
            value,
            path,
            variable_key,
            TranslationRecordOptions {
                include_target_adaptation: true,
                include_variable: true,
                record_missing: true,
                record_ignored: true,
            },
        );
    }

    fn record_string_without_fallback(&mut self, value: &str, path: String) {
        self.record_string_with_options(
            value,
            path,
            None,
            TranslationRecordOptions {
                include_target_adaptation: false,
                include_variable: false,
                record_missing: true,
                record_ignored: false,
            },
        );
    }

    fn record_string_with_options(
        &mut self,
        value: &str,
        path: String,
        variable_key: Option<&str>,
        record_options: TranslationRecordOptions,
    ) {
        let outcome = resolve_translation(
            self.translations,
            value,
            TranslationResolveOptions {
                path: &path,
                variable_key,
                include_target_adaptation: record_options.include_target_adaptation,
                include_variable: record_options.include_variable,
                include_ignored: true,
            },
        );

        match &outcome {
            TranslationOutcome::TargetAdaptation { adaptation } => {
                self.seen_target_adaptations.insert(path.clone());
                let category = if value == adaptation.expected_source {
                    TranslationCoverageCategory::TargetAdaptation
                } else {
                    TranslationCoverageCategory::InvalidTargetAdaptation
                };
                self.entries.push(TranslationCoverageEntry {
                    category,
                    path: path.clone(),
                    source: value.to_owned(),
                    old_source: Some(adaptation.expected_source.clone()),
                    translated: Some(adaptation.target.clone()),
                    context: Some(path),
                });
            }
            TranslationOutcome::Empty => {}
            TranslationOutcome::Ignored => {
                if record_options.record_ignored {
                    self.entries.push(TranslationCoverageEntry {
                        category: TranslationCoverageCategory::IgnoredSource,
                        path,
                        source: value.to_owned(),
                        old_source: None,
                        translated: None,
                        context: None,
                    });
                }
            }
            TranslationOutcome::Variable { translated } => {
                if let Some(variable_key) = variable_key {
                    self.seen_variables
                        .insert((variable_key.to_owned(), value.to_owned()));
                    self.entries.push(TranslationCoverageEntry {
                        category: TranslationCoverageCategory::VariableTranslation,
                        path,
                        source: value.to_owned(),
                        old_source: None,
                        translated: Some((*translated).to_owned()),
                        context: Some(variable_key.to_owned()),
                    });
                }
            }
            TranslationOutcome::Contextual {
                translated,
                context_path,
                ..
            } => {
                let source = self.record_seen_source(value, &path, &outcome);
                self.entries.push(TranslationCoverageEntry {
                    category: TranslationCoverageCategory::ContextualTranslation,
                    path,
                    source,
                    old_source: None,
                    translated: Some((*translated).to_owned()),
                    context: Some((*context_path).to_owned()),
                });
            }
            TranslationOutcome::Direct { translated } => {
                let source = self.record_seen_source(value, &path, &outcome);
                self.entries.push(TranslationCoverageEntry {
                    category: TranslationCoverageCategory::DirectTranslation,
                    path,
                    source,
                    old_source: None,
                    translated: Some((*translated).to_owned()),
                    context: None,
                });
            }
            TranslationOutcome::NoChange => {
                let source = self.record_seen_source(value, &path, &outcome);
                self.seen_no_change.insert(source.clone());
                self.entries.push(TranslationCoverageEntry {
                    category: TranslationCoverageCategory::NoChange,
                    path,
                    source: source.clone(),
                    old_source: None,
                    translated: Some(source),
                    context: None,
                });
            }
            TranslationOutcome::Stale { index, record } => {
                let source = self.record_seen_source(value, &path, &outcome);
                self.seen_stale_translations.insert(*index);
                self.entries.push(TranslationCoverageEntry {
                    category: TranslationCoverageCategory::StaleTranslation,
                    path,
                    source,
                    old_source: Some(record.old_source.clone()),
                    translated: Some(record.target.clone()),
                    context: record.context.clone(),
                });
            }
            TranslationOutcome::Missing => {
                let source = self.record_seen_source(value, &path, &outcome);
                if record_options.record_missing {
                    self.entries.push(TranslationCoverageEntry {
                        category: TranslationCoverageCategory::UntranslatedFallback,
                        path,
                        source: source.clone(),
                        old_source: None,
                        translated: Some(source),
                        context: None,
                    });
                }
            }
        }
    }

    fn record_seen_source(
        &mut self,
        value: &str,
        path: &str,
        outcome: &TranslationOutcome<'_>,
    ) -> String {
        let source = value.to_owned();
        self.seen_sources.insert(source.clone());
        self.source_paths
            .entry(source.clone())
            .or_default()
            .insert(path.to_owned());
        if outcome.direct_translation().is_some() {
            self.seen_direct.insert(source.clone());
        }
        for contextual_match in outcome.contextual_matches() {
            self.seen_contextual
                .insert((contextual_match.context_path.to_owned(), source.clone()));
        }
        source
    }

    fn record_tags(&mut self, tags: &BTreeSet<String>, path_prefix: &str) {
        for tag in tags {
            self.record_string(tag, map_entry_path(path_prefix, tag), None);
        }
    }

    fn record_adapter_ids(&mut self, adapter_ids: &AdapterIds, path_prefix: &str) {
        for (key, value) in adapter_ids.iter() {
            let Some(replacements) = self.translations.adapter_ids.get(key) else {
                continue;
            };
            let Some(translated) = replacements.get(value) else {
                continue;
            };
            self.seen_adapter_ids
                .insert((key.to_owned(), value.to_owned()));
            self.entries.push(TranslationCoverageEntry {
                category: TranslationCoverageCategory::AdapterIdTranslation,
                path: map_entry_path(path_prefix, key),
                source: value.to_owned(),
                old_source: None,
                translated: Some(translated.clone()),
                context: Some(key.to_owned()),
            });
        }
    }

    fn shadowing_translation_for_stale(&self, record: &StaleTranslation) -> Option<String> {
        let paths = self.source_paths.get(&record.new_source)?;
        paths.iter().find_map(|path| {
            if record
                .context
                .as_deref()
                .is_some_and(|context| !context_matches_path(context, path))
            {
                return None;
            }
            match resolve_translation(
                self.translations,
                &record.new_source,
                TranslationResolveOptions {
                    path,
                    variable_key: None,
                    include_target_adaptation: false,
                    include_variable: false,
                    include_ignored: false,
                },
            ) {
                TranslationOutcome::Direct { translated }
                | TranslationOutcome::Contextual { translated, .. }
                | TranslationOutcome::Variable { translated } => Some(translated.to_owned()),
                TranslationOutcome::NoChange => Some(record.new_source.clone()),
                _ => None,
            }
        })
    }

    fn finish(mut self, overlay_id: StableId) -> TranslationCoverageReport {
        for (source, translated) in &self.translations.direct {
            if let Some(paths) = self.structural_field_paths.get(source) {
                self.entries.push(structural_field_name_translation_entry(
                    format!("translations.direct.{source}"),
                    source,
                    translated,
                    paths.iter().cloned().collect(),
                ));
            } else if !self.seen_direct.contains(source) {
                self.entries.push(TranslationCoverageEntry {
                    category: TranslationCoverageCategory::StaleDirectKey,
                    path: format!("translations.direct.{source}"),
                    source: source.clone(),
                    old_source: None,
                    translated: Some(translated.clone()),
                    context: None,
                });
            }
        }
        for (context_path, replacements) in &self.translations.contextual {
            for (source, translated) in replacements {
                if is_structural_field_name_path(context_path) {
                    self.entries.push(structural_field_name_translation_entry(
                        format!("translations.contextual.{context_path}.{source}"),
                        source,
                        translated,
                        vec![context_path.clone()],
                    ));
                } else if !self
                    .seen_contextual
                    .contains(&(context_path.clone(), source.clone()))
                {
                    self.entries.push(TranslationCoverageEntry {
                        category: TranslationCoverageCategory::StaleContextualKey,
                        path: format!("translations.contextual.{context_path}.{source}"),
                        source: source.clone(),
                        old_source: None,
                        translated: Some(translated.clone()),
                        context: Some(context_path.clone()),
                    });
                }
            }
        }
        for source in &self.translations.no_change {
            if let Some(paths) = self.structural_field_paths.get(source) {
                self.entries.push(structural_field_name_translation_entry(
                    format!("translations.no_change.{source}"),
                    source,
                    source,
                    paths.iter().cloned().collect(),
                ));
            } else if !self.seen_sources.contains(source) {
                self.entries.push(TranslationCoverageEntry {
                    category: TranslationCoverageCategory::StaleNoChangeKey,
                    path: format!("translations.no_change.{source}"),
                    source: source.clone(),
                    old_source: None,
                    translated: Some(source.clone()),
                    context: None,
                });
            }
        }
        for (path, adaptation) in &self.translations.target_adaptations {
            if is_structural_field_name_path(path) {
                self.entries.push(structural_field_name_translation_entry(
                    format!("target_adaptations.{path}"),
                    &adaptation.expected_source,
                    &adaptation.target,
                    vec![path.clone()],
                ));
            } else if !self.seen_target_adaptations.contains(path) {
                self.entries.push(TranslationCoverageEntry {
                    category: TranslationCoverageCategory::StaleTargetAdaptation,
                    path: format!("target_adaptations.{path}"),
                    source: adaptation.expected_source.clone(),
                    old_source: None,
                    translated: Some(adaptation.target.clone()),
                    context: Some(path.clone()),
                });
            }
        }
        for (index, record) in self.translations.stale_translations.iter().enumerate() {
            if !self.seen_stale_translations.contains(&index) {
                let translated = self
                    .shadowing_translation_for_stale(record)
                    .unwrap_or_else(|| record.target.clone());
                self.entries.push(TranslationCoverageEntry {
                    category: TranslationCoverageCategory::StaleTranslation,
                    path: format!("translations.stale_translations.{index}"),
                    source: record.new_source.clone(),
                    old_source: Some(record.old_source.clone()),
                    translated: Some(translated),
                    context: record.context.clone(),
                });
            }
        }
        for (variable_key, replacements) in &self.translations.variables {
            for (source, translated) in replacements {
                if !self
                    .seen_variables
                    .contains(&(variable_key.clone(), source.clone()))
                {
                    self.entries.push(TranslationCoverageEntry {
                        category: TranslationCoverageCategory::StaleVariableKey,
                        path: format!("translations.variables.{variable_key}.{source}"),
                        source: source.clone(),
                        old_source: None,
                        translated: Some(translated.clone()),
                        context: Some(variable_key.clone()),
                    });
                }
            }
        }
        for (adapter_key, replacements) in &self.translations.adapter_ids {
            for (source, translated) in replacements {
                if !self
                    .seen_adapter_ids
                    .contains(&(adapter_key.clone(), source.clone()))
                {
                    self.entries.push(TranslationCoverageEntry {
                        category: TranslationCoverageCategory::StaleAdapterIdKey,
                        path: format!("translations.adapter_ids.{adapter_key}.{source}"),
                        source: source.clone(),
                        old_source: None,
                        translated: Some(translated.clone()),
                        context: Some(adapter_key.clone()),
                    });
                }
            }
        }

        self.entries.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then_with(|| left.category.as_str().cmp(right.category.as_str()))
                .then_with(|| left.source.cmp(&right.source))
        });

        TranslationCoverageReport {
            overlay_id,
            entries: self.entries,
        }
    }
}

fn structural_field_name_translation_entry(
    path: String,
    source: &str,
    translated: &str,
    affected_paths: Vec<String>,
) -> TranslationCoverageEntry {
    TranslationCoverageEntry {
        category: TranslationCoverageCategory::StructuralFieldNameTranslation,
        path,
        source: source.to_owned(),
        old_source: None,
        translated: Some(translated.to_owned()),
        context: Some(affected_paths.join(", ")),
    }
}

fn translation_context_view(
    deck: &CanonicalDeck,
    report: &TranslationCoverageReport,
) -> Result<TranslationContextView, FieldGraphReport> {
    let resolved = deck.resolve_field_graph(|note_id, field_id, images| {
        lower_images_from_deck(deck, note_id, field_id, images)
    })?;
    let mut source_counts = BTreeMap::<String, usize>::new();
    for entry in &report.entries {
        if !entry.source.is_empty() {
            *source_counts.entry(entry.source.clone()).or_insert(0) += 1;
        }
    }

    let entries_by_path = report
        .entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let units = report
        .entries
        .iter()
        .map(|entry| {
            translation_context_unit(deck, &resolved, entry, &source_counts, &entries_by_path)
        })
        .collect::<Vec<_>>();

    Ok(TranslationContextView {
        overlay_id: report.overlay_id.clone(),
        units,
    })
}

fn translation_context_unit(
    deck: &CanonicalDeck,
    resolved: &ResolvedFieldGraph,
    entry: &TranslationCoverageEntry,
    source_counts: &BTreeMap<String, usize>,
    entries_by_path: &BTreeMap<&str, &TranslationCoverageEntry>,
) -> TranslationContextUnit {
    let note_id = note_id_from_translation_path(&entry.path);
    let note_type_id = note_id
        .as_ref()
        .and_then(|note_id| {
            deck.notes
                .get(note_id)
                .map(|note| note.note_type_id.clone())
        })
        .or_else(|| note_type_id_from_translation_path(&entry.path));
    let note_type = note_type_id
        .as_ref()
        .and_then(|note_type_id| deck.note_types.get(note_type_id));
    let field_id = field_id_from_translation_path(&entry.path);
    let note = note_id.as_ref().and_then(|note_id| deck.notes.get(note_id));
    let field_name = note_type
        .zip(field_id.as_ref())
        .and_then(|(note_type, field_id)| field_definition_name(note_type, field_id))
        .map(str::to_owned);
    let note_fields = note
        .zip(note_type)
        .zip(note_id.as_ref())
        .map(|((_note, note_type), note_id)| {
            note_field_contexts(note_type, note_id, resolved, entries_by_path)
        })
        .unwrap_or_default();
    let message = note.zip(note_id.as_ref()).zip(field_id.as_ref()).and_then(
        |((note, note_id), field_id)| {
            message_context(note, note_id, field_id, resolved, entries_by_path)
        },
    );
    let card_templates = note_type
        .zip(field_name.as_deref())
        .map(|(note_type, field_name)| card_contexts_for_field(note_type, field_name))
        .unwrap_or_default();

    TranslationContextUnit {
        category: entry.category,
        path: entry.path.clone(),
        source: entry.source.clone(),
        old_source: entry.old_source.clone(),
        translated: entry.translated.clone(),
        context: entry.context.clone(),
        note_id,
        note_type_id,
        field_id,
        field_name,
        note_fields,
        message,
        card_templates,
        source_occurrences: source_counts.get(&entry.source).copied().unwrap_or(0),
    }
}

fn note_id_from_translation_path(path: &str) -> Option<StableId> {
    match path.parse().ok()? {
        DeckPath::Note { note_id }
        | DeckPath::NoteId { note_id }
        | DeckPath::NoteNoteTypeId { note_id }
        | DeckPath::NoteVariables { note_id }
        | DeckPath::NoteVariable { note_id, .. }
        | DeckPath::NoteField { note_id, .. }
        | DeckPath::NoteFieldMessage { note_id, .. }
        | DeckPath::NoteFieldMessageComponent { note_id, .. }
        | DeckPath::NoteFieldMessageFormat { note_id, .. }
        | DeckPath::NoteFieldMessageVariable { note_id, .. }
        | DeckPath::NoteTags { note_id }
        | DeckPath::NoteTag { note_id, .. }
        | DeckPath::NoteAdapterIds { note_id }
        | DeckPath::NoteAdapterId { note_id, .. } => Some(note_id),
        _ => None,
    }
}

fn note_type_id_from_translation_path(path: &str) -> Option<StableId> {
    match path.parse().ok()? {
        DeckPath::NoteType { note_type_id }
        | DeckPath::NoteTypeId { note_type_id }
        | DeckPath::NoteTypeName { note_type_id }
        | DeckPath::NoteTypeVariables { note_type_id }
        | DeckPath::NoteTypeVariable { note_type_id, .. }
        | DeckPath::NoteTypeStyling { note_type_id }
        | DeckPath::NoteTypeFields { note_type_id }
        | DeckPath::NoteTypeField { note_type_id, .. }
        | DeckPath::NoteTypeFieldName { note_type_id, .. }
        | DeckPath::NoteTypeCardTemplates { note_type_id }
        | DeckPath::NoteTypeCardTemplate { note_type_id, .. }
        | DeckPath::NoteTypeCardTemplateName { note_type_id, .. }
        | DeckPath::NoteTypeCardTemplateVariables { note_type_id, .. }
        | DeckPath::NoteTypeCardTemplateVariable { note_type_id, .. }
        | DeckPath::NoteTypeCardTemplateQuestionFormat { note_type_id, .. }
        | DeckPath::NoteTypeCardTemplateAnswerFormat { note_type_id, .. }
        | DeckPath::NoteTypeCardTemplateAdapterIds { note_type_id, .. }
        | DeckPath::NoteTypeCardTemplateAdapterId { note_type_id, .. }
        | DeckPath::NoteTypeAdapterIds { note_type_id }
        | DeckPath::NoteTypeAdapterId { note_type_id, .. } => Some(note_type_id),
        _ => None,
    }
}

fn field_id_from_translation_path(path: &str) -> Option<StableId> {
    match path.parse().ok()? {
        DeckPath::NoteField { field_id, .. }
        | DeckPath::NoteFieldMessage { field_id, .. }
        | DeckPath::NoteFieldMessageComponent { field_id, .. }
        | DeckPath::NoteFieldMessageFormat { field_id, .. }
        | DeckPath::NoteFieldMessageVariable { field_id, .. }
        | DeckPath::NoteTypeField { field_id, .. }
        | DeckPath::NoteTypeFieldName { field_id, .. } => Some(field_id),
        _ => None,
    }
}

fn field_definition_name<'a>(note_type: &'a NoteType, field_id: &StableId) -> Option<&'a str> {
    note_type
        .fields
        .iter()
        .find(|field| &field.id == field_id)
        .map(|field| field.name.as_str())
}

fn note_field_contexts(
    note_type: &NoteType,
    note_id: &StableId,
    resolved: &ResolvedFieldGraph,
    entries_by_path: &BTreeMap<&str, &TranslationCoverageEntry>,
) -> Vec<TranslationNoteFieldContext> {
    note_type
        .fields
        .iter()
        .filter_map(|field| {
            let path = DeckPath::NoteField {
                note_id: note_id.clone(),
                field_id: field.id.clone(),
            }
            .to_string();
            let source = resolved.get(&path)?.to_owned();
            let path = DeckPath::NoteField {
                note_id: note_id.clone(),
                field_id: field.id.clone(),
            }
            .to_string();
            let entry = entries_by_path.get(path.as_str()).copied();
            Some(TranslationNoteFieldContext {
                field_id: field.id.clone(),
                field_name: field.name.clone(),
                source: source.clone(),
                translated: entry
                    .and_then(|entry| entry.translated.clone())
                    .unwrap_or(source),
                category: entry.map(|entry| entry.category),
            })
        })
        .collect()
}

fn message_context(
    note: &Note,
    note_id: &StableId,
    field_id: &StableId,
    resolved: &ResolvedFieldGraph,
    entries_by_path: &BTreeMap<&str, &TranslationCoverageEntry>,
) -> Option<TranslationMessageContext> {
    let message = note.fields.get(field_id)?.as_message()?;
    let field_path = DeckPath::NoteField {
        note_id: note_id.clone(),
        field_id: field_id.clone(),
    }
    .to_string();
    let resolved_source = resolved.get(&field_path)?.to_owned();
    let full_field_translation = entries_by_path
        .get(
            DeckPath::NoteField {
                note_id: note_id.clone(),
                field_id: field_id.clone(),
            }
            .to_string()
            .as_str(),
        )
        .and_then(|entry| entry.translated.clone());

    if let Some(format) = &message.format {
        let format_path = message_format_path(note_id, field_id);
        let format_entry = entries_by_path.get(format_path.as_str()).copied();
        let translated_format = format_entry
            .and_then(|entry| entry.translated.clone())
            .unwrap_or_else(|| format.clone());
        let format_context = TranslationMessageComponentContext {
            index: 0,
            name: None,
            kind: MessageComponentKind::Format,
            path: format_path,
            source: format.clone(),
            translated: translated_format.clone(),
            reference: None,
            category: format_entry.map(|entry| entry.category),
        };
        let mut components = Vec::new();
        let mut translated_variables = BTreeMap::new();
        for (index, (name, component)) in message.variables.iter().enumerate() {
            let path = message_variable_path(note_id, field_id, name);
            let context = message_component_context(
                resolved,
                component,
                index,
                Some(name.clone()),
                path,
                entries_by_path,
            );
            translated_variables.insert(name.clone(), context.translated.clone());
            components.push(context);
        }
        let translated = full_field_translation.unwrap_or_else(|| {
            render_message_format(&translated_format, &translated_variables).unwrap_or_else(|_| {
                components
                    .iter()
                    .map(|component| component.translated.as_str())
                    .collect::<String>()
            })
        });
        return Some(TranslationMessageContext {
            source: resolved_source,
            translated,
            format: Some(format_context),
            components,
        });
    }

    let mut components = Vec::new();
    for (index, component) in message.components.iter().enumerate() {
        components.push(message_component_context(
            resolved,
            component,
            index,
            None,
            message_component_path(note_id, field_id, index),
            entries_by_path,
        ));
    }
    let translated = full_field_translation.unwrap_or_else(|| {
        components
            .iter()
            .map(|component| component.translated.as_str())
            .collect::<String>()
    });
    Some(TranslationMessageContext {
        source: resolved_source,
        translated,
        format: None,
        components,
    })
}

fn message_component_context(
    resolved: &ResolvedFieldGraph,
    component: &MessageComponent,
    index: usize,
    name: Option<String>,
    path: String,
    entries_by_path: &BTreeMap<&str, &TranslationCoverageEntry>,
) -> TranslationMessageComponentContext {
    let entry = entries_by_path.get(path.as_str()).copied();
    let (kind, source, reference) = match component {
        MessageComponent::Literal(value) => (MessageComponentKind::Literal, value.clone(), None),
        MessageComponent::Text(value) => (MessageComponentKind::Text, value.clone(), None),
        MessageComponent::FieldRef(reference) => (
            MessageComponentKind::FieldRef,
            resolved
                .get(reference)
                .expect("message references were validated by the shared field graph")
                .to_owned(),
            Some(reference.clone()),
        ),
    };
    let translated = entry
        .and_then(|entry| entry.translated.clone())
        .unwrap_or_else(|| source.clone());
    TranslationMessageComponentContext {
        index,
        name,
        kind,
        path,
        source,
        translated,
        reference,
        category: entry.map(|entry| entry.category),
    }
}

fn card_contexts_for_field(note_type: &NoteType, field_name: &str) -> Vec<TranslationCardContext> {
    note_type
        .card_templates
        .iter()
        .filter_map(|template| {
            let mut sides = BTreeSet::new();
            if template_uses_field(&template.question_format, field_name) {
                sides.insert(CardTemplateSide::Question);
            }
            if template_uses_field(&template.answer_format, field_name) {
                sides.insert(CardTemplateSide::Answer);
            }
            if sides.is_empty() {
                None
            } else {
                Some(TranslationCardContext {
                    template_id: template.id.clone(),
                    template_name: template.name.clone(),
                    sides,
                    question_format: template.question_format.clone(),
                    answer_format: template.answer_format.clone(),
                })
            }
        })
        .collect()
}

fn template_uses_field(template_text: &str, field_name: &str) -> bool {
    [
        format!("{{{{{field_name}}}}}"),
        format!("{{{{#{field_name}}}}}"),
        format!("{{{{/{field_name}}}}}"),
        format!("{{{{^{field_name}}}}}"),
        format!("{{{{type:{field_name}}}}}"),
    ]
    .iter()
    .any(|marker| template_text.contains(marker))
}

pub(crate) fn apply_translation_dictionary(
    resolved: &mut CanonicalDeck,
    overlay: &Overlay,
    translations: &TranslationDictionary,
    changed_paths: &mut BTreeMap<String, StableId>,
    errors: &mut Vec<ComposeError>,
) {
    if let Err(error) = translations.validate_mutation_invariants() {
        errors.push(ComposeError::new(
            ComposeErrorKind::ValidationFailed,
            error.path,
            error.message,
        ));
        return;
    }
    let source_deck = resolved.clone();
    let source_graph = match source_deck.resolve_field_graph(|note_id, field_id, images| {
        lower_images_from_deck(&source_deck, note_id, field_id, images)
    }) {
        Ok(graph) => graph,
        Err(report) => {
            errors.extend(report.errors.into_iter().map(|graph_error| {
                let mut error = ComposeError::new(
                    ComposeErrorKind::ValidationFailed,
                    graph_error.consuming_path.clone(),
                    graph_error.message.clone(),
                );
                error.field_graph_error = Some(graph_error);
                error
            }));
            return;
        }
    };
    let mut seen_direct = BTreeSet::new();
    let mut seen_contextual = BTreeSet::new();
    let mut seen_target_adaptations = BTreeSet::new();
    let mut seen_variables = BTreeSet::new();
    let mut seen_adapter_ids = BTreeSet::new();
    let mut source_paths = BTreeMap::<String, BTreeSet<String>>::new();

    {
        let tombstones = resolved.tombstones.clone();
        let mut context = TranslationApplyContext {
            overlay,
            translations,
            tombstones: &tombstones,
            seen_direct: &mut seen_direct,
            seen_contextual: &mut seen_contextual,
            seen_target_adaptations: &mut seen_target_adaptations,
            seen_variables: &mut seen_variables,
            seen_adapter_ids: &mut seen_adapter_ids,
            source_paths: &mut source_paths,
            changed_paths,
            errors,
        };
        if tombstones.blocking(&TombstoneAddress::DeckName).is_none() {
            context.translate_string(&mut resolved.name, DeckPath::DeckName.to_string(), None);
        }
        if tombstones
            .blocking(&TombstoneAddress::DeckDescription)
            .is_none()
        {
            context.translate_string(
                &mut resolved.description,
                DeckPath::DeckDescription.to_string(),
                None,
            );
        }
        context.translate_variables(
            &mut resolved.variables,
            &DeckPath::DeckVariables.to_string(),
        );
        context.translate_adapter_ids(
            &mut resolved.adapter_ids,
            &DeckPath::DeckAdapterIds.to_string(),
        );

        for (note_type_id, note_type) in &mut resolved.note_types {
            if tombstones
                .blocking(&TombstoneAddress::NoteType {
                    note_type_id: note_type_id.clone(),
                })
                .is_some()
            {
                continue;
            }
            context.translate_string(
                &mut note_type.name,
                DeckPath::NoteTypeName {
                    note_type_id: note_type_id.clone(),
                }
                .to_string(),
                None,
            );
            context.translate_variables(
                &mut note_type.variables,
                &DeckPath::NoteTypeVariables {
                    note_type_id: note_type_id.clone(),
                }
                .to_string(),
            );
            // Field-definition names remain structural so Mustache and other
            // adapter references stay valid across target languages. Translate
            // display labels through source variables instead.
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
                context.translate_string(
                    &mut template.name,
                    DeckPath::NoteTypeCardTemplateName {
                        note_type_id: note_type_id.clone(),
                        template_id: template.id.clone(),
                    }
                    .to_string(),
                    None,
                );
                context.translate_variables(
                    &mut template.variables,
                    &DeckPath::NoteTypeCardTemplateVariables {
                        note_type_id: note_type_id.clone(),
                        template_id: template.id.clone(),
                    }
                    .to_string(),
                );
                context.translate_adapter_ids(
                    &mut template.adapter_ids,
                    &DeckPath::NoteTypeCardTemplateAdapterIds {
                        note_type_id: note_type_id.clone(),
                        template_id: template.id.clone(),
                    }
                    .to_string(),
                );
            }
            context.translate_adapter_ids(
                &mut note_type.adapter_ids,
                &DeckPath::NoteTypeAdapterIds {
                    note_type_id: note_type_id.clone(),
                }
                .to_string(),
            );
        }

        for (note_id, note) in &mut resolved.notes {
            if tombstones
                .blocking(&TombstoneAddress::Note {
                    note_id: note_id.clone(),
                })
                .is_some()
            {
                continue;
            }
            context.translate_variables(
                &mut note.variables,
                &DeckPath::NoteVariables {
                    note_id: note_id.clone(),
                }
                .to_string(),
            );
            let field_ids = note.fields.keys().cloned().collect::<Vec<_>>();
            for field_id in field_ids {
                if tombstones
                    .blocking(&TombstoneAddress::NoteField {
                        note_id: note_id.clone(),
                        field_id: field_id.clone(),
                    })
                    .is_some()
                {
                    continue;
                }
                let path = DeckPath::NoteField {
                    note_id: note_id.clone(),
                    field_id: field_id.clone(),
                }
                .to_string();
                let source = source_graph
                    .get(&path)
                    .expect("every live source field was resolved by the shared graph")
                    .to_owned();
                let Some(value) = note.fields.get_mut(&field_id) else {
                    continue;
                };
                match value {
                    FieldValue::Scalar(value) => context.translate_string(value, path, None),
                    FieldValue::Message(message) => {
                        let full_override = context.translate_message_field(
                            &source_graph,
                            &source,
                            path,
                            note_id,
                            &field_id,
                            message,
                        );
                        if let Some(translated) = full_override {
                            *value = FieldValue::Scalar(translated);
                        }
                    }
                    FieldValue::Images(_) => {}
                }
            }
            context.translate_tags(
                &mut note.tags,
                &DeckPath::NoteTags {
                    note_id: note_id.clone(),
                }
                .to_string(),
            );
            context.translate_adapter_ids(
                &mut note.adapter_ids,
                &DeckPath::NoteAdapterIds {
                    note_id: note_id.clone(),
                }
                .to_string(),
            );
        }
    }

    for source in translations.direct.keys() {
        if !seen_direct.contains(source) {
            errors.push(ComposeError::new(
                ComposeErrorKind::StaleTranslationEntry,
                format!("translations.direct.{source}"),
                format!(
                    "stale direct translation source {source:?} did not match any extracted non-empty source text; use target_adaptations for intentional target-language adaptations"
                ),
            ));
        }
    }
    for (context_path, replacements) in &translations.contextual {
        for (source, translated) in replacements {
            if !seen_contextual.contains(&(context_path.clone(), source.clone())) {
                errors.push(ComposeError::new(
                    ComposeErrorKind::StaleTranslationEntry,
                    format!("translations.contextual.{context_path}.{source}"),
                    format!(
                        "invalid contextual translation: source {source:?} did not match any extracted text under {context_path}; the source key may be stale, the context may be invalid, or the entry may belong in target_adaptations"
                    ),
                ));
                continue;
            }
            if let Some(shorter_context) = shortest_safe_context(
                translations,
                &source_paths,
                context_path,
                source,
                translated,
            ) {
                errors.push(ComposeError::new(
                    ComposeErrorKind::ValidationFailed,
                    format!("translations.contextual.{context_path}.{source}"),
                    format!(
                        "contextual translation for source {source:?} is more specific than necessary; use context {shorter_context:?} instead of {context_path:?}"
                    ),
                ));
            }
        }
    }
    for source in &translations.no_change {
        if !source_paths.contains_key(source) {
            errors.push(ComposeError::new(
                ComposeErrorKind::StaleTranslationEntry,
                format!("translations.no_change.{source}"),
                format!(
                    "stale no-change source {source:?} did not match any extracted non-empty source text"
                ),
            ));
        }
    }
    for path in translations.target_adaptations.keys() {
        if !seen_target_adaptations.contains(path) {
            errors.push(ComposeError::new(
                ComposeErrorKind::MissingOverlayTarget,
                format!("target_adaptations.{path}"),
                format!("target adaptation path {path} did not match any extracted field"),
            ));
        }
    }
    for (variable_key, replacements) in &translations.variables {
        for source in replacements.keys() {
            if !seen_variables.contains(&(variable_key.clone(), source.clone())) {
                errors.push(ComposeError::new(
                    ComposeErrorKind::StaleTranslationEntry,
                    format!("translations.variables.{variable_key}.{source}"),
                    format!(
                        "variable translation source {variable_key}={source:?} did not match any variable"
                    ),
                ));
            }
        }
    }
    for (adapter_key, replacements) in &translations.adapter_ids {
        for source in replacements.keys() {
            if !seen_adapter_ids.contains(&(adapter_key.clone(), source.clone())) {
                errors.push(ComposeError::new(
                    ComposeErrorKind::StaleTranslationEntry,
                    format!("translations.adapter_ids.{adapter_key}.{source}"),
                    format!(
                        "adapter id translation source {adapter_key}={source:?} did not match any adapter id"
                    ),
                ));
            }
        }
    }
}

fn message_is_single_reference(message: &StructuredMessage) -> bool {
    if message.format.is_none() {
        return matches!(
            message.components.as_slice(),
            [MessageComponent::FieldRef(_)]
        );
    }
    let Some((name, MessageComponent::FieldRef(_))) = message.variables.first_key_value() else {
        return false;
    };
    message.variables.len() == 1
        && message.format.as_deref() == Some(format!("{{{name}}}").as_str())
}

struct TranslationApplyContext<'a, 'b> {
    overlay: &'a Overlay,
    translations: &'a TranslationDictionary,
    tombstones: &'a Tombstones,
    seen_direct: &'b mut BTreeSet<String>,
    seen_contextual: &'b mut BTreeSet<(String, String)>,
    seen_target_adaptations: &'b mut BTreeSet<String>,
    seen_variables: &'b mut BTreeSet<(String, String)>,
    seen_adapter_ids: &'b mut BTreeSet<(String, String)>,
    source_paths: &'b mut BTreeMap<String, BTreeSet<String>>,
    changed_paths: &'b mut BTreeMap<String, StableId>,
    errors: &'b mut Vec<ComposeError>,
}

impl TranslationApplyContext<'_, '_> {
    fn translate_variables(&mut self, variables: &mut BTreeMap<String, String>, path_prefix: &str) {
        for (key, value) in variables {
            self.translate_string(value, map_entry_path(path_prefix, key), Some(key));
        }
    }

    fn translate_message_field(
        &mut self,
        source_graph: &ResolvedFieldGraph,
        resolved_source: &str,
        path: String,
        note_id: &StableId,
        field_id: &StableId,
        message: &mut StructuredMessage,
    ) -> Option<String> {
        let full_field_outcome = resolve_translation(
            self.translations,
            resolved_source,
            TranslationResolveOptions {
                path: &path,
                variable_key: None,
                include_target_adaptation: true,
                include_variable: true,
                include_ignored: true,
            },
        );
        let is_live_reference_alias = message_is_single_reference(message);
        let has_path_specific_full_override = match full_field_outcome {
            TranslationOutcome::TargetAdaptation { .. } | TranslationOutcome::Contextual { .. } => {
                true
            }
            TranslationOutcome::Direct { .. } => !is_live_reference_alias,
            TranslationOutcome::Stale { record, .. } => {
                record.context.is_some() || !is_live_reference_alias
            }
            _ => false,
        };
        if has_path_specific_full_override {
            let mut translated = resolved_source.to_owned();
            self.translate_string(&mut translated, path, None);
            return Some(translated);
        }

        if resolved_source.is_empty() || is_ignored_translation_path(self.translations, &path) {
            return None;
        }

        if let Some(format) = &mut message.format {
            self.translate_string_without_missing(format, message_format_path(note_id, field_id));
            for (variable, component) in &mut message.variables {
                self.translate_message_component(
                    source_graph,
                    component,
                    message_variable_path(note_id, field_id, variable),
                );
            }
        } else {
            for (index, component) in message.components.iter_mut().enumerate() {
                self.translate_message_component(
                    source_graph,
                    component,
                    message_component_path(note_id, field_id, index),
                );
            }
        }
        None
    }

    fn translate_message_component(
        &mut self,
        source_graph: &ResolvedFieldGraph,
        component: &mut MessageComponent,
        path: String,
    ) {
        match component {
            MessageComponent::Literal(_) => {}
            MessageComponent::Text(value) => {
                self.translate_string(value, path, None);
            }
            MessageComponent::FieldRef(reference) => {
                let source = source_graph
                    .get(reference)
                    .expect("message references were validated by the shared field graph");
                let materialize_path_specific_translation = matches!(
                    resolve_translation(
                        self.translations,
                        source,
                        TranslationResolveOptions {
                            path: &path,
                            variable_key: None,
                            include_target_adaptation: true,
                            include_variable: true,
                            include_ignored: true,
                        },
                    ),
                    TranslationOutcome::TargetAdaptation { .. }
                        | TranslationOutcome::Contextual { .. }
                        | TranslationOutcome::Stale {
                            record: StaleTranslation {
                                context: Some(_),
                                ..
                            },
                            ..
                        }
                );
                let dependency_outcome = resolve_translation(
                    self.translations,
                    source,
                    TranslationResolveOptions {
                        path: reference,
                        variable_key: None,
                        include_target_adaptation: true,
                        include_variable: true,
                        include_ignored: true,
                    },
                );
                let dependency_target = match dependency_outcome {
                    TranslationOutcome::TargetAdaptation { adaptation } => {
                        adaptation.target.as_str()
                    }
                    TranslationOutcome::Variable { translated }
                    | TranslationOutcome::Direct { translated }
                    | TranslationOutcome::Contextual { translated, .. } => translated,
                    TranslationOutcome::Stale { record, .. } => record.target.as_str(),
                    TranslationOutcome::NoChange
                    | TranslationOutcome::Missing
                    | TranslationOutcome::Ignored
                    | TranslationOutcome::Empty => source,
                };
                let mut translated = source.to_owned();
                self.translate_string(&mut translated, path, None);
                // Keep the edge live when the dependency itself receives the same reusable
                // translation. Materialize only a consuming-path decision or a translation that
                // the dependency path intentionally ignores, so later overlays still propagate
                // whenever semantics permit.
                if translated != source
                    && (materialize_path_specific_translation || translated != dependency_target)
                {
                    *component = MessageComponent::Literal(translated);
                }
            }
        }
    }

    fn translate_string(&mut self, value: &mut String, path: String, variable_key: Option<&str>) {
        self.translate_string_with_options(value, path, variable_key, true, true, true);
    }

    fn translate_string_without_missing(&mut self, value: &mut String, path: String) {
        self.translate_string_with_options(value, path, None, false, false, false);
    }

    fn translate_string_with_options(
        &mut self,
        value: &mut String,
        path: String,
        variable_key: Option<&str>,
        include_target_adaptation: bool,
        include_variable: bool,
        record_missing: bool,
    ) {
        let tombstone = path
            .parse::<DeckPath>()
            .ok()
            .and_then(|deck_path| TombstoneAddress::try_from(deck_path).ok())
            .and_then(|address| self.tombstones.blocking(&address));
        let outcome = resolve_translation(
            self.translations,
            value,
            TranslationResolveOptions {
                path: &path,
                variable_key,
                include_target_adaptation,
                include_variable,
                include_ignored: true,
            },
        );

        let translated_value = match &outcome {
            TranslationOutcome::TargetAdaptation { adaptation } => Some(adaptation.target.as_str()),
            TranslationOutcome::Variable { translated }
            | TranslationOutcome::Contextual { translated, .. }
            | TranslationOutcome::Direct { translated } => Some(*translated),
            TranslationOutcome::Stale { record, .. } => Some(record.target.as_str()),
            TranslationOutcome::Empty
            | TranslationOutcome::Ignored
            | TranslationOutcome::NoChange
            | TranslationOutcome::Missing => None,
        };
        if let Some(record) = tombstone
            && translated_value.is_some_and(|translated| translated != value)
        {
            let removal = record
                .provenance
                .as_ref()
                .map(|provenance| format!(" by overlay {}", provenance.overlay_id))
                .unwrap_or_else(|| " in legacy canonical source".to_owned());
            let mut error = ComposeError::new(
                ComposeErrorKind::TombstonedAddressReuse,
                path.clone(),
                format!(
                    "translation overlay {} cannot replace {path}; typed address {} was removed{removal}",
                    self.overlay.id, record.address
                ),
            );
            error.intent = Some(ChangeIntent::Replace);
            error.overlay_id = Some(self.overlay.id.clone());
            error.original_removal = Some(record.clone());
            self.errors.push(error);
            return;
        }

        match &outcome {
            TranslationOutcome::TargetAdaptation { adaptation } => {
                self.seen_target_adaptations.insert(path.clone());
                if value != &adaptation.expected_source {
                    self.errors.push(ComposeError::new(
                        ComposeErrorKind::ExpectedBaseMismatch,
                        path,
                        format!(
                            "target adaptation expected source {:?}, found {:?}",
                            adaptation.expected_source, value
                        ),
                    ));
                    return;
                }
                if !record_change_path(
                    &path,
                    self.overlay,
                    ChangeIntent::Replace,
                    self.changed_paths,
                    self.errors,
                ) {
                    return;
                }
                if value != &adaptation.target {
                    *value = adaptation.target.clone();
                }
            }
            TranslationOutcome::Empty | TranslationOutcome::Ignored => {}
            TranslationOutcome::Variable { translated } => {
                if let Some(variable_key) = variable_key {
                    let source = value.clone();
                    self.seen_variables
                        .insert((variable_key.to_owned(), source));
                    self.apply_translated_value(value, &path, translated);
                }
            }
            TranslationOutcome::Contextual { translated, .. }
            | TranslationOutcome::Direct { translated } => {
                self.record_apply_source(value, &path, &outcome);
                self.apply_translated_value(value, &path, translated);
            }
            TranslationOutcome::NoChange => {
                self.record_apply_source(value, &path, &outcome);
            }
            TranslationOutcome::Stale { record, .. } => {
                self.record_apply_source(value, &path, &outcome);
                self.apply_translated_value(value, &path, &record.target);
            }
            TranslationOutcome::Missing => {
                self.record_apply_source(value, &path, &outcome);
                if record_missing && self.translations.require_complete {
                    self.errors.push(ComposeError::new(
                        ComposeErrorKind::MissingTranslation,
                        path.clone(),
                        format!(
                            "missing direct or contextual translation for {value:?} at {path}; add translations.direct, add translations.no_change for intentionally unchanged text, add a translations.contextual entry for path-specific text, or ignore the path"
                        ),
                    ));
                }
            }
        }
    }

    fn record_apply_source(&mut self, value: &str, path: &str, outcome: &TranslationOutcome<'_>) {
        let source = value.to_owned();
        self.source_paths
            .entry(source.clone())
            .or_default()
            .insert(path.to_owned());
        if outcome.direct_translation().is_some() {
            self.seen_direct.insert(source.clone());
        }
        for contextual_match in outcome.contextual_matches() {
            self.seen_contextual
                .insert((contextual_match.context_path.to_owned(), source.clone()));
        }
    }

    fn apply_translated_value(&mut self, value: &mut String, path: &str, translated: &str) {
        if value == translated {
            return;
        }
        if !record_change_path(
            path,
            self.overlay,
            ChangeIntent::Replace,
            self.changed_paths,
            self.errors,
        ) {
            return;
        }
        *value = translated.to_owned();
    }

    fn translate_tags(&mut self, tags: &mut BTreeSet<String>, path_prefix: &str) {
        for tag in tags.iter().cloned().collect::<Vec<_>>() {
            let mut translated = tag.clone();
            self.translate_string(&mut translated, map_entry_path(path_prefix, &tag), None);
            if translated != tag {
                tags.remove(&tag);
                tags.insert(translated);
            }
        }
    }

    fn translate_adapter_ids(&mut self, adapter_ids: &mut AdapterIds, path_prefix: &str) {
        let current = adapter_ids
            .iter()
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect::<Vec<_>>();
        for (key, value) in current {
            let Some(replacements) = self.translations.adapter_ids.get(&key) else {
                continue;
            };
            let Some(translated) = replacements.get(&value) else {
                continue;
            };
            self.seen_adapter_ids.insert((key.clone(), value.clone()));
            if value != *translated {
                let path = map_entry_path(path_prefix, &key);
                if !record_change_path(
                    &path,
                    self.overlay,
                    ChangeIntent::Replace,
                    self.changed_paths,
                    self.errors,
                ) {
                    continue;
                }
                adapter_ids.insert(key, translated.clone());
            }
        }
    }
}

fn shortest_safe_context(
    translations: &TranslationDictionary,
    source_paths: &BTreeMap<String, BTreeSet<String>>,
    context_path: &str,
    source: &str,
    translated: &str,
) -> Option<String> {
    if is_note_field_context(context_path) {
        return None;
    }
    context_parent_candidates(context_path)
        .into_iter()
        .filter(|candidate| {
            contextual_replacement_is_safe(
                translations,
                source_paths,
                candidate,
                source,
                translated,
            )
        })
        .min_by_key(|candidate| candidate.len())
}

fn is_note_field_context(context_path: &str) -> bool {
    matches!(
        context_path.parse().ok(),
        Some(
            DeckPath::NoteField { .. }
                | DeckPath::NoteFieldMessage { .. }
                | DeckPath::NoteFieldMessageComponent { .. }
                | DeckPath::NoteFieldMessageFormat { .. }
                | DeckPath::NoteFieldMessageVariable { .. }
        )
    )
}

fn contextual_replacement_is_safe(
    translations: &TranslationDictionary,
    source_paths: &BTreeMap<String, BTreeSet<String>>,
    candidate_context: &str,
    source: &str,
    translated: &str,
) -> bool {
    if translations
        .contextual
        .get(candidate_context)
        .and_then(|replacements| replacements.get(source))
        .is_some_and(|existing| existing != translated)
    {
        return false;
    }

    let Some(paths) = source_paths.get(source) else {
        return false;
    };
    paths
        .iter()
        .filter(|path| context_matches_path(candidate_context, path))
        .all(|path| {
            let current = direct_or_contextual_translation_for_path(translations, source, path);
            let candidate = direct_or_contextual_translation_for_path_with_candidate(
                translations,
                candidate_context,
                source,
                translated,
                path,
            );
            current == candidate
        })
}

fn direct_or_contextual_translation_for_path<'a>(
    translations: &'a TranslationDictionary,
    source: &str,
    path: &str,
) -> Option<&'a str> {
    translations
        .contextual
        .iter()
        .filter(|(context_path, replacements)| {
            context_matches_path(context_path, path) && replacements.contains_key(source)
        })
        .max_by_key(|(context_path, _)| context_path.len())
        .and_then(|(_, replacements)| replacements.get(source).map(String::as_str))
        .or_else(|| translations.direct.get(source).map(String::as_str))
}

fn direct_or_contextual_translation_for_path_with_candidate<'a>(
    translations: &'a TranslationDictionary,
    candidate_context: &'a str,
    source: &str,
    translated: &'a str,
    path: &str,
) -> Option<&'a str> {
    let best_contextual = translations
        .contextual
        .iter()
        .filter(|(context_path, replacements)| {
            context_matches_path(context_path, path) && replacements.contains_key(source)
        })
        .max_by_key(|(context_path, _)| context_path.len());

    if context_matches_path(candidate_context, path)
        && best_contextual
            .is_none_or(|(context_path, _)| candidate_context.len() > context_path.len())
    {
        return Some(translated);
    }

    best_contextual
        .and_then(|(_, replacements)| replacements.get(source).map(String::as_str))
        .or_else(|| translations.direct.get(source).map(String::as_str))
}

fn context_parent_candidates(context_path: &str) -> Vec<String> {
    match context_path.parse().ok() {
        Some(DeckPath::NoteFieldMessageFormat { .. })
        | Some(DeckPath::NoteFieldMessageVariable { .. }) => Vec::new(),
        Some(
            DeckPath::NoteField { note_id, .. }
            | DeckPath::NoteFieldMessage { note_id, .. }
            | DeckPath::NoteFieldMessageComponent { note_id, .. }
            | DeckPath::NoteTag { note_id, .. }
            | DeckPath::NoteVariable { note_id, .. },
        ) => vec![DeckPath::Note { note_id }.to_string()],
        Some(
            DeckPath::NoteTypeField { note_type_id, .. }
            | DeckPath::NoteTypeFieldName { note_type_id, .. }
            | DeckPath::NoteTypeCardTemplate { note_type_id, .. }
            | DeckPath::NoteTypeCardTemplateName { note_type_id, .. }
            | DeckPath::NoteTypeCardTemplateVariables { note_type_id, .. }
            | DeckPath::NoteTypeCardTemplateVariable { note_type_id, .. }
            | DeckPath::NoteTypeCardTemplateQuestionFormat { note_type_id, .. }
            | DeckPath::NoteTypeCardTemplateAnswerFormat { note_type_id, .. }
            | DeckPath::NoteTypeCardTemplateAdapterIds { note_type_id, .. }
            | DeckPath::NoteTypeCardTemplateAdapterId { note_type_id, .. },
        ) => vec![DeckPath::NoteType { note_type_id }.to_string()],
        _ => Vec::new(),
    }
}

fn context_matches_path(context_path: &str, path: &str) -> bool {
    path == context_path
        || path
            .strip_prefix(context_path)
            .is_some_and(|suffix| suffix.starts_with('.'))
}

fn map_entry_path(path_prefix: &str, key: &str) -> String {
    match path_prefix.parse().ok() {
        Some(DeckPath::DeckVariables) => DeckPath::DeckVariable {
            key: key.to_owned(),
        },
        Some(DeckPath::DeckAdapterIds) => DeckPath::DeckAdapterId {
            key: key.to_owned(),
        },
        Some(DeckPath::NoteTypeVariables { note_type_id }) => DeckPath::NoteTypeVariable {
            note_type_id,
            key: key.to_owned(),
        },
        Some(DeckPath::NoteTypeAdapterIds { note_type_id }) => DeckPath::NoteTypeAdapterId {
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
        Some(DeckPath::NoteTypeCardTemplateAdapterIds {
            note_type_id,
            template_id,
        }) => DeckPath::NoteTypeCardTemplateAdapterId {
            note_type_id,
            template_id,
            key: key.to_owned(),
        },
        Some(DeckPath::NoteVariables { note_id }) => DeckPath::NoteVariable {
            note_id,
            key: key.to_owned(),
        },
        Some(DeckPath::NoteTags { note_id }) => DeckPath::NoteTag {
            note_id,
            tag: key.to_owned(),
        },
        Some(DeckPath::NoteAdapterIds { note_id }) => DeckPath::NoteAdapterId {
            note_id,
            key: key.to_owned(),
        },
        _ => panic!("unsupported deck path collection {path_prefix:?}"),
    }
    .to_string()
}

fn matching_stale_translation<'a>(
    translations: &'a TranslationDictionary,
    source: &str,
    path: &str,
) -> Option<(usize, &'a StaleTranslation)> {
    translations
        .stale_translations
        .iter()
        .enumerate()
        .filter(|(_, record)| {
            record.new_source == source
                && record
                    .context
                    .as_deref()
                    .is_none_or(|context| context_matches_path(context, path))
        })
        .max_by_key(|(_, record)| record.context.as_ref().map_or(0, String::len))
}

fn is_ignored_translation_path(translations: &TranslationDictionary, path: &str) -> bool {
    translations
        .ignore_paths
        .iter()
        .any(|pattern| glob_matches(pattern, path))
}

pub fn glob_matches(pattern: &str, value: &str) -> bool {
    if !pattern.contains('*') {
        return pattern == value;
    }

    let starts_with_star = pattern.starts_with('*');
    let ends_with_star = pattern.ends_with('*');
    let segments = pattern
        .split('*')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    if segments.is_empty() {
        return true;
    }

    let mut remaining = value;
    let mut first_middle_segment = 0;
    if !starts_with_star {
        let first = segments[0];
        if !remaining.starts_with(first) {
            return false;
        }
        remaining = &remaining[first.len()..];
        first_middle_segment = 1;
    }

    let middle_segment_end = if ends_with_star {
        segments.len()
    } else {
        segments.len() - 1
    };
    for segment in &segments[first_middle_segment..middle_segment_end] {
        let Some(offset) = remaining.find(segment) else {
            return false;
        };
        remaining = &remaining[offset + segment.len()..];
    }

    ends_with_star || remaining.ends_with(segments[segments.len() - 1])
}
