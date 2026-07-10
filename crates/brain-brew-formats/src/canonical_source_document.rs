//! Include-preserving, validated Canonical Deck source editing.
//!
//! A document parses through the strict Canonical YAML codec, keeps YAML details
//! private, and offers closed typed edits. Emission canonicalizes the complete
//! root document. Unedited scalar include files are not emitted; a targeted
//! scalar include edit emits only that file. An edited structural media include
//! is canonicalized as a complete standalone media map.

use std::collections::BTreeMap;

use brain_brew_core::{CanonicalDeck, StableId};

use crate::canonical_yaml;
use crate::source_document::{
    EditLocation, ImageConversionReport, IncludeRequest, IncludeState, SourceDocumentEmission,
    SourceDocumentError, SourceFile, SourceProvenance, convert_text_to_images, prepare_source,
};

/// Closed set of scalar Canonical Deck source locations supported by mutators.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalScalarTarget {
    DeckName,
    DeckDescription,
    DeckVariable(String),
    NoteTypeName {
        note_type_id: StableId,
    },
    NoteTypeVariable {
        note_type_id: StableId,
        key: String,
    },
    NoteTypeStyling {
        note_type_id: StableId,
    },
    FieldName {
        note_type_id: StableId,
        field_id: StableId,
    },
    CardTemplateName {
        note_type_id: StableId,
        template_id: StableId,
    },
    CardTemplateQuestion {
        note_type_id: StableId,
        template_id: StableId,
    },
    CardTemplateAnswer {
        note_type_id: StableId,
        template_id: StableId,
    },
    CardTemplateVariable {
        note_type_id: StableId,
        template_id: StableId,
        key: String,
    },
    NoteVariable {
        note_id: StableId,
        key: String,
    },
    NoteField {
        note_id: StableId,
        field_id: StableId,
    },
}

impl CanonicalScalarTarget {
    fn schema_path(&self) -> String {
        match self {
            Self::DeckName => "deck.name".to_owned(),
            Self::DeckDescription => "deck.description".to_owned(),
            Self::DeckVariable(key) => format!("deck.variables.{key}"),
            Self::NoteTypeName { note_type_id } => format!("note_types.{note_type_id}.name"),
            Self::NoteTypeVariable { note_type_id, key } => {
                format!("note_types.{note_type_id}.variables.{key}")
            }
            Self::NoteTypeStyling { note_type_id } => {
                format!("note_types.{note_type_id}.styling")
            }
            Self::FieldName {
                note_type_id,
                field_id,
            } => format!("note_types.{note_type_id}.fields.{field_id}.name"),
            Self::CardTemplateName {
                note_type_id,
                template_id,
            } => format!("note_types.{note_type_id}.card_templates.{template_id}.name"),
            Self::CardTemplateQuestion {
                note_type_id,
                template_id,
            } => format!("note_types.{note_type_id}.card_templates.{template_id}.question_format"),
            Self::CardTemplateAnswer {
                note_type_id,
                template_id,
            } => format!("note_types.{note_type_id}.card_templates.{template_id}.answer_format"),
            Self::CardTemplateVariable {
                note_type_id,
                template_id,
                key,
            } => format!("note_types.{note_type_id}.card_templates.{template_id}.variables.{key}"),
            Self::NoteVariable { note_id, key } => format!("notes.{note_id}.variables.{key}"),
            Self::NoteField { note_id, field_id } => {
                format!("notes.{note_id}.fields.{field_id}")
            }
        }
    }
}

/// Deep source module for one Canonical Deck file and its loaded includes.
#[derive(Clone)]
pub struct CanonicalSourceDocument {
    provenance: SourceProvenance,
    deck: CanonicalDeck,
    resolved_deck: CanonicalDeck,
    includes: IncludeState,
}

impl std::fmt::Debug for CanonicalSourceDocument {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CanonicalSourceDocument")
            .field("provenance", &self.provenance)
            .field("deck", &self.deck)
            .finish_non_exhaustive()
    }
}

impl CanonicalSourceDocument {
    /// Parse source that contains no includes.
    pub fn parse(source: SourceFile) -> Result<Self, SourceDocumentError> {
        Self::parse_with_includes(source, |request| {
            Err(format!(
                "no include loader was provided for {:?}",
                request.target()
            ))
        })
    }

    /// Parse source with caller-owned include loading and provenance.
    ///
    /// The loader owns path authorization and bytes retrieval. This method owns
    /// strict duplicate/scalar/union/schema validation and never performs I/O.
    pub fn parse_with_includes(
        source: SourceFile,
        mut loader: impl FnMut(&IncludeRequest) -> Result<SourceFile, String>,
    ) -> Result<Self, SourceDocumentError> {
        let prepared = prepare_source(source, true, &mut loader)?;
        let mut deck =
            canonical_yaml::from_str(&prepared.yaml_without_directives).map_err(|error| {
                SourceDocumentError::source(prepared.root.provenance(), error.to_string())
            })?;
        if let Some(media) = prepared.includes.media() {
            deck.media = media.clone();
        }
        let mut resolved_deck =
            canonical_yaml::from_str(&prepared.materialized_yaml).map_err(|error| {
                SourceDocumentError::source(prepared.root.provenance(), error.to_string())
            })?;
        if let Some(media) = prepared.includes.media() {
            resolved_deck.media = media.clone();
        }
        canonical_yaml::to_string(&deck).map_err(|error| {
            SourceDocumentError::source(prepared.root.provenance(), error.to_string())
        })?;
        Ok(Self {
            provenance: prepared.root.provenance().clone(),
            deck,
            resolved_deck,
            includes: prepared.includes,
        })
    }

    /// Construct a source document from an already validated domain deck.
    pub fn from_deck(
        provenance: SourceProvenance,
        deck: CanonicalDeck,
    ) -> Result<Self, SourceDocumentError> {
        canonical_yaml::to_string(&deck)
            .map_err(|error| SourceDocumentError::source(&provenance, error.to_string()))?;
        Ok(Self {
            provenance,
            resolved_deck: deck.clone(),
            deck,
            includes: IncludeState::default(),
        })
    }

    pub fn provenance(&self) -> &SourceProvenance {
        &self.provenance
    }

    /// Read-only validated domain view. Mutation remains behind typed methods.
    pub fn deck(&self) -> &CanonicalDeck {
        &self.deck
    }

    /// Fully materialized read view with scalar include contents substituted.
    pub fn resolved_deck(&self) -> &CanonicalDeck {
        &self.resolved_deck
    }

    /// Compare-and-set one scalar field or metadata value.
    pub fn set_scalar(
        &mut self,
        target: CanonicalScalarTarget,
        expected: &str,
        replacement: &str,
    ) -> Result<EditLocation, SourceDocumentError> {
        let path = target.schema_path();
        let mut next = self.clone();
        if let Some(location) =
            next.includes
                .edit_scalar(&path, expected, replacement, &next.provenance)?
        {
            next.validate()?;
            *self = next;
            return Ok(location);
        }
        let value = scalar_mut(&mut next.deck, &target).ok_or_else(|| {
            SourceDocumentError::at(
                &next.provenance,
                &path,
                "typed scalar target is not present in this Canonical Deck",
            )
        })?;
        if value != expected {
            return Err(SourceDocumentError::at(
                &next.provenance,
                &path,
                format!("expected {expected:?}, found {value:?}"),
            ));
        }
        *value = replacement.to_owned();
        next.validate()?;
        *self = next;
        Ok(EditLocation::Root)
    }

    /// Compare-and-set one declared media hash, whether inline or included.
    pub fn set_media_hash(
        &mut self,
        media_id: &StableId,
        expected_path: &str,
        sha256: &str,
    ) -> Result<EditLocation, SourceDocumentError> {
        let path = format!("media.{media_id}.sha256");
        let mut next = self.clone();
        if next.includes.media().is_some() {
            let (media, dirty) = next.includes.media_mut().expect("media include exists");
            let reference = media.get_mut(media_id).ok_or_else(|| {
                SourceDocumentError::at(
                    &next.provenance,
                    &path,
                    format!("media ID {media_id} is not declared in the included media map"),
                )
            })?;
            ensure_media_path(
                &next.provenance,
                &path,
                reference.path.as_str(),
                expected_path,
            )?;
            reference.sha256 = sha256.to_owned();
            *dirty = true;
            next.deck.media = media.clone();
            let location = next
                .includes
                .media_source()
                .map(EditLocation::Included)
                .expect("media source exists");
            next.validate()?;
            *self = next;
            return Ok(location);
        }
        let reference = next.deck.media.get_mut(media_id).ok_or_else(|| {
            SourceDocumentError::at(
                &next.provenance,
                &path,
                format!("media ID {media_id} is not declared"),
            )
        })?;
        ensure_media_path(
            &next.provenance,
            &path,
            reference.path.as_str(),
            expected_path,
        )?;
        reference.sha256 = sha256.to_owned();
        next.validate()?;
        *self = next;
        Ok(EditLocation::Root)
    }

    /// Convert strict whole-field image HTML using a caller-built path lookup.
    pub fn convert_strict_image_fields(
        &mut self,
        lookup: &BTreeMap<String, Option<StableId>>,
    ) -> Result<ImageConversionReport, SourceDocumentError> {
        let mut next = self.clone();
        let mut report = ImageConversionReport::default();
        for note in next.deck.notes.values_mut() {
            let field_ids = note.fields.keys().cloned().collect::<Vec<_>>();
            for field_id in field_ids {
                if note.field_messages.contains_key(&field_id)
                    || note.field_images.contains_key(&field_id)
                {
                    continue;
                }
                let text = &note.fields[&field_id];
                if let Some(images) = convert_text_to_images(text, lookup, &mut report) {
                    note.fields.insert(field_id.clone(), String::new());
                    note.field_images.insert(field_id, images);
                }
            }
        }
        next.validate()?;
        *self = next;
        Ok(report)
    }

    /// Emit deterministic canonical root YAML and changed include outputs.
    pub fn emit(&self) -> Result<SourceDocumentEmission, SourceDocumentError> {
        self.validate()?;
        let mut deck = self.deck.clone();
        if self.includes.media().is_some() {
            deck.media.clear();
        }
        let canonical = canonical_yaml::to_string(&deck)
            .map_err(|error| SourceDocumentError::source(&self.provenance, error.to_string()))?;
        let canonical = self.includes.restore_directives(canonical)?;
        // The codec generated every schema value; this final strict pass protects
        // directive restoration from introducing a duplicate mapping key.
        crate::strict_yaml::reject_duplicate_keys(&canonical)
            .map_err(|error| SourceDocumentError::source(&self.provenance, error.to_string()))?;
        let root = SourceFile::new(self.provenance.clone(), canonical);
        Ok(SourceDocumentEmission::new(
            root,
            self.includes.changed_sources()?,
        ))
    }

    fn validate(&self) -> Result<(), SourceDocumentError> {
        canonical_yaml::to_string(&self.deck)
            .map(|_| ())
            .map_err(|error| SourceDocumentError::source(&self.provenance, error.to_string()))
    }
}

fn scalar_mut<'a>(
    deck: &'a mut CanonicalDeck,
    target: &CanonicalScalarTarget,
) -> Option<&'a mut String> {
    match target {
        CanonicalScalarTarget::DeckName => Some(&mut deck.name),
        CanonicalScalarTarget::DeckDescription => Some(&mut deck.description),
        CanonicalScalarTarget::DeckVariable(key) => deck.variables.get_mut(key),
        CanonicalScalarTarget::NoteTypeName { note_type_id } => {
            Some(&mut deck.note_types.get_mut(note_type_id)?.name)
        }
        CanonicalScalarTarget::NoteTypeVariable { note_type_id, key } => deck
            .note_types
            .get_mut(note_type_id)?
            .variables
            .get_mut(key),
        CanonicalScalarTarget::NoteTypeStyling { note_type_id } => {
            Some(&mut deck.note_types.get_mut(note_type_id)?.styling)
        }
        CanonicalScalarTarget::FieldName {
            note_type_id,
            field_id,
        } => deck
            .note_types
            .get_mut(note_type_id)?
            .fields
            .iter_mut()
            .find(|field| &field.id == field_id)
            .map(|field| &mut field.name),
        CanonicalScalarTarget::CardTemplateName {
            note_type_id,
            template_id,
        } => template_mut(deck, note_type_id, template_id).map(|template| &mut template.name),
        CanonicalScalarTarget::CardTemplateQuestion {
            note_type_id,
            template_id,
        } => template_mut(deck, note_type_id, template_id)
            .map(|template| &mut template.question_format),
        CanonicalScalarTarget::CardTemplateAnswer {
            note_type_id,
            template_id,
        } => template_mut(deck, note_type_id, template_id)
            .map(|template| &mut template.answer_format),
        CanonicalScalarTarget::CardTemplateVariable {
            note_type_id,
            template_id,
            key,
        } => template_mut(deck, note_type_id, template_id)?
            .variables
            .get_mut(key),
        CanonicalScalarTarget::NoteVariable { note_id, key } => {
            deck.notes.get_mut(note_id)?.variables.get_mut(key)
        }
        CanonicalScalarTarget::NoteField { note_id, field_id } => {
            let note = deck.notes.get_mut(note_id)?;
            if note.field_messages.contains_key(field_id)
                || note.field_images.contains_key(field_id)
            {
                return None;
            }
            note.fields.get_mut(field_id)
        }
    }
}

fn template_mut<'a>(
    deck: &'a mut CanonicalDeck,
    note_type_id: &StableId,
    template_id: &StableId,
) -> Option<&'a mut brain_brew_core::CardTemplate> {
    deck.note_types
        .get_mut(note_type_id)?
        .card_templates
        .iter_mut()
        .find(|template| &template.id == template_id)
}

fn ensure_media_path(
    provenance: &SourceProvenance,
    schema_path: &str,
    actual: &str,
    expected: &str,
) -> Result<(), SourceDocumentError> {
    if actual == expected {
        Ok(())
    } else {
        Err(SourceDocumentError::at(
            provenance,
            schema_path,
            format!("expected media path {expected:?}, found {actual:?}"),
        ))
    }
}
