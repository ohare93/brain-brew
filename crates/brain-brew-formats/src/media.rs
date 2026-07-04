use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use brain_brew_core::{CanonicalDeck, DeckPath};
use sha2::{Digest, Sha256};

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
        paths.push(path.to_owned());

        rest = trim_start_ascii_whitespace(after_tag);
        if rest.is_empty() {
            return Some(paths);
        }
    }
}

/// Extract Anki-compatible media paths used by note fields and card templates.
pub fn referenced_paths(deck: &CanonicalDeck) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();

    for note in deck.notes.values() {
        for value in note.fields.values() {
            paths.extend(extract_media_references_from_rendered_field(value));
        }
        for images in note.field_images.values() {
            for image in images {
                if let Some(media) = deck.media.get(&image.media_id) {
                    paths.insert(media.path.clone());
                }
            }
        }
    }

    for note_type in deck.note_types.values() {
        paths.extend(extract_media_references_from_rendered_field(
            &note_type.styling,
        ));
        for template in &note_type.card_templates {
            paths.extend(extract_media_references_from_rendered_field(
                &template.question_format,
            ));
            paths.extend(extract_media_references_from_rendered_field(
                &template.answer_format,
            ));
        }
    }

    paths
}

/// Extract media paths from rendered Anki-compatible field/template text.
///
/// This intentionally centralizes today's string scanner so future structured media
/// references can replace the extraction internals without changing callers.
pub fn extract_media_references_from_rendered_field(text: &str) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    extract_from_text(text, &mut paths);
    paths
}

/// Compute a lowercase SHA-256 hex digest.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Validate content hashes using caller-supplied media asset bytes.
pub fn validate_hashes(
    deck: &CanonicalDeck,
    assets: &BTreeMap<String, Vec<u8>>,
) -> Result<(), MediaValidationReport> {
    let mut errors = Vec::new();

    for media in deck.media.values() {
        if media.sha256.is_empty() {
            errors.push(MediaValidationError {
                kind: MediaValidationErrorKind::EmptyHash,
                path: media.path.clone(),
                message: format!("media entry {} ({}) has empty sha256", media.id, media.path),
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
    let used = referenced_paths(deck);
    let declared = deck
        .media
        .values()
        .map(|media| media.path.clone())
        .collect::<BTreeSet<_>>();
    let mut errors = structured_image_reference_errors(deck);
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

fn structured_image_reference_errors(deck: &CanonicalDeck) -> Vec<MediaValidationError> {
    let mut errors = Vec::new();
    for (note_id, note) in &deck.notes {
        for (field_id, images) in &note.field_images {
            let field_path = DeckPath::NoteField {
                note_id: note_id.clone(),
                field_id: field_id.clone(),
            }
            .to_string();
            for image in images {
                if !deck.media.contains_key(&image.media_id) {
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

fn extract_from_text(text: &str, paths: &mut BTreeSet<String>) {
    extract_sound_references(text, paths);
    extract_attribute_references(text, "src=", paths);
    extract_attribute_references(text, "href=", paths);
    extract_css_url_references(text, paths);
}

fn extract_sound_references(text: &str, paths: &mut BTreeSet<String>) {
    let mut rest = text;
    while let Some(start) = rest.find("[sound:") {
        rest = &rest[start + "[sound:".len()..];
        let Some(end) = rest.find(']') else {
            break;
        };
        let path = rest[..end].trim();
        if !path.is_empty() {
            paths.insert(path.to_owned());
        }
        rest = &rest[end + 1..];
    }
}

fn extract_attribute_references(text: &str, attribute: &str, paths: &mut BTreeSet<String>) {
    let mut rest = text;
    while let Some(start) = rest.find(attribute) {
        rest = &rest[start + attribute.len()..];
        let Some((path, consumed)) = consume_media_path(rest, |ch| ch.is_whitespace() || ch == '>')
        else {
            break;
        };
        insert_media_path(path, paths);
        rest = &rest[consumed..];
    }
}

fn extract_css_url_references(text: &str, paths: &mut BTreeSet<String>) {
    let mut rest = text;
    while let Some(start) = rest.find("url(") {
        rest = &rest[start + "url(".len()..];
        let Some((path, consumed)) = consume_media_path(rest, |ch| ch == ')') else {
            break;
        };
        insert_media_path(path, paths);
        rest = &rest[consumed..];
    }
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

fn insert_media_path(path: &str, paths: &mut BTreeSet<String>) {
    if !path.is_empty() && !path.starts_with("#") && !has_uri_scheme(path) {
        paths.insert(path.to_owned());
    }
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
}
