# MojoColib — training and ML pipeline marketplace PRD

Status: **Draft for implementation planning**
Version: 0.1
Date: 2026-07-12
Product owner: Project founder
Technical owner: Lead Rust systems engineer
Primary reference: `docs/decentralized-ml-training-marketplace-mvp.md`
Implementation plan: `docs/plans/mojocolib-training-mvp-plan.md`

## 1. Product decision

MojoColib will prove that useful, checkpointable machine-learning work can be scheduled across a small trusted network of heterogeneous computers and paid for according to accepted results.

The launch workflow is one reproducible, eight-trial LoRA fine-tuning sweep:

```text
prepare a pinned public dataset
  -> run eight independent LoRA trials
  -> checkpoint every trial
  -> interrupt and reassign selected trials
  -> evaluate all outputs under one versioned policy
  -> verify the winner on a different provider
  -> publish the accepted adapter and provenance
  -> reconcile test-credit payments exactly
```

This is the core MVP. Dataset preparation is its prerequisite. Quantization, Colibri inference, Mojo kernels and Solana settlement are separately gated extensions and must not block the core result.

## 2. Language and runtime decision

PyTorch is **not** 99% Rust. Current official repository statistics show roughly 64% Python, 28% C++, 2.6% CUDA and no Rust entry. PyTorch describes itself as Python-first, with a substantial C++ tensor, autograd and runtime core. GitHub's figures measure repository bytes rather than runtime work, but they decisively rule out a Rust rewrite. See the [official language statistics](https://api.github.com/repos/pytorch/pytorch/languages), [PyTorch repository](https://github.com/pytorch/pytorch#readme) and [PyTorch C++ API](https://docs.pytorch.org/cppdocs/).

The likely source of the 99%-Rust figure is [Burn](https://github.com/tracel-ai/burn), a separate Rust-native framework with PyTorch-like ergonomics. [Candle](https://github.com/huggingface/candle) is another Rust-native ML framework, while [tch-rs](https://github.com/LaurentMazare/tch-rs) wraps PyTorch's C++ LibTorch runtime rather than replacing it.

The MVP therefore uses each language where it reduces risk:

| Plane | MVP technology | Responsibility |
| --- | --- | --- |
| Control and trust | Rust | Coordinator, worker, protocol, scheduling, manifests, metering, audit and ledger |
| Training | Python and PyTorch | Autograd, optimizers, PEFT/LoRA, checkpoint state and evaluation |
| Artifacts | S3-compatible storage | Datasets, checkpoints, metrics, manifests and outputs |
| State | PostgreSQL | Pipelines, leases, events, identities and off-chain ledger |
| Kernel/inference extension | Mojo and Colibri | Quantization, portable kernel experiments and NVMe-backed evaluation |
| Settlement extension | Rust and Solana/Anchor | Localnet/devnet escrow mirroring after the off-chain ledger is exact |

The executor contract must be runtime-neutral. A later `burn-train-canary-v1` or Candle template may run as a signed native binary without changing the coordinator, worker or artifact protocol. The Rust-native training spike is experimental and does not replace the PyTorch baseline until it passes the same correctness, hardware and checkpoint tests.

## 3. Problem

Small ML teams and independent developers often need many modest experiments rather than one giant training run. Their work includes data preparation, LoRA fine-tuning, evaluation, quantization and deployment benchmarking. These jobs are frequently queued, manually moved between machines and difficult to reproduce.

At the same time, gaming PCs, workstations, Apple Silicon machines and older accelerators sit idle. Existing compute markets mainly expose generic GPU rentals or modern NVIDIA capacity. They do not make checkpoint recovery, cross-provider verification, artifact lineage and power-adjusted provider economics the product boundary.

The product hypothesis is:

> If ML work is divided at checkpointable stage and independent-trial boundaries, a heterogeneous provider network can complete useful experiments reliably without pretending that residential GPUs form one low-latency supercomputer.

## 4. Why now

- LoRA and other parameter-efficient methods reduce the amount of trainable state and the size of useful output artifacts.
- Training sweeps naturally fan out into independent jobs.
- Consumer machines increasingly have usable local accelerators and fast NVMe storage.
- Content-addressed artifacts and signed manifests make lineage auditable.
- Rust is suitable for a small, secure control plane, while PyTorch supplies the mature training ecosystem.
- Mojo creates a plausible future path for shared accelerator source across CUDA, HIP and Metal, but must earn its place through measured kernels rather than claims.

## 5. Users and jobs to be done

### 5.1 Pipeline author

- Trigger: needs to fine-tune or evaluate a small model without manually managing several machines.
- Job: submit one pinned, reproducible pipeline and receive a verified artifact plus a complete cost and provenance report.
- Minimum trust evidence: exact model/data/runtime revisions, visible stage states, signed provider receipts and independent final evaluation.
- Unacceptable failure: a checkpoint cannot be resumed, results cannot be reproduced, or a rejected artifact is treated as accepted.
- Success measure: the reference sweep completes with no manual worker intervention.

### 5.2 Compute provider

- Trigger: has an idle compatible computer and a defined availability/power window.
- Job: enrol it safely, accept bounded jobs and receive credits only for accepted work.
- Minimum trust evidence: signed templates, declared resource bounds, no inbound port exposure, predictable storage use and a local stop/drain control.
- Unacceptable failure: customer code reaches wallet keys, personal files or unrestricted network access; a job continues past the declared window.
- Success measure: accepted-stage earnings are projected above incremental electricity and storage costs.

### 5.3 Marketplace operator

- Trigger: operates the trusted pilot and approves participants and templates.
- Job: admit providers, inspect failures, quarantine bad actors, version policies and reconcile every accepted stage.
- Minimum trust evidence: append-only events, deterministic state reconstruction, signed artifacts and an actionable audit trail.
- Unacceptable failure: double settlement, stranded reservations, silent protocol drift or an untraceable decision.
- Success measure: all pilot balances and artifacts reconcile from the event log.

### 5.4 Verification operator

- Trigger: a training stage or final pipeline artifact is submitted.
- Job: apply the template's declared checks and independently evaluate selected outputs.
- Minimum trust evidence: hidden fixtures, canonical manifests, bounded scripts and provider separation.
- Unacceptable failure: general training loss is treated as sufficient proof, or a known corrupt/replayed artifact passes.
- Success measure: the fixed adversarial corpus meets its detection and false-rejection targets.

## 6. MVP outcome and success measures

The MVP is successful when all of the following are demonstrated in one controlled pilot:

| Measure | Target |
| --- | --- |
| Trusted providers | 5–10 enrolled machines |
| Hardware diversity | At least 3 configurations and at least 2 accelerator backend families |
| Pilot volume | At least 50 complete pipelines and 500 attempted stages |
| Valid-stage completion | At least 95% after automatic retries |
| Reference sweep | Eight trials across at least 3 providers |
| Forced recovery | 20/20 interruptions resume from the last promoted checkpoint |
| Lost work | No more than the declared checkpoint interval; default maximum 10 minutes |
| Independent verification | Winning adapter loads and evaluates on a different provider |
| Artifact provenance | 100% of accepted artifacts pass manifest, lineage, signature, hash and type checks |
| Known fraud/corruption corpus | At least 95/100 detected, with at most 5/100 honest fixtures falsely rejected |
| Verification overhead | At most 15% of total useful compute |
| Ledger integrity | Zero double credits and zero stranded reservations after replay/restart tests |
| Data policy | Zero unapproved dataset manifest IDs admitted; every admitted manifest carries an allowed declared data class and review record |

Metric definitions:

- A **valid stage** has passed schema, template, licence/data-policy and resource validation and had at least one compatible admitted provider during its scheduling window. The completion-rate denominator includes injected failures, provider disconnects, lease expiries and stages that exhaust retries. It excludes customer cancellation before first lease, policy rejection and stages for which no compatible provider existed; each excluded class is reported separately. The numerator is valid stages eventually accepted.
- **Useful compute** is benchmark-normalised reference-equivalent compute-seconds consumed by accepted non-verification stage attempts.
- **Verification compute** uses the same normalisation and includes duplicate runs, hidden/canary evaluation, second-provider evaluation and failed verifier attempts.
- **Verification overhead** is `verification_compute / useful_compute`; it must be no greater than 0.15 for the pilot.
- Data-policy admission proves that an approved immutable manifest and declared class were used. It does not guarantee that a nominally public dataset contains no personal or regulated content.

Economics is a go/no-go measure, not a hard technical completion condition:

- record the timestamped median price from at least two comparable hosted alternatives for the pinned reference workload;
- project each provider's incremental electricity and storage cost from measured duration and declared tariff;
- target customer cost no greater than 1.2 times the hosted median during the pilot;
- target median provider gross margin of at least 20% above estimated incremental cost without token subsidy.

## 7. Scope prioritisation

### 7.1 Must ship — core MVP

- Invitation-only customer, provider and operator identities.
- One immutable, versioned pipeline DAG format.
- Three signed templates: dataset preparation, PyTorch/PEFT LoRA training and evaluation.
- Rust coordinator and outbound-only Rust worker.
- Provider capability inventory and admission benchmark.
- Stage leases, fencing, retries and deterministic state transitions.
- S3-compatible artifact transfer and content-addressed manifests.
- Atomic checkpoint promotion and cross-worker resume.
- Eight-trial sweep fan-out/fan-in.
- Versioned evaluation and second-provider winner verification.
- Fixed-integer quotes, reservations and off-chain test-credit ledger.
- Audit events, metrics, logs and operator quarantine controls.
- Linux NVIDIA and Apple Silicon worker paths for the first complete pilot.

### 7.2 Should ship — pilot extensions

- Linux AMD/ROCm worker qualification when compatible hardware is available.
- Windows 11 through a constrained WSL2 worker package.
- Provider-local multi-GPU training treated as one resource.
- Quantization/export stage.
- Colibri evaluation and NVMe streaming benchmark.
- Mojo backend benchmark on at least Apple and one discrete GPU target.
- Solana localnet and then devnet ledger mirroring.
- Provider fleet view for an operator managing multiple machines.

### 7.3 Could ship — research follow-ups

- Burn or Candle native-training canary using the same executor contract.
- QLoRA hardware tier.
- AMD and NVIDIA vendor-specific Mojo optimisation branches.
- Synthetic-data generation pipeline.
- Reputation-weighted anonymous admission.
- Decentralised scheduling or verification.

### 7.4 Explicit non-goals

- Training a foundation model from scratch.
- DDP, FSDP, tensor parallelism or gradient exchange across unrelated homes.
- Anonymous public providers.
- Arbitrary customer containers, shell commands or unreviewed Python.
- Private, proprietary, personal or regulated training data.
- Confidential computing guarantees.
- Real-value payments, token issuance or mainnet Solana.
- Retail PlayStation, Xbox or Coral Edge TPU execution.
- A universal Mojo binary or a claim that Mojo removes vendor drivers.
- Replacing PyTorch before a Rust-native alternative passes the same acceptance suite.

## 8. Reference product journey

1. The operator approves a provider identity and one machine.
2. The worker inventories CPU, RAM, accelerator, backend, storage, network and power-policy capabilities.
3. The coordinator issues a signed benchmark challenge and records a versioned capability result.
4. The customer submits the pinned LoRA sweep and requests validation.
5. The coordinator rejects invalid DAGs, unapproved templates, unsupported data classes and unaffordable stages before funding.
6. The customer funds an off-chain test-credit budget.
7. The scheduler leases runnable stages to compatible providers.
8. Workers fetch only signed template and content-addressed input artifacts.
9. Training templates periodically produce complete checkpoints containing adapter, optimizer, scheduler and RNG state.
10. The worker uploads to an attempt-specific temporary key; the coordinator validates and atomically promotes it.
11. At least 20 pilot attempts are deliberately interrupted. Selected stages resume on another compatible provider.
12. All eight trials complete or exhaust their fixed retry budgets.
13. Evaluation stages apply one versioned policy and rank the trials.
14. The winning adapter is evaluated on a provider that did not produce it.
15. Accepted artifacts, metrics, provenance and an economics report are published to the customer.
16. The ledger settles accepted stages once and releases unused reservations.

## 9. Functional requirements

`P0` means required for the core MVP. `P1` means pilot extension. `P2` means research follow-up.

### 9.1 Identity and provider lifecycle

- **FR-ID-001 (P0):** The system MUST support distinct customer, provider, worker and operator identities.
- **FR-ID-002 (P0):** A worker MUST use a unique Ed25519 keypair and MUST NOT use the provider's wallet or settlement key.
- **FR-ID-003 (P0):** Provider enrolment MUST require an operator invitation, signed challenge and recorded machine fingerprint.
- **FR-ID-004 (P0):** The operator MUST be able to suspend, quarantine, reinstate and retire a provider or individual machine.
- **FR-ID-005 (P0):** Key rotation MUST preserve historical signature verification and invalidate new sessions from the retired key.
- **FR-ID-006 (P0):** Workers MUST maintain an outbound authenticated session; the MVP MUST NOT require inbound provider ports.

### 9.2 Capability discovery and admission

- **FR-CAP-001 (P0):** Workers MUST report OS, architecture, CPU, RAM, accelerator, backend/driver, VRAM or unified memory, available storage and supported precisions.
- **FR-CAP-002 (P0):** Capabilities MUST be backed by a signed, versioned benchmark result before the worker receives training stages.
- **FR-CAP-003 (P0):** Workers MUST declare availability, maximum temperature policy, power limit where supported and local drain windows.
- **FR-CAP-004 (P0):** The coordinator MUST expire capability records after a runtime, driver or material hardware change.
- **FR-CAP-005 (P1):** Legacy devices MUST be admitted by passing template fixtures, not by product name alone.

### 9.3 Pipeline authoring and validation

- **FR-PIPE-001 (P0):** A pipeline MUST be an immutable DAG with a canonical `PipelineSpecV1` hash.
- **FR-PIPE-002 (P0):** Every stage MUST reference an approved template digest, typed input manifests, expected output types, resource bounds, retry count, quote cap, checkpoint policy and verification policy.
- **FR-PIPE-003 (P0):** Validation MUST reject cycles, missing dependencies, duplicate IDs, unsupported schema versions and resource values outside configured limits.
- **FR-PIPE-004 (P0):** The core MVP MUST cap a pipeline at 100 stages, fan-out at 32 and dependency depth at 20.
- **FR-PIPE-005 (P0):** A funded pipeline's canonical spec MUST NOT change. Amendments MUST create a new pipeline/version.
- **FR-PIPE-006 (P0):** Cancellation MUST stop unscheduled stages, request drain/checkpoint from running stages and release unused reservations.
- **FR-PIPE-007 (P0):** Downstream stages MUST consume only accepted artifact manifests.

### 9.4 Template registry and execution

- **FR-TPL-001 (P0):** Only operator-signed template digests MAY execute.
- **FR-TPL-002 (P0):** Each template MUST declare supported platforms/backends, parameter schema, artifact contract, network policy, resource limits and verification policy.
- **FR-TPL-003 (P0):** The initial registry MUST contain `dataset-prepare-v1`, `hf-peft-sft-v1` and `hf-evaluate-v1`.
- **FR-TPL-004 (P0):** Training MUST record model, tokenizer, dataset, code and environment revisions plus all available random seeds.
- **FR-TPL-005 (P0):** Training MUST checkpoint adapter, optimizer, scheduler, step/epoch counters and RNG state.
- **FR-TPL-006 (P0):** A template MUST respond to graceful drain and cancellation within 30 seconds, excluding an already-running atomic artifact write.
- **FR-TPL-007 (P0):** Network egress MUST be disabled by default and permitted only through template-specific allowlists.
- **FR-TPL-008 (P1):** A native executor template MAY run a signed Rust binary under the same manifest and resource contract.

### 9.5 Scheduling, leases and retries

- **FR-SCH-001 (P0):** Scheduling MUST first apply hard compatibility, policy, availability, reliability and budget filters.
- **FR-SCH-002 (P0):** The scheduler MUST store a `scheduler_policy_version` and all normalized inputs used for every assignment.
- **FR-SCH-003 (P0):** Equivalent scores MUST use a deterministic tie-breaker based on provider ID and stage ID.
- **FR-SCH-004 (P0):** A lease MUST include pipeline, stage, attempt, worker, template, input-manifest, expiry and fencing-token commitments.
- **FR-SCH-005 (P0):** Workers MUST heartbeat every 10 seconds. Three missed heartbeats MUST mark the worker unavailable.
- **FR-SCH-006 (P0):** Active leases MUST renew at least every 20 seconds and expire after 60 seconds without a valid renewal.
- **FR-SCH-007 (P0):** Outputs signed with a stale fencing token MUST be rejected even if the underlying computation was valid.
- **FR-SCH-008 (P0):** Retries MUST never exceed the funded retry budget.
- **FR-SCH-009 (P0):** Independent sweep trials SHOULD be spread across at least three providers when compatible capacity exists.

### 9.6 Artifacts and checkpoints

- **FR-ART-001 (P0):** Large artifacts MUST move directly between workers and object storage through short-lived presigned operations.
- **FR-ART-002 (P0):** Each artifact MUST include SHA-256, byte length, logical type, pipeline/stage/attempt lineage, input hashes, environment digest, provider signature, data class and retention policy.
- **FR-ART-003 (P0):** Uploads MUST use attempt-specific temporary keys and MUST become visible as accepted artifacts only through atomic promotion.
- **FR-ART-004 (P0):** Checkpoint promotion MUST be idempotent.
- **FR-ART-005 (P0):** A checkpoint MUST carry a compatibility key covering template, model, tokenizer, data manifest, parameter schema and checkpoint format versions.
- **FR-ART-006 (P0):** Resume MUST reject an incompatible checkpoint before accelerator work starts.
- **FR-ART-007 (P0):** The default training template MUST checkpoint at least every 50 optimizer steps or 10 minutes, whichever occurs first.
- **FR-ART-008 (P0):** Failed temporary uploads MUST be deleted within 24 hours; accepted pilot artifacts MUST be retained for at least 90 days.

### 9.7 Verification and reproducibility

- **FR-VER-001 (P0):** Artifact hashes alone MUST NOT qualify a training stage for acceptance.
- **FR-VER-002 (P0):** The training verifier MUST check checkpoint load, shape/type schema, finite values, permitted adapter modules, example/step counts, metric trace and lineage.
- **FR-VER-003 (P0):** Evaluation MUST use a versioned policy with a primary metric, guardrails, tolerances and deterministic ranking tie-breaker.
- **FR-VER-004 (P0):** The reference sweep's final winner MUST be evaluated on a compatible worker operated by a different provider. If no such provider is available, the pilot does not pass.
- **FR-VER-005 (P0):** The operator MUST be able to require duplicate execution or a hidden canary for a selected attempt.
- **FR-VER-006 (P0):** The system MUST preserve rejection reasons and MUST NOT settle a rejected attempt.
- **FR-VER-007 (P0):** Known corruption/fraud tests MUST use a versioned corpus so detection claims are reproducible.
- **FR-VER-008 (P0):** The product MUST describe verification as trusted and probabilistic; it MUST NOT claim cryptographic proof of learning.

### 9.8 Pricing and off-chain ledger

- **FR-LED-001 (P0):** Quotes, reservations, fees and credits MUST use signed 64-bit integer microcredits; floating-point money is prohibited.
- **FR-LED-002 (P0):** Funding MUST reserve the maximum accepted quote plus declared verification reserve before scheduling.
- **FR-LED-003 (P0):** A stage settlement MUST commit to pipeline, stage, accepted attempt, artifact manifest, provider, quote and verification decision.
- **FR-LED-004 (P0):** Settlement MUST be idempotent and unique by accepted stage ID.
- **FR-LED-005 (P0):** Pipeline cancellation or completion MUST release every unused reservation.
- **FR-LED-006 (P0):** Replaying the append-only event stream MUST reproduce exactly the same balances and reservation states.
- **FR-LED-007 (P0):** The provider report MUST separate measured execution, estimated energy, tariff assumption, projected cost and projected margin.
- **FR-LED-008 (P0):** The MVP MUST use valueless test credits only.
- **FR-LED-009 (P1):** Solana localnet/devnet MAY mirror an already-final off-chain settlement; chain availability MUST NOT decide training acceptance.

### 9.9 Operator controls and audit

- **FR-OPS-001 (P0):** Every material state transition MUST append an immutable audit event in the same database transaction as the current state update.
- **FR-OPS-002 (P0):** Operators MUST be able to inspect pipeline, stage, attempt, lease, checkpoint, verification and ledger histories.
- **FR-OPS-003 (P0):** Operators MUST be able to pause new assignments globally or by template, provider or backend.
- **FR-OPS-004 (P0):** Manual acceptance or rejection MUST require a reason and operator identity and MUST remain visible in the audit trail.
- **FR-OPS-005 (P0):** No manual operation MAY bypass idempotency or create a second settlement.

### 9.10 Mojo, Colibri and Rust-native extensions

- **FR-EXT-001 (P1):** Colibri evaluation MUST consume only accepted checkpoint lineage.
- **FR-EXT-002 (P1):** Mojo kernels MUST be compared with a CPU or established backend reference before performance results are published.
- **FR-EXT-003 (P1):** Vendor-targeted Mojo builds MUST report compiler, runtime, driver and accelerator target versions.
- **FR-EXT-004 (P1):** A Mojo failure MUST fail only the extension stage and MUST NOT corrupt an accepted training artifact.
- **FR-EXT-005 (P2):** Burn/Candle experiments MUST run the same tiny-model correctness, interruption, checkpoint and evaluation fixtures as PyTorch.

## 10. Non-functional requirements

### 10.1 Reliability and recovery

- **NFR-REL-001:** Valid core stages MUST achieve at least 95% completion after automatic retries over the pilot.
- **NFR-REL-002:** A coordinator restart MUST recover scheduling within 10 minutes and MUST not lose an accepted event or create a duplicate lease/settlement.
- **NFR-REL-003:** State mutation and its audit event MUST commit atomically in PostgreSQL.
- **NFR-REL-004:** The object store MAY be temporarily unavailable without losing the last promoted checkpoint.
- **NFR-REL-005:** Twenty fixed interruption tests MUST each resume from the latest promoted checkpoint with no more than one declared interval of lost work.

### 10.2 Performance and capacity

- **NFR-PERF-001:** Non-artifact API endpoints MUST respond within 500 ms at p95 under 20 requests/second in the pilot environment.
- **NFR-PERF-002:** A ready stage with an idle compatible worker MUST receive a lease offer within 10 seconds at p95.
- **NFR-PERF-003:** The core coordinator MUST support 25 registered workers, 10 connected/active workers, 20 concurrent stages and 100-stage pipelines.
- **NFR-PERF-004:** Artifact payloads MUST bypass the coordinator process.
- **NFR-PERF-005:** The core artifact contract MUST support individual artifacts up to 20 GiB and checkpoints up to 5 GiB.

### 10.3 Security and isolation

- **NFR-SEC-001:** All network sessions MUST use authenticated TLS.
- **NFR-SEC-002:** Identity and wallet/settlement keys MUST be separate.
- **NFR-SEC-003:** Linux GPU jobs MUST run rootless with bounded CPU, RAM, GPU visibility, storage, process count and wall time.
- **NFR-SEC-004:** Apple jobs MUST use a signed locked environment and bounded subprocess on a dedicated project-owned machine or OS account containing no personal files and exposing no worker identity key to the executor. Filesystem, keychain, process and network enforcement tests MUST pass before a general provider-owned Mac is admitted. The product MUST disclose the weaker isolation compared with Linux containers.
- **NFR-SEC-005:** Secrets MUST never be included in pipeline specs, logs, metrics or artifacts.
- **NFR-SEC-006:** Dependencies, images, models and datasets MUST be pinned by immutable digest or revision.
- **NFR-SEC-007:** No critical finding in executor escape, signature validation, artifact replay or ledger replay MAY remain open at pilot launch.

### 10.4 Reproducibility

- **NFR-REP-001:** Every accepted artifact MUST trace to immutable model, tokenizer, dataset, template and input-manifest revisions.
- **NFR-REP-002:** Deterministic fixtures MUST reproduce bit-for-bit where the backend promises determinism; other checks MUST declare numerical tolerances.
- **NFR-REP-003:** Scheduler and evaluation policies MUST be versioned and reconstructable.
- **NFR-REP-004:** Published benchmark results MUST include hardware, software, thermal, power and storage context.

### 10.5 Observability and retention

- **NFR-OBS-001:** Pipeline, stage and attempt IDs MUST propagate through coordinator, worker, executor, artifact, verification and ledger logs.
- **NFR-OBS-002:** Logs and operational metrics MUST be retained for at least 30 days during the pilot.
- **NFR-OBS-003:** Dataset rows, prompts and raw model outputs MUST be excluded from default logs.
- **NFR-OBS-004:** The operator MUST see queue time, assignment latency, stage duration, retries, lost work, checkpoint time, transfer time, cache hits, verification mismatches and ledger reconciliation.

## 11. Product architecture

```text
Customer CLI/SDK
      |
      v
Rust coordinator/API ---- PostgreSQL event/state/ledger
      |        |
      |        +---------- S3-compatible artifact storage
      |
      +---- outbound authenticated worker sessions
                         |
                         v
                 Rust worker supervisor
                         |
              +----------+-----------+
              |                      |
       Python/PyTorch           signed native stage
       approved template        (future Burn/Candle)
              |
              v
      checkpoints + metrics
              |
              +---- optional Mojo/Colibri evaluation
              |
              +---- optional Solana settlement mirror
```

The Rust control plane does not perform model backpropagation. The Python process does not own provider identity, scheduling or settlement. Mojo does not own the pipeline. Solana does not verify training.

## 12. Required contracts

The implementation MUST define canonical schemas before service code expands beyond a simulated worker:

- `PipelineSpecV1` and `StageSpecV1`;
- `TemplateManifestV1`;
- `ProviderCapabilitiesV1` and `BenchmarkResultV1`;
- `StageLeaseV1`, `LeaseRenewalV1` and `FencingTokenV1`;
- `ArtifactManifestV1` and `CheckpointManifestV1`;
- `ExecutorRequestV1`, `ProgressEventV1` and `CompletionSummaryV1`;
- `VerificationPolicyV1` and `VerificationDecisionV1`;
- `QuoteV1`, `ReservationV1` and `StageSettlementV1`;
- `AuditEventV1`.

Canonical signing rules:

- JSON is permitted for human-facing APIs, but signed commitments MUST use a single canonical binary or canonical-JSON encoding selected in the architecture decision record.
- Unknown fields MUST be rejected for signed v1 commitments.
- Every signed message MUST include protocol version, type, nonce or sequence, issued time, expiry where relevant and domain separator.
- All byte lengths, integer bounds and hash algorithms MUST be explicit.

## 13. API requirements

Minimum customer API:

```text
POST /v1/pipelines/validate
POST /v1/pipelines
POST /v1/pipelines/{pipeline_id}/fund
GET  /v1/pipelines/{pipeline_id}
GET  /v1/pipelines/{pipeline_id}/events
GET  /v1/pipelines/{pipeline_id}/artifacts
POST /v1/pipelines/{pipeline_id}/cancel
GET  /v1/templates
POST /v1/pricing/estimate
```

API-wide requirements:

- bearer identity for the trusted MVP, with roles enforced server-side;
- idempotency key on every create, cancel, fund, reserve and settlement action;
- RFC 7807-style machine-readable errors;
- cursor pagination for event and artifact lists;
- UTC timestamps and explicit integer units;
- OpenAPI document generated and contract-tested in CI;
- optimistic concurrency version on operator mutations.

`POST /v1/pipelines/{pipeline_id}/fund` MUST commit the accepted quote version, a maximum reservation equal to the accepted stage quote caps plus the declared verification reserve, funding-account identity and idempotency key. It MUST require the current pipeline concurrency version, reject a changed/expired quote and atomically move an unfunded validated pipeline to funded with its reservation.

Provider protocol message catalogue:

```text
Hello / Challenge / ChallengeResponse
Capabilities / BenchmarkAssignment / BenchmarkResult
Heartbeat / DrainState
LeaseOffer / LeaseAccept / LeaseReject / LeaseRenew
StageStarted / Progress / CheckpointSubmitted
ArtifactSubmitted / CompletionSubmitted / StageCancelled
CoordinatorAck / CoordinatorError
```

Reconnect MUST be safe: the worker presents its last acknowledged sequence and active lease. The coordinator either resumes that exact fenced lease or explicitly invalidates it.

## 14. Reference model, data and evaluation lock

Before core implementation Slice 3 begins, an architecture decision record MUST pin:

- one openly licensed causal language model no larger than 2B parameters;
- one immutable model revision and tokenizer revision;
- one public or project-generated non-sensitive dataset manifest;
- one deterministic training subset and held-out evaluation subset;
- sequence length, precision, batch size, accumulation, LoRA targets/rank and maximum steps;
- the primary metric, guardrail metrics and selection rule;
- expected memory on the 16 GiB Apple reference machine and NVIDIA baseline;
- model and dataset licence review.

Default candidate: `Qwen/Qwen2.5-1.5B-Instruct`, subject to the memory, current support and licence gate. If it fails any gate, the fallback is a smaller Apache-2.0 instruction model. The revision hash—not a mutable model name—becomes the contract.

The deterministic test suite MUST also contain a tiny locally generated model/dataset fixture so unit and chaos tests do not depend on external downloads or stochastic quality judgements.

## 15. Security and data policy

Allowed:

- public, synthetic or project-generated datasets;
- openly redistributable model revisions;
- non-sensitive evaluation prompts;
- operator-reviewed template dependencies.

Prohibited:

- personal data, customer secrets, regulated records or private model weights;
- arbitrary containers, remote shells and user-provided package installation;
- unbounded network access;
- mainnet wallets or assets with real value.

The provider sees plaintext inputs and weights during execution. The customer must acknowledge that the MVP offers provenance and bounded execution, not data confidentiality.

Required threat tests include forged signatures, replayed receipts, stale fencing tokens, malicious manifests, path traversal, archive bombs, symlink escape, oversized artifacts, dependency substitution, network-policy bypass, log secret leakage and double-settlement attempts.

## 16. Acceptance test suite

### AT-01 — Canonical DAG

Given the pinned reference pipeline, validation produces one stable hash. Reordered semantically equivalent input serializations resolve according to the documented canonicalisation rule. Cycles, duplicate IDs, unknown signed fields and unsupported schema versions are rejected before funding.

### AT-02 — Complete data-stage vertical slice

A customer submits `dataset-prepare-v1`; the coordinator produces an accepted quote, reserves its maximum amount plus the declared verification reserve through `/fund`, and leases the stage. A real worker executes it; the output uploads directly to object storage; and the verifier promotes it. The accepted artifact reconstructs its full lineage. Production settlement is exercised later by AT-09.

### AT-03 — Lease fencing

A worker loses contact until its lease expires. The stage is reassigned. The original worker later submits an output using the stale token. The stale output is rejected and cannot alter the accepted attempt or ledger.

### AT-04 — Checkpoint recovery

Across 20 scripted interruption points, the training job resumes from the last promoted checkpoint, restores optimizer/scheduler/RNG state, completes and loses no more than one checkpoint interval.

### AT-05 — Sweep fan-out/fan-in

Eight pinned LoRA trials execute across at least three providers. All accepted trials are evaluated under one policy and converge into exactly one winner using the declared tie-breaker.

### AT-06 — Independent winner verification

The winning adapter is fetched by a compatible worker operated by a different provider, loads without mutation and reproduces the declared evaluation within tolerance.

### AT-07 — Artifact integrity

Every accepted artifact passes hash, size, type, lineage, environment, signature and data-class checks. Corrupt temporary uploads are never promoted.

### AT-08 — Known fraud and corruption corpus

The versioned corpus contains at least 100 replay, corruption, wrong-lineage, random/empty adapter and fabricated-metric cases plus 100 honest cases. At least 95 malicious fixtures are rejected and no more than five honest fixtures are rejected.

### AT-09 — Idempotent accounting

The legal causal order is quote acceptance -> funding/reservation -> artifact acceptance -> settlement -> unused-reservation release/pipeline close. Duplicates and reordering among independent events produce the same final microcredit balances. Causally invalid events, including settlement before reservation or acceptance, are rejected without state change. No stage receives two settlements and no completed/cancelled pipeline retains a reservation. Golden event streams define permitted commutative reorder classes.

### AT-10 — Coordinator recovery

The coordinator is terminated during active leases and artifact submission. It restarts within 10 minutes, reconstructs state from PostgreSQL and resumes without lost accepted events, duplicate assignment or duplicate settlement.

### AT-11 — Resource and policy boundary

Templates exceeding CPU, RAM, disk, wall time or network policy are stopped and rejected. On Linux, isolation tests prove the executor cannot reach the worker identity key or files outside the assigned workspace. Apple tests verify the dedicated project account/machine, filesystem scope, keychain denial, process boundary and network policy; no general provider-owned Mac is admitted until those controls pass.

### AT-12 — Pilot evidence

The trusted pilot completes at least 50 pipelines and 500 attempted stages on 5–10 providers. The final report contains completion, recovery, verification, hardware, cost, energy, provider margin and hosted-comparator results.

## 17. Delivery slices and release gates

### Slice 0 — Decisions and deterministic fixture

Deliver:

- source repository bootstrap and upstream provenance;
- architecture decision records for canonical encoding, the model/data/evaluation candidate and lock procedure, runtime isolation and off-chain ledger units;
- decision records for checkpoint compensation, the minimum operator interface, tariff/comparator methodology and licensing/commercial posture;
- tiny deterministic model/data fixture;
- protocol and threat-model drafts.

Gate: all blocking dependencies have an owner, version/fallback and decision deadline; named people are assigned to the product-owner and technical-owner roles before implementation begins; fixture training and resume runs locally.

### Slice 1 — Contract-first simulator

Deliver:

- Rust workspace;
- canonical schemas and state machines;
- simulated coordinator, worker, object store and ledger;
- unit/property tests for validation, fencing and idempotency.

Gate: simulated pipelines complete, retry, recover and replay to identical final state.

### Slice 2 — Dataset-stage vertical slice

Deliver:

- real coordinator and PostgreSQL persistence;
- real outbound worker session;
- S3-compatible artifact path;
- minimal fixed-integer quote, funding and reservation path including the declared verification reserve; settlement remains in Slice 5;
- `dataset-prepare-v1`;
- OpenAPI and operator audit view or CLI.

Gate: 100 executions under injected worker, network, store and coordinator failures meet the artifact and retry requirements.

### Slice 3 — Checkpointed LoRA across workers

Deliver:

- pinned model/data/evaluation decision;
- `hf-peft-sft-v1` and `hf-evaluate-v1`;
- atomic checkpoint promotion;
- resume compatibility and cross-worker transfer;
- hardware admission for Apple and NVIDIA baselines.

Gate: AT-04 passes 20/20 and produces an accepted adapter.

### Slice 4 — Sweep, verification and ranking

Deliver:

- fan-out/fan-in scheduling;
- hidden canary and duplicate selection;
- versioned evaluation/ranking;
- fixed fraud/corruption corpus;
- independent winner verification.

Gate: AT-05, AT-06 and AT-08 pass; verification uses no more than 15% of useful compute.

### Slice 5 — Ledger and economics

Deliver:

- fixed-integer quotes, reservations and settlement;
- event replay/reconciliation;
- provider energy and margin report;
- hosted comparator capture.

Gate: AT-09 passes every replay/order case; all pipeline balances reconcile exactly.

### Slice 6 — Trusted provider pilot

Deliver:

- 5–10 enrolled providers;
- operator runbook and incident controls;
- 50-pipeline/500-stage trial;
- final technical and economic go/no-go report.

Gate: AT-12 and every core success measure pass or receive an explicit failed-hypothesis decision.

### Extension A — Mojo and Colibri

Begins only after Slice 4. Follows `docs/plans/mojo-colibri-olmoe-plan.md`. Its results are a multi-hardware inference/evaluation report, not a prerequisite for accepting the training marketplace.

### Extension B — Solana

Begins only after Slice 5 has demonstrated exact event replay and off-chain accounting. Localnet precedes devnet. Mainnet and real-value payment remain prohibited.

### Extension C — Rust-native training

Begins only after Slice 3 supplies the PyTorch correctness baseline. Burn and Candle are evaluated against the same fixture, checkpoint, hardware and evaluation contract. Adoption requires a written decision showing a material portability, safety or performance advantage.

## 18. Dependencies and decision register

| Decision/dependency | Blocking slice | Owner role | Decision deadline | Version/pin | Required evidence | Fallback |
| --- | --- | --- | --- | --- | --- | --- |
| Source repository layout | 0 | Technical owner | Before Slice 0 exit | ADR-0001 + upstream commit | clean baseline and provenance | separate repos with versioned protocol package |
| Licensing/commercial posture | 0 | Product owner | Before dependency pinning | ADR-0009 | upstream/model/data/runtime licence review | research-only distribution |
| Canonical signed encoding | 1 | Technical owner | Before Slice 1 code | ADR-0002 + schema v1 | cross-language golden fixtures | deterministic protobuf |
| Training candidate/lock procedure | 0 | ML owner | Before Slice 0 exit | ADR-0003 draft | candidate licence and smoke benchmark | smaller Apache-2.0 candidate |
| Immutable model/data/evaluation lock | 3 | ML owner | Before Slice 3 code | final ADR-0003 + revision hashes | memory, PEFT, licence and deterministic split evidence | fully generated fixture and smaller model |
| Checkpoint compensation | 1 | Product owner | Before ledger contract | ADR-0006 | provider-risk/economic scenario table | no partial credit in core MVP |
| Operator interface | 2 | Product + operations | Before Slice 2 code | ADR-0007 | journey and incident-response walkthrough | CLI plus generated audit report |
| Linux isolation | 2 | Security owner | Before provider execution | ADR-0004 + pinned rootless OCI/runtime | GPU access and escape tests | alternative rootless runtime; block Linux provider admission if none passes |
| macOS isolation | 3 | Security owner | Before Apple admission | ADR-0004 + OS/toolchain pins | filesystem, keychain, process and network tests | dedicated project-owned Mac/account only |
| Object store | 2 | Technical owner | Before Slice 2 code | pinned S3-compatible server/client | multipart/hash/presign tests | filesystem adapter for tests only |
| Coordinator database | 2 | Technical owner | Before Slice 2 code | pinned PostgreSQL major | transaction/event replay tests | in-memory state for Slice 1 simulator only; no pilot fallback |
| Observability backend | 2 | Operations owner | Before Slice 2 exit | pinned OpenTelemetry schema/exporter | trace propagation | structured local logs/metrics |
| Tariff/comparator method | 5 | Product owner | Before Slice 5 code | ADR-0008 | repeatable tariff and hosted-price capture | report economics as inconclusive |
| AMD hardware | Extension | Provider/ML owner | Before AMD admission | pinned ROCm/driver/template matrix | template fixture pass | defer without blocking core |
| Mojo toolchain/licence | Extension | Mojo owner | Before Mojo build | pinned compiler/runtime + licence decision | C ABI, target and redistribution evidence | keep established backend |
| Solana/Anchor versions | Extension | Ledger owner | After Slice 5 | pinned Solana/Anchor versions | localnet replay/security tests | off-chain ledger remains source of truth |

## 19. Principal risks and mitigations

| Risk | Consequence | Mitigation/decision gate |
| --- | --- | --- |
| MVP becomes a generic cloud platform | Delivery never reaches a proof | one launch pipeline, three templates and fixed limits |
| Heterogeneous training is not reproducible | Winner cannot be trusted | versioned policy, tolerances and second-provider evaluation |
| Provider churn destroys long jobs | Economics collapse | checkpoint every 10 minutes, drain and reassignment tests |
| Malicious or replayed artifacts pass | Invalid payment/results | fencing, signed lineage, hidden canaries and fixed fraud corpus |
| Python template escapes | Provider compromise | signed templates, rootless isolation, no arbitrary packages/egress |
| Apple/AMD backend differences | Cross-vendor scope stalls | Apple+NVIDIA core; AMD independently gated |
| Mojo maturity/licence blocks distribution | Extension cannot ship | optional plugin, pinned toolchain, licence review, no core dependency |
| Solana distracts from useful-work proof | Payment demo masks weak product | exact off-chain ledger first; Solana mirrors only accepted state |
| Provider economics rely on token subsidy | Unsustainable network | test credits and explicit energy/hosted comparison |
| Data privacy limits demand | Narrow workload set | public/synthetic-only positioning; no false confidentiality claim |

## 20. Go/no-go decision

Proceed to a broader trusted beta only if:

- core acceptance tests pass;
- provider recovery and artifact verification remain inside the stated overhead;
- at least one customer workload meets the hosted-cost or differentiated-access threshold;
- projected provider margin is positive without speculative rewards;
- security review has no open critical finding;
- at least two accelerator backend families complete accepted work.

Reposition toward batch evaluation, quantization and inference if training environments are too heterogeneous but artifact execution and Colibri streaming remain useful.

Pause the marketplace if accepted training cannot be verified affordably, interruptions repeatedly exceed the loss budget, provider economics remain negative, or only current data-centre NVIDIA hardware passes admission.

## 21. Open product questions

These questions do not block the PRD itself, but Slice 0 must resolve them before their dependent work begins:

1. Which exact model revision, dataset and evaluation policy become the reference lock?
2. Is Linux NVIDIA plus Apple MPS sufficient for the first pilot, or is one AMD machine already committed?
3. What electricity tariff input is acceptable: provider-declared, regional reference or both?
4. Should valid promoted checkpoints receive partial credit when the final stage later fails?
5. What operator UI is minimally necessary beyond a CLI and generated audit report?
6. Is the project intended to remain research-first, or should licensing choices preserve a future commercial network from day one?

## 22. Source notes

- Product and engineering rationale: `docs/decentralized-ml-training-marketplace-mvp.md`.
- Mojo/Colibri implementation planning: `docs/plans/mojo-colibri-olmoe-plan.md`.
- PyTorch is Python-first with a substantial C++ core: [official repository](https://github.com/pytorch/pytorch) and [C++ frontend](https://docs.pytorch.org/cppdocs/frontend.html).
- PyTorch distributed and elastic recovery applies inside a provider-owned cluster, not as a justification for WAN gradient synchronisation: [DDP](https://docs.pytorch.org/docs/stable/notes/ddp.html) and [Torch Distributed Elastic](https://docs.pytorch.org/docs/stable/distributed.elastic.html).
- PEFT/LoRA is the initial training method: [Hugging Face PEFT](https://huggingface.co/docs/transformers/peft).
- Mojo/MAX is treated as a kernel/inference extension rather than a complete training framework: [MAX overview](https://docs.modular.com/stable/max/intro/) and [model development](https://docs.modular.com/stable/max/develop/).
