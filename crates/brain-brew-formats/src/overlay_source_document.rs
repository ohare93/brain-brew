//! Include-preserving, validated overlay source editing.
//!
//! Translation precedence cleanup, sparse field edits, media hashes, and image
//! conversion are centralized here so callers never mutate YAML maps directly.

use std::collections::{BTreeMap, BTreeSet};

use brain_brew_core::{Overlay, StableId, TranslationDictionary};

use crate::canonical_yaml;
use crate::source_document::{
    EditLocation, ImageConversionReport, IncludeRequest, IncludeState, SourceDocumentEmission,
    SourceDocumentError, SourceFile, SourceProvenance, convert_text_to_images, ensure_non_empty,
    matching_contexts, prepare_source,
};

/// One validated translator decision. The selected source and occurrence path
/// are separate method arguments so cross-map cleanup is unavoidable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TranslationDecision {
    Direct(String),
    Contextual { context: String, target: String },
    NoChange,
}

/// Translation-dictionary consequence of editing source text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceTranslationImpact {
    MarkStale {
        target: String,
        context: Option<String>,
    },
    MigrateKey {
        target: String,
        context: Option<String>,
    },
}

/// Typed batch inserted by translation-report workflows.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TranslationStubs {
    pub direct: BTreeSet<String>,
    pub contextual: BTreeMap<String, BTreeSet<String>>,
    pub no_change: BTreeSet<String>,
    pub ignore_paths: BTreeSet<String>,
}

/// Deep source module for one sparse overlay and its scalar includes.
#[derive(Clone)]
pub struct OverlaySourceDocument {
    provenance: SourceProvenance,
    overlay: Overlay,
    includes: IncludeState,
}

impl std::fmt::Debug for OverlaySourceDocument {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OverlaySourceDocument")
            .field("provenance", &self.provenance)
            .field("overlay", &self.overlay)
            .finish_non_exhaustive()
    }
}

impl OverlaySourceDocument {
    pub fn parse(source: SourceFile) -> Result<Self, SourceDocumentError> {
        Self::parse_with_includes(source, |request| {
            Err(format!(
                "no include loader was provided for {:?}",
                request.target()
            ))
        })
    }

    pub fn parse_with_includes(
        source: SourceFile,
        mut loader: impl FnMut(&IncludeRequest) -> Result<SourceFile, String>,
    ) -> Result<Self, SourceDocumentError> {
        let prepared = prepare_source(source, false, &mut loader)?;
        let overlay = canonical_yaml::overlay_from_str(&prepared.yaml_without_directives).map_err(
            |error| SourceDocumentError::source(prepared.root.provenance(), error.to_string()),
        )?;
        canonical_yaml::overlay_to_string(&overlay).map_err(|error| {
            SourceDocumentError::source(prepared.root.provenance(), error.to_string())
        })?;
        Ok(Self {
            provenance: prepared.root.provenance().clone(),
            overlay,
            includes: prepared.includes,
        })
    }

    pub fn from_overlay(
        provenance: SourceProvenance,
        overlay: Overlay,
    ) -> Result<Self, SourceDocumentError> {
        canonical_yaml::overlay_to_string(&overlay)
            .map_err(|error| SourceDocumentError::source(&provenance, error.to_string()))?;
        Ok(Self {
            provenance,
            overlay,
            includes: IncludeState::default(),
        })
    }

    pub fn provenance(&self) -> &SourceProvenance {
        &self.provenance
    }

    /// Read-only validated domain view. Mutation remains behind typed methods.
    pub fn overlay(&self) -> &Overlay {
        &self.overlay
    }

    /// Apply a translation decision and clean entries that would conflict at the
    /// selected occurrence path.
    pub fn set_translation_decision(
        &mut self,
        occurrence_path: &str,
        source: &str,
        decision: TranslationDecision,
    ) -> Result<(), SourceDocumentError> {
        ensure_non_empty(occurrence_path, "translation occurrence path")
            .and_then(|_| ensure_non_empty(source, "translation source"))
            .map_err(|message| {
                SourceDocumentError::at(&self.provenance, "translations", message)
            })?;
        let mut next = self.clone();
        let translations = next
            .overlay
            .translations
            .get_or_insert_with(TranslationDictionary::default);
        remove_stale_for_path_source(translations, occurrence_path, source);
        match decision {
            TranslationDecision::Direct(target) => {
                translations.no_change.remove(source);
                remove_contextual_for_path(translations, occurrence_path, source);
                translations.direct.insert(source.to_owned(), target);
            }
            TranslationDecision::Contextual { context, target } => {
                if context != occurrence_path
                    && !occurrence_path
                        .strip_prefix(&context)
                        .is_some_and(|suffix| suffix.starts_with('.'))
                {
                    return Err(SourceDocumentError::at(
                        &next.provenance,
                        "translations.contextual",
                        format!(
                            "context {context:?} is not an ancestor of occurrence {occurrence_path:?}"
                        ),
                    ));
                }
                translations.no_change.remove(source);
                translations
                    .contextual
                    .entry(context)
                    .or_default()
                    .insert(source.to_owned(), target);
            }
            TranslationDecision::NoChange => {
                translations.direct.remove(source);
                remove_contextual_for_path(translations, occurrence_path, source);
                translations.no_change.insert(source.to_owned());
            }
        }
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Record the translation consequence of a compare-and-set source edit.
    ///
    /// This operation removes the superseded key from every conflicting
    /// dictionary branch before either creating explicit review debt or moving
    /// the retained target to the new source key.
    pub fn apply_source_translation_impact(
        &mut self,
        occurrence_path: &str,
        old_source: &str,
        new_source: &str,
        impact: SourceTranslationImpact,
    ) -> Result<(), SourceDocumentError> {
        ensure_non_empty(occurrence_path, "source occurrence path")
            .and_then(|_| ensure_non_empty(old_source, "old source"))
            .and_then(|_| ensure_non_empty(new_source, "new source"))
            .map_err(|message| {
                SourceDocumentError::at(&self.provenance, "translations", message)
            })?;
        let mut next = self.clone();
        let translations = next
            .overlay
            .translations
            .get_or_insert_with(TranslationDictionary::default);
        let (target, context, mark_stale) = match impact {
            SourceTranslationImpact::MarkStale { target, context } => (target, context, true),
            SourceTranslationImpact::MigrateKey { target, context } => (target, context, false),
        };
        if let Some(context) = &context
            && context != occurrence_path
            && !occurrence_path
                .strip_prefix(context)
                .is_some_and(|suffix| suffix.starts_with('.'))
        {
            return Err(SourceDocumentError::at(
                &next.provenance,
                "translations",
                format!("context {context:?} is not an ancestor of occurrence {occurrence_path:?}"),
            ));
        }
        if context.is_some() {
            remove_contextual_for_path(translations, occurrence_path, old_source);
        } else {
            translations.direct.remove(old_source);
            translations.no_change.remove(old_source);
            remove_contextual_everywhere(translations, old_source);
        }
        remove_stale_for_path_source(translations, occurrence_path, new_source);
        if mark_stale {
            translations.stale_translations.retain(|record| {
                !(record.old_source == old_source
                    && record.new_source == new_source
                    && record.context == context)
            });
            translations
                .stale_translations
                .push(brain_brew_core::StaleTranslation {
                    old_source: old_source.to_owned(),
                    new_source: new_source.to_owned(),
                    target,
                    context,
                });
        } else if let Some(context) = context {
            translations
                .contextual
                .entry(context)
                .or_default()
                .insert(new_source.to_owned(), target);
        } else {
            translations.direct.insert(new_source.to_owned(), target);
        }
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Insert missing translation work without overwriting existing decisions.
    pub fn add_translation_stubs(
        &mut self,
        stubs: TranslationStubs,
    ) -> Result<(), SourceDocumentError> {
        let mut next = self.clone();
        let translations = next
            .overlay
            .translations
            .get_or_insert_with(TranslationDictionary::default);
        for source in stubs.direct {
            ensure_non_empty(&source, "direct translation source").map_err(|message| {
                SourceDocumentError::at(&next.provenance, "translations.direct", message)
            })?;
            translations.direct.entry(source.clone()).or_insert(source);
        }
        for (context, sources) in stubs.contextual {
            ensure_non_empty(&context, "translation context").map_err(|message| {
                SourceDocumentError::at(&next.provenance, "translations.contextual", message)
            })?;
            let replacements = translations.contextual.entry(context).or_default();
            for source in sources {
                ensure_non_empty(&source, "contextual translation source").map_err(|message| {
                    SourceDocumentError::at(&next.provenance, "translations.contextual", message)
                })?;
                replacements.entry(source.clone()).or_insert(source);
            }
        }
        for source in stubs.no_change {
            ensure_non_empty(&source, "no-change source").map_err(|message| {
                SourceDocumentError::at(&next.provenance, "translations.no_change", message)
            })?;
            translations.no_change.insert(source);
        }
        translations.ignore_paths.extend(stubs.ignore_paths);
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Resolve one stale record, optionally replacing its retained target text.
    pub fn resolve_stale_translation(
        &mut self,
        old_source: &str,
        new_source: &str,
        context: Option<&str>,
        replacement: Option<&str>,
    ) -> Result<(), SourceDocumentError> {
        let mut next = self.clone();
        let translations = next.overlay.translations.as_mut().ok_or_else(|| {
            SourceDocumentError::at(
                &next.provenance,
                "translations.stale_translations",
                "overlay has no translation dictionary",
            )
        })?;
        let position = translations
            .stale_translations
            .iter()
            .position(|record| {
                record.old_source == old_source
                    && record.new_source == new_source
                    && record.context.as_deref() == context
            })
            .ok_or_else(|| {
                SourceDocumentError::at(
                    &next.provenance,
                    "translations.stale_translations",
                    "matching stale translation was not found",
                )
            })?;
        let mut record = translations.stale_translations.remove(position);
        if let Some(replacement) = replacement {
            record.target = replacement.to_owned();
        }
        if let Some(context) = record.context {
            translations
                .contextual
                .entry(context)
                .or_default()
                .insert(record.new_source, record.target);
        } else {
            translations.direct.insert(record.new_source, record.target);
        }
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Compare-and-set one sparse scalar note-field payload. Includes route to
    /// their own source output instead of being materialized into YAML.
    pub fn set_note_field_text(
        &mut self,
        note_id: &StableId,
        field_id: &StableId,
        expected: &str,
        replacement: &str,
    ) -> Result<EditLocation, SourceDocumentError> {
        let path = format!("notes.{note_id}.fields.{field_id}.value");
        let mut next = self.clone();
        if let Some(location) =
            next.includes
                .edit_scalar(&path, expected, replacement, &next.provenance)?
        {
            next.validate()?;
            *self = next;
            return Ok(location);
        }
        let change = next
            .overlay
            .note_changes
            .get_mut(note_id)
            .and_then(|note| note.fields.get_mut(field_id))
            .ok_or_else(|| {
                SourceDocumentError::at(
                    &next.provenance,
                    &path,
                    "sparse note field change is not present",
                )
            })?;
        let current = change.value.as_mut().ok_or_else(|| {
            SourceDocumentError::at(
                &next.provenance,
                &path,
                "field change is not represented as scalar text",
            )
        })?;
        if current != expected {
            return Err(SourceDocumentError::at(
                &next.provenance,
                &path,
                format!("expected {expected:?}, found {current:?}"),
            ));
        }
        *current = replacement.to_owned();
        next.validate()?;
        *self = next;
        Ok(EditLocation::Root)
    }

    /// Compare-and-set one inline overlay media hash.
    pub fn set_media_hash(
        &mut self,
        media_id: &StableId,
        expected_path: &str,
        sha256: &str,
    ) -> Result<EditLocation, SourceDocumentError> {
        let path = format!("media.{media_id}.sha256");
        let mut next = self.clone();
        let media = next
            .overlay
            .media_changes
            .get_mut(media_id)
            .and_then(|change| change.media.as_mut())
            .ok_or_else(|| {
                SourceDocumentError::at(
                    &next.provenance,
                    &path,
                    "overlay media payload is not present",
                )
            })?;
        if media.path != expected_path {
            return Err(SourceDocumentError::at(
                &next.provenance,
                &path,
                format!(
                    "expected media path {expected_path:?}, found {:?}",
                    media.path
                ),
            ));
        }
        media.sha256 = sha256.to_owned();
        next.validate()?;
        *self = next;
        Ok(EditLocation::Root)
    }

    /// Convert strict image HTML in added notes and sparse field changes.
    pub fn convert_strict_image_fields(
        &mut self,
        lookup: &BTreeMap<String, Option<StableId>>,
    ) -> Result<ImageConversionReport, SourceDocumentError> {
        let mut next = self.clone();
        let mut report = ImageConversionReport::default();
        for note_change in next.overlay.note_changes.values_mut() {
            if let Some(note) = &mut note_change.note {
                let field_ids = note.fields.keys().cloned().collect::<Vec<_>>();
                for field_id in field_ids {
                    if note.field_messages.contains_key(&field_id)
                        || note.field_images.contains_key(&field_id)
                    {
                        continue;
                    }
                    if let Some(images) =
                        convert_text_to_images(&note.fields[&field_id], lookup, &mut report)
                    {
                        note.fields.insert(field_id.clone(), String::new());
                        note.field_images.insert(field_id, images);
                    }
                }
            }
            for field_change in note_change.fields.values_mut() {
                let Some(text) = field_change.value.as_deref() else {
                    continue;
                };
                if let Some(images) = convert_text_to_images(text, lookup, &mut report) {
                    field_change.value = None;
                    field_change.images = Some(images);
                }
            }
        }
        next.validate()?;
        *self = next;
        Ok(report)
    }

    pub fn emit(&self) -> Result<SourceDocumentEmission, SourceDocumentError> {
        self.validate()?;
        let canonical = canonical_yaml::overlay_to_string(&self.overlay)
            .map_err(|error| SourceDocumentError::source(&self.provenance, error.to_string()))?;
        let canonical = self.includes.restore_directives(canonical)?;
        crate::strict_yaml::reject_duplicate_keys(&canonical)
            .map_err(|error| SourceDocumentError::source(&self.provenance, error.to_string()))?;
        Ok(SourceDocumentEmission::new(
            SourceFile::new(self.provenance.clone(), canonical),
            self.includes.changed_sources()?,
        ))
    }

    fn validate(&self) -> Result<(), SourceDocumentError> {
        canonical_yaml::overlay_to_string(&self.overlay)
            .map(|_| ())
            .map_err(|error| SourceDocumentError::source(&self.provenance, error.to_string()))
    }
}

fn remove_contextual_for_path(translations: &mut TranslationDictionary, path: &str, source: &str) {
    for context in matching_contexts(&translations.contextual, path, source) {
        if let Some(replacements) = translations.contextual.get_mut(&context) {
            replacements.remove(source);
            if replacements.is_empty() {
                translations.contextual.remove(&context);
            }
        }
    }
}

fn remove_contextual_everywhere(translations: &mut TranslationDictionary, source: &str) {
    let contexts = translations
        .contextual
        .iter()
        .filter(|(_, replacements)| replacements.contains_key(source))
        .map(|(context, _)| context.clone())
        .collect::<Vec<_>>();
    for context in contexts {
        if let Some(replacements) = translations.contextual.get_mut(&context) {
            replacements.remove(source);
            if replacements.is_empty() {
                translations.contextual.remove(&context);
            }
        }
    }
}

fn remove_stale_for_path_source(
    translations: &mut TranslationDictionary,
    path: &str,
    source: &str,
) {
    translations.stale_translations.retain(|record| {
        !(record.new_source == source
            && record.context.as_deref().is_none_or(|context| {
                context == path
                    || path
                        .strip_prefix(context)
                        .is_some_and(|suffix| suffix.starts_with('.'))
            }))
    });
}
