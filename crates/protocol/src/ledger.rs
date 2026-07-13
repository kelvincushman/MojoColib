//! Fixed-integer test-credit contracts. No floating-point value crosses this boundary.

use core::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

use crate::{
    ProtocolError, SCHEMA_VERSION_V1,
    digest::Sha256Digest,
    ids::{
        ArtifactId, AttemptId, CustomerId, PipelineId, ProviderId, QuoteId, ReservationId,
        SettlementId, StageId,
    },
    signing::SignedContract,
};

/// A signed 64-bit count of valueless test-credit millionths.
///
/// The wire representation is a decimal string so JavaScript clients cannot silently round it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Microcredits(i64);

impl Microcredits {
    pub const ZERO: Self = Self(0);

    /// Creates an amount. Negative values are only valid for balancing ledger entries.
    #[must_use]
    pub const fn new(value: i64) -> Self {
        Self(value)
    }

    /// Creates an amount accepted by quotes, reservations and settlements.
    pub fn non_negative(value: i64) -> Result<Self, ProtocolError> {
        if value < 0 {
            return Err(ProtocolError::Invariant(
                "commercial amounts must not be negative".into(),
            ));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn value(self) -> i64 {
        self.0
    }

    pub fn checked_add(self, other: Self) -> Result<Self, ProtocolError> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or(ProtocolError::ArithmeticOverflow)
    }

    pub fn checked_sub(self, other: Self) -> Result<Self, ProtocolError> {
        self.0
            .checked_sub(other.0)
            .map(Self)
            .ok_or(ProtocolError::ArithmeticOverflow)
    }
}

impl fmt::Display for Microcredits {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for Microcredits {
    type Err = ProtocolError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let digits = value.strip_prefix('-').unwrap_or(value);
        let canonical = value == "0"
            || (!digits.is_empty()
                && !digits.starts_with('0')
                && digits.bytes().all(|byte| byte.is_ascii_digit()));
        if !canonical || value == "-0" || value.starts_with('+') {
            return Err(ProtocolError::Invariant(
                "microcredits must use canonical base-10 integer text".into(),
            ));
        }
        value
            .parse::<i64>()
            .map(Self)
            .map_err(|_| ProtocolError::Invariant("invalid microcredit integer".into()))
    }
}

impl Serialize for Microcredits {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for Microcredits {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

/// A provider's immutable offer to execute one stage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuoteV1 {
    pub schema_version: u16,
    pub quote_id: QuoteId,
    pub pipeline_id: PipelineId,
    pub stage_id: StageId,
    pub provider_id: ProviderId,
    pub execution: Microcredits,
    pub verification_reserve: Microcredits,
    pub policy_version: u32,
    pub expires_at_unix_ms: u64,
}

impl SignedContract for QuoteV1 {
    const CONTRACT_TYPE: &'static str = "quote_v1";

    fn validate_signed_contract(&self, now_unix_ms: u64) -> Result<(), ProtocolError> {
        self.validate(now_unix_ms)
    }
}

impl QuoteV1 {
    pub fn validate(&self, now_unix_ms: u64) -> Result<(), ProtocolError> {
        validate_v1(self.schema_version)?;
        if self.execution < Microcredits::ZERO || self.verification_reserve < Microcredits::ZERO {
            return Err(ProtocolError::Invariant(
                "quote amounts must be non-negative".into(),
            ));
        }
        if self.expires_at_unix_ms <= now_unix_ms {
            return Err(ProtocolError::Invariant("quote has expired".into()));
        }
        let _ = self.maximum_reservation()?;
        Ok(())
    }

    pub fn maximum_reservation(&self) -> Result<Microcredits, ProtocolError> {
        self.execution.checked_add(self.verification_reserve)
    }
}

/// Funds reserved before a stage can be scheduled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReservationV1 {
    pub schema_version: u16,
    pub reservation_id: ReservationId,
    pub customer_id: CustomerId,
    pub quote_id: QuoteId,
    pub pipeline_id: PipelineId,
    pub stage_id: StageId,
    pub amount: Microcredits,
}

/// The unique accounting commitment for one accepted stage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettlementV1 {
    pub schema_version: u16,
    pub settlement_id: SettlementId,
    pub reservation_id: ReservationId,
    pub quote_id: QuoteId,
    pub pipeline_id: PipelineId,
    pub stage_id: StageId,
    pub attempt_id: AttemptId,
    pub artifact_id: ArtifactId,
    pub artifact_manifest_digest: Sha256Digest,
    pub provider_id: ProviderId,
    pub verification_decision_digest: Sha256Digest,
    pub provider_credit: Microcredits,
    pub operator_fee: Microcredits,
}

pub type StageSettlementV1 = SettlementV1;

impl SettlementV1 {
    pub fn total_debit(&self) -> Result<Microcredits, ProtocolError> {
        if self.provider_credit < Microcredits::ZERO || self.operator_fee < Microcredits::ZERO {
            return Err(ProtocolError::Invariant(
                "settlement amounts must be non-negative".into(),
            ));
        }
        self.provider_credit.checked_add(self.operator_fee)
    }
}

fn validate_v1(actual: u16) -> Result<(), ProtocolError> {
    if actual == SCHEMA_VERSION_V1 {
        Ok(())
    } else {
        Err(ProtocolError::UnsupportedSchemaVersion {
            actual,
            expected: SCHEMA_VERSION_V1,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Microcredits;

    #[test]
    fn monetary_wire_format_is_lossless_at_i64_boundary() {
        let amount = Microcredits::new(i64::MAX);
        let json = serde_json::to_string(&amount).unwrap();
        assert_eq!(json, format!("\"{}\"", i64::MAX));
        assert_eq!(serde_json::from_str::<Microcredits>(&json).unwrap(), amount);
    }

    #[test]
    fn checked_arithmetic_rejects_overflow() {
        assert!(
            Microcredits::new(i64::MAX)
                .checked_add(Microcredits::new(1))
                .is_err()
        );
    }

    #[test]
    fn rejects_non_canonical_monetary_text() {
        for invalid in ["+1", "01", "-0", "1.0", "1e3", " 1"] {
            assert!(invalid.parse::<Microcredits>().is_err(), "{invalid}");
        }
    }
}
