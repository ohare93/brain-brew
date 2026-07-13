//! Include-preserving, validated overlay source editing.
//!
//! Translation precedence cleanup, sparse field edits, media hashes, and image
//! conversion are centralized here so callers never mutate YAML maps directly.

use std::collections::BTreeMap;

use brain_brew_core::{FieldValue, Overlay, StableId, TranslationDictionary};

use crate::canonical_yaml;
use crate::source_document::{
    EditLocation, ImageConversionReport, IncludeRequest, IncludeState, IncludedSource,
    SourceDocumentEmission, SourceDocumentError, SourceFile, SourceProvenance,
    convert_text_to_images, ensure_non_empty, prepare_source,
};

pub use brain_brew_core::{SourceTranslationImpact, TranslationDecision, TranslationStubs};

/// Deep source module for one sparse overlay and its scalar includes.
#[derive(Clone)]
pub struct OverlaySourceDocument {
    provenance: SourceProvenance,
    overlay: Overlay,
    resolved_overlay: Overlay,
    includes: IncludeState,
    original_sources: BTreeMap<SourceProvenance, SourceFile>,
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
        let resolved_overlay = canonical_yaml::overlay_from_str(&prepared.materialized_yaml)
            .map_err(|error| {
                SourceDocumentError::source(prepared.root.provenance(), error.to_string())
            })?;
        canonical_yaml::overlay_to_string(&overlay).map_err(|error| {
            SourceDocumentError::source(prepared.root.provenance(), error.to_string())
        })?;
        let original_sources = prepared.original_sources()?;
        Ok(Self {
            provenance: prepared.root.provenance().clone(),
            overlay,
            resolved_overlay,
            includes: prepared.includes,
            original_sources,
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
            resolved_overlay: overlay.clone(),
            overlay,
            includes: IncludeState::default(),
            original_sources: BTreeMap::new(),
        })
    }

    pub fn provenance(&self) -> &SourceProvenance {
        &self.provenance
    }

    /// Every scalar source loaded by this document.
    pub fn included_sources(&self) -> Vec<IncludedSource> {
        self.includes.source_provenance()
    }

    /// Read-only validated domain view. Mutation remains behind typed methods.
    pub fn overlay(&self) -> &Overlay {
        &self.overlay
    }

    /// Fully materialized read view with scalar include contents substituted.
    pub fn resolved_overlay(&self) -> &Overlay {
        &self.resolved_overlay
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
        let included_target = match &decision {
            TranslationDecision::Direct(target) => Some((
                format!("translations.direct.{source}"),
                next.resolved_overlay
                    .translations
                    .as_ref()
                    .and_then(|translations| translations.direct.get(source))
                    .cloned(),
                target.as_str(),
            )),
            TranslationDecision::Contextual { context, target } => Some((
                format!("translations.contextual.{context}.{source}"),
                next.resolved_overlay
                    .translations
                    .as_ref()
                    .and_then(|translations| translations.contextual.get(context))
                    .and_then(|replacements| replacements.get(source))
                    .cloned(),
                target.as_str(),
            )),
            TranslationDecision::NoChange => None,
        };
        if let Some((path, Some(expected), target)) = included_target
            && next
                .includes
                .edit_scalar(&path, &expected, target, &next.provenance)?
                .is_some()
        {
            next.overlay
                .translations
                .get_or_insert_with(TranslationDictionary::default)
                .clear_superseded_translation_decisions(occurrence_path, source, &decision)
                .map_err(|error| {
                    SourceDocumentError::at(&next.provenance, &error.path, error.message)
                })?;
            apply_translation_decision(
                &mut next.resolved_overlay,
                occurrence_path,
                source,
                decision,
                &next.provenance,
            )?;
            next.validate()?;
            *self = next;
            return Ok(());
        }
        apply_translation_decision(
            &mut next.overlay,
            occurrence_path,
            source,
            decision.clone(),
            &next.provenance,
        )?;
        apply_translation_decision(
            &mut next.resolved_overlay,
            occurrence_path,
            source,
            decision,
            &next.provenance,
        )?;
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
        let context = match &impact {
            SourceTranslationImpact::MarkStale { context, .. }
            | SourceTranslationImpact::MigrateKey { context, .. } => context.as_deref(),
        };
        let old_path = context.map_or_else(
            || format!("translations.direct.{old_source}"),
            |context| format!("translations.contextual.{context}.{old_source}"),
        );
        let mut root_impact = impact.clone();
        if let Some(sentinel) = next.includes.scalar_sentinel(&old_path).map(str::to_owned) {
            match &impact {
                SourceTranslationImpact::MarkStale { .. } => {
                    next.includes.remove_scalar(&old_path);
                }
                SourceTranslationImpact::MigrateKey { context, .. } => {
                    let new_path = context.as_ref().map_or_else(
                        || format!("translations.direct.{new_source}"),
                        |context| format!("translations.contextual.{context}.{new_source}"),
                    );
                    next.includes.move_scalar(&old_path, new_path);
                    root_impact = SourceTranslationImpact::MigrateKey {
                        target: sentinel,
                        context: context.clone(),
                    };
                }
            }
        }
        apply_source_impact(
            &mut next.overlay,
            occurrence_path,
            old_source,
            new_source,
            root_impact,
            &next.provenance,
        )?;
        apply_source_impact(
            &mut next.resolved_overlay,
            occurrence_path,
            old_source,
            new_source,
            impact,
            &next.provenance,
        )?;
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
        next.overlay
            .translations
            .get_or_insert_with(TranslationDictionary::default)
            .add_translation_stubs(stubs.clone())
            .map_err(|error| {
                SourceDocumentError::at(&next.provenance, &error.path, error.message)
            })?;
        next.resolved_overlay
            .translations
            .get_or_insert_with(TranslationDictionary::default)
            .add_translation_stubs(stubs)
            .map_err(|error| {
                SourceDocumentError::at(&next.provenance, &error.path, error.message)
            })?;
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
        next.overlay
            .translations
            .as_mut()
            .ok_or_else(|| {
                SourceDocumentError::at(
                    &next.provenance,
                    "translations.stale_translations",
                    "overlay has no translation dictionary",
                )
            })?
            .resolve_stale_translation_decision(old_source, new_source, context, replacement)
            .map_err(|error| {
                SourceDocumentError::at(&next.provenance, &error.path, error.message)
            })?;
        next.resolved_overlay
            .translations
            .as_mut()
            .ok_or_else(|| {
                SourceDocumentError::at(
                    &next.provenance,
                    "translations.stale_translations",
                    "overlay has no translation dictionary",
                )
            })?
            .resolve_stale_translation_decision(old_source, new_source, context, replacement)
            .map_err(|error| {
                SourceDocumentError::at(&next.provenance, &error.path, error.message)
            })?;
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
        let current = change
            .value
            .as_mut()
            .and_then(FieldValue::as_scalar_mut)
            .ok_or_else(|| {
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
                    let Some(text) = note.fields[&field_id].as_scalar() else {
                        continue;
                    };
                    if let Some(images) = convert_text_to_images(text, lookup, &mut report) {
                        note.fields.insert(field_id, FieldValue::Images(images));
                    }
                }
            }
            for field_change in note_change.fields.values_mut() {
                let Some(text) = field_change.value.as_ref().and_then(FieldValue::as_scalar) else {
                    continue;
                };
                if let Some(images) = convert_text_to_images(text, lookup, &mut report) {
                    field_change.value = Some(FieldValue::Images(images));
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
            self.original_sources.clone(),
        ))
    }

    fn validate(&self) -> Result<(), SourceDocumentError> {
        canonical_yaml::overlay_to_string(&self.overlay)
            .map(|_| ())
            .map_err(|error| SourceDocumentError::source(&self.provenance, error.to_string()))
    }
}

fn apply_translation_decision(
    overlay: &mut Overlay,
    occurrence_path: &str,
    source: &str,
    decision: TranslationDecision,
    provenance: &SourceProvenance,
) -> Result<(), SourceDocumentError> {
    overlay
        .translations
        .get_or_insert_with(TranslationDictionary::default)
        .set_translation_decision(occurrence_path, source, decision)
        .map_err(|error| SourceDocumentError::at(provenance, &error.path, error.message))
}

fn apply_source_impact(
    overlay: &mut Overlay,
    occurrence_path: &str,
    old_source: &str,
    new_source: &str,
    impact: SourceTranslationImpact,
    provenance: &SourceProvenance,
) -> Result<(), SourceDocumentError> {
    overlay
        .translations
        .get_or_insert_with(TranslationDictionary::default)
        .apply_source_translation_impact(occurrence_path, old_source, new_source, impact)
        .map_err(|error| SourceDocumentError::at(provenance, &error.path, error.message))
}
