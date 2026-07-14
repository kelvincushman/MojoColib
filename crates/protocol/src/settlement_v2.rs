//! Versioned settlement terms that bind customer funding before a worker is scheduled.
//!
//! V2 deliberately does not create a chain transaction. It makes the economic
//! terms, funding authorization and accepted-work evidence reproducible so a
//! coordinator can later mirror a final settlement to an escrow program.

use core::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

use crate::{
    ProtocolError, SCHEMA_VERSION_V2, canonical,
    digest::Sha256Digest,
    ids::{
        ActorId, ArtifactId, AttemptId, ChainReceiptId, CustomerId, FundingAuthorizationId,
        PipelineId, ProviderId, QuoteId, ReservationId, SettlementId, SettlementNonceId, StageId,
    },
    signing::{SignedContract, SignedEnvelopeV1, SignerKind, TrustedEd25519KeyV1},
};

const BASIS_POINTS_DENOMINATOR: i128 = 10_000;

/// A signed 64-bit amount expressed in the atomic units of a declared asset.
///
/// The JSON representation is canonical base-10 text so JavaScript clients
/// cannot round a funding or payout amount.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AtomicAmount(i64);

impl AtomicAmount {
    pub const ZERO: Self = Self(0);

    /// Creates an amount. Contract validators reject negative amounts.
    #[must_use]
    pub const fn new(value: i64) -> Self {
        Self(value)
    }

    /// Creates a non-negative amount suitable for settlement terms.
    pub fn non_negative(value: i64) -> Result<Self, ProtocolError> {
        if value < 0 {
            return Err(ProtocolError::Invariant(
                "settlement amounts must not be negative".into(),
            ));
        }
        Ok(Self(value))
    }

    /// Returns the integer representation.
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

impl fmt::Display for AtomicAmount {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for AtomicAmount {
    type Err = ProtocolError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let digits = value.strip_prefix('-').unwrap_or(value);
        let canonical = value == "0"
            || (!digits.is_empty()
                && !digits.starts_with('0')
                && digits.bytes().all(|byte| byte.is_ascii_digit()));
        if !canonical || value == "-0" || value.starts_with('+') {
            return Err(ProtocolError::Invariant(
                "atomic amounts must use canonical base-10 integer text".into(),
            ));
        }
        value
            .parse::<i64>()
            .map(Self)
            .map_err(|_| ProtocolError::Invariant("invalid atomic amount".into()))
    }
}

impl Serialize for AtomicAmount {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for AtomicAmount {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

/// The asset and pricing evidence committed by a V2 quote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettlementAssetV2 {
    /// Stable identifier for the settlement asset or test-credit rail.
    pub asset_id: String,
    /// Decimal precision used by the asset's smallest unit.
    pub decimals: u8,
    /// Stable accounting unit used to produce the quote.
    pub accounting_unit: String,
    /// Versioned, auditable conversion source or fixed-price policy reference.
    pub conversion_source: String,
    /// Time at which the conversion source was observed or fixed.
    pub conversion_observed_at_unix_ms: u64,
}

impl SettlementAssetV2 {
    /// Validates machine-readable asset and conversion references.
    pub fn validate(&self) -> Result<(), ProtocolError> {
        validate_reference("asset id", &self.asset_id)?;
        validate_reference("accounting unit", &self.accounting_unit)?;
        validate_reference("conversion source", &self.conversion_source)?;
        Ok(())
    }
}

/// A recipient identity and an opaque settlement-rail destination reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PayoutRecipientV2 {
    pub actor_id: ActorId,
    pub destination_ref: String,
}

impl PayoutRecipientV2 {
    /// Validates the destination reference without assuming a specific chain.
    pub fn validate(&self) -> Result<(), ProtocolError> {
        validate_reference("payout destination", &self.destination_ref)
    }
}

/// Exact terms offered for one stage before customer funding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuoteV2 {
    pub schema_version: u16,
    pub quote_id: QuoteId,
    pub pipeline_id: PipelineId,
    pub stage_id: StageId,
    pub provider_id: ProviderId,
    pub asset: SettlementAssetV2,
    pub accepted_execution_cap_atomic: AtomicAmount,
    pub verification_reserve_cap_atomic: AtomicAmount,
    pub chain_fee_reserve_cap_atomic: AtomicAmount,
    pub provider_payout_cap_atomic: AtomicAmount,
    pub verifier_payout_cap_atomic: AtomicAmount,
    pub operator_fee_bps: u16,
    pub operator_fee_cap_atomic: AtomicAmount,
    pub provider_recipient: PayoutRecipientV2,
    pub verifier_recipient: PayoutRecipientV2,
    pub operator_recipient: PayoutRecipientV2,
    pub chain_fee_payer: PayoutRecipientV2,
    pub pricing_policy_version: u32,
    pub expires_at_unix_ms: u64,
}

impl SignedContract for QuoteV2 {
    const CONTRACT_TYPE: &'static str = "quote_v2";

    fn validate_signed_contract(&self, now_unix_ms: u64) -> Result<(), ProtocolError> {
        self.validate(now_unix_ms)
    }
}

impl QuoteV2 {
    /// Validates terms that remain meaningful after the quote's expiry.
    pub fn validate_terms(&self) -> Result<(), ProtocolError> {
        validate_v2(self.schema_version)?;
        self.asset.validate()?;
        for (label, amount) in [
            ("accepted execution cap", self.accepted_execution_cap_atomic),
            (
                "verification reserve cap",
                self.verification_reserve_cap_atomic,
            ),
            ("chain fee reserve cap", self.chain_fee_reserve_cap_atomic),
            ("provider payout cap", self.provider_payout_cap_atomic),
            ("verifier payout cap", self.verifier_payout_cap_atomic),
            ("operator fee cap", self.operator_fee_cap_atomic),
        ] {
            require_non_negative(amount, label)?;
        }
        if self.operator_fee_bps > 10_000 {
            return Err(ProtocolError::Invariant(
                "operator fee basis points must not exceed 10000".into(),
            ));
        }
        if self.provider_payout_cap_atomic > self.accepted_execution_cap_atomic
            || self.operator_fee_cap_atomic > self.accepted_execution_cap_atomic
            || self.verifier_payout_cap_atomic > self.verification_reserve_cap_atomic
        {
            return Err(ProtocolError::Invariant(
                "recipient payout caps exceed their reserved component".into(),
            ));
        }
        self.provider_recipient.validate()?;
        self.verifier_recipient.validate()?;
        self.operator_recipient.validate()?;
        self.chain_fee_payer.validate()?;
        let _ = self.funded_reservation_atomic()?;
        Ok(())
    }

    /// Validates a quote at the authoritative time it is accepted.
    pub fn validate(&self, now_unix_ms: u64) -> Result<(), ProtocolError> {
        self.validate_terms()?;
        if self.asset.conversion_observed_at_unix_ms > now_unix_ms {
            return Err(ProtocolError::Invariant(
                "conversion observation cannot be in the future".into(),
            ));
        }
        if self.expires_at_unix_ms <= now_unix_ms {
            return Err(ProtocolError::Invariant("quote has expired".into()));
        }
        Ok(())
    }

    /// Returns the exact maximum amount the customer must fund.
    pub fn funded_reservation_atomic(&self) -> Result<AtomicAmount, ProtocolError> {
        self.accepted_execution_cap_atomic
            .checked_add(self.verification_reserve_cap_atomic)?
            .checked_add(self.chain_fee_reserve_cap_atomic)
    }

    /// Computes the fee using the quote's immutable BPS and cap terms.
    pub fn expected_operator_fee(
        &self,
        accepted_execution_component_atomic: AtomicAmount,
    ) -> Result<AtomicAmount, ProtocolError> {
        require_non_negative(
            accepted_execution_component_atomic,
            "accepted execution component",
        )?;
        if accepted_execution_component_atomic > self.accepted_execution_cap_atomic {
            return Err(ProtocolError::Invariant(
                "accepted execution component exceeds quote cap".into(),
            ));
        }
        let fee = i128::from(accepted_execution_component_atomic.value())
            .checked_mul(i128::from(self.operator_fee_bps))
            .ok_or(ProtocolError::ArithmeticOverflow)?
            / BASIS_POINTS_DENOMINATOR;
        let fee = i64::try_from(fee).map_err(|_| ProtocolError::ArithmeticOverflow)?;
        Ok(AtomicAmount::new(fee).min(self.operator_fee_cap_atomic))
    }

    /// Computes the canonical digest that a funding authorization must bind.
    pub fn canonical_digest(&self) -> Result<Sha256Digest, ProtocolError> {
        self.validate_terms()?;
        canonical::digest(self)
    }
}

/// The customer-signed intent to fund one preallocated reservation for one immutable V2 quote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FundingAuthorizationV2 {
    pub schema_version: u16,
    pub authorization_id: FundingAuthorizationId,
    pub customer_id: CustomerId,
    /// The single reservation this authorization is allowed to fund.
    pub reservation_id: ReservationId,
    pub quote_id: QuoteId,
    pub quote_digest: Sha256Digest,
    pub funding_account_ref: String,
    pub funded_reservation_atomic: AtomicAmount,
    pub issued_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}

impl SignedContract for FundingAuthorizationV2 {
    const CONTRACT_TYPE: &'static str = "funding_authorization_v2";

    fn validate_signed_contract(&self, now_unix_ms: u64) -> Result<(), ProtocolError> {
        self.validate(now_unix_ms)
    }
}

impl FundingAuthorizationV2 {
    /// Validates authorization shape and current validity.
    pub fn validate(&self, now_unix_ms: u64) -> Result<(), ProtocolError> {
        validate_v2(self.schema_version)?;
        require_non_negative(self.funded_reservation_atomic, "funded reservation")?;
        validate_reference("funding account", &self.funding_account_ref)?;
        if self.issued_at_unix_ms > now_unix_ms || self.expires_at_unix_ms <= now_unix_ms {
            return Err(ProtocolError::Invariant(
                "funding authorization is not active".into(),
            ));
        }
        if self.expires_at_unix_ms <= self.issued_at_unix_ms {
            return Err(ProtocolError::Invariant(
                "funding authorization expires before or at issue time".into(),
            ));
        }
        Ok(())
    }

    /// Checks that this authorization is for the exact quote terms and amount.
    pub fn validate_for_quote(
        &self,
        quote: &QuoteV2,
        now_unix_ms: u64,
    ) -> Result<(), ProtocolError> {
        quote.validate(now_unix_ms)?;
        self.validate(now_unix_ms)?;
        if self.quote_id != quote.quote_id
            || self.quote_digest != quote.canonical_digest()?
            || self.funded_reservation_atomic != quote.funded_reservation_atomic()?
            || self.expires_at_unix_ms > quote.expires_at_unix_ms
        {
            return Err(ProtocolError::Invariant(
                "funding authorization does not bind the exact active quote".into(),
            ));
        }
        Ok(())
    }

    /// Verifies a customer signature and the current one-to-one pilot identity binding.
    ///
    /// V2 pilot identities intentionally use the same stable text for a
    /// [`CustomerId`] and the customer [`ActorId`][crate::ids::ActorId] in the
    /// trusted key registry. A federation registry may replace this with an
    /// explicit mapping, but must preserve the typed signer check.
    pub fn verify_customer_envelope(
        envelope: &SignedEnvelopeV1<Self>,
        trusted_key: &TrustedEd25519KeyV1,
        now_unix_ms: u64,
        minimum_sequence: u64,
    ) -> Result<(), ProtocolError> {
        if trusted_key.signer_kind != SignerKind::Customer {
            return Err(ProtocolError::InvalidSignature {
                field: "customer signer kind",
            });
        }
        if trusted_key.signer_id.as_str() != envelope.payload.customer_id.as_str() {
            return Err(ProtocolError::InvalidSignature {
                field: "customer identity binding",
            });
        }
        envelope.verify_with_registry_key(trusted_key, now_unix_ms, minimum_sequence)
    }
}

/// Funds reserved from a verified customer authorization before scheduling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReservationV2 {
    pub schema_version: u16,
    pub reservation_id: ReservationId,
    pub customer_id: CustomerId,
    pub quote_id: QuoteId,
    pub quote_digest: Sha256Digest,
    pub authorization_id: FundingAuthorizationId,
    pub funded_reservation_atomic: AtomicAmount,
    pub funded_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}

impl SignedContract for ReservationV2 {
    const CONTRACT_TYPE: &'static str = "reservation_v2";

    fn validate_signed_contract(&self, now_unix_ms: u64) -> Result<(), ProtocolError> {
        self.validate(now_unix_ms)
    }
}

impl ReservationV2 {
    /// Validates reservation shape and its active funding window.
    pub fn validate(&self, now_unix_ms: u64) -> Result<(), ProtocolError> {
        self.validate_terms()?;
        if self.funded_at_unix_ms > now_unix_ms || self.expires_at_unix_ms <= now_unix_ms {
            return Err(ProtocolError::Invariant("reservation is not active".into()));
        }
        Ok(())
    }

    /// Validates terms that remain meaningful after an expiry decision.
    pub fn validate_terms(&self) -> Result<(), ProtocolError> {
        validate_v2(self.schema_version)?;
        require_non_negative(self.funded_reservation_atomic, "funded reservation")?;
        if self.expires_at_unix_ms <= self.funded_at_unix_ms {
            return Err(ProtocolError::Invariant(
                "reservation expires before or at funding time".into(),
            ));
        }
        Ok(())
    }

    /// Checks that a reservation consumes exactly one customer authorization.
    pub fn validate_for_quote_and_authorization(
        &self,
        quote: &QuoteV2,
        authorization: &FundingAuthorizationV2,
        now_unix_ms: u64,
    ) -> Result<(), ProtocolError> {
        authorization.validate_for_quote(quote, now_unix_ms)?;
        self.validate(now_unix_ms)?;
        if self.customer_id != authorization.customer_id
            || self.reservation_id != authorization.reservation_id
            || self.quote_id != quote.quote_id
            || self.quote_digest != quote.canonical_digest()?
            || self.authorization_id != authorization.authorization_id
            || self.funded_reservation_atomic != quote.funded_reservation_atomic()?
            || self.expires_at_unix_ms > authorization.expires_at_unix_ms
            || self.expires_at_unix_ms > quote.expires_at_unix_ms
        {
            return Err(ProtocolError::Invariant(
                "reservation does not bind its quote and customer authorization".into(),
            ));
        }
        Ok(())
    }
}

/// Accepted work evidence required before a V2 settlement can be created.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptedArtifactV2 {
    pub schema_version: u16,
    pub reservation_id: ReservationId,
    pub quote_id: QuoteId,
    pub quote_digest: Sha256Digest,
    pub pipeline_id: PipelineId,
    pub stage_id: StageId,
    pub attempt_id: AttemptId,
    pub artifact_id: ArtifactId,
    pub artifact_manifest_digest: Sha256Digest,
    pub provider_id: ProviderId,
    pub verification_decision_digest: Sha256Digest,
}

impl AcceptedArtifactV2 {
    /// Validates the self-contained V2 schema marker before lineage checks.
    pub fn validate_terms(&self) -> Result<(), ProtocolError> {
        validate_v2(self.schema_version)
    }

    /// Checks that accepted work belongs to one quoted, funded stage.
    pub fn validate_for_quote_and_reservation(
        &self,
        quote: &QuoteV2,
        reservation: &ReservationV2,
    ) -> Result<(), ProtocolError> {
        self.validate_terms()?;
        reservation.validate_terms()?;
        quote.validate_terms()?;
        if self.reservation_id != reservation.reservation_id
            || self.quote_id != quote.quote_id
            || self.quote_digest != quote.canonical_digest()?
            || self.pipeline_id != quote.pipeline_id
            || self.stage_id != quote.stage_id
            || self.provider_id != quote.provider_id
        {
            return Err(ProtocolError::Invariant(
                "accepted artifact does not match its quote and reservation".into(),
            ));
        }
        Ok(())
    }
}

/// A verifiable claim for reimbursement of a rail fee paid by the quote-bound payer.
///
/// The receipt is deliberately bound to one settlement intent so it cannot be
/// reused to justify a reimbursement on another reservation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChainFeeReceiptV2 {
    pub schema_version: u16,
    pub receipt_id: ChainReceiptId,
    pub rail_id: String,
    pub asset_id: String,
    pub reservation_id: ReservationId,
    pub quote_id: QuoteId,
    pub settlement_id: SettlementId,
    pub settlement_nonce: SettlementNonceId,
    pub payer: PayoutRecipientV2,
    pub transaction_reference: String,
    pub paid_atomic: AtomicAmount,
    pub observed_at_unix_ms: u64,
}

impl ChainFeeReceiptV2 {
    /// Validates a receipt reference and amount before reconciliation.
    pub fn validate(&self) -> Result<(), ProtocolError> {
        validate_v2(self.schema_version)?;
        validate_reference("settlement rail", &self.rail_id)?;
        validate_reference("receipt asset", &self.asset_id)?;
        self.payer.validate()?;
        validate_reference("chain transaction reference", &self.transaction_reference)?;
        require_non_negative(self.paid_atomic, "chain fee receipt")?;
        Ok(())
    }
}

/// The rail attestor's disposition for a submitted settlement transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RailFinalityOutcomeV2 {
    Final,
    NotFinal,
}

/// Signed evidence from a rail-specific attestor for one settlement transaction.
///
/// This contract records a digest of the complete settlement intent and its
/// nonce. It is therefore unusable for another settlement, another retry, or
/// a different submitted transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RailFinalityAttestationV2 {
    pub schema_version: u16,
    pub rail_id: String,
    pub settlement_id: SettlementId,
    pub settlement_nonce: SettlementNonceId,
    pub settlement_digest: Sha256Digest,
    pub transaction_reference: String,
    pub outcome: RailFinalityOutcomeV2,
    pub rail_proof_digest: Sha256Digest,
    pub observed_at_unix_ms: u64,
}

impl SignedContract for RailFinalityAttestationV2 {
    const CONTRACT_TYPE: &'static str = "rail_finality_attestation_v2";

    fn validate_signed_contract(&self, now_unix_ms: u64) -> Result<(), ProtocolError> {
        self.validate_at(now_unix_ms)
    }
}

impl RailFinalityAttestationV2 {
    /// Validates self-contained finality evidence at the attestor's current time.
    pub fn validate_at(&self, now_unix_ms: u64) -> Result<(), ProtocolError> {
        validate_v2(self.schema_version)?;
        validate_reference("settlement rail", &self.rail_id)?;
        validate_reference("chain transaction reference", &self.transaction_reference)?;
        if self.observed_at_unix_ms > now_unix_ms {
            return Err(ProtocolError::Invariant(
                "rail finality observation cannot be in the future".into(),
            ));
        }
        Ok(())
    }

    /// Binds a signed rail disposition to the exact submitted settlement.
    pub fn validate_for_settlement(
        &self,
        settlement: &SettlementV2,
        expected_transaction_reference: &str,
        expected_outcome: RailFinalityOutcomeV2,
    ) -> Result<(), ProtocolError> {
        self.validate_at(self.observed_at_unix_ms)?;
        if self.rail_id != settlement.rail_id
            || self.settlement_id != settlement.settlement_id
            || self.settlement_nonce != settlement.settlement_nonce
            || self.settlement_digest != settlement.canonical_digest()?
            || self.transaction_reference != expected_transaction_reference
            || self.outcome != expected_outcome
        {
            return Err(ProtocolError::Invariant(
                "rail finality evidence does not bind the submitted settlement".into(),
            ));
        }
        Ok(())
    }
}

/// The one idempotent off-chain settlement intent for accepted work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettlementV2 {
    pub schema_version: u16,
    pub settlement_id: SettlementId,
    /// A unique per-settlement nonce retained across reconciliation attempts.
    pub settlement_nonce: SettlementNonceId,
    /// The only rail on which this intent may be submitted and finalized.
    pub rail_id: String,
    pub reservation_id: ReservationId,
    pub quote_id: QuoteId,
    pub quote_digest: Sha256Digest,
    pub pipeline_id: PipelineId,
    pub stage_id: StageId,
    pub attempt_id: AttemptId,
    pub artifact_id: ArtifactId,
    pub artifact_manifest_digest: Sha256Digest,
    pub provider_id: ProviderId,
    pub verification_decision_digest: Sha256Digest,
    pub accepted_execution_component_atomic: AtomicAmount,
    pub provider_payout_atomic: AtomicAmount,
    pub verifier_payout_atomic: AtomicAmount,
    pub operator_fee_atomic: AtomicAmount,
    pub chain_fee_reimbursement_atomic: AtomicAmount,
    pub chain_fee_receipt: Option<ChainFeeReceiptV2>,
}

impl SignedContract for SettlementV2 {
    const CONTRACT_TYPE: &'static str = "settlement_v2";

    fn validate_signed_contract(&self, _now_unix_ms: u64) -> Result<(), ProtocolError> {
        self.validate_terms()
    }
}

impl SettlementV2 {
    /// Validates settlement fields that do not require their source quote.
    pub fn validate_terms(&self) -> Result<(), ProtocolError> {
        validate_v2(self.schema_version)?;
        validate_reference("settlement rail", &self.rail_id)?;
        for (label, amount) in [
            (
                "accepted execution component",
                self.accepted_execution_component_atomic,
            ),
            ("provider payout", self.provider_payout_atomic),
            ("verifier payout", self.verifier_payout_atomic),
            ("operator fee", self.operator_fee_atomic),
            (
                "chain fee reimbursement",
                self.chain_fee_reimbursement_atomic,
            ),
        ] {
            require_non_negative(amount, label)?;
        }
        match (&self.chain_fee_receipt, self.chain_fee_reimbursement_atomic) {
            (Some(receipt), reimbursement) if reimbursement > AtomicAmount::ZERO => {
                receipt.validate()?;
                if receipt.rail_id != self.rail_id
                    || receipt.reservation_id != self.reservation_id
                    || receipt.quote_id != self.quote_id
                    || receipt.settlement_id != self.settlement_id
                    || receipt.settlement_nonce != self.settlement_nonce
                {
                    return Err(ProtocolError::Invariant(
                        "chain fee receipt is not bound to this settlement intent".into(),
                    ));
                }
                Ok(())
            }
            (None, reimbursement) if reimbursement == AtomicAmount::ZERO => Ok(()),
            _ => Err(ProtocolError::Invariant(
                "chain fee reimbursement must have exactly one non-zero receipt".into(),
            )),
        }
    }

    /// Calculates every debit applied to the funded reservation.
    pub fn total_debit_atomic(&self) -> Result<AtomicAmount, ProtocolError> {
        self.provider_payout_atomic
            .checked_add(self.verifier_payout_atomic)?
            .checked_add(self.operator_fee_atomic)?
            .checked_add(self.chain_fee_reimbursement_atomic)
    }

    /// Computes the immutable digest that rail evidence must attest.
    pub fn canonical_digest(&self) -> Result<Sha256Digest, ProtocolError> {
        self.validate_terms()?;
        canonical::digest(self)
    }

    /// Validates the economic and lineage invariants against the immutable source records.
    #[allow(clippy::too_many_lines)]
    pub fn validate_for_quote_reservation_and_artifact(
        &self,
        quote: &QuoteV2,
        reservation: &ReservationV2,
        accepted_artifact: &AcceptedArtifactV2,
    ) -> Result<(), ProtocolError> {
        self.validate_terms()?;
        quote.validate_terms()?;
        reservation.validate_terms()?;
        accepted_artifact.validate_for_quote_and_reservation(quote, reservation)?;
        if self.reservation_id != reservation.reservation_id
            || self.quote_id != quote.quote_id
            || self.quote_digest != quote.canonical_digest()?
            || self.pipeline_id != accepted_artifact.pipeline_id
            || self.stage_id != accepted_artifact.stage_id
            || self.attempt_id != accepted_artifact.attempt_id
            || self.artifact_id != accepted_artifact.artifact_id
            || self.artifact_manifest_digest != accepted_artifact.artifact_manifest_digest
            || self.provider_id != accepted_artifact.provider_id
            || self.verification_decision_digest != accepted_artifact.verification_decision_digest
        {
            return Err(ProtocolError::Invariant(
                "settlement does not match its accepted artifact lineage".into(),
            ));
        }
        if self.accepted_execution_component_atomic > quote.accepted_execution_cap_atomic {
            return Err(ProtocolError::Invariant(
                "accepted execution component exceeds quote cap".into(),
            ));
        }
        let expected_fee = quote.expected_operator_fee(self.accepted_execution_component_atomic)?;
        if self.operator_fee_atomic != expected_fee {
            return Err(ProtocolError::Invariant(
                "operator fee does not match the quote formula".into(),
            ));
        }
        let expected_provider_payout = self
            .accepted_execution_component_atomic
            .checked_sub(self.operator_fee_atomic)?;
        if self.provider_payout_atomic != expected_provider_payout
            || self.provider_payout_atomic > quote.provider_payout_cap_atomic
        {
            return Err(ProtocolError::Invariant(
                "provider payout does not match its capped execution component".into(),
            ));
        }
        if self.verifier_payout_atomic > quote.verifier_payout_cap_atomic
            || self.verifier_payout_atomic > quote.verification_reserve_cap_atomic
        {
            return Err(ProtocolError::Invariant(
                "verifier payout exceeds its quote cap".into(),
            ));
        }
        if self.chain_fee_reimbursement_atomic > quote.chain_fee_reserve_cap_atomic {
            return Err(ProtocolError::Invariant(
                "chain fee reimbursement exceeds its quote cap".into(),
            ));
        }
        if let Some(receipt) = &self.chain_fee_receipt
            && (receipt.payer != quote.chain_fee_payer
                || receipt.asset_id != quote.asset.asset_id
                || receipt.paid_atomic < self.chain_fee_reimbursement_atomic)
        {
            return Err(ProtocolError::Invariant(
                "chain fee receipt does not support the requested reimbursement".into(),
            ));
        }
        if self.total_debit_atomic()? > reservation.funded_reservation_atomic {
            return Err(ProtocolError::Invariant(
                "settlement debits exceed funded reservation".into(),
            ));
        }
        Ok(())
    }
}

fn validate_v2(actual: u16) -> Result<(), ProtocolError> {
    if actual == SCHEMA_VERSION_V2 {
        Ok(())
    } else {
        Err(ProtocolError::UnsupportedSchemaVersion {
            actual,
            expected: SCHEMA_VERSION_V2,
        })
    }
}

fn require_non_negative(amount: AtomicAmount, label: &'static str) -> Result<(), ProtocolError> {
    if amount < AtomicAmount::ZERO {
        Err(ProtocolError::Invariant(format!(
            "{label} must be non-negative"
        )))
    } else {
        Ok(())
    }
}

fn validate_reference(label: &'static str, value: &str) -> Result<(), ProtocolError> {
    if value.is_empty()
        || value.len() > 256
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err(ProtocolError::Invariant(format!(
            "{label} must contain 1 to 256 ASCII reference bytes"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
    use ed25519_dalek::{Signer as _, SigningKey};

    use super::{
        AcceptedArtifactV2, AtomicAmount, ChainFeeReceiptV2, FundingAuthorizationV2,
        PayoutRecipientV2, QuoteV2, RailFinalityAttestationV2, RailFinalityOutcomeV2,
        ReservationV2, SettlementAssetV2, SettlementV2,
    };
    use crate::{
        SCHEMA_VERSION_V1, SCHEMA_VERSION_V2,
        digest::Sha256Digest,
        ids::{
            ActorId, ArtifactId, AttemptId, ChainReceiptId, CustomerId, FundingAuthorizationId,
            KeyId, MessageId, PipelineId, ProviderId, QuoteId, ReservationId, SettlementId,
            SettlementNonceId, StageId,
        },
        signing::{
            Ed25519SignatureV1, SignedContract, SignedEnvelopeV1, SignerKind, TrustedEd25519KeyV1,
        },
    };

    fn recipient(actor: &str, destination: &str) -> PayoutRecipientV2 {
        PayoutRecipientV2 {
            actor_id: ActorId::new(actor).unwrap(),
            destination_ref: destination.into(),
        }
    }

    fn quote() -> QuoteV2 {
        QuoteV2 {
            schema_version: SCHEMA_VERSION_V2,
            quote_id: QuoteId::new("quote-v2-1").unwrap(),
            pipeline_id: PipelineId::new("pipeline-1").unwrap(),
            stage_id: StageId::new("stage-1").unwrap(),
            provider_id: ProviderId::new("provider-1").unwrap(),
            asset: SettlementAssetV2 {
                asset_id: "test:credits".into(),
                decimals: 6,
                accounting_unit: "usd-micro".into(),
                conversion_source: "fixed:pilot-v1".into(),
                conversion_observed_at_unix_ms: 900,
            },
            accepted_execution_cap_atomic: AtomicAmount::new(1_000),
            verification_reserve_cap_atomic: AtomicAmount::new(200),
            chain_fee_reserve_cap_atomic: AtomicAmount::new(50),
            provider_payout_cap_atomic: AtomicAmount::new(950),
            verifier_payout_cap_atomic: AtomicAmount::new(150),
            operator_fee_bps: 500,
            operator_fee_cap_atomic: AtomicAmount::new(60),
            provider_recipient: recipient("provider-actor", "wallet:provider-1"),
            verifier_recipient: recipient("verifier-actor", "wallet:verifier-1"),
            operator_recipient: recipient("operator-actor", "wallet:operator-1"),
            chain_fee_payer: recipient("operator-actor", "wallet:operator-1"),
            pricing_policy_version: 1,
            expires_at_unix_ms: 2_000,
        }
    }

    fn authorization(quote: &QuoteV2) -> FundingAuthorizationV2 {
        FundingAuthorizationV2 {
            schema_version: SCHEMA_VERSION_V2,
            authorization_id: FundingAuthorizationId::new("authorization-1").unwrap(),
            customer_id: CustomerId::new("customer-1").unwrap(),
            reservation_id: ReservationId::new("reservation-v2-1").unwrap(),
            quote_id: quote.quote_id.clone(),
            quote_digest: quote.canonical_digest().unwrap(),
            funding_account_ref: "wallet:customer-1".into(),
            funded_reservation_atomic: quote.funded_reservation_atomic().unwrap(),
            issued_at_unix_ms: 1_000,
            expires_at_unix_ms: 1_900,
        }
    }

    fn signed_customer_authorization(
        authorization: FundingAuthorizationV2,
    ) -> (
        SignedEnvelopeV1<FundingAuthorizationV2>,
        TrustedEd25519KeyV1,
    ) {
        let signing_key = SigningKey::from_bytes(&[13_u8; 32]);
        let trusted_key = TrustedEd25519KeyV1 {
            schema_version: SCHEMA_VERSION_V1,
            key_id: KeyId::new("customer-key-1").unwrap(),
            signer_kind: SignerKind::Customer,
            signer_id: ActorId::new("customer-1").unwrap(),
            public_key_base64: STANDARD_NO_PAD.encode(signing_key.verifying_key().as_bytes()),
            valid_from_unix_ms: 900,
            expires_at_unix_ms: 2_000,
        };
        let mut envelope = SignedEnvelopeV1 {
            schema_version: SCHEMA_VERSION_V1,
            contract_type: FundingAuthorizationV2::CONTRACT_TYPE.into(),
            message_id: MessageId::new("customer-authorization-1").unwrap(),
            sequence: 1,
            issued_at_unix_ms: 1_000,
            expires_at_unix_ms: Some(1_900),
            payload: authorization,
            signature: Ed25519SignatureV1 {
                key_id: trusted_key.key_id.clone(),
                signer_kind: trusted_key.signer_kind,
                signer_id: trusted_key.signer_id.clone(),
                signature_base64: String::new(),
            },
        };
        let signature = signing_key.sign(&envelope.signing_bytes().unwrap());
        envelope.signature.signature_base64 = STANDARD_NO_PAD.encode(signature.to_bytes());
        (envelope, trusted_key)
    }

    fn reservation(quote: &QuoteV2, authorization: &FundingAuthorizationV2) -> ReservationV2 {
        ReservationV2 {
            schema_version: SCHEMA_VERSION_V2,
            reservation_id: authorization.reservation_id.clone(),
            customer_id: authorization.customer_id.clone(),
            quote_id: quote.quote_id.clone(),
            quote_digest: quote.canonical_digest().unwrap(),
            authorization_id: authorization.authorization_id.clone(),
            funded_reservation_atomic: quote.funded_reservation_atomic().unwrap(),
            funded_at_unix_ms: 1_100,
            expires_at_unix_ms: 1_800,
        }
    }

    fn acceptance(quote: &QuoteV2, reservation: &ReservationV2) -> AcceptedArtifactV2 {
        AcceptedArtifactV2 {
            schema_version: SCHEMA_VERSION_V2,
            reservation_id: reservation.reservation_id.clone(),
            quote_id: quote.quote_id.clone(),
            quote_digest: quote.canonical_digest().unwrap(),
            pipeline_id: quote.pipeline_id.clone(),
            stage_id: quote.stage_id.clone(),
            attempt_id: AttemptId::new("attempt-1").unwrap(),
            artifact_id: ArtifactId::new("artifact-1").unwrap(),
            artifact_manifest_digest: Sha256Digest::of_bytes(b"manifest"),
            provider_id: quote.provider_id.clone(),
            verification_decision_digest: Sha256Digest::of_bytes(b"verification"),
        }
    }

    fn settlement(
        quote: &QuoteV2,
        reservation: &ReservationV2,
        acceptance: &AcceptedArtifactV2,
    ) -> SettlementV2 {
        SettlementV2 {
            schema_version: SCHEMA_VERSION_V2,
            settlement_id: SettlementId::new("settlement-v2-1").unwrap(),
            settlement_nonce: SettlementNonceId::new("settlement-nonce-1").unwrap(),
            rail_id: "test:rail".into(),
            reservation_id: reservation.reservation_id.clone(),
            quote_id: quote.quote_id.clone(),
            quote_digest: quote.canonical_digest().unwrap(),
            pipeline_id: acceptance.pipeline_id.clone(),
            stage_id: acceptance.stage_id.clone(),
            attempt_id: acceptance.attempt_id.clone(),
            artifact_id: acceptance.artifact_id.clone(),
            artifact_manifest_digest: acceptance.artifact_manifest_digest,
            provider_id: acceptance.provider_id.clone(),
            verification_decision_digest: acceptance.verification_decision_digest,
            accepted_execution_component_atomic: AtomicAmount::new(1_000),
            provider_payout_atomic: AtomicAmount::new(950),
            verifier_payout_atomic: AtomicAmount::new(100),
            operator_fee_atomic: AtomicAmount::new(50),
            chain_fee_reimbursement_atomic: AtomicAmount::new(20),
            chain_fee_receipt: Some(ChainFeeReceiptV2 {
                schema_version: SCHEMA_VERSION_V2,
                receipt_id: ChainReceiptId::new("receipt-1").unwrap(),
                rail_id: "test:rail".into(),
                asset_id: quote.asset.asset_id.clone(),
                reservation_id: reservation.reservation_id.clone(),
                quote_id: quote.quote_id.clone(),
                settlement_id: SettlementId::new("settlement-v2-1").unwrap(),
                settlement_nonce: SettlementNonceId::new("settlement-nonce-1").unwrap(),
                payer: quote.chain_fee_payer.clone(),
                transaction_reference: "solana:transaction-1".into(),
                paid_atomic: AtomicAmount::new(20),
                observed_at_unix_ms: 1_500,
            }),
        }
    }

    #[test]
    fn quote_authorization_and_settlement_conserve_atomic_funding() {
        let quote = quote();
        let authorization = authorization(&quote);
        let reservation = reservation(&quote, &authorization);
        let acceptance = acceptance(&quote, &reservation);
        let settlement = settlement(&quote, &reservation, &acceptance);

        authorization.validate_for_quote(&quote, 1_200).unwrap();
        reservation
            .validate_for_quote_and_authorization(&quote, &authorization, 1_200)
            .unwrap();
        settlement
            .validate_for_quote_reservation_and_artifact(&quote, &reservation, &acceptance)
            .unwrap();
        assert_eq!(
            settlement.total_debit_atomic().unwrap(),
            AtomicAmount::new(1_120)
        );
        assert_eq!(
            reservation
                .funded_reservation_atomic
                .checked_sub(settlement.total_debit_atomic().unwrap())
                .unwrap(),
            AtomicAmount::new(130)
        );
    }

    #[test]
    fn customer_authorization_and_rail_evidence_are_bound_to_one_settlement() {
        let quote = quote();
        let authorization = authorization(&quote);
        let (envelope, trusted_key) = signed_customer_authorization(authorization.clone());
        FundingAuthorizationV2::verify_customer_envelope(&envelope, &trusted_key, 1_200, 1)
            .unwrap();

        let mut wrong_customer_key = trusted_key.clone();
        wrong_customer_key.signer_id = ActorId::new("customer-2").unwrap();
        assert!(
            FundingAuthorizationV2::verify_customer_envelope(
                &envelope,
                &wrong_customer_key,
                1_200,
                1,
            )
            .is_err()
        );

        let mut mismatched_reservation = reservation(&quote, &authorization);
        mismatched_reservation.reservation_id = ReservationId::new("reservation-v2-2").unwrap();
        assert!(
            mismatched_reservation
                .validate_for_quote_and_authorization(&quote, &authorization, 1_200)
                .is_err()
        );

        let reservation = reservation(&quote, &authorization);
        let accepted = acceptance(&quote, &reservation);
        let settlement = settlement(&quote, &reservation, &accepted);
        let attestation = RailFinalityAttestationV2 {
            schema_version: SCHEMA_VERSION_V2,
            rail_id: settlement.rail_id.clone(),
            settlement_id: settlement.settlement_id.clone(),
            settlement_nonce: settlement.settlement_nonce.clone(),
            settlement_digest: settlement.canonical_digest().unwrap(),
            transaction_reference: "solana:transaction-1".into(),
            outcome: RailFinalityOutcomeV2::Final,
            rail_proof_digest: Sha256Digest::of_bytes(b"rail-proof"),
            observed_at_unix_ms: 1_700,
        };
        attestation
            .validate_for_settlement(
                &settlement,
                "solana:transaction-1",
                RailFinalityOutcomeV2::Final,
            )
            .unwrap();

        let mut reused_receipt = settlement.clone();
        reused_receipt
            .chain_fee_receipt
            .as_mut()
            .unwrap()
            .settlement_nonce = SettlementNonceId::new("settlement-nonce-2").unwrap();
        assert!(reused_receipt.validate_terms().is_err());
    }

    #[test]
    fn altered_fee_or_unquoted_chain_reimbursement_is_rejected() {
        let quote = quote();
        let authorization = authorization(&quote);
        let reservation = reservation(&quote, &authorization);
        let acceptance = acceptance(&quote, &reservation);
        let mut altered_fee = settlement(&quote, &reservation, &acceptance);
        altered_fee.operator_fee_atomic = AtomicAmount::new(49);
        assert!(
            altered_fee
                .validate_for_quote_reservation_and_artifact(&quote, &reservation, &acceptance)
                .is_err()
        );

        let mut altered_reimbursement = settlement(&quote, &reservation, &acceptance);
        altered_reimbursement.chain_fee_reimbursement_atomic = AtomicAmount::new(51);
        altered_reimbursement
            .chain_fee_receipt
            .as_mut()
            .unwrap()
            .paid_atomic = AtomicAmount::new(51);
        assert!(
            altered_reimbursement
                .validate_for_quote_reservation_and_artifact(&quote, &reservation, &acceptance)
                .is_err()
        );
    }

    #[test]
    fn atomic_amount_wire_form_rejects_rounding_and_noncanonical_text() {
        for invalid in ["+1", "01", "-0", "1.0", "1e3", " 1"] {
            assert!(invalid.parse::<AtomicAmount>().is_err(), "{invalid}");
        }
        let amount = AtomicAmount::new(i64::MAX);
        assert_eq!(
            serde_json::to_string(&amount).unwrap(),
            format!("\"{}\"", i64::MAX)
        );
    }
}
