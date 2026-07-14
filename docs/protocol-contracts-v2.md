# MojoColib V2 settlement protocol inventory

V2 is a **contract, signed-event and replay-state foundation**, not a payment
feature. It includes a narrow, local, single-writer coordinator journal, but
does not connect a wallet, custody funds, call Solana, run a coordinator
database, or admit public providers. Those capabilities remain gated by the
trusted pilot and the architecture decisions in [ADR-0010](adr/0010-network-topology-and-progressive-decentralisation.md)
and [ADR-0011](adr/0011-platform-fee-and-settlement-policy.md).

The implementation is in
[`crates/protocol/src/settlement_v2.rs`](../crates/protocol/src/settlement_v2.rs),
[`crates/state-machine/src/settlement_v2.rs`](../crates/state-machine/src/settlement_v2.rs)
and [`crates/coordinator-core/src/lib.rs`](../crates/coordinator-core/src/lib.rs).

## Contract family

All V2 records use `schema_version = 2`. Monetary values use `AtomicAmount`, a
canonical signed 64-bit integer serialized as base-10 text. Contracts reject
negative terms; no floating-point value participates in a quote, reservation
or settlement.

| Rust type | Purpose |
| --- | --- |
| `SettlementAssetV2` | Asset identity, precision, stable accounting unit and conversion evidence |
| `PayoutRecipientV2` | Actor-bound, rail-neutral payout destination |
| `QuoteV2` | Exact execution, verification, chain-fee and recipient caps plus the fee policy |
| `FundingAuthorizationV2` | Customer-signed authorization of one quote digest, exact amount and preallocated reservation |
| `ReservationV2` | One funding reservation bound to the quote digest and authorization |
| `AcceptedArtifactV2` | Artifact, attempt, provider and verifier evidence eligible for settlement |
| `ChainFeeReceiptV2` | Rail-, asset-, reservation-, nonce- and settlement-bound evidence for a capped fee reimbursement |
| `SettlementV2` | One immutable rail-pinned settlement intent with a unique nonce |
| `RailFinalityAttestationV2` | Rail-attestor evidence bound to the complete settlement digest, submitted reference and a `final`/`not_final` outcome |

`QuoteV2`, `FundingAuthorizationV2`, `ReservationV2`, `SettlementV2` and
`RailFinalityAttestationV2` implement `SignedContract`. The implemented
`SettlementSequencer` accepts only a signed outer lifecycle event. Its
constructor takes a registry-derived, reservation-pinned
`SettlementAuthoritySet`; individual events cannot supply or select their own
customer or rail witness. The set verifies the configured system sequencer
key, a `Customer` key whose registry `ActorId` equals the authorization's
`CustomerId`, and a distinct `System` rail-attestor key selected for the
attested rail. Distinct roles must use distinct decoded Ed25519 public-key
material, not merely different key IDs. The same three layers are verified
before an append and during journal replay. The local set retains historic rotated keys, but registry
publication and key-rotation policy are still production-coordinator work.

## Conservation rule

The funded reservation is fixed at quote time:

```text
funded_reservation_atomic = accepted_execution_cap_atomic
                          + verification_reserve_cap_atomic
                          + chain_fee_reserve_cap_atomic

operator_fee_atomic = min(
    operator_fee_cap_atomic,
    floor(accepted_execution_component_atomic * operator_fee_bps / 10_000)
)

provider_payout_atomic = accepted_execution_component_atomic - operator_fee_atomic
accepted_debits = provider_payout_atomic
               + verifier_payout_atomic
               + operator_fee_atomic
               + chain_fee_reimbursement_atomic
refund_atomic = funded_reservation_atomic - accepted_debits
```

The fee is visible, quote-bound and capped. It is allocated from accepted
execution value, so a provider cannot be charged a different rate after work
is accepted. A chain-fee reimbursement requires a matching quote-bound payer
and a receipt bound to the same asset, rail, reservation, settlement ID,
settlement nonce and submitted transaction reference; it cannot exceed its
reserve.

## Lifecycle reducer

`SettlementMachineV2` makes authority explicit:

```text
Draft -> QuoteAuthorised -> Reserved -> WorkAccepted -> SettlementIntent
      -> ChainPending -> FinancialFinal
                         ^
                         | reconciliation only
              ReconciliationRequired

ReservationExpired is permitted only before a chain submission.
```

An accepted artifact makes work technically eligible; it does not make a
financial transfer. A submitted chain reference is not final. A confirmation
or reconciliation carries signed rail evidence bound to the settlement digest,
nonce and original submitted reference. Retrying requires an attested
`not_final` disposition; financial finality requires `final`. Once the reducer
records `FinancialFinal`, no off-chain expiry or reassignment event can replace
it. For the local coordinator boundary, lifecycle observations must equal the
signed outer event timestamp and the coordinator's supplied admission time;
the local core rejects a split or backdated event timestamp relative to that
supplied value. The observation clock is nondecreasing, expiry cannot be
recorded in the future, and work acceptance or intent creation reject admission
at or after the reservation expiry. This is a trusted-pilot admission contract,
not proof of an external wall clock; a production coordinator must own and
persist the clock source.

## Current durable boundary

`SettlementSequencer` is a deliberately small local pilot component. It writes
checksummed, Ed25519-signed event frames for **one reservation stream**, verifies
the configured sequencer signature plus any required customer or rail signature
against its pinned authority set on both append and replay, rejects out-of-order
sequences, and recovers only an interrupted trailing frame. A corrupt complete
frame or a missing historic authority key fails closed. `apply_signed` requires
the coordinating service to supply its admission time; it rejects a signed event
whose outer or lifecycle timestamp differs, then reuses the signed outer time on
replay. Raw reducer events are not accepted through the coordinator-core public
API. The current method still receives a scalar time from its trusted local
caller, so it is not a substitute for a production-owned clock capability.

The journal intentionally does not provide a cross-process lock, database
transaction, backup, audit/outbox, key-rotation service, customer/rail registry,
multi-reservation authorization consumption, global receipt/transaction
uniqueness, or a refund action. Those are prerequisites for any financial rail.

## Not implemented by this inventory

- a production coordinator database, transactions, audit/outbox, backup,
  cross-process ownership or multi-reservation recovery;
- customer account registration, custody, real-value balance handling or a
  wallet interface;
- Solana/localnet/devnet program code, transaction submission, registry-backed
  rail adapters or real receipt verification;
- network worker sessions, peer discovery, verifier quorum or public provider
  admission.

These are deliberately separate milestones. V2 lets them share unambiguous,
testable terms instead of retrofitting money semantics into V1 test credits.
