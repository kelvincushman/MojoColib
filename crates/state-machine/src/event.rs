//! Replay-safe envelopes shared by deterministic reducers.

use colibri_protocol::ids::MessageId;
use serde::{Deserialize, Serialize};

/// A stable event identity paired with its immutable payload.
///
/// Reducers remember the canonical payload digest for every accepted ID. An
/// exact retry is ignored, while reusing the ID for different content is an
/// error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventEnvelope<E> {
    pub event_id: MessageId,
    pub payload: E,
}

impl<E> EventEnvelope<E> {
    #[must_use]
    pub const fn new(event_id: MessageId, payload: E) -> Self {
        Self { event_id, payload }
    }
}

/// Whether an event changed materialized state or was an exact retry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyOutcome {
    Applied,
    DuplicateIgnored,
}
