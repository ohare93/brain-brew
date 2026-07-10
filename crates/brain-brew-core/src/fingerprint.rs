use std::fmt;
use std::str::FromStr;

use sha2::{Digest, Sha256};

use crate::{
    AdapterIds, CardTemplate, FieldDefinition, FieldValue, MediaReference, MessageComponent, Note,
    NoteType, StableId, StructuredMessage,
};

/// Canonical entity fingerprint schema version.
pub const ENTITY_FINGERPRINT_SCHEMA_VERSION: u32 = 1;

/// Maintained digest algorithm used by canonical entity fingerprints.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EntityFingerprintAlgorithm {
    Sha256,
}

impl EntityFingerprintAlgorithm {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sha256 => "sha256",
        }
    }
}

/// A validated fingerprint of one complete canonical entity.
///
/// The canonical text form is `sha256:v1:<64 lowercase hex digits>`.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EntityFingerprint {
    algorithm: EntityFingerprintAlgorithm,
    schema_version: u32,
    digest: [u8; 32],
}

impl EntityFingerprint {
    pub fn algorithm(&self) -> EntityFingerprintAlgorithm {
        self.algorithm
    }

    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    fn sha256_v1(digest: [u8; 32]) -> Self {
        Self {
            algorithm: EntityFingerprintAlgorithm::Sha256,
            schema_version: ENTITY_FINGERPRINT_SCHEMA_VERSION,
            digest,
        }
    }
}

impl fmt::Display for EntityFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:v{}:", self.algorithm.as_str(), self.schema_version)?;
        for byte in self.digest {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl FromStr for EntityFingerprint {
    type Err = InvalidEntityFingerprint;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let Some((algorithm, rest)) = value.split_once(':') else {
            return Err(InvalidEntityFingerprint::new(
                value,
                "missing algorithm separator",
            ));
        };
        if algorithm != "sha256" {
            return Err(InvalidEntityFingerprint::new(
                value,
                "unsupported algorithm; expected sha256",
            ));
        }
        let Some((version, digest)) = rest.split_once(':') else {
            return Err(InvalidEntityFingerprint::new(
                value,
                "missing version separator",
            ));
        };
        if version != "v1" {
            return Err(InvalidEntityFingerprint::new(
                value,
                "unsupported schema version; expected v1",
            ));
        }
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(InvalidEntityFingerprint::new(
                value,
                "digest must contain exactly 64 lowercase hexadecimal digits",
            ));
        }
        let mut bytes = [0_u8; 32];
        for (index, pair) in digest.as_bytes().chunks_exact(2).enumerate() {
            bytes[index] = (hex_nibble(pair[0]) << 4) | hex_nibble(pair[1]);
        }
        Ok(Self::sha256_v1(bytes))
    }
}

fn hex_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => unreachable!("validated hexadecimal byte"),
    }
}

/// A malformed, unsupported, or non-canonical entity fingerprint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvalidEntityFingerprint {
    value: String,
    reason: &'static str,
}

impl InvalidEntityFingerprint {
    fn new(value: &str, reason: &'static str) -> Self {
        Self {
            value: value.to_owned(),
            reason,
        }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn reason(&self) -> &str {
        self.reason
    }
}

impl fmt::Display for InvalidEntityFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid entity fingerprint {:?}: {}",
            self.value, self.reason
        )
    }
}

impl std::error::Error for InvalidEntityFingerprint {}

/// Fingerprint a complete note type, including sequence order and adapter configuration.
pub fn fingerprint_note_type(value: &NoteType) -> EntityFingerprint {
    fingerprint("note-type", |encoder| encode_note_type(encoder, value))
}

/// Fingerprint a complete field definition.
pub fn fingerprint_field_definition(value: &FieldDefinition) -> EntityFingerprint {
    fingerprint("field-definition", |encoder| {
        encode_field_definition(encoder, value)
    })
}

/// Fingerprint a complete card template, including adapter configuration.
pub fn fingerprint_card_template(value: &CardTemplate) -> EntityFingerprint {
    fingerprint("card-template", |encoder| {
        encode_card_template(encoder, value)
    })
}

/// Fingerprint a complete note and every semantic field-value variant.
pub fn fingerprint_note(value: &Note) -> EntityFingerprint {
    fingerprint("note", |encoder| encode_note(encoder, value))
}

/// Fingerprint a complete media declaration/reference.
pub fn fingerprint_media_reference(value: &MediaReference) -> EntityFingerprint {
    fingerprint("media-reference", |encoder| encode_media(encoder, value))
}

fn fingerprint(kind: &str, encode: impl FnOnce(&mut CanonicalEncoder)) -> EntityFingerprint {
    let mut encoder = CanonicalEncoder::default();
    encoder.string(
        1,
        &format!("brainbrew:{ENTITY_FINGERPRINT_SCHEMA_VERSION}:{kind}"),
    );
    encode(&mut encoder);
    EntityFingerprint::sha256_v1(Sha256::digest(encoder.bytes).into())
}

#[derive(Default)]
struct CanonicalEncoder {
    bytes: Vec<u8>,
}

impl CanonicalEncoder {
    fn tag(&mut self, tag: u8) {
        self.bytes.push(tag);
    }

    fn length(&mut self, length: usize) {
        self.bytes.extend_from_slice(&(length as u64).to_be_bytes());
    }

    fn string(&mut self, tag: u8, value: &str) {
        self.tag(tag);
        self.length(value.len());
        self.bytes.extend_from_slice(value.as_bytes());
    }

    fn stable_id(&mut self, tag: u8, value: &StableId) {
        self.string(tag, value.as_str());
    }

    fn sequence(&mut self, tag: u8, length: usize, encode: impl FnOnce(&mut Self)) {
        self.tag(tag);
        self.length(length);
        encode(self);
    }

    fn option(&mut self, tag: u8, present: bool, encode: impl FnOnce(&mut Self)) {
        self.tag(tag);
        self.bytes.push(u8::from(present));
        if present {
            encode(self);
        }
    }
}

fn encode_note_type(encoder: &mut CanonicalEncoder, value: &NoteType) {
    encoder.stable_id(2, &value.id);
    encoder.string(3, &value.name);
    encode_string_map(encoder, 4, &value.variables);
    encoder.sequence(5, value.fields.len(), |encoder| {
        for field in &value.fields {
            encoder.tag(1);
            encode_field_definition(encoder, field);
        }
    });
    encoder.sequence(6, value.card_templates.len(), |encoder| {
        for template in &value.card_templates {
            encoder.tag(1);
            encode_card_template(encoder, template);
        }
    });
    encoder.string(7, &value.styling);
    encode_adapter_ids(encoder, 8, &value.adapter_ids);
}

fn encode_field_definition(encoder: &mut CanonicalEncoder, value: &FieldDefinition) {
    encoder.stable_id(2, &value.id);
    encoder.string(3, &value.name);
}

fn encode_card_template(encoder: &mut CanonicalEncoder, value: &CardTemplate) {
    encoder.stable_id(2, &value.id);
    encoder.string(3, &value.name);
    encode_string_map(encoder, 4, &value.variables);
    encoder.string(5, &value.question_format);
    encoder.string(6, &value.answer_format);
    encode_adapter_ids(encoder, 7, &value.adapter_ids);
}

fn encode_note(encoder: &mut CanonicalEncoder, value: &Note) {
    encoder.stable_id(2, &value.id);
    encoder.stable_id(3, &value.note_type_id);
    encode_string_map(encoder, 4, &value.variables);
    encoder.sequence(5, value.fields.len(), |encoder| {
        for (field_id, field_value) in &value.fields {
            encoder.stable_id(1, field_id);
            encode_field_value(encoder, field_value);
        }
    });
    encoder.sequence(6, value.tags.len(), |encoder| {
        for tag in &value.tags {
            encoder.string(1, tag);
        }
    });
    encode_adapter_ids(encoder, 7, &value.adapter_ids);
}

fn encode_media(encoder: &mut CanonicalEncoder, value: &MediaReference) {
    encoder.stable_id(2, &value.id);
    encoder.string(3, &value.path);
    encoder.string(4, &value.sha256);
}

fn encode_field_value(encoder: &mut CanonicalEncoder, value: &FieldValue) {
    match value {
        FieldValue::Scalar(value) => encoder.string(1, value),
        FieldValue::Images(images) => encoder.sequence(2, images.len(), |encoder| {
            for image in images {
                encoder.stable_id(1, &image.media_id);
            }
        }),
        FieldValue::Message(message) => {
            encoder.tag(3);
            encode_message(encoder, message);
        }
    }
}

fn encode_message(encoder: &mut CanonicalEncoder, value: &StructuredMessage) {
    encoder.sequence(1, value.components.len(), |encoder| {
        for component in &value.components {
            encode_message_component(encoder, component);
        }
    });
    encoder.option(2, value.format.is_some(), |encoder| {
        encoder.string(1, value.format.as_deref().expect("present format"));
    });
    encoder.sequence(3, value.variables.len(), |encoder| {
        for (key, component) in &value.variables {
            encoder.string(1, key);
            encode_message_component(encoder, component);
        }
    });
}

fn encode_message_component(encoder: &mut CanonicalEncoder, value: &MessageComponent) {
    match value {
        MessageComponent::Literal(value) => encoder.string(1, value),
        MessageComponent::Text(value) => encoder.string(2, value),
        MessageComponent::FieldRef(value) => encoder.string(3, value),
    }
}

fn encode_string_map(
    encoder: &mut CanonicalEncoder,
    tag: u8,
    values: &std::collections::BTreeMap<String, String>,
) {
    encoder.sequence(tag, values.len(), |encoder| {
        for (key, value) in values {
            encoder.string(1, key);
            encoder.string(2, value);
        }
    });
}

fn encode_adapter_ids(encoder: &mut CanonicalEncoder, tag: u8, values: &AdapterIds) {
    encoder.sequence(tag, values.iter().count(), |encoder| {
        for (key, value) in values.iter() {
            encoder.string(1, key);
            encoder.string(2, value);
        }
    });
}
