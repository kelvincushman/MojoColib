# Decentralized inference marketplace — MVP brief

> **Scope note:** This inference-focused brief is now a companion module. The primary marketplace brief has been broadened to model training and ML pipelines: [decentralized-ml-training-marketplace-mvp.md](./decentralized-ml-training-marketplace-mvp.md).

Status: product and engineering brief
Project: **MojoColib**
Initial network: 5–10 trusted provider PCs
Initial settlement: Solana localnet and devnet only
Initial model: `allenai/OLMoE-1B-7B-0125-Instruct`
Primary implementation language: Rust
Accelerator layer: Mojo with the existing Colibri C runtime

## 1. Executive summary

The MVP will test whether idle consumer and legacy GPUs can provide reliable, economically useful AI inference through a coordinated marketplace.

Hardware owners run a lightweight Rust worker daemon. The daemon benchmarks the machine, downloads a signed model package, keeps the model warm when work is available, executes inference through Colibri/Mojo, records metering data and returns a signed completion receipt. A central Rust coordinator admits providers, matches jobs to suitable machines, verifies results and authorizes settlement. A small Solana program holds devnet escrow and records job settlement.

The MVP is deliberately **not** a fully permissionless peer-to-peer network. Providers are invited and identified, the scheduler is centralized, and settlement authority initially belongs to the coordinator. This allows the project to answer the difficult questions—reliability, verification, heterogeneous performance, power economics and provider earnings—before decentralizing trust.

The project’s potential differentiation is not blockchain alone. Existing markets already aggregate third-party GPUs. The distinctive hypothesis is that a portable Mojo-oriented runtime, integer-quantized inference and Colibri-style NVMe expert streaming can make underused hardware economically relevant, including Apple GPUs, AMD GPUs and older NVIDIA cards that mainstream platforms may ignore.

## 2. Product hypothesis

If a network can:

1. characterize heterogeneous consumer hardware accurately;
2. place complete inference requests on nodes that can execute them locally;
3. verify completed work with acceptable overhead;
4. price jobs using measured throughput and estimated energy rather than a claimed GPU name; and
5. settle providers transparently;

then owners of gaming PCs and older workstations can earn from idle hardware while inference customers gain access to a broader, more competitive supply pool.

The hypothesis is invalid if verification costs, node unreliability, model distribution, residential-network latency or electricity costs make the service less reliable or more expensive than ordinary hosted inference.

## 3. Why Rust

Rust is recommended for the marketplace and node-control layers.

Rust should own:

- the coordinator API and scheduler;
- provider identity and session management;
- the worker daemon and automatic updates;
- model download, verification and cache management;
- subprocess supervision;
- metering and signed receipts;
- the Solana client;
- the on-chain Solana program;
- shared protocol types and state machines.

Mojo should own:

- portable accelerator kernels;
- device contexts and accelerator memory;
- q8/q4 matrix operations;
- fused routed-expert execution;
- Metal, CUDA and HIP target-specific optimizations.

The existing Colibri C engine should initially own:

- model loading and tensor indexing;
- tokenization and sampling;
- attention and KV cache;
- MoE routing;
- expert cache and NVMe streaming;
- the host-facing inference loop.

Rust and native inference should initially communicate through a local subprocess protocol. This gives crash isolation, makes engine upgrades reversible and prevents unsafe FFI from sharing a process with wallets and networking. Once the protocol and ABI are stable, the worker may optionally load Colibri/Mojo through a versioned C interface. Rust supports explicit C-compatible foreign-function boundaries, while Mojo can emit native shared libraries. See the [Rust FFI guidance](https://doc.rust-lang.org/nomicon/ffi.html) and [Mojo compilation documentation](https://mojolang.org/docs/tools/compilation/).

## 4. MVP goals

The MVP must prove or disprove the following:

- Five to ten heterogeneous provider PCs can join and remain observable.
- A coordinator can select a compatible node using measured performance and current state.
- One open model can be distributed, verified, loaded and executed consistently.
- Jobs can recover from worker disconnects without double payment.
- Integer-quantized output is reproducible enough to support verification.
- Provider compensation can be calculated from useful work, measured speed and estimated energy.
- Solana devnet escrow can represent the payment lifecycle without placing inference data on-chain.
- The service can produce a credible comparison with ordinary hosted inference.

## 5. Non-goals

The MVP will not:

- split transformer layers across unrelated residential nodes;
- route every MoE layer over the public internet;
- support anonymous providers;
- use real-value payments or deploy to Solana mainnet;
- issue a new token;
- promise private prompts against a malicious provider;
- train or fine-tune models;
- support arbitrary user-supplied containers;
- support PlayStation or Xbox retail consoles;
- provide automatic VRAM pooling across machines;
- claim production-grade decentralization;
- market the service before provider economics are measured.

## 6. Target users

### Compute customer

The first customer is a developer or researcher who needs non-sensitive batch inference:

- prompt evaluation;
- synthetic text generation;
- structured extraction;
- model regression testing;
- embeddings or reranking in a later iteration;
- internal experiments where occasional retries are acceptable.

Interactive chat can be tested, but batch inference is the safer initial product because it tolerates residential latency and node churn.

### Compute provider

The first provider is a trusted participant with:

- a Windows, Linux or Apple Silicon computer;
- a supported GPU or a legacy GPU selected for experimentation;
- sufficient RAM and model storage;
- a stable broadband connection;
- permission to run sustained workloads;
- transparent electricity-cost and availability settings.

## 7. Supported hardware for the MVP

The network should intentionally include different hardware classes:

1. Apple M-series GPU using Mojo/Metal.
2. A supported NVIDIA Turing-or-newer GPU, ideally a T4 as the compatibility control.
3. A legacy NVIDIA Pascal card such as a P40 as an experimental path.
4. An AMD ROCm-compatible GPU if one is available during the trial.
5. A CPU-only worker as a correctness and scheduling control, not as a competitive provider tier.

Provider hardware is admitted based on benchmark results and model compatibility, not its marketing name.

Retail PlayStation devices are excluded because official PS5 development access is restricted to registered partners and approved projects. Xbox retail Dev Mode exists, but an unattended commercial compute worker would require platform approval and further investigation. Console support is therefore a future partnership question, not an MVP engineering task. See [Sony’s development hardware programme](https://sonyinteractive.com/en/news/blog/complimentary-development-hardware/) and [Microsoft’s Xbox Dev Mode overview](https://blogs.windows.com/windowsdeveloper/2020/09/17/app-compatibility-for-xbox-series-x-and-xbox-series-s-consoles/).

## 8. Initial model and workload

Use `allenai/OLMoE-1B-7B-0125-Instruct` for the first network trial.

Reasons:

- 7B total parameters with roughly 1B active per token;
- Apache-2.0 model licence;
- existing Colibri OLMoE engine and converter;
- enough expert sparsity to exercise the project’s differentiation;
- small enough to run on modest machines using quantization and constrained caches;
- suitable for testing resident execution and NVMe streaming with the same checkpoint.

Only one immutable converted model manifest is accepted during the MVP. Every node must report the same manifest hash, tokenizer hash, runtime build ID and quantization version.

Initial inference settings:

- greedy decoding for verification jobs;
- fixed-seed sampling for capability demonstrations;
- maximum prompt length: 2,048 tokens;
- maximum completion: 256 tokens;
- one active job per worker initially;
- text-only requests;
- no tools, images, audio or user-provided grammars in the first release.

## 9. System architecture

```text
                         Solana localnet/devnet
                      +--------------------------+
                      | escrow + settlement log  |
                      +-------------+------------+
                                    |
                                    | signed settlement transaction
                                    |
+------------+   HTTPS/SSE   +------+-------+   outbound secure session   +-------------+
| API client | ------------> | coordinator  | <-------------------------- | Rust worker |
+------------+               +------+-------+                             +------+------+
                                    |                                            |
                                    | Postgres                                   | local IPC
                                    v                                            v
                              jobs/providers                              Colibri C engine
                              receipts/metrics                                  |
                                                                                v
                                                                         Mojo GPU backend
```

Inference payloads, model weights and generated text never go on-chain. Solana stores only the minimum settlement state and opaque job commitments.

## 10. Components

### 10.1 Coordinator

A Rust service responsible for:

- authenticating customers and providers;
- maintaining provider heartbeats and capabilities;
- validating model/runtime versions;
- accepting jobs and estimating cost;
- matching jobs to workers;
- leasing jobs with deadlines;
- receiving token streams and final receipts;
- running verification policy;
- retrying abandoned work;
- submitting devnet settlement transactions;
- exposing customer and operator APIs;
- emitting an auditable event trail.

The coordinator is intentionally centralized during the MVP. Its decisions must nevertheless be deterministic from recorded inputs so they can later be challenged or distributed.

### 10.2 Worker daemon

A Rust daemon installed by the provider. It:

- creates or imports a provider Ed25519 identity;
- collects static hardware information;
- runs signed benchmark challenges;
- establishes an outbound coordinator connection, avoiding inbound port forwarding;
- downloads signed runtime and model manifests;
- verifies hashes before execution;
- manages model storage and free-space limits;
- starts and supervises the inference subprocess;
- reports readiness, temperature, memory and benchmark status;
- executes leased jobs;
- streams output to the coordinator;
- records duration, tokens, bytes, cache statistics and energy telemetry;
- signs a completion receipt;
- observes provider-defined schedules and power limits.

The worker must never expose the provider’s wallet private key to the inference subprocess.

### 10.3 Inference engine

The engine is Colibri plus the optional Mojo backend. The MVP uses a local framed JSON or MessagePack protocol over stdin/stdout or a Unix-domain socket.

Required engine commands:

- `hello`: protocol and build capabilities;
- `load`: load a specific model manifest;
- `health`: memory, accelerator and model status;
- `infer`: submit one bounded request;
- `cancel`: stop the current request;
- `metrics`: retrieve timing/cache/I/O counters;
- `shutdown`: clean termination.

Every request carries a unique job ID and deterministic settings. Every response includes token IDs, not only decoded text.

### 10.4 Scheduler

The scheduler chooses among ready nodes that already have, or can load, the required model.

Hard constraints:

- exact model/runtime compatibility;
- sufficient RAM, accelerator memory and storage;
- provider is online and not already leased;
- customer latency region is allowed;
- provider price is within the customer ceiling;
- provider has passed recent benchmark and integrity challenges.

Initial score:

```text
score =
    0.35 * normalized_predicted_latency
  + 0.25 * normalized_job_price
  + 0.15 * recent_failure_rate
  + 0.10 * model_load_penalty
  + 0.10 * network_latency
  + 0.05 * energy_intensity
```

Lower is better. Weight changes must be versioned and included in the job audit log.

### 10.5 Verifier

The verifier decides whether work is payable.

MVP verification techniques:

- exact manifest and runtime commitments;
- deterministic greedy challenge prompts;
- hidden canary jobs with expected token IDs;
- random duplicate execution on another provider;
- plausibility checks on latency and reported hardware counters;
- provider reputation and recent challenge history;
- signed job leases and receipts.

The MVP should duplicate 5–10% of jobs. This is intentionally expensive enough to measure verification cost before attempting more sophisticated cryptographic proofs.

### 10.6 Solana settlement program

Use Rust with Anchor for the first on-chain program. Solana’s official documentation recommends Anchor for most Rust program development, and Anchor supports local validator and devnet workflows. See [Solana program execution](https://solana.com/docs/core/programs/program-execution) and [Anchor local development](https://www.anchor-lang.com/docs/quickstart/local).

On-chain accounts:

- `NetworkConfig`: administrator, settlement authority, fee basis points, pause flag.
- `ProviderAccount`: provider wallet, status, optional metadata commitment.
- `JobEscrow`: customer, assigned provider, opaque job hash, maximum payment, expiry, state.
- `SettlementRecord`: job hash, billed units, provider payment, platform fee, receipt hash.

Instructions:

- `initialize_network`;
- `register_provider`;
- `create_job_escrow`;
- `assign_provider`;
- `settle_job`;
- `cancel_unassigned`;
- `expire_job`;
- `pause_network`.

MVP trust assumption:

The coordinator is the settlement authority and signs accepted results. The chain prevents unilateral movement of escrow outside the program rules, but it does not independently verify inference. This limitation must be explicit.

Use localnet first and devnet second. Do not accept real-value funds or deploy to mainnet during the MVP.

## 11. Job lifecycle

```text
CREATED
  -> FUNDED
  -> QUEUED
  -> LEASED
  -> RUNNING
  -> RESULT_SUBMITTED
  -> VERIFYING
  -> SETTLED
```

Failure paths:

```text
QUEUED -> CANCELLED
LEASED/RUNNING -> LEASE_EXPIRED -> QUEUED
VERIFYING -> REJECTED -> QUEUED or FAILED
FUNDED -> EXPIRED -> REFUNDED
```

Rules:

- A lease has a monotonically increasing attempt number.
- Results from expired attempts are never payable.
- Retrying a job must not create a second escrow.
- Settlement is idempotent by job hash.
- A customer cancellation is allowed only before a valid lease begins.
- Customer-visible output is held until minimum verification passes.
- Every transition is recorded as an append-only coordinator event.

## 12. Provider lifecycle

```text
UNREGISTERED
  -> ENROLLED
  -> BENCHMARKING
  -> READY
  -> LEASED
  -> BUSY
  -> READY
```

Exceptional states:

- `DRAINING`: finish current job, accept no more.
- `QUARANTINED`: integrity or correctness failure.
- `OFFLINE`: missed heartbeat or clean disconnect.
- `UPDATE_REQUIRED`: incompatible runtime/model version.
- `COOLING`: thermal or provider-defined power limit reached.

## 13. Network protocol

Use HTTPS for customer APIs and one outbound persistent worker session per provider. WebSocket or HTTP/2 streaming is sufficient for the MVP; public peer discovery and direct customer-to-worker traffic are unnecessary.

All protocol messages include:

- protocol version;
- message ID;
- provider or job ID;
- timestamp and expiry;
- nonce;
- payload hash;
- Ed25519 signature where identity or payment state is affected.

Large model files are downloaded from content-addressed object storage or mirrors, not proxied through the coordinator.

## 14. API outline

Customer-facing endpoints:

```text
POST /v1/jobs
GET  /v1/jobs/{job_id}
GET  /v1/jobs/{job_id}/events
POST /v1/jobs/{job_id}/cancel
GET  /v1/models
GET  /v1/pricing/estimate
```

Operator endpoints:

```text
GET  /internal/providers
GET  /internal/providers/{provider_id}
POST /internal/providers/{provider_id}/challenge
POST /internal/providers/{provider_id}/quarantine
GET  /internal/jobs/{job_id}/audit
GET  /internal/metrics
```

The worker uses the persistent control session rather than public inbound endpoints.

## 15. Shared Rust protocol types

Representative types:

```rust
struct ModelManifest {
    model_id: String,
    revision: String,
    tokenizer_sha256: [u8; 32],
    weights_sha256: [u8; 32],
    runtime_build_id: String,
    quantization: Quantization,
}

struct WorkerCapabilities {
    provider_id: ProviderId,
    cpu: CpuInfo,
    accelerators: Vec<AcceleratorInfo>,
    ram_bytes: u64,
    storage: Vec<StorageInfo>,
    supported_backends: Vec<Backend>,
    benchmark: BenchmarkSummary,
}

struct JobLease {
    job_id: JobId,
    attempt: u32,
    model: ModelManifestId,
    input: InferenceInput,
    limits: InferenceLimits,
    deterministic: bool,
    expires_at: Timestamp,
    coordinator_signature: Signature,
}

struct CompletionReceipt {
    job_id: JobId,
    attempt: u32,
    provider_id: ProviderId,
    model_manifest: ModelManifestId,
    input_hash: [u8; 32],
    output_token_hash: [u8; 32],
    prompt_tokens: u32,
    completion_tokens: u32,
    started_at: Timestamp,
    completed_at: Timestamp,
    metering: MeteringSummary,
    worker_signature: Signature,
}
```

Sign canonical serialized bytes, never ordinary JSON text whose field ordering may change.

## 16. Proposed Rust workspace

```text
marketplace/
  Cargo.toml
  crates/
    protocol/                 shared IDs, messages and state machines
    crypto/                   canonical signing and hash commitments
    model-manifest/           content-addressed model metadata
    scheduler/                compatibility filters and scoring
    metering/                 usage and energy calculations
    chain-client/             coordinator Solana integration
  services/
    coordinator/              API, leases, verification, settlement
    worker/                   provider daemon
    engine-supervisor/        local Colibri process protocol
  programs/
    inference-escrow/         Anchor/Solana program
  tests/
    integration/              multi-worker local test environment
    chaos/                    disconnect, timeout and corruption scenarios
  deploy/
    local/                    local validator and coordinator configuration
  docs/
    protocol.md
    threat-model.md
    provider-guide.md
```

Likely Rust libraries, to be pinned during implementation:

- Tokio for asynchronous execution;
- Axum for HTTP and streaming APIs;
- Serde for bounded protocol serialization;
- SQLx with PostgreSQL for coordinator state;
- rustls for transport security;
- tracing and OpenTelemetry-compatible export for observability;
- Anchor for the Solana program;
- Solana Rust client crates for coordinator transactions.

Dependency versions must be locked and audited. The on-chain program should remain much smaller than the off-chain coordinator.

## 17. Model and runtime distribution

Every distributed artifact has a signed manifest:

- logical name and revision;
- licence identifier and source URL;
- expected file sizes and SHA-256 hashes;
- quantization format version;
- compatible engine protocol range;
- compatible backends and minimum memory;
- benchmark fixture hash;
- publisher signature.

The worker downloads into a temporary directory, verifies every file and atomically promotes the completed package. Partial packages are never visible to the engine.

The MVP may distribute model weights through existing object storage. Building a decentralized model CDN is out of scope.

## 18. Metering

Measure useful work rather than claimed GPU hours.

Required counters:

- prompt and completion tokens;
- time to first token;
- prefill and decode duration;
- wall-clock job duration;
- model load time;
- accelerator busy time where available;
- peak host and accelerator memory;
- expert cache hit/miss count;
- NVMe bytes read;
- retries and verification duplicates;
- average and peak board power where available;
- throttling or thermal-limit events.

Hardware telemetry is evidence, not proof. Consumer-controlled nodes can falsify it. Payouts must therefore be primarily tied to verified useful output and agreed price, with telemetry used for scheduling and economic analysis.

## 19. Transparent power-adjusted pricing

The customer should see how an estimate was constructed.

```text
predicted_seconds = predicted_tokens / benchmark_tokens_per_second

estimated_energy_kwh =
    estimated_system_watts * predicted_seconds / 3_600_000

estimated_energy_cost =
    estimated_energy_kwh * provider_electricity_price_per_kwh

quoted_price =
    base_job_fee
  + useful_token_fee
  + estimated_energy_cost
  + provider_margin
  + verification_reserve
  + platform_fee
```

The MVP uses provider-declared electricity price and benchmark-calibrated system power. Both values are displayed to the customer and retained in the audit record.

Provider payout should be based on the accepted quote, not retroactively changed by untrusted telemetry. Later versions may use signed smart-plug or platform telemetry where available.

Do not introduce a speculative reward token. Localnet/devnet SOL or non-transferable test credits are sufficient for validating accounting.

## 20. Reliability strategy

- Workers maintain outbound heartbeats every 10 seconds.
- A missed-heartbeat threshold marks the worker unavailable.
- Job leases include start and completion deadlines.
- The coordinator checkpoints streamed token IDs but treats only the final receipt as payable.
- Abandoned jobs return to the queue with a new attempt number.
- Providers can enter drain mode before shutdown or gaming use.
- Model-prewarmed workers are preferred over nodes that must load weights.
- Each provider has concurrency one until memory safety is demonstrated.
- Coordinator restarts reconstruct active leases from the database event log.

## 21. Security and threat model

Primary threats:

- fake hardware or benchmark results;
- fabricated inference output;
- cheaper or modified model substituted for the required model;
- replayed job receipts;
- provider disappears after accepting a lease;
- customer submits maliciously large or malformed input;
- compromised runtime package;
- theft of provider or coordinator wallet keys;
- prompt or output disclosure by a provider;
- Sybil providers colluding during duplicate verification;
- coordinator settles an invalid result.

MVP mitigations:

- invited providers with verified contact identity;
- signed provider challenges and short-lived job leases;
- immutable model/runtime manifests;
- bounded input sizes and timeouts;
- inference engine isolated as a low-privilege subprocess;
- no arbitrary customer code;
- duplicate jobs assigned across different operators when possible;
- append-only audit events;
- wallet keys isolated from worker inference processes;
- settlement amount and expiry enforced on-chain;
- operator pause switch for the devnet program.

Prompt confidentiality limitation:

TLS protects traffic in transit, but the provider machine normally sees plaintext prompts during inference. The MVP must prohibit sensitive personal, medical, legal, financial or proprietary data. Confidential inference requires trusted execution or cryptographic techniques beyond this MVP.

## 22. Observability

The coordinator exposes:

- ready/busy/offline/quarantined provider counts;
- queued/running/verifying/settled/failed job counts;
- dispatch and queue latency;
- time to first token and decode throughput by hardware/backend;
- lease expiry and retry rates;
- verification mismatch rate;
- quoted versus measured cost;
- devnet settlement success and confirmation latency;
- provider earnings and estimated energy cost;
- runtime/model version distribution.

Every job receives a trace ID propagated through API, scheduler, worker, inference engine, verifier and settlement transaction.

Logs must never contain plaintext prompts by default.

## 23. MVP user experience

### Provider

```text
$ colibri-worker enroll
$ colibri-worker benchmark
$ colibri-worker models install olmoe-1b-7b-q8
$ colibri-worker serve --schedule evenings --max-power 180
```

The provider dashboard shows:

- hardware and accepted benchmark tier;
- model readiness;
- current state and temperature;
- jobs completed and rejected;
- test-credit earnings;
- estimated electricity cost;
- projected net earnings;
- runtime updates and integrity warnings.

### Customer

```text
$ mojocolib submit prompts.jsonl \
    --model olmoe-1b-7b-q8 \
    --max-output 128 \
    --price-limit 0.10
```

The customer receives:

- quote breakdown;
- job and escrow identifiers;
- queue and execution status;
- streamed or batch results;
- backend/hardware tier without exposing provider identity;
- verification status;
- final receipt and devnet transaction.

## 24. Testing strategy

### Unit tests

- canonical serialization and signatures;
- job/provider state transitions;
- scheduler hard constraints and score ordering;
- pricing arithmetic and overflow;
- lease expiry and idempotent settlement;
- manifest path traversal and hash validation;
- receipt replay rejection.

### Integration tests

- coordinator plus two simulated workers;
- real worker plus CPU fixture engine;
- real Mojo/Colibri worker when available;
- local Solana validator escrow lifecycle;
- coordinator restart during running jobs;
- worker reconnect and lease recovery;
- duplicate verification and mismatch handling.

### Chaos tests

- worker killed during model load;
- network disconnect during streaming;
- corrupt model chunk;
- stale runtime version;
- deliberately slow or dishonest worker;
- coordinator restart before and after settlement submission;
- Solana RPC unavailable;
- repeated settlement transaction;
- thermal/power-limit transition during a job.

### Security tests

- forged provider signature;
- replayed lease and receipt;
- oversized and malformed requests;
- path traversal in model manifests;
- subprocess output injection;
- coordinator authorization bypass;
- on-chain account substitution and unauthorized settlement;
- secret scanning and dependency audit.

## 25. Acceptance criteria

The MVP is successful only if:

- 5–10 invited workers complete enrollment and benchmark challenges;
- at least three materially different accelerator configurations participate;
- one signed model package runs across every admitted compatible node;
- at least 1,000 bounded inference jobs are attempted;
- at least 98% of admitted jobs complete after automatic retries;
- p95 coordinator dispatch time is below one second for ready workers;
- prewarmed interactive jobs achieve p95 time to first token below five seconds on the trial network;
- deterministic verification jobs achieve at least 99.5% token agreement, with every mismatch classified;
- simulated dishonest results are detected at least 95% of the time under the selected challenge policy;
- duplicate verification consumes no more than 10% of total inference work;
- no job is paid twice and no completed escrow is stranded;
- provider earnings, estimated energy cost and net return can be calculated for every accepted job;
- a 30-minute continuous workload completes on every hardware tier without uncontrolled memory growth;
- all Solana settlement testing remains on localnet/devnet;
- prompt privacy limitations are enforced in UI and API documentation.

## 26. Delivery phases

### Phase 0 — Protocol and threat model

Deliverables:

- shared Rust protocol types;
- job/provider state machines;
- signed manifest and receipt formats;
- written threat model;
- mocked engine and chain interfaces.

Exit gate: two simulated workers can complete and retry jobs deterministically.

Estimated effort: 1–2 weeks.

### Phase 1 — Local Rust coordinator and worker

Deliverables:

- coordinator API and PostgreSQL state;
- outbound worker session;
- enrollment, heartbeat and leasing;
- subprocess supervisor;
- CPU fixture inference;
- structured metrics and audit events.

Exit gate: 100 local jobs complete with forced disconnect/retry tests and no duplicate acceptance.

Estimated effort: 2 weeks.

### Phase 2 — Colibri/Mojo inference integration

Deliverables:

- signed OLMoE model manifest;
- engine control protocol;
- Mojo/Metal worker and at least one NVIDIA worker;
- deterministic output receipts;
- model cache and atomic installation.

Exit gate: identical fixture jobs pass across admitted hardware and the real model completes the capability suite.

Estimated effort: 2–4 weeks, depending on Mojo backend readiness.

### Phase 3 — Trusted multi-provider trial

Deliverables:

- 5–10 invited providers;
- scheduler and price estimates;
- customer batch API/CLI;
- challenge and duplicate verification;
- provider dashboard or CLI status;
- churn and thermal testing.

Exit gate: 1,000-job acceptance criteria are met or failures are classified.

Estimated effort: 2–3 weeks.

### Phase 4 — Solana localnet/devnet settlement

Deliverables:

- Anchor escrow program;
- Rust chain client;
- idempotent settlement worker;
- local validator tests;
- devnet deployment and explorer-visible records;
- reconciliation report between coordinator ledger and chain.

Exit gate: every trial job’s accounting reconciles, including expiry, cancellation and retries.

Estimated effort: 2 weeks plus security review.

### Phase 5 — Economics and go/no-go report

Deliverables:

- throughput, latency and reliability by hardware;
- estimated energy and net provider return;
- verification overhead;
- customer cost comparison;
- privacy and operational findings;
- recommendation on permissionless beta, hardware expansion or project pause.

Estimated effort: 1 week after sufficient trial data.

Overall engineering range: approximately 10–14 focused weeks, with the Mojo backend on the critical path.

## 27. Team shape

Minimum practical roles:

- Rust distributed-systems engineer: coordinator, worker and protocol.
- Mojo/inference engineer: Colibri backend and cross-vendor kernels.
- Solana/Rust engineer: Anchor program, chain client and settlement tests.
- Security/reliability reviewer: threat model, chaos tests and key boundaries.
- Product/data owner: provider recruitment, pricing assumptions and trial analysis.

One person can prototype several roles, but the on-chain settlement and native inference boundaries require independent review before any real-value deployment.

## 28. Principal risks

| Risk | Consequence | MVP response |
|---|---|---|
| Residential nodes are unreliable | failed or slow jobs | leases, retries, warm-node preference, batch-first product |
| Cross-vendor output differs | verification false positives | deterministic q8 fixtures, token/logit diagnostics, backend-specific tolerances |
| Providers fake telemetry | distorted energy claims | pay for verified output; use telemetry only for estimates and scheduling |
| Duplicate verification is expensive | marketplace loses cost advantage | measure 5–10% sampling; adapt challenge rate by reputation |
| Model download is too large | slow enrollment and provider churn | one model, resumable content-addressed downloads, preflight storage checks |
| Mojo backend is immature | broken portability promise | process boundary, pinned compiler, CPU/C backend fallback, staged hardware admission |
| Legacy NVIDIA requires old tooling | maintenance and security burden | isolated toolchain, supported T4 control, legacy tier marked experimental |
| Customer prompts leak | trust and legal harm | non-sensitive workloads only; explicit policy; no prompt logging |
| Coordinator is compromised | false settlement or data exposure | minimal authority, key isolation, pause switch, devnet only |
| Token economics distract from product | speculation replaces useful demand | no new token; test credits/devnet only |
| Existing marketplaces dominate | weak differentiation | focus on unsupported hardware, deterministic integer inference, NVMe MoE and transparent economics |

## 29. Competitive context

The concept enters an existing category. Akash already lets providers list CPU/GPU capacity and earn from deployments, while io.net describes decentralized GPU aggregation and batch inference/model serving. See [Akash](https://akash.network/) and [io.net](https://io.net/docs/guides/getting-started).

The MVP therefore must demonstrate differentiation rather than merely reproduce a GPU marketplace:

- measurably useful old and modest GPUs;
- Apple, AMD and NVIDIA execution from a common Mojo-oriented backend;
- Colibri NVMe expert streaming for models larger than local accelerator memory;
- deterministic integer inference suitable for result verification;
- power-adjusted, auditable provider economics;
- a small, inspectable worker rather than arbitrary opaque containers.

## 30. Legal and operational review before real payments

Before mainnet or public enrollment, obtain advice on:

- marketplace and money-transmission obligations;
- sanctions and geographic restrictions;
- provider tax reporting;
- consumer electricity and equipment-warranty implications;
- model and tokenizer licences;
- Mojo/Modular redistribution terms;
- privacy law and data-processing roles;
- acceptable-use enforcement;
- liability for generated content;
- console and platform terms if partnerships are explored.

The MVP avoids most payment risk by using localnet/devnet and non-sensitive invited workloads.

## 31. Go/no-go decision

Proceed toward a permissionless beta only if:

- verified jobs are materially cheaper or access materially different hardware than hosted alternatives;
- provider net earnings remain positive after realistic electricity and failure rates;
- verification overhead stays below 10%;
- heterogeneous correctness is manageable;
- at least five providers voluntarily continue after the trial;
- customers value the supported workloads;
- security review finds no critical escrow or key-management issue.

Pause or reposition if:

- real-time latency remains unacceptable but batch workloads are viable;
- only modern NVIDIA cards are economical, eliminating the differentiation;
- cross-vendor output cannot be verified reliably;
- provider revenue depends on token subsidy rather than customer demand;
- model distribution and warm capacity dominate operating costs;
- privacy limitations eliminate the intended customer segment.

## 32. Recommended immediate next steps

1. Keep this marketplace as a separate workstream from the Mojo-Colibri kernel MVP.
2. Finish the deterministic q8/fused-MoE backend contract first.
3. Create the Rust workspace with protocol, coordinator, worker and mocked inference crates.
4. Write the threat model and state-machine tests before networking.
5. Run two local simulated workers with credits, not blockchain.
6. Integrate one real M2 worker through the subprocess protocol.
7. Add a T4 or other supported NVIDIA control worker.
8. Recruit the remaining trusted providers.
9. Add Solana localnet escrow only after off-chain job accounting is correct.
10. Deploy to devnet after local expiry, retry and double-settlement tests pass.

The first milestone is not “decentralized AI.” It is a boring, auditable system that sends one bounded inference job to one suitable computer, verifies the result, and accounts for it exactly. Everything more ambitious should be earned from that evidence.
