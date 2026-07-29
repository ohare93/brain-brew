//! Strict YAML preflight checks shared by all maintainer-source codecs.

use std::fmt;

use serde::de::{self, DeserializeSeed, EnumAccess, MapAccess, SeqAccess, VariantAccess, Visitor};
use serde_yaml::{Mapping, Value};

/// Reject duplicate keys anywhere in one YAML document before a typed decoder
/// can deserialize a mapping into an overwrite-on-insert map.
pub fn reject_duplicate_keys(input: &str) -> Result<(), serde_yaml::Error> {
    StrictYamlSeed {
        path: String::new(),
        scalar_policy: None,
    }
    .deserialize(serde_yaml::Deserializer::from_str(input))
}

/// Return the byte offset of an exact, unindented mapping key line.
///
/// Canonical emitters may put either a block value (`key:`) or an inline value
/// (`key: {}`) on the line. Indented scalar content and longer key names do not
/// match.
pub(crate) fn top_level_mapping_key_offset(input: &str, key: &str) -> Option<usize> {
    let marker = format!("{key}:");
    let mut offset = 0;
    for raw_line in input.split_inclusive('\n') {
        let line = raw_line.strip_suffix('\n').unwrap_or(raw_line);
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line == marker
            || line
                .strip_prefix(&marker)
                .is_some_and(|suffix| suffix.starts_with(' '))
        {
            return Some(offset);
        }
        offset += raw_line.len();
    }
    None
}

/// Maintainer-source schema whose intentionally typed scalar positions are
/// exempt from the otherwise string-only YAML scalar rule.
#[derive(Clone, Copy)]
pub enum ScalarPolicy {
    CanonicalDeck,
    Overlay,
    Manifest,
    Lockfile,
    MediaMap,
}

/// Reject YAML booleans, nulls, and numbers before serde can coerce them into
/// schema strings. Only explicitly typed schema positions are permitted.
pub fn reject_unintended_scalars(
    input: &str,
    policy: ScalarPolicy,
) -> Result<(), serde_yaml::Error> {
    StrictYamlSeed {
        path: String::new(),
        scalar_policy: Some(policy),
    }
    .deserialize(serde_yaml::Deserializer::from_str(input))
}

struct StrictYamlSeed {
    path: String,
    scalar_policy: Option<ScalarPolicy>,
}

impl<'de> DeserializeSeed<'de> for StrictYamlSeed {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_any(StrictYamlVisitor {
            path: self.path,
            scalar_policy: self.scalar_policy,
        })
    }
}

struct StrictYamlVisitor {
    path: String,
    scalar_policy: Option<ScalarPolicy>,
}

impl<'de> Visitor<'de> for StrictYamlVisitor {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a YAML value")
    }

    fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.validate_scalar::<E>("boolean")
    }

    fn visit_i64<E>(self, _value: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.validate_scalar::<E>("number")
    }

    fn visit_u64<E>(self, _value: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.validate_scalar::<E>("number")
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.validate_scalar::<E>("number")
    }

    fn visit_char<E>(self, _value: char) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_str<E>(self, _value: &str) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_string<E>(self, _value: String) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_bytes<E>(self, _value: &[u8]) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_byte_buf<E>(self, _value: Vec<u8>) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.validate_scalar::<E>("null")
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        StrictYamlSeed {
            path: self.path,
            scalar_policy: self.scalar_policy,
        }
        .deserialize(deserializer)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.validate_scalar::<E>("null")
    }

    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        StrictYamlSeed {
            path: self.path,
            scalar_policy: self.scalar_policy,
        }
        .deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut index = 0;
        while sequence
            .next_element_seed(StrictYamlSeed {
                path: sequence_path(&self.path, index),
                scalar_policy: self.scalar_policy,
            })?
            .is_some()
        {
            index += 1;
        }
        Ok(())
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut seen = Mapping::new();
        while let Some(key) = map.next_key::<Value>()? {
            let key_name = display_key(&key);
            let key_path = mapping_path(&self.path, &key_name);
            if self.scalar_policy.is_some() && !matches!(key, Value::String(_)) {
                return Err(de::Error::custom(format!(
                    "expected YAML string map key at schema path {key_path}, found {}",
                    scalar_kind(&key)
                )));
            }
            if seen.insert(key, Value::Null).is_some() {
                return Err(de::Error::custom(format!(
                    "duplicate key {key_name:?} at schema path {key_path}"
                )));
            }
            map.next_value_seed(StrictYamlSeed {
                path: key_path,
                scalar_policy: self.scalar_policy,
            })?;
        }
        Ok(())
    }

    fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
    where
        A: EnumAccess<'de>,
    {
        let (_tag, value) = data.variant::<String>()?;
        value.newtype_variant_seed(StrictYamlSeed {
            path: self.path,
            scalar_policy: self.scalar_policy,
        })
    }
}

impl StrictYamlVisitor {
    fn validate_scalar<E>(self, kind: &str) -> Result<(), E>
    where
        E: de::Error,
    {
        let Some(policy) = self.scalar_policy else {
            return Ok(());
        };
        if typed_scalar_is_allowed(policy, &self.path, kind) {
            return Ok(());
        }
        Err(E::custom(format!(
            "expected YAML string at schema path {}, found {kind}",
            if self.path.is_empty() {
                "<root>"
            } else {
                &self.path
            }
        )))
    }
}

fn typed_scalar_is_allowed(policy: ScalarPolicy, path: &str, kind: &str) -> bool {
    if kind == "null" && is_collection_path(path) {
        return true;
    }
    match policy {
        ScalarPolicy::CanonicalDeck | ScalarPolicy::MediaMap => false,
        ScalarPolicy::Overlay => kind == "boolean" && path == "translations.require_complete",
        ScalarPolicy::Manifest => {
            kind == "boolean" && path.starts_with("languages.") && path.ends_with(".source")
        }
        ScalarPolicy::Lockfile => kind == "number" && path == "version",
    }
}

fn is_collection_path(path: &str) -> bool {
    const COLLECTION_KEYS: &[&str] = &[
        "adapter_ids",
        "card_template_order",
        "card_templates",
        "field_order",
        "fields",
        "media",
        "message",
        "note_types",
        "notes",
        "tags",
        "tombstones",
        "variables",
    ];
    COLLECTION_KEYS
        .iter()
        .any(|key| path == *key || path.ends_with(&format!(".{key}")))
}

fn scalar_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Sequence(_) => "sequence",
        Value::Mapping(_) => "mapping",
        Value::Tagged(_) => "tagged value",
    }
}

fn mapping_path(parent: &str, key: &str) -> String {
    if parent.is_empty() {
        key.to_owned()
    } else {
        format!("{parent}.{key}")
    }
}

fn sequence_path(parent: &str, index: usize) -> String {
    if parent.is_empty() {
        format!("[{index}]")
    } else {
        format!("{parent}[{index}]")
    }
}

fn display_key(key: &Value) -> String {
    match key {
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Null => "null".to_owned(),
        _ => serde_yaml::to_string(key)
            .unwrap_or_else(|_| format!("{key:?}"))
            .trim()
            .to_owned(),
    }
}
