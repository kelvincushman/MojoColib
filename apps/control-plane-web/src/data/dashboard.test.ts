import { describe, expect, it } from "vitest"

import {
  artifactStatus,
  buildAttentionItems,
  countByStageState,
  dashboardSnapshot,
  formatFixtureTimestamp,
  formatMicrocredits,
  getTrialStages,
  shortenDigest,
  stageStatus,
} from "./dashboard"

describe("dashboard fixture semantics", () => {
  it("keeps the reference sweep at eight independent trials", () => {
    const trials = getTrialStages(dashboardSnapshot.pipeline)

    expect(trials).toHaveLength(8)
    expect(trials.map((stage) => stage.trial)).toEqual([1, 2, 3, 4, 5, 6, 7, 8])
  })

  it("reports stage states without converting checkpoints into a stage status", () => {
    const counts = countByStageState(dashboardSnapshot.pipeline.stages)

    expect(counts).toMatchObject({
      accepted: 3,
      verifying: 1,
      running: 1,
      retryable: 1,
      leased: 1,
      runnable: 1,
      pending: 3,
    })
    expect(stageStatus).not.toHaveProperty("checkpointed")
  })

  it("surfaces all priority attention in a deterministic order", () => {
    const attention = buildAttentionItems(dashboardSnapshot)

    expect(attention.map((item) => item.id)).toEqual([
      "provider-provider-legacy",
      "retry-stage_trial_04",
      "verification-verification_01",
    ])
    expect(attention[0]).toMatchObject({ tone: "danger", view: "providers" })
    expect(attention[1]).toMatchObject({ stageId: "stage_trial_04", tone: "warning" })
  })

  it("does not collapse repeated attention records into a single item", () => {
    const retryable = dashboardSnapshot.pipeline.stages.find((stage) => stage.state === "retryable")
    const inconclusive = dashboardSnapshot.verifications.find((verification) => verification.state === "inconclusive")
    const quarantined = dashboardSnapshot.providers.find((provider) => provider.state === "quarantined")

    expect(retryable).toBeDefined()
    expect(inconclusive).toBeDefined()
    expect(quarantined).toBeDefined()

    const attention = buildAttentionItems({
      ...dashboardSnapshot,
      pipeline: {
        ...dashboardSnapshot.pipeline,
        stages: [...dashboardSnapshot.pipeline.stages, { ...retryable!, id: "stage_trial_09", name: "Trial 09 · learning rate 4e-5" }],
      },
      verifications: [...dashboardSnapshot.verifications, { ...inconclusive!, id: "verification_04", artifact: "Trial 09 LoRA adapter" }],
      providers: [...dashboardSnapshot.providers, { ...quarantined!, id: "provider-legacy-02", displayName: "Legacy Rack · Tesla P40 02" }],
    })

    expect(attention).toHaveLength(6)
    expect(attention.filter((item) => item.view === "pipelines")).toHaveLength(2)
    expect(attention.filter((item) => item.title.includes("needs independent evaluation"))).toHaveLength(2)
    expect(attention.filter((item) => item.title.includes("is quarantined"))).toHaveLength(2)
  })

  it("orders action-required failures, verification and worker health by priority", () => {
    const retryable = dashboardSnapshot.pipeline.stages.find((stage) => stage.state === "retryable")
    const rejected = dashboardSnapshot.verifications.find((verification) => verification.state === "rejected")
    const quarantined = dashboardSnapshot.providers.find((provider) => provider.state === "quarantined")

    expect(retryable).toBeDefined()
    expect(rejected).toBeDefined()
    expect(quarantined).toBeDefined()

    const attention = buildAttentionItems({
      ...dashboardSnapshot,
      pipeline: {
        ...dashboardSnapshot.pipeline,
        stages: [{
          ...retryable!,
          id: "stage_failed",
          name: "Prepare public dataset",
          state: "failed",
          attempt: { ...retryable!.attempt!, lastEvent: "Input manifest integrity check failed" },
        }],
      },
      verifications: [{
        ...rejected!,
        id: "verification_requires_review",
        artifact: "Trial 09 LoRA adapter",
        requiresOperatorAction: true,
      }],
      providers: [
        { ...quarantined!, id: "provider-offline", displayName: "Offline worker", state: "offline" },
        { ...quarantined!, id: "provider-update", displayName: "Update worker", state: "update required" },
      ],
    })

    expect(attention.map((item) => item.id)).toEqual([
      "failed-stage_failed",
      "verification-verification_requires_review",
      "provider-provider-offline",
      "provider-provider-update",
    ])
    expect(attention.map((item) => item.tone)).toEqual(["danger", "danger", "warning", "warning"])
  })

  it("distinguishes a candidate artifact from accepted output", () => {
    expect(artifactStatus.candidate).toMatchObject({ label: "Candidate", tone: "warning" })
    expect(artifactStatus.accepted).toMatchObject({ label: "Accepted", tone: "success" })
  })

  it("formats fixed integer microcredits without floating point", () => {
    expect(formatMicrocredits("5000000")).toBe("5.000000 test credits")
    expect(formatMicrocredits("1")).toBe("0.000001 test credits")
    expect(formatMicrocredits("-1200000")).toBe("−1.200000 test credits")
  })

  it("formats the fixture timestamp from its authoritative UTC value", () => {
    expect(formatFixtureTimestamp(dashboardSnapshot.generatedAt)).toBe("20:52 BST")
  })

  it("shortens a digest without removing its algorithm prefix or ending", () => {
    const digest = dashboardSnapshot.pipeline.specificationDigest

    expect(shortenDigest(digest)).toBe("sha256:6cf2a8d7…9d3a2fc8")
  })
})
