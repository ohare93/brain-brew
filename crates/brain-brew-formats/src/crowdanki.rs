use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::media;
use brain_brew_core::{
    AdapterIds, CanonicalDeck, CardTemplate, FieldDefinition, FieldImageReference, FieldValue,
    MediaReference, Note, NoteType, StableId, TombstoneAddress, Tombstones, ValidationReport,
    VariableRenderReport,
};
use serde::{Deserialize, Serialize};

/// Normalized CrowdAnki export artifacts and adapter report data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrowdAnkiExport {
    pub deck_json: String,
    pub omitted_tombstones: Vec<TombstoneAddress>,
}

/// Export a CanonicalDeck to deterministic normalized CrowdAnki `deck.json` bytes.
pub fn export_deck(deck: &CanonicalDeck) -> Result<CrowdAnkiExport, CrowdAnkiError> {
    deck.validate().map_err(CrowdAnkiError::Validation)?;
    media::validate_paths(deck).map_err(CrowdAnkiError::Media)?;
    let rendered_deck = deck
        .render_variables()
        .map_err(CrowdAnkiError::VariableRender)?;
    rendered_deck
        .validate()
        .map_err(CrowdAnkiError::Validation)?;
    let deck = &rendered_deck;

    let note_models = deck
        .note_types
        .values()
        .filter(|note_type| {
            deck.tombstones
                .blocking(&TombstoneAddress::NoteType {
                    note_type_id: note_type.id.clone(),
                })
                .is_none()
        })
        .map(|note_type| export_note_model(note_type, deck))
        .collect::<Result<Vec<_>, _>>()?;

    let note_type_uuids = deck
        .note_types
        .iter()
        .filter(|(id, _)| {
            deck.tombstones
                .blocking(&TombstoneAddress::NoteType {
                    note_type_id: (*id).clone(),
                })
                .is_none()
        })
        .map(|(id, note_type)| Ok((id.clone(), crowdanki_note_model_uuid(note_type)?)))
        .collect::<Result<BTreeMap<_, _>, CrowdAnkiError>>()?;

    let mut omitted_tombstones = Vec::new();
    let mut notes = Vec::new();
    for (id, note) in &deck.notes {
        let address = TombstoneAddress::Note {
            note_id: id.clone(),
        };
        if deck.tombstones.blocking(&address).is_some() {
            omitted_tombstones.push(address);
            continue;
        }
        notes.push(export_note(note, deck, &note_type_uuids)?);
    }

    let deck_config_uuid = crowdanki_deck_config_uuid(deck);
    let deck_json = CrowdAnkiDeckJson {
        type_: "Deck".to_owned(),
        children: Vec::new(),
        crowdanki_uuid: crowdanki_deck_uuid(deck),
        deck_config_uuid: deck_config_uuid.clone(),
        deck_configurations: vec![default_deck_config_json(
            &deck_config_uuid,
            &crowdanki_deck_config_name(deck),
        )],
        desc: deck.description.clone(),
        dyn_: 0,
        extend_new: 10,
        extend_rev: 50,
        media_files: deck
            .media
            .values()
            .filter(|media| {
                deck.tombstones
                    .blocking(&TombstoneAddress::MediaReference {
                        media_id: media.id.clone(),
                    })
                    .is_none()
            })
            .map(|media| media.path.clone())
            .collect::<Vec<_>>(),
        name: deck.name.clone(),
        note_models,
        notes,
    };

    let mut serialized = serde_json::to_string_pretty(&deck_json).map_err(CrowdAnkiError::Json)?;
    serialized.push('\n');

    Ok(CrowdAnkiExport {
        deck_json: serialized,
        omitted_tombstones,
    })
}

/// Import normalized CrowdAnki `deck.json`, accepting generated stable IDs.
pub fn import_deck_accept_suggested_ids(input: &str) -> Result<CanonicalDeck, CrowdAnkiError> {
    let mut deserializer = serde_json::Deserializer::from_str(input);
    let deck_json: CrowdAnkiDeckJson = serde_path_to_error::deserialize(&mut deserializer)
        .map_err(|error| CrowdAnkiError::JsonPath {
            path: json_path(error.path()),
            message: error.inner().to_string(),
        })?;
    deck_json.into_deck()
}

/// Named canonical equivalence profile for a CrowdAnki export/import round trip.
///
/// Exact canonical diff is never weakened. Callers explicitly project both sides with this
/// profile, then use [`CanonicalDeck::semantic_diff`] as the exact oracle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CrowdAnkiRoundTripProfile {
    pub name: &'static str,
    pub losses: &'static [CrowdAnkiRoundTripLoss],
}

/// Canonical information CrowdAnki cannot preserve through `deck.json`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CrowdAnkiRoundTripLoss {
    SourceVariablesAreRendered,
    StructuredFieldRepresentationsAreLowered,
    MediaHashesAreNotStored,
    TypedTombstonesBecomePhysicalOmissions,
    UnsupportedAdapterIdsAreDiscarded,
    StableIdsAreRegeneratedFromAdapterContent,
}

pub const CROWDANKI_ROUND_TRIP_PROFILE: CrowdAnkiRoundTripProfile = CrowdAnkiRoundTripProfile {
    name: "crowdanki-export-import-v1",
    losses: &[
        CrowdAnkiRoundTripLoss::SourceVariablesAreRendered,
        CrowdAnkiRoundTripLoss::StructuredFieldRepresentationsAreLowered,
        CrowdAnkiRoundTripLoss::MediaHashesAreNotStored,
        CrowdAnkiRoundTripLoss::TypedTombstonesBecomePhysicalOmissions,
        CrowdAnkiRoundTripLoss::UnsupportedAdapterIdsAreDiscarded,
        CrowdAnkiRoundTripLoss::StableIdsAreRegeneratedFromAdapterContent,
    ],
};

/// Project a canonical deck to the exact semantics representable by a CrowdAnki round trip.
///
/// Stable IDs are normalized with the import suggestion algorithm because `deck.json` does not
/// store canonical identity. Colliding suggestions fail explicitly instead of being equated.
/// Adapter-visible fallback UUIDs/GUIDs are materialized before this normalization.
pub fn project_deck_for_crowdanki_round_trip(
    deck: &CanonicalDeck,
) -> Result<CanonicalDeck, CrowdAnkiError> {
    deck.validate().map_err(CrowdAnkiError::Validation)?;
    let mut projected = deck
        .render_variables()
        .map_err(CrowdAnkiError::VariableRender)?;
    let tombstones = projected.tombstones.clone();

    projected.note_types.retain(|note_type_id, _| {
        tombstones
            .blocking(&TombstoneAddress::NoteType {
                note_type_id: note_type_id.clone(),
            })
            .is_none()
    });
    for (note_type_id, note_type) in &mut projected.note_types {
        note_type.fields.retain(|field| {
            tombstones
                .blocking(&TombstoneAddress::FieldDefinition {
                    note_type_id: note_type_id.clone(),
                    field_id: field.id.clone(),
                })
                .is_none()
        });
        note_type.card_templates.retain(|template| {
            tombstones
                .blocking(&TombstoneAddress::CardTemplate {
                    note_type_id: note_type_id.clone(),
                    template_id: template.id.clone(),
                })
                .is_none()
        });
    }
    projected.notes.retain(|note_id, _| {
        tombstones
            .blocking(&TombstoneAddress::Note {
                note_id: note_id.clone(),
            })
            .is_none()
    });
    for note in projected.notes.values_mut() {
        if let Some(note_type) = projected.note_types.get(&note.note_type_id) {
            let exported_fields = note_type
                .fields
                .iter()
                .map(|field| field.id.clone())
                .collect::<BTreeSet<_>>();
            note.fields
                .retain(|field_id, _| exported_fields.contains(field_id));
        }
    }
    projected.media.retain(|media_id, _| {
        tombstones
            .blocking(&TombstoneAddress::MediaReference {
                media_id: media_id.clone(),
            })
            .is_none()
    });

    // Materialize adapter-visible fallback identities before canonical stable IDs are
    // normalized to the IDs import will suggest.
    projected.adapter_ids = projected_deck_adapter_ids(&projected);
    for note_type in projected.note_types.values_mut() {
        note_type.adapter_ids = projected_note_type_adapter_ids(note_type)?;
    }
    for note in projected.notes.values_mut() {
        note.adapter_ids = projected_note_adapter_ids(note);
    }

    normalize_projected_stable_ids(&mut projected)?;
    projected.variables.clear();
    for note_type in projected.note_types.values_mut() {
        note_type.variables.clear();
        for template in &mut note_type.card_templates {
            template.variables.clear();
            template.adapter_ids = AdapterIds::new();
        }
    }
    for note in projected.notes.values_mut() {
        note.variables.clear();
    }
    for media in projected.media.values_mut() {
        media.sha256.clear();
    }
    projected.tombstones = Tombstones::default();
    Ok(projected)
}

fn normalize_projected_stable_ids(deck: &mut CanonicalDeck) -> Result<(), CrowdAnkiError> {
    deck.id = prefixed_stable_id("deck", &deck.name)?;

    let mut note_type_ids = BTreeMap::new();
    let mut field_ids = BTreeMap::<StableId, BTreeMap<StableId, StableId>>::new();
    let mut normalized_note_types = BTreeMap::new();
    for (old_note_type_id, mut note_type) in std::mem::take(&mut deck.note_types) {
        let new_note_type_id = prefixed_stable_id("note-type", &note_type.name)?;
        let mut note_type_field_ids = BTreeMap::new();
        for field in &mut note_type.fields {
            let old_field_id = field.id.clone();
            field.id = prefixed_stable_id("field", &field.name)?;
            note_type_field_ids.insert(old_field_id, field.id.clone());
        }
        for template in &mut note_type.card_templates {
            template.id = prefixed_stable_id("template", &template.name)?;
        }
        note_type.id = new_note_type_id.clone();
        if normalized_note_types
            .insert(new_note_type_id.clone(), note_type)
            .is_some()
        {
            return Err(CrowdAnkiError::Unsupported(format!(
                "{} profile generated duplicate note type stable ID {}",
                CROWDANKI_ROUND_TRIP_PROFILE.name, new_note_type_id
            )));
        }
        field_ids.insert(old_note_type_id.clone(), note_type_field_ids);
        note_type_ids.insert(old_note_type_id, new_note_type_id);
    }
    deck.note_types = normalized_note_types;

    let mut normalized_notes = BTreeMap::new();
    for (_old_note_id, mut note) in std::mem::take(&mut deck.notes) {
        let old_note_type_id = note.note_type_id.clone();
        let new_note_type_id = note_type_ids.get(&old_note_type_id).ok_or_else(|| {
            CrowdAnkiError::Unsupported(format!(
                "{} profile cannot map note type {}",
                CROWDANKI_ROUND_TRIP_PROFILE.name, old_note_type_id
            ))
        })?;
        let note_type = deck
            .note_types
            .get(new_note_type_id)
            .expect("normalized note type ID was inserted");
        let mapping = field_ids
            .get(&old_note_type_id)
            .expect("normalized field IDs were recorded");
        note.fields = note
            .fields
            .iter()
            .map(|(old_field_id, value)| {
                mapping
                    .get(old_field_id)
                    .cloned()
                    .map(|field_id| (field_id, value.clone()))
                    .ok_or_else(|| {
                        CrowdAnkiError::Unsupported(format!(
                            "{} profile cannot map field {}",
                            CROWDANKI_ROUND_TRIP_PROFILE.name, old_field_id
                        ))
                    })
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?
            .into();
        note.note_type_id = new_note_type_id.clone();
        let first_field = note_type
            .fields
            .first()
            .and_then(|field| note.fields.get(&field.id))
            .and_then(FieldValue::as_scalar)
            .unwrap_or_else(|| {
                note.adapter_ids
                    .get("crowdanki:guid")
                    .expect("exported notes always have a first field or effective GUID")
            });
        note.id = prefixed_stable_id("note", first_field)?;
        if normalized_notes.insert(note.id.clone(), note).is_some() {
            return Err(CrowdAnkiError::Unsupported(format!(
                "{} profile generated duplicate note stable ID",
                CROWDANKI_ROUND_TRIP_PROFILE.name
            )));
        }
    }
    deck.notes = normalized_notes;

    let mut normalized_media = BTreeMap::new();
    for (_old_media_id, mut media) in std::mem::take(&mut deck.media) {
        media.id = prefixed_stable_id("media", &media.path)?;
        if normalized_media.insert(media.id.clone(), media).is_some() {
            return Err(CrowdAnkiError::Unsupported(format!(
                "{} profile generated duplicate media stable ID",
                CROWDANKI_ROUND_TRIP_PROFILE.name
            )));
        }
    }
    deck.media = normalized_media;
    Ok(())
}

fn projected_deck_adapter_ids(deck: &CanonicalDeck) -> AdapterIds {
    let mut ids = AdapterIds::new();
    ids.insert("crowdanki:uuid", crowdanki_deck_uuid(deck));
    ids.insert(
        "crowdanki:deck_config_uuid",
        crowdanki_deck_config_uuid(deck),
    );
    ids.insert(
        "crowdanki:deck_config_name",
        crowdanki_deck_config_name(deck),
    );
    ids
}

fn projected_note_type_adapter_ids(note_type: &NoteType) -> Result<AdapterIds, CrowdAnkiError> {
    let mut ids = AdapterIds::new();
    ids.insert("crowdanki:uuid", crowdanki_note_model_uuid(note_type)?);
    Ok(ids)
}

fn projected_note_adapter_ids(note: &Note) -> AdapterIds {
    let mut ids = AdapterIds::new();
    ids.insert("crowdanki:guid", crowdanki_note_guid(note));
    ids
}

fn json_path(path: &serde_path_to_error::Path) -> String {
    let path = path.to_string();
    if path.is_empty() || path == "." {
        "$".to_owned()
    } else if path.starts_with('[') {
        format!("${path}")
    } else {
        format!("$.{path}")
    }
}

/// Options for comparing generated CrowdAnki JSON with an expected oracle.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CrowdAnkiParityOptions {
    /// JSON path globs explicitly allowed to differ.
    pub allowed_path_globs: BTreeSet<String>,
}

/// A CrowdAnki parity comparison failure report.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CrowdAnkiParityReport {
    pub differences: Vec<CrowdAnkiParityDifference>,
}

/// One exact JSON difference between expected and actual CrowdAnki output.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CrowdAnkiParityDifference {
    pub path: String,
    pub kind: CrowdAnkiParityDifferenceKind,
    pub expected: Option<serde_json::Value>,
    pub actual: Option<serde_json::Value>,
}

/// The broad shape of a CrowdAnki JSON parity difference.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CrowdAnkiParityDifferenceKind {
    MissingActual,
    ExtraActual,
    ValueMismatch,
    LengthMismatch,
}

/// Compare two CrowdAnki `deck.json` values exactly, with only explicit path allowlists.
pub fn compare_deck_json_values(
    expected: &serde_json::Value,
    actual: &serde_json::Value,
    options: &CrowdAnkiParityOptions,
) -> Result<(), CrowdAnkiParityReport> {
    let mut differences = Vec::new();
    compare_json_value(expected, actual, "$", options, &mut differences);
    if differences.is_empty() {
        Ok(())
    } else {
        Err(CrowdAnkiParityReport { differences })
    }
}

impl fmt::Display for CrowdAnkiParityReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} CrowdAnki JSON difference(s)", self.differences.len())?;
        let grouped_paths = repeated_difference_groups(&self.differences);
        if !grouped_paths.is_empty() {
            writeln!(f, "Repeated differences:")?;
            for group in &grouped_paths {
                writeln!(
                    f,
                    "{} × {} ({:?}): expected {}, actual {}",
                    group.count,
                    group.path_pattern,
                    group.kind,
                    json_value_summary(group.expected.as_ref()),
                    json_value_summary(group.actual.as_ref())
                )?;
            }
        }
        let grouped_patterns = grouped_paths
            .iter()
            .map(|group| group.path_pattern.as_str())
            .collect::<BTreeSet<_>>();
        let mut shown = 0;
        for difference in &self.differences {
            if grouped_patterns.contains(normalize_repeated_path(&difference.path).as_str()) {
                continue;
            }
            if shown >= 20 {
                break;
            }
            writeln!(
                f,
                "{} ({:?}): expected {}, actual {}",
                difference.path,
                difference.kind,
                json_value_summary(difference.expected.as_ref()),
                json_value_summary(difference.actual.as_ref())
            )?;
            shown += 1;
        }
        let ungrouped_count = self
            .differences
            .iter()
            .filter(|difference| {
                !grouped_patterns.contains(normalize_repeated_path(&difference.path).as_str())
            })
            .count();
        if ungrouped_count > shown {
            writeln!(f, "... {} more", ungrouped_count - shown)?;
        }
        Ok(())
    }
}

struct RepeatedDifferenceGroup {
    path_pattern: String,
    kind: CrowdAnkiParityDifferenceKind,
    expected: Option<serde_json::Value>,
    actual: Option<serde_json::Value>,
    count: usize,
}

fn repeated_difference_groups(
    differences: &[CrowdAnkiParityDifference],
) -> Vec<RepeatedDifferenceGroup> {
    let mut groups = BTreeMap::<(String, String, String, String), RepeatedDifferenceGroup>::new();
    for difference in differences {
        let path_pattern = normalize_repeated_path(&difference.path);
        if path_pattern == difference.path {
            continue;
        }
        let key = (
            path_pattern.clone(),
            format!("{:?}", difference.kind),
            json_value_summary(difference.expected.as_ref()),
            json_value_summary(difference.actual.as_ref()),
        );
        groups
            .entry(key)
            .and_modify(|group| group.count += 1)
            .or_insert_with(|| RepeatedDifferenceGroup {
                path_pattern,
                kind: difference.kind.clone(),
                expected: difference.expected.clone(),
                actual: difference.actual.clone(),
                count: 1,
            });
    }

    groups
        .into_values()
        .filter(|group| group.count > 1)
        .collect()
}

fn normalize_repeated_path(path: &str) -> String {
    let mut normalized = String::new();
    let mut chars = path.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '[' {
            normalized.push(ch);
            continue;
        }

        let mut bracket = String::from("[");
        for bracket_ch in chars.by_ref() {
            bracket.push(bracket_ch);
            if bracket_ch == ']' {
                break;
            }
        }
        if bracket
            .chars()
            .skip(1)
            .all(|ch| ch.is_ascii_digit() || ch == ']')
            || bracket.contains('=')
        {
            normalized.push_str("[*]");
        } else {
            normalized.push_str(&bracket);
        }
    }
    normalized
}

fn compare_json_value(
    expected: &serde_json::Value,
    actual: &serde_json::Value,
    path: &str,
    options: &CrowdAnkiParityOptions,
    differences: &mut Vec<CrowdAnkiParityDifference>,
) {
    if expected == actual || is_allowed_parity_path(options, path) {
        return;
    }

    match (expected, actual) {
        (serde_json::Value::Object(expected), serde_json::Value::Object(actual)) => {
            let keys = expected
                .keys()
                .chain(actual.keys())
                .collect::<BTreeSet<_>>();
            for key in keys {
                let child_path = json_path_key(path, key);
                if is_allowed_parity_path(options, &child_path) {
                    continue;
                }
                match (expected.get(key), actual.get(key)) {
                    (Some(expected), Some(actual)) => {
                        compare_json_value(expected, actual, &child_path, options, differences);
                    }
                    (Some(expected), None) => differences.push(CrowdAnkiParityDifference {
                        path: child_path,
                        kind: CrowdAnkiParityDifferenceKind::MissingActual,
                        expected: Some(expected.clone()),
                        actual: None,
                    }),
                    (None, Some(actual)) => differences.push(CrowdAnkiParityDifference {
                        path: child_path,
                        kind: CrowdAnkiParityDifferenceKind::ExtraActual,
                        expected: None,
                        actual: Some(actual.clone()),
                    }),
                    (None, None) => {}
                }
            }
        }
        (serde_json::Value::Array(expected), serde_json::Value::Array(actual)) => {
            if compare_json_array_by_identity(expected, actual, path, options, differences) {
                return;
            }
            for index in 0..expected.len().min(actual.len()) {
                let child_path = format!("{path}[{index}]");
                compare_json_value(
                    &expected[index],
                    &actual[index],
                    &child_path,
                    options,
                    differences,
                );
            }
            if expected.len() != actual.len() {
                let length_path = format!("{path}.length");
                if !is_allowed_parity_path(options, &length_path) {
                    differences.push(CrowdAnkiParityDifference {
                        path: length_path,
                        kind: CrowdAnkiParityDifferenceKind::LengthMismatch,
                        expected: Some(serde_json::json!(expected.len())),
                        actual: Some(serde_json::json!(actual.len())),
                    });
                }
            }
            for (index, value) in expected.iter().enumerate().skip(actual.len()) {
                let child_path = format!("{path}[{index}]");
                if !is_allowed_parity_path(options, &child_path) {
                    differences.push(CrowdAnkiParityDifference {
                        path: child_path,
                        kind: CrowdAnkiParityDifferenceKind::MissingActual,
                        expected: Some(value.clone()),
                        actual: None,
                    });
                }
            }
            for (index, value) in actual.iter().enumerate().skip(expected.len()) {
                let child_path = format!("{path}[{index}]");
                if !is_allowed_parity_path(options, &child_path) {
                    differences.push(CrowdAnkiParityDifference {
                        path: child_path,
                        kind: CrowdAnkiParityDifferenceKind::ExtraActual,
                        expected: None,
                        actual: Some(value.clone()),
                    });
                }
            }
        }
        _ => differences.push(CrowdAnkiParityDifference {
            path: path.to_owned(),
            kind: CrowdAnkiParityDifferenceKind::ValueMismatch,
            expected: Some(expected.clone()),
            actual: Some(actual.clone()),
        }),
    }
}

fn compare_json_array_by_identity(
    expected: &[serde_json::Value],
    actual: &[serde_json::Value],
    path: &str,
    options: &CrowdAnkiParityOptions,
    differences: &mut Vec<CrowdAnkiParityDifference>,
) -> bool {
    if path == "$.media_files" {
        return compare_json_string_array_as_multiset(expected, actual, path, options, differences);
    }

    let Some(identity) = array_identity(path) else {
        return false;
    };

    let Some(expected_by_key) = array_by_identity(expected, identity) else {
        return false;
    };
    let Some(actual_by_key) = array_by_identity(actual, identity) else {
        return false;
    };

    let keys = expected_by_key
        .keys()
        .chain(actual_by_key.keys())
        .collect::<BTreeSet<_>>();
    for key in keys {
        let child_path = format!("{path}[{}={}]", identity.name, json_path_label(key));
        if is_allowed_parity_path(options, &child_path) {
            continue;
        }
        match (expected_by_key.get(key), actual_by_key.get(key)) {
            (Some(expected), Some(actual)) => {
                compare_json_value(expected, actual, &child_path, options, differences);
            }
            (Some(expected), None) => differences.push(CrowdAnkiParityDifference {
                path: child_path,
                kind: CrowdAnkiParityDifferenceKind::MissingActual,
                expected: Some((*expected).clone()),
                actual: None,
            }),
            (None, Some(actual)) => differences.push(CrowdAnkiParityDifference {
                path: child_path,
                kind: CrowdAnkiParityDifferenceKind::ExtraActual,
                expected: None,
                actual: Some((*actual).clone()),
            }),
            (None, None) => {}
        }
    }

    true
}

fn compare_json_string_array_as_multiset(
    expected: &[serde_json::Value],
    actual: &[serde_json::Value],
    path: &str,
    options: &CrowdAnkiParityOptions,
    differences: &mut Vec<CrowdAnkiParityDifference>,
) -> bool {
    let Some(expected_counts) = string_array_multiset(expected) else {
        return false;
    };
    let Some(actual_counts) = string_array_multiset(actual) else {
        return false;
    };

    let keys = expected_counts
        .keys()
        .chain(actual_counts.keys())
        .collect::<BTreeSet<_>>();
    for key in keys {
        let child_path = format!("{path}[path={}]", json_path_label(key));
        if is_allowed_parity_path(options, &child_path) {
            continue;
        }
        let expected_count = expected_counts.get(key).copied().unwrap_or_default();
        let actual_count = actual_counts.get(key).copied().unwrap_or_default();
        match (expected_count, actual_count) {
            (expected_count, actual_count) if expected_count == actual_count => {}
            (0, actual_count) => differences.push(CrowdAnkiParityDifference {
                path: child_path,
                kind: CrowdAnkiParityDifferenceKind::ExtraActual,
                expected: None,
                actual: Some(serde_json::json!(actual_count)),
            }),
            (expected_count, 0) => differences.push(CrowdAnkiParityDifference {
                path: child_path,
                kind: CrowdAnkiParityDifferenceKind::MissingActual,
                expected: Some(serde_json::json!(expected_count)),
                actual: None,
            }),
            (expected_count, actual_count) => differences.push(CrowdAnkiParityDifference {
                path: child_path,
                kind: CrowdAnkiParityDifferenceKind::LengthMismatch,
                expected: Some(serde_json::json!(expected_count)),
                actual: Some(serde_json::json!(actual_count)),
            }),
        }
    }

    true
}

fn string_array_multiset(values: &[serde_json::Value]) -> Option<BTreeMap<String, usize>> {
    let mut counts = BTreeMap::new();
    for value in values {
        let key = value.as_str()?.to_owned();
        *counts.entry(key).or_insert(0) += 1;
    }
    Some(counts)
}

#[derive(Clone, Copy)]
struct ArrayIdentity {
    name: &'static str,
    value: fn(&serde_json::Value) -> Option<String>,
}

fn array_identity(path: &str) -> Option<ArrayIdentity> {
    match path {
        "$.notes" => Some(ArrayIdentity {
            name: "guid",
            value: |value| value.get("guid")?.as_str().map(str::to_owned),
        }),
        "$.note_models" => Some(ArrayIdentity {
            name: "model",
            value: |value| {
                value
                    .get("crowdanki_uuid")
                    .and_then(serde_json::Value::as_str)
                    .or_else(|| value.get("name").and_then(serde_json::Value::as_str))
                    .map(str::to_owned)
            },
        }),
        path if path.ends_with(".flds") => Some(ArrayIdentity {
            name: "name",
            value: |value| value.get("name")?.as_str().map(str::to_owned),
        }),
        path if path.ends_with(".tmpls") => Some(ArrayIdentity {
            name: "template",
            value: |value| {
                value
                    .get("ord")
                    .and_then(serde_json::Value::as_i64)
                    .map(|ord| ord.to_string())
                    .or_else(|| {
                        value
                            .get("name")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_owned)
                    })
            },
        }),
        _ => None,
    }
}

fn array_by_identity(
    values: &[serde_json::Value],
    identity: ArrayIdentity,
) -> Option<BTreeMap<String, &serde_json::Value>> {
    let mut by_key = BTreeMap::new();
    for value in values {
        let key = (identity.value)(value)?;
        if by_key.insert(key, value).is_some() {
            return None;
        }
    }
    Some(by_key)
}

fn json_path_label(value: &str) -> String {
    serde_json::to_string(value).expect("serializing a JSON path label cannot fail")
}

fn is_allowed_parity_path(options: &CrowdAnkiParityOptions, path: &str) -> bool {
    options
        .allowed_path_globs
        .iter()
        .any(|pattern| brain_brew_core::glob_matches(pattern, path))
}

fn json_path_key(parent: &str, key: &str) -> String {
    if key
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        format!("{parent}.{key}")
    } else {
        format!(
            "{parent}[{}]",
            serde_json::to_string(key).expect("serializing a JSON key cannot fail")
        )
    }
}

fn json_value_summary(value: Option<&serde_json::Value>) -> String {
    let Some(value) = value else {
        return "<missing>".to_owned();
    };
    let mut summary = serde_json::to_string(value).expect("serializing JSON value cannot fail");
    if summary.len() > 120 {
        summary.truncate(117);
        summary.push_str("...");
    }
    summary
}

fn export_note_model(
    note_type: &NoteType,
    deck: &CanonicalDeck,
) -> Result<CrowdAnkiNoteModelJson, CrowdAnkiError> {
    Ok(CrowdAnkiNoteModelJson {
        kind: "NoteModel".to_owned(),
        crowdanki_uuid: crowdanki_note_model_uuid(note_type)?,
        css: note_type.styling.clone(),
        flds: note_type
            .fields
            .iter()
            .filter(|field| {
                deck.tombstones
                    .blocking(&TombstoneAddress::FieldDefinition {
                        note_type_id: note_type.id.clone(),
                        field_id: field.id.clone(),
                    })
                    .is_none()
            })
            .enumerate()
            .map(|(ord, field)| CrowdAnkiFieldJson {
                font: "Arial".to_owned(),
                media: Vec::new(),
                name: field.name.clone(),
                ord,
                rtl: false,
                size: 20,
                sticky: false,
            })
            .collect(),
        latex_post: "\\end{document}".to_owned(),
        latex_pre: default_latex_pre(),
        latex_svg: false,
        name: note_type.name.clone(),
        req: Vec::new(),
        sortf: 0,
        tags: Vec::new(),
        tmpls: note_type
            .card_templates
            .iter()
            .filter(|template| {
                deck.tombstones
                    .blocking(&TombstoneAddress::CardTemplate {
                        note_type_id: note_type.id.clone(),
                        template_id: template.id.clone(),
                    })
                    .is_none()
            })
            .enumerate()
            .map(|(ord, template)| CrowdAnkiTemplateJson {
                afmt: template.answer_format.clone(),
                bafmt: String::new(),
                bfont: Some(String::new()),
                bqfmt: String::new(),
                bsize: Some(0),
                did: None,
                name: template.name.clone(),
                ord,
                qfmt: template.question_format.clone(),
                scratch_pad: Some(0),
            })
            .collect(),
        model_type: 0,
        vers: Vec::new(),
    })
}

fn export_note(
    note: &Note,
    deck: &CanonicalDeck,
    note_type_uuids: &BTreeMap<StableId, String>,
) -> Result<CrowdAnkiNoteJson, CrowdAnkiError> {
    let note_type = deck.note_types.get(&note.note_type_id).ok_or_else(|| {
        CrowdAnkiError::Unsupported(format!(
            "note {} references missing note type {}",
            note.id, note.note_type_id
        ))
    })?;
    let note_model_uuid = note_type_uuids
        .get(&note.note_type_id)
        .cloned()
        .expect("note type uuid was precomputed");

    let fields = note_type
        .fields
        .iter()
        .filter(|field| {
            deck.tombstones
                .blocking(&TombstoneAddress::FieldDefinition {
                    note_type_id: note_type.id.clone(),
                    field_id: field.id.clone(),
                })
                .is_none()
        })
        .map(|field| {
            note.fields
                .get(&field.id)
                .and_then(FieldValue::as_scalar)
                .map(str::to_owned)
                .ok_or_else(|| {
                    CrowdAnkiError::Unsupported(format!(
                        "note {} field {} was not lowered to scalar adapter text",
                        note.id, field.id
                    ))
                })
        })
        .collect::<Result<_, _>>()?;

    Ok(CrowdAnkiNoteJson {
        type_: "Note".to_owned(),
        data: String::new(),
        fields,
        flags: 0,
        guid: crowdanki_note_guid(note),
        note_model_uuid,
        tags: note.tags.iter().cloned().collect(),
    })
}

fn crowdanki_deck_uuid(deck: &CanonicalDeck) -> String {
    deck.adapter_ids
        .get("crowdanki:uuid")
        .map(str::to_owned)
        .unwrap_or_else(|| deck.id.to_string())
}

fn crowdanki_deck_config_uuid(deck: &CanonicalDeck) -> String {
    deck.adapter_ids
        .get("crowdanki:deck_config_uuid")
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{}:deck-config", deck.id))
}

fn crowdanki_deck_config_name(deck: &CanonicalDeck) -> String {
    deck.adapter_ids
        .get("crowdanki:deck_config_name")
        .map(str::to_owned)
        .unwrap_or_else(|| deck.name.clone())
}

fn crowdanki_note_model_uuid(note_type: &NoteType) -> Result<String, CrowdAnkiError> {
    note_type
        .adapter_ids
        .get("crowdanki:uuid")
        .map(str::to_owned)
        .ok_or_else(|| {
            CrowdAnkiError::Unsupported(format!(
                "note type {} is missing crowdanki:uuid adapter id",
                note_type.id
            ))
        })
}

fn crowdanki_note_guid(note: &Note) -> String {
    note.adapter_ids
        .get("crowdanki:guid")
        .map(str::to_owned)
        .unwrap_or_else(|| note.id.to_string())
}

fn default_latex_pre() -> String {
    "\\documentclass[12pt]{article}\n\\special{papersize=3in,5in}\n\\usepackage{amssymb,amsmath}\n\\pagestyle{empty}\n\\setlength{\\parindent}{0in}\n\\begin{document}\n"
        .to_owned()
}

fn default_deck_config_json(uuid: &str, name: &str) -> serde_json::Value {
    serde_json::json!({
        "__type__": "DeckConfig",
        "crowdanki_uuid": uuid,
        "name": name,
        "autoplay": false,
        "dyn": false,
        "lapse": {
            "delays": [10],
            "leechAction": 0,
            "leechFails": 8,
            "minInt": 1,
            "mult": 0,
        },
        "maxTaken": 60,
        "new": {
            "bury": true,
            "delays": [1, 10],
            "initialFactor": 2500,
            "ints": [1, 4, 7],
            "order": 0,
            "perDay": 15,
            "separate": true,
        },
        "replayq": true,
        "rev": {
            "bury": true,
            "ease4": 1.3,
            "fuzz": 0.05,
            "ivlFct": 1,
            "maxIvl": 36500,
            "minSpace": 1,
            "perDay": 100,
        },
        "timer": 0,
    })
}

fn validate_supported_deck_configurations(
    uuid: &str,
    configurations: &[serde_json::Value],
) -> Result<String, CrowdAnkiError> {
    if configurations.len() != 1 {
        return Err(CrowdAnkiError::Unsupported(format!(
            "expected one default deck configuration, found {}",
            configurations.len()
        )));
    }
    let Some(name) = configurations[0]
        .get("name")
        .and_then(serde_json::Value::as_str)
    else {
        return Err(CrowdAnkiError::Unsupported(
            "deck configuration is missing a name".to_owned(),
        ));
    };
    let expected = default_deck_config_json(uuid, name);
    if configurations[0] != expected {
        return Err(CrowdAnkiError::Unsupported(
            "non-default deck configurations are not modeled yet".to_owned(),
        ));
    }
    Ok(name.to_owned())
}

#[derive(Debug)]
pub enum CrowdAnkiError {
    Json(serde_json::Error),
    JsonPath { path: String, message: String },
    StableId(String),
    Unsupported(String),
    Validation(ValidationReport),
    VariableRender(VariableRenderReport),
    Media(media::MediaValidationReport),
}

impl fmt::Display for CrowdAnkiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(f, "CrowdAnki JSON error: {error}"),
            Self::JsonPath { path, message } => {
                write!(f, "CrowdAnki JSON error at schema path {path}: {message}")
            }
            Self::StableId(id) => write!(f, "generated invalid stable id {id:?}"),
            Self::Unsupported(message) => write!(f, "unsupported CrowdAnki data: {message}"),
            Self::Validation(report) => write!(f, "imported deck failed validation: {report}"),
            Self::VariableRender(report) => write!(f, "deck variable rendering failed: {report}"),
            Self::Media(report) => write!(f, "CrowdAnki media path validation failed: {report}"),
        }
    }
}

impl std::error::Error for CrowdAnkiError {}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CrowdAnkiDeckJson {
    #[serde(rename = "__type__")]
    type_: String,
    children: Vec<serde_json::Value>,
    crowdanki_uuid: String,
    deck_config_uuid: String,
    deck_configurations: Vec<serde_json::Value>,
    desc: String,
    #[serde(rename = "dyn")]
    dyn_: i64,
    #[serde(rename = "extendNew")]
    extend_new: i64,
    #[serde(rename = "extendRev")]
    extend_rev: i64,
    media_files: Vec<String>,
    name: String,
    note_models: Vec<CrowdAnkiNoteModelJson>,
    notes: Vec<CrowdAnkiNoteJson>,
}

impl CrowdAnkiDeckJson {
    fn into_deck(self) -> Result<CanonicalDeck, CrowdAnkiError> {
        if self.type_ != "Deck" {
            return Err(CrowdAnkiError::Unsupported(format!(
                "expected __type__ Deck, found {}",
                self.type_
            )));
        }
        if !self.children.is_empty() {
            return Err(CrowdAnkiError::Unsupported(
                "child decks are not modeled yet".to_owned(),
            ));
        }
        if self.dyn_ != 0 || self.extend_new != 10 || self.extend_rev != 50 {
            return Err(CrowdAnkiError::Unsupported(format!(
                "non-default deck scheduling header is not modeled yet (dyn={}, extendNew={}, extendRev={})",
                self.dyn_, self.extend_new, self.extend_rev
            )));
        }
        let deck_config_name = validate_supported_deck_configurations(
            &self.deck_config_uuid,
            &self.deck_configurations,
        )?;

        let deck_id = prefixed_stable_id("deck", &self.name)?;
        let mut deck_adapter_ids = AdapterIds::new();
        deck_adapter_ids.insert("crowdanki:uuid", self.crowdanki_uuid);
        deck_adapter_ids.insert("crowdanki:deck_config_uuid", self.deck_config_uuid);
        deck_adapter_ids.insert("crowdanki:deck_config_name", deck_config_name);

        let mut note_type_by_uuid: BTreeMap<String, StableId> = BTreeMap::new();
        let mut note_types: BTreeMap<StableId, NoteType> = BTreeMap::new();
        for note_model in self.note_models {
            let (uuid, id, note_type) = note_model.into_note_type()?;
            if let Some(existing) = note_types.get(&id) {
                return Err(CrowdAnkiError::Unsupported(format!(
                    "CrowdAnki note models {:?} and {:?} both derive suggested stable ID {}; {}",
                    existing.name,
                    note_type.name,
                    id,
                    suggested_id_collision_resolution()
                )));
            }
            if let Some(existing_id) = note_type_by_uuid.get(&uuid) {
                let existing = note_types
                    .get(existing_id)
                    .expect("note type UUID map points at inserted note type");
                return Err(CrowdAnkiError::Unsupported(format!(
                    "CrowdAnki note models {:?} and {:?} share crowdanki_uuid {:?}; {}",
                    existing.name,
                    note_type.name,
                    uuid,
                    suggested_id_collision_resolution()
                )));
            }
            note_type_by_uuid.insert(uuid, id.clone());
            note_types.insert(id, note_type);
        }

        let mut note_sources: BTreeMap<StableId, CrowdAnkiNoteSource> = BTreeMap::new();
        let mut notes: BTreeMap<StableId, Note> = BTreeMap::new();
        for note_json in self.notes {
            let source = CrowdAnkiNoteSource::from_note_json(&note_json);
            let (id, note) = note_json.into_note(&note_types, &note_type_by_uuid)?;
            if let Some(existing) = note_sources.get(&id) {
                return Err(CrowdAnkiError::Unsupported(format!(
                    "CrowdAnki notes {} and {} both derive suggested stable ID {}; {}",
                    existing.describe(),
                    source.describe(),
                    id,
                    suggested_id_collision_resolution()
                )));
            }
            note_sources.insert(id.clone(), source);
            notes.insert(id, note);
        }

        let ambiguous_media_file_paths = duplicate_paths(&self.media_files);
        let mut media_sources: BTreeMap<StableId, String> = BTreeMap::new();
        let mut media = BTreeMap::new();
        for path in self.media_files {
            let id = prefixed_stable_id("media", &path)?;
            if let Some(existing_path) = media_sources.get(&id) {
                if existing_path != &path {
                    return Err(CrowdAnkiError::Unsupported(format!(
                        "CrowdAnki media files {:?} and {:?} both derive suggested stable ID {}; {}",
                        existing_path,
                        path,
                        id,
                        suggested_id_collision_resolution()
                    )));
                }
                continue;
            }
            media_sources.insert(id.clone(), path.clone());
            media.insert(
                id.clone(),
                MediaReference {
                    id,
                    path,
                    sha256: String::new(),
                },
            );
        }

        let media_path_lookup = media_path_lookup(&media, &ambiguous_media_file_paths);
        for note in notes.values_mut() {
            reverse_map_strict_image_fields(note, &media_path_lookup);
        }

        let deck = CanonicalDeck {
            id: deck_id,
            name: self.name,
            description: self.desc,
            note_types,
            notes,
            media,
            tombstones: Tombstones::default(),
            variables: BTreeMap::new(),
            adapter_ids: deck_adapter_ids,
        };
        deck.validate().map_err(CrowdAnkiError::Validation)?;
        media::validate_paths(&deck).map_err(CrowdAnkiError::Media)?;
        Ok(deck)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CrowdAnkiNoteModelJson {
    #[serde(rename = "__type__")]
    kind: String,
    crowdanki_uuid: String,
    css: String,
    flds: Vec<CrowdAnkiFieldJson>,
    #[serde(rename = "latexPost")]
    latex_post: String,
    #[serde(rename = "latexPre")]
    latex_pre: String,
    #[serde(rename = "latexsvg")]
    latex_svg: bool,
    name: String,
    req: Vec<serde_json::Value>,
    sortf: usize,
    tags: Vec<String>,
    tmpls: Vec<CrowdAnkiTemplateJson>,
    #[serde(rename = "type")]
    model_type: i64,
    vers: Vec<serde_json::Value>,
}

impl CrowdAnkiNoteModelJson {
    fn validate_supported_defaults(&self) -> Result<(), CrowdAnkiError> {
        if self.latex_post != "\\end{document}"
            || self.latex_pre != default_latex_pre()
            || self.latex_svg
            || !self.req.is_empty()
            || self.sortf != 0
            || !self.tags.is_empty()
            || !self.vers.is_empty()
        {
            return Err(CrowdAnkiError::Unsupported(format!(
                "note model {} has non-default CrowdAnki options that are not modeled yet",
                self.name
            )));
        }
        Ok(())
    }

    fn into_note_type(self) -> Result<(String, StableId, NoteType), CrowdAnkiError> {
        if self.kind != "NoteModel" {
            return Err(CrowdAnkiError::Unsupported(format!(
                "expected note model __type__ NoteModel, found {}",
                self.kind
            )));
        }
        if self.model_type != 0 {
            return Err(CrowdAnkiError::Unsupported(format!(
                "only standard note models are supported, found type {}",
                self.model_type
            )));
        }
        self.validate_supported_defaults()?;

        let id = prefixed_stable_id("note-type", &self.name)?;
        let mut adapter_ids = AdapterIds::new();
        adapter_ids.insert("crowdanki:uuid", self.crowdanki_uuid.clone());

        let fields = self
            .flds
            .into_iter()
            .enumerate()
            .map(|(index, field)| {
                field.validate_supported_defaults(index)?;
                Ok(FieldDefinition {
                    id: prefixed_stable_id("field", &field.name)?,
                    name: field.name,
                })
            })
            .collect::<Result<Vec<_>, CrowdAnkiError>>()?;

        let mut templates = self.tmpls;
        templates.sort_by_key(|template| template.ord);
        let card_templates = templates
            .into_iter()
            .map(|template| {
                template.validate_supported_defaults()?;
                Ok(CardTemplate {
                    id: prefixed_stable_id("template", &template.name)?,
                    name: template.name,
                    variables: BTreeMap::new(),
                    question_format: template.qfmt,
                    answer_format: template.afmt,
                    adapter_ids: AdapterIds::new(),
                })
            })
            .collect::<Result<Vec<_>, CrowdAnkiError>>()?;

        let note_type = NoteType {
            id: id.clone(),
            name: self.name,
            variables: BTreeMap::new(),
            fields,
            card_templates,
            styling: self.css,
            adapter_ids,
        };

        Ok((self.crowdanki_uuid, id, note_type))
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CrowdAnkiFieldJson {
    font: String,
    media: Vec<serde_json::Value>,
    name: String,
    ord: usize,
    rtl: bool,
    size: usize,
    sticky: bool,
}

impl CrowdAnkiFieldJson {
    fn validate_supported_defaults(&self, expected_ord: usize) -> Result<(), CrowdAnkiError> {
        if self.font != "Arial"
            || !self.media.is_empty()
            || self.ord != expected_ord
            || self.rtl
            || self.size != 20
            || self.sticky
        {
            return Err(CrowdAnkiError::Unsupported(format!(
                "field {} has non-default CrowdAnki options that are not modeled yet",
                self.name
            )));
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CrowdAnkiTemplateJson {
    afmt: String,
    bafmt: String,
    bfont: Option<String>,
    bqfmt: String,
    bsize: Option<i64>,
    did: Option<i64>,
    name: String,
    ord: usize,
    qfmt: String,
    #[serde(rename = "scratchPad")]
    scratch_pad: Option<i64>,
}

impl CrowdAnkiTemplateJson {
    fn validate_supported_defaults(&self) -> Result<(), CrowdAnkiError> {
        if !self.bafmt.is_empty()
            || self.bfont.as_deref().unwrap_or_default() != ""
            || !self.bqfmt.is_empty()
            || self.bsize.unwrap_or_default() != 0
            || self.did.is_some()
            || self.scratch_pad.unwrap_or_default() != 0
        {
            return Err(CrowdAnkiError::Unsupported(format!(
                "card template {} has non-default browser options that are not modeled yet",
                self.name
            )));
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CrowdAnkiNoteJson {
    #[serde(rename = "__type__")]
    type_: String,
    data: String,
    fields: Vec<String>,
    flags: i64,
    guid: String,
    note_model_uuid: String,
    tags: Vec<String>,
}

struct CrowdAnkiNoteSource {
    guid: String,
    first_field: String,
}

impl CrowdAnkiNoteSource {
    fn from_note_json(note: &CrowdAnkiNoteJson) -> Self {
        Self {
            guid: note.guid.clone(),
            first_field: note
                .fields
                .first()
                .cloned()
                .unwrap_or_else(|| note.guid.clone()),
        }
    }

    fn describe(&self) -> String {
        format!(
            "guid {:?} with first field {:?}",
            self.guid, self.first_field
        )
    }
}

impl CrowdAnkiNoteJson {
    fn into_note(
        self,
        note_types: &BTreeMap<StableId, NoteType>,
        note_type_by_uuid: &BTreeMap<String, StableId>,
    ) -> Result<(StableId, Note), CrowdAnkiError> {
        if self.type_ != "Note" {
            return Err(CrowdAnkiError::Unsupported(format!(
                "expected note __type__ Note, found {}",
                self.type_
            )));
        }
        if !self.data.is_empty() || self.flags != 0 {
            return Err(CrowdAnkiError::Unsupported(format!(
                "note {} has non-default data/flags that are not modeled yet",
                self.guid
            )));
        }
        let note_type_id = note_type_by_uuid
            .get(&self.note_model_uuid)
            .ok_or_else(|| {
                CrowdAnkiError::Unsupported(format!(
                    "note references missing note_model_uuid {}",
                    self.note_model_uuid
                ))
            })?
            .clone();
        let note_type = note_types
            .get(&note_type_id)
            .expect("note type id came from note type map");
        if self.fields.len() != note_type.fields.len() {
            return Err(CrowdAnkiError::Unsupported(format!(
                "note {} has {} fields but note type {} has {} fields",
                self.guid,
                self.fields.len(),
                note_type.id,
                note_type.fields.len()
            )));
        }

        let first_field = self
            .fields
            .first()
            .map(String::as_str)
            .unwrap_or(&self.guid);
        let id = prefixed_stable_id("note", first_field)?;
        let fields = note_type
            .fields
            .iter()
            .zip(self.fields)
            .map(|(field, value)| (field.id.clone(), FieldValue::Scalar(value)))
            .collect();
        let mut adapter_ids = AdapterIds::new();
        adapter_ids.insert("crowdanki:guid", self.guid);

        Ok((
            id.clone(),
            Note {
                id,
                note_type_id,
                variables: BTreeMap::new(),
                fields,
                tags: self.tags.into_iter().collect(),
                adapter_ids,
            },
        ))
    }
}

fn suggested_id_collision_resolution() -> &'static str {
    "resolve by correcting the suggested-ID override path before calling import_deck_accept_suggested_ids"
}

fn reverse_map_strict_image_fields(
    note: &mut Note,
    media_path_lookup: &BTreeMap<String, Option<StableId>>,
) {
    let field_ids = note.fields.keys().cloned().collect::<Vec<_>>();
    for field_id in field_ids {
        let Some(value) = note.fields.get(&field_id).and_then(FieldValue::as_scalar) else {
            continue;
        };
        let Some(paths) = media::strict_image_tag_paths(value) else {
            continue;
        };

        let mut images = Vec::new();
        for path in paths {
            let Some(Some(media_id)) = media_path_lookup.get(&path) else {
                images.clear();
                break;
            };
            images.push(FieldImageReference {
                media_id: media_id.clone(),
            });
        }
        if images.is_empty() {
            continue;
        }

        note.fields.insert(field_id, FieldValue::Images(images));
    }
}

fn media_path_lookup(
    media: &BTreeMap<StableId, MediaReference>,
    ambiguous_paths: &BTreeSet<String>,
) -> BTreeMap<String, Option<StableId>> {
    let mut lookup: BTreeMap<String, Option<StableId>> = ambiguous_paths
        .iter()
        .map(|path| (path.clone(), None))
        .collect();

    for (id, reference) in media {
        if ambiguous_paths.contains(&reference.path) {
            lookup.insert(reference.path.clone(), None);
            continue;
        }
        lookup
            .entry(reference.path.clone())
            .and_modify(|existing| *existing = None)
            .or_insert_with(|| Some(id.clone()));
    }

    lookup
}

fn duplicate_paths(paths: &[String]) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    let mut duplicates = BTreeSet::new();
    for path in paths {
        if !seen.insert(path.clone()) {
            duplicates.insert(path.clone());
        }
    }
    duplicates
}

fn prefixed_stable_id(prefix: &str, source: &str) -> Result<StableId, CrowdAnkiError> {
    let slug = slugify(source);
    let id = format!("{prefix}.{slug}");
    StableId::new(&id).map_err(|_| CrowdAnkiError::StableId(id))
}

fn slugify(source: &str) -> String {
    let mut slug = String::new();
    let mut last_was_separator = false;
    for ch in source.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_was_separator = false;
        } else if !last_was_separator && !slug.is_empty() {
            slug.push('-');
            last_was_separator = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        "unnamed".to_owned()
    } else {
        slug
    }
}
