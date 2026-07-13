//! Versioned verification requirements and decisions.

use serde::{Deserialize, Serialize};

use crate::{
    ProtocolError, SCHEMA_VERSION_V1,
    artifact::ArtifactManifestV1,
    digest::Sha256Digest,
    ids::{ArtifactId, AttemptId, ProviderId, StageId},
};

/// Verification work requested for a stage output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationPolicyV1 {
    pub schema_version: u16,
    pub policy_digest: Sha256Digest,
    pub require_checkpoint_load: bool,
    pub require_independent_provider: bool,
    pub hidden_canary: bool,
    pub duplicate_execution_basis_points: u16,
}

impl VerificationPolicyV1 {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.schema_version != SCHEMA_VERSION_V1 {
            return Err(ProtocolError::UnsupportedSchemaVersion {
                actual: self.schema_version,
                expected: SCHEMA_VERSION_V1,
            });
        }
        if self.duplicate_execution_basis_points > 10_000 {
            return Err(ProtocolError::Invariant(
                "duplicate execution rate exceeds 10000 basis points".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationOutcome {
    Accepted,
    Rejected,
    Inconclusive,
}

/// Immutable result of applying one verification policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationDecisionV1 {
    pub schema_version: u16,
    pub stage_id: StageId,
    pub attempt_id: AttemptId,
    pub artifact_id: ArtifactId,
    pub policy_digest: Sha256Digest,
    pub verifier_provider_id: ProviderId,
    pub producer_provider_id: ProviderId,
    pub outcome: VerificationOutcome,
    pub reason_codes: Vec<String>,
    pub evidence_manifest_digest: Sha256Digest,
}

impl VerificationDecisionV1 {
    fn validate_basic(&self) -> Result<(), ProtocolError> {
        if self.schema_version != SCHEMA_VERSION_V1 {
            return Err(ProtocolError::UnsupportedSchemaVersion {
                actual: self.schema_version,
                expected: SCHEMA_VERSION_V1,
            });
        }
        if self.reason_codes.is_empty() {
            return Err(ProtocolError::Invariant(
                "verification must preserve at least one reason code".into(),
            ));
        }
        Ok(())
    }

    /// Validates the decision against the signed policy and produced artifact.
    pub fn validate_against(
        &self,
        policy: &VerificationPolicyV1,
        artifact: &ArtifactManifestV1,
    ) -> Result<(), ProtocolError> {
        self.validate_basic()?;
        policy.validate()?;
        artifact.validate()?;
        if self.policy_digest != policy.policy_digest
            || self.artifact_id != artifact.artifact_id
            || self.producer_provider_id != artifact.lineage.provider_id
            || self.stage_id != artifact.lineage.stage_id
            || self.attempt_id != artifact.lineage.attempt_id
        {
            return Err(ProtocolError::Invariant(
                "verification decision does not match its policy or artifact".into(),
            ));
        }
        if policy.require_independent_provider
            && self.verifier_provider_id == self.producer_provider_id
        {
            return Err(ProtocolError::Invariant(
                "independent verification used the producing provider".into(),
            ));
        }
        Ok(())
    }
}
