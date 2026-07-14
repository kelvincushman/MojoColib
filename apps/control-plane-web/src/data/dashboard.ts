export type DashboardView = "overview" | "pipelines" | "providers" | "artifacts" | "verification" | "credits"

export type Tone = "neutral" | "info" | "success" | "warning" | "danger"

export type PipelineState = "draft" | "validated" | "funded" | "running" | "draining" | "completed" | "failed" | "cancelled"

export type StageState = "pending" | "runnable" | "leased" | "running" | "verifying" | "retryable" | "draining" | "accepted" | "failed" | "cancelled"

export type ProviderState = "enrolling" | "benchmarking" | "ready" | "busy" | "draining" | "offline" | "quarantined" | "update required" | "cooling"

export type VerificationState = "accepted" | "rejected" | "inconclusive"

export type ArtifactState = "candidate" | "accepted" | "rejected"

export interface StatusMeta {
  label: string
  tone: Tone
}

export const pipelineStatus: Record<PipelineState, StatusMeta> = {
  draft: { label: "Draft", tone: "neutral" },
  validated: { label: "Validated", tone: "info" },
  funded: { label: "Funded", tone: "info" },
  running: { label: "Running", tone: "info" },
  draining: { label: "Draining", tone: "warning" },
  completed: { label: "Completed", tone: "success" },
  failed: { label: "Failed", tone: "danger" },
  cancelled: { label: "Cancelled", tone: "neutral" },
}

export const stageStatus: Record<StageState, StatusMeta> = {
  pending: { label: "Pending", tone: "neutral" },
  runnable: { label: "Runnable", tone: "info" },
  leased: { label: "Leased", tone: "info" },
  running: { label: "Running", tone: "info" },
  verifying: { label: "Verifying", tone: "warning" },
  retryable: { label: "Retryable", tone: "warning" },
  draining: { label: "Draining", tone: "warning" },
  accepted: { label: "Accepted", tone: "success" },
  failed: { label: "Failed", tone: "danger" },
  cancelled: { label: "Cancelled", tone: "neutral" },
}

export const providerStatus: Record<ProviderState, StatusMeta> = {
  enrolling: { label: "Enrolling", tone: "info" },
  benchmarking: { label: "Benchmarking", tone: "info" },
  ready: { label: "Ready", tone: "success" },
  busy: { label: "Busy", tone: "info" },
  draining: { label: "Draining", tone: "warning" },
  offline: { label: "Offline", tone: "neutral" },
  quarantined: { label: "Quarantined", tone: "danger" },
  "update required": { label: "Update required", tone: "warning" },
  cooling: { label: "Cooling", tone: "warning" },
}

export const verificationStatus: Record<VerificationState, StatusMeta> = {
  accepted: { label: "Accepted", tone: "success" },
  rejected: { label: "Rejected", tone: "danger" },
  inconclusive: { label: "Inconclusive", tone: "warning" },
}

export const artifactStatus: Record<ArtifactState, StatusMeta> = {
  candidate: { label: "Candidate", tone: "warning" },
  accepted: { label: "Accepted", tone: "success" },
  rejected: { label: "Rejected", tone: "danger" },
}

export interface StageAttempt {
  id: string
  worker: string | null
  lease: string | null
  leaseExpiresAt: string | null
  lastEvent: string
  checkpoint: string | null
  checkpointAt: string | null
  emittedStep: number | null
}

export interface Stage {
  id: string
  name: string
  template: string
  state: StageState
  trial: number | null
  retry: number
  maxRetries: number
  inputSummary: string
  outputSummary: string
  attempt: StageAttempt | null
}

export interface Pipeline {
  id: string
  name: string
  state: PipelineState
  revision: string
  specificationDigest: string
  templateSet: string
  submittedAt: string
  dataClass: string
  stages: Stage[]
}

export interface Provider {
  id: string
  workerId: string
  displayName: string
  state: ProviderState
  backend: string
  accelerator: string
  memory: string
  scratch: string
  runtime: string
  heartbeat: string
  availability: string
  benchmark: string
  note: string
}

export interface Artifact {
  id: string
  name: string
  kind: string
  state: ArtifactState
  digest: string
  size: string
  source: string
  producer: string
  verifier: string | null
  retention: string
}

export interface Verification {
  id: string
  artifact: string
  state: VerificationState
  policy: string
  producer: string
  verifier: string | null
  evidenceDigest: string | null
  requiresOperatorAction: boolean
  reason: string
}

export interface AttentionItem {
  id: string
  tone: Tone
  title: string
  detail: string
  view: DashboardView
  stageId?: string
}

export interface ActivityEvent {
  id: string
  at: string
  title: string
  detail: string
  tone: Tone
}

export interface CreditLedger {
  issuedMicrocredits: string
  reservedMicrocredits: string
  acceptedMicrocredits: string
  releasedMicrocredits: string
  reconciliation: "ready" | "attention"
  policy: string
}

export interface DashboardSnapshot {
  generatedAt: string
  pipeline: Pipeline
  providers: Provider[]
  artifacts: Artifact[]
  verifications: Verification[]
  credits: CreditLedger
  activity: ActivityEvent[]
}

export const dashboardSnapshot: DashboardSnapshot = {
  generatedAt: "2026-07-13T19:52:00Z",
  pipeline: {
    id: "pipe_01JQ3X4V6ZH7TN3FR6P8",
    name: "Public instruction tuning — reference LoRA sweep",
    state: "running",
    revision: "pipeline/v1.0.0",
    specificationDigest: "sha256:6cf2a8d7f4b901e5c3dce89b45e13b79b80f7a021c5d92aa74fbe0019d3a2fc8",
    templateSet: "pilot-template-set/2026-07-13",
    submittedAt: "2026-07-13T18:03:00Z",
    dataClass: "Public, operator-approved",
    stages: [
      {
        id: "stage_dataset_prepare",
        name: "Prepare pinned public dataset",
        template: "dataset-prepare-v1",
        state: "accepted",
        trial: null,
        retry: 0,
        maxRetries: 2,
        inputSummary: "Pinned JSONL source manifest",
        outputSummary: "Canonical training shard manifest",
        attempt: {
          id: "attempt_01JQ3X6Y6JQ6B3H8DRN9",
          worker: "worker-atlas-a6000",
          lease: "fence:17",
          leaseExpiresAt: null,
          lastEvent: "Accepted artifact promoted",
          checkpoint: null,
          checkpointAt: null,
          emittedStep: null,
        },
      },
      {
        id: "stage_trial_01",
        name: "Trial 01 · learning rate 2e-4",
        template: "pytorch-lora-train-v1",
        state: "accepted",
        trial: 1,
        retry: 0,
        maxRetries: 2,
        inputSummary: "Accepted dataset manifest",
        outputSummary: "LoRA adapter + metrics",
        attempt: {
          id: "attempt_01JQ3Y4TFMCE66VKB1SY",
          worker: "worker-northstar-m2",
          lease: "fence:22",
          leaseExpiresAt: null,
          lastEvent: "Evaluation policy passed",
          checkpoint: "checkpoint/step-2400",
          checkpointAt: "2026-07-13T19:06:00Z",
          emittedStep: 2400,
        },
      },
      {
        id: "stage_trial_02",
        name: "Trial 02 · learning rate 1e-4",
        template: "pytorch-lora-train-v1",
        state: "verifying",
        trial: 2,
        retry: 0,
        maxRetries: 2,
        inputSummary: "Accepted dataset manifest",
        outputSummary: "Candidate adapter awaiting evaluation",
        attempt: {
          id: "attempt_01JQ3Y7C57VR5GA8D8Y0",
          worker: "worker-atlas-a6000",
          lease: "fence:23",
          leaseExpiresAt: "2026-07-13T20:12:00Z",
          lastEvent: "Candidate adapter submitted for policy v1",
          checkpoint: "checkpoint/step-2400",
          checkpointAt: "2026-07-13T19:24:00Z",
          emittedStep: 2400,
        },
      },
      {
        id: "stage_trial_03",
        name: "Trial 03 · learning rate 6e-5",
        template: "pytorch-lora-train-v1",
        state: "running",
        trial: 3,
        retry: 0,
        maxRetries: 2,
        inputSummary: "Accepted dataset manifest",
        outputSummary: "LoRA adapter + metrics",
        attempt: {
          id: "attempt_01JQ3Y93NZK4HCX1J77F",
          worker: "worker-northstar-m2",
          lease: "fence:24",
          leaseExpiresAt: "2026-07-13T20:04:00Z",
          lastEvent: "Optimizer step emitted",
          checkpoint: "checkpoint/step-1600",
          checkpointAt: "2026-07-13T19:41:00Z",
          emittedStep: 1775,
        },
      },
      {
        id: "stage_trial_04",
        name: "Trial 04 · learning rate 5e-5",
        template: "pytorch-lora-train-v1",
        state: "retryable",
        trial: 4,
        retry: 1,
        maxRetries: 2,
        inputSummary: "Accepted dataset manifest",
        outputSummary: "Resume from promoted checkpoint",
        attempt: {
          id: "attempt_01JQ3YB3TY6PCF7NQ7KX",
          worker: "worker-legacy-p40",
          lease: "fence:25",
          leaseExpiresAt: "2026-07-13T19:55:00Z",
          lastEvent: "Worker disconnected after checkpoint promotion",
          checkpoint: "checkpoint/step-800",
          checkpointAt: "2026-07-13T19:33:00Z",
          emittedStep: 800,
        },
      },
      {
        id: "stage_trial_05",
        name: "Trial 05 · learning rate 3e-5",
        template: "pytorch-lora-train-v1",
        state: "accepted",
        trial: 5,
        retry: 0,
        maxRetries: 2,
        inputSummary: "Accepted dataset manifest",
        outputSummary: "LoRA adapter + metrics",
        attempt: {
          id: "attempt_01JQ3YCPD13T3P08T71D",
          worker: "worker-amd-7900xtx",
          lease: "fence:26",
          leaseExpiresAt: null,
          lastEvent: "Metrics artifact accepted",
          checkpoint: "checkpoint/step-2400",
          checkpointAt: "2026-07-13T19:11:00Z",
          emittedStep: 2400,
        },
      },
      {
        id: "stage_trial_06",
        name: "Trial 06 · learning rate 2e-5",
        template: "pytorch-lora-train-v1",
        state: "leased",
        trial: 6,
        retry: 0,
        maxRetries: 2,
        inputSummary: "Accepted dataset manifest",
        outputSummary: "LoRA adapter + metrics",
        attempt: {
          id: "attempt_01JQ3YE9A0QY4RXMNB8S",
          worker: "worker-atlas-a6000",
          lease: "fence:27",
          leaseExpiresAt: "2026-07-13T20:21:00Z",
          lastEvent: "Signed template admitted",
          checkpoint: null,
          checkpointAt: null,
          emittedStep: null,
        },
      },
      {
        id: "stage_trial_07",
        name: "Trial 07 · learning rate 1e-5",
        template: "pytorch-lora-train-v1",
        state: "runnable",
        trial: 7,
        retry: 0,
        maxRetries: 2,
        inputSummary: "Accepted dataset manifest",
        outputSummary: "LoRA adapter + metrics",
        attempt: null,
      },
      {
        id: "stage_trial_08",
        name: "Trial 08 · learning rate 8e-6",
        template: "pytorch-lora-train-v1",
        state: "pending",
        trial: 8,
        retry: 0,
        maxRetries: 2,
        inputSummary: "Accepted dataset manifest",
        outputSummary: "LoRA adapter + metrics",
        attempt: null,
      },
      {
        id: "stage_rank_trials",
        name: "Rank accepted trials",
        template: "evaluation-v1",
        state: "pending",
        trial: null,
        retry: 0,
        maxRetries: 1,
        inputSummary: "Accepted trial adapters and metric manifests",
        outputSummary: "Versioned sweep ranking",
        attempt: null,
      },
      {
        id: "stage_independent_verify",
        name: "Independently verify winning adapter",
        template: "evaluation-v1",
        state: "pending",
        trial: null,
        retry: 0,
        maxRetries: 1,
        inputSummary: "Selected adapter and hidden policy fixture",
        outputSummary: "Independent verification decision",
        attempt: null,
      },
    ],
  },
  providers: [
    {
      id: "provider-atlas",
      workerId: "worker-atlas-a6000",
      displayName: "Atlas Lab · workstation 01",
      state: "busy",
      backend: "CUDA",
      accelerator: "RTX A6000 · 48 GB VRAM",
      memory: "128 GB RAM",
      scratch: "1.8 TB NVMe available",
      runtime: "PyTorch reference environment r4",
      heartbeat: "Observed 18s ago",
      availability: "Until 22:00 BST",
      benchmark: "pilot-benchmark/2026-07-13 · passed",
      note: "Two assigned stages; no drain requested.",
    },
    {
      id: "provider-northstar",
      workerId: "worker-northstar-m2",
      displayName: "Northstar Studio · M2 Max",
      state: "busy",
      backend: "MPS",
      accelerator: "Apple M2 Max · 32 GB unified memory",
      memory: "32 GB unified memory",
      scratch: "620 GB NVMe available",
      runtime: "Dedicated pilot environment r4",
      heartbeat: "Observed 42s ago",
      availability: "Until 21:30 BST",
      benchmark: "pilot-benchmark/2026-07-13 · passed",
      note: "Checkpointing trial 03; Apple path remains pilot-scoped.",
    },
    {
      id: "provider-legacy",
      workerId: "worker-legacy-p40",
      displayName: "Legacy Rack · Tesla P40",
      state: "quarantined",
      backend: "CUDA",
      accelerator: "Tesla P40 · 24 GB VRAM",
      memory: "64 GB RAM",
      scratch: "470 GB NVMe available",
      runtime: "PyTorch reference environment r3",
      heartbeat: "Not observed for 17m",
      availability: "Unknown until review",
      benchmark: "pilot-benchmark/2026-07-12 · expired",
      note: "Quarantined after a disconnect; promoted checkpoint remains safe for reassignment.",
    },
    {
      id: "provider-aurora",
      workerId: "worker-amd-7900xtx",
      displayName: "Aurora Compute · RX 7900 XTX",
      state: "draining",
      backend: "ROCm",
      accelerator: "RX 7900 XTX · 24 GB VRAM",
      memory: "64 GB RAM",
      scratch: "930 GB NVMe available",
      runtime: "PyTorch reference environment r4",
      heartbeat: "Observed 1m ago",
      availability: "Drain begins at 20:15 BST",
      benchmark: "pilot-benchmark/2026-07-13 · passed",
      note: "No new assignments after the active work closes.",
    },
  ],
  artifacts: [
    {
      id: "artifact_dataset_01",
      name: "Public instruction dataset manifest",
      kind: "Dataset manifest",
      state: "accepted",
      digest: "sha256:37de987a7f1cf2a95d906156e9cb54ac74de97f5ec8018d17f94e0277be8e146",
      size: "38.4 MB",
      source: "Prepare pinned public dataset",
      producer: "worker-atlas-a6000",
      verifier: "local verifier policy/dataset-prepare-v1",
      retention: "Pilot evidence · 90 days",
    },
    {
      id: "artifact_trial_02_candidate",
      name: "Trial 02 LoRA adapter",
      kind: "Adapter checkpoint",
      state: "candidate",
      digest: "sha256:4df163d0ad7d5820c7a8d9295a5cb9ca2c4f044221412a27d8d8b2e81b5f2014",
      size: "184 MB",
      source: "Trial 02 · learning rate 1e-4",
      producer: "worker-atlas-a6000",
      verifier: null,
      retention: "Temporary until policy decision",
    },
    {
      id: "artifact_trial_05",
      name: "Trial 05 adapter and metrics",
      kind: "Adapter checkpoint",
      state: "accepted",
      digest: "sha256:aa42a6f4e6b0e0d6e20a62ee4b6e7ac2e0f1d02e8a24589a15e002b036c52bdf",
      size: "186 MB",
      source: "Trial 05 · learning rate 3e-5",
      producer: "worker-amd-7900xtx",
      verifier: "evaluation-v1 policy/2026-07-13",
      retention: "Pilot evidence · 90 days",
    },
    {
      id: "artifact_trial_04_checkpoint",
      name: "Trial 04 promoted checkpoint · step 800",
      kind: "Resumable checkpoint",
      state: "accepted",
      digest: "sha256:009c71ac153b9aa4e84c8ebd12ce57efc75a80a96fd8c9af6b674f1c8e711a04",
      size: "412 MB",
      source: "Trial 04 · learning rate 5e-5",
      producer: "worker-legacy-p40",
      verifier: "checkpoint policy/2026-07-13",
      retention: "Retain through reassignment",
    },
  ],
  verifications: [
    {
      id: "verification_01",
      artifact: "Trial 02 LoRA adapter",
      state: "inconclusive",
      policy: "evaluation-v1 / policy 2026-07-13",
      producer: "worker-atlas-a6000",
      verifier: null,
      evidenceDigest: null,
      requiresOperatorAction: true,
      reason: "Awaiting independent evaluator assignment; candidate is not usable downstream.",
    },
    {
      id: "verification_02",
      artifact: "Trial 05 adapter and metrics",
      state: "accepted",
      policy: "evaluation-v1 / policy 2026-07-13",
      producer: "worker-amd-7900xtx",
      verifier: "worker-northstar-m2",
      evidenceDigest: "sha256:7fb692aec11f848d601546f2b3cf5535e8e32fb01589b842243b2f2fe81ea84b",
      requiresOperatorAction: false,
      reason: "Artifact, lineage, environment and evaluation policy checks passed.",
    },
    {
      id: "verification_03",
      artifact: "Trial 04 resumed attempt",
      state: "rejected",
      policy: "lease-fencing-v1 / policy 2026-07-13",
      producer: "worker-legacy-p40",
      verifier: "coordinator policy engine",
      evidenceDigest: "sha256:71b5fd518f764e4c70f7e9b5bdb49e67a3cd12854466b1bfd1dc27f9e6cc2326",
      requiresOperatorAction: false,
      reason: "Stale worker submission rejected after the fenced lease lost authority.",
    },
  ],
  credits: {
    issuedMicrocredits: "5000000",
    reservedMicrocredits: "2800000",
    acceptedMicrocredits: "1400000",
    releasedMicrocredits: "800000",
    reconciliation: "ready",
    policy: "pilot-pricing/v1 · valueless test credits",
  },
  activity: [
    {
      id: "event_01",
      at: "19:52 BST",
      title: "Trial 02 submitted a candidate adapter",
      detail: "Awaiting independent evaluation; no acceptance or credit release has occurred.",
      tone: "warning",
    },
    {
      id: "event_02",
      at: "19:44 BST",
      title: "Trial 03 emitted optimizer step 1,775",
      detail: "Most recent promoted checkpoint remains step 1,600.",
      tone: "info",
    },
    {
      id: "event_03",
      at: "19:38 BST",
      title: "Legacy Rack was quarantined",
      detail: "Disconnect recorded after a safe checkpoint promotion; retry is eligible for reassignment.",
      tone: "danger",
    },
    {
      id: "event_04",
      at: "19:21 BST",
      title: "Trial 05 was accepted",
      detail: "Independent evaluation recorded against policy 2026-07-13.",
      tone: "success",
    },
  ],
}

export function countByStageState(stages: Stage[]): Partial<Record<StageState, number>> {
  return stages.reduce<Partial<Record<StageState, number>>>((counts, stage) => {
    counts[stage.state] = (counts[stage.state] ?? 0) + 1
    return counts
  }, {})
}

export function getTrialStages(pipeline: Pipeline): Stage[] {
  return pipeline.stages.filter((stage) => stage.trial !== null)
}

export function formatFixtureTimestamp(value: string): string {
  return new Intl.DateTimeFormat("en-GB", {
    hour: "2-digit",
    minute: "2-digit",
    timeZone: "Europe/London",
    timeZoneName: "short",
  }).format(new Date(value))
}

export function buildAttentionItems(snapshot: DashboardSnapshot): AttentionItem[] {
  const rankedItems: Array<{ priority: number; item: AttentionItem }> = []
  const add = (priority: number, item: AttentionItem) => rankedItems.push({ priority, item })

  for (const stage of snapshot.pipeline.stages) {
    if (stage.state === "retryable") {
      add(1, {
        id: `retry-${stage.id}`,
        tone: "warning",
        title: `${stage.name} is ready for reassignment`,
        detail: `${stage.attempt?.checkpoint ?? "No checkpoint"} is promoted; retry ${stage.retry} of ${stage.maxRetries} remains available.`,
        view: "pipelines",
        stageId: stage.id,
      })
    }

    if (stage.state === "failed") {
      add(0, {
        id: `failed-${stage.id}`,
        tone: "danger",
        title: `${stage.name} failed`,
        detail: `${stage.attempt?.lastEvent ?? "No attempt detail is available."} A policy-reviewed recovery decision is required.`,
        view: "pipelines",
        stageId: stage.id,
      })
    }
  }

  for (const verification of snapshot.verifications) {
    if (verification.state === "rejected" && verification.requiresOperatorAction) {
      add(0, {
        id: `verification-${verification.id}`,
        tone: "danger",
        title: `${verification.artifact} was rejected`,
        detail: `${verification.reason} Inspect the decision before scheduling replacement work.`,
        view: "verification",
      })
    }

    if (verification.state === "inconclusive" && verification.requiresOperatorAction) {
      add(2, {
        id: `verification-${verification.id}`,
        tone: "warning",
        title: `${verification.artifact} needs independent evaluation`,
        detail: verification.reason,
        view: "verification",
      })
    }
  }

  for (const provider of snapshot.providers) {
    if (provider.state === "quarantined") {
      add(0, {
        id: `provider-${provider.id}`,
        tone: "danger",
        title: `${provider.displayName} is quarantined`,
        detail: provider.note,
        view: "providers",
      })
    }

    if (provider.state === "offline") {
      add(1, {
        id: `provider-${provider.id}`,
        tone: "warning",
        title: `${provider.displayName} is offline`,
        detail: provider.note,
        view: "providers",
      })
    }

    if (provider.state === "update required") {
      add(1, {
        id: `provider-${provider.id}`,
        tone: "warning",
        title: `${provider.displayName} needs a runtime update`,
        detail: provider.note,
        view: "providers",
      })
    }
  }

  return rankedItems
    .sort((left, right) => {
      if (left.priority !== right.priority) return left.priority - right.priority
      if (left.item.id < right.item.id) return -1
      if (left.item.id > right.item.id) return 1
      return 0
    })
    .map(({ item }) => item)
}

export function formatMicrocredits(value: string): string {
  const microcredits = BigInt(value)
  const negative = microcredits < 0n
  const absolute = negative ? -microcredits : microcredits
  const whole = absolute / 1_000_000n
  const fraction = (absolute % 1_000_000n).toString().padStart(6, "0")
  const sign = negative ? "−" : ""

  return `${sign}${whole.toString()}.${fraction} test credits`
}

export function shortenDigest(digest: string): string {
  if (digest.length <= 24) return digest
  return `${digest.slice(0, 15)}…${digest.slice(-8)}`
}
