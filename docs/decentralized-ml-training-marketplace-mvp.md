# Decentralized ML training and pipeline marketplace — MVP brief

Status: primary product and engineering brief
Project: **MojoColib**
Initial network: 5–10 trusted provider machines
Primary implementation language: Rust
Training runtime: Python and PyTorch
Kernel/inference runtime: Mojo plus Colibri
Settlement: V2 signed pilot journal; future Solana localnet/devnet only

> [!NOTE]
> The current authority and settlement source of truth is
> [ADR-0010](adr/0010-network-topology-and-progressive-decentralisation.md)
> and [ADR-0011](adr/0011-platform-fee-and-settlement-policy.md). The Solana
> design later in this brief is a gated future target, not an implemented
> client, program, wallet flow or payment feature.

## 1. Executive summary

MojoColib is a proposed marketplace for running machine-learning pipelines on underused consumer and legacy hardware. Providers contribute idle gaming PCs, workstations, Apple Silicon machines or small local GPU servers. Customers submit reproducible pipelines containing data preparation, fine-tuning, experiment sweeps, evaluation, quantization and inference stages. Providers earn test credits during the MVP for completing verified pipeline stages.

The network will not attempt to make unrelated residential GPUs behave like one tightly connected supercomputer. Wide-area synchronized training is a poor first workload because gradient exchange is latency-sensitive and one unstable participant can restart the entire training group. Instead, the MVP schedules **complete, checkpointable stages** onto individual providers:

- dataset preprocessing and sharding;
- embedding or synthetic-data generation;
- single-node LoRA fine-tuning;
- independent hyperparameter and seed sweeps;
- evaluation and red-team suites;
- quantization and model conversion;
- final inference and serving benchmarks.

Rust provides the control plane: pipeline state, scheduling, provider identity, artifact provenance, worker supervision, metering and later V2 settlement integration. Python/PyTorch remains the training runtime because that ecosystem already supplies autograd, optimizers, datasets and parameter-efficient fine-tuning. Mojo supplies selected portable kernels and the Colibri inference path, but it is not treated as the sole training framework.

The MVP remains centralized where trust is still unsolved. Providers are invited, the coordinator schedules stages, artifacts live in ordinary object storage, and a coordinator authority approves valueless test-credit settlement. The experiment’s purpose is to prove useful work, reproducibility, recovery and provider economics before attempting anonymous participation, decentralized scheduling or a settlement rail.

## 2. Revised product thesis

The strongest market is broader than inference. AI development contains many expensive but naturally separable jobs:

- preparing and validating datasets;
- running many independent fine-tuning trials;
- evaluating checkpoints across task suites;
- generating synthetic examples;
- computing embeddings;
- converting, pruning and quantizing models;
- benchmarking final artifacts on different hardware.

These jobs map better to geographically distributed consumer hardware than per-token model parallelism. They can be checkpointed, retried, compared and paid after artifact verification.

The hypothesis is:

> A heterogeneous network can reduce the cost of practical ML experimentation if work is divided at pipeline-stage and experiment-trial boundaries, rather than synchronizing every training step across residential nodes.

## 3. Differentiation

The MVP should test six differentiators:

1. **Old and overlooked GPUs** — admit hardware by measured capability instead of product generation.
2. **Apple, AMD and NVIDIA** — use platform-specific training runtimes today and a Mojo-oriented kernel layer where it is technically mature.
3. **Checkpoint-first pipelines** — design every expensive stage to survive interruption and reassignment.
4. **NVMe-aware model workflows** — use local model caches and Colibri streaming for evaluation/inference artifacts larger than accelerator memory.
5. **Reproducible integer inference** — verify final checkpoints across hardware using fixed quantization and deterministic test suites.
6. **Power-adjusted economics** — quote completed work using benchmark speed, estimated energy and artifact value, not a claimed GPU-hour alone.

Existing decentralized compute markets validate demand, but the MVP must demonstrate a more accessible and reproducible ML workflow rather than another generic container auction. Akash already operates a decentralized compute marketplace, and io.net describes GPU aggregation for ML workloads. See [Akash](https://akash.network/) and [io.net](https://io.net/docs/guides/getting-started).

## 4. Why Rust, Python and Mojo each have a role

### Rust: control and trust plane

Rust owns:

- coordinator APIs and the pipeline scheduler;
- worker daemon and process supervision;
- provider identity and signed sessions;
- artifact manifests and hash verification;
- pipeline and stage state machines;
- checkpoint upload/download orchestration;
- metering, pricing and signed receipts;
- the Solana client and Anchor program;
- audit logging and reliability tooling.

Rust is well suited to long-running network services and explicit security boundaries. It also integrates cleanly with Solana programs, which are primarily written in Rust; Solana recommends Anchor for most program development. See [Solana program execution](https://solana.com/docs/core/programs/program-execution).

### Python/PyTorch: training plane

Python/PyTorch owns:

- autograd and optimizer execution;
- dataset loading and transforms;
- training loops and checkpoint serialization;
- LoRA/PEFT and optional QLoRA;
- experiment metrics;
- model evaluation;
- local multi-GPU DDP where a single provider owns a stable cluster.

Parameter-efficient fine-tuning updates a small adapter rather than every model parameter, greatly reducing trainable state and checkpoint size. This makes LoRA a natural first marketplace workload. See the [Hugging Face PEFT documentation](https://huggingface.co/docs/transformers/peft).

### Mojo/Colibri: kernel and inference plane

Mojo owns selected accelerator kernels only after a measured need is established:

- quantized matrix operations;
- preprocessing or postprocessing kernels;
- deterministic q8 inference kernels;
- fused MoE evaluation and serving;
- portable Metal/CUDA/HIP experiments.

Colibri owns the final quantized MoE evaluation/inference path and NVMe expert streaming.

This division is intentionally honest. Current Modular documentation presents MAX primarily as a model-serving and inference framework, with Mojo for custom operations and kernels. The MVP should not assume a complete Mojo autograd/training ecosystem that is not yet documented as production-ready. See [Modular’s platform overview](https://docs.modular.com/stable/max/intro/) and [MAX model development](https://docs.modular.com/stable/max/develop/).

## 5. What the MVP will run

### Tier 1: data and evaluation stages

- JSONL/Parquet validation and normalization;
- deduplication and filtering;
- tokenization and length statistics;
- deterministic dataset sharding;
- embedding generation;
- checkpoint evaluation;
- safety and regression prompt suites;
- quantization and artifact conversion.

These stages are easiest to retry and verify and should be implemented first.

### Tier 2: independent training stages

- LoRA supervised fine-tuning on one provider;
- small vision-classifier fine-tuning;
- independent hyperparameter trials;
- independent random-seed trials;
- learning-rate range tests;
- ablation experiments;
- small reward-model or classifier training later.

### Tier 3: provider-local distributed training

A provider with multiple locally connected GPUs may advertise a local cluster and run PyTorch DistributedDataParallel or TorchElastic internally. The marketplace treats the cluster as one provider resource. PyTorch recommends DistributedDataParallel for multi-GPU training and provides Torch Distributed Elastic for restart and membership handling. See [PyTorch DDP](https://docs.pytorch.org/docs/stable/notes/ddp.html) and [Torch Distributed Elastic](https://docs.pytorch.org/docs/stable/distributed.elastic.html).

### Excluded: cross-provider synchronous training

The MVP will not run DDP, FSDP or tensor parallelism across unrelated homes. Those approaches repeatedly exchange gradients or activations and are sensitive to latency, bandwidth and membership changes. TorchElastic improves failure recovery, but failures can still restart the worker group from a checkpoint. Cross-provider distributed training remains a research lane after the marketplace proves checkpointable independent work.

## 6. Reference MVP pipelines

### Pipeline A — dataset preparation

```text
ingest public dataset
  -> validate schema
  -> deduplicate
  -> filter and redact
  -> tokenize
  -> produce deterministic train/validation shards
  -> publish dataset manifest
```

Purpose: prove CPU/GPU stage execution, content-addressed artifacts, retries and provenance without introducing training variability.

### Pipeline B — LoRA experiment sweep

```text
dataset manifest
  -> launch 8 independent LoRA trials
  -> checkpoint every N optimizer steps
  -> evaluate every checkpoint
  -> rank by primary and guardrail metrics
  -> verify the winning adapter on a second provider
  -> publish adapter + model card + provenance manifest
```

Purpose: prove that heterogeneous providers can execute useful independent experiments and that the network can select a verified artifact.

### Pipeline C — quantize and benchmark

```text
winning adapter
  -> merge or load adapter
  -> export safetensors
  -> quantize q8/q4 variants
  -> evaluate quality delta
  -> benchmark on Apple, AMD and NVIDIA workers
  -> publish hardware/cost matrix
```

Purpose: connect training to the Mojo/Colibri differentiation and compare deployment economics.

### Pipeline D — synthetic-data loop

```text
seed examples
  -> distributed generation batches
  -> rule/model-based filtering
  -> deduplication
  -> human sample review
  -> LoRA fine-tuning
  -> held-out evaluation
```

Purpose: evaluate a full ML pipeline where inference, data processing and training are each schedulable stages.

## 7. Initial model choice

Use two different artifacts for two different purposes.

### Training reference model

Use a small openly licensed transformer that can be fine-tuned in ordinary FP16/BF16 LoRA mode on the lowest admitted GPU tier. The implementation phase must lock one exact model, revision and licence after benchmarking candidate memory requirements.

Selection constraints:

- <=2B parameters for the first cross-vendor LoRA trial;
- permissive redistribution and fine-tuning licence;
- supported by current PyTorch Transformers/PEFT;
- adapter checkpoints below 500 MB;
- no custom remote code;
- deterministic tokenizer and evaluation fixtures;
- fits the Apple M2 16 GiB reference machine without swap.

Do not make QLoRA mandatory for the first cross-vendor test because quantized training backends differ by vendor. Establish ordinary LoRA first; add QLoRA as an optional NVIDIA/AMD capability tier.

### Evaluation/inference reference model

Retain `allenai/OLMoE-1B-7B-0125-Instruct` for the Colibri/Mojo evaluation and streaming track. It tests sparse MoE execution and NVMe expert streaming without forcing the training MVP to implement MoE backpropagation.

## 8. MVP goals

The MVP must prove or disprove:

- pipelines can be represented as immutable, typed DAGs;
- stages can be assigned to heterogeneous providers safely;
- datasets, checkpoints and metrics remain content-addressed and reproducible;
- a failed training stage resumes from its last accepted checkpoint;
- independent sweep trials can run concurrently across providers;
- the winning checkpoint can be evaluated by a different provider;
- provider compensation can be tied to accepted artifacts and useful work;
- Solana devnet can represent pipeline escrow and stage settlement;
- the combined cost is competitive with ordinary hosted experimentation;
- providers earn more than their estimated incremental energy cost.

## 9. Non-goals

The MVP will not:

- train a foundation model from scratch;
- synchronize training steps across residential providers;
- accept arbitrary unreviewed customer containers;
- process private or regulated datasets;
- support anonymous providers;
- use real money or issue a token;
- guarantee confidential computation;
- run retail PlayStation or Xbox nodes;
- support every ML framework;
- replace PyTorch with Mojo;
- build a decentralized artifact CDN;
- execute unbounded customer Python;
- expose provider machines directly to customers.

## 10. Target users

### ML customer

The initial user is a researcher, indie developer or small ML team running public/non-sensitive experiments:

- LoRA fine-tuning;
- prompt and dataset evaluation;
- hyperparameter sweeps;
- synthetic-data generation;
- quantization comparisons;
- multi-hardware deployment benchmarks.

### Compute provider

The initial provider owns:

- a supported Windows/Linux PC or Apple Silicon Mac;
- a GPU with sufficient memory for at least one admitted stage type;
- local SSD/NVMe space for model and dataset caches;
- reliable broadband;
- permission to run sustained compute;
- a provider-defined availability and power budget.

## 11. System architecture

```text
                           Solana localnet/devnet
                       +---------------------------+
                       | pipeline escrow + records |
                       +-------------+-------------+
                                     |
                                     |
+-------------+        +-------------+-------------+        +----------------+
| Rust/Python | -----> | Rust coordinator/scheduler| <----- | Rust workers   |
| customer SDK|        +------+------+-------------+        +--------+-------+
+-------------+               |      |                               |
                              |      |                               | supervises
                              |      +------------------+            v
                              |                         |     Python/PyTorch job
                              v                         v            |
                         PostgreSQL               object storage     +-- checkpoints
                      events/leases/state     datasets/checkpoints   +-- metrics
                                                                         |
                                                                         v
                                                              Mojo/Colibri evaluation
```

The coordinator is the control plane. Object storage is the data plane. Solana is the settlement plane. Provider machines are the execution plane.

## 12. Pipeline specification

A pipeline is an immutable DAG with typed inputs and outputs.

Each stage declares:

- stage ID and implementation template;
- runtime image/environment digest;
- command and bounded parameters;
- input artifact manifests;
- expected output types;
- CPU, RAM, GPU, VRAM and storage minimums;
- permitted accelerator vendors/backends;
- estimated duration and maximum wall time;
- checkpoint interval and resume contract;
- verification policy;
- maximum quote and retry count;
- network-access policy;
- deterministic seed policy;
- licence and data-classification declarations.

Example:

```yaml
pipeline: lora-sweep-v1
inputs:
  dataset: sha256:...
  base_model: sha256:...
stages:
  - id: train-r16-lr1
    template: hf-peft-sft-v1
    needs:
      gpu_memory_gb: 10
      system_memory_gb: 16
      storage_gb: 20
    params:
      seed: 101
      lora_rank: 16
      learning_rate: 0.0002
      max_steps: 500
    checkpoint_every_steps: 50
    outputs: [adapter, trainer_state, metrics]
  - id: evaluate-r16-lr1
    needs: [train-r16-lr1]
    template: eval-suite-v1
    outputs: [metrics, predictions]
```

Only administrator-approved stage templates are permitted in the MVP.

## 13. Rust coordinator

Responsibilities:

- authenticate customers and providers;
- validate pipeline DAGs and manifests;
- create pipeline escrow and budget reservations;
- maintain provider heartbeats/capabilities;
- schedule runnable stages;
- issue signed stage leases;
- track checkpoints and resumability;
- receive metrics and final artifact receipts;
- apply verification policy;
- retry, reassign, quarantine or settle stages;
- reconcile the internal ledger with Solana;
- expose pipeline status and audit data.

The coordinator persists an append-only event for every state transition. Material decisions must be reconstructable from events, manifests and scheduler-version inputs.

## 14. Rust worker daemon

Responsibilities:

- provider enrollment and Ed25519 identity;
- hardware and software inventory;
- signed benchmark challenges;
- outbound coordinator session;
- content-addressed cache management;
- artifact download and hash verification;
- Python environment/container launch;
- process, resource and timeout supervision;
- checkpoint upload;
- log and metric collection;
- final artifact manifest generation;
- signed stage receipt;
- local schedule, thermal and power controls.

The worker never gives customer code direct access to provider wallet keys or the coordinator control session.

## 15. Training executor

The training executor is a constrained Python subprocess launched by the Rust worker.

Initial approved templates:

- `dataset-transform-v1`;
- `hf-peft-sft-v1`;
- `hf-evaluate-v1`;
- `embedding-batch-v1`;
- `quantize-export-v1`;
- `colibri-benchmark-v1`.

Linux providers should use pinned rootless OCI images where hardware support allows it. Apple workers should use a signed, locked Python environment managed separately from the user’s global Python installation. Because macOS isolation is weaker than a Linux container, the MVP remains invite-only and accepts only signed stage templates.

Every training template must:

- set all available random seeds;
- write an environment manifest;
- report dataset/model revisions;
- checkpoint optimizer, scheduler and RNG state;
- flush metrics periodically;
- respond to graceful cancellation;
- upload checkpoints atomically;
- produce a machine-readable completion summary.

## 16. Artifact and dataset plane

Use S3-compatible object storage with presigned, short-lived URLs.

Artifact types:

- raw dataset references;
- normalized dataset shards;
- tokenizer outputs;
- base model manifests;
- adapters and full checkpoints;
- optimizer/trainer state;
- metrics and predictions;
- quantized model variants;
- evaluation and benchmark reports.

Every artifact manifest contains:

- SHA-256 digest and byte size;
- logical media/type identifier;
- producing pipeline/stage/attempt;
- input-manifest hashes;
- runtime/environment digest;
- provider signature;
- coordinator acceptance signature;
- licence and data classification;
- creation time and retention policy.

Providers download directly from object storage and upload to an attempt-specific temporary key. The coordinator promotes an artifact only after size/hash/type validation.

## 17. Pipeline and stage state machines

Pipeline:

```text
DRAFT -> VALIDATED -> FUNDED -> RUNNING -> VERIFYING -> COMPLETED
                     |           |            |
                     v           v            v
                  CANCELLED    FAILED       REJECTED
```

Stage:

```text
BLOCKED -> READY -> LEASED -> PREPARING -> RUNNING
                                      -> CHECKPOINTED
                                      -> ARTIFACT_SUBMITTED
                                      -> VERIFYING
                                      -> ACCEPTED
```

Failure paths:

```text
LEASED/PREPARING/RUNNING -> LEASE_EXPIRED -> READY
RUNNING -> CHECKPOINTED -> READY on another compatible provider
VERIFYING -> REJECTED -> READY or FAILED
```

Rules:

- every attempt has a unique monotonically increasing attempt number;
- only artifacts from the active attempt are eligible for acceptance;
- checkpoint promotion is idempotent;
- an accepted stage cannot be paid twice;
- downstream stages consume only accepted artifact manifests;
- retry budgets are fixed when the pipeline is funded;
- nondeterministic stages must declare comparison tolerances.

## 18. Checkpoint-first reliability

Training jobs are longer than inference jobs, so checkpointing is a product requirement rather than an optimization.

Each training template declares:

- maximum steps between checkpoints;
- maximum minutes of acceptable lost work;
- checkpoint contents;
- resume compatibility rules;
- expected checkpoint size;
- upload timeout;
- validation command.

The worker should request drain mode before the provider’s gaming schedule begins. The training subprocess writes a local checkpoint, the worker uploads it, and the coordinator marks it accepted before releasing the lease.

PyTorch’s elastic tooling can restart worker groups and expects training scripts to save/load checkpoints; this informs provider-local multi-GPU support, but marketplace-level recovery remains stage/checkpoint based. See [TorchElastic training scripts](https://docs.pytorch.org/docs/stable/elastic/train_script.html).

## 19. Scheduling

Hard filters:

- approved stage template and compatible platform;
- exact model/runtime availability;
- sufficient RAM/VRAM/storage;
- required precision and training backend;
- provider availability window exceeds minimum lease duration;
- dataset geography/policy permits the provider region;
- provider quote is within stage budget;
- benchmark and reliability thresholds pass;
- no active quarantine or thermal cooldown.

Initial score, lower is better:

```text
score =
    0.25 * predicted_completion_time
  + 0.20 * quoted_stage_cost
  + 0.15 * recent_failure_rate
  + 0.10 * checkpoint_resume_risk
  + 0.10 * artifact_download_penalty
  + 0.10 * energy_intensity
  + 0.05 * verification_risk
  + 0.05 * network_latency
```

Sweep-aware scheduling should deliberately spread independent trials across different provider operators when this improves fault independence and later comparison.

## 20. Verification and reproducibility

Verifying training is harder than verifying deterministic inference. A checkpoint hash proves which bytes were produced, not that useful training occurred.

MVP verification combines:

- signed environment, dataset and model manifests;
- bounded training scripts controlled by the platform;
- complete loss/metric traces;
- checkpoint structure and finite-value validation;
- adapter parameter norms and update statistics;
- evaluation on a coordinator-selected hidden validation subset;
- re-evaluation of final artifacts on a second provider;
- short hidden canary training jobs with known expected ranges;
- random duplicate execution of small trials;
- reputation and stake/test-credit consequences.

Acceptance is based on declared criteria, not merely lower training loss. A stage may require:

- successful checkpoint load;
- exact dataset/example count;
- no NaN/Inf values;
- metric threshold or confidence interval;
- deterministic predictions for a fixed fixture where applicable;
- downstream evaluation completion;
- no prohibited network access.

The MVP does not claim cryptographic proof of learning. The coordinator remains a trusted verifier.

## 21. Metering

Training-stage counters:

- examples and tokens processed;
- optimizer steps;
- forward/backward/optimizer duration;
- checkpoint count and upload bytes;
- accelerator busy time;
- average/peak accelerator memory;
- system RAM and local storage peak;
- average/peak board power where available;
- estimated whole-system power;
- artifact download/upload bytes;
- retries and lost work since last checkpoint;
- evaluation metrics and completion state.

Telemetry is useful for scheduling and analysis but remains provider-controlled. Payments must depend on accepted stage outputs and agreed quotes, not unverified wattage alone.

## 22. Power-adjusted stage pricing

```text
predicted_hours =
    predicted_training_units / benchmark_units_per_hour

estimated_energy_kwh =
    estimated_system_watts * predicted_hours / 1000

estimated_energy_cost =
    estimated_energy_kwh * provider_electricity_price_per_kwh

stage_quote =
    base_stage_fee
  + useful_work_fee
  + estimated_energy_cost
  + checkpoint_storage_reserve
  + verification_reserve
  + provider_margin
  + platform_fee
```

Useful work units depend on the stage:

- training: accepted optimizer steps or tokens/examples processed;
- preprocessing: accepted input bytes/examples transformed;
- embeddings: accepted vectors;
- evaluation: accepted examples/checkpoints evaluated;
- quantization: accepted artifact conversion and validation;
- inference: accepted output tokens.

The accepted quote caps payout. A stage that fails verification receives no completion payment, but the MVP should separately measure whether limited checkpoint compensation is necessary to make honest providers willing to accept long jobs.

## 23. Future Solana settlement (gated)

Only after the V2 coordinator, replay/reconciliation, independent security and
operational gates in [ADR-0011](adr/0011-platform-fee-and-settlement-policy.md)
are met may Rust and Anchor be evaluated on localnet first and devnet second.
No listed account or instruction below currently exists in this repository.

Accounts:

- `NetworkConfig`;
- `ProviderAccount`;
- `PipelineEscrow`;
- `StageReservation`;
- `StageSettlement`.

Instructions:

- `initialize_network`;
- `register_provider`;
- `create_pipeline_escrow`;
- `reserve_stage_budget`;
- `assign_stage_provider`;
- `settle_stage`;
- `release_unused_budget`;
- `cancel_or_expire_pipeline`;
- `pause_network`.

On-chain data contains only opaque hashes, wallets, limits, states and amounts. Datasets, model names, metrics and checkpoints remain off-chain.

The coordinator is the settlement authority in the MVP's valueless test-credit ledger. A future Solana program could enforce escrow movement and provide an audit trail, but it would not verify training. No mainnet deployment or real-value payment is allowed during MVP testing.

## 24. Security and data policy

The MVP accepts only:

- public datasets;
- synthetic datasets;
- openly redistributable models;
- non-sensitive prompts and evaluation cases;
- signed platform templates.

Primary threats:

- poisoned or malicious dataset;
- model or dependency supply-chain compromise;
- provider returns fabricated checkpoint;
- customer attempts arbitrary code execution;
- provider accesses or republishes data;
- forged artifact/receipt;
- checkpoint replay from another job;
- worker tampers with metrics;
- coordinator settles a rejected stage;
- wallet or signing-key theft;
- Sybil providers collude on verification.

MVP mitigations:

- invitation-only providers;
- content-addressed artifacts;
- signed runtime templates;
- no unrestricted shell or arbitrary container upload;
- network egress disabled by default inside stages;
- bounded CPU/RAM/GPU/storage/time;
- worker/inference keys separated from wallets;
- hidden validation data disclosed only when needed;
- second-provider evaluation;
- append-only audit events;
- localnet/devnet settlement only.

Provider machines see plaintext training data and model weights during execution. Confidential or proprietary training is not supported without trusted execution or other privacy technology.

## 25. Provider hardware tiers

Workers advertise capabilities rather than a universal “GPU” category.

Example tiers:

- `data-cpu`: dataset transforms and evaluation orchestration;
- `train-small`: <=2B LoRA training;
- `train-quantized`: vendor-supported QLoRA;
- `train-local-multi-gpu`: stable provider-owned DDP cluster;
- `embed-gpu`: embedding and synthetic-data batches;
- `quantize`: model conversion and validation;
- `colibri-stream`: NVMe-backed MoE evaluation/inference;
- `legacy-experimental`: older GPU accepted through explicit fixtures.

The Tesla T4 is a useful supported NVIDIA control. A Tesla P40 is a legacy q8/inference experiment, but its Pascal toolchain limitations make it a poor first cross-vendor training baseline. Apple M-series and AMD workers should initially receive only templates verified for their native PyTorch backend.

## 26. Network protocol and APIs

Customer API:

```text
POST /v1/pipelines/validate
POST /v1/pipelines
GET  /v1/pipelines/{pipeline_id}
GET  /v1/pipelines/{pipeline_id}/events
GET  /v1/pipelines/{pipeline_id}/artifacts
POST /v1/pipelines/{pipeline_id}/cancel
GET  /v1/templates
GET  /v1/pricing/estimate
```

Provider sessions are outbound and persistent. Messages contain protocol version, nonce, expiry, payload hash and signatures where identity or money is affected.

Large artifacts always move through object storage; they do not traverse the coordinator process.

## 27. Proposed Rust workspace

```text
marketplace/
  Cargo.toml
  crates/
    protocol/                 IDs, messages, DAG and state machines
    crypto/                   signing and canonical commitments
    manifests/                dataset/model/checkpoint manifests
    scheduler/                filters, scoring and reservations
    metering/                 useful-work and energy calculations
    chain-client/             Solana transactions/reconciliation
    artifact-client/          content-addressed object-store client
  services/
    coordinator/              API, scheduler, verification, settlement
    worker/                   provider daemon
    executor-supervisor/      Python/native subprocess control
  programs/
    ml-pipeline-escrow/       Anchor program
  python/
    colibri_mesh_sdk/         pipeline authoring/client SDK
    templates/                approved training/eval templates
  tests/
    integration/
    chaos/
    security/
```

Likely Rust libraries, pinned during implementation:

- Tokio;
- Axum;
- Serde;
- SQLx/PostgreSQL;
- rustls;
- tracing/OpenTelemetry-compatible export;
- Anchor and Solana Rust client crates.

Likely Python libraries, pinned per template:

- PyTorch;
- Transformers;
- PEFT;
- Datasets;
- Evaluate or template-specific metric packages;
- safetensors.

## 28. Observability

Pipeline metrics:

- pipelines/stages by state;
- queue time and scheduling failures;
- stage duration prediction error;
- checkpoint interval and lost-work duration;
- artifact transfer time and cache hit rate;
- retries, provider churn and quarantine rate;
- verification mismatch rate;
- metric distribution across duplicate trials;
- quoted versus measured cost;
- estimated provider energy and net return;
- Solana reconciliation state.

Every pipeline, stage and attempt receives a trace ID propagated through coordinator, worker, training subprocess, artifact operations, verifier and settlement.

Dataset rows, prompts and model outputs are excluded from default logs.

## 29. Testing strategy

### Unit

- DAG validation and cycle rejection;
- state transitions and retry budgets;
- canonical manifest serialization/signatures;
- scheduler compatibility and score ordering;
- quote arithmetic and overflow;
- checkpoint promotion/idempotency;
- artifact type/size/hash validation;
- receipt and settlement replay prevention.

### Integration

- coordinator plus simulated workers;
- real Rust worker plus tiny PyTorch training template;
- checkpoint interruption and resume on a second worker;
- sweep fan-out and evaluation fan-in;
- object-store upload/promotion;
- local Solana validator lifecycle;
- one real Apple and one real NVIDIA worker;
- Colibri evaluation of an accepted artifact where compatible.

### Chaos

- disconnect during checkpoint upload;
- provider shutdown between optimizer steps;
- corrupt dataset shard;
- full local disk;
- GPU out-of-memory;
- thermal cooldown;
- coordinator restart;
- object store unavailable;
- Solana RPC unavailable;
- duplicate/stale stage receipt;
- dishonest metrics and fabricated artifact.

### ML correctness

- deterministic tiny-model fixture;
- checkpoint load and resume equivalence;
- loss decreases within expected range on hidden canary data;
- adapter changes only permitted modules;
- evaluation reproducibility across providers;
- quantization quality delta within declared bound;
- final inference manifest matches accepted checkpoint lineage.

## 30. MVP acceptance criteria

The MVP succeeds only if:

- 5–10 invited providers enroll and pass benchmarks;
- at least three materially different hardware configurations participate;
- at least 50 complete pipelines and 500 stages are attempted;
- >=95% of valid stages complete after automatic retries;
- every forced training interruption resumes from the last accepted checkpoint;
- lost work is below the template’s declared maximum interval;
- all accepted artifacts have complete, valid provenance manifests;
- eight-trial LoRA sweeps can fan out across at least three providers and converge into one verified winner;
- final adapters reload and evaluate successfully on a different provider;
- simulated fabricated checkpoints are detected >=95% of the time under the chosen policy;
- verification/re-execution consumes <=15% of total useful compute;
- no stage is paid twice and no completed pipeline escrow is stranded;
- quoted cost, estimated energy, provider payout and platform fee reconcile exactly;
- providers receive a credible net-return report after electricity estimates;
- all payment tests remain on localnet/devnet;
- no private dataset is admitted.

## 31. Delivery phases

### Phase 0 — protocol, DAG and threat model

- Rust protocol crate;
- pipeline/stage state machines;
- artifact and receipt manifests;
- threat model;
- mocked workers and object store.

Gate: simulated pipelines retry and settle idempotently.

Estimated effort: 1–2 weeks.

### Phase 1 — data pipeline

- coordinator and worker;
- approved dataset transform template;
- artifact plane;
- stage scheduling and checkpoints;
- audit/observability.

Gate: 100 data stages complete under injected failures.

Estimated effort: 2–3 weeks.

### Phase 2 — single-node LoRA

- pinned PyTorch/PEFT template;
- small reference model and dataset;
- checkpoint/resume;
- metrics and hidden evaluation;
- provider hardware admission.

Gate: a training stage resumes on another compatible provider and produces an accepted adapter.

Estimated effort: 2–4 weeks.

### Phase 3 — experiment sweeps

- DAG fan-out/fan-in;
- parallel hyperparameter trials;
- evaluation and ranking;
- duplicate/canary verification;
- pricing and provider reports.

Gate: eight-trial sweep acceptance criteria pass.

Estimated effort: 2–3 weeks.

### Phase 4 — Mojo/Colibri evaluation pipeline

- quantization/conversion template;
- Mojo/Colibri benchmark stage;
- Apple/NVIDIA/AMD result matrix where hardware exists;
- NVMe streaming measurements;
- reproducible deployment artifact.

Gate: the accepted model lineage reaches a verified multi-hardware inference report.

Estimated effort: 2–4 weeks, dependent on the Mojo backend.

### Phase 5 — Solana devnet settlement

- Anchor pipeline escrow;
- stage reservations and settlements;
- Rust reconciliation worker;
- local validator and devnet tests;
- expiry, cancellation and replay security tests.

Gate: internal and on-chain ledgers reconcile for all trial pipelines.

Estimated effort: 2 weeks plus independent review.

### Phase 6 — trusted provider trial and economics

- 5–10 provider network;
- 50 pipelines/500 stages;
- chaos/fraud simulations;
- provider retention and earnings analysis;
- hosted-compute cost comparison;
- go/no-go report.

Estimated effort: 2–3 weeks after technical readiness.

Overall range: approximately 13–19 focused weeks. The largest uncertainties are cross-platform training environments, artifact verification and the Mojo evaluation backend.

## 32. Go/no-go criteria

Proceed toward a broader beta only if:

- useful pipeline stages are cheaper or access meaningfully different hardware than alternatives;
- checkpoint recovery keeps lost work within the declared bound;
- accepted training artifacts reproduce across providers;
- verification overhead remains <=15%;
- provider net return is positive without token subsidy;
- customers value sweep, data or fine-tuning throughput;
- security review finds no critical artifact, executor or escrow issue.

Reposition toward batch inference/evaluation if training environments prove too heterogeneous but inference remains economical.

Pause if:

- provider churn repeatedly destroys long-running work;
- only modern data-centre NVIDIA hardware is reliable;
- useful outputs cannot be verified affordably;
- data privacy eliminates the target workloads;
- provider returns depend on speculative rewards;
- object storage and artifact transfer erase the compute saving.

## 33. Recommended immediate sequence

1. Treat this document as the primary marketplace brief; retain the inference brief as a companion serving module.
2. Define the Rust DAG, stage lease, artifact manifest and checkpoint receipt formats.
3. Build a simulated worker and tiny deterministic PyTorch training fixture.
4. Implement the dataset-preparation pipeline before GPU training.
5. Run one LoRA template locally with forced interruption/resume.
6. Move the resumed stage between two trusted compatible workers.
7. Add sweep fan-out and second-provider evaluation.
8. Integrate the Mojo/Colibri evaluation stage after its kernel contract is stable.
9. Add Solana localnet only after off-chain stage accounting is exact.
10. Recruit 5–10 providers and run the measured trial.

The first convincing demonstration is not thousands of anonymous GPUs training one giant model. It is a reproducible pipeline in which eight useful experiments run on several ordinary machines, survive failures, produce one independently verified checkpoint, and pay each provider exactly for accepted work.
