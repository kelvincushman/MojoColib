use thiserror::Error;

/// A validation or encoding failure in a public protocol contract.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("unsupported schema version {actual}; expected {expected}")]
    UnsupportedSchemaVersion { actual: u16, expected: u16 },

    #[error("invalid {kind} identifier: {reason}")]
    InvalidIdentifier {
        kind: &'static str,
        reason: &'static str,
    },

    #[error("invalid SHA-256 digest")]
    InvalidDigest,

    #[error("invalid Ed25519 {field}")]
    InvalidSignature { field: &'static str },

    #[error("canonical encoding failed: {0}")]
    CanonicalEncoding(String),

    #[error("strict JSON decoding failed: {0}")]
    StrictJson(String),

    #[error("contract invariant failed: {0}")]
    Invariant(String),

    #[error("arithmetic overflow")]
    ArithmeticOverflow,
}
