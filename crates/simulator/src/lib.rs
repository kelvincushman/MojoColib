//! Deterministic, in-memory execution of the Step 1 reducer happy path.

#![forbid(unsafe_code)]
#![allow(clippy::missing_errors_doc)]

use colibri_protocol::{
    ProtocolError, SCHEMA_VERSION_V1,
    digest::Sha256Digest,
    ids::{
        ArtifactId, AttemptId, CustomerId, LedgerEventId, MessageId, PipelineId, ProviderId,
        QuoteId, ReservationId, SettlementId, StageId, WorkerId,
    },
    lease::{FencingTokenV1, StageLeaseV1},
    ledger::{Microcredits, QuoteV1, ReservationV1, SettlementV1},
};
use colibri_state_machine::{
    ApplyOutcome as ReducerOutcome, EventEnvelope, TransitionError,
    ledger::{ApplyOutcome as LedgerOutcome, ArtifactAcceptance, LedgerEvent, LedgerMachine},
    pipeline::{PipelineEvent, PipelineMachine},
    stage::{StageEvent, StageMachine},
};
use serde::Serialize;
use thiserror::Error;

const PIPELINE_SPEC: &[u8] = b"step-1-pipeline-spec";
const TEMPLATE: &[u8] = b"step-1-template";
const ARTIFACT_MANIFEST: &[u8] = b"step-1-artifact";
const VERIFICATION_DECISION: &[u8] = b"step-1-verification";

#[derive(Debug, Error)]
pub enum SimulationError {
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    #[error(transparent)]
    Transition(#[from] TransitionError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("{0} exact replay was not ignored")]
    ReplayNotIgnored(&'static str),
    #[error("{0} state changed during exact replay")]
    ReplayChangedState(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompletionSummary {
    pub simulation: &'static str,
    pub pipeline: PipelineSummary,
    pub stage: StageSummary,
    pub ledger: LedgerSummary,
    pub replay: ReplaySummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PipelineSummary {
    pub id: String,
    pub phase: &'static str,
    pub sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StageSummary {
    pub id: String,
    pub phase: &'static str,
    pub sequence: u64,
    pub attempts_started: u16,
    pub artifact_manifest_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LedgerSummary {
    pub customer_id: String,
    pub customer_available_microcredits: i64,
    pub provider_id: String,
    pub provider_credit_microcredits: i64,
    pub operator_fee_microcredits: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ReplaySummary {
    pub exact_replays_ignored: u8,
    pub state_unchanged: bool,
}

/// Runs a fixed happy path through the pipeline, stage, and ledger reducers.
#[allow(clippy::too_many_lines)]
pub fn run_happy_path() -> Result<CompletionSummary, SimulationError> {
    let pipeline_id = PipelineId::new("pipeline-step-1")?;
    let stage_id = StageId::new("stage-training-1")?;
    let attempt_id = AttemptId::new("attempt-1")?;
    let customer_id = CustomerId::new("customer-demo")?;
    let provider_id = ProviderId::new("provider-demo")?;
    let artifact_id = ArtifactId::new("artifact-output-1")?;
    let quote_id = QuoteId::new("quote-1")?;
    let reservation_id = ReservationId::new("reservation-1")?;
    let spec_digest = Sha256Digest::of_bytes(PIPELINE_SPEC);
    let artifact_manifest_digest = Sha256Digest::of_bytes(ARTIFACT_MANIFEST);
    let verification_decision_digest = Sha256Digest::of_bytes(VERIFICATION_DECISION);

    let mut pipeline = PipelineMachine::new(pipeline_id.clone(), spec_digest);
    let pipeline_events = [
        envelope("pipeline-event-validate", PipelineEvent::Validate)?,
        envelope("pipeline-event-fund", PipelineEvent::Fund)?,
        envelope("pipeline-event-start", PipelineEvent::Start)?,
        envelope("pipeline-event-complete", PipelineEvent::AllStagesAccepted)?,
    ];
    for event in &pipeline_events[..3] {
        pipeline.apply(event)?;
    }

    let lease = StageLeaseV1 {
        schema_version: SCHEMA_VERSION_V1,
        pipeline_id: pipeline_id.clone(),
        pipeline_spec_digest: spec_digest,
        stage_id: stage_id.clone(),
        attempt_id: attempt_id.clone(),
        worker_id: WorkerId::new("worker-demo")?,
        template_digest: Sha256Digest::of_bytes(TEMPLATE),
        input_manifest_digests: Vec::new(),
        issued_at_unix_ms: 30_000,
        expires_at_unix_ms: 60_000,
        fencing_token: FencingTokenV1::new(1)?,
    };
    let stage_events = [
        envelope("stage-event-dependencies", StageEvent::DependenciesAccepted)?,
        envelope(
            "stage-event-lease-issued",
            StageEvent::LeaseIssued {
                lease: lease.clone(),
            },
        )?,
        envelope(
            "stage-event-lease-accepted",
            StageEvent::LeaseAccepted {
                attempt_id: attempt_id.clone(),
                fencing_token: lease.fencing_token,
                observed_at_unix_ms: 31_000,
            },
        )?,
        envelope(
            "stage-event-execution-succeeded",
            StageEvent::ExecutionSucceeded {
                attempt_id: attempt_id.clone(),
                fencing_token: lease.fencing_token,
                observed_at_unix_ms: 40_000,
                manifest_digest: artifact_manifest_digest,
            },
        )?,
        envelope(
            "stage-event-verification-accepted",
            StageEvent::VerificationAccepted {
                attempt_id: attempt_id.clone(),
                manifest_digest: artifact_manifest_digest,
            },
        )?,
    ];
    let mut stage = StageMachine::new(stage_id.clone(), 2)?;
    for event in &stage_events {
        stage.apply_event(event)?;
    }

    let quote = QuoteV1 {
        schema_version: SCHEMA_VERSION_V1,
        quote_id: quote_id.clone(),
        pipeline_id: pipeline_id.clone(),
        stage_id: stage_id.clone(),
        provider_id: provider_id.clone(),
        execution: Microcredits::new(700),
        verification_reserve: Microcredits::new(200),
        policy_version: 1,
        expires_at_unix_ms: 120_000,
    };
    let reservation = ReservationV1 {
        schema_version: SCHEMA_VERSION_V1,
        reservation_id: reservation_id.clone(),
        customer_id: customer_id.clone(),
        quote_id: quote_id.clone(),
        pipeline_id: pipeline_id.clone(),
        stage_id: stage_id.clone(),
        amount: Microcredits::new(900),
    };
    let acceptance = ArtifactAcceptance {
        reservation_id: reservation_id.clone(),
        quote_id: quote_id.clone(),
        pipeline_id: pipeline_id.clone(),
        stage_id: stage_id.clone(),
        attempt_id: attempt_id.clone(),
        artifact_id: artifact_id.clone(),
        artifact_manifest_digest,
        provider_id: provider_id.clone(),
        verification_decision_digest,
    };
    let settlement = SettlementV1 {
        schema_version: SCHEMA_VERSION_V1,
        settlement_id: SettlementId::new("settlement-1")?,
        reservation_id: reservation_id.clone(),
        quote_id: quote_id.clone(),
        pipeline_id: pipeline_id.clone(),
        stage_id: stage_id.clone(),
        attempt_id,
        artifact_id,
        artifact_manifest_digest,
        provider_id: provider_id.clone(),
        verification_decision_digest,
        provider_credit: Microcredits::new(650),
        operator_fee: Microcredits::new(150),
    };
    let ledger_events = [
        LedgerEvent::TestCreditsIssued {
            event_id: LedgerEventId::new("ledger-event-credit")?,
            customer_id: customer_id.clone(),
            amount: Microcredits::new(1_000),
        },
        LedgerEvent::QuoteAccepted {
            event_id: LedgerEventId::new("ledger-event-quote")?,
            accepted_at_unix_ms: 10_000,
            quote,
        },
        LedgerEvent::ReservationCreated {
            event_id: LedgerEventId::new("ledger-event-reservation")?,
            funded_at_unix_ms: 20_000,
            reservation,
        },
        LedgerEvent::ArtifactAccepted {
            event_id: LedgerEventId::new("ledger-event-artifact")?,
            acceptance,
        },
        LedgerEvent::SettlementAccepted {
            event_id: LedgerEventId::new("ledger-event-settlement")?,
            settlement,
        },
        LedgerEvent::ReservationReleased {
            event_id: LedgerEventId::new("ledger-event-release")?,
            reservation_id,
        },
    ];
    let mut ledger = LedgerMachine::default();
    for event in &ledger_events {
        ledger.apply(event)?;
    }
    pipeline.apply(&pipeline_events[3])?;

    verify_pipeline_replay(&mut pipeline, &pipeline_events[2])?;
    verify_stage_replay(&mut stage, &stage_events[3])?;
    verify_ledger_replay(&mut ledger, &ledger_events[4])?;

    Ok(CompletionSummary {
        simulation: "mojocolib-step-1-happy-path-v1",
        pipeline: PipelineSummary {
            id: pipeline_id.to_string(),
            phase: "completed",
            sequence: pipeline.sequence(),
        },
        stage: StageSummary {
            id: stage_id.to_string(),
            phase: "accepted",
            sequence: stage.sequence(),
            attempts_started: stage.attempts_started(),
            artifact_manifest_digest: artifact_manifest_digest.to_string(),
        },
        ledger: LedgerSummary {
            customer_id: customer_id.to_string(),
            customer_available_microcredits: ledger.customer_available(&customer_id).value(),
            provider_id: provider_id.to_string(),
            provider_credit_microcredits: ledger.provider_credit(&provider_id).value(),
            operator_fee_microcredits: ledger.operator_fees().value(),
        },
        replay: ReplaySummary {
            exact_replays_ignored: 3,
            state_unchanged: true,
        },
    })
}

pub fn run_happy_path_json() -> Result<String, SimulationError> {
    Ok(serde_json::to_string_pretty(&run_happy_path()?)?)
}

fn envelope<E>(event_id: &str, payload: E) -> Result<EventEnvelope<E>, SimulationError> {
    Ok(EventEnvelope::new(MessageId::new(event_id)?, payload))
}

fn verify_pipeline_replay(
    pipeline: &mut PipelineMachine,
    event: &EventEnvelope<PipelineEvent>,
) -> Result<(), SimulationError> {
    let before = pipeline.clone();
    if pipeline.apply(event)? != ReducerOutcome::DuplicateIgnored {
        return Err(SimulationError::ReplayNotIgnored("pipeline"));
    }
    if *pipeline != before {
        return Err(SimulationError::ReplayChangedState("pipeline"));
    }
    Ok(())
}

fn verify_stage_replay(
    stage: &mut StageMachine,
    event: &EventEnvelope<StageEvent>,
) -> Result<(), SimulationError> {
    let before = stage.clone();
    if stage.apply_event(event)? != ReducerOutcome::DuplicateIgnored {
        return Err(SimulationError::ReplayNotIgnored("stage"));
    }
    if *stage != before {
        return Err(SimulationError::ReplayChangedState("stage"));
    }
    Ok(())
}

fn verify_ledger_replay(
    ledger: &mut LedgerMachine,
    event: &LedgerEvent,
) -> Result<(), SimulationError> {
    let before = ledger.clone();
    if ledger.apply(event)? != LedgerOutcome::DuplicateIgnored {
        return Err(SimulationError::ReplayNotIgnored("ledger"));
    }
    if *ledger != before {
        return Err(SimulationError::ReplayChangedState("ledger"));
    }
    Ok(())
}
