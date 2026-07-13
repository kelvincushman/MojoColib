//! Immutable audit records emitted atomically with material state changes.

use serde::{Deserialize, Serialize};

use crate::{
    ProtocolError, SCHEMA_VERSION_V1, canonical,
    digest::Sha256Digest,
    ids::{ActorId, AuditEventId},
    signing::SignedContract,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActorKind {
    Customer,
    Provider,
    Worker,
    Operator,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditedEntityKind {
    Pipeline,
    Stage,
    Attempt,
    Lease,
    Artifact,
    Verification,
    Reservation,
    Settlement,
    Provider,
}

/// One append-only explanation of a state transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditEventV1 {
    pub schema_version: u16,
    pub event_id: AuditEventId,
    pub sequence: u64,
    pub occurred_at_unix_ms: u64,
    pub actor_kind: ActorKind,
    pub actor_id: ActorId,
    pub entity_kind: AuditedEntityKind,
    pub entity_id: String,
    pub previous_state: String,
    pub new_state: String,
    pub reason_code: String,
    pub command_digest: Sha256Digest,
    pub trace_id: String,
}

impl SignedContract for AuditEventV1 {
    const CONTRACT_TYPE: &'static str = "audit_event_v1";

    fn validate_signed_contract(&self, _now_unix_ms: u64) -> Result<(), ProtocolError> {
        self.validate()
    }
}

impl AuditEventV1 {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.schema_version != SCHEMA_VERSION_V1 {
            return Err(ProtocolError::UnsupportedSchemaVersion {
                actual: self.schema_version,
                expected: SCHEMA_VERSION_V1,
            });
        }
        if self.sequence == 0 || self.entity_id.is_empty() || self.entity_id.len() > 128 {
            return Err(ProtocolError::Invariant(
                "audit sequence and entity id are invalid".into(),
            ));
        }
        for (label, value) in [
            ("previous state", self.previous_state.as_str()),
            ("new state", self.new_state.as_str()),
            ("reason code", self.reason_code.as_str()),
            ("trace id", self.trace_id.as_str()),
        ] {
            if value.is_empty() || value.len() > 128 {
                return Err(ProtocolError::Invariant(format!(
                    "audit {label} must contain 1 to 128 bytes"
                )));
            }
        }
        Ok(())
    }

    pub fn canonical_digest(&self) -> Result<Sha256Digest, ProtocolError> {
        self.validate()?;
        canonical::digest(self)
    }
}
