//! Strict, filesystem-free CSV note Authoring Source materialization.
//!
//! This module owns the typed `!csv` declaration, explicit flat joins, and
//! literal localized-column parameters. Callers authorize and inject all
//! descriptor and CSV bytes; no path is opened here.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use brain_brew_core::{AdapterIds, FieldImageReference, FieldValue, Note, NoteType, StableId};
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
        let mut replacement = format!(
            "notes: !csv\n  descriptor: {}\n",
            crate::yaml_scalar::scalar(&self.descriptor)
        );
        if self.parameters.is_empty() {
            replacement.push_str("  parameters: {}\n");
        } else {
            replacement.push_str("  parameters:\n");
            for (name, value) in &self.parameters {
                let name = crate::yaml_scalar::key(name)
                    .ok_or_else(|| format!("parameter name {name:?} cannot be emitted"))?;
                replacement.push_str(&format!(
                    "    {name}: {}\n",
                    crate::yaml_scalar::scalar(value)
                ));
            }
        }
        canonical.replace_range(start..end, &replacement);
        Ok(canonical)
    }
}

/// Strict reusable descriptor for CSV-backed notes with explicit flat joins.
#[derive(Clone, Debug)]
pub struct CsvNoteSourceDescriptor {
    provenance: SourceProvenance,
    primary_table: String,
    tables: BTreeMap<String, CsvTableDescriptor>,
    parameters: BTreeMap<String, CsvLocalizedColumnParameter>,
    joins: Vec<CsvJoinDescriptor>,
    note: CsvNoteMapping,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDescriptor {
    version: u32,
    primary_table: String,
    tables: BTreeMap<String, CsvTableDescriptor>,
    parameters: BTreeMap<String, CsvLocalizedColumnParameter>,
    joins: Vec<CsvJoinDescriptor>,
    note: CsvNoteMapping,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CsvTableDescriptor {
    path: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CsvLocalizedColumnParameter {
    #[serde(rename = "type")]
    kind: String,
    default: String,
    separator: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CsvJoinDescriptor {
    left: String,
    right: String,
    #[serde(default = "default_required_join")]
    required: bool,
}

fn default_required_join() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CsvNoteMapping {
    id: String,
    note_type_id: String,
    fields: BTreeMap<String, CsvFieldMapping>,
    tags: CsvTagsMapping,
    adapter_ids: BTreeMap<String, CsvAdapterIdMapping>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CsvFieldMapping {
    column: String,
    #[serde(default)]
    localized_by: Option<String>,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Clone, Copy)]
enum CsvFieldType {
    Scalar,
    Image,
}

impl CsvFieldType {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "scalar" => Some(Self::Scalar),
            "image" => Some(Self::Image),
            _ => None,
        }
    }
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
    #[serde(default)]
    localized_by: Option<String>,
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
        validate_name(
            source.provenance(),
            "primary table alias",
            &raw.primary_table,
        )?;
        let Some(_) = raw.tables.get(&raw.primary_table) else {
            return Err(CsvNoteSourceError::descriptor(
                source.provenance(),
                format!("primary table {:?} is not declared", raw.primary_table),
            ));
        };
        for (alias, table) in &raw.tables {
            validate_name(source.provenance(), "table alias", alias)?;
            if table.path.is_empty() {
                return Err(CsvNoteSourceError::descriptor(
                    source.provenance(),
                    format!("table {alias:?} path must not be empty"),
                ));
            }
        }
        for (name, parameter) in &raw.parameters {
            validate_name(source.provenance(), "parameter name", name)?;
            if parameter.kind != "localized_column" {
                return Err(CsvNoteSourceError::descriptor(
                    source.provenance(),
                    format!(
                        "parameter {name:?} has unsupported type {:?}; expected localized_column",
                        parameter.kind
                    ),
                ));
            }
            if !parameter.default.is_empty() {
                return Err(CsvNoteSourceError::descriptor(
                    source.provenance(),
                    format!("localized_column parameter {name:?} default must be empty"),
                ));
            }
        }

        let mut joined_aliases = BTreeSet::new();
        for join in &raw.joins {
            let (left_alias, _) = qualified_column(source.provenance(), &join.left, &raw.tables)?;
            let (right_alias, _) = qualified_column(source.provenance(), &join.right, &raw.tables)?;
            if left_alias != raw.primary_table {
                return Err(CsvNoteSourceError::descriptor(
                    source.provenance(),
                    format!(
                        "join left column {:?} must belong to primary table {:?}",
                        join.left, raw.primary_table
                    ),
                ));
            }
            if right_alias == raw.primary_table {
                return Err(CsvNoteSourceError::descriptor(
                    source.provenance(),
                    format!(
                        "join right column {:?} must belong to a joined table",
                        join.right
                    ),
                ));
            }
            if !joined_aliases.insert(right_alias.to_owned()) {
                return Err(CsvNoteSourceError::descriptor(
                    source.provenance(),
                    format!("joined table alias {right_alias:?} is joined more than once"),
                ));
            }
        }
        for alias in raw.tables.keys() {
            if alias != &raw.primary_table && !joined_aliases.contains(alias) {
                return Err(CsvNoteSourceError::descriptor(
                    source.provenance(),
                    format!("non-primary table {alias:?} has no explicit join"),
                ));
            }
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
        let (id_alias, _) = qualified_column(source.provenance(), &raw.note.id, &raw.tables)?;
        if id_alias != raw.primary_table {
            return Err(CsvNoteSourceError::descriptor(
                source.provenance(),
                format!(
                    "note ID column must belong to primary table {:?}",
                    raw.primary_table
                ),
            ));
        }
        for (field_id, field) in &raw.note.fields {
            StableId::new(field_id.clone()).map_err(|error| {
                CsvNoteSourceError::descriptor(source.provenance(), error.to_string())
            })?;
            qualified_column(source.provenance(), &field.column, &raw.tables)?;
            let Some(field_type) = CsvFieldType::parse(&field.kind) else {
                return Err(CsvNoteSourceError::descriptor(
                    source.provenance(),
                    format!(
                        "field mapping {field_id} has unsupported type {:?}; expected scalar or image",
                        field.kind
                    ),
                ));
            };
            if matches!(field_type, CsvFieldType::Image) && field.localized_by.is_some() {
                return Err(CsvNoteSourceError::descriptor(
                    source.provenance(),
                    format!(
                        "image field mapping {field_id} cannot use localized_by; media ID columns must remain unsuffixed"
                    ),
                ));
            }
            validate_localized_by(
                source.provenance(),
                field.localized_by.as_deref(),
                &raw.parameters,
                &format!("field mapping {field_id}"),
            )?;
        }
        qualified_column(source.provenance(), &raw.note.tags.column, &raw.tables)?;
        for (namespace, mapping) in &raw.note.adapter_ids {
            if namespace.is_empty() {
                return Err(CsvNoteSourceError::descriptor(
                    source.provenance(),
                    "adapter namespace must not be empty",
                ));
            }
            qualified_column(source.provenance(), &mapping.column, &raw.tables)?;
            validate_localized_by(
                source.provenance(),
                mapping.localized_by.as_deref(),
                &raw.parameters,
                &format!("adapter-ID mapping {namespace:?}"),
            )?;
        }
        StableId::new(raw.note.note_type_id.clone()).map_err(|error| {
            CsvNoteSourceError::descriptor(source.provenance(), error.to_string())
        })?;
        Ok(Self {
            provenance: source.provenance().clone(),
            primary_table: raw.primary_table,
            tables: raw.tables,
            parameters: raw.parameters,
            joins: raw.joins,
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

    fn parameter_values(
        &self,
        arguments: &BTreeMap<String, String>,
    ) -> Result<BTreeMap<String, String>, CsvNoteSourceError> {
        if let Some(unknown) = arguments
            .keys()
            .find(|name| !self.parameters.contains_key(*name))
        {
            return Err(CsvNoteSourceError::descriptor(
                &self.provenance,
                format!("unknown CSV source parameter argument {unknown:?}"),
            ));
        }
        Ok(self
            .parameters
            .iter()
            .map(|(name, parameter)| {
                (
                    name.clone(),
                    arguments
                        .get(name)
                        .cloned()
                        .unwrap_or_else(|| parameter.default.clone()),
                )
            })
            .collect())
    }
}

fn validate_name(
    provenance: &SourceProvenance,
    kind: &str,
    value: &str,
) -> Result<(), CsvNoteSourceError> {
    if value.is_empty() || value.contains('.') || value.contains(['\n', '\r']) {
        return Err(CsvNoteSourceError::descriptor(
            provenance,
            format!("invalid {kind} {value:?}; expected a non-empty name without '.'"),
        ));
    }
    Ok(())
}

fn qualified_column<'a>(
    provenance: &SourceProvenance,
    value: &'a str,
    tables: &BTreeMap<String, CsvTableDescriptor>,
) -> Result<(&'a str, &'a str), CsvNoteSourceError> {
    let Some((alias, header)) = value.split_once('.') else {
        return Err(CsvNoteSourceError::descriptor(
            provenance,
            format!("column reference {value:?} must be qualified as table.header"),
        ));
    };
    if alias.is_empty() || header.is_empty() {
        return Err(CsvNoteSourceError::descriptor(
            provenance,
            format!("column reference {value:?} has an empty alias or header"),
        ));
    }
    if !tables.contains_key(alias) {
        return Err(CsvNoteSourceError::descriptor(
            provenance,
            format!("column reference {value:?} uses undeclared table alias {alias:?}"),
        ));
    }
    Ok((alias, header))
}

fn validate_localized_by(
    provenance: &SourceProvenance,
    localized_by: Option<&str>,
    parameters: &BTreeMap<String, CsvLocalizedColumnParameter>,
    mapping: &str,
) -> Result<(), CsvNoteSourceError> {
    if let Some(name) = localized_by
        && !parameters.contains_key(name)
    {
        return Err(CsvNoteSourceError::descriptor(
            provenance,
            format!("{mapping} references unknown localized_by parameter {name:?}"),
        ));
    }
    Ok(())
}

/// Materializes one validated descriptor into ordinary core notes.
pub struct CsvNoteSourceMaterializer {
    descriptor: CsvNoteSourceDescriptor,
    parameters: BTreeMap<String, String>,
}

struct LoadedTable<'a> {
    source: &'a CsvSourceFile,
    headers: csv::StringRecord,
    rows: Vec<LoadedRow>,
}

struct LoadedRow {
    logical_row: u64,
    record: csv::StringRecord,
}

struct MappedColumn {
    alias: String,
    index: usize,
    header: String,
}

struct MappedField {
    column: MappedColumn,
    kind: CsvFieldType,
}

struct PreparedJoin {
    alias: String,
    left_index: usize,
    left_header: String,
    required: bool,
    lookup: BTreeMap<String, usize>,
}

impl CsvNoteSourceMaterializer {
    pub fn new(descriptor: CsvNoteSourceDescriptor) -> Self {
        let parameters = descriptor
            .parameter_values(&BTreeMap::new())
            .expect("validated parameter defaults");
        Self {
            descriptor,
            parameters,
        }
    }

    pub fn with_parameters(
        mut self,
        parameters: &BTreeMap<String, String>,
    ) -> Result<Self, CsvNoteSourceError> {
        self.parameters = self.descriptor.parameter_values(parameters)?;
        Ok(self)
    }

    pub fn table_paths(&self) -> impl Iterator<Item = (&str, &str)> {
        self.descriptor.table_paths()
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

        if let Some(alias) = tables
            .keys()
            .find(|alias| !self.descriptor.tables.contains_key(*alias))
        {
            return Err(CsvNoteSourceError::descriptor(
                &self.descriptor.provenance,
                format!("caller provided bytes for undeclared table {alias:?}"),
            ));
        }
        let loaded = self
            .descriptor
            .tables
            .keys()
            .map(|alias| {
                let source = tables.get(alias).ok_or_else(|| {
                    CsvNoteSourceError::descriptor(
                        &self.descriptor.provenance,
                        format!("caller did not provide bytes for table {alias:?}"),
                    )
                })?;
                Ok((alias.clone(), load_table(source)?))
            })
            .collect::<Result<BTreeMap<_, _>, CsvNoteSourceError>>()?;

        let primary = &loaded[&self.descriptor.primary_table];
        let (_, id_header) = qualified_column(
            &self.descriptor.provenance,
            &self.descriptor.note.id,
            &self.descriptor.tables,
        )?;
        let id_index = required_header(primary.source, &primary.headers, id_header)?;
        let field_columns = self
            .descriptor
            .note
            .fields
            .iter()
            .map(|(field_id, mapping)| {
                Ok((
                    StableId::new(field_id.clone()).expect("mapping IDs validated"),
                    MappedField {
                        column: self.mapped_column(
                            &loaded,
                            &mapping.column,
                            mapping.localized_by.as_deref(),
                        )?,
                        kind: CsvFieldType::parse(&mapping.kind).expect("mapping types validated"),
                    },
                ))
            })
            .collect::<Result<BTreeMap<_, _>, CsvNoteSourceError>>()?;
        let tags_column = self.mapped_column(&loaded, &self.descriptor.note.tags.column, None)?;
        let adapter_columns = self
            .descriptor
            .note
            .adapter_ids
            .iter()
            .map(|(namespace, mapping)| {
                Ok((
                    namespace.clone(),
                    self.mapped_column(&loaded, &mapping.column, mapping.localized_by.as_deref())?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>, CsvNoteSourceError>>()?;
        let joins = self.prepare_joins(&loaded)?;

        let mut notes = BTreeMap::new();
        for (primary_index, row) in primary.rows.iter().enumerate() {
            let mut selected =
                BTreeMap::from([(self.descriptor.primary_table.clone(), Some(primary_index))]);
            for join in &joins {
                let key = &row.record[join.left_index];
                if key.is_empty() {
                    return Err(CsvNoteSourceError::cell(
                        primary.source,
                        row.logical_row,
                        join.left_index,
                        &join.left_header,
                        "join left key must not be empty",
                    ));
                }
                let matched = join.lookup.get(key).copied();
                if join.required && matched.is_none() {
                    return Err(CsvNoteSourceError::cell(
                        primary.source,
                        row.logical_row,
                        join.left_index,
                        &join.left_header,
                        format!(
                            "required join to table {:?} has no match for key {key:?}",
                            join.alias
                        ),
                    ));
                }
                selected.insert(join.alias.clone(), matched);
            }

            let id_cell = &row.record[id_index];
            let id = StableId::new(id_cell.to_owned()).map_err(|error| {
                CsvNoteSourceError::cell(
                    primary.source,
                    row.logical_row,
                    id_index,
                    &primary.headers[id_index],
                    error.to_string(),
                )
            })?;
            if notes.contains_key(&id) {
                return Err(CsvNoteSourceError::cell(
                    primary.source,
                    row.logical_row,
                    id_index,
                    &primary.headers[id_index],
                    format!("duplicate stable note ID {id}"),
                ));
            }
            let fields = field_columns
                .iter()
                .map(|(field_id, field)| {
                    let value = cell(&loaded, &selected, &field.column);
                    let value = match field.kind {
                        CsvFieldType::Scalar => FieldValue::Scalar(value.to_owned()),
                        CsvFieldType::Image if value.is_empty() => {
                            FieldValue::Scalar(String::new())
                        }
                        CsvFieldType::Image => {
                            let media_id = StableId::new(value.to_owned()).map_err(|error| {
                                CsvNoteSourceError::cell(
                                    loaded[&field.column.alias].source,
                                    selected_row(&loaded, &selected, &field.column)
                                        .map_or(row.logical_row, |row| row.logical_row),
                                    field.column.index,
                                    &field.column.header,
                                    error.to_string(),
                                )
                            })?;
                            FieldValue::Images(vec![FieldImageReference { media_id }])
                        }
                    };
                    Ok((field_id.clone(), value))
                })
                .collect::<Result<_, CsvNoteSourceError>>()?;
            let tags = parse_tags(
                loaded[&tags_column.alias].source,
                selected_row(&loaded, &selected, &tags_column)
                    .map_or(row.logical_row, |row| row.logical_row),
                tags_column.index,
                &tags_column.header,
                cell(&loaded, &selected, &tags_column),
                &self.descriptor.note.tags.delimiter,
            )?;
            let mut adapter_ids = AdapterIds::new();
            for (namespace, column) in &adapter_columns {
                let value = cell(&loaded, &selected, column);
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

    fn mapped_column(
        &self,
        loaded: &BTreeMap<String, LoadedTable<'_>>,
        qualified: &str,
        localized_by: Option<&str>,
    ) -> Result<MappedColumn, CsvNoteSourceError> {
        let (alias, base_header) = qualified_column(
            &self.descriptor.provenance,
            qualified,
            &self.descriptor.tables,
        )?;
        let header = if let Some(parameter_name) = localized_by {
            let parameter = &self.descriptor.parameters[parameter_name];
            let value = &self.parameters[parameter_name];
            if value.is_empty() {
                base_header.to_owned()
            } else {
                format!("{base_header}{}{value}", parameter.separator)
            }
        } else {
            base_header.to_owned()
        };
        let table = &loaded[alias];
        Ok(MappedColumn {
            alias: alias.to_owned(),
            index: required_header(table.source, &table.headers, &header)?,
            header,
        })
    }

    fn prepare_joins(
        &self,
        loaded: &BTreeMap<String, LoadedTable<'_>>,
    ) -> Result<Vec<PreparedJoin>, CsvNoteSourceError> {
        let primary = &loaded[&self.descriptor.primary_table];
        self.descriptor
            .joins
            .iter()
            .map(|join| {
                let (_, left_header) = qualified_column(
                    &self.descriptor.provenance,
                    &join.left,
                    &self.descriptor.tables,
                )?;
                let (right_alias, right_header) = qualified_column(
                    &self.descriptor.provenance,
                    &join.right,
                    &self.descriptor.tables,
                )?;
                let left_index = required_header(primary.source, &primary.headers, left_header)?;
                let right = &loaded[right_alias];
                let right_index = required_header(right.source, &right.headers, right_header)?;
                let mut lookup = BTreeMap::new();
                for (index, row) in right.rows.iter().enumerate() {
                    let key = &row.record[right_index];
                    if key.is_empty() {
                        return Err(CsvNoteSourceError::cell(
                            right.source,
                            row.logical_row,
                            right_index,
                            right_header,
                            "join right key must not be empty",
                        ));
                    }
                    if lookup.insert(key.to_owned(), index).is_some() {
                        return Err(CsvNoteSourceError::cell(
                            right.source,
                            row.logical_row,
                            right_index,
                            right_header,
                            format!("duplicate join right key {key:?}"),
                        ));
                    }
                }
                Ok(PreparedJoin {
                    alias: right_alias.to_owned(),
                    left_index,
                    left_header: left_header.to_owned(),
                    required: join.required,
                    lookup,
                })
            })
            .collect()
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

fn load_table(source: &CsvSourceFile) -> Result<LoadedTable<'_>, CsvNoteSourceError> {
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
    let rows = reader
        .records()
        .map(|record| {
            let record = record.map_err(|error| csv_error(source, error))?;
            let logical_row = record
                .position()
                .map_or(2, |position| position.record().saturating_add(1));
            Ok(LoadedRow {
                logical_row,
                record,
            })
        })
        .collect::<Result<Vec<_>, CsvNoteSourceError>>()?;
    Ok(LoadedTable {
        source,
        headers,
        rows,
    })
}

fn selected_row<'a>(
    loaded: &'a BTreeMap<String, LoadedTable<'a>>,
    selected: &BTreeMap<String, Option<usize>>,
    column: &MappedColumn,
) -> Option<&'a LoadedRow> {
    selected[&column.alias].map(|index| &loaded[&column.alias].rows[index])
}

fn cell<'a>(
    loaded: &'a BTreeMap<String, LoadedTable<'a>>,
    selected: &BTreeMap<String, Option<usize>>,
    column: &MappedColumn,
) -> &'a str {
    selected_row(loaded, selected, column).map_or("", |row| &row.record[column.index])
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
