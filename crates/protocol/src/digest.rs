//! SHA-256 commitments represented in a single canonical wire format.

use core::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use sha2::{Digest as _, Sha256};

use crate::ProtocolError;

const PREFIX: &str = "sha256:";

/// A SHA-256 digest encoded as `sha256:` followed by 64 lowercase hex characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Sha256Digest([u8; 32]);

impl Sha256Digest {
    /// Hashes arbitrary bytes.
    #[must_use]
    pub fn of_bytes(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    /// Returns the raw 32-byte digest.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for Sha256Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{PREFIX}{}", hex::encode(self.0))
    }
}

impl FromStr for Sha256Digest {
    type Err = ProtocolError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let encoded = value
            .strip_prefix(PREFIX)
            .ok_or(ProtocolError::InvalidDigest)?;
        if encoded.len() != 64 || encoded.bytes().any(|byte| !byte.is_ascii_hexdigit()) {
            return Err(ProtocolError::InvalidDigest);
        }
        if encoded.bytes().any(|byte| byte.is_ascii_uppercase()) {
            return Err(ProtocolError::InvalidDigest);
        }
        let mut bytes = [0_u8; 32];
        hex::decode_to_slice(encoded, &mut bytes).map_err(|_| ProtocolError::InvalidDigest)?;
        Ok(Self(bytes))
    }
}

impl Serialize for Sha256Digest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Sha256Digest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::Sha256Digest;

    #[test]
    fn uses_stable_prefixed_lowercase_encoding() {
        let digest = Sha256Digest::of_bytes(b"colibri");
        let encoded = digest.to_string();
        assert_eq!(encoded.len(), 71);
        assert_eq!(encoded.parse::<Sha256Digest>(), Ok(digest));
        assert!(encoded.to_uppercase().parse::<Sha256Digest>().is_err());
    }
}
