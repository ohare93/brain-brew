//! Strict, filesystem-free CSV note Authoring Source materialization.
//!
//! This module owns the typed `!csv` declaration and descriptor for the first,
//! deliberately narrow single-table slice. Callers authorize and inject all
//! descriptor and CSV bytes; no path is opened here.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use brain_brew_core::{AdapterIds, FieldValue, Note, NoteType, StableId};
use serde::Deserialize;
use serde_yaml::Value;

use crate::source_document::{SourceFile, SourceProvenance};

/// Bytes for one caller-authorized CSV authoring input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CsvSourceFile {
    provenance: SourceProvenance,
    bytes: Vec<u8>,
}

impl CsvSourceFile {
    pub fn new(provenance: SourceProvenance, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            provenance,
            bytes: bytes.into(),
        }
    }

    pub fn provenance(&self) -> &SourceProvenance {
        &self.provenance
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// The kind of source requested by [`CanonicalSourceDocument`](crate::canonical_source_document::CanonicalSourceDocument).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CsvSourceRequestKind {
    Descriptor,
    Table { alias: String },
}

/// Caller-owned load request for a descriptor or CSV table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CsvSourceRequest {
    referring_source: SourceProvenance,
    schema_path: String,
    target: String,
    kind: CsvSourceRequestKind,
}

impl CsvSourceRequest {
    pub(crate) fn descriptor(referring_source: SourceProvenance, target: String) -> Self {
        Self {
            referring_source,
            schema_path: "notes.descriptor".to_owned(),
            target,
            kind: CsvSourceRequestKind::Descriptor,
        }
    }

    pub(crate) fn table(referring_source: SourceProvenance, alias: String, target: String) -> Self {
        Self {
            referring_source,
            schema_path: format!("notes.tables.{alias}"),
            target,
            kind: CsvSourceRequestKind::Table { alias },
        }
    }

    pub fn referring_source(&self) -> &SourceProvenance {
        &self.referring_source
    }

    pub fn schema_path(&self) -> &str {
        &self.schema_path
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn kind(&self) -> &CsvSourceRequestKind {
        &self.kind
    }
}

/// Source-preserved declaration stored at the root `notes: !csv` boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CsvNoteSourceDeclaration {
    descriptor: String,
    parameters: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDeclaration {
    descriptor: String,
    parameters: BTreeMap<String, String>,
}

impl CsvNoteSourceDeclaration {
    pub fn descriptor(&self) -> &str {
        &self.descriptor
    }

    pub fn parameters(&self) -> &BTreeMap<String, String> {
        &self.parameters
    }

    pub(crate) fn take_from_root(
        root: &mut Value,
        provenance: &SourceProvenance,
    ) -> Result<Option<Self>, CsvNoteSourceError> {
        let Some(mapping) = root.as_mapping_mut() else {
            return Ok(None);
        };
        let notes_key = Value::String("notes".to_owned());
        let Some(notes) = mapping.get_mut(&notes_key) else {
            return Ok(None);
        };
        let Value::Tagged(tagged) = notes else {
            return Ok(None);
        };
        if tagged.tag != "csv" {
            return Ok(None);
        }
        let raw: RawDeclaration =
            serde_yaml::from_value(tagged.value.clone()).map_err(|error| {
                CsvNoteSourceError::new(
                    provenance.clone(),
                    None,
                    None,
                    None,
                    format!("invalid notes !csv declaration: {error}"),
                )
            })?;
        if raw.descriptor.is_empty() {
            return Err(CsvNoteSourceError::new(
                provenance.clone(),
                None,
                None,
                None,
                "notes !csv descriptor path must not be empty",
            ));
        }
        if !raw.parameters.is_empty() {
            return Err(CsvNoteSourceError::new(
                provenance.clone(),
                None,
                None,
                None,
                "notes !csv parameters are not supported by the single-table materialization slice",
            ));
        }
        *notes = Value::Mapping(serde_yaml::Mapping::new());
        Ok(Some(Self {
            descriptor: raw.descriptor,
            parameters: raw.parameters,
        }))
    }

    pub(crate) fn restore(&self, mut canonical: String) -> Result<String, String> {
        let start = crate::strict_yaml::top_level_mapping_key_offset(&canonical, "notes")
            .ok_or_else(|| "canonical emission omitted the notes section".to_owned())?;
        let relative_end =
            crate::strict_yaml::top_level_mapping_key_offset(&canonical[start..], "media")
                .ok_or_else(|| "canonical emission omitted media after notes".to_owned())?;
        let end = start + relative_end;
        let replacement = format!(
            "notes: !csv\n  descriptor: {}\n  parameters: {{}}\n",
            crate::yaml_scalar::scalar(&self.descriptor)
        );
        canonical.replace_range(start..end, &replacement);
        Ok(canonical)
    }
}

/// Strict reusable descriptor for the initial single-table scalar-note slice.
#[derive(Clone, Debug)]
pub struct CsvNoteSourceDescriptor {
    provenance: SourceProvenance,
    primary_table: String,
    tables: BTreeMap<String, CsvTableDescriptor>,
    note: CsvNoteMapping,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDescriptor {
    version: u32,
    primary_table: String,
    tables: BTreeMap<String, CsvTableDescriptor>,
    parameters: BTreeMap<String, Value>,
    joins: Vec<Value>,
    note: CsvNoteMapping,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CsvTableDescriptor {
    path: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CsvNoteMapping {
    id: String,
    note_type_id: String,
    fields: BTreeMap<String, CsvScalarFieldMapping>,
    tags: CsvTagsMapping,
    adapter_ids: BTreeMap<String, CsvAdapterIdMapping>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CsvScalarFieldMapping {
    column: String,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CsvTagsMapping {
    column: String,
    delimiter: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CsvAdapterIdMapping {
    column: String,
}

impl CsvNoteSourceDescriptor {
    pub fn parse(source: SourceFile) -> Result<Self, CsvNoteSourceError> {
        crate::strict_yaml::reject_duplicate_keys(source.text()).map_err(|error| {
            CsvNoteSourceError::descriptor(source.provenance(), error.to_string())
        })?;
        let raw: RawDescriptor = serde_yaml::from_str(source.text()).map_err(|error| {
            CsvNoteSourceError::descriptor(source.provenance(), error.to_string())
        })?;
        if raw.version != 1 {
            return Err(CsvNoteSourceError::descriptor(
                source.provenance(),
                format!(
                    "unsupported CSV note descriptor version {}; expected 1",
                    raw.version
                ),
            ));
        }
        if raw.primary_table.is_empty() {
            return Err(CsvNoteSourceError::descriptor(
                source.provenance(),
                "primary_table must not be empty",
            ));
        }
        if raw.tables.len() != 1 {
            return Err(CsvNoteSourceError::descriptor(
                source.provenance(),
                "the single-table materialization slice requires exactly one table",
            ));
        }
        let Some(table) = raw.tables.get(&raw.primary_table) else {
            return Err(CsvNoteSourceError::descriptor(
                source.provenance(),
                format!("primary table {:?} is not declared", raw.primary_table),
            ));
        };
        if table.path.is_empty() {
            return Err(CsvNoteSourceError::descriptor(
                source.provenance(),
                format!("table {:?} path must not be empty", raw.primary_table),
            ));
        }
        if !raw.parameters.is_empty() {
            return Err(CsvNoteSourceError::descriptor(
                source.provenance(),
                "parameters are not supported by the single-table materialization slice",
            ));
        }
        if !raw.joins.is_empty() {
            return Err(CsvNoteSourceError::descriptor(
                source.provenance(),
                "joins are not supported by the single-table materialization slice",
            ));
        }
        if raw.note.fields.is_empty() {
            return Err(CsvNoteSourceError::descriptor(
                source.provenance(),
                "note fields mapping must not be empty",
            ));
        }
        if raw.note.tags.delimiter.is_empty() {
            return Err(CsvNoteSourceError::descriptor(
                source.provenance(),
                "tag delimiter must not be empty",
            ));
        }
        validate_qualified_column(source.provenance(), &raw.note.id, &raw.primary_table)?;
        for (field_id, field) in &raw.note.fields {
            StableId::new(field_id.clone()).map_err(|error| {
                CsvNoteSourceError::descriptor(source.provenance(), error.to_string())
            })?;
            validate_qualified_column(source.provenance(), &field.column, &raw.primary_table)?;
            if field.kind != "scalar" {
                return Err(CsvNoteSourceError::descriptor(
                    source.provenance(),
                    format!(
                        "field mapping {field_id} has unsupported type {:?}; expected scalar",
                        field.kind
                    ),
                ));
            }
        }
        validate_qualified_column(
            source.provenance(),
            &raw.note.tags.column,
            &raw.primary_table,
        )?;
        for (namespace, mapping) in &raw.note.adapter_ids {
            if namespace.is_empty() {
                return Err(CsvNoteSourceError::descriptor(
                    source.provenance(),
                    "adapter namespace must not be empty",
                ));
            }
            validate_qualified_column(source.provenance(), &mapping.column, &raw.primary_table)?;
        }
        StableId::new(raw.note.note_type_id.clone()).map_err(|error| {
            CsvNoteSourceError::descriptor(source.provenance(), error.to_string())
        })?;
        Ok(Self {
            provenance: source.provenance().clone(),
            primary_table: raw.primary_table,
            tables: raw.tables,
            note: raw.note,
        })
    }

    pub fn provenance(&self) -> &SourceProvenance {
        &self.provenance
    }

    pub fn table_paths(&self) -> impl Iterator<Item = (&str, &str)> {
        self.tables
            .iter()
            .map(|(alias, table)| (alias.as_str(), table.path.as_str()))
    }
}

fn validate_qualified_column(
    provenance: &SourceProvenance,
    value: &str,
    primary_table: &str,
) -> Result<(), CsvNoteSourceError> {
    let Some((alias, header)) = value.split_once('.') else {
        return Err(CsvNoteSourceError::descriptor(
            provenance,
            format!("column reference {value:?} must be qualified as table.header"),
        ));
    };
    if alias != primary_table {
        return Err(CsvNoteSourceError::descriptor(
            provenance,
            format!("column reference {value:?} does not use primary table {primary_table:?}"),
        ));
    }
    if header.is_empty() {
        return Err(CsvNoteSourceError::descriptor(
            provenance,
            format!("column reference {value:?} has an empty header"),
        ));
    }
    Ok(())
}

fn column_header<'a>(qualified: &'a str, alias: &str) -> &'a str {
    qualified
        .strip_prefix(alias)
        .and_then(|value| value.strip_prefix('.'))
        .expect("descriptor qualification was validated")
}

/// Materializes one validated single-table descriptor into ordinary core notes.
pub struct CsvNoteSourceMaterializer {
    descriptor: CsvNoteSourceDescriptor,
}

impl CsvNoteSourceMaterializer {
    pub fn new(descriptor: CsvNoteSourceDescriptor) -> Self {
        Self { descriptor }
    }

    pub fn materialize(
        &self,
        tables: &BTreeMap<String, CsvSourceFile>,
        note_types: &BTreeMap<StableId, NoteType>,
    ) -> Result<BTreeMap<StableId, Note>, CsvNoteSourceError> {
        let note_type_id =
            StableId::new(self.descriptor.note.note_type_id.clone()).map_err(|error| {
                CsvNoteSourceError::descriptor(&self.descriptor.provenance, error.to_string())
            })?;
        let note_type = note_types.get(&note_type_id).ok_or_else(|| {
            CsvNoteSourceError::descriptor(
                &self.descriptor.provenance,
                format!("mapped note type {note_type_id} is not declared by the deck"),
            )
        })?;
        self.validate_field_completeness(note_type)?;

        let source = tables.get(&self.descriptor.primary_table).ok_or_else(|| {
            CsvNoteSourceError::descriptor(
                &self.descriptor.provenance,
                format!(
                    "caller did not provide bytes for primary table {:?}",
                    self.descriptor.primary_table
                ),
            )
        })?;
        validate_csv_bytes(source)?;
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .flexible(false)
            .from_reader(source.bytes());
        let headers = reader
            .headers()
            .map_err(|error| csv_error(source, error))?
            .clone();
        validate_headers(source, &headers)?;

        let alias = &self.descriptor.primary_table;
        let id_index = required_header(
            source,
            &headers,
            column_header(&self.descriptor.note.id, alias),
        )?;
        let field_indexes = self
            .descriptor
            .note
            .fields
            .iter()
            .map(|(field_id, mapping)| {
                let stable_id = StableId::new(field_id.clone()).expect("mapping IDs validated");
                let header = column_header(&mapping.column, alias);
                Ok((stable_id, required_header(source, &headers, header)?))
            })
            .collect::<Result<BTreeMap<_, _>, CsvNoteSourceError>>()?;
        let tags_header = column_header(&self.descriptor.note.tags.column, alias);
        let tags_index = required_header(source, &headers, tags_header)?;
        let adapter_indexes = self
            .descriptor
            .note
            .adapter_ids
            .iter()
            .map(|(namespace, mapping)| {
                let header = column_header(&mapping.column, alias);
                Ok((
                    namespace.clone(),
                    required_header(source, &headers, header)?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>, CsvNoteSourceError>>()?;

        let mut notes = BTreeMap::new();
        for record in reader.records() {
            let record = record.map_err(|error| csv_error(source, error))?;
            let logical_row = record
                .position()
                .map_or(2, |position| position.record().saturating_add(1));
            let id_cell = &record[id_index];
            let id = StableId::new(id_cell.to_owned()).map_err(|error| {
                CsvNoteSourceError::cell(
                    source,
                    logical_row,
                    id_index,
                    &headers[id_index],
                    error.to_string(),
                )
            })?;
            if notes.contains_key(&id) {
                return Err(CsvNoteSourceError::cell(
                    source,
                    logical_row,
                    id_index,
                    &headers[id_index],
                    format!("duplicate stable note ID {id}"),
                ));
            }
            let fields = field_indexes
                .iter()
                .map(|(field_id, index)| {
                    (
                        field_id.clone(),
                        FieldValue::Scalar(record[*index].to_owned()),
                    )
                })
                .collect();
            let tags = parse_tags(
                source,
                logical_row,
                tags_index,
                tags_header,
                &record[tags_index],
                &self.descriptor.note.tags.delimiter,
            )?;
            let mut adapter_ids = AdapterIds::new();
            for (namespace, index) in &adapter_indexes {
                let value = &record[*index];
                if !value.is_empty() {
                    adapter_ids.insert(namespace, value);
                }
            }
            notes.insert(
                id.clone(),
                Note {
                    id,
                    note_type_id: note_type_id.clone(),
                    variables: BTreeMap::new(),
                    fields,
                    tags,
                    adapter_ids,
                },
            );
        }
        Ok(notes)
    }

    fn validate_field_completeness(&self, note_type: &NoteType) -> Result<(), CsvNoteSourceError> {
        let declared = note_type
            .fields
            .iter()
            .map(|field| field.id.clone())
            .collect::<BTreeSet<_>>();
        let mapped = self
            .descriptor
            .note
            .fields
            .keys()
            .map(|id| StableId::new(id.clone()).expect("mapping IDs validated"))
            .collect::<BTreeSet<_>>();
        if let Some(unknown) = mapped.difference(&declared).next() {
            return Err(CsvNoteSourceError::descriptor(
                &self.descriptor.provenance,
                format!("unknown field mapping {unknown}"),
            ));
        }
        if let Some(missing) = declared.difference(&mapped).next() {
            return Err(CsvNoteSourceError::descriptor(
                &self.descriptor.provenance,
                format!("missing field mapping {missing}"),
            ));
        }
        Ok(())
    }
}

fn validate_csv_bytes(source: &CsvSourceFile) -> Result<(), CsvNoteSourceError> {
    if source.bytes().starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Err(CsvNoteSourceError::new(
            source.provenance().clone(),
            Some(1),
            Some(1),
            None,
            "UTF-8 BOM is not allowed before the first header",
        ));
    }
    std::str::from_utf8(source.bytes()).map_err(|error| {
        CsvNoteSourceError::new(
            source.provenance().clone(),
            None,
            None,
            None,
            format!("invalid UTF-8 at byte {}: {error}", error.valid_up_to()),
        )
    })?;
    Ok(())
}

fn validate_headers(
    source: &CsvSourceFile,
    headers: &csv::StringRecord,
) -> Result<(), CsvNoteSourceError> {
    let mut seen = BTreeSet::new();
    for (index, header) in headers.iter().enumerate() {
        if header.is_empty() {
            return Err(CsvNoteSourceError::cell(
                source,
                1,
                index,
                header,
                "empty header is not allowed",
            ));
        }
        if !seen.insert(header) {
            return Err(CsvNoteSourceError::cell(
                source,
                1,
                index,
                header,
                format!("duplicate header {header:?}"),
            ));
        }
    }
    Ok(())
}

fn required_header(
    source: &CsvSourceFile,
    headers: &csv::StringRecord,
    required: &str,
) -> Result<usize, CsvNoteSourceError> {
    headers
        .iter()
        .position(|header| header == required)
        .ok_or_else(|| {
            CsvNoteSourceError::new(
                source.provenance().clone(),
                Some(1),
                None,
                Some(required.to_owned()),
                format!("mapped header {required:?} is absent"),
            )
        })
}

fn parse_tags(
    source: &CsvSourceFile,
    row: u64,
    column: usize,
    header: &str,
    value: &str,
    delimiter: &str,
) -> Result<BTreeSet<String>, CsvNoteSourceError> {
    if value.is_empty() {
        return Ok(BTreeSet::new());
    }
    let mut tags = BTreeSet::new();
    for tag in value.split(delimiter) {
        if tag.is_empty() {
            return Err(CsvNoteSourceError::cell(
                source,
                row,
                column,
                header,
                "empty tag segment is not allowed",
            ));
        }
        if !tags.insert(tag.to_owned()) {
            return Err(CsvNoteSourceError::cell(
                source,
                row,
                column,
                header,
                format!("duplicate tag {tag:?}"),
            ));
        }
    }
    Ok(tags)
}

fn csv_error(source: &CsvSourceFile, error: csv::Error) -> CsvNoteSourceError {
    let row = error
        .position()
        .map(|position| position.record().saturating_add(1));
    CsvNoteSourceError::new(
        source.provenance().clone(),
        row,
        None,
        None,
        error.to_string(),
    )
}

/// Source-, logical-row-, and column-aware CSV materialization diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CsvNoteSourceError {
    provenance: Box<SourceProvenance>,
    row: Option<u64>,
    column: Option<usize>,
    header: Option<String>,
    message: String,
}

impl CsvNoteSourceError {
    fn descriptor(provenance: &SourceProvenance, message: impl Into<String>) -> Self {
        Self::new(provenance.clone(), None, None, None, message)
    }

    fn cell(
        source: &CsvSourceFile,
        row: u64,
        zero_based_column: usize,
        header: &str,
        message: impl Into<String>,
    ) -> Self {
        Self::new(
            source.provenance().clone(),
            Some(row),
            Some(zero_based_column + 1),
            (!header.is_empty()).then(|| header.to_owned()),
            message,
        )
    }

    fn new(
        provenance: SourceProvenance,
        row: Option<u64>,
        column: Option<usize>,
        header: Option<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            provenance: Box::new(provenance),
            row,
            column,
            header,
            message: message.into(),
        }
    }

    pub fn provenance(&self) -> &SourceProvenance {
        &self.provenance
    }

    pub fn row(&self) -> Option<u64> {
        self.row
    }

    pub fn column(&self) -> Option<usize> {
        self.column
    }

    pub fn header(&self) -> Option<&str> {
        self.header.as_deref()
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for CsvNoteSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.provenance)?;
        if let Some(row) = self.row {
            write!(formatter, ":row {row}")?;
        }
        if let Some(column) = self.column {
            write!(formatter, ":column {column}")?;
        }
        if let Some(header) = &self.header {
            write!(formatter, " ({header})")?;
        }
        write!(formatter, ": {}", self.message)
    }
}

impl std::error::Error for CsvNoteSourceError {}
