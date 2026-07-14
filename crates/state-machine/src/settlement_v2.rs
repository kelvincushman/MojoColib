//! Replay-safe V2 settlement lifecycle with explicit off-chain and chain authority.
//!
//! The coordinator uses this reducer for a single reservation. It can decide
//! whether work is eligible for settlement, but cannot treat a submission as
//! financially final. Only a confirmed or reconciled chain-finality event can
//! enter [`SettlementPhaseV2::FinancialFinal`].

use std::collections::BTreeMap;

use colibri_protocol::{
    ProtocolError, canonical,
    digest::Sha256Digest,
    ids::MessageId,
    settlement_v2::{
        AcceptedArtifactV2, FundingAuthorizationV2, QuoteV2, RailFinalityAttestationV2,
        RailFinalityOutcomeV2, ReservationV2, SettlementV2,
    },
    signing::{SignedContract, SignedEnvelopeV1},
};
use serde::{Deserialize, Serialize};

use crate::{ApplyOutcome, EventEnvelope, TransitionError};

/// The authoritative lifecycle for a single V2 reservation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettlementPhaseV2 {
    Draft,
    QuoteAuthorised,
    Reserved,
    WorkAccepted,
    SettlementIntent,
    ChainPending,
    FinancialFinal,
    ExpiredRefund,
    ReconciliationRequired,
}

impl SettlementPhaseV2 {
    const fn name(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::QuoteAuthorised => "quote_authorised",
            Self::Reserved => "reserved",
            Self::WorkAccepted => "work_accepted",
            Self::SettlementIntent => "settlement_intent",
            Self::ChainPending => "chain_pending",
            Self::FinancialFinal => "financial_final",
            Self::ExpiredRefund => "expired_refund",
            Self::ReconciliationRequired => "reconciliation_required",
        }
    }
}

/// Inputs applied by a reservation-pinned settlement sequencer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum SettlementEventV2 {
    QuoteAuthorised {
        quote: QuoteV2,
        authorization: SignedEnvelopeV1<FundingAuthorizationV2>,
        observed_at_unix_ms: u64,
    },
    ReservationCreated {
        reservation: ReservationV2,
        observed_at_unix_ms: u64,
    },
    WorkAccepted {
        accepted_artifact: AcceptedArtifactV2,
        observed_at_unix_ms: u64,
    },
    SettlementIntentCreated {
        settlement: SettlementV2,
        observed_at_unix_ms: u64,
    },
    ChainSubmitted {
        transaction_reference: String,
    },
    ChainConfirmationRecorded {
        finality: SignedEnvelopeV1<RailFinalityAttestationV2>,
    },
    ChainOutcomeUncertain {
        transaction_reference: String,
        reason_code: String,
    },
    ChainRetryAuthorised {
        reconciliation: SignedEnvelopeV1<RailFinalityAttestationV2>,
    },
    ChainFinalityReconciled {
        finality: SignedEnvelopeV1<RailFinalityAttestationV2>,
    },
    ReservationExpired {
        observed_at_unix_ms: u64,
    },
}

impl SettlementEventV2 {
    const fn name(&self) -> &'static str {
        match self {
            Self::QuoteAuthorised { .. } => "quote_authorised",
            Self::ReservationCreated { .. } => "reservation_created",
            Self::WorkAccepted { .. } => "work_accepted",
            Self::SettlementIntentCreated { .. } => "settlement_intent_created",
            Self::ChainSubmitted { .. } => "chain_submitted",
            Self::ChainConfirmationRecorded { .. } => "chain_confirmation_recorded",
            Self::ChainOutcomeUncertain { .. } => "chain_outcome_uncertain",
            Self::ChainRetryAuthorised { .. } => "chain_retry_authorised",
            Self::ChainFinalityReconciled { .. } => "chain_finality_reconciled",
            Self::ReservationExpired { .. } => "reservation_expired",
        }
    }

    /// Returns the coordinator-recorded lifecycle time when this event has one.
    #[must_use]
    pub const fn observed_at_unix_ms(&self) -> Option<u64> {
        match self {
            Self::QuoteAuthorised {
                observed_at_unix_ms,
                ..
            }
            | Self::ReservationCreated {
                observed_at_unix_ms,
                ..
            }
            | Self::WorkAccepted {
                observed_at_unix_ms,
                ..
            }
            | Self::SettlementIntentCreated {
                observed_at_unix_ms,
                ..
            }
            | Self::ReservationExpired {
                observed_at_unix_ms,
            } => Some(*observed_at_unix_ms),
            Self::ChainSubmitted { .. }
            | Self::ChainConfirmationRecorded { .. }
            | Self::ChainOutcomeUncertain { .. }
            | Self::ChainRetryAuthorised { .. }
            | Self::ChainFinalityReconciled { .. } => None,
        }
    }
}

impl SignedContract for SettlementEventV2 {
    const CONTRACT_TYPE: &'static str = "settlement_event_v2";

    fn validate_signed_contract(&self, now_unix_ms: u64) -> Result<(), ProtocolError> {
        match self {
            Self::QuoteAuthorised {
                quote,
                authorization,
                observed_at_unix_ms,
            } => {
                validate_observed_at(*observed_at_unix_ms, now_unix_ms)?;
                authorization
                    .payload
                    .validate_for_quote(quote, *observed_at_unix_ms)?;
            }
            Self::ReservationCreated {
                reservation,
                observed_at_unix_ms,
            } => {
                validate_observed_at(*observed_at_unix_ms, now_unix_ms)?;
                reservation.validate(*observed_at_unix_ms)?;
            }
            Self::WorkAccepted {
                accepted_artifact,
                observed_at_unix_ms,
            } => {
                validate_observed_at(*observed_at_unix_ms, now_unix_ms)?;
                accepted_artifact.validate_terms()?;
            }
            Self::SettlementIntentCreated {
                settlement,
                observed_at_unix_ms,
            } => {
                validate_observed_at(*observed_at_unix_ms, now_unix_ms)?;
                settlement.validate_terms()?;
            }
            Self::ChainConfirmationRecorded { finality }
            | Self::ChainFinalityReconciled { finality } => {
                finality.payload.validate_at(now_unix_ms)?;
            }
            Self::ChainRetryAuthorised { reconciliation } => {
                reconciliation.payload.validate_at(now_unix_ms)?;
            }
            Self::ReservationExpired {
                observed_at_unix_ms,
            } => validate_observed_at(*observed_at_unix_ms, now_unix_ms)?,
            Self::ChainSubmitted { .. } | Self::ChainOutcomeUncertain { .. } => {}
        }
        Ok(())
    }
}

/// Materialized state reconstructed solely from ordered signed events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SettlementMachineV2 {
    phase: SettlementPhaseV2,
    quote: Option<QuoteV2>,
    authorization: Option<FundingAuthorizationV2>,
    reservation: Option<ReservationV2>,
    accepted_artifact: Option<AcceptedArtifactV2>,
    settlement: Option<SettlementV2>,
    chain_transaction_reference: Option<String>,
    last_lifecycle_observed_at_unix_ms: Option<u64>,
    sequence: u64,
    applied_events: BTreeMap<MessageId, Sha256Digest>,
}

impl Default for SettlementMachineV2 {
    fn default() -> Self {
        Self::new()
    }
}

impl SettlementMachineV2 {
    /// Creates an empty reservation lifecycle.
    #[must_use]
    pub fn new() -> Self {
        Self {
            phase: SettlementPhaseV2::Draft,
            quote: None,
            authorization: None,
            reservation: None,
            accepted_artifact: None,
            settlement: None,
            chain_transaction_reference: None,
            last_lifecycle_observed_at_unix_ms: None,
            sequence: 0,
            applied_events: BTreeMap::new(),
        }
    }

    /// Returns the current lifecycle phase.
    #[must_use]
    pub const fn phase(&self) -> SettlementPhaseV2 {
        self.phase
    }

    /// Returns the accepted quote, if authorization has started.
    #[must_use]
    pub const fn quote(&self) -> Option<&QuoteV2> {
        self.quote.as_ref()
    }

    /// Returns the funded reservation, if present.
    #[must_use]
    pub const fn reservation(&self) -> Option<&ReservationV2> {
        self.reservation.as_ref()
    }

    /// Returns the accepted artifact evidence, if present.
    #[must_use]
    pub const fn accepted_artifact(&self) -> Option<&AcceptedArtifactV2> {
        self.accepted_artifact.as_ref()
    }

    /// Returns the idempotent settlement intent, if present.
    #[must_use]
    pub const fn settlement(&self) -> Option<&SettlementV2> {
        self.settlement.as_ref()
    }

    /// Returns the submitted chain reference, which remains visible for reconciliation.
    #[must_use]
    pub fn chain_transaction_reference(&self) -> Option<&str> {
        self.chain_transaction_reference.as_deref()
    }

    /// Returns the count of successfully applied state-changing events.
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Applies an already-authorized event exactly once.
    ///
    /// This is a pure, trusted reducer entry point. It does not verify customer,
    /// sequencer, or rail signatures; service callers must use
    /// `colibri_coordinator_core::SettlementSequencer` instead.
    pub fn apply_trusted(
        &mut self,
        envelope: &EventEnvelope<SettlementEventV2>,
    ) -> Result<ApplyOutcome, TransitionError> {
        let event_digest = canonical::digest(&envelope.payload)?;
        if let Some(previous_digest) = self.applied_events.get(&envelope.event_id) {
            return if *previous_digest == event_digest {
                Ok(ApplyOutcome::DuplicateIgnored)
            } else {
                Err(TransitionError::EventIdConflict)
            };
        }

        let mut next = self.clone();
        next.apply_new_event(&envelope.payload)?;
        next.applied_events
            .insert(envelope.event_id.clone(), event_digest);
        *self = next;
        Ok(ApplyOutcome::Applied)
    }

    #[allow(clippy::too_many_lines)]
    fn apply_new_event(&mut self, event: &SettlementEventV2) -> Result<(), TransitionError> {
        match (&self.phase, event) {
            (
                SettlementPhaseV2::Draft,
                SettlementEventV2::QuoteAuthorised {
                    quote,
                    authorization,
                    observed_at_unix_ms,
                },
            ) => {
                self.record_lifecycle_observation(*observed_at_unix_ms)?;
                authorization
                    .payload
                    .validate_for_quote(quote, *observed_at_unix_ms)?;
                self.quote = Some(quote.clone());
                self.authorization = Some(authorization.payload.clone());
                self.phase = SettlementPhaseV2::QuoteAuthorised;
            }
            (
                SettlementPhaseV2::QuoteAuthorised,
                SettlementEventV2::ReservationCreated {
                    reservation,
                    observed_at_unix_ms,
                },
            ) => {
                self.record_lifecycle_observation(*observed_at_unix_ms)?;
                reservation.validate_for_quote_and_authorization(
                    self.quote
                        .as_ref()
                        .expect("quote is set in quote_authorised phase"),
                    self.authorization
                        .as_ref()
                        .expect("authorization is set in quote_authorised phase"),
                    *observed_at_unix_ms,
                )?;
                self.reservation = Some(reservation.clone());
                self.phase = SettlementPhaseV2::Reserved;
            }
            (
                SettlementPhaseV2::Reserved,
                SettlementEventV2::WorkAccepted {
                    accepted_artifact,
                    observed_at_unix_ms,
                },
            ) => {
                self.record_lifecycle_observation(*observed_at_unix_ms)?;
                self.require_before_reservation_expiry(*observed_at_unix_ms)?;
                accepted_artifact.validate_for_quote_and_reservation(
                    self.quote.as_ref().expect("quote is set after reservation"),
                    self.reservation
                        .as_ref()
                        .expect("reservation is set in reserved phase"),
                )?;
                self.accepted_artifact = Some(accepted_artifact.clone());
                self.phase = SettlementPhaseV2::WorkAccepted;
            }
            (
                SettlementPhaseV2::WorkAccepted,
                SettlementEventV2::SettlementIntentCreated {
                    settlement,
                    observed_at_unix_ms,
                },
            ) => {
                self.record_lifecycle_observation(*observed_at_unix_ms)?;
                self.require_before_reservation_expiry(*observed_at_unix_ms)?;
                settlement.validate_for_quote_reservation_and_artifact(
                    self.quote.as_ref().expect("quote is set after reservation"),
                    self.reservation
                        .as_ref()
                        .expect("reservation is set after reservation"),
                    self.accepted_artifact
                        .as_ref()
                        .expect("artifact is set in work_accepted phase"),
                )?;
                self.settlement = Some(settlement.clone());
                self.phase = SettlementPhaseV2::SettlementIntent;
            }
            (
                SettlementPhaseV2::SettlementIntent,
                SettlementEventV2::ChainSubmitted {
                    transaction_reference,
                },
            ) => {
                validate_reference("chain transaction reference", transaction_reference)?;
                if self
                    .settlement
                    .as_ref()
                    .and_then(|settlement| settlement.chain_fee_receipt.as_ref())
                    .is_some_and(|receipt| {
                        receipt.transaction_reference.as_str() != transaction_reference
                    })
                {
                    return Err(TransitionError::Protocol(
                        "chain fee receipt does not match the submitted transaction".into(),
                    ));
                }
                self.chain_transaction_reference = Some(transaction_reference.clone());
                self.phase = SettlementPhaseV2::ChainPending;
            }
            (
                SettlementPhaseV2::ChainPending,
                SettlementEventV2::ChainConfirmationRecorded { finality },
            ) => {
                let transaction_reference =
                    self.chain_transaction_reference.as_deref().ok_or_else(|| {
                        TransitionError::Protocol(
                            "chain confirmation has no submitted transaction".into(),
                        )
                    })?;
                finality.payload.validate_for_settlement(
                    self.settlement
                        .as_ref()
                        .expect("settlement is set in chain_pending phase"),
                    transaction_reference,
                    RailFinalityOutcomeV2::Final,
                )?;
                self.phase = SettlementPhaseV2::FinancialFinal;
            }
            (
                SettlementPhaseV2::ChainPending,
                SettlementEventV2::ChainOutcomeUncertain {
                    transaction_reference,
                    reason_code,
                },
            ) => {
                self.require_chain_reference(transaction_reference)?;
                validate_reference("chain uncertainty reason", reason_code)?;
                self.phase = SettlementPhaseV2::ReconciliationRequired;
            }
            (
                SettlementPhaseV2::ReconciliationRequired,
                SettlementEventV2::ChainRetryAuthorised { reconciliation },
            ) => {
                let transaction_reference =
                    self.chain_transaction_reference.as_deref().ok_or_else(|| {
                        TransitionError::Protocol("chain retry has no submitted transaction".into())
                    })?;
                reconciliation.payload.validate_for_settlement(
                    self.settlement
                        .as_ref()
                        .expect("settlement is set in reconciliation_required phase"),
                    transaction_reference,
                    RailFinalityOutcomeV2::NotFinal,
                )?;
                self.chain_transaction_reference = None;
                self.phase = SettlementPhaseV2::SettlementIntent;
            }
            (
                SettlementPhaseV2::ReconciliationRequired,
                SettlementEventV2::ChainFinalityReconciled { finality },
            ) => {
                let transaction_reference =
                    self.chain_transaction_reference.as_deref().ok_or_else(|| {
                        TransitionError::Protocol(
                            "reconciled finality has no submitted transaction".into(),
                        )
                    })?;
                finality.payload.validate_for_settlement(
                    self.settlement
                        .as_ref()
                        .expect("settlement is set in reconciliation_required phase"),
                    transaction_reference,
                    RailFinalityOutcomeV2::Final,
                )?;
                self.phase = SettlementPhaseV2::FinancialFinal;
            }
            (
                SettlementPhaseV2::Reserved
                | SettlementPhaseV2::WorkAccepted
                | SettlementPhaseV2::SettlementIntent,
                SettlementEventV2::ReservationExpired {
                    observed_at_unix_ms,
                },
            ) => {
                self.record_lifecycle_observation(*observed_at_unix_ms)?;
                let reservation = self.reservation.as_ref().ok_or_else(|| {
                    TransitionError::Protocol("cannot expire an unfunded reservation".into())
                })?;
                if *observed_at_unix_ms < reservation.expires_at_unix_ms {
                    return Err(TransitionError::Protocol(
                        "reservation expiry was recorded before its expiry time".into(),
                    ));
                }
                self.phase = SettlementPhaseV2::ExpiredRefund;
            }
            _ => {
                return Err(TransitionError::InvalidTransition {
                    state: self.phase.name(),
                    event: event.name(),
                });
            }
        }
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or(TransitionError::ArithmeticOverflow)?;
        Ok(())
    }

    fn require_chain_reference(&self, provided: &str) -> Result<(), TransitionError> {
        if self.chain_transaction_reference.as_deref() != Some(provided) {
            return Err(TransitionError::Protocol(
                "chain event does not match the submitted transaction".into(),
            ));
        }
        Ok(())
    }

    fn require_before_reservation_expiry(
        &self,
        observed_at_unix_ms: u64,
    ) -> Result<(), TransitionError> {
        let reservation = self.reservation.as_ref().ok_or_else(|| {
            TransitionError::Protocol("active lifecycle has no funded reservation".into())
        })?;
        if observed_at_unix_ms >= reservation.expires_at_unix_ms {
            return Err(TransitionError::Protocol(
                "work or settlement intent was recorded after reservation expiry".into(),
            ));
        }
        Ok(())
    }

    fn record_lifecycle_observation(
        &mut self,
        observed_at_unix_ms: u64,
    ) -> Result<(), TransitionError> {
        if self
            .last_lifecycle_observed_at_unix_ms
            .is_some_and(|previous| observed_at_unix_ms < previous)
        {
            return Err(TransitionError::Protocol(
                "settlement lifecycle observation moved backwards in time".into(),
            ));
        }
        self.last_lifecycle_observed_at_unix_ms = Some(observed_at_unix_ms);
        Ok(())
    }
}

fn validate_observed_at(observed_at_unix_ms: u64, now_unix_ms: u64) -> Result<(), ProtocolError> {
    if observed_at_unix_ms > now_unix_ms {
        return Err(ProtocolError::Invariant(
            "settlement event observation cannot be in the future".into(),
        ));
    }
    Ok(())
}

fn validate_reference(label: &'static str, value: &str) -> Result<(), TransitionError> {
    if value.is_empty()
        || value.len() > 256
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err(TransitionError::Protocol(format!(
            "{label} must contain 1 to 256 ASCII reference bytes"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{SettlementEventV2, SettlementMachineV2, SettlementPhaseV2};
    use crate::{ApplyOutcome, EventEnvelope, TransitionError};
    use colibri_protocol::{
        SCHEMA_VERSION_V1, SCHEMA_VERSION_V2,
        digest::Sha256Digest,
        ids::{
            ActorId, ArtifactId, AttemptId, ChainReceiptId, CustomerId, FundingAuthorizationId,
            KeyId, MessageId, PipelineId, ProviderId, QuoteId, ReservationId, SettlementId,
            SettlementNonceId, StageId,
        },
        settlement_v2::{
            AcceptedArtifactV2, AtomicAmount, ChainFeeReceiptV2, FundingAuthorizationV2,
            PayoutRecipientV2, QuoteV2, RailFinalityAttestationV2, RailFinalityOutcomeV2,
            ReservationV2, SettlementAssetV2, SettlementV2,
        },
        signing::{Ed25519SignatureV1, SignedContract, SignedEnvelopeV1, SignerKind},
    };

    fn event(id: &str, payload: SettlementEventV2) -> EventEnvelope<SettlementEventV2> {
        EventEnvelope::new(MessageId::new(id).unwrap(), payload)
    }

    fn unsigned_envelope<T: SignedContract>(
        id: &str,
        signer_kind: SignerKind,
        signer_id: &str,
        payload: T,
    ) -> SignedEnvelopeV1<T> {
        SignedEnvelopeV1 {
            schema_version: SCHEMA_VERSION_V1,
            contract_type: T::CONTRACT_TYPE.into(),
            message_id: MessageId::new(id).unwrap(),
            sequence: 1,
            issued_at_unix_ms: 1_700,
            expires_at_unix_ms: Some(1_900),
            payload,
            signature: Ed25519SignatureV1 {
                key_id: KeyId::new(format!("{id}-key")).unwrap(),
                signer_kind,
                signer_id: ActorId::new(signer_id).unwrap(),
                signature_base64: String::new(),
            },
        }
    }

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

    fn artifact(quote: &QuoteV2, reservation: &ReservationV2) -> AcceptedArtifactV2 {
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
        artifact: &AcceptedArtifactV2,
    ) -> SettlementV2 {
        SettlementV2 {
            schema_version: SCHEMA_VERSION_V2,
            settlement_id: SettlementId::new("settlement-v2-1").unwrap(),
            settlement_nonce: SettlementNonceId::new("settlement-nonce-1").unwrap(),
            rail_id: "test:rail".into(),
            reservation_id: reservation.reservation_id.clone(),
            quote_id: quote.quote_id.clone(),
            quote_digest: quote.canonical_digest().unwrap(),
            pipeline_id: artifact.pipeline_id.clone(),
            stage_id: artifact.stage_id.clone(),
            attempt_id: artifact.attempt_id.clone(),
            artifact_id: artifact.artifact_id.clone(),
            artifact_manifest_digest: artifact.artifact_manifest_digest,
            provider_id: artifact.provider_id.clone(),
            verification_decision_digest: artifact.verification_decision_digest,
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

    fn finality(
        settlement: &SettlementV2,
        transaction_reference: &str,
        outcome: RailFinalityOutcomeV2,
    ) -> SignedEnvelopeV1<RailFinalityAttestationV2> {
        unsigned_envelope(
            "rail-attestation-1",
            SignerKind::System,
            "rail-attestor-1",
            RailFinalityAttestationV2 {
                schema_version: SCHEMA_VERSION_V2,
                rail_id: settlement.rail_id.clone(),
                settlement_id: settlement.settlement_id.clone(),
                settlement_nonce: settlement.settlement_nonce.clone(),
                settlement_digest: settlement.canonical_digest().unwrap(),
                transaction_reference: transaction_reference.into(),
                outcome,
                rail_proof_digest: Sha256Digest::of_bytes(b"rail-proof"),
                observed_at_unix_ms: 1_700,
            },
        )
    }

    fn advance_to_intent(machine: &mut SettlementMachineV2) {
        let quote = quote();
        let authorization = authorization(&quote);
        let reservation = reservation(&quote, &authorization);
        let artifact = artifact(&quote, &reservation);
        let settlement = settlement(&quote, &reservation, &artifact);
        machine
            .apply_trusted(&event(
                "event-1",
                SettlementEventV2::QuoteAuthorised {
                    quote,
                    authorization: unsigned_envelope(
                        "customer-authorization-1",
                        SignerKind::Customer,
                        "customer-1",
                        authorization,
                    ),
                    observed_at_unix_ms: 1_200,
                },
            ))
            .unwrap();
        machine
            .apply_trusted(&event(
                "event-2",
                SettlementEventV2::ReservationCreated {
                    reservation,
                    observed_at_unix_ms: 1_200,
                },
            ))
            .unwrap();
        machine
            .apply_trusted(&event(
                "event-3",
                SettlementEventV2::WorkAccepted {
                    accepted_artifact: artifact,
                    observed_at_unix_ms: 1_500,
                },
            ))
            .unwrap();
        machine
            .apply_trusted(&event(
                "event-4",
                SettlementEventV2::SettlementIntentCreated {
                    settlement,
                    observed_at_unix_ms: 1_600,
                },
            ))
            .unwrap();
    }

    #[test]
    fn only_chain_confirmation_can_make_a_settlement_financially_final() {
        let mut machine = SettlementMachineV2::new();
        advance_to_intent(&mut machine);
        assert_eq!(machine.phase(), SettlementPhaseV2::SettlementIntent);
        machine
            .apply_trusted(&event(
                "event-5",
                SettlementEventV2::ChainSubmitted {
                    transaction_reference: "solana:transaction-1".into(),
                },
            ))
            .unwrap();
        assert_eq!(machine.phase(), SettlementPhaseV2::ChainPending);
        let settlement = machine.settlement().unwrap().clone();
        machine
            .apply_trusted(&event(
                "event-6",
                SettlementEventV2::ChainConfirmationRecorded {
                    finality: finality(
                        &settlement,
                        "solana:transaction-1",
                        RailFinalityOutcomeV2::Final,
                    ),
                },
            ))
            .unwrap();
        assert_eq!(machine.phase(), SettlementPhaseV2::FinancialFinal);
        let before = machine.clone();
        assert!(
            machine
                .apply_trusted(&event(
                    "event-7",
                    SettlementEventV2::ReservationExpired {
                        observed_at_unix_ms: 2_000,
                    },
                ))
                .is_err()
        );
        assert_eq!(machine, before);
    }

    #[test]
    fn uncertain_submission_needs_attested_non_finality_before_retry() {
        let mut machine = SettlementMachineV2::new();
        advance_to_intent(&mut machine);
        let submitted = event(
            "event-5",
            SettlementEventV2::ChainSubmitted {
                transaction_reference: "solana:transaction-1".into(),
            },
        );
        assert_eq!(machine.apply_trusted(&submitted), Ok(ApplyOutcome::Applied));
        assert_eq!(
            machine.apply_trusted(&submitted),
            Ok(ApplyOutcome::DuplicateIgnored)
        );
        machine
            .apply_trusted(&event(
                "event-6",
                SettlementEventV2::ChainOutcomeUncertain {
                    transaction_reference: "solana:transaction-1".into(),
                    reason_code: "rpc-timeout".into(),
                },
            ))
            .unwrap();
        assert_eq!(machine.phase(), SettlementPhaseV2::ReconciliationRequired);
        let before = machine.clone();
        assert_eq!(
            machine.apply_trusted(&event(
                "event-7",
                SettlementEventV2::ChainSubmitted {
                    transaction_reference: "solana:transaction-2".into(),
                },
            )),
            Err(TransitionError::InvalidTransition {
                state: "reconciliation_required",
                event: "chain_submitted",
            })
        );
        assert_eq!(machine, before);
        let settlement = machine.settlement().unwrap().clone();
        machine
            .apply_trusted(&event(
                "event-8",
                SettlementEventV2::ChainRetryAuthorised {
                    reconciliation: finality(
                        &settlement,
                        "solana:transaction-1",
                        RailFinalityOutcomeV2::NotFinal,
                    ),
                },
            ))
            .unwrap();
        assert_eq!(machine.phase(), SettlementPhaseV2::SettlementIntent);
    }

    #[test]
    fn work_and_settlement_intent_cannot_cross_reservation_expiry() {
        let quote = quote();
        let authorization = authorization(&quote);
        let mut reservation = reservation(&quote, &authorization);
        reservation.expires_at_unix_ms = 1_500;
        let artifact = artifact(&quote, &reservation);
        let mut machine = SettlementMachineV2::new();
        machine
            .apply_trusted(&event(
                "event-1",
                SettlementEventV2::QuoteAuthorised {
                    quote: quote.clone(),
                    authorization: unsigned_envelope(
                        "customer-authorization-1",
                        SignerKind::Customer,
                        "customer-1",
                        authorization,
                    ),
                    observed_at_unix_ms: 1_200,
                },
            ))
            .unwrap();
        machine
            .apply_trusted(&event(
                "event-2",
                SettlementEventV2::ReservationCreated {
                    reservation,
                    observed_at_unix_ms: 1_200,
                },
            ))
            .unwrap();
        assert!(
            machine
                .apply_trusted(&event(
                    "event-3",
                    SettlementEventV2::WorkAccepted {
                        accepted_artifact: artifact,
                        observed_at_unix_ms: 1_500,
                    },
                ))
                .is_err()
        );
    }

    #[test]
    fn future_reservation_expiry_observation_is_rejected_before_reduction() {
        assert!(
            SettlementEventV2::ReservationExpired {
                observed_at_unix_ms: 1_701,
            }
            .validate_signed_contract(1_700)
            .is_err()
        );
    }

    #[test]
    fn lifecycle_observations_cannot_move_backwards() {
        let quote = quote();
        let authorization = authorization(&quote);
        let reservation = reservation(&quote, &authorization);
        let mut machine = SettlementMachineV2::new();
        machine
            .apply_trusted(&event(
                "event-1",
                SettlementEventV2::QuoteAuthorised {
                    quote: quote.clone(),
                    authorization: unsigned_envelope(
                        "customer-authorization-1",
                        SignerKind::Customer,
                        "customer-1",
                        authorization,
                    ),
                    observed_at_unix_ms: 1_200,
                },
            ))
            .unwrap();
        let before = machine.clone();
        assert!(
            machine
                .apply_trusted(&event(
                    "event-2",
                    SettlementEventV2::ReservationCreated {
                        reservation,
                        observed_at_unix_ms: 1_199,
                    },
                ))
                .is_err()
        );
        assert_eq!(machine, before);
    }

    #[test]
    fn fee_receipt_cannot_be_reused_for_a_different_submitted_transaction() {
        let mut machine = SettlementMachineV2::new();
        advance_to_intent(&mut machine);
        let before = machine.clone();
        assert!(
            machine
                .apply_trusted(&event(
                    "event-5",
                    SettlementEventV2::ChainSubmitted {
                        transaction_reference: "solana:transaction-2".into(),
                    },
                ))
                .is_err()
        );
        assert_eq!(machine, before);
    }

    #[test]
    fn same_event_id_with_different_content_is_rejected_without_mutation() {
        let mut machine = SettlementMachineV2::new();
        let quote = quote();
        let authorization = authorization(&quote);
        let first = event(
            "event-1",
            SettlementEventV2::QuoteAuthorised {
                quote: quote.clone(),
                authorization: unsigned_envelope(
                    "customer-authorization-1",
                    SignerKind::Customer,
                    "customer-1",
                    authorization.clone(),
                ),
                observed_at_unix_ms: 1_200,
            },
        );
        machine.apply_trusted(&first).unwrap();
        let before = machine.clone();
        assert_eq!(
            machine.apply_trusted(&event(
                "event-1",
                SettlementEventV2::QuoteAuthorised {
                    quote,
                    authorization: unsigned_envelope(
                        "customer-authorization-1",
                        SignerKind::Customer,
                        "customer-1",
                        authorization,
                    ),
                    observed_at_unix_ms: 1_201,
                },
            )),
            Err(TransitionError::EventIdConflict)
        );
        assert_eq!(machine, before);
    }
}
