# MojoColib training marketplace — implementation plan

Status: **Implementation in progress; bootstrap and contract-first happy path implemented**
Date: 2026-07-12
PRD: `docs/mojocolib-training-prd.md`
Engineering brief: `docs/decentralized-ml-training-marketplace-mvp.md`
Related Mojo plan: `docs/plans/mojo-colibri-olmoe-plan.md`

## 1. Outcome

Build and validate a trusted MVP that schedules one reproducible eight-trial LoRA sweep across 5–10 heterogeneous provider machines, survives forced interruption through promoted checkpoints, verifies the winning adapter on a different provider and settles valueless test credits exactly once for accepted work.

The plan deliberately proves the training marketplace before adding Mojo kernels or Solana. The product decision and launch workflow are defined in `docs/mojocolib-training-prd.md:11-28`; the core success measures and exact metric definitions are at `docs/mojocolib-training-prd.md:102-135`.

## 2. Current repository state

- The workspace now contains Rust protocol, replay-safe state-machine, deterministic scheduler and runnable in-memory simulator crates, deterministic offline fixtures and CI.
- The directory is a Git checkout with upstream Colibri pinned under `engine/colibri/`; branch protection and a clean hosted CI run still remain external setup work.
- The product/engineering brief already defines the control, execution, artifact and settlement planes at `docs/decentralized-ml-training-marketplace-mvp.md:278-302`.
- The PRD converts that brief into prioritised requirements, contracts and acceptance tests at `docs/mojocolib-training-prd.md:137-351` and `docs/mojocolib-training-prd.md:483-531`.
- The existing Mojo plan remains an independent extension and explicitly excludes training.

Repository bootstrap and provenance are complete. The V1 contract core and one replay-safe happy path run locally; real training/resume and the coordinator/worker service boundary remain the next gates.

## 3. Requirements summary

### Required product behaviour

- Invitation-only identities and signed outbound worker sessions.
- Immutable typed pipeline DAGs with canonical hashes.
- Approved dataset, LoRA training and evaluation templates only.
- Compatibility-aware scheduling, renewable fenced leases and bounded retries.
- Direct worker/object-store artifact transfer.
- Atomic, idempotent checkpoint promotion and cross-worker resume.
- Versioned verification, sweep ranking and second-provider winner evaluation.
- Fixed-integer off-chain reservations and stage settlement.
- Append-only audit events and exact replay/reconciliation.
- A 5–10-provider pilot with measured recovery, verification and economics.

### Required technology boundary

- Rust owns control, trust, scheduling, worker supervision, manifests, metering and ledger.
- Python/PyTorch owns the baseline training and evaluation implementation.
- The executor protocol remains runtime-neutral so a signed Burn/Candle binary can be evaluated later.
- Mojo/Colibri begins only after the training/sweep path is accepted.
- Solana begins only after the off-chain ledger replays exactly.

The detailed boundary is recorded in `docs/mojocolib-training-prd.md:30-47` and `docs/mojocolib-training-prd.md:353-381`.

## 4. Acceptance criteria

Implementation is complete only when:

1. The twelve acceptance tests in `docs/mojocolib-training-prd.md:483-531` have reproducible test artifacts and pass.
2. Twenty scripted training interruptions resume from the last promoted checkpoint with no more than one declared checkpoint interval of lost work.
3. One eight-trial LoRA sweep runs on at least three providers and produces exactly one versioned, independently verified winner.
4. At least 95 of 100 known malicious/corrupt fixtures are rejected and at most 5 of 100 honest fixtures are falsely rejected.
5. Duplicate, reordered and replayed ledger events produce identical balances, zero double settlement and zero stranded reservations.
6. The pilot attempts at least 50 pipelines and 500 stages on 5–10 machines, with at least three hardware configurations and two backend families.
7. At least 95% of valid stages finish after automatic retries.
8. Verification consumes no more than 15% of useful compute.
9. No unapproved dataset manifest or disallowed declared data class and no real-value payment enters the system.
10. The final report gives a pass/fail decision against every metric and definition in `docs/mojocolib-training-prd.md:102-135`.

## 5. Architecture decision

### Selected option — Rust control plane with constrained PyTorch executors

Use a Rust coordinator and worker around small signed PyTorch templates. Treat the training runtime as a replaceable executor rather than embedding PyTorch deeply into the protocol.

Benefits:

- fastest path to mature autograd, optimisers, PEFT and checkpointing;
- Rust still owns the security- and payment-sensitive boundary;
- the protocol can later admit Burn, Candle or native Mojo stages;
- training correctness can be separated from scheduler and marketplace correctness;
- cross-vendor training can use each platform's current supported backend.

Costs:

- two language toolchains and cross-language schemas;
- Python environment and isolation complexity;
- exact numerical reproducibility still varies by backend.

### Rejected for the core MVP — Rust-native training first

Burn and Candle are promising but are separate frameworks, not a Rust implementation of PyTorch. Starting there would make framework maturity, operator coverage and marketplace correctness fail together. They remain a controlled extension after the PyTorch baseline.

### Rejected for the core MVP — Mojo-only training

Mojo/MAX is currently strongest as a kernel and inference surface. A Mojo-only training stack would require proving autograd, optimisers, checkpoint compatibility and model support before proving the marketplace.

### Rejected for the core MVP — WAN distributed data/model parallelism

Cross-provider gradient/activation synchronisation is latency- and membership-sensitive. The marketplace schedules complete stages and independent trials; provider-local clusters may use DDP internally later.

## 6. Planned source layout

The exact repository choice is resolved in Step 0. If the marketplace and Colibri backend remain in one repository, use:

```text
Cargo.toml
rust-toolchain.toml
crates/
  protocol/
    src/pipeline.rs
    src/template.rs
    src/lease.rs
    src/artifact.rs
    src/verification.rs
    src/ledger.rs
  crypto/
  state-machine/
  scheduler/
  manifests/
  artifact-client/
  metering/
services/
  coordinator/
    src/api/
    src/db/
    src/scheduling/
    src/verification/
    migrations/
  worker/
    src/session/
    src/inventory/
    src/executor/
    src/cache/
    src/resources/
python/
  pyproject.toml
  colibri_mesh_sdk/
  templates/
    dataset_prepare_v1/
    hf_peft_sft_v1/
    hf_evaluate_v1/
schemas/
  v1/
fixtures/
  tiny_training/
  fraud_corpus/
deploy/
  dev/
tests/
  contract/
  integration/
  chaos/
  security/
docs/
  adr/
  protocol/
  runbooks/
```

All paths above are planned paths; none exists at plan time.

## 7. Dependency order

```text
Step 0: repository + decisions + fixtures
   |
   v
Step 1: signed contracts + pure state machines
   |
   v
Step 2: coordinator/worker/data-stage vertical slice
   |
   v
Step 3: PyTorch checkpoint/resume across workers
   |\
   | +----> Extension C: Burn/Candle canary
   v
Step 4: sweep + verification + ranking
   |\
   | +----> Extension A: Mojo/Colibri
   v
Step 5: ledger + economics
   |\
   | +----> Extension B: Solana
   v
Step 6: trusted provider pilot
```

Dependent steps remain sequential. Within a step, coordinator, worker/template and verification/test work can proceed in parallel once the relevant contracts are frozen.

## 8. Implementation steps

### Step 0 — Bootstrap, decision records and deterministic fixture

Purpose: remove decisions that would otherwise leak into every crate and template.

Implementation checkpoint (2026-07-12): repository import/provenance, ADRs, threat model, deterministic structural fixture and CI are implemented. The exit gate remains open because the fixture has not yet trained/checkpointed/resumed under PyTorch, and the project licence and named owners are not selected.

Actions:

1. Initialise a Git repository or clone/fork the selected Colibri upstream into a clean repository; record the upstream commit and licence.
2. Preserve the current product documents and public implementation plans under `docs/`.
3. Add `docs/adr/0001-repository-boundary.md` deciding monorepo versus versioned marketplace/backend repositories.
4. Add `docs/adr/0002-canonical-encoding.md` choosing deterministic CBOR, canonical JSON or deterministic protobuf for signed commitments.
5. Add a draft `docs/adr/0003-reference-training-lock.md` naming candidates, the selection procedure, evidence requirements and the deadline before Step 3; do not claim an immutable lock before the benchmark evidence exists.
6. Add `docs/adr/0004-executor-isolation.md` defining Linux rootless isolation and the narrower trusted macOS policy.
7. Add `docs/adr/0005-ledger-units.md` defining microcredits, rounding, reservations and settlement invariants.
8. Add `docs/adr/0006-checkpoint-compensation.md` deciding whether promoted partial work can earn credit.
9. Add `docs/adr/0007-operator-interface.md` defining the minimum operator CLI/report surface for Slice 2.
10. Add `docs/adr/0008-tariff-comparator.md` defining tariff inputs and hosted-price capture before Step 5.
11. Add `docs/adr/0009-licensing-posture.md` defining research/commercial constraints before dependencies are distributed.
12. Create a generated tiny model/dataset fixture that trains and resumes without network access.
13. Create the initial threat model and data-classification rules.
14. Create CI skeletons for Rust formatting/lint/tests, Python lint/type/tests and documentation link/schema checks.

Candidate-screening gate:

- start with the PRD's `Qwen/Qwen2.5-1.5B-Instruct` candidate;
- benchmark ordinary LoRA at the declared sequence length on the 16 GiB Apple reference and an NVIDIA baseline;
- record evidence for the candidate only if it fits without swap/OOM and the licence/data gate passes;
- otherwise select the documented smaller Apache-2.0 fallback.

The immutable model, tokenizer, dataset and evaluation revision lock is approved immediately before Step 3 source work begins.

Files:

- `docs/adr/0001-repository-boundary.md`
- `docs/adr/0002-canonical-encoding.md`
- `docs/adr/0003-reference-training-lock.md`
- `docs/adr/0004-executor-isolation.md`
- `docs/adr/0005-ledger-units.md`
- `docs/adr/0006-checkpoint-compensation.md`
- `docs/adr/0007-operator-interface.md`
- `docs/adr/0008-tariff-comparator.md`
- `docs/adr/0009-licensing-posture.md`
- `docs/threat-model.md`
- `fixtures/tiny_training/*`
- CI configuration appropriate to the selected host

Verification:

- clean baseline build if Colibri source is imported;
- tiny fixture trains, checkpoints, terminates and resumes locally;
- licence and source-provenance record is complete;
- every dependency row in `docs/mojocolib-training-prd.md:629-648` has an owner role, version/fallback and decision deadline.

Exit gate: PRD Slice 0 at `docs/mojocolib-training-prd.md:535-545` passes.

Estimated effort: 3–5 focused days.

### Step 1 — Contract-first Rust simulator

Purpose: make state, signatures, fencing and accounting correct before real GPUs or processes are involved.

Implementation checkpoint (2026-07-12): V1 contracts, strict canonical parsing/signing, replay-safe reducers, fixed-integer ledger, property tests, compatibility scheduler and a deterministic completed-stage simulator are implemented. The exit gate remains open until failure/expiry/resume scenarios, an object-store simulation and initial schemas are added.

Actions:

1. Create the Rust workspace and pin the Rust toolchain.
2. Implement the v1 domain types listed in `docs/mojocolib-training-prd.md:383-402`.
3. Implement canonical serialization and golden cross-language fixtures.
4. Implement pure pipeline and stage state machines with explicit permitted transitions.
5. Implement lease issue/accept/renew/expire semantics and fencing tokens.
6. Implement canonical artifact/checkpoint manifests and signature validation.
7. Implement an in-memory simulated worker, object store and fixed-integer ledger.
8. Implement deterministic scheduler hard filters and tie-breaking; leave cost scoring behind a versioned policy interface.
9. Add property tests for DAG validation, transition legality, idempotency, event replay and arithmetic overflow.
10. Generate initial JSON Schema/OpenAPI components from or alongside the canonical Rust types.

Key invariants:

- one accepted attempt per stage;
- one settlement per accepted stage;
- stale fencing tokens cannot promote artifacts;
- a funded pipeline spec never mutates;
- downstream stages see accepted manifests only;
- replay produces identical state and balances.

Files:

- `Cargo.toml`
- `rust-toolchain.toml`
- `crates/protocol/**`
- `crates/crypto/**`
- `crates/state-machine/**`
- `crates/scheduler/**`
- `crates/manifests/**`
- `crates/metering/**`
- `schemas/v1/**`
- `tests/contract/**`

Verification:

- `cargo fmt --check`;
- `cargo clippy --workspace --all-targets -- -D warnings`;
- `cargo test --workspace`;
- property tests generate cycles, reordered events, duplicate receipts, stale leases and boundary integer values;
- golden Rust/Python canonicalisation fixtures match byte-for-byte.

Exit gate: simulated pipelines complete, fail, retry, expire, resume and replay to the identical final state described at `docs/mojocolib-training-prd.md:547-556`.

Estimated effort: 1.5–2.5 weeks.

### Step 2 — Real dataset-stage vertical slice

Purpose: prove the complete network and artifact path without ML training variability.

Actions:

1. Implement the Axum coordinator API and PostgreSQL repository.
2. Commit state mutation and audit event in one transaction.
3. Implement role-based trusted-MVP authentication and idempotency keys.
4. Implement the production minimum `QuoteV1` and `ReservationV1` path in fixed-integer microcredits, including maximum accepted stage quotes, the declared verification reserve, quote version/expiry, atomic `/fund`, idempotency and release on pre-execution cancellation. Settlement remains Step 5 work.
5. Implement the outbound worker session, challenge/response and sequence-aware reconnect.
6. Implement hardware/software inventory and a signed CPU/data benchmark.
7. Implement direct presigned artifact download/upload plus atomic promotion.
8. Implement the signed `dataset-prepare-v1` template with schema validation, deterministic shard output and no default network egress.
9. Implement worker process supervision, resource/time limits, drain/cancel and content-addressed cache.
10. Expose pipeline/event/artifact status through the customer API and an operator CLI.
11. Add trace IDs and required metrics across coordinator, worker, executor and artifact operations.

Files:

- `services/coordinator/**`
- `services/coordinator/migrations/**`
- `services/worker/**`
- `services/coordinator/src/ledger/quote.rs`
- `services/coordinator/src/ledger/reservation.rs`
- `crates/artifact-client/**`
- `python/colibri_mesh_sdk/**`
- `python/templates/dataset_prepare_v1/**`
- `deploy/dev/**`
- `tests/integration/data_stage/**`
- `tests/chaos/data_stage/**`

Verification:

- OpenAPI contract tests for all routes in `docs/mojocolib-training-prd.md:404-444`;
- `/fund` atomically reserves the maximum accepted quotes plus the declared verification reserve, rejects stale quotes/concurrency versions and remains idempotent under retries;
- 100 data-stage executions with scripted disconnect, coordinator restart, object-store failure, corrupt upload, disk-full and cancellation cases;
- every accepted output independently validates hash, type, lineage and signature;
- p95 API and lease-offer latency meet `docs/mojocolib-training-prd.md:321-327`;
- artifact bytes never pass through coordinator memory/body handlers.

Exit gate: acceptance AT-01, AT-02, AT-03 and the data-stage Slice 2 gate pass.

Estimated effort: 2–3 weeks.

### Step 3 — Checkpointed PyTorch LoRA across workers

Purpose: prove the central training claim on the user's 16 GiB class of machine and a discrete GPU baseline.

Actions:

1. Review the candidate evidence, approve ADR-0003's immutable model/data/evaluation revision lock, and enforce it before implementing or running the production training template.
2. Implement `ExecutorRequestV1`, progress events and completion summary in the Python SDK.
3. Implement `hf-peft-sft-v1` with pinned PyTorch, Transformers, PEFT and safetensors dependencies.
4. Save adapter, optimiser, scheduler, global step/epoch and RNG state at least every 50 steps or 10 minutes.
5. Implement checkpoint compatibility key and preflight resume validation.
6. Implement graceful drain, local atomic checkpoint write, upload and coordinator promotion.
7. Implement `hf-evaluate-v1` and the tiny hidden canary.
8. Build Apple MPS and NVIDIA CUDA admission fixtures; record exact supported versions. Until platform-specific isolation tests pass, run Apple stages only on a dedicated project-owned machine/account containing no personal files and exposing no worker identity key to the executor.
9. Script 20 deterministic interruption points, including during compute, local checkpoint write, upload and after promotion.
10. Reassign selected attempts to a second compatible worker and validate resumed state.

Files:

- `python/templates/hf_peft_sft_v1/**`
- `python/templates/hf_evaluate_v1/**`
- `python/colibri_mesh_sdk/executor.py`
- `crates/protocol/src/executor.rs`
- `crates/manifests/src/checkpoint.rs`
- `services/worker/src/executor/**`
- `services/coordinator/src/checkpoints/**`
- `fixtures/tiny_training/**`
- `tests/integration/checkpoint_resume/**`
- `tests/chaos/checkpoint_resume/**`

Verification:

- uninterrupted and resumed tiny fixture produce equivalent final state within declared tolerance;
- adapter modifies only permitted modules;
- every restored optimiser/scheduler/RNG field is asserted;
- all 20 forced interruptions satisfy AT-04;
- winning adapter loads on both admitted backend families;
- Apple run does not cause sustained swap, remains inside ADR memory limits and passes the dedicated-account filesystem, keychain, process and network policy tests.

Exit gate: `docs/mojocolib-training-prd.md:571-581` passes.

Estimated effort: 2–3 weeks.

### Step 4 — Sweep orchestration, verification and ranking

Purpose: turn isolated training jobs into the product's first useful ML workflow.

Actions:

1. Add fan-out/fan-in DAG execution for eight independent trials.
2. Add provider-spread policy and deterministic scheduling inputs.
3. Implement the versioned evaluation policy, primary metric, guardrails and tie-breaker.
4. Implement checkpoint structural/finite/module/update-statistics checks.
5. Implement hidden evaluation subset selection and second-provider verification.
6. Add operator-selected duplicate and canary execution.
7. Build the fixed 100-malicious/100-honest corruption and fraud corpus.
8. Track useful compute and verification/re-execution compute separately.
9. Publish the winner's adapter, evaluation, model card and complete provenance bundle.
10. Add a pipeline-level report explaining every rejected attempt and ranking decision.

Files:

- `services/coordinator/src/verification/**`
- `services/coordinator/src/ranking/**`
- `crates/protocol/src/verification.rs`
- `python/templates/hf_evaluate_v1/**`
- `fixtures/fraud_corpus/**`
- `tests/integration/lora_sweep/**`
- `tests/security/artifact_fraud/**`

Verification:

- AT-05, AT-06, AT-07 and AT-08 pass;
- exactly one winner is selected under every tie case;
- second-provider results stay within ADR tolerances;
- malicious corpus detection is at least 95%, honest false rejection at most 5%;
- verification overhead is at most 15% of useful compute.

Exit gate: `docs/mojocolib-training-prd.md:583-593` passes.

Estimated effort: 1.5–2.5 weeks.

### Step 5 — Off-chain ledger and provider economics

Purpose: prove accepted-work accounting without allowing blockchain work to obscure ledger defects.

Actions:

1. Extend the Step 2 fixed-integer quote/reservation path with platform/provider splits and production settlement.
2. Bind settlement to the accepted stage/attempt/artifact/verification commitments.
3. Implement cancel, expiry and completion release rules.
4. Add deterministic event replay and periodic reconciliation under the legal partial order: quote -> reservation -> artifact acceptance -> settlement -> release/close. Permit duplicates and independent reordering; reject causal violations without state change.
5. Record measured duration, work units and available power telemetry without trusting telemetry as payment proof.
6. Calculate estimated energy from declared tariff and measured/estimated system watts.
7. Capture timestamped comparator prices for the same pinned workload from at least two hosted alternatives.
8. Generate customer cost and provider projected-margin reports.
9. Add adversarial duplicate, reorder, overflow and restart tests.

Files:

- `crates/protocol/src/ledger.rs`
- `crates/metering/**`
- `services/coordinator/src/ledger/**`
- `services/coordinator/src/reconciliation/**`
- `services/coordinator/migrations/**`
- `tests/integration/ledger/**`
- `tests/chaos/ledger/**`
- `docs/runbooks/reconciliation.md`

Verification:

- AT-09 passes;
- property tests cover maximum/minimum values, permitted commutative reorder classes and rejection of every causal-order violation;
- coordinator restart cannot create a second settlement;
- completed/cancelled pipelines have zero remaining reservations;
- every report states tariff and power assumptions separately from measured facts.

Exit gate: `docs/mojocolib-training-prd.md:595-604` passes.

Estimated effort: 1–2 weeks.

### Step 6 — Trusted provider pilot and go/no-go report

Purpose: test reliability and economics on real, imperfect machines rather than only fixtures.

Actions:

1. Write provider onboarding, safety, drain, incident and removal runbooks.
2. Enrol 5–10 trusted providers and collect explicit hardware/OS/backend commitments.
3. Require admission benchmarks and supported template fixtures on every machine.
4. Run at least 50 complete pipelines and 500 attempted stages.
5. Inject the agreed worker, network, storage, coordinator and verification failures.
6. Monitor thermal throttling, power policy, storage use, cache behaviour and provider churn.
7. Re-run selected accepted adapters on a separate provider/backend.
8. Reconcile all artifact and ledger state after the pilot.
9. Compare customer cost and provider projected return with the captured hosted baselines.
10. Produce the final evidence pack and explicit proceed/reposition/pause decision.

Files:

- `docs/runbooks/provider-onboarding.md`
- `docs/runbooks/provider-safety.md`
- `docs/runbooks/incident-response.md`
- `docs/pilot/experiment-protocol.md`
- `docs/pilot/final-report.md`
- machine-readable pilot result bundle under a documented results location

Verification:

- AT-10, AT-11 and AT-12 pass;
- all success measures and metric definitions at `docs/mojocolib-training-prd.md:102-135` receive evidence and verdict;
- no critical security issue remains open;
- at least two accelerator backend families produce accepted work;
- every dataset uses an operator-approved immutable manifest and allowed declared data class; no real-value payment is present. The report does not claim that manifest approval proves the absence of hidden personal data.

Exit gate: the go/no-go rules at `docs/mojocolib-training-prd.md:665-678` produce an explicit decision.

Estimated effort: 2–3 weeks after technical readiness.

## 9. Optional extension plans

### Extension A — Mojo/Colibri evaluation

Start only after Step 4. Reuse `docs/plans/mojo-colibri-olmoe-plan.md` and connect it through an approved `colibri-benchmark-v1` stage. The extension must consume accepted lineage and publish correctness before speed.

### Extension B — Solana localnet/devnet

Start only after Step 5. Mirror final off-chain stage settlements into Anchor accounts. Keep off-chain acceptance authoritative. Test localnet before devnet; prohibit mainnet and real-value assets.

### Extension C — Burn/Candle native training canary

Start only after Step 3. Implement one signed native template using the same tiny fixture, artifact/checkpoint schema and evaluation policy. Compare:

- supported operators and model import effort;
- Apple, AMD and NVIDIA backend availability;
- checkpoint completeness and portability;
- numerical correctness;
- peak memory and throughput;
- binary/toolchain/distribution burden.

Adopt a Rust-native training path only if it provides a material measured advantage without weakening correctness or provider coverage.

## 10. Test plan

### Unit and property

- DAG validation, canonical hashing and schema bounds;
- state transition legality and retry budgets;
- signature domains, expiry and key rotation;
- lease fencing and deterministic tie-breaks;
- artifact/checkpoint compatibility and promotion;
- fixed-integer arithmetic, idempotency and replay;
- scheduler normalization and policy versions.

### Contract

- Rust/Python golden serialization;
- OpenAPI request/response/error fixtures;
- provider reconnect sequence and message compatibility;
- executor request/progress/completion schemas;
- signed manifest unknown-field rejection.

### Integration

- customer to coordinator to real worker to artifact acceptance;
- PostgreSQL transaction/event atomicity;
- direct object-store multipart and presigned transfer;
- real PyTorch fixture, checkpoint and second-worker resume;
- sweep fan-out/fan-in and independent evaluation;
- quote/reserve/settle/release lifecycle.

### Chaos

- disconnect at each lease/checkpoint/artifact boundary;
- provider shutdown and drain;
- coordinator restart and database reconnect;
- object-store timeout/corruption;
- disk full, OOM and thermal cooldown;
- delayed/duplicate/reordered messages;
- stale worker returning after reassignment.

### ML correctness

- uninterrupted versus resumed equivalence;
- permitted LoRA module changes only;
- finite parameters and declared step/example count;
- fixed canary expected ranges;
- evaluation tolerance across backends;
- ranking tie and guardrail behaviour.

### Security

- forged identity and manifest signatures;
- replay and stale fencing tokens;
- path traversal, symlink escape and archive bombs;
- attempted network egress and secret access;
- oversized artifact/resource exhaustion;
- double settlement and operator override audit.

### Pilot/economics

- completion and retry distribution by backend;
- lost-work and checkpoint transfer distribution;
- verification cost/compute ratio;
- quoted versus actual duration;
- projected energy, provider margin and customer hosted comparison.

## 11. Verification commands to establish during implementation

The exact package choices are pinned in Step 0, but CI must expose one top-level verification path equivalent to:

```text
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python -m pytest python tests
python -m mypy python
python -m ruff check python tests
integration test suite with PostgreSQL and S3-compatible storage
chaos acceptance suite
security corpus suite
```

The final pilot evidence cannot be replaced by mocked tests.

## 12. Delivery estimate and staffing

Core MVP estimate: approximately 10–16 focused weeks, depending on environment isolation and access to provider hardware.

Recommended minimum staffing:

- one Rust systems engineer owning protocol, coordinator, worker and ledger;
- one ML/platform engineer owning PyTorch templates, checkpointing and evaluation;
- fractional security/reliability review before pilot;
- one product/operator owner recruiting and supporting 5–10 trusted providers.

Parallel work becomes useful after Step 1 freezes v1 contracts:

- coordinator/database/API lane;
- worker/executor/artifact lane;
- ML template/verification lane;
- independent test/security lane.

## 13. Risks and mitigations

| Risk | Plan response |
| --- | --- |
| No source repository exists yet | Step 0 establishes provenance, layout and CI before implementation |
| Exact model/data choice is unresolved | ADR and memory/licence gate block Step 3 |
| Python isolation is unsafe | signed templates, mandatory rootless Linux, dedicated restricted macOS, no arbitrary dependencies; block admission when isolation tests fail |
| Cross-vendor results differ | versioned tolerances and independent evaluation, not bitwise claims everywhere |
| Checkpoints are too large/slow | adapter-first LoRA, 5 GiB core cap, measured upload and local NVMe cache |
| Fraud verification is overstated | fixed known corpus and explicit trusted-verifier limitation |
| Old GPUs fail training support | admission by fixtures; older cards can remain inference/evaluation providers |
| Mojo delays core delivery | independent extension after accepted sweep |
| Solana masks ledger bugs | exact off-chain accounting first |
| Token economics distort demand | valueless credits and hosted/energy comparison |

## 14. Stop conditions

Stop and reposition before the full pilot if any of the following remain true after a bounded fix cycle:

- the reference model cannot train within the 16 GiB class without sustained swap/OOM and no smaller useful fallback passes;
- promoted checkpoints cannot resume reliably across compatible workers;
- verification overhead exceeds 25% in the controlled sweep;
- executor isolation has an unresolved critical escape;
- the event/ledger model cannot prevent duplicate settlement under replay;
- fewer than five trusted providers can be secured.

Do not broaden to anonymous providers, real payments, WAN distributed training or arbitrary containers to work around a failed core hypothesis.

## 15. Definition of done

- All planned core source, schemas, tests and runbooks exist.
- Formatting, lint, type, unit, property, contract, integration, chaos and security checks pass freshly.
- The trusted pilot meets or explicitly fails every measurable PRD criterion with retained evidence.
- The final artifact includes the accepted adapter, immutable provenance, independent evaluation, economics report and ledger reconciliation.
- The project records one explicit outcome: proceed to trusted beta, reposition to batch inference/evaluation, or pause.
- Mojo, Solana and Rust-native training remain separately reported; no extension result is used to hide a failed core MVP.

## 16. Plan review notes

This plan narrows the earlier engineering brief in four material ways:

1. It selects the eight-trial LoRA sweep as the only acceptance-driving launch workflow.
2. It makes three templates, Apple+NVIDIA baselines and an off-chain ledger the core boundary.
3. It turns Mojo, Solana, AMD expansion and Rust-native training into independently gated extensions.
4. It replaces broad claims such as “detect fabricated checkpoints” with a versioned, measurable corruption/fraud corpus and explicit false-rejection limit.
