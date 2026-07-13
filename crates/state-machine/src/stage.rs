//! Stage attempt, renewable lease, fencing and verification transitions.

use std::collections::{BTreeMap, BTreeSet};

use colibri_protocol::{
    canonical,
    digest::Sha256Digest,
    ids::{AttemptId, MessageId, StageId},
    lease::{FencingTokenV1, LeaseRenewalV1, StageLeaseV1},
};
use serde::{Deserialize, Serialize};

use crate::{ApplyOutcome, EventEnvelope, TransitionError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StagePhase {
    Pending,
    Runnable,
    Leased,
    Running,
    Verifying,
    Retryable,
    Draining,
    Accepted,
    Failed,
    Cancelled,
}

impl StagePhase {
    const fn name(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Runnable => "runnable",
            Self::Leased => "leased",
            Self::Running => "running",
            Self::Verifying => "verifying",
            Self::Retryable => "retryable",
            Self::Draining => "draining",
            Self::Accepted => "accepted",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AttemptBinding {
    lease: StageLeaseV1,
    candidate_manifest_digest: Option<Sha256Digest>,
}

impl AttemptBinding {
    #[must_use]
    pub const fn lease(&self) -> &StageLeaseV1 {
        &self.lease
    }

    #[must_use]
    pub const fn candidate_manifest_digest(&self) -> Option<Sha256Digest> {
        self.candidate_manifest_digest
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum StageEvent {
    DependenciesAccepted,
    LeaseIssued {
        lease: StageLeaseV1,
    },
    LeaseAccepted {
        attempt_id: AttemptId,
        fencing_token: FencingTokenV1,
        observed_at_unix_ms: u64,
    },
    LeaseRenewed {
        renewal: LeaseRenewalV1,
    },
    ExecutionSucceeded {
        attempt_id: AttemptId,
        fencing_token: FencingTokenV1,
        observed_at_unix_ms: u64,
        manifest_digest: Sha256Digest,
    },
    ExecutionFailed {
        attempt_id: AttemptId,
        fencing_token: FencingTokenV1,
        observed_at_unix_ms: u64,
    },
    LeaseExpired {
        attempt_id: AttemptId,
        fencing_token: FencingTokenV1,
        observed_at_unix_ms: u64,
    },
    VerificationAccepted {
        attempt_id: AttemptId,
        manifest_digest: Sha256Digest,
    },
    VerificationRejected {
        attempt_id: AttemptId,
    },
    RequestCancellation,
    DrainCompleted,
}

impl StageEvent {
    const fn name(&self) -> &'static str {
        match self {
            Self::DependenciesAccepted => "dependencies_accepted",
            Self::LeaseIssued { .. } => "lease_issued",
            Self::LeaseAccepted { .. } => "lease_accepted",
            Self::LeaseRenewed { .. } => "lease_renewed",
            Self::ExecutionSucceeded { .. } => "execution_succeeded",
            Self::ExecutionFailed { .. } => "execution_failed",
            Self::LeaseExpired { .. } => "lease_expired",
            Self::VerificationAccepted { .. } => "verification_accepted",
            Self::VerificationRejected { .. } => "verification_rejected",
            Self::RequestCancellation => "request_cancellation",
            Self::DrainCompleted => "drain_completed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StageMachine {
    stage_id: StageId,
    phase: StagePhase,
    attempts_started: u16,
    max_attempts: u16,
    last_fencing_token: u64,
    attempt_history: BTreeSet<AttemptId>,
    active_attempt: Option<AttemptBinding>,
    accepted_manifest_digest: Option<Sha256Digest>,
    sequence: u64,
    applied_events: BTreeMap<MessageId, Sha256Digest>,
}

impl StageMachine {
    pub fn new(stage_id: StageId, max_attempts: u16) -> Result<Self, TransitionError> {
        if max_attempts == 0 {
            return Err(TransitionError::RetryBudgetExhausted);
        }
        Ok(Self {
            stage_id,
            phase: StagePhase::Pending,
            attempts_started: 0,
            max_attempts,
            last_fencing_token: 0,
            attempt_history: BTreeSet::new(),
            active_attempt: None,
            accepted_manifest_digest: None,
            sequence: 0,
            applied_events: BTreeMap::new(),
        })
    }

    #[must_use]
    pub const fn stage_id(&self) -> &StageId {
        &self.stage_id
    }

    #[must_use]
    pub const fn phase(&self) -> StagePhase {
        self.phase
    }

    #[must_use]
    pub const fn attempts_started(&self) -> u16 {
        self.attempts_started
    }

    #[must_use]
    pub const fn max_attempts(&self) -> u16 {
        self.max_attempts
    }

    #[must_use]
    pub const fn active_attempt(&self) -> Option<&AttemptBinding> {
        self.active_attempt.as_ref()
    }

    #[must_use]
    pub const fn accepted_manifest_digest(&self) -> Option<Sha256Digest> {
        self.accepted_manifest_digest
    }

    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Applies an event exactly once, safely ignoring an identical retry.
    pub fn apply_event(
        &mut self,
        envelope: &EventEnvelope<StageEvent>,
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
        next.apply_payload(envelope.payload.clone())?;
        next.applied_events
            .insert(envelope.event_id.clone(), event_digest);
        *self = next;
        Ok(ApplyOutcome::Applied)
    }

    #[cfg(test)]
    fn apply(&mut self, event: StageEvent) -> Result<(), TransitionError> {
        self.apply_payload(event)
    }

    fn apply_payload(&mut self, event: StageEvent) -> Result<(), TransitionError> {
        let mut next = self.clone();
        next.apply_inner(event)?;
        next.sequence = next
            .sequence
            .checked_add(1)
            .ok_or(TransitionError::ArithmeticOverflow)?;
        *self = next;
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn apply_inner(&mut self, event: StageEvent) -> Result<(), TransitionError> {
        match (&self.phase, event) {
            (StagePhase::Pending, StageEvent::DependenciesAccepted) => {
                self.phase = StagePhase::Runnable;
            }
            (StagePhase::Runnable | StagePhase::Retryable, StageEvent::LeaseIssued { lease }) => {
                lease.validate()?;
                if lease.stage_id != self.stage_id {
                    return Err(TransitionError::AttemptMismatch);
                }
                if self.attempts_started >= self.max_attempts {
                    return Err(TransitionError::RetryBudgetExhausted);
                }
                if lease.fencing_token.value() <= self.last_fencing_token {
                    return Err(TransitionError::StaleFencingToken);
                }
                if !self.attempt_history.insert(lease.attempt_id.clone()) {
                    return Err(TransitionError::AttemptMismatch);
                }
                self.attempts_started += 1;
                self.last_fencing_token = lease.fencing_token.value();
                self.active_attempt = Some(AttemptBinding {
                    lease,
                    candidate_manifest_digest: None,
                });
                self.phase = StagePhase::Leased;
            }
            (
                StagePhase::Leased,
                StageEvent::LeaseAccepted {
                    attempt_id,
                    fencing_token,
                    observed_at_unix_ms,
                },
            ) => {
                self.ensure_active(&attempt_id, fencing_token, Some(observed_at_unix_ms))?;
                self.phase = StagePhase::Running;
            }
            (StagePhase::Leased | StagePhase::Running, StageEvent::LeaseRenewed { renewal }) => {
                let active = self
                    .active_attempt
                    .as_mut()
                    .ok_or(TransitionError::AttemptMismatch)?;
                active.lease = renewal.apply_to(&active.lease)?;
            }
            (
                StagePhase::Running,
                StageEvent::ExecutionSucceeded {
                    attempt_id,
                    fencing_token,
                    observed_at_unix_ms,
                    manifest_digest,
                },
            ) => {
                self.ensure_active(&attempt_id, fencing_token, Some(observed_at_unix_ms))?;
                self.active_attempt
                    .as_mut()
                    .expect("active attempt checked")
                    .candidate_manifest_digest = Some(manifest_digest);
                self.phase = StagePhase::Verifying;
            }
            (
                StagePhase::Leased | StagePhase::Running,
                StageEvent::ExecutionFailed {
                    attempt_id,
                    fencing_token,
                    observed_at_unix_ms,
                },
            ) => {
                self.ensure_active(&attempt_id, fencing_token, Some(observed_at_unix_ms))?;
                self.retry_or_fail();
            }
            (
                StagePhase::Leased | StagePhase::Running,
                StageEvent::LeaseExpired {
                    attempt_id,
                    fencing_token,
                    observed_at_unix_ms,
                },
            ) => {
                self.ensure_active(&attempt_id, fencing_token, None)?;
                let active = self
                    .active_attempt
                    .as_ref()
                    .ok_or(TransitionError::AttemptMismatch)?;
                if observed_at_unix_ms < active.lease.expires_at_unix_ms {
                    return Err(TransitionError::Protocol(
                        "lease expiry event arrived before expiry".into(),
                    ));
                }
                self.retry_or_fail();
            }
            (
                StagePhase::Verifying,
                StageEvent::VerificationAccepted {
                    attempt_id,
                    manifest_digest,
                },
            ) => {
                let active = self
                    .active_attempt
                    .as_ref()
                    .ok_or(TransitionError::AttemptMismatch)?;
                if active.lease.attempt_id != attempt_id
                    || active.candidate_manifest_digest != Some(manifest_digest)
                {
                    return Err(TransitionError::AttemptMismatch);
                }
                self.accepted_manifest_digest = Some(manifest_digest);
                self.active_attempt = None;
                self.phase = StagePhase::Accepted;
            }
            (StagePhase::Verifying, StageEvent::VerificationRejected { attempt_id }) => {
                let active = self
                    .active_attempt
                    .as_ref()
                    .ok_or(TransitionError::AttemptMismatch)?;
                if active.lease.attempt_id != attempt_id {
                    return Err(TransitionError::AttemptMismatch);
                }
                self.retry_or_fail();
            }
            (
                StagePhase::Pending | StagePhase::Runnable | StagePhase::Retryable,
                StageEvent::RequestCancellation,
            )
            | (StagePhase::Draining, StageEvent::DrainCompleted) => {
                self.active_attempt = None;
                self.phase = StagePhase::Cancelled;
            }
            (
                StagePhase::Leased | StagePhase::Running | StagePhase::Verifying,
                StageEvent::RequestCancellation,
            ) => {
                self.phase = StagePhase::Draining;
            }
            (phase, event) => {
                return Err(TransitionError::InvalidTransition {
                    state: phase.name(),
                    event: event.name(),
                });
            }
        }
        Ok(())
    }

    fn ensure_active(
        &self,
        attempt_id: &AttemptId,
        fencing_token: FencingTokenV1,
        observed_at_unix_ms: Option<u64>,
    ) -> Result<(), TransitionError> {
        let active = self
            .active_attempt
            .as_ref()
            .ok_or(TransitionError::AttemptMismatch)?;
        if &active.lease.attempt_id != attempt_id {
            return Err(TransitionError::AttemptMismatch);
        }
        if active.lease.fencing_token != fencing_token {
            return Err(TransitionError::StaleFencingToken);
        }
        if let Some(observed_at_unix_ms) = observed_at_unix_ms {
            active
                .lease
                .accepts_fencing_token(fencing_token, observed_at_unix_ms)?;
        }
        Ok(())
    }

    fn retry_or_fail(&mut self) {
        self.active_attempt = None;
        self.phase = if self.attempts_started < self.max_attempts {
            StagePhase::Retryable
        } else {
            StagePhase::Failed
        };
    }
}

#[cfg(test)]
mod tests {
    use super::{StageEvent, StageMachine, StagePhase};
    use crate::{ApplyOutcome, EventEnvelope, TransitionError};
    use colibri_protocol::{
        SCHEMA_VERSION_V1,
        digest::Sha256Digest,
        ids::{AttemptId, MessageId, PipelineId, StageId, WorkerId},
        lease::{FencingTokenV1, LeaseRenewalV1, StageLeaseV1},
    };
    use proptest::prelude::*;

    fn lease(attempt: &str, token: u64) -> StageLeaseV1 {
        StageLeaseV1 {
            schema_version: SCHEMA_VERSION_V1,
            pipeline_id: PipelineId::new("pipeline-1").unwrap(),
            pipeline_spec_digest: Sha256Digest::of_bytes(b"pipeline"),
            stage_id: StageId::new("train").unwrap(),
            attempt_id: AttemptId::new(attempt).unwrap(),
            worker_id: WorkerId::new("worker-1").unwrap(),
            template_digest: Sha256Digest::of_bytes(b"template"),
            input_manifest_digests: vec![Sha256Digest::of_bytes(b"input")],
            issued_at_unix_ms: 1_000,
            expires_at_unix_ms: 61_000,
            fencing_token: FencingTokenV1::new(token).unwrap(),
        }
    }

    fn accepted_stage(max_attempts: u16, lease: StageLeaseV1) -> StageMachine {
        let mut stage = StageMachine::new(StageId::new("train").unwrap(), max_attempts).unwrap();
        stage.apply(StageEvent::DependenciesAccepted).unwrap();
        stage.apply(StageEvent::LeaseIssued { lease }).unwrap();
        stage
    }

    #[test]
    fn stale_fencing_token_cannot_promote_output() {
        let active = lease("attempt-1", 10);
        let attempt = active.attempt_id.clone();
        let mut stage = accepted_stage(2, active);
        stage
            .apply(StageEvent::LeaseAccepted {
                attempt_id: attempt.clone(),
                fencing_token: FencingTokenV1::new(10).unwrap(),
                observed_at_unix_ms: 2_000,
            })
            .unwrap();
        let before = stage.clone();
        assert!(
            stage
                .apply(StageEvent::ExecutionSucceeded {
                    attempt_id: attempt,
                    fencing_token: FencingTokenV1::new(9).unwrap(),
                    observed_at_unix_ms: 3_000,
                    manifest_digest: Sha256Digest::of_bytes(b"artifact"),
                })
                .is_err()
        );
        assert_eq!(stage, before);
        assert_eq!(stage.phase(), StagePhase::Running);
    }

    #[test]
    fn retry_budget_ends_in_failed_state() {
        let active = lease("attempt-1", 1);
        let attempt = active.attempt_id.clone();
        let mut stage = accepted_stage(1, active);
        stage
            .apply(StageEvent::LeaseExpired {
                attempt_id: attempt,
                fencing_token: FencingTokenV1::new(1).unwrap(),
                observed_at_unix_ms: 61_000,
            })
            .unwrap();
        assert_eq!(stage.phase(), StagePhase::Failed);
    }

    #[test]
    fn retry_requires_new_attempt_and_higher_fence() {
        let first_lease = lease("attempt-1", 4);
        let first = first_lease.attempt_id.clone();
        let mut stage = accepted_stage(3, first_lease);
        stage
            .apply(StageEvent::LeaseExpired {
                attempt_id: first,
                fencing_token: FencingTokenV1::new(4).unwrap(),
                observed_at_unix_ms: 61_000,
            })
            .unwrap();

        let before = stage.clone();
        assert!(
            stage
                .apply(StageEvent::LeaseIssued {
                    lease: lease("attempt-1", 5),
                })
                .is_err()
        );
        assert_eq!(stage, before);
        assert!(
            stage
                .apply(StageEvent::LeaseIssued {
                    lease: lease("attempt-2", 4),
                })
                .is_err()
        );
    }

    #[test]
    fn renewal_extends_same_binding_without_changing_fence() {
        let active = lease("attempt-1", 8);
        let attempt = active.attempt_id.clone();
        let mut stage = accepted_stage(2, active);
        stage
            .apply(StageEvent::LeaseAccepted {
                attempt_id: attempt.clone(),
                fencing_token: FencingTokenV1::new(8).unwrap(),
                observed_at_unix_ms: 2_000,
            })
            .unwrap();
        stage
            .apply(StageEvent::LeaseRenewed {
                renewal: LeaseRenewalV1 {
                    schema_version: SCHEMA_VERSION_V1,
                    pipeline_id: PipelineId::new("pipeline-1").unwrap(),
                    stage_id: StageId::new("train").unwrap(),
                    attempt_id: attempt.clone(),
                    worker_id: WorkerId::new("worker-1").unwrap(),
                    fencing_token: FencingTokenV1::new(8).unwrap(),
                    previous_expires_at_unix_ms: 61_000,
                    renewed_at_unix_ms: 20_000,
                    expires_at_unix_ms: 80_000,
                },
            })
            .unwrap();
        assert!(
            stage
                .apply(StageEvent::ExecutionSucceeded {
                    attempt_id: attempt,
                    fencing_token: FencingTokenV1::new(8).unwrap(),
                    observed_at_unix_ms: 70_000,
                    manifest_digest: Sha256Digest::of_bytes(b"artifact"),
                })
                .is_ok()
        );
        assert_eq!(stage.phase(), StagePhase::Verifying);
    }

    #[test]
    fn exact_event_replay_is_ignored_and_conflicting_replay_is_atomic() {
        let mut stage = StageMachine::new(StageId::new("train").unwrap(), 2).unwrap();
        let accepted = EventEnvelope::new(
            MessageId::new("stage-event-1").unwrap(),
            StageEvent::DependenciesAccepted,
        );
        assert_eq!(stage.apply_event(&accepted), Ok(ApplyOutcome::Applied));
        let after_first = stage.clone();
        assert_eq!(
            stage.apply_event(&accepted),
            Ok(ApplyOutcome::DuplicateIgnored)
        );
        assert_eq!(stage, after_first);

        let conflict = EventEnvelope::new(
            MessageId::new("stage-event-1").unwrap(),
            StageEvent::RequestCancellation,
        );
        assert_eq!(
            stage.apply_event(&conflict),
            Err(TransitionError::EventIdConflict)
        );
        assert_eq!(stage, after_first);
    }

    proptest! {
        #[test]
        fn retry_fencing_tokens_are_strictly_monotonic_and_atomic(
            first_token in 1_u64..u64::MAX,
            stale_token in 1_u64..u64::MAX,
        ) {
            prop_assume!(stale_token <= first_token);
            let first_lease = lease("attempt-1", first_token);
            let first_attempt = first_lease.attempt_id.clone();
            let mut stage = accepted_stage(3, first_lease);
            stage
                .apply(StageEvent::LeaseExpired {
                    attempt_id: first_attempt,
                    fencing_token: FencingTokenV1::new(first_token).unwrap(),
                    observed_at_unix_ms: 61_000,
                })
                .unwrap();

            let before_stale_retry = stage.clone();
            let stale_retry = stage.apply(StageEvent::LeaseIssued {
                lease: lease("attempt-2", stale_token),
            });
            prop_assert!(stale_retry.is_err());
            prop_assert_eq!(&stage, &before_stale_retry);
            prop_assert_eq!(stage.attempts_started(), 1);

            stage
                .apply(StageEvent::LeaseIssued {
                    lease: lease("attempt-2", first_token + 1),
                })
                .unwrap();
            prop_assert_eq!(stage.attempts_started(), 2);
            prop_assert_eq!(stage.phase(), StagePhase::Leased);
            prop_assert_eq!(
                stage
                    .active_attempt()
                    .expect("retry lease is active")
                    .lease()
                    .fencing_token
                    .value(),
                first_token + 1
            );
        }

        #[test]
        fn attempt_ids_cannot_be_reused_even_with_a_higher_fence(
            first_token in 1_u64..u64::MAX,
        ) {
            let first_lease = lease("attempt-1", first_token);
            let first_attempt = first_lease.attempt_id.clone();
            let mut stage = accepted_stage(3, first_lease);
            stage
                .apply(StageEvent::LeaseExpired {
                    attempt_id: first_attempt,
                    fencing_token: FencingTokenV1::new(first_token).unwrap(),
                    observed_at_unix_ms: 61_000,
                })
                .unwrap();

            let before_reuse = stage.clone();
            let reused_attempt = stage.apply(StageEvent::LeaseIssued {
                lease: lease("attempt-1", first_token + 1),
            });
            prop_assert!(reused_attempt.is_err());
            prop_assert_eq!(&stage, &before_reuse);
            prop_assert_eq!(stage.attempts_started(), 1);
        }
    }
}
