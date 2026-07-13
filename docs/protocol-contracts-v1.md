# MojoColib V1 protocol contract inventory

This document names the public Rust contract surface currently implemented in `crates/protocol`. It is an implementation inventory, not a claim that the coordinator, worker, training executor or marketplace services exist.

Every versioned top-level V1 contract uses `schema_version = 1`. The types in the signed-contract table implement `SignedContract` and travel inside `SignedEnvelopeV1<T>`. Incoming untrusted signed JSON must use `strict_json::parse_signed_envelope`; see [ADR-0002](adr/0002-canonical-encoding.md).

## Signed contract types

| Rust type | `contract_type` | Purpose |
| --- | --- | --- |
| `PipelineSpecV1` | `pipeline_spec_v1` | Bounded typed stage DAG and customer work request |
| `TemplateManifestV1` | `template_manifest_v1` | Operator-approved runtime, platform, resource, network and verification policy |
| `StageLeaseV1` | `stage_lease_v1` | Time-bounded attempt assignment with fencing token |
| `LeaseRenewalV1` | `lease_renewal_v1` | Monotonic lease extension for the same attempt/fence |
| `ExecutorRequestV1` | `executor_request_v1` | Bounded request passed to an isolated executor |
| `ArtifactManifestV1` | `artifact_manifest_v1` | Content-addressed output, retention and lineage commitment |
| `CheckpointManifestV1` | `checkpoint_manifest_v1` | Checkpoint artifact plus resume-compatibility commitment |
| `WorkerCapabilityV1` | `worker_capability_v1` | Expiring worker/runtime/hardware capability statement; alias `ProviderCapabilitiesV1` |
| `BenchmarkResultV1` | `benchmark_result_v1` | Expiring fixed-integer admission benchmark evidence |
| `QuoteV1` | `quote_v1` | Maximum execution and verification reservation quote |
| `AuditEventV1` | `audit_event_v1` | Actor-bound event commitment for an audited entity |

## Embedded and result contracts

| Module | Public contract/support types |
| --- | --- |
| Pipeline | `StageSpecV1`, `ResourceRequirementsV1`, `CheckpointPolicyV1`, `RuntimeKind`, `ArtifactType`, `DataClass` |
| Template | `TemplateManifestV1` |
| Lease | `FencingTokenV1`, `StageLeaseV1`, `LeaseRenewalV1` |
| Executor | `NetworkPolicy`, `ExecutorRequestV1`, `ExecutorEventV1`, `ProgressEventV1`, `CompletionStatus`, `ExecutorCompletionV1`, `CompletionSummaryV1` |
| Artifact/checkpoint | `RetentionClass`, `ArtifactLineageV1`, `ArtifactManifestV1`, `CheckpointCompatibilityV1`, `CheckpointManifestV1` |
| Capability/benchmark | `AcceleratorVendor`, `AcceleratorCapabilityV1`, `WorkerCapabilityV1`, `ProviderCapabilitiesV1`, `BenchmarkResultV1` |
| Verification | `VerificationPolicyV1`, `VerificationOutcome`, `VerificationDecisionV1` |
| Ledger | `Microcredits`, `QuoteV1`, `ReservationV1`, `SettlementV1`, `StageSettlementV1` |
| Audit | `ActorKind`, `AuditedEntityKind`, `AuditEventV1` |
| Signing/digest | `Sha256Digest`, `SignedContract`, `SignerKind`, `TrustedEd25519KeyV1`, `Ed25519SignatureV1`, `SignedEnvelopeV1<T>` |

The validated identifier family is: `PipelineId`, `StageId`, `AttemptId`, `ProviderId`, `WorkerId`, `CustomerId`, `ArtifactId`, `KeyId`, `QuoteId`, `ReservationId`, `SettlementId`, `TemplateId`, `LedgerEventId`, `AuditEventId`, `ActorId`, `MessageId` and `BenchmarkId`.

## State-machine contracts

`crates/state-machine` currently implements in-memory, deterministic reducers over the protocol types:

- pipeline: `PipelinePhase`, `PipelineEvent`, `PipelineMachine`;
- stage: `StagePhase`, `AttemptBinding`, `StageEvent`, `StageMachine`; and
- ledger: `ArtifactAcceptance`, `LedgerEvent`, `ApplyOutcome`, `LedgerMachine`.

Pipeline and stage events enter through replay-safe `EventEnvelope<T>` values with stable `MessageId` identities. Exact retries return `ApplyOutcome::DuplicateIgnored`; reusing an ID for different canonical content is rejected without mutation.

`crates/scheduler` adds `SchedulerPolicyV1`, `CandidateV1` and `AssignmentV1`. It applies versioned compatibility, resource, availability, reliability and fixed-integer budget filters before an order-independent deterministic tie-break. It deliberately has no cost-scoring heuristic yet.

These reducers prove transition, replay and accounting invariants in unit/property tests. They do not yet provide persistence, transactions, authentication, networking or process supervision.

## Current verification and remaining work

Rust tests cover contract bounds, canonical commitments, signatures, strict signed-JSON parsing and state-machine behavior. The shared Rust/Python golden corpus is an early positive corpus, not complete RFC 8785 conformance evidence.

Still required before these contracts can be used across a trust boundary:

- complete duplicate-key, Unicode, numeric-boundary and negative golden cases across every implementation language;
- generated API/schema artifacts and contract tests;
- key-registry, expiry and replay enforcement in the coordinator/worker services;
- persistence transactions connecting accepted transitions and audit events; and
- executor, artifact-store and cross-worker checkpoint/resume integration tests.
