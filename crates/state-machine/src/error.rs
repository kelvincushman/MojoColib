use thiserror::Error;

/// A rejected event. Rejection never mutates the state machine.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TransitionError {
    #[error("event {event} is invalid while in {state}")]
    InvalidTransition {
        state: &'static str,
        event: &'static str,
    },

    #[error("event binding does not match the active attempt")]
    AttemptMismatch,

    #[error("stale fencing token")]
    StaleFencingToken,

    #[error("retry budget is exhausted")]
    RetryBudgetExhausted,

    #[error("ledger invariant failed: {0}")]
    LedgerInvariant(String),

    #[error("arithmetic overflow")]
    ArithmeticOverflow,

    #[error("event id was reused with different content")]
    EventIdConflict,

    #[error("protocol error: {0}")]
    Protocol(String),
}

impl From<colibri_protocol::ProtocolError> for TransitionError {
    fn from(error: colibri_protocol::ProtocolError) -> Self {
        Self::Protocol(error.to_string())
    }
}
