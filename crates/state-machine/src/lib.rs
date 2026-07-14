//! Deterministic, side-effect-free state transitions for coordinator simulation and replay.

#![forbid(unsafe_code)]
#![allow(clippy::missing_errors_doc)]

pub mod ledger;
pub mod pipeline;
pub mod settlement_v2;
pub mod stage;

mod event;

mod error;

pub use error::TransitionError;
pub use event::{ApplyOutcome, EventEnvelope};
