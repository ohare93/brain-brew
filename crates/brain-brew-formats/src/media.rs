use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use brain_brew_core::{CanonicalDeck, DeckPath, FieldValue, StableId, TombstoneAddress};
use sha2::{Digest, Sha256};

use crate::safe_relative_path::SafeRelativePath;

/// Extract paths from a whole-field sequence of strict Anki image tags.
///
/// Accepted syntax matches the CrowdAnki import and source migration contract:
/// `<img src="PATH" />` repeated one or more times with optional ASCII whitespace
/// around the whole value and between tags. Any other attributes, quote style,
/// non-self-closing tag, mixed content, empty path, or unsafe path character makes
/// the whole value non-strict.
pub fn strict_image_tag_paths(value: &str) -> Option<Vec<String>> {
    let mut rest = trim_ascii_whitespace(value);
    if rest.is_empty() {
        return None;
    }

    let mut paths = Vec::new();
    loop {
        let after_prefix = rest.strip_prefix("<img src=\"")?;
        let quote_index = after_prefix.find('"')?;
        let path = &after_prefix[..quote_index];
        if path.is_empty() || path.contains(['"', '<', '>', '\r', '\n']) {
            return None;
        }
        let after_quote = &after_prefix[quote_index + 1..];
        let after_tag = after_quote.strip_prefix(" />")?;
        paths.push(normalize_rendered_reference(path).ok()?);

        rest = trim_start_ascii_whitespace(after_tag);
        if rest.is_empty() {
            return Some(paths);
        }
    }
}

/// Extract Anki-compatible media paths used by note fields and card templates.
pub fn referenced_paths(deck: &CanonicalDeck) -> BTreeSet<String> {
    collect_references(deck).paths
}

/// Extract media paths from rendered Anki-compatible field/template text.
///
/// This intentionally centralizes today's string scanner so future structured media
/// references can replace the extraction internals without changing callers.
pub fn extract_media_references_from_rendered_field(text: &str) -> BTreeSet<String> {
    let mut collected = CollectedReferences::default();
    extract_from_text(text, "rendered content", &mut collected);
    collected.paths
}

/// Compute a lowercase SHA-256 hex digest.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Whether declaration hashes are mandatory or optional development metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaHashPolicy {
    Required,
    Optional,
}

/// Validate portable declaration paths without imposing a release hash policy.
pub fn validate_paths(deck: &CanonicalDeck) -> Result<(), MediaValidationReport> {
    let errors = deck
        .media
        .values()
        .filter(|media| active_media(deck, &media.id))
        .filter_map(|media| {
            SafeRelativePath::new(&media.path)
                .err()
                .map(|error| MediaValidationError {
                    kind: MediaValidationErrorKind::UnsafePath,
                    path: media.path.clone(),
                    message: format!(
                        "media entry {} has unsafe non-portable path {:?}: {error}",
                        media.id, media.path
                    ),
                })
        })
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(MediaValidationReport { errors })
    }
}

/// Validate portable declaration paths and canonical lowercase SHA-256 syntax.
pub fn validate_declarations(
    deck: &CanonicalDeck,
    hash_policy: MediaHashPolicy,
) -> Result<(), MediaValidationReport> {
    let errors = declaration_errors(deck, hash_policy);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(MediaValidationReport { errors })
    }
}

/// Return whether a hash is exactly 64 lowercase hexadecimal characters.
pub fn is_canonical_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

/// Validate content hashes using caller-supplied media asset bytes.
pub fn validate_hashes(
    deck: &CanonicalDeck,
    assets: &BTreeMap<String, Vec<u8>>,
) -> Result<(), MediaValidationReport> {
    let mut errors = Vec::new();

    for media in deck
        .media
        .values()
        .filter(|media| active_media(deck, &media.id))
    {
        if media.sha256.is_empty() {
            errors.push(MediaValidationError {
                kind: MediaValidationErrorKind::EmptyHash,
                path: media.path.clone(),
                message: format!("media entry {} ({}) has empty sha256", media.id, media.path),
            });
            continue;
        }
        if !is_canonical_sha256(&media.sha256) {
            errors.push(MediaValidationError {
                kind: MediaValidationErrorKind::InvalidHash,
                path: media.path.clone(),
                message: format!(
                    "media entry {} ({}) sha256 must be exactly 64 lowercase hexadecimal characters",
                    media.id, media.path
                ),
            });
            continue;
        }
        let Some(bytes) = assets.get(&media.path) else {
            errors.push(MediaValidationError {
                kind: MediaValidationErrorKind::MissingAsset,
                path: media.path.clone(),
                message: format!(
                    "media entry {} ({}) is declared but the file was not supplied for hash validation",
                    media.id, media.path
                ),
            });
            continue;
        };
        let actual = sha256_hex(bytes);
        if actual != media.sha256 {
            errors.push(MediaValidationError {
                kind: MediaValidationErrorKind::HashMismatch,
                path: media.path.clone(),
                message: format!(
                    "media entry {} ({}) sha256 mismatch: expected {}, actual {}",
                    media.id, media.path, media.sha256, actual
                ),
            });
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(MediaValidationReport { errors })
    }
}

/// Build a media reference report with missing declarations as errors and unused declarations as warnings.
pub fn reference_report(deck: &CanonicalDeck) -> MediaReferenceReport {
    let collected = collect_references(deck);
    let used = collected.paths;
    let declared = deck
        .media
        .values()
        .filter(|media| active_media(deck, &media.id))
        .map(|media| media.path.clone())
        .collect::<BTreeSet<_>>();
    let mut errors = declaration_errors(deck, MediaHashPolicy::Optional);
    errors.extend(collected.errors);
    errors.extend(structured_image_reference_errors(deck));
    let mut warnings = Vec::new();

    for path in used.difference(&declared) {
        errors.push(MediaValidationError {
            kind: MediaValidationErrorKind::MissingReference,
            path: path.clone(),
            message: format!("media path {path} is used but not declared"),
        });
    }

    for path in declared.difference(&used) {
        warnings.push(MediaValidationError {
            kind: MediaValidationErrorKind::UnusedReference,
            path: path.clone(),
            message: format!("media path {path} is declared but not used"),
        });
    }

    MediaReferenceReport { errors, warnings }
}

fn declaration_errors(
    deck: &CanonicalDeck,
    hash_policy: MediaHashPolicy,
) -> Vec<MediaValidationError> {
    let mut errors = Vec::new();
    let mut paths = BTreeMap::<&str, (&str, &str)>::new();
    for media in deck
        .media
        .values()
        .filter(|media| active_media(deck, &media.id))
    {
        if let Err(error) = SafeRelativePath::new(&media.path) {
            errors.push(MediaValidationError {
                kind: MediaValidationErrorKind::UnsafePath,
                path: media.path.clone(),
                message: format!(
                    "media entry {} has unsafe non-portable path {:?}: {error}",
                    media.id, media.path
                ),
            });
        }
        if let Some((previous_id, previous_hash)) =
            paths.insert(&media.path, (media.id.as_str(), &media.sha256))
            && previous_hash != media.sha256
        {
            errors.push(MediaValidationError {
                kind: MediaValidationErrorKind::PathCollision,
                path: media.path.clone(),
                message: format!(
                    "media declarations {previous_id} and {} collide at output path {:?} with different hashes",
                    media.id, media.path
                ),
            });
        }
        if media.sha256.is_empty() {
            if hash_policy == MediaHashPolicy::Required {
                errors.push(MediaValidationError {
                    kind: MediaValidationErrorKind::EmptyHash,
                    path: media.path.clone(),
                    message: format!("media entry {} ({}) has empty sha256", media.id, media.path),
                });
            }
        } else if !is_canonical_sha256(&media.sha256) {
            errors.push(MediaValidationError {
                kind: MediaValidationErrorKind::InvalidHash,
                path: media.path.clone(),
                message: format!(
                    "media entry {} ({}) sha256 must be exactly 64 lowercase hexadecimal characters",
                    media.id, media.path
                ),
            });
        }
    }
    errors
}

#[derive(Default)]
struct CollectedReferences {
    paths: BTreeSet<String>,
    errors: Vec<MediaValidationError>,
}

fn collect_references(deck: &CanonicalDeck) -> CollectedReferences {
    let mut collected = CollectedReferences::default();
    if deck
        .tombstones
        .blocking(&TombstoneAddress::DeckDescription)
        .is_none()
    {
        extract_from_text(&deck.description, "deck.description", &mut collected);
    }
    for (note_id, note) in &deck.notes {
        if deck
            .tombstones
            .blocking(&TombstoneAddress::Note {
                note_id: note_id.clone(),
            })
            .is_some()
        {
            continue;
        }
        for (field_id, value) in &note.fields {
            if deck
                .tombstones
                .blocking(&TombstoneAddress::NoteField {
                    note_id: note_id.clone(),
                    field_id: field_id.clone(),
                })
                .is_some()
            {
                continue;
            }
            match value {
                FieldValue::Scalar(value) => extract_from_text(
                    value,
                    &DeckPath::NoteField {
                        note_id: note_id.clone(),
                        field_id: field_id.clone(),
                    }
                    .to_string(),
                    &mut collected,
                ),
                FieldValue::Images(images) => {
                    for image in images {
                        if let Some(media) = deck.media.get(&image.media_id)
                            && active_media(deck, &image.media_id)
                        {
                            collected.paths.insert(media.path.clone());
                        }
                    }
                }
                FieldValue::Message(_) => {}
            }
        }
    }
    for (note_type_id, note_type) in &deck.note_types {
        if deck
            .tombstones
            .blocking(&TombstoneAddress::NoteType {
                note_type_id: note_type_id.clone(),
            })
            .is_some()
        {
            continue;
        }
        extract_from_text(
            &note_type.styling,
            &format!("note_types.{note_type_id}.styling"),
            &mut collected,
        );
        for template in &note_type.card_templates {
            if deck
                .tombstones
                .blocking(&TombstoneAddress::CardTemplate {
                    note_type_id: note_type_id.clone(),
                    template_id: template.id.clone(),
                })
                .is_some()
            {
                continue;
            }
            extract_from_text(
                &template.question_format,
                &format!(
                    "note_types.{note_type_id}.card_templates.{}.question_format",
                    template.id
                ),
                &mut collected,
            );
            extract_from_text(
                &template.answer_format,
                &format!(
                    "note_types.{note_type_id}.card_templates.{}.answer_format",
                    template.id
                ),
                &mut collected,
            );
        }
    }
    collected
}

fn structured_image_reference_errors(deck: &CanonicalDeck) -> Vec<MediaValidationError> {
    let mut errors = Vec::new();
    for (note_id, note) in &deck.notes {
        if deck
            .tombstones
            .blocking(&TombstoneAddress::Note {
                note_id: note_id.clone(),
            })
            .is_some()
        {
            continue;
        }
        for (field_id, value) in &note.fields {
            if deck
                .tombstones
                .blocking(&TombstoneAddress::NoteField {
                    note_id: note_id.clone(),
                    field_id: field_id.clone(),
                })
                .is_some()
            {
                continue;
            }
            let FieldValue::Images(images) = value else {
                continue;
            };
            let field_path = DeckPath::NoteField {
                note_id: note_id.clone(),
                field_id: field_id.clone(),
            }
            .to_string();
            for image in images {
                if !deck.media.contains_key(&image.media_id) || !active_media(deck, &image.media_id)
                {
                    errors.push(MediaValidationError {
                        kind: MediaValidationErrorKind::UnknownMediaId,
                        path: field_path.clone(),
                        message: format!(
                            "unknown media id `{}` referenced in field `{field_path}`",
                            image.media_id
                        ),
                    });
                }
            }
        }
    }
    errors
}

fn active_media(deck: &CanonicalDeck, media_id: &StableId) -> bool {
    deck.tombstones
        .blocking(&TombstoneAddress::MediaReference {
            media_id: media_id.clone(),
        })
        .is_none()
}

/// Validate that every used media path is declared.
pub fn validate_references(deck: &CanonicalDeck) -> Result<(), MediaValidationReport> {
    let report = reference_report(deck);
    if report.errors.is_empty() {
        Ok(())
    } else {
        Err(MediaValidationReport {
            errors: report.errors,
        })
    }
}

fn trim_ascii_whitespace(value: &str) -> &str {
    value.trim_matches(|ch: char| ch.is_ascii_whitespace())
}

fn trim_start_ascii_whitespace(value: &str) -> &str {
    value.trim_start_matches(|ch: char| ch.is_ascii_whitespace())
}

fn extract_from_text(text: &str, context: &str, collected: &mut CollectedReferences) {
    extract_sound_references(text, context, collected);
    extract_attribute_references(text, "src", context, collected);
    extract_attribute_references(text, "href", context, collected);
    extract_css_url_references(text, context, collected);
}

fn extract_sound_references(text: &str, context: &str, collected: &mut CollectedReferences) {
    let mut rest = text;
    while let Some(start) = rest.find("[sound:") {
        rest = &rest[start + "[sound:".len()..];
        let Some(end) = rest.find(']') else {
            break;
        };
        insert_media_path(rest[..end].trim(), context, collected);
        rest = &rest[end + 1..];
    }
}

fn extract_attribute_references(
    text: &str,
    attribute: &str,
    context: &str,
    collected: &mut CollectedReferences,
) {
    let bytes = text.as_bytes();
    let attribute = attribute.as_bytes();
    let mut search = 0;
    while search + attribute.len() <= bytes.len() {
        let Some(relative) = bytes[search..]
            .windows(attribute.len())
            .position(|candidate| candidate.eq_ignore_ascii_case(attribute))
        else {
            break;
        };
        let start = search + relative;
        let boundary_before = start == 0
            || !matches!(bytes[start - 1], b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-');
        let mut cursor = start + attribute.len();
        let boundary_after = cursor == bytes.len()
            || !matches!(bytes[cursor], b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-');
        if !boundary_before || !boundary_after {
            search = start + attribute.len();
            continue;
        }
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if bytes.get(cursor) != Some(&b'=') {
            search = start + attribute.len();
            continue;
        }
        cursor += 1;
        let Some((path, consumed)) =
            consume_media_path(&text[cursor..], |ch| ch.is_whitespace() || ch == '>')
        else {
            break;
        };
        insert_media_path(path, context, collected);
        search = cursor + consumed;
    }
}

fn extract_css_url_references(text: &str, context: &str, collected: &mut CollectedReferences) {
    let mut rest = text;
    while let Some(start) = find_ascii_case_insensitive(rest, "url(") {
        rest = &rest[start + "url(".len()..];
        let Some((path, consumed)) = consume_media_path(rest, |ch| ch == ')') else {
            break;
        };
        insert_media_path(path, context, collected);
        rest = &rest[consumed..];
    }
}

fn find_ascii_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
    haystack
        .as_bytes()
        .windows(needle.len())
        .position(|candidate| candidate.eq_ignore_ascii_case(needle.as_bytes()))
}

fn consume_media_path(rest: &str, unquoted_end: impl Fn(char) -> bool) -> Option<(&str, usize)> {
    let trimmed = rest.trim_start();
    let consumed_whitespace = rest.len() - trimmed.len();
    let first = trimmed.chars().next()?;
    let (path, consumed) = if first == '"' || first == '\'' {
        let quote_len = first.len_utf8();
        let after_quote = &trimmed[quote_len..];
        let end = after_quote.find(first)?;
        (&after_quote[..end], quote_len + end + quote_len)
    } else {
        let end = trimmed.find(unquoted_end).unwrap_or(trimmed.len());
        (&trimmed[..end], end)
    };
    Some((path.trim(), consumed_whitespace + consumed))
}

fn insert_media_path(path: &str, context: &str, collected: &mut CollectedReferences) {
    if path.is_empty() || path.starts_with('#') || has_uri_scheme(path) {
        return;
    }
    match normalize_rendered_reference(path) {
        Ok(path) => {
            collected.paths.insert(path);
        }
        Err(reason) => collected.errors.push(MediaValidationError {
            kind: MediaValidationErrorKind::InvalidReferenceEncoding,
            path: context.to_owned(),
            message: format!(
                "media reference {path:?} at {context} cannot round trip to a safe declared path: {reason}"
            ),
        }),
    }
}

fn normalize_rendered_reference(path: &str) -> Result<String, String> {
    let html_decoded = decode_html_attribute_entities(path)?;
    let decoded = percent_decode_utf8(&html_decoded)?;
    SafeRelativePath::new(&decoded).map_err(|error| error.to_string())?;
    Ok(decoded)
}

fn percent_decode_utf8(value: &str) -> Result<String, String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return Err("incomplete percent escape".to_owned());
            }
            let high =
                hex_value(bytes[index + 1]).ok_or_else(|| "invalid percent escape".to_owned())?;
            let low =
                hex_value(bytes[index + 2]).ok_or_else(|| "invalid percent escape".to_owned())?;
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).map_err(|_| "percent-decoded path is not UTF-8".to_owned())
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn decode_html_attribute_entities(value: &str) -> Result<String, String> {
    let mut decoded = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(index) = rest.find('&') {
        decoded.push_str(&rest[..index]);
        rest = &rest[index..];
        let Some(end) = rest.find(';') else {
            decoded.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        let character = match entity {
            "amp" => '&',
            "quot" => '"',
            "apos" | "#39" => '\'',
            "lt" => '<',
            "gt" => '>',
            _ if entity.starts_with("#x") || entity.starts_with("#X") => {
                let value = u32::from_str_radix(&entity[2..], 16)
                    .map_err(|_| format!("invalid HTML entity &{entity};"))?;
                char::from_u32(value).ok_or_else(|| format!("invalid HTML entity &{entity};"))?
            }
            _ if entity.starts_with('#') => {
                let value = entity[1..]
                    .parse::<u32>()
                    .map_err(|_| format!("invalid HTML entity &{entity};"))?;
                char::from_u32(value).ok_or_else(|| format!("invalid HTML entity &{entity};"))?
            }
            _ => return Err(format!("unsupported or ambiguous HTML entity &{entity};")),
        };
        decoded.push(character);
        rest = &rest[end + 1..];
    }
    decoded.push_str(rest);
    Ok(decoded)
}

fn has_uri_scheme(path: &str) -> bool {
    let Some(colon) = path.find(':') else {
        return false;
    };
    let scheme = &path[..colon];
    !scheme.is_empty()
        && scheme
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.'))
}

/// A media reference validation report with errors and warnings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaReferenceReport {
    pub errors: Vec<MediaValidationError>,
    pub warnings: Vec<MediaValidationError>,
}

impl MediaReferenceReport {
    /// Returns true when the report contains at least one warning of the given kind.
    pub fn has_warning_kind(&self, kind: MediaValidationErrorKind) -> bool {
        self.warnings.iter().any(|error| error.kind == kind)
    }
}

/// A media validation error report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaValidationReport {
    pub errors: Vec<MediaValidationError>,
}

impl MediaValidationReport {
    /// Returns true when the report contains at least one error of the given kind.
    pub fn has_kind(&self, kind: MediaValidationErrorKind) -> bool {
        self.errors.iter().any(|error| error.kind == kind)
    }
}

impl fmt::Display for MediaValidationReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, error) in self.errors.iter().enumerate() {
            if index > 0 {
                writeln!(f)?;
            }
            write!(f, "{}: {}", error.path, error.message)?;
        }
        Ok(())
    }
}

impl std::error::Error for MediaValidationReport {}

/// One media validation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaValidationError {
    pub kind: MediaValidationErrorKind,
    pub path: String,
    pub message: String,
}

/// Machine-readable media validation error kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaValidationErrorKind {
    MissingReference,
    UnusedReference,
    UnknownMediaId,
    MissingAsset,
    EmptyHash,
    HashMismatch,
    InvalidHash,
    UnsafePath,
    InvalidReferenceEncoding,
    PathCollision,
}
