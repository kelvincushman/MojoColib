//! Versioned capability benchmark evidence used for worker admission.

use serde::{Deserialize, Serialize};

use crate::{
    ProtocolError, SCHEMA_VERSION_V1,
    digest::Sha256Digest,
    ids::{BenchmarkId, ProviderId, WorkerId},
    signing::SignedContract,
};

/// Fixed-integer benchmark evidence for one exact hardware/runtime configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkResultV1 {
    pub schema_version: u16,
    pub benchmark_id: BenchmarkId,
    pub benchmark_suite_digest: Sha256Digest,
    pub provider_id: ProviderId,
    pub worker_id: WorkerId,
    pub machine_fingerprint_digest: Sha256Digest,
    pub environment_digest: Sha256Digest,
    pub backend: String,
    pub driver_version: String,
    pub precision: String,
    pub tokens_per_second_milli: u64,
    pub peak_memory_bytes: u64,
    pub average_power_milliwatts: Option<u64>,
    pub fixture_passed: bool,
    pub recorded_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}

impl BenchmarkResultV1 {
    pub fn validate(&self, now_unix_ms: u64) -> Result<(), ProtocolError> {
        if self.schema_version != SCHEMA_VERSION_V1 {
            return Err(ProtocolError::UnsupportedSchemaVersion {
                actual: self.schema_version,
                expected: SCHEMA_VERSION_V1,
            });
        }
        if !self.fixture_passed
            || self.tokens_per_second_milli == 0
            || self.peak_memory_bytes == 0
            || self.recorded_at_unix_ms >= self.expires_at_unix_ms
            || now_unix_ms >= self.expires_at_unix_ms
        {
            return Err(ProtocolError::Invariant(
                "benchmark is failed, empty or expired".into(),
            ));
        }
        Ok(())
    }
}

impl SignedContract for BenchmarkResultV1 {
    const CONTRACT_TYPE: &'static str = "benchmark_result_v1";

    fn validate_signed_contract(&self, now_unix_ms: u64) -> Result<(), ProtocolError> {
        self.validate(now_unix_ms)
    }
}
