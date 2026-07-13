# ADR-0005: Fixed microcredit ledger units

- Status: Accepted
- Date: 2026-07-12
- Owners: Ledger and protocol owners

## Decision

The MVP uses valueless test credits denominated in integer microcredits: one displayed credit equals 1,000,000 microcredits. All internal arithmetic is signed 64-bit checked arithmetic; overflow, underflow and negative quote/reservation/settlement amounts are hard errors. No floating-point value participates in funding, settlement or reconciliation.

The canonical JSON wire form is a base-10 string representing one `i64`, with no decimal point, exponent, leading plus sign or surrounding whitespace. APIs may add separately labelled display strings, but those are never authoritative.

Before scheduling, a customer reserves the accepted execution quote plus its declared verification reserve. A settlement binds the pipeline, stage, accepted attempt, artifact manifest, provider, quote and verification decision. The total provider credit plus operator fee cannot exceed the open reservation. A stage may settle exactly once. Completion or cancellation releases all remaining reserved value to the customer.

Energy and tariff estimates never alter a previously accepted quote. They are evidence fields in the provider-economics report. Initial quote construction rounds each independently computed cost component upward to the nearest microcredit and records the pricing-policy version; it never rounds the final total downward.

## Replay invariants

- Legal causal order is accepted quote, reservation, accepted artifact/verification, settlement, release/close.
- Repeating an event ID with identical canonical content is a no-op.
- Reusing an event ID with different content is corruption.
- Replaying events produces exactly the same balances and reservation states.
- A causally premature or mismatched event is rejected without changing state.
- Sum of available customer credits, open reservations, provider credits and operator fees equals issued test credits.

The first reducer is implemented in [`crates/state-machine/src/ledger.rs`](../../crates/state-machine/src/ledger.rs). Solana may mirror final settlements later; chain state never decides whether training output is accepted.
