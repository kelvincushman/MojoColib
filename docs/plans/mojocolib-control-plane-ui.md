# MojoColib Control — UI/UX implementation plan

Status: **Implemented locally — fixture-backed operator console**
Date: 2026-07-13
Design contract: [`DESIGN.md`](../../DESIGN.md)
Product requirements: [`docs/mojocolib-training-prd.md`](../mojocolib-training-prd.md)

## Outcome

Deliver a product-owned, read-only operator dashboard that makes the trusted-pilot workflow understandable before a coordinator service exists. The dashboard is a UX and integration seam, not evidence that the distributed marketplace is live.

It must make these questions answerable in one click:

1. What requires operator attention now?
2. Which pipelines, stages and trials are active, blocked, accepted or retryable?
3. Which providers are ready, busy, draining, offline or quarantined?
4. What is the lineage and verification status of an artifact?
5. What pilot test-credit reservation or reconciliation state exists?

## Product boundary

- Create the console under `apps/control-plane-web/`; do not add product UI to the imported `engine/colibri/` vendor tree.
- Keep the upstream Colibri web client as a separate local inference console.
- Use deterministic sample data until the Rust coordinator exposes authenticated read APIs.
- Label every sample-data page **Pilot preview — local sample data**.
- Do not add mutations, browser-stored secrets, wallet flows, direct worker connectivity, object-store links or real-time claims in this increment.

## First-release information architecture

```text
Overview
Pipelines
  └─ selected pipeline details: stage/trial progress, checkpoints, accepted output
Providers
Artifacts
Verification
Credits (pilot accounting only)
```

The first implementation uses one selected, eight-trial LoRA sweep to make the reference journey visible. It deliberately exposes factual stage/attempt status rather than fabricated global progress or an ETA.

## Implementation sequence

### Phase 1 — Design and data contract

1. Record the durable `DESIGN.md` contract.
2. Define typed fixture data representing an immutable pipeline, stages/trials, provider fleet, artifacts, verification work, audit activity and integer test-credit balances.
3. Add pure helpers for counts, attention ordering and status semantics with focused tests.

### Phase 2 — Read-only operator console

1. Add a responsive app shell, navigation rail and persistent pilot-preview notice.
2. Build the Overview with attention items, stage summary, fleet health, active pipeline/trials and recent audit activity.
3. Build the Pipelines view with an immutable-spec summary and visible selected-stage detail.
4. Build Providers, Artifacts, Verification and Credits views from the same typed snapshot.
5. Keep all interactions local to presentation (screen selection and selected stage); no state-changing controls appear.

### Phase 3 — Verification and quality gate

1. Unit test data derivation and status vocabulary.
2. Run the app test suite and production build.
3. Verify keyboard navigation, responsive breakpoint layout, visible focus treatment and reduced-motion handling.
4. Add the web checks to CI after the new application is stable locally.

### Phase 4 — Coordinator integration (later)

1. Replace the fixture provider behind a narrow `DashboardDataSource` interface; do not couple view components to transport details.
2. Add authenticated read APIs for pipelines, stages, attempts, providers, artifacts, verification, audit and ledger reports.
3. Add explicit freshness/offline state and pagination before pilot volumes grow.
4. Introduce operator mutations only after coordinator API idempotency, audit-event and reason-code contracts exist.

## API seam

The first release owns a `DashboardSnapshot` in the browser. A later API adapter must map authenticated coordinator responses into the same display model. It must never surface:

- worker identity/private keys;
- object-storage grants or presigned URLs;
- executor credentials or unredacted host paths;
- settlement credentials or real-value payment data;
- raw dataset/model content.

The coordinator remains authoritative for scheduling, promotion, verification and settlement. The UI only presents signed/authoritative state returned by it.

## Acceptance criteria for this increment

- A product-owned app exists outside `engine/colibri/`.
- Its landing screen clearly says that it is local sample data and no coordinator is connected.
- The primary nav and six read-only views exist and are keyboard-accessible.
- The overview contains attention, pipeline/stage, provider, artifact/verification and pilot-credit information.
- Status labels follow the PRD/state-machine vocabulary; colour is never the only signal.
- The selected reference sweep makes attempts, checkpoints and independent verification visible without claiming overall training completion percentages.
- Unit tests and a production build pass.

## Deferred work

- Live coordinator API, authentication, polling/streaming and role permissions.
- Provider invitation/benchmark/drain/quarantine mutations.
- Pipeline submission, funding, cancellation and manual verification controls.
- Artifact download/provenance export, full audit search and reconciliation workflows.
- Solana/localnet/devnet presentation.
- Customer and provider self-service experiences.
