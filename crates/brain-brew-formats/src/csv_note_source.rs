//! Strict, filesystem-free CSV note Authoring Source materialization.
//!
//! This module owns the typed `!csv` declaration, explicit flat joins, and
//! literal localized-column parameters. Callers authorize and inject all
//! descriptor and CSV bytes; no path is opened here.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use brain_brew_core::{
    AdapterIds, CanonicalDeck, DeckPath, FieldImageReference, FieldValue, Note, NoteType, Overlay,
    OverlayKind, StableId, TargetAdaptation, TargetAdaptationIntent, TargetAdaptationOwnership,
    TranslationCoverageCategory, TranslationDictionary,
};
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
    pub(crate) fn descriptor(
        referring_source: SourceProvenance,
        schema_path: String,
        target: String,
    ) -> Self {
        Self {
            referring_source,
            schema_path,
            target,
            kind: CsvSourceRequestKind::Descriptor,
        }
    }

    pub(crate) fn table(
        referring_source: SourceProvenance,
        schema_path: String,
        alias: String,
        target: String,
    ) -> Self {
        Self {
            referring_source,
            schema_path,
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

/// Source-preserved declaration stored at a `!csv` notes source boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CsvNoteSourceDeclaration {
    descriptor: String,
    parameters: BTreeMap<String, String>,
    exclude_note_ids: Vec<StableId>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDeclaration {
    descriptor: String,
    parameters: BTreeMap<String, String>,
    #[serde(default)]
    exclude: RawExclusions,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawExclusions {
    #[serde(default)]
    note_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum NoteSourceExpression {
    Csv(CsvNoteSourceDeclaration),
    Sequence(Vec<NoteSourceItem>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum NoteSourceItem {
    Csv(CsvNoteSourceDeclaration),
    Inline { note_ids: Vec<StableId> },
}

impl CsvNoteSourceDeclaration {
    pub fn descriptor(&self) -> &str {
        &self.descriptor
    }

    pub fn parameters(&self) -> &BTreeMap<String, String> {
        &self.parameters
    }

    pub fn excluded_note_ids(&self) -> &[StableId] {
        &self.exclude_note_ids
    }

    fn parse(value: Value, provenance: &SourceProvenance) -> Result<Self, CsvNoteSourceError> {
        let raw: RawDeclaration = serde_yaml::from_value(value).map_err(|error| {
            CsvNoteSourceError::descriptor(
                provenance,
                format!("invalid notes !csv declaration: {error}"),
            )
        })?;
        if raw.descriptor.is_empty() {
            return Err(CsvNoteSourceError::descriptor(
                provenance,
                "notes !csv descriptor path must not be empty",
            ));
        }
        let mut seen = BTreeSet::new();
        let mut exclude_note_ids = Vec::new();
        for value in raw.exclude.note_ids {
            let id = StableId::new(value)
                .map_err(|error| CsvNoteSourceError::descriptor(provenance, error.to_string()))?;
            if !seen.insert(id.clone()) {
                return Err(CsvNoteSourceError::descriptor(
                    provenance,
                    format!("duplicate excluded note ID {id}"),
                ));
            }
            exclude_note_ids.push(id);
        }
        Ok(Self {
            descriptor: raw.descriptor,
            parameters: raw.parameters,
            exclude_note_ids,
        })
    }

    fn emit(&self, item_indent: &str, property_indent: &str) -> Result<String, String> {
        let mut output = format!(
            "{item_indent}!csv\n{property_indent}descriptor: {}\n",
            crate::yaml_scalar::scalar(&self.descriptor)
        );
        if self.parameters.is_empty() {
            output.push_str(&format!("{property_indent}parameters: {{}}\n"));
        } else {
            output.push_str(&format!("{property_indent}parameters:\n"));
            for (name, value) in &self.parameters {
                let name = crate::yaml_scalar::key(name)
                    .ok_or_else(|| format!("parameter name {name:?} cannot be emitted"))?;
                output.push_str(&format!(
                    "{property_indent}  {name}: {}\n",
                    crate::yaml_scalar::scalar(value)
                ));
            }
        }
        if !self.exclude_note_ids.is_empty() {
            output.push_str(&format!("{property_indent}exclude:\n"));
            output.push_str(&format!("{property_indent}  note_ids:\n"));
            for id in &self.exclude_note_ids {
                output.push_str(&format!("{property_indent}    - {id}\n"));
            }
        }
        Ok(output)
    }
}

/// Source-preserved declaration stored at a sparse field-addition values boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CsvSparseFieldSourceDeclaration {
    descriptor: String,
    parameters: BTreeMap<String, String>,
    exclude_note_ids: Vec<StableId>,
}

impl CsvSparseFieldSourceDeclaration {
    pub(crate) fn parse(
        value: Value,
        provenance: &SourceProvenance,
    ) -> Result<Self, CsvNoteSourceError> {
        let raw: RawDeclaration = serde_yaml::from_value(value).map_err(|error| {
            CsvNoteSourceError::descriptor(
                provenance,
                format!("invalid sparse field CSV declaration: {error}"),
            )
        })?;
        if raw.descriptor.is_empty() {
            return Err(CsvNoteSourceError::descriptor(
                provenance,
                "sparse field CSV descriptor path must not be empty",
            ));
        }
        let mut seen = BTreeSet::new();
        let mut exclude_note_ids = Vec::new();
        for value in raw.exclude.note_ids {
            let id = StableId::new(value)
                .map_err(|error| CsvNoteSourceError::descriptor(provenance, error.to_string()))?;
            if !seen.insert(id.clone()) {
                return Err(CsvNoteSourceError::descriptor(
                    provenance,
                    format!("duplicate excluded note ID {id}"),
                ));
            }
            exclude_note_ids.push(id);
        }
        Ok(Self {
            descriptor: raw.descriptor,
            parameters: raw.parameters,
            exclude_note_ids,
        })
    }

    pub fn descriptor(&self) -> &str {
        &self.descriptor
    }

    pub fn parameters(&self) -> &BTreeMap<String, String> {
        &self.parameters
    }

    pub fn excluded_note_ids(&self) -> &[StableId] {
        &self.exclude_note_ids
    }

    pub(crate) fn emit(&self, indent: &str) -> Result<String, String> {
        let mut output = format!(
            "{indent}- descriptor: {}\n",
            crate::yaml_scalar::scalar(&self.descriptor)
        );
        if self.parameters.is_empty() {
            output.push_str(&format!("{indent}  parameters: {{}}\n"));
        } else {
            output.push_str(&format!("{indent}  parameters:\n"));
            for (name, value) in &self.parameters {
                let name = crate::yaml_scalar::key(name)
                    .ok_or_else(|| format!("parameter name {name:?} cannot be emitted"))?;
                output.push_str(&format!(
                    "{indent}    {name}: {}\n",
                    crate::yaml_scalar::scalar(value)
                ));
            }
        }
        if !self.exclude_note_ids.is_empty() {
            output.push_str(&format!("{indent}  exclude:\n"));
            output.push_str(&format!("{indent}    note_ids:\n"));
            for id in &self.exclude_note_ids {
                output.push_str(&format!("{indent}      - {id}\n"));
            }
        }
        Ok(output)
    }
}

/// Strict `translations.from_csv` declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CsvTranslationSourceDeclaration {
    descriptor: String,
    parameters: BTreeMap<String, String>,
    exclude: CsvTranslationExclusions,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct CsvTranslationExclusions {
    source_texts: Vec<String>,
    note_ids: Vec<StableId>,
    paths: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTranslationDeclaration {
    descriptor: String,
    parameters: BTreeMap<String, String>,
    exclude: RawTranslationExclusions,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTranslationExclusions {
    source_texts: Vec<String>,
    note_ids: Vec<String>,
    paths: Vec<String>,
}

impl CsvTranslationSourceDeclaration {
    pub(crate) fn parse(
        value: Value,
        provenance: &SourceProvenance,
    ) -> Result<Self, CsvNoteSourceError> {
        let raw: RawTranslationDeclaration = serde_yaml::from_value(value).map_err(|error| {
            CsvNoteSourceError::descriptor(
                provenance,
                format!("invalid translations.from_csv declaration: {error}"),
            )
        })?;
        if raw.descriptor.is_empty() {
            return Err(CsvNoteSourceError::descriptor(
                provenance,
                "translations.from_csv descriptor path must not be empty",
            ));
        }
        let mut source_texts = Vec::new();
        let mut seen = BTreeSet::new();
        for source_text in raw.exclude.source_texts {
            if source_text.is_empty() {
                return Err(CsvNoteSourceError::descriptor(
                    provenance,
                    "excluded source_text must not be empty",
                ));
            }
            if !seen.insert(source_text.clone()) {
                return Err(CsvNoteSourceError::descriptor(
                    provenance,
                    format!("duplicate excluded source_text {source_text:?}"),
                ));
            }
            source_texts.push(source_text);
        }
        let mut note_ids = Vec::new();
        let mut seen = BTreeSet::new();
        for note_id in raw.exclude.note_ids {
            let note_id = StableId::new(note_id)
                .map_err(|error| CsvNoteSourceError::descriptor(provenance, error.to_string()))?;
            if !seen.insert(note_id.clone()) {
                return Err(CsvNoteSourceError::descriptor(
                    provenance,
                    format!("duplicate excluded note ID {note_id}"),
                ));
            }
            note_ids.push(note_id);
        }
        let mut paths = Vec::new();
        let mut seen = BTreeSet::new();
        for path in raw.exclude.paths {
            let valid = path.parse::<DeckPath>().is_ok_and(|parsed| {
                matches!(
                    &parsed,
                    DeckPath::NoteField { .. } | DeckPath::NoteAdapterId { .. }
                ) && parsed.to_string() == path
            });
            if !valid {
                return Err(CsvNoteSourceError::descriptor(
                    provenance,
                    format!("invalid canonical import occurrence path {path:?}"),
                ));
            }
            if !seen.insert(path.clone()) {
                return Err(CsvNoteSourceError::descriptor(
                    provenance,
                    format!("duplicate excluded path {path:?}"),
                ));
            }
            paths.push(path);
        }
        let exclude = CsvTranslationExclusions {
            source_texts,
            note_ids,
            paths,
        };
        Ok(Self {
            descriptor: raw.descriptor,
            parameters: raw.parameters,
            exclude,
        })
    }

    pub fn descriptor(&self) -> &str {
        &self.descriptor
    }

    pub fn parameters(&self) -> &BTreeMap<String, String> {
        &self.parameters
    }

    pub(crate) fn exclusions(&self) -> (&[String], &[StableId], &[String]) {
        (
            &self.exclude.source_texts,
            &self.exclude.note_ids,
            &self.exclude.paths,
        )
    }

    pub(crate) fn emit(&self, indent: &str) -> Result<String, String> {
        let mut output = format!(
            "{indent}- descriptor: {}\n",
            crate::yaml_scalar::scalar(&self.descriptor)
        );
        if self.parameters.is_empty() {
            output.push_str(&format!("{indent}  parameters: {{}}\n"));
        } else {
            output.push_str(&format!("{indent}  parameters:\n"));
            for (name, value) in &self.parameters {
                let name = crate::yaml_scalar::key(name)
                    .ok_or_else(|| format!("parameter name {name:?} cannot be emitted"))?;
                output.push_str(&format!(
                    "{indent}    {name}: {}\n",
                    crate::yaml_scalar::scalar(value)
                ));
            }
        }
        output.push_str(&format!("{indent}  exclude:\n"));
        emit_string_list(
            &mut output,
            indent,
            "source_texts",
            &self.exclude.source_texts,
        );
        emit_stable_id_list(&mut output, indent, "note_ids", &self.exclude.note_ids);
        emit_string_list(&mut output, indent, "paths", &self.exclude.paths);
        Ok(output)
    }
}

fn emit_string_list(output: &mut String, indent: &str, name: &str, values: &[String]) {
    if values.is_empty() {
        output.push_str(&format!("{indent}    {name}: []\n"));
    } else {
        output.push_str(&format!("{indent}    {name}:\n"));
        for value in values {
            output.push_str(&format!(
                "{indent}      - {}\n",
                crate::yaml_scalar::scalar(value)
            ));
        }
    }
}

fn emit_stable_id_list(output: &mut String, indent: &str, name: &str, values: &[StableId]) {
    if values.is_empty() {
        output.push_str(&format!("{indent}    {name}: []\n"));
    } else {
        output.push_str(&format!("{indent}    {name}:\n"));
        for value in values {
            output.push_str(&format!("{indent}      - {value}\n"));
        }
    }
}

impl NoteSourceExpression {
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
        match notes {
            Value::Tagged(tagged) if tagged.tag == "csv" => {
                let declaration = CsvNoteSourceDeclaration::parse(
                    std::mem::replace(&mut tagged.value, Value::Null),
                    provenance,
                )?;
                *notes = Value::Mapping(serde_yaml::Mapping::new());
                Ok(Some(Self::Csv(declaration)))
            }
            Value::Tagged(tagged) => Err(CsvNoteSourceError::descriptor(
                provenance,
                format!(
                    "unsupported direct notes tag {}; expected !csv or an ordinary note map",
                    tagged.tag
                ),
            )),
            Value::Sequence(sequence) => {
                if sequence.is_empty() {
                    return Err(CsvNoteSourceError::descriptor(
                        provenance,
                        "notes source sequence must not be empty",
                    ));
                }
                let mut combined = serde_yaml::Mapping::new();
                let mut sources = Vec::new();
                for (index, item) in std::mem::take(sequence).into_iter().enumerate() {
                    let Value::Tagged(tagged) = item else {
                        return Err(CsvNoteSourceError::descriptor(
                            provenance,
                            format!(
                                "notes source item {index} must be explicitly tagged !csv or !inline"
                            ),
                        ));
                    };
                    if tagged.tag == "csv" {
                        sources.push(NoteSourceItem::Csv(CsvNoteSourceDeclaration::parse(
                            tagged.value,
                            provenance,
                        )?));
                    } else if tagged.tag == "inline" {
                        let Value::Mapping(inline) = tagged.value else {
                            return Err(CsvNoteSourceError::descriptor(
                                provenance,
                                format!(
                                    "notes !inline source item {index} must contain a note map"
                                ),
                            ));
                        };
                        let mut note_ids = Vec::new();
                        for (key, value) in inline {
                            let Some(id) = key.as_str() else {
                                return Err(CsvNoteSourceError::descriptor(
                                    provenance,
                                    format!(
                                        "notes !inline source item {index} has a non-string note ID"
                                    ),
                                ));
                            };
                            let id = StableId::new(id.to_owned()).map_err(|error| {
                                CsvNoteSourceError::descriptor(provenance, error.to_string())
                            })?;
                            if combined
                                .insert(Value::String(id.to_string()), value)
                                .is_some()
                            {
                                return Err(CsvNoteSourceError::descriptor(
                                    provenance,
                                    format!("duplicate inline ownership of note ID {id}"),
                                ));
                            }
                            note_ids.push(id);
                        }
                        sources.push(NoteSourceItem::Inline { note_ids });
                    } else {
                        return Err(CsvNoteSourceError::descriptor(
                            provenance,
                            format!(
                                "unsupported notes source item tag {}; expected !csv or !inline",
                                tagged.tag
                            ),
                        ));
                    }
                }
                *notes = Value::Mapping(combined);
                Ok(Some(Self::Sequence(sources)))
            }
            Value::Mapping(_) => Ok(None),
            _ => Ok(None),
        }
    }

    pub(crate) fn direct_csv(&self) -> Option<&CsvNoteSourceDeclaration> {
        match self {
            Self::Csv(declaration) => Some(declaration),
            Self::Sequence(_) => None,
        }
    }

    pub(crate) fn restore(
        &self,
        mut canonical: String,
        deck: &CanonicalDeck,
    ) -> Result<String, String> {
        let start = crate::strict_yaml::top_level_mapping_key_offset(&canonical, "notes")
            .ok_or_else(|| "canonical emission omitted the notes section".to_owned())?;
        let relative_end =
            crate::strict_yaml::top_level_mapping_key_offset(&canonical[start..], "media")
                .ok_or_else(|| "canonical emission omitted media after notes".to_owned())?;
        let end = start + relative_end;
        let replacement = match self {
            Self::Csv(declaration) => declaration.emit("notes: ", "  ")?,
            Self::Sequence(sources) => {
                let mut output = String::from("notes:\n");
                for source in sources {
                    match source {
                        NoteSourceItem::Csv(declaration) => {
                            output.push_str(&declaration.emit("  - ", "    ")?);
                        }
                        NoteSourceItem::Inline { note_ids } if note_ids.is_empty() => {
                            output.push_str("  - !inline {}\n");
                        }
                        NoteSourceItem::Inline { note_ids } => {
                            output.push_str("  - !inline\n");
                            let body = canonical_notes_body(deck, note_ids)?;
                            for line in body.lines() {
                                output.push_str("    ");
                                output.push_str(line);
                                output.push('\n');
                            }
                        }
                    }
                }
                output
            }
        };
        canonical.replace_range(start..end, &replacement);
        Ok(canonical)
    }
}

fn canonical_notes_body(deck: &CanonicalDeck, note_ids: &[StableId]) -> Result<String, String> {
    let mut selected = deck.clone();
    selected.notes.clear();
    for id in note_ids {
        let note = deck
            .notes
            .get(id)
            .ok_or_else(|| format!("inline note {id} is absent during source emission"))?;
        selected.notes.insert(id.clone(), note.clone());
    }
    let canonical =
        crate::canonical_yaml::to_string(&selected).map_err(|error| error.to_string())?;
    let start = crate::strict_yaml::top_level_mapping_key_offset(&canonical, "notes")
        .ok_or_else(|| "canonical emission omitted notes".to_owned())?;
    let body_start = canonical[start..]
        .find('\n')
        .map(|offset| start + offset + 1)
        .ok_or_else(|| "canonical notes section has no body".to_owned())?;
    let end = body_start
        + crate::strict_yaml::top_level_mapping_key_offset(&canonical[body_start..], "media")
            .ok_or_else(|| "canonical emission omitted media after notes".to_owned())?;
    Ok(canonical[body_start..end]
        .lines()
        .map(|line| line.strip_prefix("  ").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n"))
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
    #[serde(default)]
    delimiter: Option<String>,
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
            if field.delimiter.as_deref() == Some("") {
                return Err(CsvNoteSourceError::descriptor(
                    source.provenance(),
                    format!("field mapping {field_id} delimiter must not be empty"),
                ));
            }
            if field.delimiter.is_some() && !matches!(field_type, CsvFieldType::Image) {
                return Err(CsvNoteSourceError::descriptor(
                    source.provenance(),
                    format!("scalar field mapping {field_id} cannot declare a delimiter"),
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

    pub(crate) fn note_type_id(&self) -> StableId {
        StableId::new(self.note.note_type_id.clone()).expect("descriptor note type ID validated")
    }

    pub(crate) fn mapped_field_ids(&self) -> impl Iterator<Item = StableId> + '_ {
        self.note
            .fields
            .keys()
            .map(|id| StableId::new(id.clone()).expect("descriptor field IDs validated"))
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
    delimiter: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CsvCellProvenance {
    pub table_alias: String,
    pub source: SourceProvenance,
    pub logical_row: Option<u64>,
    pub header: String,
    pub column: usize,
}

pub(crate) struct CsvNoteMaterialization {
    pub notes: BTreeMap<StableId, Note>,
    pub note_provenance: BTreeMap<StableId, CsvCellProvenance>,
    pub field_provenance: BTreeMap<(StableId, StableId), CsvCellProvenance>,
    pub adapter_provenance: BTreeMap<(StableId, String), CsvCellProvenance>,
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
        Ok(self.materialize_with_provenance(tables, note_types)?.notes)
    }

    pub(crate) fn materialize_with_provenance(
        &self,
        tables: &BTreeMap<String, CsvSourceFile>,
        note_types: &BTreeMap<StableId, NoteType>,
    ) -> Result<CsvNoteMaterialization, CsvNoteSourceError> {
        self.materialize_with_field_completeness(tables, note_types, true)
    }

    fn materialize_field_subset_with_provenance(
        &self,
        tables: &BTreeMap<String, CsvSourceFile>,
        note_types: &BTreeMap<StableId, NoteType>,
    ) -> Result<CsvNoteMaterialization, CsvNoteSourceError> {
        self.materialize_with_field_completeness(tables, note_types, false)
    }

    fn materialize_with_field_completeness(
        &self,
        tables: &BTreeMap<String, CsvSourceFile>,
        note_types: &BTreeMap<StableId, NoteType>,
        require_complete_fields: bool,
    ) -> Result<CsvNoteMaterialization, CsvNoteSourceError> {
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
        self.validate_field_completeness(note_type, require_complete_fields)?;

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
                        delimiter: mapping.delimiter.clone(),
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
        let mut note_provenance = BTreeMap::new();
        let mut field_provenance = BTreeMap::new();
        let mut adapter_provenance = BTreeMap::new();
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
            let mut fields = BTreeMap::new();
            for (field_id, field) in &field_columns {
                let selected_field_row = selected_row(&loaded, &selected, &field.column);
                let value = cell(&loaded, &selected, &field.column);
                let value = match field.kind {
                    CsvFieldType::Scalar => FieldValue::Scalar(value.to_owned()),
                    CsvFieldType::Image if value.is_empty() => FieldValue::Scalar(String::new()),
                    CsvFieldType::Image => {
                        let images = field
                            .delimiter
                            .as_deref()
                            .map_or_else(
                                || vec![value],
                                |delimiter| value.split(delimiter).collect(),
                            )
                            .into_iter()
                            .map(|value| {
                                let media_id =
                                    StableId::new(value.to_owned()).map_err(|error| {
                                        CsvNoteSourceError::cell(
                                            loaded[&field.column.alias].source,
                                            selected_field_row
                                                .map_or(row.logical_row, |row| row.logical_row),
                                            field.column.index,
                                            &field.column.header,
                                            error.to_string(),
                                        )
                                    })?;
                                Ok(FieldImageReference { media_id })
                            })
                            .collect::<Result<Vec<_>, CsvNoteSourceError>>()?;
                        FieldValue::Images(images)
                    }
                };
                fields.insert(field_id.clone(), value);
                field_provenance.insert(
                    (id.clone(), field_id.clone()),
                    CsvCellProvenance {
                        table_alias: field.column.alias.clone(),
                        source: loaded[&field.column.alias].source.provenance().clone(),
                        logical_row: selected_field_row.map(|row| row.logical_row),
                        header: field.column.header.clone(),
                        column: field.column.index + 1,
                    },
                );
            }
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
                let selected_adapter_row = selected_row(&loaded, &selected, column);
                let value = cell(&loaded, &selected, column);
                if !value.is_empty() {
                    adapter_ids.insert(namespace, value);
                }
                adapter_provenance.insert(
                    (id.clone(), namespace.clone()),
                    CsvCellProvenance {
                        table_alias: column.alias.clone(),
                        source: loaded[&column.alias].source.provenance().clone(),
                        logical_row: selected_adapter_row.map(|row| row.logical_row),
                        header: column.header.clone(),
                        column: column.index + 1,
                    },
                );
            }
            note_provenance.insert(
                id.clone(),
                CsvCellProvenance {
                    table_alias: self.descriptor.primary_table.clone(),
                    source: primary.source.provenance().clone(),
                    logical_row: Some(row.logical_row),
                    header: primary.headers[id_index].to_owned(),
                    column: id_index + 1,
                },
            );
            notes.insert(
                id.clone(),
                Note {
                    id,
                    note_type_id: note_type_id.clone(),
                    variables: BTreeMap::new(),
                    fields: fields.into(),
                    tags,
                    adapter_ids,
                },
            );
        }
        Ok(CsvNoteMaterialization {
            notes,
            note_provenance,
            field_provenance,
            adapter_provenance,
        })
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

    fn validate_field_completeness(
        &self,
        note_type: &NoteType,
        require_complete_fields: bool,
    ) -> Result<(), CsvNoteSourceError> {
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
        if require_complete_fields && let Some(missing) = declared.difference(&mapped).next() {
            return Err(CsvNoteSourceError::descriptor(
                &self.descriptor.provenance,
                format!("missing field mapping {missing}"),
            ));
        }
        Ok(())
    }
}

pub(crate) const CSV_LEGACY_IMPORT_REASON: &str = "imported from a legacy CSV translation source; move ownership to native YAML for historical stale detection";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CsvTranslationAuthoringLocation {
    file: SourceProvenance,
    logical_row: Option<u64>,
    header: String,
    column: usize,
    canonical_path: String,
}

impl CsvTranslationAuthoringLocation {
    pub fn file(&self) -> &SourceProvenance {
        &self.file
    }

    pub fn logical_row(&self) -> Option<u64> {
        self.logical_row
    }

    pub fn header(&self) -> &str {
        &self.header
    }

    pub fn column(&self) -> usize {
        self.column
    }

    pub fn canonical_path(&self) -> &str {
        &self.canonical_path
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CsvTranslationAuthoringCategory {
    Direct,
    Contextual,
    NoChange,
    Adaptation,
    Deletion,
    AdapterId,
}

impl CsvTranslationAuthoringCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Contextual => "contextual",
            Self::NoChange => "no_change",
            Self::Adaptation => "adaptation",
            Self::Deletion => "deletion",
            Self::AdapterId => "adapter_id",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CsvTranslationAuthoringUnit {
    declaration: String,
    descriptor: SourceProvenance,
    location: CsvTranslationAuthoringLocation,
    category: CsvTranslationAuthoringCategory,
    source: String,
    target: String,
}

impl CsvTranslationAuthoringUnit {
    pub fn declaration(&self) -> &str {
        &self.declaration
    }

    pub fn descriptor(&self) -> &SourceProvenance {
        &self.descriptor
    }

    pub fn file(&self) -> &SourceProvenance {
        self.location.file()
    }

    pub fn logical_row(&self) -> Option<u64> {
        self.location.logical_row()
    }

    pub fn header(&self) -> &str {
        self.location.header()
    }

    pub fn column(&self) -> usize {
        self.location.column()
    }

    pub fn canonical_path(&self) -> &str {
        self.location.canonical_path()
    }

    pub fn category(&self) -> CsvTranslationAuthoringCategory {
        self.category
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn target(&self) -> &str {
        &self.target
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CsvTranslationAuthoringProvenance {
    adaptations: BTreeMap<String, CsvTranslationAuthoringLocation>,
    units: Vec<CsvTranslationAuthoringUnit>,
}

impl CsvTranslationAuthoringProvenance {
    pub fn adaptation(&self, path: &str) -> Option<&CsvTranslationAuthoringLocation> {
        self.adaptations.get(path)
    }

    pub fn adaptations(&self) -> impl Iterator<Item = (&str, &CsvTranslationAuthoringLocation)> {
        self.adaptations
            .iter()
            .map(|(path, location)| (path.as_str(), location))
    }

    pub fn units(&self) -> impl Iterator<Item = &CsvTranslationAuthoringUnit> {
        self.units.iter()
    }

    pub(crate) fn merge(&mut self, other: Self) {
        self.adaptations.extend(other.adaptations);
        self.units.extend(other.units);
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CsvTranslationPair {
    pub note_id: StableId,
    pub path: String,
    pub source: String,
    pub target: String,
    location: CsvTranslationAuthoringLocation,
}

#[derive(Clone, Debug)]
pub(crate) struct CsvAdapterTranslationPair {
    pub note_id: StableId,
    pub namespace: String,
    pub path: String,
    pub source: String,
    pub target: String,
    location: CsvTranslationAuthoringLocation,
}

#[derive(Clone, Debug)]
pub(crate) enum ExcludedCsvTranslationOccurrence {
    Text(CsvTranslationPair),
    Adaptation {
        path: String,
        adaptation: TargetAdaptation,
    },
    Adapter(CsvAdapterTranslationPair),
}

pub(crate) struct CsvTranslationMaterialization {
    pub translations: TranslationDictionary,
    pub provenance: CsvTranslationAuthoringProvenance,
    pub owned_text: Vec<CsvTranslationPair>,
    owned_adapters: Vec<CsvAdapterTranslationPair>,
    global_paths: BTreeMap<String, BTreeSet<String>>,
    descriptor: SourceProvenance,
}

impl CsvTranslationMaterialization {
    pub(crate) fn owned_paths(&self) -> impl Iterator<Item = &str> {
        self.owned_text
            .iter()
            .map(|pair| pair.path.as_str())
            .chain(
                self.translations
                    .target_adaptations
                    .keys()
                    .map(String::as_str),
            )
            .chain(self.owned_adapters.iter().map(|pair| pair.path.as_str()))
    }

    pub(crate) fn apply_exclusions(
        &mut self,
        declaration: &CsvTranslationSourceDeclaration,
        declaration_path: &str,
    ) -> Result<Vec<ExcludedCsvTranslationOccurrence>, String> {
        let (source_texts, note_ids, paths) = declaration.exclusions();
        let mut matched_sources = vec![false; source_texts.len()];
        let mut matched_notes = vec![false; note_ids.len()];
        let mut matched_paths = vec![false; paths.len()];
        let mut excluded_sources = BTreeSet::new();
        let mut excluded = Vec::new();

        for pair in &self.owned_text {
            let matches = mark_exclusion_matches(
                &pair.source,
                &pair.note_id,
                &pair.path,
                source_texts,
                note_ids,
                paths,
                &mut matched_sources,
                &mut matched_notes,
                &mut matched_paths,
            );
            if matches {
                excluded_sources.insert(pair.source.clone());
                excluded.push(ExcludedCsvTranslationOccurrence::Text(pair.clone()));
            }
        }
        for (path, adaptation) in &self.translations.target_adaptations {
            let DeckPath::NoteField { note_id, .. } = path
                .parse::<DeckPath>()
                .expect("materialized field path is canonical")
            else {
                unreachable!("CSV adaptations are note fields")
            };
            let matches = mark_exclusion_matches(
                &adaptation.expected_source,
                &note_id,
                path,
                source_texts,
                note_ids,
                paths,
                &mut matched_sources,
                &mut matched_notes,
                &mut matched_paths,
            );
            if matches {
                if !adaptation.expected_source.is_empty() {
                    excluded_sources.insert(adaptation.expected_source.clone());
                }
                excluded.push(ExcludedCsvTranslationOccurrence::Adaptation {
                    path: path.clone(),
                    adaptation: adaptation.clone(),
                });
            }
        }
        for pair in &self.owned_adapters {
            let matches = mark_exclusion_matches(
                "",
                &pair.note_id,
                &pair.path,
                &[],
                note_ids,
                paths,
                &mut [],
                &mut matched_notes,
                &mut matched_paths,
            );
            if matches {
                excluded.push(ExcludedCsvTranslationOccurrence::Adapter(pair.clone()));
            }
        }

        for (index, matched) in matched_sources.into_iter().enumerate() {
            if !matched {
                return Err(format!(
                    "excluded source_text {:?} matched no otherwise-importable occurrence",
                    source_texts[index]
                ));
            }
        }
        for (index, matched) in matched_notes.into_iter().enumerate() {
            if !matched {
                return Err(format!(
                    "excluded note ID {} matched no otherwise-importable occurrence",
                    note_ids[index]
                ));
            }
        }
        for (index, matched) in matched_paths.into_iter().enumerate() {
            if !matched {
                return Err(format!(
                    "excluded path {:?} matched no otherwise-importable occurrence",
                    paths[index]
                ));
            }
        }

        let excluded_paths = excluded
            .iter()
            .map(|occurrence| match occurrence {
                ExcludedCsvTranslationOccurrence::Text(pair) => pair.path.as_str(),
                ExcludedCsvTranslationOccurrence::Adaptation { path, .. } => path.as_str(),
                ExcludedCsvTranslationOccurrence::Adapter(pair) => pair.path.as_str(),
            })
            .collect::<BTreeSet<_>>();
        self.owned_text
            .retain(|pair| !excluded_paths.contains(pair.path.as_str()));
        self.owned_adapters
            .retain(|pair| !excluded_paths.contains(pair.path.as_str()));
        self.translations
            .target_adaptations
            .retain(|path, _| !excluded_paths.contains(path.as_str()));
        self.provenance
            .adaptations
            .retain(|path, _| !excluded_paths.contains(path.as_str()));

        self.translations.direct.clear();
        self.translations.contextual.clear();
        self.translations.no_change.clear();
        self.translations.adapter_ids.clear();
        infer_csv_text_translations(
            &mut self.translations,
            &self.owned_text,
            &self.global_paths,
            &excluded_sources,
        );
        for pair in &self.owned_adapters {
            self.translations
                .adapter_ids
                .entry(pair.namespace.clone())
                .or_default()
                .insert(pair.source.clone(), pair.target.clone());
        }

        self.provenance.units.clear();
        for pair in &self.owned_text {
            let category = if self.translations.direct.contains_key(&pair.source) {
                CsvTranslationAuthoringCategory::Direct
            } else if self.translations.no_change.contains(&pair.source) {
                CsvTranslationAuthoringCategory::NoChange
            } else {
                CsvTranslationAuthoringCategory::Contextual
            };
            self.provenance.units.push(CsvTranslationAuthoringUnit {
                declaration: declaration_path.to_owned(),
                descriptor: self.descriptor.clone(),
                location: pair.location.clone(),
                category,
                source: pair.source.clone(),
                target: pair.target.clone(),
            });
        }
        for (path, adaptation) in &self.translations.target_adaptations {
            self.provenance.units.push(CsvTranslationAuthoringUnit {
                declaration: declaration_path.to_owned(),
                descriptor: self.descriptor.clone(),
                location: self.provenance.adaptations[path].clone(),
                category: if adaptation.intent == TargetAdaptationIntent::Delete {
                    CsvTranslationAuthoringCategory::Deletion
                } else {
                    CsvTranslationAuthoringCategory::Adaptation
                },
                source: adaptation.expected_source.clone(),
                target: adaptation.target.clone(),
            });
        }
        for pair in &self.owned_adapters {
            self.provenance.units.push(CsvTranslationAuthoringUnit {
                declaration: declaration_path.to_owned(),
                descriptor: self.descriptor.clone(),
                location: pair.location.clone(),
                category: CsvTranslationAuthoringCategory::AdapterId,
                source: pair.source.clone(),
                target: pair.target.clone(),
            });
        }
        self.provenance.units.sort_by(|left, right| {
            left.canonical_path()
                .cmp(right.canonical_path())
                .then_with(|| left.category.as_str().cmp(right.category.as_str()))
                .then_with(|| left.source.cmp(&right.source))
        });
        Ok(excluded)
    }
}

#[allow(clippy::too_many_arguments)]
fn mark_exclusion_matches(
    source: &str,
    note_id: &StableId,
    path: &str,
    source_texts: &[String],
    note_ids: &[StableId],
    paths: &[String],
    matched_sources: &mut [bool],
    matched_notes: &mut [bool],
    matched_paths: &mut [bool],
) -> bool {
    let mut matched = false;
    for (index, selector) in source_texts.iter().enumerate() {
        if selector == source {
            matched_sources[index] = true;
            matched = true;
        }
    }
    for (index, selector) in note_ids.iter().enumerate() {
        if selector == note_id {
            matched_notes[index] = true;
            matched = true;
        }
    }
    for (index, selector) in paths.iter().enumerate() {
        if selector == path {
            matched_paths[index] = true;
            matched = true;
        }
    }
    matched
}

/// Pairs the unsuffixed and explicitly localized views of one note descriptor.
pub(crate) struct CsvTranslationSourceMaterializer {
    descriptor: CsvNoteSourceDescriptor,
    parameters: BTreeMap<String, String>,
}

impl CsvTranslationSourceMaterializer {
    pub(crate) fn new(
        descriptor: CsvNoteSourceDescriptor,
        parameters: &BTreeMap<String, String>,
    ) -> Result<Self, CsvNoteSourceError> {
        let parameters = descriptor.parameter_values(parameters)?;
        let localized_parameters = descriptor
            .note
            .fields
            .values()
            .filter_map(|mapping| mapping.localized_by.as_ref())
            .chain(
                descriptor
                    .note
                    .adapter_ids
                    .values()
                    .filter_map(|mapping| mapping.localized_by.as_ref()),
            )
            .collect::<BTreeSet<_>>();
        if localized_parameters.is_empty() {
            return Err(CsvNoteSourceError::descriptor(
                &descriptor.provenance,
                "CSV translation descriptor has no localized scalar field or adapter-ID mapping",
            ));
        }
        for parameter in localized_parameters {
            if parameters[parameter].is_empty() {
                return Err(CsvNoteSourceError::descriptor(
                    &descriptor.provenance,
                    format!(
                        "CSV translation localized_column parameter {parameter:?} must be non-empty; source-to-itself pairing is forbidden"
                    ),
                ));
            }
        }
        Ok(Self {
            descriptor,
            parameters,
        })
    }

    pub(crate) fn table_paths(&self) -> impl Iterator<Item = (&str, &str)> {
        self.descriptor.table_paths()
    }

    pub(crate) fn materialize(
        &self,
        tables: &BTreeMap<String, CsvSourceFile>,
        source_deck: &CanonicalDeck,
    ) -> Result<CsvTranslationMaterialization, CsvNoteSourceError> {
        let mut source_parameters = self.parameters.clone();
        for mapping in self.descriptor.note.fields.values() {
            if let Some(parameter) = &mapping.localized_by {
                source_parameters.insert(parameter.clone(), String::new());
            }
        }
        for mapping in self.descriptor.note.adapter_ids.values() {
            if let Some(parameter) = &mapping.localized_by {
                source_parameters.insert(parameter.clone(), String::new());
            }
        }

        let source = CsvNoteSourceMaterializer::new(self.descriptor.clone())
            .with_parameters(&source_parameters)?
            .materialize_field_subset_with_provenance(tables, &source_deck.note_types)?;
        let target = CsvNoteSourceMaterializer::new(self.descriptor.clone())
            .with_parameters(&self.parameters)?
            .materialize_field_subset_with_provenance(tables, &source_deck.note_types)?;

        let mut owned_text = Vec::new();
        let mut provenance = CsvTranslationAuthoringProvenance::default();
        let mut translations = TranslationDictionary::default();
        for (field_id, mapping) in &self.descriptor.note.fields {
            if mapping.localized_by.is_none() {
                continue;
            }
            if !matches!(
                CsvFieldType::parse(&mapping.kind),
                Some(CsvFieldType::Scalar)
            ) {
                return Err(CsvNoteSourceError::descriptor(
                    &self.descriptor.provenance,
                    format!("localized translation field mapping {field_id} must be scalar"),
                ));
            }
            let field_id = StableId::new(field_id.clone()).expect("descriptor field IDs validated");
            for (note_id, source_note) in &source.notes {
                let path = format!("notes.{note_id}.fields.{field_id}");
                let source_value = source_note.fields[&field_id]
                    .as_scalar()
                    .expect("localized scalar mapping materialized as scalar");
                let target_value = target.notes[note_id].fields[&field_id]
                    .as_scalar()
                    .expect("localized scalar mapping materialized as scalar");
                let live_value = source_deck
                    .notes
                    .get(note_id)
                    .and_then(|note| note.fields.get(&field_id))
                    .and_then(FieldValue::as_scalar)
                    .ok_or_else(|| {
                        CsvNoteSourceError::descriptor(
                            &self.descriptor.provenance,
                            format!("CSV translation path {path} is absent or is not a scalar field in the resolved source deck"),
                        )
                    })?;
                if live_value != source_value {
                    return Err(CsvNoteSourceError::descriptor(
                        &self.descriptor.provenance,
                        format!(
                            "CSV translation source mismatch at {path}: descriptor cell is {source_value:?}, resolved source is {live_value:?}"
                        ),
                    ));
                }
                if source_value.is_empty() && target_value.is_empty() {
                    continue;
                }
                if source_value.is_empty() || target_value.is_empty() {
                    let intent = if source_value.is_empty() {
                        TargetAdaptationIntent::Adapt
                    } else {
                        TargetAdaptationIntent::Delete
                    };
                    translations.target_adaptations.insert(
                        path.clone(),
                        TargetAdaptation {
                            intent,
                            ownership: TargetAdaptationOwnership::Translation,
                            expected_source: source_value.to_owned(),
                            target: target_value.to_owned(),
                            reason: CSV_LEGACY_IMPORT_REASON.to_owned(),
                        },
                    );
                    let cell = &target.field_provenance[&(note_id.clone(), field_id.clone())];
                    provenance.adaptations.insert(
                        path.clone(),
                        CsvTranslationAuthoringLocation {
                            file: cell.source.clone(),
                            logical_row: cell.logical_row,
                            header: cell.header.clone(),
                            column: cell.column,
                            canonical_path: path,
                        },
                    );
                    continue;
                }
                let cell = &target.field_provenance[&(note_id.clone(), field_id.clone())];
                owned_text.push(CsvTranslationPair {
                    note_id: note_id.clone(),
                    path: path.clone(),
                    source: source_value.to_owned(),
                    target: target_value.to_owned(),
                    location: CsvTranslationAuthoringLocation {
                        file: cell.source.clone(),
                        logical_row: cell.logical_row,
                        header: cell.header.clone(),
                        column: cell.column,
                        canonical_path: path,
                    },
                });
            }
        }

        let coverage_overlay = Overlay {
            id: StableId::new("overlay.csv-translation-occurrences")
                .expect("static overlay ID is valid"),
            kind: OverlayKind::Translation,
            translations: Some(TranslationDictionary::default()),
            deck_change: None,
            note_changes: BTreeMap::new(),
            note_type_changes: BTreeMap::new(),
            media_changes: BTreeMap::new(),
        };
        let coverage = source_deck
            .translation_coverage(&coverage_overlay)
            .map_err(|error| {
                CsvNoteSourceError::descriptor(
                    &self.descriptor.provenance,
                    format!("could not enumerate resolved source translation occurrences: {error}"),
                )
            })?;
        let mut global_paths = BTreeMap::<String, BTreeSet<String>>::new();
        for entry in coverage.entries {
            if entry.category == TranslationCoverageCategory::UntranslatedFallback {
                global_paths
                    .entry(entry.source)
                    .or_default()
                    .insert(entry.path);
            }
        }
        infer_csv_text_translations(
            &mut translations,
            &owned_text,
            &global_paths,
            &BTreeSet::new(),
        );

        let mut owned_adapters = Vec::new();
        for (namespace, mapping) in &self.descriptor.note.adapter_ids {
            if mapping.localized_by.is_none() {
                continue;
            }
            let mut replacements = BTreeMap::<String, String>::new();
            for (note_id, source_note) in &source.notes {
                let source_id = source_note.adapter_ids.get(namespace).unwrap_or("");
                let target_id = target.notes[note_id]
                    .adapter_ids
                    .get(namespace)
                    .unwrap_or("");
                let live_id = source_deck
                    .notes
                    .get(note_id)
                    .and_then(|note| note.adapter_ids.get(namespace))
                    .unwrap_or("");
                if live_id != source_id {
                    return Err(CsvNoteSourceError::descriptor(
                        &self.descriptor.provenance,
                        format!(
                            "CSV translation adapter ID source mismatch at notes.{note_id}.adapter_ids.{namespace}: descriptor cell is {source_id:?}, resolved source is {live_id:?}"
                        ),
                    ));
                }
                if source_id.is_empty() && target_id.is_empty() || source_id == target_id {
                    continue;
                }
                if source_id.is_empty() || target_id.is_empty() {
                    let cell = if target_id.is_empty() {
                        &target.adapter_provenance[&(note_id.clone(), namespace.clone())]
                    } else {
                        &source.adapter_provenance[&(note_id.clone(), namespace.clone())]
                    };
                    return Err(CsvNoteSourceError::new(
                        cell.source.clone(),
                        cell.logical_row,
                        Some(cell.column),
                        Some(cell.header.clone()),
                        format!(
                            "adapter ID pair for namespace {namespace:?} has exactly one blank cell; source={source_id:?}, target={target_id:?}"
                        ),
                    ));
                }
                if let Some(previous) =
                    replacements.insert(source_id.to_owned(), target_id.to_owned())
                    && previous != target_id
                {
                    let cell = &target.adapter_provenance[&(note_id.clone(), namespace.clone())];
                    return Err(CsvNoteSourceError::new(
                        cell.source.clone(),
                        cell.logical_row,
                        Some(cell.column),
                        Some(cell.header.clone()),
                        format!(
                            "conflicting adapter ID translations for namespace {namespace:?} and source {source_id:?}: {previous:?} and {target_id:?}"
                        ),
                    ));
                }
                let path = format!("notes.{note_id}.adapter_ids.{namespace}");
                let cell = &target.adapter_provenance[&(note_id.clone(), namespace.clone())];
                owned_adapters.push(CsvAdapterTranslationPair {
                    note_id: note_id.clone(),
                    namespace: namespace.clone(),
                    path: path.clone(),
                    source: source_id.to_owned(),
                    target: target_id.to_owned(),
                    location: CsvTranslationAuthoringLocation {
                        file: cell.source.clone(),
                        logical_row: cell.logical_row,
                        header: cell.header.clone(),
                        column: cell.column,
                        canonical_path: path,
                    },
                });
            }
            if !replacements.is_empty() {
                translations
                    .adapter_ids
                    .insert(namespace.clone(), replacements);
            }
        }
        translations
            .validate_mutation_invariants()
            .map_err(|error| {
                CsvNoteSourceError::descriptor(&self.descriptor.provenance, error.to_string())
            })?;
        Ok(CsvTranslationMaterialization {
            translations,
            provenance,
            owned_text,
            owned_adapters,
            global_paths,
            descriptor: self.descriptor.provenance.clone(),
        })
    }
}

fn infer_csv_text_translations(
    translations: &mut TranslationDictionary,
    owned_text: &[CsvTranslationPair],
    global_paths: &BTreeMap<String, BTreeSet<String>>,
    force_contextual: &BTreeSet<String>,
) {
    let mut pairs_by_source = BTreeMap::<String, Vec<&CsvTranslationPair>>::new();
    for pair in owned_text {
        pairs_by_source
            .entry(pair.source.clone())
            .or_default()
            .push(pair);
    }
    for (source_text, pairs) in pairs_by_source {
        let owned_paths = pairs
            .iter()
            .map(|pair| pair.path.clone())
            .collect::<BTreeSet<_>>();
        let targets = pairs
            .iter()
            .map(|pair| pair.target.as_str())
            .collect::<BTreeSet<_>>();
        let globally_covered = global_paths.get(&source_text) == Some(&owned_paths);
        if !force_contextual.contains(&source_text) && globally_covered && targets.len() == 1 {
            let target_text = *targets.first().expect("one target exists");
            if target_text == source_text {
                translations.no_change.insert(source_text);
            } else {
                translations
                    .direct
                    .insert(source_text, target_text.to_owned());
            }
        } else {
            for pair in pairs {
                translations
                    .contextual
                    .entry(pair.path.clone())
                    .or_default()
                    .insert(pair.source.clone(), pair.target.clone());
            }
        }
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
