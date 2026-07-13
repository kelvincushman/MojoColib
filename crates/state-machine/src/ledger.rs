//! Replayable off-chain test-credit ledger with causal ordering and idempotency.

use std::collections::BTreeMap;

use colibri_protocol::{
    SCHEMA_VERSION_V1, canonical,
    digest::Sha256Digest,
    ids::{
        ArtifactId, AttemptId, CustomerId, LedgerEventId, PipelineId, ProviderId, QuoteId,
        ReservationId, SettlementId, StageId,
    },
    ledger::{Microcredits, QuoteV1, ReservationV1, SettlementV1},
};
use serde::{Deserialize, Serialize};

use crate::TransitionError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactAcceptance {
    pub reservation_id: ReservationId,
    pub quote_id: QuoteId,
    pub pipeline_id: PipelineId,
    pub stage_id: StageId,
    pub attempt_id: AttemptId,
    pub artifact_id: ArtifactId,
    pub artifact_manifest_digest: Sha256Digest,
    pub provider_id: ProviderId,
    pub verification_decision_digest: Sha256Digest,
}

/// Append-only inputs accepted by the deterministic ledger reducer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum LedgerEvent {
    TestCreditsIssued {
        event_id: LedgerEventId,
        customer_id: CustomerId,
        amount: Microcredits,
    },
    QuoteAccepted {
        event_id: LedgerEventId,
        accepted_at_unix_ms: u64,
        quote: QuoteV1,
    },
    ReservationCreated {
        event_id: LedgerEventId,
        funded_at_unix_ms: u64,
        reservation: ReservationV1,
    },
    ArtifactAccepted {
        event_id: LedgerEventId,
        acceptance: ArtifactAcceptance,
    },
    SettlementAccepted {
        event_id: LedgerEventId,
        settlement: SettlementV1,
    },
    ReservationReleased {
        event_id: LedgerEventId,
        reservation_id: ReservationId,
    },
}

impl LedgerEvent {
    #[must_use]
    pub const fn event_id(&self) -> &LedgerEventId {
        match self {
            Self::TestCreditsIssued { event_id, .. }
            | Self::QuoteAccepted { event_id, .. }
            | Self::ReservationCreated { event_id, .. }
            | Self::ArtifactAccepted { event_id, .. }
            | Self::SettlementAccepted { event_id, .. }
            | Self::ReservationReleased { event_id, .. } => event_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ReservationRecord {
    reservation: ReservationV1,
    settled: Microcredits,
    released: bool,
}

impl ReservationRecord {
    fn remaining(&self) -> Result<Microcredits, TransitionError> {
        Ok(self.reservation.amount.checked_sub(self.settled)?)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyOutcome {
    Applied,
    DuplicateIgnored,
}

/// Materialized balances and commitments reconstructed only from ledger events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct LedgerMachine {
    customer_available: BTreeMap<CustomerId, Microcredits>,
    provider_credits: BTreeMap<ProviderId, Microcredits>,
    operator_fees: Microcredits,
    quotes: BTreeMap<QuoteId, QuoteV1>,
    reservations: BTreeMap<ReservationId, ReservationRecord>,
    reservations_by_quote: BTreeMap<QuoteId, ReservationId>,
    accepted_artifacts: BTreeMap<(PipelineId, StageId), ArtifactAcceptance>,
    settlement_ids: BTreeMap<SettlementId, Sha256Digest>,
    settled_stages: BTreeMap<(PipelineId, StageId), Sha256Digest>,
    applied_events: BTreeMap<LedgerEventId, Sha256Digest>,
}

impl LedgerMachine {
    #[must_use]
    pub fn customer_available(&self, customer_id: &CustomerId) -> Microcredits {
        self.customer_available
            .get(customer_id)
            .copied()
            .unwrap_or(Microcredits::ZERO)
    }

    #[must_use]
    pub fn provider_credit(&self, provider_id: &ProviderId) -> Microcredits {
        self.provider_credits
            .get(provider_id)
            .copied()
            .unwrap_or(Microcredits::ZERO)
    }

    #[must_use]
    pub const fn operator_fees(&self) -> Microcredits {
        self.operator_fees
    }

    pub fn apply(&mut self, event: &LedgerEvent) -> Result<ApplyOutcome, TransitionError> {
        let event_digest = canonical::digest(event)?;
        if let Some(previous_digest) = self.applied_events.get(event.event_id()) {
            return if *previous_digest == event_digest {
                Ok(ApplyOutcome::DuplicateIgnored)
            } else {
                Err(TransitionError::LedgerInvariant(
                    "ledger event id was reused with different content".into(),
                ))
            };
        }

        let mut next = self.clone();
        let outcome = next.apply_new_event(event)?;
        next.applied_events
            .insert(event.event_id().clone(), event_digest);
        *self = next;
        Ok(outcome)
    }

    #[allow(clippy::too_many_lines)]
    fn apply_new_event(&mut self, event: &LedgerEvent) -> Result<ApplyOutcome, TransitionError> {
        match event {
            LedgerEvent::TestCreditsIssued {
                customer_id,
                amount,
                ..
            } => {
                require_non_negative(*amount, "issued credits")?;
                let current = self.customer_available(customer_id);
                self.customer_available
                    .insert(customer_id.clone(), current.checked_add(*amount)?);
                Ok(ApplyOutcome::Applied)
            }
            LedgerEvent::QuoteAccepted {
                accepted_at_unix_ms,
                quote,
                ..
            } => {
                quote.validate(*accepted_at_unix_ms)?;
                if let Some(existing) = self.quotes.get(&quote.quote_id) {
                    return if existing == quote {
                        Ok(ApplyOutcome::DuplicateIgnored)
                    } else {
                        Err(TransitionError::LedgerInvariant(
                            "quote id already exists with different content".into(),
                        ))
                    };
                }
                self.quotes.insert(quote.quote_id.clone(), quote.clone());
                Ok(ApplyOutcome::Applied)
            }
            LedgerEvent::ReservationCreated {
                funded_at_unix_ms,
                reservation,
                ..
            } => {
                if reservation.schema_version != SCHEMA_VERSION_V1 {
                    return Err(TransitionError::LedgerInvariant(
                        "unsupported reservation schema version".into(),
                    ));
                }
                require_non_negative(reservation.amount, "reservation")?;
                if let Some(existing) = self.reservations.get(&reservation.reservation_id) {
                    return if existing.reservation == *reservation {
                        Ok(ApplyOutcome::DuplicateIgnored)
                    } else {
                        Err(TransitionError::LedgerInvariant(
                            "reservation id already exists with different content".into(),
                        ))
                    };
                }
                let quote = self.quotes.get(&reservation.quote_id).ok_or_else(|| {
                    TransitionError::LedgerInvariant("reservation has no accepted quote".into())
                })?;
                quote.validate(*funded_at_unix_ms)?;
                if self
                    .reservations_by_quote
                    .contains_key(&reservation.quote_id)
                {
                    return Err(TransitionError::LedgerInvariant(
                        "quote already has a reservation".into(),
                    ));
                }
                if reservation.pipeline_id != quote.pipeline_id
                    || reservation.stage_id != quote.stage_id
                    || reservation.amount != quote.maximum_reservation()?
                {
                    return Err(TransitionError::LedgerInvariant(
                        "reservation does not match its quote commitment".into(),
                    ));
                }
                let available = self.customer_available(&reservation.customer_id);
                if available < reservation.amount {
                    return Err(TransitionError::LedgerInvariant(
                        "customer has insufficient test credits".into(),
                    ));
                }
                self.customer_available.insert(
                    reservation.customer_id.clone(),
                    available.checked_sub(reservation.amount)?,
                );
                self.reservations.insert(
                    reservation.reservation_id.clone(),
                    ReservationRecord {
                        reservation: reservation.clone(),
                        settled: Microcredits::ZERO,
                        released: false,
                    },
                );
                self.reservations_by_quote.insert(
                    reservation.quote_id.clone(),
                    reservation.reservation_id.clone(),
                );
                Ok(ApplyOutcome::Applied)
            }
            LedgerEvent::ArtifactAccepted { acceptance, .. } => {
                let key = (acceptance.pipeline_id.clone(), acceptance.stage_id.clone());
                let reservation = self
                    .reservations
                    .get(&acceptance.reservation_id)
                    .ok_or_else(|| {
                        TransitionError::LedgerInvariant(
                            "artifact acceptance has no reservation".into(),
                        )
                    })?;
                if reservation.released
                    || reservation.reservation.quote_id != acceptance.quote_id
                    || reservation.reservation.pipeline_id != acceptance.pipeline_id
                    || reservation.reservation.stage_id != acceptance.stage_id
                {
                    return Err(TransitionError::LedgerInvariant(
                        "artifact acceptance does not match an open reservation".into(),
                    ));
                }
                if let Some(existing) = self.accepted_artifacts.get(&key) {
                    return if existing == acceptance {
                        Ok(ApplyOutcome::DuplicateIgnored)
                    } else {
                        Err(TransitionError::LedgerInvariant(
                            "stage already has a different accepted artifact".into(),
                        ))
                    };
                }
                self.accepted_artifacts.insert(key, acceptance.clone());
                Ok(ApplyOutcome::Applied)
            }
            LedgerEvent::SettlementAccepted { settlement, .. } => self.apply_settlement(settlement),
            LedgerEvent::ReservationReleased { reservation_id, .. } => {
                self.apply_release(reservation_id)
            }
        }
    }

    fn apply_settlement(
        &mut self,
        settlement: &SettlementV1,
    ) -> Result<ApplyOutcome, TransitionError> {
        if settlement.schema_version != SCHEMA_VERSION_V1 {
            return Err(TransitionError::LedgerInvariant(
                "unsupported settlement schema version".into(),
            ));
        }
        let settlement_digest = canonical::digest(settlement)?;
        let stage_key = (settlement.pipeline_id.clone(), settlement.stage_id.clone());
        if let Some(existing_digest) = self.settled_stages.get(&stage_key) {
            return if *existing_digest == settlement_digest {
                Ok(ApplyOutcome::DuplicateIgnored)
            } else {
                Err(TransitionError::LedgerInvariant(
                    "accepted stage is already settled differently".into(),
                ))
            };
        }
        if let Some(existing_digest) = self.settlement_ids.get(&settlement.settlement_id) {
            return if *existing_digest == settlement_digest {
                Ok(ApplyOutcome::DuplicateIgnored)
            } else {
                Err(TransitionError::LedgerInvariant(
                    "settlement id was reused with different content".into(),
                ))
            };
        }
        let acceptance = self.accepted_artifacts.get(&stage_key).ok_or_else(|| {
            TransitionError::LedgerInvariant("settlement has no accepted artifact".into())
        })?;
        if acceptance.reservation_id != settlement.reservation_id
            || acceptance.quote_id != settlement.quote_id
            || acceptance.attempt_id != settlement.attempt_id
            || acceptance.artifact_id != settlement.artifact_id
            || acceptance.artifact_manifest_digest != settlement.artifact_manifest_digest
            || acceptance.provider_id != settlement.provider_id
            || acceptance.verification_decision_digest != settlement.verification_decision_digest
        {
            return Err(TransitionError::LedgerInvariant(
                "settlement does not match accepted artifact commitments".into(),
            ));
        }
        let reservation = self
            .reservations
            .get(&settlement.reservation_id)
            .ok_or_else(|| {
                TransitionError::LedgerInvariant("settlement has no reservation".into())
            })?;
        if reservation.released
            || reservation.reservation.pipeline_id != settlement.pipeline_id
            || reservation.reservation.stage_id != settlement.stage_id
            || reservation.reservation.quote_id != settlement.quote_id
        {
            return Err(TransitionError::LedgerInvariant(
                "settlement does not match an open reservation".into(),
            ));
        }
        let quote = self
            .quotes
            .get(&settlement.quote_id)
            .ok_or_else(|| TransitionError::LedgerInvariant("settlement has no quote".into()))?;
        if quote.provider_id != settlement.provider_id {
            return Err(TransitionError::LedgerInvariant(
                "settlement provider does not match quote".into(),
            ));
        }
        if settlement.provider_credit > quote.execution
            || settlement.operator_fee > quote.verification_reserve
        {
            return Err(TransitionError::LedgerInvariant(
                "settlement components exceed their quote caps".into(),
            ));
        }
        let total = settlement.total_debit()?;
        if total > reservation.remaining()? || total > quote.maximum_reservation()? {
            return Err(TransitionError::LedgerInvariant(
                "settlement exceeds reserved credits".into(),
            ));
        }

        let provider_before = self.provider_credit(&settlement.provider_id);
        let provider_after = provider_before.checked_add(settlement.provider_credit)?;
        let fees_after = self.operator_fees.checked_add(settlement.operator_fee)?;
        self.reservations
            .get_mut(&settlement.reservation_id)
            .expect("reservation was checked")
            .settled = total;
        self.provider_credits
            .insert(settlement.provider_id.clone(), provider_after);
        self.operator_fees = fees_after;
        self.settlement_ids
            .insert(settlement.settlement_id.clone(), settlement_digest);
        self.settled_stages.insert(stage_key, settlement_digest);
        Ok(ApplyOutcome::Applied)
    }

    fn apply_release(
        &mut self,
        reservation_id: &ReservationId,
    ) -> Result<ApplyOutcome, TransitionError> {
        let reservation = self
            .reservations
            .get(reservation_id)
            .ok_or_else(|| TransitionError::LedgerInvariant("release has no reservation".into()))?;
        if reservation.released {
            return Ok(ApplyOutcome::DuplicateIgnored);
        }
        let remaining = reservation.remaining()?;
        let customer_id = reservation.reservation.customer_id.clone();
        let available_after = self
            .customer_available(&customer_id)
            .checked_add(remaining)?;
        self.reservations
            .get_mut(reservation_id)
            .expect("reservation was checked")
            .released = true;
        self.customer_available.insert(customer_id, available_after);
        Ok(ApplyOutcome::Applied)
    }
}

fn require_non_negative(amount: Microcredits, label: &'static str) -> Result<(), TransitionError> {
    if amount < Microcredits::ZERO {
        Err(TransitionError::LedgerInvariant(format!(
            "{label} must be non-negative"
        )))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ApplyOutcome, ArtifactAcceptance, LedgerEvent, LedgerMachine};
    use colibri_protocol::{
        SCHEMA_VERSION_V1,
        digest::Sha256Digest,
        ids::{
            ArtifactId, AttemptId, CustomerId, LedgerEventId, PipelineId, ProviderId, QuoteId,
            ReservationId, SettlementId, StageId,
        },
        ledger::{Microcredits, QuoteV1, ReservationV1, SettlementV1},
    };
    use proptest::prelude::*;

    struct Fixture {
        customer: CustomerId,
        provider: ProviderId,
        pipeline: PipelineId,
        stage: StageId,
        quote: QuoteV1,
        reservation: ReservationV1,
        acceptance: ArtifactAcceptance,
        settlement: SettlementV1,
    }

    fn fixture() -> Fixture {
        let customer = CustomerId::new("customer-1").unwrap();
        let provider = ProviderId::new("provider-1").unwrap();
        let pipeline = PipelineId::new("pipeline-1").unwrap();
        let stage = StageId::new("stage-1").unwrap();
        let quote = QuoteV1 {
            schema_version: SCHEMA_VERSION_V1,
            quote_id: QuoteId::new("quote-1").unwrap(),
            pipeline_id: pipeline.clone(),
            stage_id: stage.clone(),
            provider_id: provider.clone(),
            execution: Microcredits::new(700),
            verification_reserve: Microcredits::new(300),
            policy_version: 1,
            expires_at_unix_ms: 10_000,
        };
        let reservation = ReservationV1 {
            schema_version: SCHEMA_VERSION_V1,
            reservation_id: ReservationId::new("reservation-1").unwrap(),
            customer_id: customer.clone(),
            quote_id: quote.quote_id.clone(),
            pipeline_id: pipeline.clone(),
            stage_id: stage.clone(),
            amount: Microcredits::new(1_000),
        };
        let acceptance = ArtifactAcceptance {
            reservation_id: reservation.reservation_id.clone(),
            quote_id: quote.quote_id.clone(),
            pipeline_id: pipeline.clone(),
            stage_id: stage.clone(),
            attempt_id: AttemptId::new("attempt-1").unwrap(),
            artifact_id: ArtifactId::new("artifact-1").unwrap(),
            artifact_manifest_digest: Sha256Digest::of_bytes(b"manifest"),
            provider_id: provider.clone(),
            verification_decision_digest: Sha256Digest::of_bytes(b"verification"),
        };
        let settlement = SettlementV1 {
            schema_version: SCHEMA_VERSION_V1,
            settlement_id: SettlementId::new("settlement-1").unwrap(),
            reservation_id: reservation.reservation_id.clone(),
            quote_id: quote.quote_id.clone(),
            pipeline_id: pipeline.clone(),
            stage_id: stage.clone(),
            attempt_id: acceptance.attempt_id.clone(),
            artifact_id: acceptance.artifact_id.clone(),
            artifact_manifest_digest: acceptance.artifact_manifest_digest,
            provider_id: provider.clone(),
            verification_decision_digest: acceptance.verification_decision_digest,
            provider_credit: Microcredits::new(650),
            operator_fee: Microcredits::new(50),
        };
        Fixture {
            customer,
            provider,
            pipeline,
            stage,
            quote,
            reservation,
            acceptance,
            settlement,
        }
    }

    fn id(value: &str) -> LedgerEventId {
        LedgerEventId::new(value).unwrap()
    }

    #[test]
    fn duplicate_events_do_not_double_settle() {
        let fixture = fixture();
        let events = [
            LedgerEvent::TestCreditsIssued {
                event_id: id("event-1"),
                customer_id: fixture.customer.clone(),
                amount: Microcredits::new(2_000),
            },
            LedgerEvent::QuoteAccepted {
                event_id: id("event-2"),
                accepted_at_unix_ms: 1_000,
                quote: fixture.quote.clone(),
            },
            LedgerEvent::ReservationCreated {
                event_id: id("event-3"),
                funded_at_unix_ms: 2_000,
                reservation: fixture.reservation.clone(),
            },
            LedgerEvent::ArtifactAccepted {
                event_id: id("event-4"),
                acceptance: fixture.acceptance.clone(),
            },
            LedgerEvent::SettlementAccepted {
                event_id: id("event-5"),
                settlement: fixture.settlement.clone(),
            },
            LedgerEvent::ReservationReleased {
                event_id: id("event-6"),
                reservation_id: fixture.reservation.reservation_id.clone(),
            },
        ];
        let mut ledger = LedgerMachine::default();
        for event in &events {
            assert_eq!(ledger.apply(event).unwrap(), ApplyOutcome::Applied);
            assert_eq!(ledger.apply(event).unwrap(), ApplyOutcome::DuplicateIgnored);
        }
        assert_eq!(
            ledger.customer_available(&fixture.customer),
            Microcredits::new(1_300)
        );
        assert_eq!(
            ledger.provider_credit(&fixture.provider),
            Microcredits::new(650)
        );
        assert_eq!(ledger.operator_fees(), Microcredits::new(50));
    }

    #[test]
    fn causal_reordering_is_rejected_without_mutation() {
        let fixture = fixture();
        let mut ledger = LedgerMachine::default();
        let customer_before = ledger.customer_available(&fixture.customer);
        let provider_before = ledger.provider_credit(&fixture.provider);
        let fees_before = ledger.operator_fees();
        let settlement = LedgerEvent::SettlementAccepted {
            event_id: id("event-too-early"),
            settlement: fixture.settlement,
        };
        assert!(ledger.apply(&settlement).is_err());
        assert_eq!(
            ledger.customer_available(&fixture.customer),
            customer_before
        );
        assert_eq!(ledger.provider_credit(&fixture.provider), provider_before);
        assert_eq!(ledger.operator_fees(), fees_before);
        assert_eq!(fixture.pipeline.as_str(), "pipeline-1");
        assert_eq!(fixture.stage.as_str(), "stage-1");
    }

    fn apply_funding_prefix(ledger: &mut LedgerMachine, fixture: &Fixture) {
        ledger
            .apply(&LedgerEvent::TestCreditsIssued {
                event_id: id("prefix-credits"),
                customer_id: fixture.customer.clone(),
                amount: Microcredits::new(2_000),
            })
            .unwrap();
        ledger
            .apply(&LedgerEvent::QuoteAccepted {
                event_id: id("prefix-quote"),
                accepted_at_unix_ms: 1_000,
                quote: fixture.quote.clone(),
            })
            .unwrap();
        ledger
            .apply(&LedgerEvent::ReservationCreated {
                event_id: id("prefix-reservation"),
                funded_at_unix_ms: 2_000,
                reservation: fixture.reservation.clone(),
            })
            .unwrap();
    }

    #[test]
    fn one_quote_cannot_double_reserve_under_a_fresh_id() {
        let fixture = fixture();
        let mut ledger = LedgerMachine::default();
        apply_funding_prefix(&mut ledger, &fixture);
        let customer_before = ledger.customer_available(&fixture.customer);
        let provider_before = ledger.provider_credit(&fixture.provider);
        let fees_before = ledger.operator_fees();
        let mut conflicting = fixture.reservation.clone();
        conflicting.reservation_id = ReservationId::new("reservation-2").unwrap();
        let event = LedgerEvent::ReservationCreated {
            event_id: id("fresh-event"),
            funded_at_unix_ms: 2_500,
            reservation: conflicting,
        };
        assert!(ledger.apply(&event).is_err());
        assert_eq!(
            ledger.customer_available(&fixture.customer),
            customer_before
        );
        assert_eq!(ledger.provider_credit(&fixture.provider), provider_before);
        assert_eq!(ledger.operator_fees(), fees_before);
    }

    #[test]
    fn semantic_settlement_retry_is_a_no_op() {
        let fixture = fixture();
        let mut ledger = LedgerMachine::default();
        apply_funding_prefix(&mut ledger, &fixture);
        ledger
            .apply(&LedgerEvent::ArtifactAccepted {
                event_id: id("acceptance"),
                acceptance: fixture.acceptance.clone(),
            })
            .unwrap();
        ledger
            .apply(&LedgerEvent::SettlementAccepted {
                event_id: id("settlement-first"),
                settlement: fixture.settlement.clone(),
            })
            .unwrap();
        let customer_before = ledger.customer_available(&fixture.customer);
        let provider_before = ledger.provider_credit(&fixture.provider);
        let fees_before = ledger.operator_fees();
        assert_eq!(
            ledger
                .apply(&LedgerEvent::SettlementAccepted {
                    event_id: id("settlement-retry"),
                    settlement: fixture.settlement,
                })
                .unwrap(),
            ApplyOutcome::DuplicateIgnored
        );
        assert_eq!(
            ledger.customer_available(&fixture.customer),
            customer_before
        );
        assert_eq!(ledger.provider_credit(&fixture.provider), provider_before);
        assert_eq!(ledger.operator_fees(), fees_before);
    }

    #[test]
    fn expired_quote_cannot_be_funded() {
        let fixture = fixture();
        let mut ledger = LedgerMachine::default();
        ledger
            .apply(&LedgerEvent::TestCreditsIssued {
                event_id: id("credits"),
                customer_id: fixture.customer,
                amount: Microcredits::new(2_000),
            })
            .unwrap();
        ledger
            .apply(&LedgerEvent::QuoteAccepted {
                event_id: id("quote"),
                accepted_at_unix_ms: 1_000,
                quote: fixture.quote,
            })
            .unwrap();
        let before = ledger.clone();
        assert!(
            ledger
                .apply(&LedgerEvent::ReservationCreated {
                    event_id: id("late-reservation"),
                    funded_at_unix_ms: 10_000,
                    reservation: fixture.reservation,
                })
                .is_err()
        );
        assert_eq!(ledger, before);
    }

    proptest! {
        #[test]
        fn arbitrary_duplicate_replay_preserves_balances_and_state(
            issued_extra in 0_i64..=10_000,
            provider_credit in 0_i64..=700,
            operator_fee in 0_i64..=300,
            replay_indices in prop::collection::vec(0_usize..6, 0..64),
        ) {
            let mut fixture = fixture();
            fixture.settlement.provider_credit = Microcredits::new(provider_credit);
            fixture.settlement.operator_fee = Microcredits::new(operator_fee);
            let issued = 1_000_i64 + issued_extra;
            let events = [
                LedgerEvent::TestCreditsIssued {
                    event_id: id("property-event-1"),
                    customer_id: fixture.customer.clone(),
                    amount: Microcredits::new(issued),
                },
                LedgerEvent::QuoteAccepted {
                    event_id: id("property-event-2"),
                    accepted_at_unix_ms: 1_000,
                    quote: fixture.quote.clone(),
                },
                LedgerEvent::ReservationCreated {
                    event_id: id("property-event-3"),
                    funded_at_unix_ms: 2_000,
                    reservation: fixture.reservation.clone(),
                },
                LedgerEvent::ArtifactAccepted {
                    event_id: id("property-event-4"),
                    acceptance: fixture.acceptance.clone(),
                },
                LedgerEvent::SettlementAccepted {
                    event_id: id("property-event-5"),
                    settlement: fixture.settlement.clone(),
                },
                LedgerEvent::ReservationReleased {
                    event_id: id("property-event-6"),
                    reservation_id: fixture.reservation.reservation_id.clone(),
                },
            ];

            let mut ledger = LedgerMachine::default();
            for event in &events {
                prop_assert_eq!(ledger.apply(event).unwrap(), ApplyOutcome::Applied);
            }
            let settled = ledger.clone();

            for index in replay_indices {
                prop_assert_eq!(
                    ledger.apply(&events[index]).unwrap(),
                    ApplyOutcome::DuplicateIgnored
                );
                prop_assert_eq!(&ledger, &settled);
            }

            prop_assert_eq!(
                ledger.customer_available(&fixture.customer),
                Microcredits::new(issued - provider_credit - operator_fee)
            );
            prop_assert_eq!(
                ledger.provider_credit(&fixture.provider),
                Microcredits::new(provider_credit)
            );
            prop_assert_eq!(ledger.operator_fees(), Microcredits::new(operator_fee));
        }

        #[test]
        fn reused_event_id_with_different_content_is_atomic(
            original in 0_i64..=10_000,
            delta in 1_i64..=10_000,
        ) {
            let fixture = fixture();
            let event_id = id("property-corruption-event");
            let mut ledger = LedgerMachine::default();
            ledger
                .apply(&LedgerEvent::TestCreditsIssued {
                    event_id: event_id.clone(),
                    customer_id: fixture.customer.clone(),
                    amount: Microcredits::new(original),
                })
                .unwrap();
            let before_conflict = ledger.clone();

            let conflicting_replay = ledger.apply(&LedgerEvent::TestCreditsIssued {
                event_id,
                customer_id: fixture.customer,
                amount: Microcredits::new(original + delta),
            });
            prop_assert!(conflicting_replay.is_err());
            prop_assert_eq!(&ledger, &before_conflict);
        }
    }
}
