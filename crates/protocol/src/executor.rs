//! Runtime-neutral messages between a worker supervisor and a constrained executor.

use serde::{Deserialize, Serialize};

use crate::{
    ProtocolError, SCHEMA_VERSION_V1,
    digest::Sha256Digest,
    ids::{ArtifactId, AttemptId, PipelineId, StageId},
    lease::{FencingTokenV1, StageLeaseV1},
    pipeline::ResourceRequirementsV1,
    signing::SignedContract,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkPolicy {
    DenyAll,
    TemplateAllowlist,
}

/// Complete immutable input to an executor process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutorRequestV1 {
    pub schema_version: u16,
    pub lease: StageLeaseV1,
    pub parameter_digest: Sha256Digest,
    pub environment_digest: Sha256Digest,
    pub resources: ResourceRequirementsV1,
    pub network_policy: NetworkPolicy,
    pub resume_checkpoint_manifest: Option<Sha256Digest>,
}

impl SignedContract for ExecutorRequestV1 {
    const CONTRACT_TYPE: &'static str = "executor_request_v1";

    fn validate_signed_contract(&self, _now_unix_ms: u64) -> Result<(), ProtocolError> {
        self.validate()
    }
}

impl ExecutorRequestV1 {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.schema_version != SCHEMA_VERSION_V1 {
            return Err(ProtocolError::UnsupportedSchemaVersion {
                actual: self.schema_version,
                expected: SCHEMA_VERSION_V1,
            });
        }
        self.lease.validate()?;
        self.resources.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ExecutorEventV1 {
    Started {
        unix_ms: u64,
    },
    Progress {
        unix_ms: u64,
        optimizer_step: u64,
        metrics_digest: Sha256Digest,
    },
    CheckpointPromoted {
        unix_ms: u64,
        artifact_id: ArtifactId,
        manifest_digest: Sha256Digest,
    },
    DrainAcknowledged {
        unix_ms: u64,
    },
}

pub type ProgressEventV1 = ExecutorEventV1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionStatus {
    Succeeded,
    RetryableFailure,
    PermanentFailure,
    Cancelled,
}

/// Terminal executor report. Artifact acceptance remains a coordinator decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutorCompletionV1 {
    pub schema_version: u16,
    pub pipeline_id: PipelineId,
    pub stage_id: StageId,
    pub attempt_id: AttemptId,
    pub fencing_token: FencingTokenV1,
    pub status: CompletionStatus,
    pub output_manifest_digests: Vec<Sha256Digest>,
    pub final_metrics_digest: Option<Sha256Digest>,
    pub reason_code: String,
}

pub type CompletionSummaryV1 = ExecutorCompletionV1;

impl ExecutorCompletionV1 {
    pub fn validate_against(
        &self,
        lease: &StageLeaseV1,
        observed_at_unix_ms: u64,
    ) -> Result<(), ProtocolError> {
        if self.schema_version != SCHEMA_VERSION_V1 {
            return Err(ProtocolError::UnsupportedSchemaVersion {
                actual: self.schema_version,
                expected: SCHEMA_VERSION_V1,
            });
        }
        if self.pipeline_id != lease.pipeline_id
            || self.stage_id != lease.stage_id
            || self.attempt_id != lease.attempt_id
        {
            return Err(ProtocolError::Invariant(
                "completion identity does not match lease".into(),
            ));
        }
        lease.accepts_fencing_token(self.fencing_token, observed_at_unix_ms)?;
        if self.reason_code.is_empty() || self.reason_code.len() > 128 {
            return Err(ProtocolError::Invariant(
                "completion reason code must contain 1 to 128 bytes".into(),
            ));
        }
        if self.status == CompletionStatus::Succeeded && self.output_manifest_digests.is_empty() {
            return Err(ProtocolError::Invariant(
                "successful completion must declare an output manifest".into(),
            ));
        }
        Ok(())
    }
}
