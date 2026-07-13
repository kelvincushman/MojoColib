//! Operator-approved executor template manifests.

use serde::{Deserialize, Serialize};

use crate::{
    ProtocolError, SCHEMA_VERSION_V1,
    digest::Sha256Digest,
    executor::NetworkPolicy,
    ids::TemplateId,
    pipeline::{ArtifactType, ResourceRequirementsV1, RuntimeKind},
    signing::SignedContract,
    verification::VerificationPolicyV1,
};

/// Immutable execution and policy contract for an operator-approved template bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateManifestV1 {
    pub schema_version: u16,
    pub template_id: TemplateId,
    pub bundle_digest: Sha256Digest,
    pub runtime: RuntimeKind,
    pub supported_operating_systems: Vec<String>,
    pub supported_architectures: Vec<String>,
    pub supported_backends: Vec<String>,
    pub parameter_schema_digest: Sha256Digest,
    pub input_types: Vec<ArtifactType>,
    pub output_types: Vec<ArtifactType>,
    pub maximum_resources: ResourceRequirementsV1,
    pub network_policy: NetworkPolicy,
    pub network_allowlist_digest: Option<Sha256Digest>,
    pub verification_policy: VerificationPolicyV1,
    pub graceful_drain_seconds: u16,
}

impl TemplateManifestV1 {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.schema_version != SCHEMA_VERSION_V1 {
            return Err(ProtocolError::UnsupportedSchemaVersion {
                actual: self.schema_version,
                expected: SCHEMA_VERSION_V1,
            });
        }
        self.maximum_resources.validate()?;
        self.verification_policy.validate()?;
        if self.supported_operating_systems.is_empty()
            || self.supported_architectures.is_empty()
            || self.supported_backends.is_empty()
            || self.output_types.is_empty()
        {
            return Err(ProtocolError::Invariant(
                "template must declare platform, backend and output support".into(),
            ));
        }
        if self.graceful_drain_seconds == 0 || self.graceful_drain_seconds > 30 {
            return Err(ProtocolError::Invariant(
                "template drain deadline must be between 1 and 30 seconds".into(),
            ));
        }
        match (self.network_policy, self.network_allowlist_digest) {
            (NetworkPolicy::DenyAll, None) | (NetworkPolicy::TemplateAllowlist, Some(_)) => Ok(()),
            _ => Err(ProtocolError::Invariant(
                "template network policy and allowlist commitment disagree".into(),
            )),
        }
    }
}

impl SignedContract for TemplateManifestV1 {
    const CONTRACT_TYPE: &'static str = "template_manifest_v1";

    fn validate_signed_contract(&self, _now_unix_ms: u64) -> Result<(), ProtocolError> {
        self.validate()
    }
}
