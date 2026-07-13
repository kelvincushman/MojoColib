//! Duplicate-key, unknown-field and floating-point rejection for signed JSON input.

use std::collections::BTreeMap;

use serde::{
    Deserialize, Deserializer,
    de::{DeserializeOwned, Error as _, MapAccess, SeqAccess, Visitor},
};

use crate::{
    ProtocolError,
    signing::{SignedContract, SignedEnvelopeV1},
};

enum StrictValue {
    Null,
    Bool(bool),
    I64(i64),
    U64(u64),
    String(String),
    Array(Vec<Self>),
    Object(BTreeMap<String, Self>),
}

struct StrictValueVisitor;

impl<'de> Visitor<'de> for StrictValueVisitor {
    type Value = StrictValue;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("JSON without duplicate keys or floating-point numbers")
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(StrictValue::Null)
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(StrictValue::Null)
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(StrictValue::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(StrictValue::I64(value))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(StrictValue::U64(value))
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Err(E::custom(
            "floating-point numbers are prohibited in signed V1 JSON",
        ))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(StrictValue::String(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(StrictValue::String(value))
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element::<StrictValue>()? {
            values.push(value);
        }
        Ok(StrictValue::Array(values))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = BTreeMap::new();
        while let Some(key) = map.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(A::Error::custom(format!("duplicate JSON key {key:?}")));
            }
            let value = map.next_value::<StrictValue>()?;
            values.insert(key, value);
        }
        Ok(StrictValue::Object(values))
    }
}

impl<'de> Deserialize<'de> for StrictValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(StrictValueVisitor)
    }
}

impl StrictValue {
    fn into_json(self) -> serde_json::Value {
        match self {
            Self::Null => serde_json::Value::Null,
            Self::Bool(value) => serde_json::Value::Bool(value),
            Self::I64(value) => serde_json::Value::Number(value.into()),
            Self::U64(value) => serde_json::Value::Number(value.into()),
            Self::String(value) => serde_json::Value::String(value),
            Self::Array(values) => {
                serde_json::Value::Array(values.into_iter().map(Self::into_json).collect())
            }
            Self::Object(values) => serde_json::Value::Object(
                values
                    .into_iter()
                    .map(|(key, value)| (key, value.into_json()))
                    .collect(),
            ),
        }
    }
}

/// Parses a signed envelope without allowing duplicate keys, floats or unknown struct fields.
pub fn parse_signed_envelope<T>(bytes: &[u8]) -> Result<SignedEnvelopeV1<T>, ProtocolError>
where
    T: SignedContract + DeserializeOwned,
{
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = StrictValue::deserialize(&mut deserializer)
        .map_err(|error| ProtocolError::StrictJson(error.to_string()))?;
    deserializer
        .end()
        .map_err(|error| ProtocolError::StrictJson(error.to_string()))?;
    serde_json::from_value(value.into_json())
        .map_err(|error| ProtocolError::StrictJson(error.to_string()))
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use super::parse_signed_envelope;
    use crate::{ProtocolError, signing::SignedContract};

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Payload {
        value: u64,
    }

    impl SignedContract for Payload {
        const CONTRACT_TYPE: &'static str = "strict_test_v1";

        fn validate_signed_contract(&self, _now_unix_ms: u64) -> Result<(), ProtocolError> {
            Ok(())
        }
    }

    #[test]
    fn rejects_duplicate_unknown_and_float_fields() {
        let duplicate = br#"{"schema_version":1,"schema_version":1}"#;
        assert!(parse_signed_envelope::<Payload>(duplicate).is_err());

        let unknown = br#"{
            "schema_version":1,
            "contract_type":"strict_test_v1",
            "message_id":"message-1",
            "sequence":1,
            "issued_at_unix_ms":1,
            "expires_at_unix_ms":null,
            "payload":{"value":1,"surprise":true},
            "signature":{"key_id":"key-1","signer_kind":"worker","signer_id":"worker-1","signature_base64":""}
        }"#;
        assert!(parse_signed_envelope::<Payload>(unknown).is_err());

        let floating = br#"{
            "schema_version":1,
            "contract_type":"strict_test_v1",
            "message_id":"message-1",
            "sequence":1,
            "issued_at_unix_ms":1,
            "expires_at_unix_ms":null,
            "payload":{"value":1.0},
            "signature":{"key_id":"key-1","signer_kind":"worker","signer_id":"worker-1","signature_base64":""}
        }"#;
        assert!(parse_signed_envelope::<Payload>(floating).is_err());
    }
}
