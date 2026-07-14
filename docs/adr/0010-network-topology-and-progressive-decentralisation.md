# ADR-0010: Network topology and progressive decentralisation

- **Status:** Accepted as the architecture baseline
- **Date:** 2026-07-13
- **Owners:** Protocol, coordinator and security owners
- **Review trigger:** Before admitting an independent coordinator or verifier
  operator, opening provider admission, enabling a money-like settlement rail,
  or changing membership, quorum or dispute rules

## Context

MojoColib aims to let independently owned machines provide useful, bounded AI
work. That does not make a collection of household machines a single
low-latency computer, nor does it remove the need for coordination, artifact
availability, verification, dispute handling and settlement.

The current repository is a trusted-pilot foundation. It has signed worker
capabilities, fenced leases, exact off-chain test-credit accounting and a
local-worker/artifact vertical slice, plus a signed single-reservation pilot
journal, but no production coordinator, remote worker service, peer discovery,
replicated event log, public admission or real-money settlement. The current
trust boundary is explicitly a trusted operator and
approved verification workers; see the [threat model](../threat-model.md) and
the [P2P boundary](../../README.md#what-p2p-means-here).

Calling that foundation "decentralised" now would be inaccurate. Conversely,
making every provider a control-plane peer from day one would make duplicate
leases, ambiguous acceptance and double settlement much harder to prevent than
the ML workloads themselves.

This decision defines a path from a trusted pilot to federated services and,
only after evidence, to public provider participation. It governs control and
acceptance authority. It does **not** approve a currency, custody model or
Solana program; those require the separate V2 fee and settlement decision.

## Decision

MojoColib will use **reservation-scoped authority with progressive
federation**.

Workers remain self-managed: a provider controls its hardware, availability,
power/thermal limits, storage quota, update window and payout address. The
trust plane remains a set of explicit services. "Self-managed" and
"federated" do not mean infrastructure-free or literally serverless.

Every funded reservation must pin a signed authority binding before work is
assigned. The binding is immutable for that reservation and includes, at
minimum:

- a network and membership-epoch identifier and digest;
- the sole lease-issuer identity;
- the sole settlement-sequencer identity;
- a verifier-policy digest, including eligible verifier identities, quorum and
  independence rules;
- a dispute-policy digest and the authority allowed to apply it; and
- an event-ordering version and artifact/retention policy digest.

These names are protocol requirements, not a commitment to their exact Rust
struct names. A scheduler may recommend a worker, but it has no authority to
issue a lease or settle a reservation. A relay may forward encrypted traffic,
but has no authority to accept work, alter event order or release value.

### Roles and authority boundaries

| Role | May do | Must not do |
| --- | --- | --- |
| Customer | Authorise a bounded quote/reservation, submit approved work, inspect evidence and invoke the published dispute process. | Directly assign a worker, accept its own output or change a settled amount. |
| Provider and worker | Advertise signed capabilities, accept a valid lease, run a bounded approved template and submit signed result evidence. | Issue leases, self-verify accepted work, sequence settlement or access customer/settlement keys. |
| Scheduler/coordinator | Validate eligibility and propose assignments from committed policies. | Make an unaudited acceptance or bypass the reservation's issuer/sequencer binding. |
| Lease issuer | Atomically issue, renew or expire the one fenced lease permitted by a reservation's state. | Issue a second live lease for the same stage/fence, accept artifacts or release settlement. |
| Artifact service | Store attempt-temporary objects and durable content-addressed promotions under short-lived grants. | Decide whether an artifact is accepted or change its manifest lineage. |
| Verifier | Reproduce the committed validation policy and sign an attestation over identical candidate evidence. | Pay a provider, change quote terms or verify work it produced where the policy forbids that relationship. |
| Settlement sequencer | Append the canonical ordered acceptance/settlement decision once its required evidence exists. | Invent verifier evidence, reorder a prior reservation event, or release more than the reservation permits. |
| Relay/rendezvous service | Help an outbound authenticated session find or reach an authorised peer within rate/bandwidth limits. | Read artifact plaintext without a grant, impersonate a worker, or become an implicit coordinator. |
| Dispute resolver and membership governance | Apply the pinned, signed policy to frozen evidence; rotate membership only under the required quorum. | Mutate history, bypass uniqueness/fencing checks, or use a dispute as an unrecorded payment override. |

The lease issuer and settlement sequencer may be operated by the same service in
the trusted pilot, but they remain separate logical authorities and must be
represented separately in audit evidence.

### One issuer and one sequencer per reservation

At every maturity phase, a reservation has exactly one valid lease issuer and
one valid settlement sequencer at a time. This is deliberately narrower than
giving every live coordinator the right to act.

The issuer must atomically record each lease against the reservation state and
its monotonically increasing fencing token before delivery. A worker result is
eligible only if it names the current or validly expired lease/fence under the
pinned policy. The sequencer is the only role that can append a canonical
acceptance or settlement intent, and it may do so once only after all required
artifact-promotion and verifier evidence is durable.

Each reservation has an append-only, signed causal event chain. A canonical
event includes its reservation identifier, event sequence, parent event digest,
membership epoch, authority binding, idempotency identifier and signed payload
digest. The normal causal order is:

```text
quote authorised -> reserved -> lease issued -> artifact candidate
-> verifier attestation(s) -> canonical acceptance
-> settlement intent -> financial finality or refund/close
```

The chain uses sequence and parent digest for ordering, not wall-clock arrival
time. Replaying the same event identifier and canonical content is a no-op;
reusing it with different content is corruption and freezes the reservation for
review. A later event cannot repair an invalid earlier event merely because it
arrived first.

On-chain settlement, if approved later, is a financial-finality record bound to
the sequenced settlement intent. It does not decide whether an artifact is
accepted, and an off-chain replay cannot silently overwrite confirmed on-chain
finality.

### Membership, event replication and authority handoff

A federation uses a signed `MembershipEpoch` record. It must identify the
network, activation/expiry window, role-specific public keys and key history,
operator and declared failure-domain identity, verifier eligibility,
quorum/threshold rules, policy digests and the signatures required to amend the
record. Multiple virtual machines, cloud accounts or keys controlled by one
operator do not count as independent members for quorum or evidence claims.

A reservation pins the epoch digest that selected its authorities. Membership
changes apply to new reservations by default; they cannot retroactively change
an open reservation's issuer, sequencer, verifier quorum or dispute policy.
Changing an open reservation requires a signed, ordered authority-handoff
record that includes the final canonical event digest, the next authority epoch
and fresh fencing epoch. The previous authority quorum and successor quorum
must both authorise that handoff after confirming no conflicting live lease or
pending canonical acceptance exists.

If the required handoff evidence cannot be formed, the reservation pauses and
eventually follows its committed expiry/refund path. It is never silently taken
over by a newly discovered peer.

Federated members replicate signed event records and artifact availability
evidence, but replication alone is not authority. A replica cannot issue a
lease or settlement merely because it has a copy of the log.

### Verification, canonical acceptance and disputes

A verifier attestation covers the same canonical candidate: reservation,
stage, attempt, lease/fence, input and output manifest digests, executor and
evaluation-policy digests, and verdict. It is evidence, not settlement.

The trusted pilot may use a pinned single-verifier policy (`q = 1`) while it is
within the existing trusted boundary. Its verifier credential must be distinct
from the producing worker and all decisions remain replayable.

Federated and public-provider phases require a policy-defined verifier quorum
of at least two operationally independent verifier operators. No producer may
supply a quorum for its own candidate, and no single underlying operator may
control the required quorum. The policy must also pin evaluator version and
independence constraints. The settlement sequencer may create canonical
acceptance only when the threshold signs the same candidate digest.

Conflicting candidate digests, equivocation by a verifier, a missing quorum or
a failed independence check produce `Inconclusive`/`Disputed`, never a
first-arrival winner. Such a reservation cannot settle. The dispute resolver
can append a policy-bound, signed resolution based on preserved evidence, but
cannot write a second settlement or rewrite historic events. Once financial
finality exists, a correction must use an explicit, auditable, contract-permitted
compensating path rather than a hidden reversal.

### Phase topology

| Phase | Membership and admission | Lease and settlement authority | Acceptance, dispute and availability posture |
| --- | --- | --- | --- |
| **Trusted pilot** | Invited providers and approved workloads. One MojoColib-operated control plane and trusted storage/verification boundary. | Each reservation names that control plane's issuer and sequencer; active/passive recovery may exist but is one authority. | `q = 1` verifier policy within the trusted boundary; operator is the signed dispute resolver. An outage fails closed for new authority actions. This phase must not claim federation or absence of a single settlement authority. |
| **Federated services** | A signed epoch lists independently operated coordinators, verifiers, relays and artifact replicas. Provider admission may remain invited. | A deterministic, reservation-pinned selection names one issuer and one sequencer; signed replicas and quorum-controlled handoff provide recovery. | At least two independent verifiers form the pinned quorum. Conflicts freeze settlement. Relay or single-operator loss must not make durable evidence or open reservations unrecoverable. |
| **Public-provider network** | Providers may apply under published capability, anti-Sybil, data-class, privacy and abuse rules. Public admission does not automatically grant coordinator, verifier or governance authority. | The same reservation-pinned issuer/sequencer and threshold-controlled handoff rules apply. A public worker cannot become a settlement authority merely by supplying GPU capacity. | Public disputes, key custody, reputation and emergency-pause governance are published and tested. Real-value payout remains separately gated on the V2 settlement decision, escrow security review and legal/operational approval. |

This is a progression in trust and operating evidence, not a promise that every
phase will be reached on a fixed date.

### Failover and partition behaviour

Safety wins over continuity where the two conflict.

- In the trusted pilot, loss of the issuer/sequencer halts new leases,
  acceptances and settlements until the durable state and the same authority
  are recovered. Existing workers may finish before their leases expire, but
  their results do not settle without normal verification and sequencing.
- In a federation, a successor needs a valid authority-handoff certificate,
  the highest canonical event sequence/digest and a higher authority/fencing
  epoch before it can issue or sequence anything. It must first reconcile the
  replicated event log and any pending artifact promotion.
- A partition, lost quorum, unknown highest event or disputed predecessor puts
  the reservation into a safe paused state. The fallback is expiry/refund under
  the reservation policy, not speculative reassignment or payment.
- A successor may not reissue a stage while an earlier lease can still be
  valid. It may retry only after expiry/fence rules and canonical state prove
  the earlier attempt cannot win.
- If an accepted result is waiting for financial finality, recovery reuses the
  same settlement intent and nonce. It must not create a new payable result.
- Relay failure removes reachability, not authority. A relay compromise is
  handled through key/grant revocation and alternate rendezvous, without giving
  the relay access to a settlement decision or artifact plaintext by default.

## Evidence gates for the "no single settlement authority" claim

MojoColib may describe the network as having no single settlement authority
only after all of the following are independently reproducible:

1. At least three operationally, economically and key-custody independent
   coordinator/settlement operators participate across distinct failure domains.
   The evidence identifies shared ownership, hosting, custody or governance
   relationships rather than treating separate machines as independent.
2. The active settlement threshold requires at least two independent operators,
   no one operator controls a sufficient threshold, and the same or stronger
   threshold protects membership changes, authority handoff, emergency pause
   and key rotation. A hidden operator-controlled recovery key invalidates this
   claim.
3. Every open reservation demonstrably pins its membership epoch, issuer,
   sequencer, verifier policy and event chain. Independent implementations can
   replay those records and derive the same eligibility and settlement intent.
4. Federated verifier quorum tests show that a producer cannot verify itself to
   a payout, a single verifier loss does not manufacture acceptance, and
   conflicting attestations freeze rather than settle a reservation.
5. Adversarial tests cover duplicate/reordered events, concurrent issuers,
   concurrent sequencer failover, stale fences, key rotation, partition/heal,
   artifact-promotion crash recovery, lost relay, replayed settlement intent
   and verifier equivocation. They demonstrate zero duplicate settlements and
   zero unreconciled/stranded reservations.
6. The trusted economic pilot's published workload and recovery gates are met:
   at least five independent providers, 50 complete pipelines, 500 attempted
   stages, at least 95% completion of valid stages after retries, and all 20
   forced checkpoint recoveries with accepted lineage. The separate pilot
   requirements for independent verification, fraud detection, cost and
   reconciliation also remain binding.
7. An independent operator can restore from the published backup/recovery
   procedure, join without receiving another operator's private key, and audit
   settlement history and dispute outcomes from signed evidence.

Meeting these gates means that no *single technical settlement operator* can
unilaterally issue final settlement under the enforced protocol. It does not
claim that infrastructure, law, governance, hosting, asset issuers or social
power have disappeared.

## Consequences

- The early product can prove useful work and provider economics with simple,
  auditable authority rather than pretending the current local slice is a P2P
  network.
- Federation introduces real operational cost: durable event replication,
  membership governance, key lifecycle management, quorum-aware verification,
  backup/restore, relay abuse controls and a dispute process are required
  product capabilities, not marketing features.
- The reservation-scoped binding prevents availability improvements from
  turning into permission to issue duplicate leases or settlements.
- Worker-provider openness is decoupled from control-plane openness. This lets
  MojoColib support older and heterogeneous hardware through evidence-based
  admission without handing every machine financial authority.
- The production coordinator and remote worker service remain the next
  implementation milestone. Peer discovery, public admission and a Solana
  program are later work and must not be represented as current functionality.

## Rejected alternatives

- **Start with a fully permissionless peer mesh.** Rejected because peer
  discovery does not establish unique lease, acceptance or settlement authority
  and would obscure the pilot's security boundary.
- **Let every reachable coordinator issue a lease or settlement.** Rejected
  because replication lag and partitions would create valid-looking duplicate
  work and financial claims.
- **Use wall-clock order or first arrival to resolve conflicts.** Rejected
  because clocks, relays and partitions are not a trustworthy source of
  canonical state; signed causal order and explicit dispute states are needed.
- **Treat verifier plurality as acceptance.** Rejected because incompatible
  candidates or verifier equivocation need a pinned threshold, identity and
  independence policy before a result can be paid.
- **Put all progress events on-chain.** Rejected because the chain is not a
  practical scheduler, heartbeat transport or ML verifier. It is considered
  later only for bounded financial finality.
- **Call the target serverless.** Rejected because verified distributed work
  still needs accountable coordination, storage, recovery and settlement
  services even when no single operator owns all of them.

## Related records

- [ADR-0005: Fixed microcredit ledger units](0005-ledger-units.md)
- [ADR-0006: No automatic payment for unaccepted partial checkpoints](0006-checkpoint-compensation.md)
- [MVP threat model](../threat-model.md)
- [Training marketplace PRD](../mojocolib-training-prd.md)
