//! Validated identifiers used to join protocol records without stringly typed mistakes.

use core::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

use crate::ProtocolError;

fn validate_identifier(kind: &'static str, value: &str) -> Result<(), ProtocolError> {
    if value.is_empty() {
        return Err(ProtocolError::InvalidIdentifier {
            kind,
            reason: "must not be empty",
        });
    }
    if value.len() > 96 {
        return Err(ProtocolError::InvalidIdentifier {
            kind,
            reason: "must be at most 96 bytes",
        });
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err(ProtocolError::InvalidIdentifier {
            kind,
            reason: "contains characters outside [A-Za-z0-9._:-]",
        });
    }
    let windows_drive_prefix = value.as_bytes().get(1) == Some(&b':')
        && value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic);
    if matches!(value, "." | "..")
        || value.starts_with('.')
        || value.ends_with('.')
        || windows_drive_prefix
    {
        return Err(ProtocolError::InvalidIdentifier {
            kind,
            reason: "resembles a filesystem path-control segment",
        });
    }
    Ok(())
}

macro_rules! define_identifier {
    ($name:ident, $kind:literal) => {
        #[doc = concat!("A validated ", $kind, " identifier.")]
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            /// Creates an identifier after enforcing the wire-format bounds.
            pub fn new(value: impl Into<String>) -> Result<Self, ProtocolError> {
                let value = value.into();
                validate_identifier($kind, &value)?;
                Ok(Self(value))
            }

            /// Returns the identifier as text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::new(value).map_err(D::Error::custom)
            }
        }
    };
}

define_identifier!(PipelineId, "pipeline");
define_identifier!(StageId, "stage");
define_identifier!(AttemptId, "attempt");
define_identifier!(ProviderId, "provider");
define_identifier!(WorkerId, "worker");
define_identifier!(CustomerId, "customer");
define_identifier!(ArtifactId, "artifact");
define_identifier!(KeyId, "key");
define_identifier!(QuoteId, "quote");
define_identifier!(ReservationId, "reservation");
define_identifier!(SettlementId, "settlement");
define_identifier!(TemplateId, "template");
define_identifier!(LedgerEventId, "ledger event");
define_identifier!(AuditEventId, "audit event");
define_identifier!(ActorId, "actor");
define_identifier!(MessageId, "message");
define_identifier!(BenchmarkId, "benchmark");

#[cfg(test)]
mod tests {
    use super::{PipelineId, WorkerId};

    #[test]
    fn rejects_path_like_identifiers() {
        assert!(PipelineId::new("../../secret").is_err());
        assert!(WorkerId::new("worker/one").is_err());
        assert!(WorkerId::new("..").is_err());
        assert!(WorkerId::new("C:").is_err());
    }

    #[test]
    fn serialisation_cannot_bypass_validation() {
        let result = serde_json::from_str::<PipelineId>(r#""not valid""#);
        assert!(result.is_err());
    }
}
