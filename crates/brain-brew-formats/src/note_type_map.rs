use std::collections::BTreeMap;

use brain_brew_core::{NoteType, StableId};

use crate::canonical_yaml::{self, CanonicalYamlError};

/// Parse a standalone note-type map used by a base deck structural include.
pub fn from_str(input: &str) -> Result<BTreeMap<StableId, NoteType>, CanonicalYamlError> {
    canonical_yaml::note_type_map_from_str(input)
}

/// Parse and re-emit a standalone note-type map using canonical deck ordering and scalars.
pub fn format_str(input: &str) -> Result<String, CanonicalYamlError> {
    let note_types = from_str(input)?;
    to_string(&note_types)
}

/// Emit a standalone note-type map using canonical deck ordering and scalars.
pub fn to_string(note_types: &BTreeMap<StableId, NoteType>) -> Result<String, CanonicalYamlError> {
    canonical_yaml::note_type_map_to_string(note_types)
}
