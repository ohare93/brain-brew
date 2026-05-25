use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use brain_brew_core::CanonicalDeck;
use sha2::{Digest, Sha256};

/// Extract Anki-compatible media paths used by note fields and card templates.
pub fn referenced_paths(deck: &CanonicalDeck) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();

    for note in deck.notes.values() {
        for value in note.fields.values() {
            extract_from_text(value, &mut paths);
        }
    }

    for note_type in deck.note_types.values() {
        extract_from_text(&note_type.styling, &mut paths);
        for template in &note_type.card_templates {
            extract_from_text(&template.question_format, &mut paths);
            extract_from_text(&template.answer_format, &mut paths);
        }
    }

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
            continue;
        }
        let Some(bytes) = assets.get(&media.path) else {
            errors.push(MediaValidationError {
                kind: MediaValidationErrorKind::MissingAsset,
                path: media.path.clone(),
                message: format!(
                    "media asset {} was not supplied for hash validation",
                    media.path
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
                    "media asset {} has sha256 {}, expected {}",
                    media.path, actual, media.sha256
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

/// Validate that every used media path is declared and every declaration is used.
pub fn validate_references(deck: &CanonicalDeck) -> Result<(), MediaValidationReport> {
    let used = referenced_paths(deck);
    let declared = deck
        .media
        .values()
        .map(|media| media.path.clone())
        .collect::<BTreeSet<_>>();
    let mut errors = Vec::new();

    for path in used.difference(&declared) {
        errors.push(MediaValidationError {
            kind: MediaValidationErrorKind::MissingReference,
            path: path.clone(),
            message: format!("media path {path} is used but not declared"),
        });
    }

    for path in declared.difference(&used) {
        errors.push(MediaValidationError {
            kind: MediaValidationErrorKind::UnusedReference,
            path: path.clone(),
            message: format!("media path {path} is declared but not used"),
        });
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(MediaValidationReport { errors })
    }
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

/// A media reference validation report.
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
    MissingAsset,
    HashMismatch,
}
