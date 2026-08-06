//! Include-preserving, validated overlay source editing.
//!
//! Translation precedence cleanup, sparse field edits, media hashes, and image
//! conversion are centralized here so callers never mutate YAML maps directly.

use std::collections::{BTreeMap, BTreeSet};

use brain_brew_core::{
    AdapterIds, CanonicalDeck, ChangeIntent, FieldChange, FieldDefinition, FieldValue, NoteChange,
    NoteType, Overlay, StableId, TranslationDictionary,
};
use serde_yaml::Value;

use crate::csv_note_source::{
    CsvNoteSourceDescriptor, CsvNoteSourceMaterializer, CsvSourceFile, CsvSourceRequest,
    CsvSourceRequestKind, CsvSparseFieldSourceDeclaration, CsvTranslationAuthoringProvenance,
    CsvTranslationPair, CsvTranslationSourceDeclaration, CsvTranslationSourceMaterializer,
    ExcludedCsvTranslationOccurrence,
};

use crate::canonical_source_document::{NoteAuthoringLocation, NoteAuthoringProvenance};
use crate::canonical_yaml;
use crate::source_document::{
    EditLocation, ImageConversionReport, IncludeRequest, IncludeState, IncludedSource,
    SourceDocumentEmission, SourceDocumentError, SourceFile, SourceProvenance,
    convert_text_to_images, ensure_non_empty, prepare_source,
};

pub use brain_brew_core::{SourceTranslationImpact, TranslationDecision, TranslationStubs};

#[derive(Clone, Copy)]
enum CsvMaterialization<'a> {
    Inventory,
    SparseFields(&'a CanonicalDeck),
    All {
        source_deck: &'a CanonicalDeck,
        occurrence_deck: &'a CanonicalDeck,
    },
}

/// Deep source module for one sparse overlay and its scalar includes.
#[derive(Clone)]
pub struct OverlaySourceDocument {
    provenance: SourceProvenance,
    overlay: Overlay,
    resolved_overlay: Overlay,
    includes: IncludeState,
    csv_translation_sources: Vec<CsvTranslationSourceDeclaration>,
    csv_translation_provenance: CsvTranslationAuthoringProvenance,
    csv_sparse_field_sources: BTreeMap<StableId, Vec<CsvSparseFieldSourceDeclaration>>,
    csv_sparse_field_provenance: NoteAuthoringProvenance,
    csv_sources: Vec<(CsvSourceRequestKind, CsvSourceFile)>,
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
        Self::parse_with_loaders(
            source,
            CsvMaterialization::Inventory,
            &mut loader,
            &mut |request| {
                Err(format!(
                    "no CSV source loader was provided for {:?}",
                    request.target()
                ))
            },
        )
    }

    /// Parse and materialize CSV translation declarations against the complete source deck.
    pub fn parse_with_csv_translations(
        source: SourceFile,
        source_deck: &CanonicalDeck,
        include_loader: impl FnMut(&IncludeRequest) -> Result<SourceFile, String>,
        csv_loader: impl FnMut(&CsvSourceRequest) -> Result<CsvSourceFile, String>,
    ) -> Result<Self, SourceDocumentError> {
        Self::parse_with_csv_translations_and_occurrences(
            source,
            source_deck,
            source_deck,
            include_loader,
            csv_loader,
        )
    }

    /// Materialize CSV translations against the current deck while classifying
    /// reusable text against all occurrences in the resolved target stack.
    pub fn parse_with_csv_translations_and_occurrences(
        source: SourceFile,
        source_deck: &CanonicalDeck,
        occurrence_deck: &CanonicalDeck,
        mut include_loader: impl FnMut(&IncludeRequest) -> Result<SourceFile, String>,
        mut csv_loader: impl FnMut(&CsvSourceRequest) -> Result<CsvSourceFile, String>,
    ) -> Result<Self, SourceDocumentError> {
        Self::parse_with_loaders(
            source,
            CsvMaterialization::All {
                source_deck,
                occurrence_deck,
            },
            &mut include_loader,
            &mut csv_loader,
        )
    }

    /// Materialize sparse CSV field additions while leaving translations inventoried only.
    pub fn parse_with_csv_sparse_fields(
        source: SourceFile,
        source_deck: &CanonicalDeck,
        mut include_loader: impl FnMut(&IncludeRequest) -> Result<SourceFile, String>,
        mut csv_loader: impl FnMut(&CsvSourceRequest) -> Result<CsvSourceFile, String>,
    ) -> Result<Self, SourceDocumentError> {
        Self::parse_with_loaders(
            source,
            CsvMaterialization::SparseFields(source_deck),
            &mut include_loader,
            &mut csv_loader,
        )
    }

    /// Parse and authorize every CSV input without materializing deck-dependent decisions.
    pub fn parse_with_csv_inventory(
        source: SourceFile,
        mut include_loader: impl FnMut(&IncludeRequest) -> Result<SourceFile, String>,
        mut csv_loader: impl FnMut(&CsvSourceRequest) -> Result<CsvSourceFile, String>,
    ) -> Result<Self, SourceDocumentError> {
        Self::parse_with_loaders(
            source,
            CsvMaterialization::Inventory,
            &mut include_loader,
            &mut csv_loader,
        )
    }

    fn parse_with_loaders(
        source: SourceFile,
        materialization: CsvMaterialization<'_>,
        include_loader: &mut impl FnMut(&IncludeRequest) -> Result<SourceFile, String>,
        csv_loader: &mut impl FnMut(&CsvSourceRequest) -> Result<CsvSourceFile, String>,
    ) -> Result<Self, SourceDocumentError> {
        let prepared = prepare_source(source, false, include_loader)?;
        let (source_yaml, translation_declarations) = strip_csv_translation_declarations(
            &prepared.yaml_without_directives,
            prepared.root.provenance(),
        )?;
        let (resolved_yaml, resolved_translation_declarations) =
            strip_csv_translation_declarations(
                &prepared.materialized_yaml,
                prepared.root.provenance(),
            )?;
        if translation_declarations != resolved_translation_declarations {
            return Err(SourceDocumentError::at(
                prepared.root.provenance(),
                "translations.from_csv",
                "scalar include materialization changed CSV translation declarations",
            ));
        }
        let (source_yaml, sparse_declarations) =
            strip_csv_sparse_field_declarations(&source_yaml, prepared.root.provenance())?;
        let (resolved_yaml, resolved_sparse_declarations) =
            strip_csv_sparse_field_declarations(&resolved_yaml, prepared.root.provenance())?;
        if sparse_declarations != resolved_sparse_declarations {
            return Err(SourceDocumentError::at(
                prepared.root.provenance(),
                "field_additions",
                "scalar include materialization changed sparse CSV field declarations",
            ));
        }
        let overlay = canonical_yaml::overlay_from_str(&source_yaml).map_err(|error| {
            SourceDocumentError::source(prepared.root.provenance(), error.to_string())
        })?;
        let mut resolved_overlay =
            canonical_yaml::overlay_from_str(&resolved_yaml).map_err(|error| {
                SourceDocumentError::source(prepared.root.provenance(), error.to_string())
            })?;
        let sparse_source_deck = match materialization {
            CsvMaterialization::Inventory => None,
            CsvMaterialization::SparseFields(deck)
            | CsvMaterialization::All {
                source_deck: deck, ..
            } => Some(deck),
        };
        let (csv_sparse_field_provenance, mut csv_sources) = load_csv_sparse_field_sources(
            &sparse_declarations,
            sparse_source_deck,
            &mut resolved_overlay,
            prepared.root.provenance(),
            csv_loader,
        )?;
        let translation_decks = match materialization {
            CsvMaterialization::All {
                source_deck,
                occurrence_deck,
            } => Some((source_deck, occurrence_deck)),
            CsvMaterialization::Inventory | CsvMaterialization::SparseFields(_) => None,
        };
        let (csv_translation_provenance, translation_sources) = load_csv_translation_sources(
            &translation_declarations,
            translation_decks,
            &mut resolved_overlay,
            prepared.root.provenance(),
            csv_loader,
        )?;
        csv_sources.extend(translation_sources);
        canonical_yaml::overlay_to_string(&overlay).map_err(|error| {
            SourceDocumentError::source(prepared.root.provenance(), error.to_string())
        })?;
        let original_sources = prepared.original_sources()?;
        Ok(Self {
            provenance: prepared.root.provenance().clone(),
            overlay,
            resolved_overlay,
            includes: prepared.includes,
            csv_translation_sources: translation_declarations,
            csv_translation_provenance,
            csv_sparse_field_sources: sparse_declarations,
            csv_sparse_field_provenance,
            csv_sources,
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
            csv_translation_sources: Vec::new(),
            csv_translation_provenance: CsvTranslationAuthoringProvenance::default(),
            csv_sparse_field_sources: BTreeMap::new(),
            csv_sparse_field_provenance: NoteAuthoringProvenance::default(),
            csv_sources: Vec::new(),
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

    pub fn csv_translation_sources(&self) -> &[CsvTranslationSourceDeclaration] {
        &self.csv_translation_sources
    }

    pub fn csv_translation_provenance(&self) -> &CsvTranslationAuthoringProvenance {
        &self.csv_translation_provenance
    }

    pub fn csv_sparse_field_sources(
        &self,
    ) -> &BTreeMap<StableId, Vec<CsvSparseFieldSourceDeclaration>> {
        &self.csv_sparse_field_sources
    }

    pub fn csv_sparse_field_provenance(&self) -> &NoteAuthoringProvenance {
        &self.csv_sparse_field_provenance
    }

    /// Every authoritative descriptor and table loaded by CSV declarations.
    pub fn csv_sources(&self) -> &[(CsvSourceRequestKind, CsvSourceFile)] {
        &self.csv_sources
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
        let resolved = next
            .resolved_overlay
            .note_changes
            .get_mut(note_id)
            .and_then(|note| note.fields.get_mut(field_id))
            .and_then(|change| change.value.as_mut())
            .and_then(FieldValue::as_scalar_mut)
            .ok_or_else(|| {
                SourceDocumentError::at(
                    &next.provenance,
                    &path,
                    "resolved sparse note field change is not represented as scalar text",
                )
            })?;
        if resolved != expected {
            return Err(SourceDocumentError::at(
                &next.provenance,
                &path,
                format!("expected resolved value {expected:?}, found {resolved:?}"),
            ));
        }
        *resolved = replacement.to_owned();
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
        let canonical =
            restore_csv_sparse_field_declarations(canonical, &self.csv_sparse_field_sources)
                .map_err(|message| {
                    SourceDocumentError::at(&self.provenance, "field_additions", message)
                })?;
        let canonical =
            restore_csv_translation_declarations(canonical, &self.csv_translation_sources)
                .map_err(|message| {
                    SourceDocumentError::at(&self.provenance, "translations.from_csv", message)
                })?;
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

fn strip_csv_sparse_field_declarations(
    yaml: &str,
    provenance: &SourceProvenance,
) -> Result<
    (
        String,
        BTreeMap<StableId, Vec<CsvSparseFieldSourceDeclaration>>,
    ),
    SourceDocumentError,
> {
    let mut root = serde_yaml::from_str::<Value>(yaml)
        .map_err(|error| SourceDocumentError::source(provenance, error.to_string()))?;
    let Some(root_mapping) = root.as_mapping_mut() else {
        return Ok((yaml.to_owned(), BTreeMap::new()));
    };
    let Some(field_additions) = root_mapping
        .get_mut(Value::String("field_additions".to_owned()))
        .and_then(Value::as_mapping_mut)
    else {
        return Ok((yaml.to_owned(), BTreeMap::new()));
    };
    let mut declarations = BTreeMap::new();
    for (note_type_key, addition) in field_additions {
        let Some(note_type) = note_type_key.as_str() else {
            continue;
        };
        let note_type_id = StableId::new(note_type.to_owned()).map_err(|error| {
            SourceDocumentError::at(provenance, "field_additions", error.to_string())
        })?;
        let Some(values) = addition
            .as_mapping_mut()
            .and_then(|addition| addition.get_mut(Value::String("values".to_owned())))
            .and_then(Value::as_mapping_mut)
        else {
            continue;
        };
        let Some(from_csv) = values.remove(Value::String("from_csv".to_owned())) else {
            continue;
        };
        let Value::Sequence(entries) = from_csv else {
            return Err(SourceDocumentError::at(
                provenance,
                format!("field_additions.{note_type_id}.values.from_csv"),
                "sparse field values from_csv must be a non-empty sequence",
            ));
        };
        if entries.is_empty() {
            return Err(SourceDocumentError::at(
                provenance,
                format!("field_additions.{note_type_id}.values.from_csv"),
                "sparse field values from_csv must be a non-empty sequence",
            ));
        }
        let parsed = entries
            .into_iter()
            .enumerate()
            .map(|(index, entry)| {
                CsvSparseFieldSourceDeclaration::parse(entry, provenance).map_err(|error| {
                    SourceDocumentError::at(
                        provenance,
                        format!("field_additions.{note_type_id}.values.from_csv[{index}]"),
                        error.to_string(),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        declarations.insert(note_type_id, parsed);
    }
    let yaml = serde_yaml::to_string(&root)
        .map_err(|error| SourceDocumentError::source(provenance, error.to_string()))?;
    Ok((yaml, declarations))
}

fn restore_csv_sparse_field_declarations(
    mut canonical: String,
    declarations: &BTreeMap<StableId, Vec<CsvSparseFieldSourceDeclaration>>,
) -> Result<String, String> {
    for (note_type_id, sources) in declarations {
        let key = crate::yaml_scalar::key(note_type_id.as_str())
            .ok_or_else(|| format!("note type ID {note_type_id:?} cannot be emitted"))?;
        let note_type_marker = format!("  {key}:\n");
        let note_type_start = canonical
            .find(&note_type_marker)
            .ok_or_else(|| format!("canonical field additions omitted {note_type_id}"))?;
        let body_start = note_type_start + note_type_marker.len();
        let mut body_end = canonical.len();
        let mut offset = body_start;
        for line in canonical[body_start..].split_inclusive('\n') {
            if !line.trim().is_empty()
                && (!line.starts_with(' ') || (line.starts_with("  ") && !line.starts_with("    ")))
            {
                body_end = offset;
                break;
            }
            offset += line.len();
        }
        let mut section = String::from("    values:\n      from_csv:\n");
        for source in sources {
            section.push_str(&source.emit("        ")?);
        }
        if let Some(values_relative) = canonical[body_start..body_end].find("    values:") {
            let values_start = body_start + values_relative;
            let values_line_end = canonical[values_start..]
                .find('\n')
                .map_or(canonical.len(), |offset| values_start + offset + 1);
            let values_line = canonical[values_start..values_line_end].trim_end();
            if values_line == "values: {}" || values_line == "    values: {}" {
                canonical.replace_range(values_start..values_line_end, &section);
            } else if values_line == "values:" || values_line == "    values:" {
                canonical.insert_str(values_line_end, &section["    values:\n".len()..]);
            } else {
                return Err(format!(
                    "canonical field-addition values for {note_type_id} have an unexpected shape"
                ));
            }
        } else {
            canonical.insert_str(body_end, &section);
        }
    }
    Ok(canonical)
}

fn load_csv_sparse_field_sources(
    declarations: &BTreeMap<StableId, Vec<CsvSparseFieldSourceDeclaration>>,
    source_deck: Option<&CanonicalDeck>,
    resolved_overlay: &mut Overlay,
    root: &SourceProvenance,
    csv_loader: &mut impl FnMut(&CsvSourceRequest) -> Result<CsvSourceFile, String>,
) -> Result<
    (
        NoteAuthoringProvenance,
        Vec<(CsvSourceRequestKind, CsvSourceFile)>,
    ),
    SourceDocumentError,
> {
    let inline_overlay = resolved_overlay.clone();
    let mut provenance = NoteAuthoringProvenance::default();
    let mut loaded_sources = Vec::new();
    let mut csv_owned = BTreeSet::<(StableId, StableId)>::new();
    for (note_type_id, sources) in declarations {
        for (index, declaration) in sources.iter().enumerate() {
            let declaration_path =
                format!("field_additions.{note_type_id}.values.from_csv[{index}]");
            let descriptor_path = format!("{declaration_path}.descriptor");
            let descriptor_request = CsvSourceRequest::descriptor(
                root.clone(),
                descriptor_path.clone(),
                declaration.descriptor().to_owned(),
            );
            let descriptor_bytes = csv_loader(&descriptor_request).map_err(|message| {
                SourceDocumentError::at(
                    root,
                    &descriptor_path,
                    format!(
                        "could not load sparse field CSV descriptor {:?}: {message}",
                        declaration.descriptor()
                    ),
                )
            })?;
            loaded_sources.push((CsvSourceRequestKind::Descriptor, descriptor_bytes.clone()));
            let descriptor_text =
                std::str::from_utf8(descriptor_bytes.bytes()).map_err(|error| {
                    SourceDocumentError::at(
                        descriptor_bytes.provenance(),
                        &descriptor_path,
                        format!("descriptor is not valid UTF-8: {error}"),
                    )
                })?;
            let descriptor = CsvNoteSourceDescriptor::parse(SourceFile::new(
                descriptor_bytes.provenance().clone(),
                descriptor_text,
            ))
            .map_err(|error| SourceDocumentError::at(root, &declaration_path, error.to_string()))?;
            if descriptor.note_type_id() != *note_type_id {
                return Err(SourceDocumentError::at(
                    root,
                    &declaration_path,
                    format!(
                        "sparse CSV descriptor maps note type {}, but declaration belongs to {note_type_id}",
                        descriptor.note_type_id()
                    ),
                ));
            }
            let mapped_fields = descriptor.mapped_field_ids().collect::<Vec<_>>();
            let Some(note_type_change) = inline_overlay.note_type_changes.get(note_type_id) else {
                return Err(SourceDocumentError::at(
                    root,
                    &declaration_path,
                    format!("sparse CSV values have no field additions for {note_type_id}"),
                ));
            };
            for field_id in &mapped_fields {
                let owned = note_type_change.fields.get(field_id).is_some_and(|change| {
                    change.intent == ChangeIntent::Add && change.field.is_some()
                });
                if !owned {
                    return Err(SourceDocumentError::at(
                        root,
                        &declaration_path,
                        format!(
                            "sparse CSV field mapping {field_id} is not added by field_additions.{note_type_id}.fields"
                        ),
                    ));
                }
            }
            let descriptor_provenance = descriptor.provenance().clone();
            let materializer = CsvNoteSourceMaterializer::new(descriptor)
                .with_parameters(declaration.parameters())
                .map_err(|error| {
                    SourceDocumentError::at(root, &declaration_path, error.to_string())
                })?;
            let requests = materializer
                .table_paths()
                .map(|(alias, target)| {
                    CsvSourceRequest::table(
                        descriptor_provenance.clone(),
                        format!("{declaration_path}.tables.{alias}"),
                        alias.to_owned(),
                        target.to_owned(),
                    )
                })
                .collect::<Vec<_>>();
            let mut tables = BTreeMap::new();
            for request in requests {
                let alias = match request.kind() {
                    CsvSourceRequestKind::Table { alias } => alias.clone(),
                    CsvSourceRequestKind::Descriptor => unreachable!(),
                };
                let table = csv_loader(&request).map_err(|message| {
                    SourceDocumentError::at(
                        root,
                        &declaration_path,
                        format!("could not load CSV table {:?}: {message}", request.target()),
                    )
                })?;
                loaded_sources.push((request.kind().clone(), table.clone()));
                tables.insert(alias, table);
            }
            let Some(source_deck) = source_deck else {
                continue;
            };
            let synthetic_note_type = NoteType {
                id: note_type_id.clone(),
                name: note_type_id.to_string(),
                variables: BTreeMap::new(),
                fields: mapped_fields
                    .iter()
                    .cloned()
                    .map(|id| FieldDefinition {
                        name: id.to_string(),
                        id,
                        rtl: false,
                        message_pattern: None,
                    })
                    .collect(),
                card_templates: Vec::new(),
                styling: String::new(),
                adapter_ids: AdapterIds::new(),
            };
            let materialized = materializer
                .materialize_with_provenance(
                    &tables,
                    &BTreeMap::from([(note_type_id.clone(), synthetic_note_type)]),
                )
                .map_err(|error| {
                    SourceDocumentError::at(root, &declaration_path, error.to_string())
                })?;
            let excluded = declaration
                .excluded_note_ids()
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>();
            let mut matched_exclusions = BTreeSet::new();
            for (note_id, note) in materialized.notes {
                let non_empty = note
                    .fields
                    .into_iter()
                    .filter(|(_, value)| !field_value_is_empty(value))
                    .collect::<Vec<_>>();
                if non_empty.is_empty() {
                    continue;
                }
                let source_note = source_deck.notes.get(&note_id).ok_or_else(|| {
                    SourceDocumentError::at(
                        root,
                        &declaration_path,
                        format!("sparse CSV value references unknown note {note_id}"),
                    )
                })?;
                if source_note.note_type_id != *note_type_id {
                    return Err(SourceDocumentError::at(
                        root,
                        &declaration_path,
                        format!(
                            "sparse CSV note {note_id} has note type {}, expected {note_type_id}",
                            source_note.note_type_id
                        ),
                    ));
                }
                for (field_id, value) in non_empty {
                    let path = format!("notes.{note_id}.fields.{field_id}");
                    let inline = inline_overlay
                        .note_changes
                        .get(&note_id)
                        .and_then(|change| change.fields.get(field_id));
                    if excluded.contains(&note_id) {
                        matched_exclusions.insert(note_id.clone());
                        let covered = inline.is_some_and(|change| {
                            change.intent == ChangeIntent::Add
                                && change.value.as_ref() == Some(value)
                        });
                        if !covered {
                            return Err(SourceDocumentError::at(
                                root,
                                &declaration_path,
                                format!(
                                    "excluded sparse CSV value at {path} has missing or conflicting inline ownership"
                                ),
                            ));
                        }
                        continue;
                    }
                    if inline.is_some() {
                        return Err(SourceDocumentError::at(
                            root,
                            &declaration_path,
                            format!("sparse CSV value at {path} conflicts with inline ownership"),
                        ));
                    }
                    if !csv_owned.insert((note_id.clone(), field_id.clone())) {
                        return Err(SourceDocumentError::at(
                            root,
                            &declaration_path,
                            format!("sparse CSV value at {path} has duplicate CSV ownership"),
                        ));
                    }
                    let cell = &materialized.field_provenance[&(note_id.clone(), field_id.clone())];
                    let location = NoteAuthoringLocation::csv_field(
                        root.clone(),
                        declaration_path.clone(),
                        descriptor_provenance.clone(),
                        cell,
                        path,
                    );
                    provenance.insert_field(note_id.clone(), field_id.clone(), location);
                    let note_change = resolved_overlay
                        .note_changes
                        .entry(note_id.clone())
                        .or_insert_with(empty_note_merge_change);
                    if note_change.intent != ChangeIntent::Merge {
                        return Err(SourceDocumentError::at(
                            root,
                            &declaration_path,
                            "sparse CSV values can only merge into existing notes",
                        ));
                    }
                    note_change.fields.insert(
                        field_id.clone(),
                        FieldChange {
                            intent: ChangeIntent::Add,
                            value: Some(value.clone()),
                            expected_base: None,
                        },
                    );
                }
            }
            if let Some(unmatched) = excluded.difference(&matched_exclusions).next() {
                return Err(SourceDocumentError::at(
                    root,
                    &declaration_path,
                    format!(
                        "excluded note ID {unmatched} matched no otherwise-importable sparse field value"
                    ),
                ));
            }
        }
    }
    Ok((provenance, loaded_sources))
}

fn field_value_is_empty(value: &FieldValue) -> bool {
    value.as_scalar().is_some_and(str::is_empty)
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

fn strip_csv_translation_declarations(
    yaml: &str,
    provenance: &SourceProvenance,
) -> Result<(String, Vec<CsvTranslationSourceDeclaration>), SourceDocumentError> {
    let mut root = serde_yaml::from_str::<Value>(yaml)
        .map_err(|error| SourceDocumentError::source(provenance, error.to_string()))?;
    let Some(root_mapping) = root.as_mapping_mut() else {
        return Ok((yaml.to_owned(), Vec::new()));
    };
    let translations_key = Value::String("translations".to_owned());
    let Some(translations) = root_mapping.get_mut(&translations_key) else {
        return Ok((yaml.to_owned(), Vec::new()));
    };
    let Some(translations) = translations.as_mapping_mut() else {
        return Ok((yaml.to_owned(), Vec::new()));
    };
    let from_csv_key = Value::String("from_csv".to_owned());
    let Some(from_csv) = translations.remove(&from_csv_key) else {
        return Ok((yaml.to_owned(), Vec::new()));
    };
    let Value::Sequence(entries) = from_csv else {
        return Err(SourceDocumentError::at(
            provenance,
            "translations.from_csv",
            "translations.from_csv must be a non-empty sequence",
        ));
    };
    if entries.is_empty() {
        return Err(SourceDocumentError::at(
            provenance,
            "translations.from_csv",
            "translations.from_csv must be a non-empty sequence",
        ));
    }
    let declarations = entries
        .into_iter()
        .enumerate()
        .map(|(index, entry)| {
            CsvTranslationSourceDeclaration::parse(entry, provenance).map_err(|error| {
                SourceDocumentError::at(
                    provenance,
                    format!("translations.from_csv[{index}]"),
                    error.to_string(),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let yaml = serde_yaml::to_string(&root)
        .map_err(|error| SourceDocumentError::source(provenance, error.to_string()))?;
    Ok((yaml, declarations))
}

fn restore_csv_translation_declarations(
    mut canonical: String,
    declarations: &[CsvTranslationSourceDeclaration],
) -> Result<String, String> {
    if declarations.is_empty() {
        return Ok(canonical);
    }
    let mut section = String::from("translations:\n  from_csv:\n");
    for declaration in declarations {
        section.push_str(&declaration.emit("    ")?);
    }
    let start = crate::strict_yaml::top_level_mapping_key_offset(&canonical, "translations")
        .ok_or_else(|| "canonical emission omitted translations".to_owned())?;
    let line_end = canonical[start..]
        .find('\n')
        .map_or(canonical.len(), |offset| start + offset + 1);
    let line = canonical[start..line_end].trim_end_matches(['\r', '\n']);
    if matches!(line, "translations: {}" | "translations:") {
        canonical.replace_range(start..line_end, &section);
    } else {
        return Err("canonical translations section has an unexpected shape".to_owned());
    }
    Ok(canonical)
}

fn load_csv_translation_sources(
    declarations: &[CsvTranslationSourceDeclaration],
    decks: Option<(&CanonicalDeck, &CanonicalDeck)>,
    resolved_overlay: &mut Overlay,
    root: &SourceProvenance,
    csv_loader: &mut impl FnMut(&CsvSourceRequest) -> Result<CsvSourceFile, String>,
) -> Result<
    (
        CsvTranslationAuthoringProvenance,
        Vec<(CsvSourceRequestKind, CsvSourceFile)>,
    ),
    SourceDocumentError,
> {
    let mut provenance = CsvTranslationAuthoringProvenance::default();
    let mut loaded_sources = Vec::new();
    let inline_translations = resolved_overlay.translations.clone().unwrap_or_default();
    let mut csv_path_owners = BTreeMap::<String, String>::new();
    let mut excluded_paths = BTreeMap::<String, String>::new();
    for (index, declaration) in declarations.iter().enumerate() {
        let declaration_path = format!("translations.from_csv[{index}]");
        let descriptor_path = format!("{declaration_path}.descriptor");
        let descriptor_request = CsvSourceRequest::descriptor(
            root.clone(),
            descriptor_path.clone(),
            declaration.descriptor().to_owned(),
        );
        let descriptor_bytes = csv_loader(&descriptor_request).map_err(|message| {
            SourceDocumentError::at(
                root,
                &descriptor_path,
                format!(
                    "could not load CSV translation descriptor {:?}: {message}",
                    declaration.descriptor()
                ),
            )
        })?;
        loaded_sources.push((CsvSourceRequestKind::Descriptor, descriptor_bytes.clone()));
        let descriptor_text = std::str::from_utf8(descriptor_bytes.bytes()).map_err(|error| {
            SourceDocumentError::at(
                descriptor_bytes.provenance(),
                &descriptor_path,
                format!("descriptor is not valid UTF-8: {error}"),
            )
        })?;
        let descriptor = CsvNoteSourceDescriptor::parse(SourceFile::new(
            descriptor_bytes.provenance().clone(),
            descriptor_text,
        ))
        .map_err(|error| SourceDocumentError::at(root, &declaration_path, error.to_string()))?;
        let descriptor_provenance = descriptor.provenance().clone();
        let materializer =
            CsvTranslationSourceMaterializer::new(descriptor, declaration.parameters()).map_err(
                |error| SourceDocumentError::at(root, &declaration_path, error.to_string()),
            )?;
        let requests = materializer
            .table_paths()
            .map(|(alias, target)| {
                CsvSourceRequest::table(
                    descriptor_provenance.clone(),
                    format!("{declaration_path}.tables.{alias}"),
                    alias.to_owned(),
                    target.to_owned(),
                )
            })
            .collect::<Vec<_>>();
        let mut tables = BTreeMap::new();
        for request in requests {
            let alias = match request.kind() {
                CsvSourceRequestKind::Table { alias } => alias.clone(),
                CsvSourceRequestKind::Descriptor => unreachable!(),
            };
            let table = csv_loader(&request).map_err(|message| {
                SourceDocumentError::at(
                    root,
                    &declaration_path,
                    format!("could not load CSV table {:?}: {message}", request.target()),
                )
            })?;
            loaded_sources.push((request.kind().clone(), table.clone()));
            tables.insert(alias, table);
        }
        if let Some((source_deck, occurrence_deck)) = decks {
            let mut materialized = materializer
                .materialize(&tables, source_deck, occurrence_deck)
                .map_err(|error| {
                    SourceDocumentError::at(root, &declaration_path, error.to_string())
                })?;
            let excluded = materialized
                .apply_exclusions(declaration, &declaration_path)
                .map_err(|message| SourceDocumentError::at(root, &declaration_path, message))?;
            validate_excluded_csv_translations(&inline_translations, &excluded)
                .map_err(|message| SourceDocumentError::at(root, &declaration_path, message))?;
            for occurrence in &excluded {
                let path = excluded_occurrence_path(occurrence);
                if let Some(owner) = csv_path_owners.get(path) {
                    return Err(SourceDocumentError::at(
                        root,
                        &declaration_path,
                        format!("excluded path {path} is still CSV-owned by {owner}"),
                    ));
                }
                excluded_paths.insert(path.to_owned(), declaration_path.clone());
            }
            for path in materialized.owned_paths() {
                if let Some(owner) = excluded_paths.get(path) {
                    return Err(SourceDocumentError::at(
                        root,
                        &declaration_path,
                        format!(
                            "CSV-owned path {path} conflicts with native ownership transferred by {owner}"
                        ),
                    ));
                }
                if let Some(owner) =
                    csv_path_owners.insert(path.to_owned(), declaration_path.clone())
                {
                    return Err(SourceDocumentError::at(
                        root,
                        &declaration_path,
                        format!("CSV-owned path {path} is already owned by {owner}"),
                    ));
                }
            }
            merge_csv_translations(
                resolved_overlay
                    .translations
                    .get_or_insert_with(TranslationDictionary::default),
                &materialized.translations,
                &materialized.owned_text,
            )
            .map_err(|message| SourceDocumentError::at(root, &declaration_path, message))?;
            provenance.merge(materialized.provenance);
        }
    }
    Ok((provenance, loaded_sources))
}

fn excluded_occurrence_path(occurrence: &ExcludedCsvTranslationOccurrence) -> &str {
    match occurrence {
        ExcludedCsvTranslationOccurrence::Text(pair) => &pair.path,
        ExcludedCsvTranslationOccurrence::Adaptation { path, .. } => path,
        ExcludedCsvTranslationOccurrence::Adapter(pair) => &pair.path,
    }
}

fn validate_excluded_csv_translations(
    inline: &TranslationDictionary,
    excluded: &[ExcludedCsvTranslationOccurrence],
) -> Result<(), String> {
    for occurrence in excluded {
        match occurrence {
            ExcludedCsvTranslationOccurrence::Text(pair) => {
                let covered = match effective_translation(inline, &pair.path, &pair.source) {
                    Some(EffectiveTranslation::Text(target)) => target == pair.target,
                    Some(EffectiveTranslation::Adaptation(adaptation)) => {
                        adaptation.ownership
                            == brain_brew_core::TargetAdaptationOwnership::Translation
                            && adaptation.expected_source == pair.source
                            && adaptation.target == pair.target
                    }
                    None => false,
                };
                if !covered {
                    return Err(format!(
                        "excluded CSV translation at {} for source {:?} has missing or conflicting inline ownership",
                        pair.path, pair.source
                    ));
                }
            }
            ExcludedCsvTranslationOccurrence::Adaptation {
                path, adaptation, ..
            } => {
                let covered = match effective_translation(inline, path, &adaptation.expected_source)
                {
                    Some(EffectiveTranslation::Text(target)) => target == adaptation.target,
                    Some(EffectiveTranslation::Adaptation(current)) => {
                        current.intent == adaptation.intent
                            && current.ownership == adaptation.ownership
                            && current.expected_source == adaptation.expected_source
                            && current.target == adaptation.target
                    }
                    None => false,
                };
                if !covered {
                    return Err(format!(
                        "excluded CSV adaptation/deletion at {path} has missing or conflicting inline ownership"
                    ));
                }
            }
            ExcludedCsvTranslationOccurrence::Adapter(pair) => {
                let covered = inline
                    .adapter_ids
                    .get(&pair.namespace)
                    .and_then(|replacements| replacements.get(&pair.source))
                    == Some(&pair.target);
                if !covered {
                    return Err(format!(
                        "excluded CSV adapter-ID translation at {} has missing or conflicting inline ownership",
                        pair.path
                    ));
                }
            }
        }
    }
    Ok(())
}

#[derive(Eq, PartialEq)]
enum EffectiveTranslation {
    Text(String),
    Adaptation(brain_brew_core::TargetAdaptation),
}

fn effective_translation(
    translations: &TranslationDictionary,
    path: &str,
    source: &str,
) -> Option<EffectiveTranslation> {
    if let Some(adaptation) = translations.target_adaptations.get(path) {
        return Some(EffectiveTranslation::Adaptation(adaptation.clone()));
    }
    if let Some(target) = translations
        .contextual
        .iter()
        .filter(|(context, replacements)| {
            replacements.contains_key(source)
                && (context.as_str() == path
                    || path
                        .strip_prefix(context.as_str())
                        .is_some_and(|suffix| suffix.starts_with('.')))
        })
        .max_by_key(|(context, _)| context.len())
        .and_then(|(_, replacements)| replacements.get(source))
    {
        return Some(EffectiveTranslation::Text(target.clone()));
    }
    translations
        .direct
        .get(source)
        .map(|target| EffectiveTranslation::Text(target.clone()))
        .or_else(|| {
            translations
                .no_change
                .contains(source)
                .then(|| EffectiveTranslation::Text(source.to_owned()))
        })
}

fn merge_csv_translations(
    existing: &mut TranslationDictionary,
    imported: &TranslationDictionary,
    owned_text: &[CsvTranslationPair],
) -> Result<(), String> {
    let mut next = existing.clone();
    for pair in owned_text {
        if let Some(current) = effective_translation(&next, &pair.path, &pair.source)
            && current != EffectiveTranslation::Text(pair.target.clone())
        {
            return Err(format!(
                "CSV translation conflict at {} for source {:?}: inline and imported decisions differ",
                pair.path, pair.source
            ));
        }
    }
    for (path, adaptation) in &imported.target_adaptations {
        let current = effective_translation(&next, path, &adaptation.expected_source);
        if let Some(current) = current
            && current != EffectiveTranslation::Adaptation(adaptation.clone())
        {
            return Err(format!(
                "CSV translation conflict at {path}: inline and imported adaptation/deletion decisions differ"
            ));
        }
    }
    for (source, target) in &imported.direct {
        if next.no_change.contains(source) && target != source {
            return Err(format!(
                "CSV direct translation for {source:?} conflicts with inline no_change"
            ));
        }
        for replacements in next.contextual.values() {
            if let Some(current) = replacements.get(source)
                && current != target
            {
                return Err(format!(
                    "CSV direct translation for {source:?} conflicts with inline contextual translation"
                ));
            }
        }
        insert_identical_or_vacant(&mut next.direct, source, target, "direct translation")?;
    }
    for (context, replacements) in &imported.contextual {
        for (source, target) in replacements {
            let inline = next.contextual.entry(context.clone()).or_default();
            insert_identical_or_vacant(inline, source, target, "contextual translation")?;
        }
    }
    for source in &imported.no_change {
        if next
            .direct
            .get(source)
            .is_some_and(|target| target != source)
        {
            return Err(format!(
                "CSV no_change for {source:?} conflicts with inline direct translation"
            ));
        }
        for replacements in next.contextual.values() {
            if replacements
                .get(source)
                .is_some_and(|target| target != source)
            {
                return Err(format!(
                    "CSV no_change for {source:?} conflicts with inline contextual translation"
                ));
            }
        }
        next.no_change.insert(source.clone());
    }
    for (path, adaptation) in &imported.target_adaptations {
        insert_identical_or_vacant(
            &mut next.target_adaptations,
            path,
            adaptation,
            "target adaptation",
        )?;
    }
    for (namespace, replacements) in &imported.adapter_ids {
        let inline = next.adapter_ids.entry(namespace.clone()).or_default();
        for (source, target) in replacements {
            insert_identical_or_vacant(inline, source, target, "adapter-ID translation")?;
        }
    }
    next.validate_mutation_invariants()
        .map_err(|error| error.to_string())?;
    *existing = next;
    Ok(())
}

fn insert_identical_or_vacant<K: Ord + Clone, V: Eq + Clone>(
    map: &mut BTreeMap<K, V>,
    key: &K,
    value: &V,
    kind: &str,
) -> Result<(), String> {
    if let Some(current) = map.get(key) {
        if current != value {
            return Err(format!("conflicting {kind} entries"));
        }
    } else {
        map.insert(key.clone(), value.clone());
    }
    Ok(())
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
