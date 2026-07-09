//! Strict YAML preflight checks shared by all maintainer-source codecs.

use std::fmt;

use serde::de::{self, DeserializeSeed, EnumAccess, MapAccess, SeqAccess, VariantAccess, Visitor};
use serde_yaml::{Mapping, Value};

/// Reject duplicate keys anywhere in one YAML document before a typed decoder
/// can deserialize a mapping into an overwrite-on-insert map.
pub fn reject_duplicate_keys(input: &str) -> Result<(), serde_yaml::Error> {
    StrictYamlSeed {
        path: String::new(),
    }
    .deserialize(serde_yaml::Deserializer::from_str(input))
}

struct StrictYamlSeed {
    path: String,
}

impl<'de> DeserializeSeed<'de> for StrictYamlSeed {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_any(StrictYamlVisitor { path: self.path })
    }
}

struct StrictYamlVisitor {
    path: String,
}

impl<'de> Visitor<'de> for StrictYamlVisitor {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a YAML value")
    }

    fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_i64<E>(self, _value: i64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_u64<E>(self, _value: u64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E> {
        Ok(())
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

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        StrictYamlSeed { path: self.path }.deserialize(deserializer)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        StrictYamlSeed { path: self.path }.deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut index = 0;
        while sequence
            .next_element_seed(StrictYamlSeed {
                path: sequence_path(&self.path, index),
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
            if seen.insert(key, Value::Null).is_some() {
                return Err(de::Error::custom(format!(
                    "duplicate key {key_name:?} at schema path {key_path}"
                )));
            }
            map.next_value_seed(StrictYamlSeed { path: key_path })?;
        }
        Ok(())
    }

    fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
    where
        A: EnumAccess<'de>,
    {
        let (_tag, value) = data.variant::<String>()?;
        value.newtype_variant_seed(StrictYamlSeed { path: self.path })
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
