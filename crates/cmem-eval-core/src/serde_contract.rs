use serde::{Deserialize, Deserializer};

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
}
