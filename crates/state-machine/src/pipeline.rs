//! Pipeline lifecycle transitions independent of storage and transport.

use std::collections::BTreeMap;

use colibri_protocol::{
    canonical,
    digest::Sha256Digest,
    ids::{MessageId, PipelineId},
};
use serde::{Deserialize, Serialize};

use crate::{ApplyOutcome, EventEnvelope, TransitionError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelinePhase {
    Draft,
    Validated,
    Funded,
    Running,
    Draining,
    Completed,
    Failed,
    Cancelled,
}

impl PipelinePhase {
    const fn name(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Validated => "validated",
            Self::Funded => "funded",
            Self::Running => "running",
            Self::Draining => "draining",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineEvent {
    Validate,
    Fund,
    Start,
    RequestCancellation,
    AllStagesAccepted,
    RetryBudgetExhausted,
    DrainCompleted,
}

impl PipelineEvent {
    const fn name(self) -> &'static str {
        match self {
            Self::Validate => "validate",
            Self::Fund => "fund",
            Self::Start => "start",
            Self::RequestCancellation => "request_cancellation",
            Self::AllStagesAccepted => "all_stages_accepted",
            Self::RetryBudgetExhausted => "retry_budget_exhausted",
            Self::DrainCompleted => "drain_completed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PipelineMachine {
    pipeline_id: PipelineId,
    spec_digest: Sha256Digest,
    phase: PipelinePhase,
    sequence: u64,
    applied_events: BTreeMap<MessageId, Sha256Digest>,
}

impl PipelineMachine {
    #[must_use]
    pub const fn new(pipeline_id: PipelineId, spec_digest: Sha256Digest) -> Self {
        Self {
            pipeline_id,
            spec_digest,
            phase: PipelinePhase::Draft,
            sequence: 0,
            applied_events: BTreeMap::new(),
        }
    }

    #[must_use]
    pub const fn pipeline_id(&self) -> &PipelineId {
        &self.pipeline_id
    }

    #[must_use]
    pub const fn spec_digest(&self) -> Sha256Digest {
        self.spec_digest
    }

    #[must_use]
    pub const fn phase(&self) -> PipelinePhase {
        self.phase
    }

    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn apply(
        &mut self,
        envelope: &EventEnvelope<PipelineEvent>,
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
        next.apply_new(envelope.payload)?;
        next.applied_events
            .insert(envelope.event_id.clone(), event_digest);
        *self = next;
        Ok(ApplyOutcome::Applied)
    }

    fn apply_new(&mut self, event: PipelineEvent) -> Result<(), TransitionError> {
        let next = match (self.phase, event) {
            (PipelinePhase::Draft, PipelineEvent::Validate) => PipelinePhase::Validated,
            (PipelinePhase::Validated, PipelineEvent::Fund) => PipelinePhase::Funded,
            (PipelinePhase::Funded, PipelineEvent::Start) => PipelinePhase::Running,
            (PipelinePhase::Funded, PipelineEvent::RequestCancellation)
            | (PipelinePhase::Draining, PipelineEvent::DrainCompleted) => PipelinePhase::Cancelled,
            (PipelinePhase::Running, PipelineEvent::RequestCancellation) => PipelinePhase::Draining,
            (PipelinePhase::Running, PipelineEvent::AllStagesAccepted) => PipelinePhase::Completed,
            (PipelinePhase::Running, PipelineEvent::RetryBudgetExhausted) => PipelinePhase::Failed,
            _ => {
                return Err(TransitionError::InvalidTransition {
                    state: self.phase.name(),
                    event: event.name(),
                });
            }
        };

        let next_sequence = self
            .sequence
            .checked_add(1)
            .ok_or(TransitionError::ArithmeticOverflow)?;
        self.phase = next;
        self.sequence = next_sequence;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{PipelineEvent, PipelineMachine, PipelinePhase};
    use crate::{ApplyOutcome, EventEnvelope};
    use colibri_protocol::{
        digest::Sha256Digest,
        ids::{MessageId, PipelineId},
    };

    fn event(id: &str, payload: PipelineEvent) -> EventEnvelope<PipelineEvent> {
        EventEnvelope::new(MessageId::new(id).unwrap(), payload)
    }

    #[test]
    fn terminal_state_rejects_more_events_without_mutation() {
        let mut pipeline = PipelineMachine::new(
            PipelineId::new("pipeline-1").unwrap(),
            Sha256Digest::of_bytes(b"spec"),
        );
        pipeline
            .apply(&event("event-1", PipelineEvent::Validate))
            .unwrap();
        pipeline
            .apply(&event("event-2", PipelineEvent::Fund))
            .unwrap();
        pipeline
            .apply(&event("event-3", PipelineEvent::Start))
            .unwrap();
        pipeline
            .apply(&event("event-4", PipelineEvent::AllStagesAccepted))
            .unwrap();
        let before = pipeline.clone();
        assert!(
            pipeline
                .apply(&event("event-5", PipelineEvent::Start))
                .is_err()
        );
        assert_eq!(pipeline, before);
        assert_eq!(pipeline.phase, PipelinePhase::Completed);
    }

    #[test]
    fn exact_replay_is_ignored_and_changed_replay_is_rejected() {
        let mut pipeline = PipelineMachine::new(
            PipelineId::new("pipeline-1").unwrap(),
            Sha256Digest::of_bytes(b"spec"),
        );
        let validate = event("event-1", PipelineEvent::Validate);
        assert_eq!(pipeline.apply(&validate), Ok(ApplyOutcome::Applied));
        let after_first = pipeline.clone();
        assert_eq!(
            pipeline.apply(&validate),
            Ok(ApplyOutcome::DuplicateIgnored)
        );
        assert_eq!(pipeline, after_first);
        assert_eq!(
            pipeline.apply(&event("event-1", PipelineEvent::Fund)),
            Err(crate::TransitionError::EventIdConflict)
        );
        assert_eq!(pipeline, after_first);
    }
}
