//! Renewable stage leases with explicit fencing commitments.

use core::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

use crate::{
    ProtocolError, SCHEMA_VERSION_V1,
    digest::Sha256Digest,
    ids::{AttemptId, PipelineId, StageId, WorkerId},
    signing::SignedContract,
};

pub const MAX_LEASE_DURATION_MS: u64 = 60_000;

/// A non-zero, monotonically increasing stage-assignment fence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FencingTokenV1(u64);

impl FencingTokenV1 {
    pub fn new(value: u64) -> Result<Self, ProtocolError> {
        if value == 0 {
            Err(ProtocolError::Invariant(
                "fencing token must be non-zero".into(),
            ))
        } else {
            Ok(Self(value))
        }
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Display for FencingTokenV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for FencingTokenV1 {
    type Err = ProtocolError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.is_empty()
            || value.starts_with(['+', '0'])
            || !value.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(ProtocolError::Invariant(
                "fencing token must be canonical non-zero decimal text".into(),
            ));
        }
        Self::new(
            value
                .parse()
                .map_err(|_| ProtocolError::Invariant("invalid fencing token".into()))?,
        )
    }
}

impl Serialize for FencingTokenV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for FencingTokenV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

/// An assignment that authorises exactly one worker attempt until expiry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StageLeaseV1 {
    pub schema_version: u16,
    pub pipeline_id: PipelineId,
    pub pipeline_spec_digest: Sha256Digest,
    pub stage_id: StageId,
    pub attempt_id: AttemptId,
    pub worker_id: WorkerId,
    pub template_digest: Sha256Digest,
    pub input_manifest_digests: Vec<Sha256Digest>,
    pub issued_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
    pub fencing_token: FencingTokenV1,
}

impl SignedContract for StageLeaseV1 {
    const CONTRACT_TYPE: &'static str = "stage_lease_v1";

    fn validate_signed_contract(&self, now_unix_ms: u64) -> Result<(), ProtocolError> {
        self.validate()?;
        if !self.is_active_at(now_unix_ms) {
            return Err(ProtocolError::Invariant(
                "signed lease is not active at verification time".into(),
            ));
        }
        Ok(())
    }
}

impl StageLeaseV1 {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.schema_version != SCHEMA_VERSION_V1 {
            return Err(ProtocolError::UnsupportedSchemaVersion {
                actual: self.schema_version,
                expected: SCHEMA_VERSION_V1,
            });
        }
        let duration = self
            .expires_at_unix_ms
            .checked_sub(self.issued_at_unix_ms)
            .ok_or_else(|| ProtocolError::Invariant("lease expires before issue time".into()))?;
        if duration == 0 || duration > MAX_LEASE_DURATION_MS {
            return Err(ProtocolError::Invariant(format!(
                "lease duration must be between 1 and {MAX_LEASE_DURATION_MS} milliseconds"
            )));
        }
        Ok(())
    }

    #[must_use]
    pub const fn is_active_at(&self, unix_ms: u64) -> bool {
        unix_ms >= self.issued_at_unix_ms && unix_ms < self.expires_at_unix_ms
    }

    pub fn accepts_fencing_token(
        &self,
        fencing_token: FencingTokenV1,
        observed_at_unix_ms: u64,
    ) -> Result<(), ProtocolError> {
        self.validate()?;
        if fencing_token != self.fencing_token {
            return Err(ProtocolError::Invariant("stale fencing token".into()));
        }
        if !self.is_active_at(observed_at_unix_ms) {
            return Err(ProtocolError::Invariant("lease is not active".into()));
        }
        Ok(())
    }
}

/// A same-attempt extension that preserves every lease commitment and fencing token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LeaseRenewalV1 {
    pub schema_version: u16,
    pub pipeline_id: PipelineId,
    pub stage_id: StageId,
    pub attempt_id: AttemptId,
    pub worker_id: WorkerId,
    pub fencing_token: FencingTokenV1,
    pub previous_expires_at_unix_ms: u64,
    pub renewed_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}

impl LeaseRenewalV1 {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.schema_version != SCHEMA_VERSION_V1 {
            return Err(ProtocolError::UnsupportedSchemaVersion {
                actual: self.schema_version,
                expected: SCHEMA_VERSION_V1,
            });
        }
        if self.renewed_at_unix_ms >= self.previous_expires_at_unix_ms {
            return Err(ProtocolError::Invariant(
                "lease renewal arrived after expiry".into(),
            ));
        }
        let duration = self
            .expires_at_unix_ms
            .checked_sub(self.renewed_at_unix_ms)
            .ok_or_else(|| ProtocolError::Invariant("renewal expiry precedes issue".into()))?;
        if self.expires_at_unix_ms <= self.previous_expires_at_unix_ms
            || duration == 0
            || duration > MAX_LEASE_DURATION_MS
        {
            return Err(ProtocolError::Invariant(
                "renewal must extend the lease by a bounded duration".into(),
            ));
        }
        Ok(())
    }

    pub fn apply_to(&self, lease: &StageLeaseV1) -> Result<StageLeaseV1, ProtocolError> {
        self.validate()?;
        lease.validate()?;
        if self.pipeline_id != lease.pipeline_id
            || self.stage_id != lease.stage_id
            || self.attempt_id != lease.attempt_id
            || self.worker_id != lease.worker_id
            || self.fencing_token != lease.fencing_token
            || self.previous_expires_at_unix_ms != lease.expires_at_unix_ms
        {
            return Err(ProtocolError::Invariant(
                "renewal does not match the active lease".into(),
            ));
        }
        let mut renewed = lease.clone();
        renewed.issued_at_unix_ms = self.renewed_at_unix_ms;
        renewed.expires_at_unix_ms = self.expires_at_unix_ms;
        renewed.validate()?;
        Ok(renewed)
    }
}

impl SignedContract for LeaseRenewalV1 {
    const CONTRACT_TYPE: &'static str = "lease_renewal_v1";

    fn validate_signed_contract(&self, now_unix_ms: u64) -> Result<(), ProtocolError> {
        self.validate()?;
        if now_unix_ms < self.renewed_at_unix_ms || now_unix_ms >= self.expires_at_unix_ms {
            return Err(ProtocolError::Invariant(
                "signed renewal is not active at verification time".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{FencingTokenV1, StageLeaseV1};
    use crate::{
        SCHEMA_VERSION_V1,
        digest::Sha256Digest,
        ids::{AttemptId, PipelineId, StageId, WorkerId},
    };

    fn lease() -> StageLeaseV1 {
        StageLeaseV1 {
            schema_version: SCHEMA_VERSION_V1,
            pipeline_id: PipelineId::new("pipeline-1").unwrap(),
            pipeline_spec_digest: Sha256Digest::of_bytes(b"pipeline"),
            stage_id: StageId::new("stage-1").unwrap(),
            attempt_id: AttemptId::new("attempt-1").unwrap(),
            worker_id: WorkerId::new("worker-1").unwrap(),
            template_digest: Sha256Digest::of_bytes(b"template"),
            input_manifest_digests: Vec::new(),
            issued_at_unix_ms: 1_000,
            expires_at_unix_ms: 61_000,
            fencing_token: FencingTokenV1::new(7).unwrap(),
        }
    }

    #[test]
    fn stale_or_expired_completion_is_rejected() {
        let lease = lease();
        assert!(
            lease
                .accepts_fencing_token(FencingTokenV1::new(6).unwrap(), 2_000)
                .is_err()
        );
        assert!(
            lease
                .accepts_fencing_token(FencingTokenV1::new(7).unwrap(), 61_000)
                .is_err()
        );
        assert_eq!(
            lease.accepts_fencing_token(FencingTokenV1::new(7).unwrap(), 60_999),
            Ok(())
        );
    }
}
