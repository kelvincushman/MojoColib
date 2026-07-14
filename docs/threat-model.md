# MojoColib MVP threat model

## Scope and trust boundary

Customers, providers and executors are mutually untrusted. The coordinator/operator, PostgreSQL transaction boundary, object-store promotion operation and approved verification workers are trusted for the MVP but audited. Provider machines are invited rather than anonymous. The V2 foundation now includes a local signed, single-reservation journal, but a production coordinator, financial rail, Solana integration and public participation remain outside the core trust boundary.

The Rust worker is the machine-side security boundary. Training/Mojo/Colibri code is untrusted executor content. Large bytes travel directly between executors and object storage; the coordinator handles only signed manifests and short-lived operation grants.

## Data classes

| Class | MVP handling |
| --- | --- |
| Public | May run on any admitted compatible provider; still content-addressed and least-retained. |
| Licensed | Provider and region policy must permit it; licence/provenance manifest required. |
| Confidential | Trusted-provider pilot only, encrypted in transit/at rest, no shared cache, shortest retention. |
| Secret | Identity keys, settlement keys, database credentials and presigned operations; never enter pipeline specs, executor environments, logs, metrics or artifacts. |

Personal, health, payment-card, export-controlled and other regulated training data are prohibited in the MVP. A dataset that cannot be confidently classified is rejected.

## Priority threats and controls

| Threat | Principal controls | Verification |
| --- | --- | --- |
| Malicious template escapes executor | Signed digest allowlist; rootless Linux isolation; dedicated trusted macOS policy; no worker keys; deny egress | Escape, filesystem, key, process and network tests |
| Stolen/replayed worker identity | Per-worker Ed25519 key, signed challenge, key rotation, outbound mTLS session, expiring capabilities | Retired-key and replay tests |
| Stale worker promotes valid output | Renewable 60-second lease, monotonically increasing fencing token, attempt-temporary key | Expiry/race tests must reject stale fence |
| Pipeline/schema resource exhaustion | Size/depth/fan-out/resource bounds before funding | Property tests around every boundary |
| Ambiguous or parser-dependent signed JSON | One strict entry point; reject duplicate/unknown fields, floats and trailing data before validation/signature checks | Adversarial strict-JSON and signature tests |
| Artifact substitution or lineage fraud | SHA-256 content address, signed canonical manifest, input/environment lineage, atomic promotion | Tamper/corruption corpus |
| Incompatible/corrupt checkpoint | Compatibility commitment before accelerator work; shape/type/finiteness and required-state checks | Resume and corruption fixtures |
| Provider fabricates learning | Independent evaluation, hidden canary/duplicate option, producer/verifier separation, preserved rejection reasons | Known-fraud corpus; no claim of cryptographic proof |
| Duplicate/over settlement | Fixed integers, pre-reservation, settlement-bound customer authorization/receipt/nonce, idempotent signed event content and replay reconciliation | Duplicate/reordered event and signed-journal tests; global cross-reservation uniqueness remains a rail gate |
| Chain timeout or finality ambiguity | Quote-bound settlement intent, no payout on submission alone, a distinct signed rail attestation over the settlement digest/nonce/reference, reconciliation hold before retry, no off-chain overwrite of finality | Finality/retry/reconciliation and signed-journal recovery tests before any rail integration |
| Coordinator leaks large/private data | Direct object-store transfer; manifests only; redaction; no artifact bodies in API handlers | Traffic/body-size integration test |
| Dependency/runtime supply chain | Immutable revisions/digests, licence inventory, SBOM, signed templates, Mojo BYO-runtime posture | Release inventory and offline-load tests |
| NVMe model/data leakage | Per-attempt directories, encrypted confidential storage, no secret material, cleanup policy | Recovery/cleanup and removed-drive tests |
| Denial by provider disconnect | Checkpoint intervals, lease expiry, bounded retries, reliability scoring | Twenty fixed interruption scenarios |

## Security invariants

- A rejected command leaves state unchanged and appends no successful transition.
- Untrusted signed JSON reaches contract validation and signature verification only through the strict parser; no boundary may silently collapse duplicate keys or ignore unknown fields.
- Current state and its audit event commit in one database transaction.
- Executor output is never accepted from an expired lease or stale fence.
- Downstream stages consume accepted manifests only.
- Verification acceptance and ledger settlement are separate decisions; rejected/inconclusive work cannot settle.
- A V2 accepted artifact can create an eligible settlement intent but cannot by itself create financial finality; a confirmed financial record cannot be silently replaced by off-chain replay.
- The public V2 coordinator mutation boundary accepts only a configured system-sequencer signature, a customer signature for funding authorization, and a distinct rail-attestor signature for finality or a retry decision. It resolves the latter two from a reservation-pinned authority set rather than a per-event caller witness, and rechecks all required signatures during replay.
- Lifecycle observations are bound to the signed outer event and supplied coordinator admission time, cannot move backwards, and cannot cause a future expiry or post-expiry acceptance when that trusted-pilot caller supplies its real clock. A production boundary must own and persist that clock rather than accept a raw scalar from an untrusted API caller.
- The local V2 journal is one reservation stream only; its signed replay protection and in-memory authority set do not substitute for future global database uniqueness, custody, durable registry/key rotation or refund controls.
- No manual control bypasses idempotency, artifact commitments or settlement uniqueness.

## Pilot stop conditions

Any critical executor escape, worker-key exposure, accepted stale-fence artifact, known corrupt artifact accepted, duplicate settlement, unreconciled credit loss, or confidential-data leak stops the pilot. Threat-model review repeats before anonymous providers, general provider-owned Macs, WAN-distributed training, money-like credits or a public binary runtime release.
