//! Runtime-neutral, content-addressed contracts shared by every `MojoColib` component.

#![forbid(unsafe_code)]
#![allow(clippy::missing_errors_doc)]

pub mod artifact;
pub mod audit;
pub mod benchmark;
pub mod canonical;
pub mod digest;
pub mod executor;
pub mod ids;
pub mod lease;
pub mod ledger;
pub mod pipeline;
pub mod provider;
pub mod signing;
pub mod strict_json;
pub mod template;
pub mod verification;

mod error;

pub use error::ProtocolError;

/// Schema version used by the first public contract family.
pub const SCHEMA_VERSION_V1: u16 = 1;
