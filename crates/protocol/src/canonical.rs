//! RFC 8785 JSON canonicalisation used before hashing and signing.

use serde::Serialize;

use crate::{ProtocolError, digest::Sha256Digest};

/// Serialises a value using the JSON Canonicalization Scheme (RFC 8785).
pub fn to_vec<T>(value: &T) -> Result<Vec<u8>, ProtocolError>
where
    T: Serialize + ?Sized,
{
    serde_jcs::to_vec(value).map_err(|error| ProtocolError::CanonicalEncoding(error.to_string()))
}

/// Computes a SHA-256 commitment over canonical JSON bytes.
pub fn digest<T>(value: &T) -> Result<Sha256Digest, ProtocolError>
where
    T: Serialize + ?Sized,
{
    Ok(Sha256Digest::of_bytes(&to_vec(value)?))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{digest, to_vec};

    #[test]
    fn object_order_does_not_change_commitment() {
        let first = serde_json::json!({"z": 2, "a": 1});
        let mut second = BTreeMap::new();
        second.insert("a", 1);
        second.insert("z", 2);

        assert_eq!(to_vec(&first).unwrap(), br#"{"a":1,"z":2}"#);
        assert_eq!(digest(&first).unwrap(), digest(&second).unwrap());
    }
}
