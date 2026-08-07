//! Pure source-document plumbing shared by canonical deck and overlay documents.
//!
//! This module owns source provenance, injected include loading, canonical emission
//! results, and include edit locality. It deliberately performs no filesystem I/O
//! and exposes no `serde_yaml` representation.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use brain_brew_core::{MediaReference, NoteType, StableId};
use serde_yaml::{Mapping, Value};

use crate::{media_map, note_type_map, strict_yaml, yaml_scalar};

/// Logical identity and diagnostic root for one source file.
///
/// Both fields are opaque labels. Callers may use paths, package IDs, in-memory
/// handles, or URI-like names without giving this crate filesystem authority.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SourceProvenance {
    source_name: String,
    source_root: Option<String>,
}

impl SourceProvenance {
    pub fn new(source_name: impl Into<String>) -> Self {
        Self {
            source_name: source_name.into(),
            source_root: None,
        }
    }

    pub fn with_source_root(mut self, source_root: impl Into<String>) -> Self {
        self.source_root = Some(source_root.into());
        self
    }

    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    pub fn source_root(&self) -> Option<&str> {
        self.source_root.as_deref()
    }
}

impl fmt::Display for SourceProvenance {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(root) = &self.source_root {
            write!(formatter, "{} ({root})", self.source_name)
        } else {
            formatter.write_str(&self.source_name)
        }
    }
}

/// Owned source bytes and their provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceFile {
    provenance: SourceProvenance,
    text: String,
}

impl SourceFile {
    pub fn new(provenance: SourceProvenance, text: impl Into<String>) -> Self {
        Self {
            provenance,
            text: text.into(),
        }
    }

    pub fn provenance(&self) -> &SourceProvenance {
        &self.provenance
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

/// Request passed to a caller-owned include loader.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IncludeRequest {
    referring_source: SourceProvenance,
    schema_path: String,
    target: String,
}

impl IncludeRequest {
    pub fn referring_source(&self) -> &SourceProvenance {
        &self.referring_source
    }

    pub fn schema_path(&self) -> &str {
        &self.schema_path
    }

    pub fn target(&self) -> &str {
        &self.target
    }
}

/// Kind and schema location of a source loaded through `!include`.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum IncludedSourceKind {
    Scalar { schema_path: String },
    MediaDeclarations,
    NoteTypeDeclarations,
}

/// Provenance retained for one source loaded while parsing a document.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct IncludedSource {
    kind: IncludedSourceKind,
    provenance: SourceProvenance,
}

impl IncludedSource {
    pub fn kind(&self) -> &IncludedSourceKind {
        &self.kind
    }

    pub fn provenance(&self) -> &SourceProvenance {
        &self.provenance
    }
}

/// Whether a typed edit changes the root YAML or the content of an include.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EditLocation {
    Root,
    Included(SourceProvenance),
}

/// Canonical root source plus only those included sources changed by edits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceDocumentEmission {
    root: SourceFile,
    included: Vec<SourceFile>,
    original_sources: BTreeMap<SourceProvenance, SourceFile>,
}

impl SourceDocumentEmission {
    pub(crate) fn new(
        root: SourceFile,
        included: Vec<SourceFile>,
        original_sources: BTreeMap<SourceProvenance, SourceFile>,
    ) -> Self {
        Self {
            root,
            included,
            original_sources,
        }
    }

    pub fn root(&self) -> &SourceFile {
        &self.root
    }

    pub fn included(&self) -> &[SourceFile] {
        &self.included
    }

    pub fn included_source(&self, source_name: &str) -> Option<&SourceFile> {
        self.included
            .iter()
            .find(|source| source.provenance.source_name == source_name)
    }

    /// Exact source bytes used to parse and compute an emitted replacement.
    ///
    /// Generated documents have no original snapshot and therefore represent
    /// an expected-absent output.
    pub fn original_source(&self, provenance: &SourceProvenance) -> Option<&SourceFile> {
        self.original_sources.get(provenance)
    }
}

/// Typed source/schema diagnostic returned by every document operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceDocumentError {
    provenance: SourceProvenance,
    schema_path: Option<String>,
    message: String,
}

impl SourceDocumentError {
    pub(crate) fn new(
        provenance: SourceProvenance,
        schema_path: impl Into<Option<String>>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            provenance,
            schema_path: schema_path.into(),
            message: message.into(),
        }
    }

    pub(crate) fn at(
        provenance: &SourceProvenance,
        schema_path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(provenance.clone(), Some(schema_path.into()), message.into())
    }

    pub(crate) fn source(provenance: &SourceProvenance, message: impl Into<String>) -> Self {
        Self::new(provenance.clone(), None, message.into())
    }

    pub fn provenance(&self) -> &SourceProvenance {
        &self.provenance
    }

    pub fn schema_path(&self) -> Option<&str> {
        self.schema_path.as_deref()
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for SourceDocumentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.schema_path {
            Some(path) if !path.is_empty() => {
                write!(formatter, "{}:{path}: {}", self.provenance, self.message)
            }
            _ => write!(formatter, "{}: {}", self.provenance, self.message),
        }
    }
}

impl std::error::Error for SourceDocumentError {}

/// Result of strict whole-field HTML-to-`!image` conversion.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ImageConversionReport {
    pub converted: usize,
    pub skipped_non_strict: usize,
    pub skipped_no_match: usize,
    pub skipped_ambiguous_path: usize,
}

#[derive(Clone)]
pub(crate) struct PreparedSource {
    pub root: SourceFile,
    pub yaml_without_directives: String,
    pub materialized_yaml: String,
    pub includes: IncludeState,
}

#[derive(Clone, Default)]
pub(crate) struct IncludeState {
    scalar: BTreeMap<String, ScalarInclude>,
    media: Option<MediaInclude>,
    note_types: Option<NoteTypesInclude>,
    media_asset_sources: BTreeMap<StableId, String>,
    loaded_sources: BTreeSet<IncludedSource>,
}

#[derive(Clone)]
struct ScalarInclude {
    sentinel: String,
    directive: String,
    source: SourceFile,
    value: String,
    fragment: Option<crate::source_includes::IncludeFragment>,
    dirty: bool,
}

struct LoadedScalarInclude {
    source: SourceFile,
    value: String,
    fragment: Option<crate::source_includes::IncludeFragment>,
}

#[derive(Clone)]
struct MediaInclude {
    directive: String,
    source: SourceFile,
    media: BTreeMap<StableId, MediaReference>,
    dirty: bool,
}

#[derive(Clone)]
struct NoteTypesInclude {
    directive: String,
    source: SourceFile,
    note_types: BTreeMap<StableId, NoteType>,
    resolved_note_types: BTreeMap<StableId, NoteType>,
    dirty: bool,
}

impl PreparedSource {
    pub(crate) fn original_sources(
        &self,
    ) -> Result<BTreeMap<SourceProvenance, SourceFile>, SourceDocumentError> {
        let mut originals = BTreeMap::<SourceProvenance, String>::new();
        insert_changed_source(&mut originals, &self.root)?;
        for include in self.includes.scalar.values() {
            insert_changed_source(&mut originals, &include.source)?;
        }
        if let Some(include) = &self.includes.media {
            insert_changed_source(&mut originals, &include.source)?;
        }
        if let Some(include) = &self.includes.note_types {
            insert_changed_source(&mut originals, &include.source)?;
        }
        Ok(originals
            .into_iter()
            .map(|(provenance, text)| (provenance.clone(), SourceFile::new(provenance, text)))
            .collect())
    }
}

impl IncludeState {
    pub(crate) fn source_provenance(&self) -> Vec<IncludedSource> {
        self.loaded_sources.iter().cloned().collect()
    }

    pub(crate) fn media(&self) -> Option<&BTreeMap<StableId, MediaReference>> {
        self.media.as_ref().map(|include| &include.media)
    }

    pub(crate) fn media_asset_sources(&self) -> &BTreeMap<StableId, String> {
        &self.media_asset_sources
    }

    pub(crate) fn note_types(&self) -> Option<&BTreeMap<StableId, NoteType>> {
        self.note_types.as_ref().map(|include| &include.note_types)
    }

    pub(crate) fn resolved_note_types(&self) -> Option<&BTreeMap<StableId, NoteType>> {
        self.note_types
            .as_ref()
            .map(|include| &include.resolved_note_types)
    }

    pub(crate) fn replace_note_types(
        &mut self,
        note_types: BTreeMap<StableId, NoteType>,
        resolved_note_types: BTreeMap<StableId, NoteType>,
    ) -> bool {
        let Some(include) = self.note_types.as_mut() else {
            return false;
        };
        include.note_types = note_types;
        include.resolved_note_types = resolved_note_types;
        include.dirty = true;
        true
    }

    pub(crate) fn replace_resolved_note_types(
        &mut self,
        resolved_note_types: BTreeMap<StableId, NoteType>,
    ) -> bool {
        let Some(include) = self.note_types.as_mut() else {
            return false;
        };
        include.resolved_note_types = resolved_note_types;
        true
    }

    pub(crate) fn note_types_source(&self) -> Option<SourceProvenance> {
        self.note_types
            .as_ref()
            .map(|include| include.source.provenance.clone())
    }

    pub(crate) fn media_mut(
        &mut self,
    ) -> Option<(&mut BTreeMap<StableId, MediaReference>, &mut bool)> {
        self.media
            .as_mut()
            .map(|include| (&mut include.media, &mut include.dirty))
    }

    pub(crate) fn media_source(&self) -> Option<SourceProvenance> {
        self.media
            .as_ref()
            .map(|include| include.source.provenance.clone())
    }

    pub(crate) fn scalar_sentinel(&self, path: &str) -> Option<&str> {
        self.scalar
            .get(path)
            .map(|include| include.sentinel.as_str())
    }

    pub(crate) fn remove_scalar(&mut self, path: &str) -> bool {
        self.scalar.remove(path).is_some()
    }

    pub(crate) fn move_scalar(&mut self, from: &str, to: String) -> bool {
        let Some(include) = self.scalar.remove(from) else {
            return false;
        };
        self.scalar.insert(to, include);
        true
    }

    pub(crate) fn edit_scalar(
        &mut self,
        path: &str,
        expected: &str,
        replacement: &str,
        root: &SourceProvenance,
    ) -> Result<Option<EditLocation>, SourceDocumentError> {
        let Some(include) = self.scalar.get(path) else {
            return Ok(None);
        };
        if include.value != expected {
            return Err(SourceDocumentError::at(
                root,
                path,
                format!(
                    "expected included scalar value {expected:?}, found {:?} in {}",
                    include.value, include.source.provenance
                ),
            ));
        }
        let provenance = include.source.provenance.clone();
        let updated_source = match include.fragment {
            Some(fragment) => crate::source_includes::replace_include_fragment(
                &include.source.text,
                fragment,
                replacement,
            )
            .map_err(|message| SourceDocumentError::at(root, path, message))?,
            None => replacement.to_owned(),
        };
        let updated_values = self
            .scalar
            .iter()
            .filter(|(_, include)| include.source.provenance == provenance)
            .map(|(schema_path, include)| {
                let value = include
                    .fragment
                    .map(|fragment| {
                        crate::source_includes::select_include_fragment(&updated_source, fragment)
                    })
                    .transpose()
                    .map_err(|message| SourceDocumentError::at(root, schema_path, message))?
                    .unwrap_or_else(|| updated_source.clone());
                Ok((schema_path.clone(), value))
            })
            .collect::<Result<BTreeMap<_, _>, SourceDocumentError>>()?;
        for (schema_path, include) in &mut self.scalar {
            if include.source.provenance == provenance {
                include.source.text = updated_source.clone();
                include.value = updated_values[schema_path].clone();
                include.dirty = true;
            }
        }
        Ok(Some(EditLocation::Included(provenance)))
    }

    pub(crate) fn restore_directives(
        &self,
        mut canonical: String,
    ) -> Result<String, SourceDocumentError> {
        canonical = self.restore_media_asset_sources(canonical)?;
        for (path, include) in &self.scalar {
            let count = canonical.matches(&include.sentinel).count();
            if count != 1 {
                return Err(SourceDocumentError::at(
                    &include.source.provenance,
                    path,
                    format!(
                        "expected one scalar include sentinel during canonical emission, found {count}"
                    ),
                ));
            }
            canonical = canonical.replace(&include.sentinel, &include.directive);
        }
        if let Some(include) = &self.note_types {
            let Some(start) = strict_yaml::top_level_mapping_key_offset(&canonical, "note_types")
            else {
                return Err(SourceDocumentError::at(
                    &include.source.provenance,
                    "note_types",
                    "canonical emission omitted the note_types section",
                ));
            };
            let Some(relative_end) =
                strict_yaml::top_level_mapping_key_offset(&canonical[start..], "notes")
            else {
                return Err(SourceDocumentError::at(
                    &include.source.provenance,
                    "note_types",
                    "canonical emission omitted notes after note_types",
                ));
            };
            let end = start + relative_end;
            canonical.replace_range(start..end, &format!("{}\n", include.directive));
        }
        if let Some(include) = &self.media {
            let Some(start) = strict_yaml::top_level_mapping_key_offset(&canonical, "media") else {
                return Err(SourceDocumentError::at(
                    &include.source.provenance,
                    "media",
                    "canonical emission omitted the media section",
                ));
            };
            let Some(relative_end) =
                strict_yaml::top_level_mapping_key_offset(&canonical[start..], "tombstones")
            else {
                return Err(SourceDocumentError::at(
                    &include.source.provenance,
                    "media",
                    "canonical emission omitted tombstones after media",
                ));
            };
            let end = start + relative_end;
            canonical.replace_range(start..end, &format!("{}\n", include.directive));
        }
        Ok(canonical)
    }

    fn restore_media_asset_sources(
        &self,
        mut canonical: String,
    ) -> Result<String, SourceDocumentError> {
        let Some(media_start) = strict_yaml::top_level_mapping_key_offset(&canonical, "media")
        else {
            if self.media_asset_sources.is_empty() {
                return Ok(canonical);
            }
            return Err(SourceDocumentError::source(
                &self
                    .scalar
                    .values()
                    .next()
                    .map(|include| include.source.provenance.clone())
                    .unwrap_or_else(|| SourceProvenance::new("<source>")),
                "canonical emission omitted media declarations with source paths",
            ));
        };
        for (id, source) in &self.media_asset_sources {
            let marker = format!("  {id}:\n");
            let relative_start = canonical[media_start..].find(&marker).ok_or_else(|| {
                SourceDocumentError::new(
                    SourceProvenance::new("<source>"),
                    Some(format!("media.{id}.source")),
                    "canonical emission omitted the source-backed media declaration",
                )
            })?;
            let declaration_start = media_start + relative_start;
            let body_start = declaration_start + marker.len();
            let mut declaration_end = canonical.len();
            let mut offset = 0;
            for line in canonical[body_start..].split_inclusive('\n') {
                if (!line.starts_with(' ') && !line.trim().is_empty())
                    || (line.starts_with("  ") && !line.starts_with("    "))
                {
                    declaration_end = body_start + offset;
                    break;
                }
                offset += line.len();
            }
            let hash_offset = canonical[declaration_start..declaration_end]
                .find("    sha256:")
                .ok_or_else(|| {
                    SourceDocumentError::new(
                        SourceProvenance::new("<source>"),
                        Some(format!("media.{id}.source")),
                        "source-backed media declaration has no emitted sha256",
                    )
                })?;
            canonical.insert_str(
                declaration_start + hash_offset,
                &format!("    source: {}\n", yaml_scalar::scalar(source)),
            );
        }
        Ok(canonical)
    }

    fn restore_note_type_scalar_directives(
        &self,
        mut canonical: String,
    ) -> Result<String, SourceDocumentError> {
        for (path, include) in self
            .scalar
            .iter()
            .filter(|(path, _)| path.starts_with("note_types."))
        {
            let count = canonical.matches(&include.sentinel).count();
            if count != 1 {
                return Err(SourceDocumentError::at(
                    &include.source.provenance,
                    path,
                    format!(
                        "expected one scalar include sentinel in the note-type map, found {count}"
                    ),
                ));
            }
            canonical = canonical.replace(&include.sentinel, &include.directive);
        }
        Ok(canonical)
    }

    pub(crate) fn changed_sources(&self) -> Result<Vec<SourceFile>, SourceDocumentError> {
        let mut changed = BTreeMap::<SourceProvenance, String>::new();
        for include in self.scalar.values().filter(|include| include.dirty) {
            insert_changed_source(&mut changed, &include.source)?;
        }
        if let Some(include) = &self.note_types
            && include.dirty
        {
            let text = note_type_map::to_string(&include.note_types).map_err(|error| {
                SourceDocumentError::at(
                    &include.source.provenance,
                    "note_types",
                    format!("could not emit included note-type map: {error}"),
                )
            })?;
            let text = self.restore_note_type_scalar_directives(text)?;
            let source = SourceFile::new(include.source.provenance.clone(), text);
            insert_changed_source(&mut changed, &source)?;
        }
        if let Some(include) = &self.media
            && include.dirty
        {
            let source = SourceFile::new(
                include.source.provenance.clone(),
                media_map::to_string(&include.media),
            );
            insert_changed_source(&mut changed, &source)?;
        }
        Ok(changed
            .into_iter()
            .map(|(provenance, text)| SourceFile::new(provenance, text))
            .collect())
    }
}

fn insert_changed_source(
    changed: &mut BTreeMap<SourceProvenance, String>,
    source: &SourceFile,
) -> Result<(), SourceDocumentError> {
    if let Some(existing) = changed.get(&source.provenance)
        && existing != &source.text
    {
        return Err(SourceDocumentError::source(
            &source.provenance,
            "multiple edits produced conflicting content for the same included source",
        ));
    }
    changed.insert(source.provenance.clone(), source.text.clone());
    Ok(())
}

pub(crate) fn prepare_source(
    root: SourceFile,
    allow_media_include: bool,
    loader: &mut impl FnMut(&IncludeRequest) -> Result<SourceFile, String>,
) -> Result<PreparedSource, SourceDocumentError> {
    strict_yaml::reject_duplicate_keys(&root.text)
        .map_err(|error| SourceDocumentError::source(&root.provenance, error.to_string()))?;
    let mut value = serde_yaml::from_str::<Value>(&root.text)
        .map_err(|error| SourceDocumentError::source(&root.provenance, error.to_string()))?;
    let mut includes = IncludeState::default();
    let original = root.text.clone();
    prepare_value(
        &mut value,
        &mut Vec::new(),
        &root.provenance,
        &original,
        allow_media_include,
        loader,
        &mut includes,
        &mut Vec::new(),
    )?;
    let yaml_without_directives = serde_yaml::to_string(&value)
        .map_err(|error| SourceDocumentError::source(&root.provenance, error.to_string()))?;
    materialize_scalar_includes(&mut value, &includes);
    let materialized_yaml = serde_yaml::to_string(&value)
        .map_err(|error| SourceDocumentError::source(&root.provenance, error.to_string()))?;
    Ok(PreparedSource {
        root,
        yaml_without_directives,
        materialized_yaml,
        includes,
    })
}

fn materialize_scalar_includes(value: &mut Value, includes: &IncludeState) {
    match value {
        Value::String(text) => {
            if let Some(include) = includes
                .scalar
                .values()
                .find(|include| include.sentinel == *text)
            {
                *text = include.value.clone();
            }
        }
        Value::Tagged(tagged) => materialize_scalar_includes(&mut tagged.value, includes),
        Value::Sequence(sequence) => {
            for item in sequence {
                materialize_scalar_includes(item, includes);
            }
        }
        Value::Mapping(mapping) => {
            for item in mapping.values_mut() {
                materialize_scalar_includes(item, includes);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn prepare_value(
    value: &mut Value,
    path: &mut Vec<String>,
    root: &SourceProvenance,
    original: &str,
    allow_media_include: bool,
    loader: &mut impl FnMut(&IncludeRequest) -> Result<SourceFile, String>,
    includes: &mut IncludeState,
    stack: &mut Vec<String>,
) -> Result<(), SourceDocumentError> {
    match value {
        Value::Tagged(tagged) if tagged.tag == "include" => {
            let Value::String(target) = &tagged.value else {
                return Err(SourceDocumentError::at(
                    root,
                    display_path(path),
                    "!include path must be a scalar string",
                ));
            };
            let target = target.clone();
            let schema_path = canonical_source_path(path);
            let request = IncludeRequest {
                referring_source: root.clone(),
                schema_path: schema_path.clone(),
                target: target.clone(),
            };
            if path.as_slice() == ["note_types"] {
                if !allow_media_include {
                    return Err(SourceDocumentError::at(
                        root,
                        schema_path,
                        "structural note_types includes are valid only in Canonical Deck source",
                    ));
                }
                let loaded = loader(&request).map_err(|message| {
                    SourceDocumentError::at(
                        root,
                        "note_types",
                        format!("could not load !include {target:?}: {message}"),
                    )
                })?;
                strict_yaml::reject_duplicate_keys(&loaded.text).map_err(|error| {
                    SourceDocumentError::at(
                        root,
                        "note_types",
                        format!(
                            "invalid included note-type map {}: {error}",
                            loaded.provenance
                        ),
                    )
                })?;
                let mut raw_value =
                    serde_yaml::from_str::<Value>(&loaded.text).map_err(|error| {
                        SourceDocumentError::at(
                            root,
                            "note_types",
                            format!(
                                "invalid included note-type map {}: {error}",
                                loaded.provenance
                            ),
                        )
                    })?;
                if !matches!(raw_value, Value::Mapping(_)) {
                    return Err(SourceDocumentError::at(
                        root,
                        "note_types",
                        format!(
                            "included note-type map {} must have a mapping root",
                            loaded.provenance
                        ),
                    ));
                }
                let mut included_path = vec!["note_types".to_owned()];
                prepare_value(
                    &mut raw_value,
                    &mut included_path,
                    &loaded.provenance,
                    &loaded.text,
                    false,
                    loader,
                    includes,
                    stack,
                )?;
                let source_yaml = serde_yaml::to_string(&raw_value).map_err(|error| {
                    SourceDocumentError::source(&loaded.provenance, error.to_string())
                })?;
                let note_types = note_type_map::from_str(&source_yaml).map_err(|error| {
                    SourceDocumentError::at(
                        root,
                        "note_types",
                        format!(
                            "invalid included note-type map {}: {error}",
                            loaded.provenance
                        ),
                    )
                })?;
                let mut resolved_value = raw_value;
                materialize_scalar_includes(&mut resolved_value, includes);
                let resolved_yaml = serde_yaml::to_string(&resolved_value).map_err(|error| {
                    SourceDocumentError::source(&loaded.provenance, error.to_string())
                })?;
                let resolved_note_types =
                    note_type_map::from_str(&resolved_yaml).map_err(|error| {
                        SourceDocumentError::at(
                            root,
                            "note_types",
                            format!(
                                "invalid included note-type map {} after resolving scalar includes: {error}",
                                loaded.provenance
                            ),
                        )
                    })?;
                includes.loaded_sources.insert(IncludedSource {
                    kind: IncludedSourceKind::NoteTypeDeclarations,
                    provenance: loaded.provenance.clone(),
                });
                includes.note_types = Some(NoteTypesInclude {
                    directive: format!("note_types: !include {}", yaml_scalar::scalar(&target)),
                    source: loaded,
                    note_types,
                    resolved_note_types,
                    dirty: false,
                });
                *value = Value::Mapping(Mapping::new());
                return Ok(());
            }
            if path.as_slice() == ["media"] {
                if !allow_media_include {
                    return Err(SourceDocumentError::at(
                        root,
                        schema_path,
                        "structural media includes are valid only in Canonical Deck source",
                    ));
                }
                let loaded = loader(&request).map_err(|message| {
                    SourceDocumentError::at(
                        root,
                        "media",
                        format!("could not load !include {target:?}: {message}"),
                    )
                })?;
                let media = media_map::from_str(&loaded.text).map_err(|error| {
                    SourceDocumentError::at(
                        &loaded.provenance,
                        "media",
                        format!("invalid included media map: {error}"),
                    )
                })?;
                includes.loaded_sources.insert(IncludedSource {
                    kind: IncludedSourceKind::MediaDeclarations,
                    provenance: loaded.provenance.clone(),
                });
                includes.media = Some(MediaInclude {
                    directive: format!("media: !include {}", yaml_scalar::scalar(&target)),
                    source: loaded,
                    media,
                    dirty: false,
                });
                *value = Value::Mapping(Mapping::new());
                return Ok(());
            }
            if !crate::source_includes::is_scalar_content_path(path) {
                return Err(SourceDocumentError::at(
                    root,
                    schema_path,
                    format!("!include {target:?} is only valid for scalar content fields"),
                ));
            }
            let loaded = load_scalar_include(request, loader, stack, &mut includes.loaded_sources)?;
            let sentinel = next_sentinel(original, includes.scalar.len());
            includes.scalar.insert(
                schema_path,
                ScalarInclude {
                    sentinel: sentinel.clone(),
                    directive: format!("!include {}", yaml_scalar::scalar(&target)),
                    source: loaded.source,
                    value: loaded.value,
                    fragment: loaded.fragment,
                    dirty: false,
                },
            );
            *value = Value::String(sentinel);
        }
        Value::Tagged(tagged) => prepare_value(
            &mut tagged.value,
            path,
            root,
            original,
            allow_media_include,
            loader,
            includes,
            stack,
        )?,
        Value::Mapping(mapping) => {
            if path.as_slice() == ["media"] {
                take_media_asset_sources(mapping, root, includes)?;
            }
            for (key, item) in mapping {
                path.push(path_segment(key));
                prepare_value(
                    item,
                    path,
                    root,
                    original,
                    allow_media_include,
                    loader,
                    includes,
                    stack,
                )?;
                path.pop();
            }
        }
        Value::Sequence(sequence) => {
            for (index, item) in sequence.iter_mut().enumerate() {
                path.push(format!("[{index}]"));
                prepare_value(
                    item,
                    path,
                    root,
                    original,
                    allow_media_include,
                    loader,
                    includes,
                    stack,
                )?;
                path.pop();
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}

fn take_media_asset_sources(
    media: &mut Mapping,
    root: &SourceProvenance,
    includes: &mut IncludeState,
) -> Result<(), SourceDocumentError> {
    let source_key = Value::String("source".to_owned());
    for (id, declaration) in media {
        let Some(id) = id.as_str() else {
            continue;
        };
        let Some(declaration) = declaration.as_mapping_mut() else {
            continue;
        };
        let Some(source) = declaration.remove(&source_key) else {
            continue;
        };
        let Value::String(source) = source else {
            return Err(SourceDocumentError::at(
                root,
                format!("media.{id}.source"),
                "media source must be a scalar package-relative path",
            ));
        };
        crate::safe_relative_path::SafeRelativePath::new(&source).map_err(|error| {
            SourceDocumentError::at(root, format!("media.{id}.source"), error.to_string())
        })?;
        let id = StableId::new(id).map_err(|error| {
            SourceDocumentError::at(root, format!("media.{id}.source"), error.to_string())
        })?;
        includes.media_asset_sources.insert(id, source);
    }
    Ok(())
}

fn load_scalar_include(
    request: IncludeRequest,
    loader: &mut impl FnMut(&IncludeRequest) -> Result<SourceFile, String>,
    stack: &mut Vec<String>,
    loaded_sources: &mut BTreeSet<IncludedSource>,
) -> Result<LoadedScalarInclude, SourceDocumentError> {
    let original_target = request.target.clone();
    let (target, fragment) = crate::source_includes::parse_include_target(&original_target)
        .map_err(|message| {
            SourceDocumentError::at(&request.referring_source, &request.schema_path, message)
        })?;
    let request = IncludeRequest {
        referring_source: request.referring_source,
        schema_path: request.schema_path,
        target: target.to_owned(),
    };
    let cycle_key = format!("{} -> {}", request.referring_source, request.target);
    if stack.contains(&cycle_key) {
        return Err(SourceDocumentError::at(
            &request.referring_source,
            &request.schema_path,
            format!(
                "cyclic scalar include: {} -> {cycle_key}",
                stack.join(" -> ")
            ),
        ));
    }
    stack.push(cycle_key);
    let loaded = loader(&request).map_err(|message| {
        SourceDocumentError::at(
            &request.referring_source,
            &request.schema_path,
            format!("could not load !include {original_target:?}: {message}"),
        )
    })?;
    loaded_sources.insert(IncludedSource {
        kind: IncludedSourceKind::Scalar {
            schema_path: request.schema_path.clone(),
        },
        provenance: loaded.provenance.clone(),
    });
    let value = fragment
        .map(|fragment| crate::source_includes::select_include_fragment(&loaded.text, fragment))
        .transpose()
        .map_err(|message| {
            SourceDocumentError::at(&loaded.provenance, &request.schema_path, message)
        })?
        .unwrap_or_else(|| loaded.text.clone());
    let nested = nested_include_target(&value).map_err(|message| {
        SourceDocumentError::at(&loaded.provenance, &request.schema_path, message)
    })?;
    let result = if let Some(target) = nested {
        load_scalar_include(
            IncludeRequest {
                referring_source: loaded.provenance.clone(),
                schema_path: request.schema_path,
                target,
            },
            loader,
            stack,
            loaded_sources,
        )
    } else {
        Ok(LoadedScalarInclude {
            source: loaded,
            value,
            fragment,
        })
    };
    stack.pop();
    result
}

fn nested_include_target(content: &str) -> Result<Option<String>, String> {
    let trimmed = content.trim();
    if trimmed != "!include" && !trimmed.starts_with("!include ") {
        return Ok(None);
    }
    let value = serde_yaml::from_str::<Value>(trimmed).map_err(|error| error.to_string())?;
    match value {
        Value::Tagged(tagged) if tagged.tag == "include" => match tagged.value {
            Value::String(path) => Ok(Some(path)),
            _ => Err("!include path must be a scalar string".to_owned()),
        },
        _ => Ok(None),
    }
}

fn next_sentinel(original: &str, index: usize) -> String {
    let mut attempt = 0;
    loop {
        let sentinel = format!("__brain_brew_source_document_include_{index}_{attempt}__");
        if !original.contains(&sentinel) {
            return sentinel;
        }
        attempt += 1;
    }
}

fn path_segment(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{value:?}"))
}

fn canonical_source_path(path: &[String]) -> String {
    if path.len() > 1 && path[0] == "notes" && path[1].starts_with('[') && path[1].ends_with(']') {
        let mut canonical = path.to_vec();
        canonical.remove(1);
        return display_path(&canonical);
    }
    display_path(path)
}

fn display_path(path: &[String]) -> String {
    let mut output = String::new();
    for segment in path {
        if segment.starts_with('[') {
            output.push_str(segment);
        } else {
            if !output.is_empty() {
                output.push('.');
            }
            output.push_str(segment);
        }
    }
    output
}

pub(crate) fn convert_text_to_images(
    text: &str,
    lookup: &BTreeMap<String, Option<StableId>>,
    report: &mut ImageConversionReport,
) -> Option<Vec<brain_brew_core::FieldImageReference>> {
    let Some(paths) = crate::media::strict_image_tag_paths(text) else {
        if text.trim().contains("<img") {
            report.skipped_non_strict += 1;
        }
        return None;
    };
    let mut ids = Vec::new();
    let mut missing = false;
    let mut ambiguous = false;
    for path in paths {
        match lookup.get(&path) {
            Some(Some(id)) => ids.push(id.clone()),
            Some(None) => ambiguous = true,
            None => missing = true,
        }
    }
    if missing {
        report.skipped_no_match += 1;
        return None;
    }
    if ambiguous {
        report.skipped_ambiguous_path += 1;
        return None;
    }
    report.converted += 1;
    Some(
        ids.into_iter()
            .map(|media_id| brain_brew_core::FieldImageReference { media_id })
            .collect(),
    )
}

pub(crate) fn ensure_non_empty(value: &str, name: &str) -> Result<(), String> {
    if value.is_empty() {
        Err(format!("{name} must not be empty"))
    } else {
        Ok(())
    }
}
