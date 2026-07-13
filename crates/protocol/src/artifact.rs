//! Content-addressed artifacts, lineage and checkpoint compatibility contracts.

use serde::{Deserialize, Serialize};

use crate::{
    ProtocolError, SCHEMA_VERSION_V1, canonical,
    digest::Sha256Digest,
    ids::{ArtifactId, AttemptId, PipelineId, ProviderId, StageId, TemplateId, WorkerId},
    lease::{FencingTokenV1, StageLeaseV1},
    pipeline::{ArtifactType, DataClass},
    signing::{SignedContract, SignerKind, TrustedEd25519KeyV1},
};

pub const MAX_ARTIFACT_BYTES: u64 = 20 * 1024 * 1024 * 1024;
pub const MAX_CHECKPOINT_BYTES: u64 = 5 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetentionClass {
    Temporary,
    PilotAccepted,
    CustomerManaged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactLineageV1 {
    pub pipeline_id: PipelineId,
    pub stage_id: StageId,
    pub attempt_id: AttemptId,
    pub provider_id: ProviderId,
    pub worker_id: WorkerId,
    pub fencing_token: FencingTokenV1,
    pub input_manifest_digests: Vec<Sha256Digest>,
}

/// Unsigned artifact metadata. The complete value is carried in a `SignedEnvelopeV1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactManifestV1 {
    pub schema_version: u16,
    pub artifact_id: ArtifactId,
    pub content_digest: Sha256Digest,
    pub byte_length: u64,
    pub logical_type: ArtifactType,
    pub lineage: ArtifactLineageV1,
    pub environment_digest: Sha256Digest,
    pub data_class: DataClass,
    pub retention: RetentionClass,
}

impl SignedContract for ArtifactManifestV1 {
    const CONTRACT_TYPE: &'static str = "artifact_manifest_v1";

    fn validate_signed_contract(&self, _now_unix_ms: u64) -> Result<(), ProtocolError> {
        self.validate()
    }
}

impl ArtifactManifestV1 {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.schema_version != SCHEMA_VERSION_V1 {
            return Err(ProtocolError::UnsupportedSchemaVersion {
                actual: self.schema_version,
                expected: SCHEMA_VERSION_V1,
            });
        }
        if self.byte_length == 0 || self.byte_length > MAX_ARTIFACT_BYTES {
            return Err(ProtocolError::Invariant(format!(
                "artifact byte length must be between 1 and {MAX_ARTIFACT_BYTES}"
            )));
        }
        if self.lineage.input_manifest_digests.len() > 100 {
            return Err(ProtocolError::Invariant(
                "artifact has too many input lineage entries".into(),
            ));
        }
        Ok(())
    }

    pub fn canonical_digest(&self) -> Result<Sha256Digest, ProtocolError> {
        self.validate()?;
        canonical::digest(self)
    }

    /// Proves that the manifest was produced by the exact active assignment.
    pub fn validate_against_lease(&self, lease: &StageLeaseV1) -> Result<(), ProtocolError> {
        self.validate()?;
        lease.validate()?;
        if self.lineage.pipeline_id != lease.pipeline_id
            || self.lineage.stage_id != lease.stage_id
            || self.lineage.attempt_id != lease.attempt_id
            || self.lineage.worker_id != lease.worker_id
            || self.lineage.fencing_token != lease.fencing_token
            || self.lineage.input_manifest_digests != lease.input_manifest_digests
        {
            return Err(ProtocolError::Invariant(
                "artifact lineage does not match its stage lease".into(),
            ));
        }
        Ok(())
    }

    /// Binds the registry-authenticated signer to the worker named in lineage.
    pub fn validate_registry_signer(
        &self,
        signer: &TrustedEd25519KeyV1,
    ) -> Result<(), ProtocolError> {
        if signer.signer_kind != SignerKind::Worker
            || signer.signer_id.as_str() != self.lineage.worker_id.as_str()
        {
            return Err(ProtocolError::InvalidSignature {
                field: "artifact worker identity",
            });
        }
        Ok(())
    }
}

/// Immutable inputs whose equality determines whether a checkpoint can be resumed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointCompatibilityV1 {
    pub template_id: TemplateId,
    pub template_digest: Sha256Digest,
    pub model_revision_digest: Sha256Digest,
    pub tokenizer_revision_digest: Sha256Digest,
    pub data_manifest_digest: Sha256Digest,
    pub parameter_schema_digest: Sha256Digest,
    pub checkpoint_format_version: u32,
}

impl CheckpointCompatibilityV1 {
    pub fn key(&self) -> Result<Sha256Digest, ProtocolError> {
        if self.checkpoint_format_version == 0 {
            return Err(ProtocolError::Invariant(
                "checkpoint format version must be non-zero".into(),
            ));
        }
        canonical::digest(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointManifestV1 {
    pub schema_version: u16,
    pub artifact: ArtifactManifestV1,
    pub compatibility: CheckpointCompatibilityV1,
    pub compatibility_key: Sha256Digest,
    pub optimizer_step: u64,
    pub epoch_micros: u64,
    pub includes_optimizer_state: bool,
    pub includes_scheduler_state: bool,
    pub includes_rng_state: bool,
}

impl SignedContract for CheckpointManifestV1 {
    const CONTRACT_TYPE: &'static str = "checkpoint_manifest_v1";

    fn validate_signed_contract(&self, _now_unix_ms: u64) -> Result<(), ProtocolError> {
        self.validate()
    }
}

impl CheckpointManifestV1 {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.schema_version != SCHEMA_VERSION_V1 {
            return Err(ProtocolError::UnsupportedSchemaVersion {
                actual: self.schema_version,
                expected: SCHEMA_VERSION_V1,
            });
        }
        self.artifact.validate()?;
        if self.artifact.logical_type != ArtifactType::Checkpoint {
            return Err(ProtocolError::Invariant(
                "checkpoint manifest must reference a checkpoint artifact".into(),
            ));
        }
        if self.artifact.byte_length > MAX_CHECKPOINT_BYTES {
            return Err(ProtocolError::Invariant(format!(
                "checkpoint exceeds {MAX_CHECKPOINT_BYTES} bytes"
            )));
        }
        if self.compatibility.key()? != self.compatibility_key {
            return Err(ProtocolError::Invariant(
                "checkpoint compatibility key does not match its inputs".into(),
            ));
        }
        if !self.includes_optimizer_state
            || !self.includes_scheduler_state
            || !self.includes_rng_state
        {
            return Err(ProtocolError::Invariant(
                "training checkpoint is missing required resume state".into(),
            ));
        }
        Ok(())
    }

    pub fn ensure_compatible(
        &self,
        expected: &CheckpointCompatibilityV1,
    ) -> Result<(), ProtocolError> {
        self.validate()?;
        if self.compatibility_key != expected.key()? {
            return Err(ProtocolError::Invariant(
                "checkpoint is incompatible with this execution request".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::CheckpointCompatibilityV1;
    use crate::{digest::Sha256Digest, ids::TemplateId};

    fn compatibility() -> CheckpointCompatibilityV1 {
        CheckpointCompatibilityV1 {
            template_id: TemplateId::new("hf-peft-sft-v1").unwrap(),
            template_digest: Sha256Digest::of_bytes(b"template"),
            model_revision_digest: Sha256Digest::of_bytes(b"model"),
            tokenizer_revision_digest: Sha256Digest::of_bytes(b"tokenizer"),
            data_manifest_digest: Sha256Digest::of_bytes(b"data"),
            parameter_schema_digest: Sha256Digest::of_bytes(b"parameters"),
            checkpoint_format_version: 1,
        }
    }

    #[test]
    fn compatibility_key_changes_with_any_committed_input() {
        let first = compatibility();
        let mut second = first.clone();
        second.data_manifest_digest = Sha256Digest::of_bytes(b"different-data");
        assert_ne!(first.key().unwrap(), second.key().unwrap());
    }
}
