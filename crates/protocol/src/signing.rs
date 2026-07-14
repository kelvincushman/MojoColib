//! Registry-bound Ed25519 envelopes over canonical contract payloads.

use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::{
    ProtocolError, SCHEMA_VERSION_V1, canonical,
    ids::{ActorId, KeyId, MessageId},
};

const SIGNING_DOMAIN_V1: &str = "mojocolib:signed-envelope:v1";

/// A payload that can be placed in a signed V1 envelope.
pub trait SignedContract: Serialize {
    const CONTRACT_TYPE: &'static str;

    /// Validates the payload at the envelope's authoritative verification time.
    fn validate_signed_contract(&self, now_unix_ms: u64) -> Result<(), ProtocolError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignerKind {
    Customer,
    Provider,
    Worker,
    Operator,
    System,
}

/// An active key record obtained from the trusted identity registry, never from the envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustedEd25519KeyV1 {
    pub schema_version: u16,
    pub key_id: KeyId,
    pub signer_kind: SignerKind,
    pub signer_id: ActorId,
    pub public_key_base64: String,
    pub valid_from_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}

impl TrustedEd25519KeyV1 {
    /// Returns the validated Ed25519 public-key material from this registry record.
    ///
    /// This deliberately does not check the record's validity interval. Callers
    /// that use a key for signature verification must still use
    /// [`SignedEnvelopeV1::verify_with_registry_key`].
    pub fn public_key_bytes(&self) -> Result<[u8; 32], ProtocolError> {
        if self.schema_version != SCHEMA_VERSION_V1 {
            return Err(ProtocolError::UnsupportedSchemaVersion {
                actual: self.schema_version,
                expected: SCHEMA_VERSION_V1,
            });
        }
        let key_bytes = STANDARD_NO_PAD
            .decode(&self.public_key_base64)
            .map_err(|_| ProtocolError::InvalidSignature {
                field: "registry public key",
            })?;
        let key_bytes: [u8; 32] =
            key_bytes
                .try_into()
                .map_err(|_| ProtocolError::InvalidSignature {
                    field: "registry public key",
                })?;
        VerifyingKey::from_bytes(&key_bytes).map_err(|_| ProtocolError::InvalidSignature {
            field: "registry public key",
        })?;
        Ok(key_bytes)
    }

    fn verifying_key(&self, now_unix_ms: u64) -> Result<VerifyingKey, ProtocolError> {
        if now_unix_ms < self.valid_from_unix_ms || now_unix_ms >= self.expires_at_unix_ms {
            return Err(ProtocolError::InvalidSignature {
                field: "inactive registry key",
            });
        }
        let key_bytes = self.public_key_bytes()?;
        VerifyingKey::from_bytes(&key_bytes).map_err(|_| ProtocolError::InvalidSignature {
            field: "registry public key",
        })
    }
}

#[derive(Serialize)]
struct SignatureCommitmentV1<'a, T> {
    domain: &'static str,
    schema_version: u16,
    contract_type: &'a str,
    message_id: &'a MessageId,
    sequence: u64,
    issued_at_unix_ms: u64,
    expires_at_unix_ms: Option<u64>,
    key_id: &'a KeyId,
    signer_kind: SignerKind,
    signer_id: &'a ActorId,
    payload: &'a T,
}

/// Signer identity and signature bytes. The public key must come from a trusted registry lookup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ed25519SignatureV1 {
    pub key_id: KeyId,
    pub signer_kind: SignerKind,
    pub signer_id: ActorId,
    pub signature_base64: String,
}

/// A typed, replay-identifiable payload signed over its canonical representation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedEnvelopeV1<T> {
    pub schema_version: u16,
    pub contract_type: String,
    pub message_id: MessageId,
    pub sequence: u64,
    pub issued_at_unix_ms: u64,
    pub expires_at_unix_ms: Option<u64>,
    pub payload: T,
    pub signature: Ed25519SignatureV1,
}

impl<T> SignedEnvelopeV1<T>
where
    T: SignedContract,
{
    /// Produces the domain-separated bytes covered by the signature.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, ProtocolError> {
        canonical::to_vec(&SignatureCommitmentV1 {
            domain: SIGNING_DOMAIN_V1,
            schema_version: self.schema_version,
            contract_type: &self.contract_type,
            message_id: &self.message_id,
            sequence: self.sequence,
            issued_at_unix_ms: self.issued_at_unix_ms,
            expires_at_unix_ms: self.expires_at_unix_ms,
            key_id: &self.signature.key_id,
            signer_kind: self.signature.signer_kind,
            signer_id: &self.signature.signer_id,
            payload: &self.payload,
        })
    }

    /// Verifies identity binding, replay metadata, payload invariants and the Ed25519 signature.
    ///
    /// The caller must also atomically record `message_id` and the monotonic sequence for the
    /// signer. This method rejects a sequence below `minimum_sequence` but does not own storage.
    pub fn verify_with_registry_key(
        &self,
        trusted_key: &TrustedEd25519KeyV1,
        now_unix_ms: u64,
        minimum_sequence: u64,
    ) -> Result<(), ProtocolError> {
        if self.schema_version != SCHEMA_VERSION_V1 {
            return Err(ProtocolError::UnsupportedSchemaVersion {
                actual: self.schema_version,
                expected: SCHEMA_VERSION_V1,
            });
        }
        if self.contract_type != T::CONTRACT_TYPE {
            return Err(ProtocolError::InvalidSignature {
                field: "contract type",
            });
        }
        if self.sequence == 0 || self.sequence < minimum_sequence {
            return Err(ProtocolError::InvalidSignature {
                field: "replayed sequence",
            });
        }
        if self.issued_at_unix_ms > now_unix_ms {
            return Err(ProtocolError::InvalidSignature {
                field: "future issue time",
            });
        }
        if self
            .expires_at_unix_ms
            .is_some_and(|expiry| expiry <= self.issued_at_unix_ms || now_unix_ms >= expiry)
        {
            return Err(ProtocolError::InvalidSignature {
                field: "expired envelope",
            });
        }
        if self.signature.key_id != trusted_key.key_id
            || self.signature.signer_kind != trusted_key.signer_kind
            || self.signature.signer_id != trusted_key.signer_id
        {
            return Err(ProtocolError::InvalidSignature {
                field: "registry identity binding",
            });
        }
        self.payload.validate_signed_contract(now_unix_ms)?;

        let signature_bytes = STANDARD_NO_PAD
            .decode(&self.signature.signature_base64)
            .map_err(|_| ProtocolError::InvalidSignature { field: "signature" })?;
        let signature = Signature::from_slice(&signature_bytes)
            .map_err(|_| ProtocolError::InvalidSignature { field: "signature" })?;
        trusted_key
            .verifying_key(now_unix_ms)?
            .verify_strict(&self.signing_bytes()?, &signature)
            .map_err(|_| ProtocolError::InvalidSignature { field: "signature" })
    }
}

#[cfg(test)]
mod tests {
    use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
    use ed25519_dalek::{Signer as _, SigningKey};
    use serde::Serialize;

    use super::{
        Ed25519SignatureV1, SignedContract, SignedEnvelopeV1, SignerKind, TrustedEd25519KeyV1,
    };
    use crate::{
        ProtocolError, SCHEMA_VERSION_V1,
        ids::{ActorId, KeyId, MessageId},
    };

    #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
    struct Payload {
        value: u32,
    }

    impl SignedContract for Payload {
        const CONTRACT_TYPE: &'static str = "test_payload_v1";

        fn validate_signed_contract(&self, _now_unix_ms: u64) -> Result<(), ProtocolError> {
            if self.value == 0 {
                Err(ProtocolError::Invariant("test value is zero".into()))
            } else {
                Ok(())
            }
        }
    }

    fn signed_fixture() -> (SignedEnvelopeV1<Payload>, TrustedEd25519KeyV1) {
        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
        let registry_key = TrustedEd25519KeyV1 {
            schema_version: SCHEMA_VERSION_V1,
            key_id: KeyId::new("key-1").unwrap(),
            signer_kind: SignerKind::Worker,
            signer_id: ActorId::new("worker-1").unwrap(),
            public_key_base64: STANDARD_NO_PAD.encode(signing_key.verifying_key().as_bytes()),
            valid_from_unix_ms: 1_000,
            expires_at_unix_ms: 10_000,
        };
        let mut envelope = SignedEnvelopeV1 {
            schema_version: SCHEMA_VERSION_V1,
            contract_type: Payload::CONTRACT_TYPE.into(),
            message_id: MessageId::new("message-1").unwrap(),
            sequence: 1,
            issued_at_unix_ms: 2_000,
            expires_at_unix_ms: Some(9_000),
            payload: Payload { value: 42 },
            signature: Ed25519SignatureV1 {
                key_id: registry_key.key_id.clone(),
                signer_kind: registry_key.signer_kind,
                signer_id: registry_key.signer_id.clone(),
                signature_base64: String::new(),
            },
        };
        let signature = signing_key.sign(&envelope.signing_bytes().unwrap());
        envelope.signature.signature_base64 = STANDARD_NO_PAD.encode(signature.to_bytes());
        (envelope, registry_key)
    }

    #[test]
    fn rejects_payload_or_identity_tampering() {
        let (mut envelope, registry_key) = signed_fixture();
        assert_eq!(
            envelope.verify_with_registry_key(&registry_key, 3_000, 1),
            Ok(())
        );
        envelope.payload.value += 1;
        assert!(
            envelope
                .verify_with_registry_key(&registry_key, 3_000, 1)
                .is_err()
        );

        let (mut envelope, registry_key) = signed_fixture();
        envelope.signature.signer_id = ActorId::new("worker-2").unwrap();
        assert!(
            envelope
                .verify_with_registry_key(&registry_key, 3_000, 1)
                .is_err()
        );
    }

    #[test]
    fn attacker_supplied_key_cannot_impersonate_registry_identity() {
        let (envelope, mut wrong_registry_key) = signed_fixture();
        let attacker = SigningKey::from_bytes(&[9_u8; 32]);
        wrong_registry_key.public_key_base64 =
            STANDARD_NO_PAD.encode(attacker.verifying_key().as_bytes());
        assert!(
            envelope
                .verify_with_registry_key(&wrong_registry_key, 3_000, 1)
                .is_err()
        );
    }
}
