//! Include-preserving, validated Canonical Deck source editing.
//!
//! A document parses through the strict Canonical YAML codec, keeps YAML details
//! private, and offers closed typed edits. Emission canonicalizes the complete
//! root document. Unedited scalar include files are not emitted; a targeted
//! scalar include edit emits only that file. An edited structural media include
//! is canonicalized as a complete standalone media map.

use std::collections::BTreeMap;

use brain_brew_core::{CanonicalDeck, FieldValue, StableId};

use crate::canonical_yaml;
use crate::csv_note_source::{
    CsvNoteSourceDeclaration, CsvNoteSourceDescriptor, CsvNoteSourceMaterializer, CsvSourceFile,
    CsvSourceRequest,
};
use crate::source_document::{
    EditLocation, ImageConversionReport, IncludeRequest, IncludeState, IncludedSource,
    SourceDocumentEmission, SourceDocumentError, SourceFile, SourceProvenance,
    convert_text_to_images, prepare_source,
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
    csv_notes: Option<CsvNoteSourceDeclaration>,
    original_sources: BTreeMap<SourceProvenance, SourceFile>,
}

impl std::fmt::Debug for CanonicalSourceDocument {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CanonicalSourceDocument")
            .field("provenance", &self.provenance)
            .field("deck", &self.deck)
            .field("csv_notes", &self.csv_notes)
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
        Self::parse_with_loaders(source, &mut loader, &mut |request| {
            Err(format!(
                "no CSV source loader was provided for {:?}",
                request.target()
            ))
        })
    }

    /// Parse source with caller-owned include and CSV Authoring Source loading.
    ///
    /// Both loaders authorize and inject bytes. This formats crate never opens a
    /// path. The CSV loader receives explicit descriptor/table request kinds.
    pub fn parse_with_csv_sources(
        source: SourceFile,
        mut include_loader: impl FnMut(&IncludeRequest) -> Result<SourceFile, String>,
        mut csv_loader: impl FnMut(&CsvSourceRequest) -> Result<CsvSourceFile, String>,
    ) -> Result<Self, SourceDocumentError> {
        Self::parse_with_loaders(source, &mut include_loader, &mut csv_loader)
    }

    fn parse_with_loaders(
        source: SourceFile,
        include_loader: &mut impl FnMut(&IncludeRequest) -> Result<SourceFile, String>,
        csv_loader: &mut impl FnMut(&CsvSourceRequest) -> Result<CsvSourceFile, String>,
    ) -> Result<Self, SourceDocumentError> {
        let prepared = prepare_source(source, true, include_loader)?;
        let root_yaml = yaml_with_included_structures_for_validation(
            &prepared.yaml_without_directives,
            prepared.includes.note_types(),
            prepared.includes.media(),
        )?;
        let (root_yaml, csv_notes) =
            strip_csv_note_declaration(&root_yaml, prepared.root.provenance())?;
        let mut deck = canonical_yaml::from_str(&root_yaml).map_err(|error| {
            SourceDocumentError::source(prepared.root.provenance(), error.to_string())
        })?;
        if let Some(media) = prepared.includes.media() {
            deck.media = media.clone();
        }
        let materialized_yaml = yaml_with_included_structures_for_validation(
            &prepared.materialized_yaml,
            prepared.includes.resolved_note_types(),
            prepared.includes.media(),
        )?;
        let (materialized_yaml, materialized_csv_notes) =
            strip_csv_note_declaration(&materialized_yaml, prepared.root.provenance())?;
        if materialized_csv_notes != csv_notes {
            return Err(SourceDocumentError::at(
                prepared.root.provenance(),
                "notes",
                "scalar include materialization changed the notes !csv declaration",
            ));
        }
        let mut resolved_deck = canonical_yaml::from_str(&materialized_yaml).map_err(|error| {
            SourceDocumentError::source(prepared.root.provenance(), error.to_string())
        })?;
        if let Some(media) = prepared.includes.media() {
            resolved_deck.media = media.clone();
        }
        if let Some(declaration) = &csv_notes {
            let descriptor_request = CsvSourceRequest::descriptor(
                prepared.root.provenance().clone(),
                declaration.descriptor().to_owned(),
            );
            let descriptor_bytes = csv_loader(&descriptor_request).map_err(|message| {
                SourceDocumentError::at(
                    prepared.root.provenance(),
                    "notes.descriptor",
                    format!(
                        "could not load CSV note descriptor {:?}: {message}",
                        declaration.descriptor()
                    ),
                )
            })?;
            let descriptor_text =
                std::str::from_utf8(descriptor_bytes.bytes()).map_err(|error| {
                    SourceDocumentError::at(
                        descriptor_bytes.provenance(),
                        "notes.descriptor",
                        format!("descriptor is not valid UTF-8: {error}"),
                    )
                })?;
            let descriptor = CsvNoteSourceDescriptor::parse(SourceFile::new(
                descriptor_bytes.provenance().clone(),
                descriptor_text,
            ))
            .map_err(|error| {
                SourceDocumentError::at(prepared.root.provenance(), "notes", error.to_string())
            })?;
            let descriptor_provenance = descriptor.provenance().clone();
            let materializer = CsvNoteSourceMaterializer::new(descriptor)
                .with_parameters(declaration.parameters())
                .map_err(|error| {
                    SourceDocumentError::at(prepared.root.provenance(), "notes", error.to_string())
                })?;
            let table_requests = materializer
                .table_paths()
                .map(|(alias, target)| {
                    CsvSourceRequest::table(
                        descriptor_provenance.clone(),
                        alias.to_owned(),
                        target.to_owned(),
                    )
                })
                .collect::<Vec<_>>();
            let mut tables = BTreeMap::new();
            for request in table_requests {
                let alias = match request.kind() {
                    crate::csv_note_source::CsvSourceRequestKind::Table { alias } => alias.clone(),
                    crate::csv_note_source::CsvSourceRequestKind::Descriptor => unreachable!(),
                };
                let table = csv_loader(&request).map_err(|message| {
                    SourceDocumentError::at(
                        prepared.root.provenance(),
                        "notes",
                        format!("could not load CSV table {:?}: {message}", request.target()),
                    )
                })?;
                tables.insert(alias, table);
            }
            resolved_deck.notes = materializer
                .materialize(&tables, &resolved_deck.note_types)
                .map_err(|error| {
                    SourceDocumentError::at(prepared.root.provenance(), "notes", error.to_string())
                })?;
            canonical_yaml::to_string(&resolved_deck).map_err(|error| {
                SourceDocumentError::source(prepared.root.provenance(), error.to_string())
            })?;
        }
        canonical_yaml::to_string(&deck).map_err(|error| {
            SourceDocumentError::source(prepared.root.provenance(), error.to_string())
        })?;
        let original_sources = prepared.original_sources()?;
        Ok(Self {
            provenance: prepared.root.provenance().clone(),
            deck,
            resolved_deck,
            includes: prepared.includes,
            csv_notes,
            original_sources,
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
            csv_notes: None,
            original_sources: BTreeMap::new(),
        })
    }

    pub fn provenance(&self) -> &SourceProvenance {
        &self.provenance
    }

    /// Source-preserved CSV note declaration, when notes are externally owned.
    pub fn csv_note_source(&self) -> Option<&CsvNoteSourceDeclaration> {
        self.csv_notes.as_ref()
    }

    /// Every scalar or structural media source loaded by this document.
    pub fn included_sources(&self) -> Vec<IncludedSource> {
        self.includes.source_provenance()
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
            let value = scalar_mut(&mut next.resolved_deck, &target).ok_or_else(|| {
                SourceDocumentError::at(
                    &next.provenance,
                    &path,
                    "typed scalar target is not present in the resolved Canonical Deck",
                )
            })?;
            *value = replacement.to_owned();
            if path.starts_with("note_types.") && next.includes.note_types().is_some() {
                next.includes
                    .replace_resolved_note_types(next.resolved_deck.note_types.clone());
            }
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
        if path.starts_with("note_types.") && next.includes.note_types().is_some() {
            let resolved_value = scalar_mut(&mut next.resolved_deck, &target).ok_or_else(|| {
                SourceDocumentError::at(
                    &next.provenance,
                    &path,
                    "typed scalar target is not present in the resolved Canonical Deck",
                )
            })?;
            *resolved_value = replacement.to_owned();
            next.includes.replace_note_types(
                next.deck.note_types.clone(),
                next.resolved_deck.note_types.clone(),
            );
            let location = next
                .includes
                .note_types_source()
                .map(EditLocation::Included)
                .expect("note-types include source exists");
            next.validate()?;
            *self = next;
            return Ok(location);
        }
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
                let Some(text) = note.fields[&field_id].as_scalar() else {
                    continue;
                };
                if let Some(images) = convert_text_to_images(text, lookup, &mut report) {
                    note.fields.insert(field_id, FieldValue::Images(images));
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
        let canonical = canonical_yaml::to_string(&self.deck)
            .map_err(|error| SourceDocumentError::source(&self.provenance, error.to_string()))?;
        let canonical = self.includes.restore_directives(canonical)?;
        let canonical = if let Some(declaration) = &self.csv_notes {
            declaration
                .restore(canonical)
                .map_err(|message| SourceDocumentError::at(&self.provenance, "notes", message))?
        } else {
            canonical
        };
        // The codec generated every schema value; this final strict pass protects
        // directive restoration from introducing a duplicate mapping key.
        crate::strict_yaml::reject_duplicate_keys(&canonical)
            .map_err(|error| SourceDocumentError::source(&self.provenance, error.to_string()))?;
        let root = SourceFile::new(self.provenance.clone(), canonical);
        Ok(SourceDocumentEmission::new(
            root,
            self.includes.changed_sources()?,
            self.original_sources.clone(),
        ))
    }

    fn validate(&self) -> Result<(), SourceDocumentError> {
        canonical_yaml::to_string(&self.deck)
            .and_then(|_| canonical_yaml::to_string(&self.resolved_deck))
            .map(|_| ())
            .map_err(|error| SourceDocumentError::source(&self.provenance, error.to_string()))
    }
}

fn strip_csv_note_declaration(
    yaml: &str,
    provenance: &SourceProvenance,
) -> Result<(String, Option<CsvNoteSourceDeclaration>), SourceDocumentError> {
    let mut value = serde_yaml::from_str::<serde_yaml::Value>(yaml)
        .map_err(|error| SourceDocumentError::source(provenance, error.to_string()))?;
    let declaration = CsvNoteSourceDeclaration::take_from_root(&mut value, provenance)
        .map_err(|error| SourceDocumentError::at(provenance, "notes", error.to_string()))?;
    let Some(declaration) = declaration else {
        return Ok((yaml.to_owned(), None));
    };
    let yaml = serde_yaml::to_string(&value)
        .map_err(|error| SourceDocumentError::source(provenance, error.to_string()))?;
    Ok((yaml, Some(declaration)))
}

fn yaml_with_included_structures_for_validation(
    yaml: &str,
    note_types: Option<&BTreeMap<StableId, brain_brew_core::NoteType>>,
    media: Option<&BTreeMap<StableId, brain_brew_core::MediaReference>>,
) -> Result<String, SourceDocumentError> {
    let mut yaml = yaml.to_owned();
    if let Some(note_types) = note_types
        && !note_types.is_empty()
    {
        let body = crate::note_type_map::to_string(note_types)
            .map_err(|error| {
                SourceDocumentError::source(
                    &SourceProvenance::new("canonical deck"),
                    format!("could not materialize included note types: {error}"),
                )
            })?
            .lines()
            .map(|line| format!("  {line}\n"))
            .collect::<String>();
        yaml = replace_structural_placeholder(&yaml, "note_types", &body)?;
    }
    if let Some(media) = media
        && !media.is_empty()
    {
        let body = crate::media_map::to_string(media)
            .lines()
            .map(|line| format!("  {line}\n"))
            .collect::<String>();
        yaml = replace_structural_placeholder(&yaml, "media", &body)?;
    }
    Ok(yaml)
}

fn replace_structural_placeholder(
    yaml: &str,
    key: &str,
    body: &str,
) -> Result<String, SourceDocumentError> {
    let invalid_placeholder = || {
        SourceDocumentError::source(
            &SourceProvenance::new("canonical deck"),
            format!("expected one empty {key} placeholder while loading structural include"),
        )
    };
    let start = crate::strict_yaml::top_level_mapping_key_offset(yaml, key)
        .ok_or_else(invalid_placeholder)?;
    let line_end = yaml[start..]
        .find('\n')
        .map_or(yaml.len(), |offset| start + offset + 1);
    let line = yaml[start..line_end]
        .strip_suffix('\n')
        .unwrap_or(&yaml[start..line_end]);
    let line = line.strip_suffix('\r').unwrap_or(line);
    if line != format!("{key}: {{}}") {
        return Err(invalid_placeholder());
    }

    let mut materialized = yaml.to_owned();
    materialized.replace_range(start..line_end, &format!("{key}:\n{body}"));
    Ok(materialized)
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
        CanonicalScalarTarget::NoteField { note_id, field_id } => deck
            .notes
            .get_mut(note_id)?
            .fields
            .get_mut(field_id)?
            .as_scalar_mut(),
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

#[cfg(test)]
mod csv_note_declaration_tests {
    use super::*;

    #[test]
    fn ordinary_yaml_is_not_reserialized_when_no_csv_declaration_exists() {
        let yaml = "deck: {id: deck.test, name: Test}\nnotes: {}\n";
        let (stripped, declaration) =
            strip_csv_note_declaration(yaml, &SourceProvenance::new("deck.yaml")).unwrap();

        assert!(declaration.is_none());
        assert_eq!(stripped, yaml);
    }
}
