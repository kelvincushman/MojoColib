//! Signed provider capability and benchmark claims.

use serde::{Deserialize, Serialize};

use crate::{
    ProtocolError, SCHEMA_VERSION_V1,
    digest::Sha256Digest,
    ids::{ProviderId, WorkerId},
    signing::SignedContract,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcceleratorVendor {
    None,
    Nvidia,
    Amd,
    Apple,
    Intel,
    Google,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceleratorCapabilityV1 {
    pub vendor: AcceleratorVendor,
    pub model: String,
    pub backend: String,
    pub driver_version: String,
    pub memory_bytes: u64,
    pub supported_precisions: Vec<String>,
}

/// Expiring machine capabilities backed by a versioned benchmark manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerCapabilityV1 {
    pub schema_version: u16,
    pub provider_id: ProviderId,
    pub worker_id: WorkerId,
    pub machine_fingerprint_digest: Sha256Digest,
    pub operating_system: String,
    pub architecture: String,
    pub cpu_cores: u16,
    pub ram_bytes: u64,
    pub available_storage_bytes: u64,
    pub accelerators: Vec<AcceleratorCapabilityV1>,
    pub benchmark_manifest_digest: Sha256Digest,
    pub available: bool,
    pub maximum_temperature_celsius: Option<u16>,
    pub power_limit_milliwatts: Option<u64>,
    pub expires_at_unix_ms: u64,
}

/// Required-contract name retained while capabilities remain worker-specific in V1.
pub type ProviderCapabilitiesV1 = WorkerCapabilityV1;

impl SignedContract for WorkerCapabilityV1 {
    const CONTRACT_TYPE: &'static str = "worker_capability_v1";

    fn validate_signed_contract(&self, now_unix_ms: u64) -> Result<(), ProtocolError> {
        self.validate(now_unix_ms)
    }
}

impl WorkerCapabilityV1 {
    pub fn validate(&self, now_unix_ms: u64) -> Result<(), ProtocolError> {
        if self.schema_version != SCHEMA_VERSION_V1 {
            return Err(ProtocolError::UnsupportedSchemaVersion {
                actual: self.schema_version,
                expected: SCHEMA_VERSION_V1,
            });
        }
        if self.cpu_cores == 0 || self.ram_bytes == 0 || self.available_storage_bytes == 0 {
            return Err(ProtocolError::Invariant(
                "worker must report non-zero CPU, RAM and storage".into(),
            ));
        }
        if self.expires_at_unix_ms <= now_unix_ms {
            return Err(ProtocolError::Invariant(
                "worker capability record has expired".into(),
            ));
        }
        if self.accelerators.len() > 16 {
            return Err(ProtocolError::Invariant(
                "worker reports more than 16 accelerators".into(),
            ));
        }
        Ok(())
    }
}
