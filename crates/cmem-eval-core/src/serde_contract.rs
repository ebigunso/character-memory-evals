use serde::de::{DeserializeSeed, Error as _, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::collections::HashSet;
use std::fmt;

#[derive(Deserialize)]
struct SchemaVersionEnvelope {
    schema_version: Option<String>,
}

/// Reads only the schema-version envelope from a raw JSON document.
///
/// Callers use this for version dispatch, then deserialize the authoritative
/// versioned contract from the same raw source. Keeping the dispatch probe
/// streaming preserves duplicate-key errors that a `serde_json::Value` hop
/// would otherwise collapse.
pub fn schema_version_from_str(raw: &str) -> serde_json::Result<Option<String>> {
    serde_json::from_str::<SchemaVersionEnvelope>(raw).map(|envelope| envelope.schema_version)
}

/// Rejects duplicate object keys at every depth without materializing the
/// document through `serde_json::Value`.
pub fn reject_duplicate_json_keys(raw: &str) -> serde_json::Result<()> {
    let mut deserializer = serde_json::Deserializer::from_str(raw);
    NoDuplicateKeys.deserialize(&mut deserializer)?;
    deserializer.end()
}

struct NoDuplicateKeys;

impl<'de> DeserializeSeed<'de> for NoDuplicateKeys {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(NoDuplicateKeysVisitor)
    }
}

struct NoDuplicateKeysVisitor;

impl<'de> Visitor<'de> for NoDuplicateKeysVisitor {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("any JSON value without duplicate object keys")
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

    fn visit_str<E>(self, _value: &str) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_string<E>(self, _value: String) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        NoDuplicateKeys.deserialize(deserializer)
    }

    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        NoDuplicateKeys.deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while sequence.next_element_seed(NoDuplicateKeys)?.is_some() {}
        Ok(())
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut keys = HashSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key.clone()) {
                return Err(A::Error::custom(format!(
                    "duplicate JSON object key {key:?}"
                )));
            }
            map.next_value_seed(NoDuplicateKeys)?;
        }
        Ok(())
    }
}

/// Deserializes a field whose key is required by the wire contract while its
/// value may explicitly be `null`.
pub fn required_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize, PartialEq)]
    struct RequiredNullableField {
        #[serde(deserialize_with = "required_option")]
        value: Option<u64>,
    }

    #[test]
    fn required_option_accepts_null_and_value_but_rejects_absence() {
        assert_eq!(
            serde_json::from_value::<RequiredNullableField>(serde_json::json!({
                "value": null
            }))
            .unwrap(),
            RequiredNullableField { value: None }
        );
        assert_eq!(
            serde_json::from_value::<RequiredNullableField>(serde_json::json!({
                "value": 7
            }))
            .unwrap(),
            RequiredNullableField { value: Some(7) }
        );
        let error = serde_json::from_value::<RequiredNullableField>(serde_json::json!({}))
            .unwrap_err()
            .to_string();
        assert!(error.contains("missing field `value`"), "{error}");
    }

    #[test]
    fn schema_version_probe_ignores_payload_but_rejects_duplicate_version() {
        assert_eq!(
            schema_version_from_str(r#"{"schema_version":"2.0.0","payload":{"key":1}}"#).unwrap(),
            Some("2.0.0".to_string())
        );

        let error =
            schema_version_from_str(r#"{"schema_version":"2.0.0","schema_version":"2.0.0"}"#)
                .unwrap_err()
                .to_string();
        assert!(
            error.contains("duplicate field `schema_version`"),
            "{error}"
        );
    }

    #[test]
    fn duplicate_key_validator_rejects_root_and_nested_duplicates() {
        reject_duplicate_json_keys(r#"{"root":1,"nested":{"key":2},"list":[{"id":3}]}"#).unwrap();
        reject_duplicate_json_keys(r#"{"latency_ms":340282366920938463463374607431768211455}"#)
            .unwrap();

        for raw in [
            r#"{"root":1,"root":2}"#,
            r#"{"nested":{"key":1,"key":2}}"#,
            r#"[{"id":1,"id":2}]"#,
        ] {
            let error = reject_duplicate_json_keys(raw).unwrap_err().to_string();
            assert!(error.contains("duplicate JSON object key"), "{error}");
        }
    }
}
