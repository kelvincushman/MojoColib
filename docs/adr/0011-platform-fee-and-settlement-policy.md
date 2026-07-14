# ADR-0011: V2 platform-fee and settlement policy

- **Status:** Accepted; signed pilot journal implemented, financial rail disabled
- **Date:** 2026-07-13
- **Owners:** Ledger, settlement and security owners
- **Review trigger:** Before connecting a durable coordinator, changing a fee
  schedule or recipient, adding a settlement rail, or accepting any real value

## Context

The V1 ledger deliberately uses valueless microcredits. A `QuoteV1` reserves
execution and verification amounts, and a `SettlementV1` has a provider credit
and an operator-fee field. It has no customer-bound funding authorization,
asset/recipient binding, chain-fee reimbursement evidence, or distinction
between technical acceptance and financial finality.

Reinterpreting V1's `operator_fee` as a real platform fee would make historical
records ambiguous. V1 stays immutable and continues to settle only under
[ADR-0005](0005-ledger-units.md). This record establishes a separately
versioned V2 family for the later marketplace path.

## Decision

V2 is a quote-bound, fixed-integer settlement model. It is implemented as
protocol contracts and a replay-safe lifecycle reducer, but it is **not** a
wallet flow, custody system, Solana integration, customer account ledger or
real-money feature.

1. V2 consists of `QuoteV2`, `FundingAuthorizationV2`, `ReservationV2`,
   `AcceptedArtifactV2`, `ChainFeeReceiptV2`, `SettlementV2`,
   `RailFinalityAttestationV2` and their V2 lifecycle events. The current
   `SettlementSequencer` persists only signed outer events for one reservation
   stream. V1 records remain unchanged and cannot fund or settle a V2
   reservation.
2. Every V2 monetary value is an `AtomicAmount`: canonical base-10 signed
   64-bit text that must be non-negative in a contract. The quote declares the
   settlement asset, decimals, stable accounting unit and conversion evidence.
   No floating-point value participates in funding, payout or reconciliation.
3. The customer funds exactly one canonical quote digest and one preallocated
   reservation ID. A valid `FundingAuthorizationV2` names the customer, quote,
   funding account, authorization ID, reservation ID, exact funded amount and
   bounded expiry. It must be signed by a trusted `Customer` key. In the pilot,
   that key's registry `ActorId` must equal the `CustomerId`; a federation may
   replace this with an explicit registry mapping, not an unsigned alias.
4. The operator fee is transparent, immutable and percentage-based. It is a
   quote-bound allocation of accepted execution value, not a hidden
   settlement-time change and not a fee on every incoming SOL transfer. The
   customer sees the fee rule, cap and recipient before authorizing funding.
5. Technical acceptance and financial finality are separate. A promoted,
   verified artifact can create only an off-chain `SettlementV2` intent. A
   later settlement rail may make that intent financially final; no chain
   receipt decides whether ML work is accepted.
6. A confirmed financial movement can never be overwritten by an off-chain
   replay. A timeout or uncertain chain submission requires reconciliation
   before retry or finality, rather than a second payment or automatic refund.

## Immutable V2 terms

### Quote and funding

`QuoteV2` contains these exact maximum terms:

- `accepted_execution_cap_atomic`;
- `verification_reserve_cap_atomic`;
- `chain_fee_reserve_cap_atomic`;
- provider, verifier and operator recipient identities and payout caps;
- `operator_fee_bps` in `0..=10_000` and `operator_fee_cap_atomic`;
- one quote-bound `chain_fee_payer`; and
- asset, conversion, pricing-policy and expiry evidence.

The exact required funding amount is:

```text
funded_reservation_atomic = accepted_execution_cap_atomic
                          + verification_reserve_cap_atomic
                          + chain_fee_reserve_cap_atomic
```

`FundingAuthorizationV2` binds that amount, the canonical quote digest, an
authorization ID, customer identity, funding-account reference, preallocated
reservation ID and expiry. `ReservationV2` must reproduce those bindings
exactly; it cannot reserve more than the quote or outlive the
authorization/quote window.

### Accepted work and settlement intent

`AcceptedArtifactV2` binds a reservation and quote digest to one pipeline,
stage, attempt, artifact manifest, provider and verification-decision digest.
It is the sole lineage accepted by a `SettlementV2`.

For accepted work, the fee and payout must obey this deterministic formula:

```text
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

The accepted execution component cannot exceed the quote cap. Provider and
verifier payouts cannot exceed their recipient and reserve caps. The sum of
accepted debits cannot exceed the funded reservation. Integer division rounds
the percentage fee down before applying its absolute cap.

`chain_fee_reimbursement_atomic` is zero unless `ChainFeeReceiptV2` names the
quote-bound payer, rail, asset, reservation, quote, settlement ID and settlement
nonce, provides the submitted transaction reference and proves an amount at
least as large as the reimbursement. It can never exceed the chain-fee reserve.
The current receipt is rail-neutral evidence, not a claim that a chain program or RPC
verifier exists.

## Authority and lifecycle

`SettlementMachineV2` is the deterministic lifecycle for one reservation:

```text
Draft -> QuoteAuthorised -> Reserved -> WorkAccepted -> SettlementIntent
      -> ChainPending -> FinancialFinal
                         ^
                         | reconciled finality only
              ReconciliationRequired

ReservationExpired is legal only before a chain submission.
```

The core rules are:

- an exact replay is ignored; reusing an event ID with different canonical
  content fails without mutation;
- quote authorization validates the quote digest, customer funding amount and
  active expiry; reservation validates the authorization again;
- an accepted artifact and settlement intent must match the same quote,
  reservation, attempt, artifact, provider and verification decision;
- only a matching rail-attestor-signed `ChainConfirmationRecorded` or
  `ChainFinalityReconciled` event can enter `FinancialFinal`; each attests the
  full settlement digest, nonce and originally submitted reference;
- a submitted but uncertain transaction enters
  `ReconciliationRequired`; an attested `not_final` `ChainRetryAuthorised` is
  the only route back to a new submission; and
- neither expiry nor reassignment can alter `FinancialFinal`.

The current local `SettlementSequencer` establishes an initial protocol
boundary: it accepts only a signed system-sequencer event and receives a
registry-derived, reservation-pinned `SettlementAuthoritySet` at construction.
Customer and rail evidence are independently verified against that set on both
append and replay, so a caller cannot select a witness per event. Checksummed
frames replay with strict sequence ordering, and a corrupt complete frame or a
missing historic authority key fails closed. The sequencer, customer and rail
roles must have distinct decoded Ed25519 public-key material, even if their
registry key IDs differ. Lifecycle observations must equal the signed outer
event timestamp and the coordinating service's admission time; they are
monotonic, and reject a future expiry or mismatched pre-expiry acceptance. The
local core trusts that caller-provided admission value; a production wrapper must
source and persist it itself. It is one single-writer reservation
journal, not a production coordinator. A production coordinator must persist
the event, materialized state, audit record and outbound settlement intent
atomically, source and rotate the authority set through a durable registry, and
retain historic keys for replay. A chain sender runs outside that database
transaction and uses the exact settlement identity/nonce. It may not create a
second payable intent while the first is pending or unreconciled.

## Required persistence and rail gates

Before V2 can move beyond local, valueless contract tests, the coordinator must
provide all of the following:

1. a serializable transaction covering state, audit, idempotency and an
   outbox record;
2. durable **global** uniqueness for authorization identity, reservation,
   accepted stage, settlement intent, fee receipt and rail transaction
   reference across all reservation streams;
3. backup/recovery that replays the ledger and leaves no duplicated payout or
   stranded reservation;
4. customer key registration and signature/replay enforcement, without a
   coordinator signature standing in for customer consent;
5. a durable coordinator-owned admission clock, chronological rail evidence and
   receipt validation, retry/reconciliation, expiry/refund and
   compensation rules for the chosen settlement rail; and
6. localnet then devnet testing, independent security review, legal/tax,
   custody and incident-response approval before any real-value operation.

Solana is a possible later escrow/finality rail, not an ML verifier, scheduler,
heartbeat channel or required runtime. Prompts, datasets, checkpoints and
weights must never be placed on-chain.

## V1-to-V2 migration

- Existing V1 quotes, reservations, settlements and events are never
  rewritten. Historical V1 `operator_fee` is not retroactively relabelled as a
  V2 platform fee.
- An open V1 reservation must settle or release under V1. It cannot be upgraded
  in place because it lacks V2 funding consent and recipient/finality terms.
- A user who needs V2 receives a fresh quote and signs a fresh
  `FundingAuthorizationV2`; no V1 quote, signature or reservation can fund it.
- Any balance carry-forward is a separate, audited coordinator migration with
  reconciliation evidence. It must not consume a V1 reserved balance or add a
  fee.

## Consequences

The platform can eventually earn a stated percentage only when useful work is
accepted under terms the customer authorized. Providers can verify how their
payout follows from the same quote. A consumer-facing price comparison can
include verification, storage, network, retry and fee costs instead of relying
on unverified GPU-hour claims.

The cost is intentional complexity: customer authorization, replay-safe state,
durable artifact/verification evidence, an outbox and financial reconciliation
are mandatory before connecting a real settlement rail.

## Rejected alternatives

- **Apply a global fee at settlement time.** Rejected because it permits a
  post-authorization price change.
- **Charge a fee for claimed GPU time or every transfer.** Rejected because
  payment must be tied to accepted work and a bounded reservation.
- **Treat a submitted transaction as financially final.** Rejected because a
  timeout, drop or reorganization could otherwise cause duplicate payment.
- **Let the chain decide artifact acceptance.** Rejected because technical
  verification remains off-chain.
- **Convert V1 reservations in place.** Rejected because V1 does not contain
  the consent and finality commitments required for V2.

## Related records

- [ADR-0005: Fixed microcredit ledger units](0005-ledger-units.md)
- [ADR-0010: Network topology and progressive decentralisation](0010-network-topology-and-progressive-decentralisation.md)
- [V2 settlement protocol inventory](../protocol-contracts-v2.md)
- [Training marketplace PRD](../mojocolib-training-prd.md)
