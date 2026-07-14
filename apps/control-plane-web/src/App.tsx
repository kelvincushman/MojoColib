import { useMemo, useState, type ReactNode } from "react"

import {
  artifactStatus,
  buildAttentionItems,
  countByStageState,
  dashboardSnapshot,
  formatFixtureTimestamp,
  formatMicrocredits,
  getTrialStages,
  pipelineStatus,
  providerStatus,
  shortenDigest,
  stageStatus,
  verificationStatus,
  type DashboardView,
  type Provider,
  type Stage,
  type StatusMeta,
  type Tone,
} from "./data/dashboard"

const navigation: Array<{ id: DashboardView; label: string; symbol: string; description: string }> = [
  { id: "overview", label: "Overview", symbol: "⌂", description: "Pilot health and attention" },
  { id: "pipelines", label: "Pipelines", symbol: "≋", description: "Immutable work and attempts" },
  { id: "providers", label: "Providers", symbol: "◫", description: "Invited worker fleet" },
  { id: "artifacts", label: "Artifacts", symbol: "◇", description: "Lineage and promotion status" },
  { id: "verification", label: "Verification", symbol: "✓", description: "Policy decisions and evidence" },
  { id: "credits", label: "Credits", symbol: "◎", description: "Pilot accounting only" },
]

const pageIntro: Record<DashboardView, { eyebrow: string; title: string; detail: string }> = {
  overview: {
    eyebrow: "Trusted-pilot control",
    title: "Know what needs attention.",
    detail: "A read-only view of one reference sweep, its workers and the evidence around it.",
  },
  pipelines: {
    eyebrow: "Immutable pipeline",
    title: "Reference LoRA sweep",
    detail: "Stages and attempts remain distinct so retries and checkpoints stay auditable.",
  },
  providers: {
    eyebrow: "Invitation-only fleet",
    title: "Provider readiness",
    detail: "Worker health is a derived fleet view. It is not a substitute for signed coordinator evidence.",
  },
  artifacts: {
    eyebrow: "Content-addressed evidence",
    title: "Artifact lineage",
    detail: "Candidates are never presented as downstream-usable accepted output.",
  },
  verification: {
    eyebrow: "Policy-bound decisions",
    title: "Verification queue",
    detail: "A completed executor run is not an acceptance decision.",
  },
  credits: {
    eyebrow: "Pilot accounting",
    title: "Test-credit visibility",
    detail: "These are valueless fixed-integer pilot credits, not a wallet or payment balance.",
  },
}

function StatusBadge({ status, compact = false }: { status: StatusMeta; compact?: boolean }) {
  return (
    <span className={`status-badge status-badge--${status.tone}${compact ? " status-badge--compact" : ""}`}>
      <span className="status-dot" aria-hidden="true" />
      {status.label}
    </span>
  )
}

function ToneDot({ tone }: { tone: Tone }) {
  return <span className={`tone-dot tone-dot--${tone}`} aria-hidden="true" />
}

function MetricCard({ label, value, detail, tone = "neutral" }: { label: string; value: string | number; detail: string; tone?: Tone }) {
  return (
    <article className={`metric-card metric-card--${tone}`}>
      <span className="metric-card__label">{label}</span>
      <strong className="metric-card__value">{value}</strong>
      <span className="metric-card__detail">{detail}</span>
    </article>
  )
}

function SectionHeading({ eyebrow, title, detail, action }: {
  eyebrow: string
  title: string
  detail?: string
  action?: ReactNode
}) {
  return (
    <div className="section-heading">
      <div>
        <span className="eyebrow">{eyebrow}</span>
        <h2>{title}</h2>
        {detail ? <p>{detail}</p> : null}
      </div>
      {action}
    </div>
  )
}

function KeyValue({ label, value, mono = false }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="key-value">
      <span>{label}</span>
      <strong className={mono ? "mono" : undefined}>{value}</strong>
    </div>
  )
}

function ProviderCard({ provider }: { provider: Provider }) {
  return (
    <article className="provider-card">
      <div className="card-title-row">
        <div>
          <span className="small-label">{provider.backend}</span>
          <h3>{provider.displayName}</h3>
        </div>
        <StatusBadge status={providerStatus[provider.state]} />
      </div>
      <p className="provider-card__accelerator">{provider.accelerator}</p>
      <dl className="provider-card__facts">
        <div><dt>Worker</dt><dd className="mono">{provider.workerId}</dd></div>
        <div><dt>Scratch</dt><dd>{provider.scratch}</dd></div>
        <div><dt>Heartbeat</dt><dd>{provider.heartbeat}</dd></div>
        <div><dt>Window</dt><dd>{provider.availability}</dd></div>
      </dl>
      <div className="provider-card__note">{provider.note}</div>
    </article>
  )
}

function App() {
  const [view, setView] = useState<DashboardView>("overview")
  const [selectedStageId, setSelectedStageId] = useState("stage_trial_04")
  const snapshot = dashboardSnapshot
  const pipeline = snapshot.pipeline
  const stageCounts = useMemo(() => countByStageState(pipeline.stages), [pipeline.stages])
  const attention = useMemo(() => buildAttentionItems(snapshot), [snapshot])
  const trialStages = useMemo(() => getTrialStages(pipeline), [pipeline])
  const selectedStage = pipeline.stages.find((stage) => stage.id === selectedStageId) ?? pipeline.stages[0]
  const activeStageCount = (stageCounts.leased ?? 0) + (stageCounts.running ?? 0) + (stageCounts.verifying ?? 0)
  const schedulingReadyProviders = snapshot.providers.filter((provider) => provider.state === "ready").length

  function goTo(nextView: DashboardView, stageId?: string) {
    if (stageId) setSelectedStageId(stageId)
    setView(nextView)
  }

  const header = pageIntro[view]

  return (
    <div className="control-shell">
      <aside className="side-rail" aria-label="MojoColib Control navigation">
        <div className="brand-block">
          <div className="brand-mark" aria-hidden="true">M</div>
          <div>
            <span className="brand-name">MojoColib</span>
            <span className="brand-subtitle">Control</span>
          </div>
        </div>

        <div className="rail-label">Pilot workspace</div>
        <nav className="primary-nav" aria-label="Control views">
          {navigation.map((item) => (
            <button
              className={`nav-item${view === item.id ? " nav-item--active" : ""}`}
              type="button"
              key={item.id}
              onClick={() => goTo(item.id)}
              aria-current={view === item.id ? "page" : undefined}
              title={item.description}
            >
              <span className="nav-symbol" aria-hidden="true">{item.symbol}</span>
              <span>{item.label}</span>
            </button>
          ))}
        </nav>

        <div className="rail-spacer" />
        <div className="rail-surface">
          <span className="rail-surface__eyebrow">Separate surface</span>
          <strong>Colibri inference console</strong>
          <p>Runs independently from this coordinator preview.</p>
        </div>
        <div className="rail-foot">
          <ToneDot tone="warning" />
          <span>Local preview</span>
        </div>
      </aside>

      <main className="main-content">
        <header className="page-header">
          <div>
            <span className="eyebrow">{header.eyebrow}</span>
            <h1>{header.title}</h1>
            <p>{header.detail}</p>
          </div>
          <div className="header-status">
            <span className="header-status__value">No coordinator connected</span>
            <span className="header-status__detail">Fixture snapshot · {formatFixtureTimestamp(snapshot.generatedAt)}</span>
          </div>
        </header>

        <div className="preview-notice" role="status">
          <span className="preview-notice__mark" aria-hidden="true">i</span>
          <div>
            <strong>Pilot preview — local sample data</strong>
            <span>This console demonstrates the intended operator experience. It cannot schedule, promote, verify or settle work.</span>
          </div>
        </div>

        {view === "overview" ? (
          <Overview
            attention={attention}
            activeStageCount={activeStageCount}
            schedulingReadyProviders={schedulingReadyProviders}
            onNavigate={goTo}
            selectedStage={selectedStage}
            stageCounts={stageCounts}
            trialStages={trialStages}
          />
        ) : null}
        {view === "pipelines" ? (
          <Pipelines
            selectedStage={selectedStage}
            stageCounts={stageCounts}
            onSelectStage={setSelectedStageId}
          />
        ) : null}
        {view === "providers" ? <Providers /> : null}
        {view === "artifacts" ? <Artifacts /> : null}
        {view === "verification" ? <Verification /> : null}
        {view === "credits" ? <Credits /> : null}
      </main>
    </div>
  )
}

function Overview({
  attention,
  activeStageCount,
  schedulingReadyProviders,
  onNavigate,
  selectedStage,
  stageCounts,
  trialStages,
}: {
  attention: ReturnType<typeof buildAttentionItems>
  activeStageCount: number
  schedulingReadyProviders: number
  onNavigate: (view: DashboardView, stageId?: string) => void
  selectedStage: Stage
  stageCounts: Partial<Record<keyof typeof stageStatus, number>>
  trialStages: Stage[]
}) {
  const acceptedTrials = trialStages.filter((stage) => stage.state === "accepted").length

  return (
    <div className="page-stack">
      <section className="overview-hero">
        <div>
          <span className="eyebrow">Reference workflow</span>
          <h2>{dashboardSnapshot.pipeline.name}</h2>
          <p>The pilot uses independent, checkpointable trials—not cross-provider gradient sharing—to make progress and recovery auditable.</p>
        </div>
        <button className="button button--primary" type="button" onClick={() => onNavigate("pipelines", selectedStage.id)}>
          Inspect sweep
          <span aria-hidden="true">→</span>
        </button>
      </section>

      <section className="metric-grid" aria-label="Pilot snapshot">
        <MetricCard label="Needs attention" value={attention.length} detail="Retry, verification and fleet items" tone="warning" />
        <MetricCard label="Active stages" value={activeStageCount} detail="Leased, running or verifying" tone="info" />
        <MetricCard label="Accepted trials" value={`${acceptedTrials} / ${trialStages.length}`} detail="Accepted output is downstream-usable" tone="success" />
        <MetricCard
          label="Workers ready"
          value={`${schedulingReadyProviders} / ${dashboardSnapshot.providers.length}`}
          detail={schedulingReadyProviders === 0 ? "No worker can take a new assignment" : "Ready for a new assignment"}
          tone={schedulingReadyProviders === 0 ? "warning" : "success"}
        />
      </section>

      <section className="split-grid split-grid--attention">
        <article className="panel panel--attention">
          <SectionHeading eyebrow="Operator attention" title="Resolve the facts, not the symptoms." detail="Each item links to its evidence-bearing surface." />
          <div className="attention-list">
            {attention.map((item) => (
              <button className="attention-item" type="button" key={item.id} onClick={() => onNavigate(item.view, item.stageId)}>
                <ToneDot tone={item.tone} />
                <span className="attention-item__body">
                  <strong>{item.title}</strong>
                  <span>{item.detail}</span>
                </span>
                <span className="attention-item__arrow" aria-hidden="true">→</span>
              </button>
            ))}
          </div>
        </article>

        <article className="panel state-panel">
          <SectionHeading eyebrow="Stage distribution" title="Pipeline state, at a glance." />
          <div className="state-list">
            <StateRow label="Accepted" value={stageCounts.accepted ?? 0} tone="success" />
            <StateRow label="Active" value={activeStageCount} tone="info" />
            <StateRow label="Retryable" value={stageCounts.retryable ?? 0} tone="warning" />
            <StateRow label="Not started" value={(stageCounts.pending ?? 0) + (stageCounts.runnable ?? 0)} tone="neutral" />
          </div>
          <p className="panel-footnote">Checkpointed progress is visible in each attempt; it is not a terminal stage state.</p>
        </article>
      </section>

      <section className="split-grid split-grid--work">
        <article className="panel">
          <SectionHeading
            eyebrow="Trial activity"
            title="Eight independent variants"
            detail="The console reports observed state rather than inventing a completion percentage."
            action={<button className="button button--quiet" type="button" onClick={() => onNavigate("pipelines")}>All stages <span aria-hidden="true">→</span></button>}
          />
          <div className="trial-list">
            {trialStages.map((stage) => (
              <button className="trial-row" type="button" key={stage.id} onClick={() => onNavigate("pipelines", stage.id)}>
                <span className="trial-number">{String(stage.trial).padStart(2, "0")}</span>
                <span className="trial-name">{stage.name.replace(/^Trial \d+ · /, "")}</span>
                <StatusBadge status={stageStatus[stage.state]} compact />
              </button>
            ))}
          </div>
        </article>

        <article className="panel">
          <SectionHeading
            eyebrow="Fleet health"
            title="Provider signals"
            detail="Derived health never overrides signed coordinator decisions."
            action={<button className="button button--quiet" type="button" onClick={() => onNavigate("providers")}>Fleet detail <span aria-hidden="true">→</span></button>}
          />
          <div className="provider-summary-list">
            {dashboardSnapshot.providers.map((provider) => (
              <div className="provider-summary" key={provider.id}>
                <span className="provider-summary__mark">{provider.backend.slice(0, 1)}</span>
                <div>
                  <strong>{provider.displayName}</strong>
                  <span>{provider.accelerator}</span>
                </div>
                <StatusBadge status={providerStatus[provider.state]} compact />
              </div>
            ))}
          </div>
        </article>
      </section>

      <section className="panel activity-panel">
        <SectionHeading eyebrow="Evidence trail" title="Recent pilot activity" detail="This preview uses static events; a production console will link every event to an audit record." />
        <ol className="activity-list">
          {dashboardSnapshot.activity.map((event) => (
            <li key={event.id}>
              <ToneDot tone={event.tone} />
              <div>
                <strong>{event.title}</strong>
                <span>{event.detail}</span>
              </div>
              <time>{event.at}</time>
            </li>
          ))}
        </ol>
      </section>
    </div>
  )
}

function StateRow({ label, value, tone }: { label: string; value: number; tone: Tone }) {
  return (
    <div className="state-row">
      <span><ToneDot tone={tone} />{label}</span>
      <strong>{value}</strong>
    </div>
  )
}

function Pipelines({
  selectedStage,
  stageCounts,
  onSelectStage,
}: {
  selectedStage: Stage
  stageCounts: Partial<Record<keyof typeof stageStatus, number>>
  onSelectStage: (id: string) => void
}) {
  const pipeline = dashboardSnapshot.pipeline

  return (
    <div className="page-stack">
      <section className="pipeline-summary panel">
        <div className="pipeline-summary__main">
          <div className="card-title-row">
            <div>
              <span className="small-label">{pipeline.revision}</span>
              <h2>{pipeline.name}</h2>
            </div>
            <StatusBadge status={pipelineStatus[pipeline.state]} />
          </div>
          <p>Immutable specification and approved template set shown from a fixture. Future edits create a new pipeline revision rather than mutating funded work.</p>
        </div>
        <div className="pipeline-summary__facts">
          <KeyValue label="Spec digest" value={shortenDigest(pipeline.specificationDigest)} mono />
          <KeyValue label="Template set" value={pipeline.templateSet} />
          <KeyValue label="Data policy" value={pipeline.dataClass} />
        </div>
      </section>

      <section className="metric-grid metric-grid--pipeline" aria-label="Pipeline stage summary">
        <MetricCard label="Accepted" value={stageCounts.accepted ?? 0} detail="Eligible downstream output" tone="success" />
        <MetricCard label="Active" value={(stageCounts.leased ?? 0) + (stageCounts.running ?? 0) + (stageCounts.verifying ?? 0)} detail="Fenced attempt in progress" tone="info" />
        <MetricCard label="Retryable" value={stageCounts.retryable ?? 0} detail="Resumes from a promoted checkpoint" tone="warning" />
        <MetricCard label="Queued" value={(stageCounts.pending ?? 0) + (stageCounts.runnable ?? 0)} detail="No execution has begun" tone="neutral" />
      </section>

      <section className="panel table-panel">
        <SectionHeading eyebrow="Stage graph" title="Stages and independent trials" detail="Select a stage to inspect its current or latest attempt." />
        <div className="table-scroll">
          <table>
            <thead>
              <tr>
                <th scope="col">Stage</th>
                <th scope="col">Template</th>
                <th scope="col">State</th>
                <th scope="col">Attempt</th>
                <th scope="col">Latest checkpoint</th>
              </tr>
            </thead>
            <tbody>
              {pipeline.stages.map((stage) => (
                <tr className={selectedStage.id === stage.id ? "is-selected" : undefined} key={stage.id}>
                  <td>
                    <button className="stage-link" type="button" onClick={() => onSelectStage(stage.id)}>
                      <span>{stage.trial ? `Trial ${String(stage.trial).padStart(2, "0")}` : "Stage"}</span>
                      <strong>{stage.name.replace(/^Trial \d+ · /, "")}</strong>
                    </button>
                  </td>
                  <td><code>{stage.template}</code></td>
                  <td><StatusBadge status={stageStatus[stage.state]} compact /></td>
                  <td>{stage.attempt ? <code>{stage.attempt.id}</code> : <span className="muted">Not assigned</span>}</td>
                  <td>{stage.attempt?.checkpoint ?? <span className="muted">—</span>}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>

      <StageDetail stage={selectedStage} />
    </div>
  )
}

function StageDetail({ stage }: { stage: Stage }) {
  const attempt = stage.attempt

  return (
    <section className="stage-detail panel" aria-labelledby="selected-stage-heading">
      <div className="stage-detail__heading">
        <div>
          <span className="eyebrow">Selected stage</span>
          <h2 id="selected-stage-heading">{stage.name}</h2>
          <p>{stage.inputSummary} → {stage.outputSummary}</p>
        </div>
        <StatusBadge status={stageStatus[stage.state]} />
      </div>

      <div className="stage-detail__grid">
        <div className="detail-block">
          <span className="small-label">Execution boundary</span>
          <KeyValue label="Template" value={stage.template} mono />
          <KeyValue label="Retry budget" value={`${stage.retry} used · ${stage.maxRetries} maximum`} />
          <KeyValue label="Latest event" value={attempt?.lastEvent ?? "No attempt has been assigned"} />
        </div>
        <div className="detail-block">
          <span className="small-label">Attempt provenance</span>
          <KeyValue label="Attempt" value={attempt?.id ?? "Not assigned"} mono />
          <KeyValue label="Worker" value={attempt?.worker ?? "Not assigned"} mono={Boolean(attempt?.worker)} />
          <KeyValue label="Fence" value={attempt?.lease ?? "No active lease"} mono={Boolean(attempt?.lease)} />
          <KeyValue label="Lease" value={attempt?.leaseExpiresAt ? `Observed expiry ${attempt.leaseExpiresAt}` : "No active lease"} />
        </div>
        <div className="detail-block detail-block--checkpoint">
          <span className="small-label">Checkpoint progress</span>
          <strong>{attempt?.checkpoint ?? "No promoted checkpoint"}</strong>
          <p>{attempt?.checkpointAt ? `Promoted at ${attempt.checkpointAt}` : "A checkpoint is a resumable artifact milestone, not an accepted result."}</p>
          {attempt?.emittedStep ? <span className="step-event">Latest emitted optimizer step: {attempt.emittedStep.toLocaleString()}</span> : null}
        </div>
      </div>
    </section>
  )
}

function Providers() {
  const visibleStates = ["ready", "busy", "draining", "quarantined"] as const

  return (
    <div className="page-stack">
      <section className="metric-grid metric-grid--providers" aria-label="Provider fleet summary">
        {visibleStates.map((state) => (
          <MetricCard
            key={state}
            label={providerStatus[state].label}
            value={dashboardSnapshot.providers.filter((provider) => provider.state === state).length}
            detail={state === "quarantined" ? "Requires explicit review" : "Derived fleet state"}
            tone={providerStatus[state].tone}
          />
        ))}
      </section>

      <section className="panel provider-notice">
        <div className="notice-symbol" aria-hidden="true">!</div>
        <div>
          <strong>Read-only provider preview</strong>
          <p>Invitation, benchmark, drain, quarantine and re-instatement actions will be coordinator-backed, reason-coded and audit-linked. They are intentionally absent here.</p>
        </div>
      </section>

      <section className="provider-grid" aria-label="Provider cards">
        {dashboardSnapshot.providers.map((provider) => <ProviderCard provider={provider} key={provider.id} />)}
      </section>
    </div>
  )
}

function Artifacts() {
  return (
    <div className="page-stack">
      <section className="panel artifact-guide">
        <div>
          <span className="eyebrow">Lineage rule</span>
          <h2>Candidate is not accepted.</h2>
          <p>Only a promoted artifact with valid type, hash, lineage, environment and verification evidence becomes usable downstream.</p>
        </div>
        <div className="lineage-flow" aria-label="Artifact lineage">
          <span>Pipeline</span><b>→</b><span>Stage</span><b>→</b><span>Attempt</span><b>→</b><span>Candidate</span><b>→</b><span className="lineage-flow__accepted">Accepted</span>
        </div>
      </section>

      <section className="artifact-grid" aria-label="Artifact records">
        {dashboardSnapshot.artifacts.map((artifact) => (
          <article className="artifact-card" key={artifact.id}>
            <div className="card-title-row">
              <div>
                <span className="small-label">{artifact.kind}</span>
                <h2>{artifact.name}</h2>
              </div>
              <StatusBadge status={artifactStatus[artifact.state]} />
            </div>
            <p className="artifact-card__meaning">
              {artifact.state === "candidate" ? "Submitted for checking; not available to downstream stages." : artifact.state === "accepted" ? "Promoted evidence eligible for the declared downstream contract." : "Rejected evidence remains retained only for audit according to policy."}
            </p>
            <div className="artifact-card__facts">
              <KeyValue label="Digest" value={shortenDigest(artifact.digest)} mono />
              <KeyValue label="Size" value={artifact.size} />
              <KeyValue label="Source stage" value={artifact.source} />
              <KeyValue label="Producer" value={artifact.producer} mono />
              <KeyValue label="Verifier" value={artifact.verifier ?? "Not assigned"} mono={Boolean(artifact.verifier)} />
              <KeyValue label="Retention" value={artifact.retention} />
            </div>
          </article>
        ))}
      </section>
    </div>
  )
}

function Verification() {
  return (
    <div className="page-stack">
      <section className="verification-intro panel">
        <div>
          <span className="eyebrow">Decision boundary</span>
          <h2>Verification is distinct from execution.</h2>
          <p>Every decision needs an explicit policy version, evidence and reason. The producing worker is shown separately from the verifying worker.</p>
        </div>
        <div className="verification-legend" aria-label="Verification status legend">
          {Object.values(verificationStatus).map((status) => <StatusBadge key={status.label} status={status} compact />)}
        </div>
      </section>

      <section className="verification-list" aria-label="Verification decisions">
        {dashboardSnapshot.verifications.map((verification) => (
          <article className="verification-card" key={verification.id}>
            <div className="verification-card__head">
              <div>
                <span className="small-label">{verification.policy}</span>
                <h2>{verification.artifact}</h2>
              </div>
              <StatusBadge status={verificationStatus[verification.state]} />
            </div>
            <p>{verification.reason}</p>
            <div className="verification-card__facts">
              <KeyValue label="Producer" value={verification.producer} mono />
              <KeyValue label="Verifier" value={verification.verifier ?? "Not assigned"} mono={Boolean(verification.verifier)} />
              <KeyValue label="Evidence" value={verification.evidenceDigest ? shortenDigest(verification.evidenceDigest) : "No decision evidence yet"} mono={Boolean(verification.evidenceDigest)} />
            </div>
          </article>
        ))}
      </section>
    </div>
  )
}

function Credits() {
  const credits = dashboardSnapshot.credits

  return (
    <div className="page-stack">
      <section className="credit-notice panel">
        <div className="notice-symbol" aria-hidden="true">◎</div>
        <div>
          <strong>Valueless pilot accounting only</strong>
          <p>MojoColib uses signed, integer test-credit records during the MVP. This view does not represent a wallet, token, payout or Solana settlement authority.</p>
        </div>
      </section>

      <section className="credit-grid" aria-label="Pilot test-credit summary">
        <MetricCard label="Issued" value={formatMicrocredits(credits.issuedMicrocredits)} detail="Pilot allocation" tone="neutral" />
        <MetricCard label="Open reservation" value={formatMicrocredits(credits.reservedMicrocredits)} detail="Bound to accepted quotes" tone="info" />
        <MetricCard label="Accepted work" value={formatMicrocredits(credits.acceptedMicrocredits)} detail="Recorded against accepted stages" tone="success" />
        <MetricCard label="Released" value={formatMicrocredits(credits.releasedMicrocredits)} detail="Unused reservation returned" tone="neutral" />
      </section>

      <section className="panel reconciliation-panel">
        <SectionHeading eyebrow="Reconciliation" title="Pilot ledger status" detail="A future coordinator will reconstruct this report from append-only events, not browser state." />
        <div className="reconciliation-grid">
          <div>
            <span className="small-label">Policy</span>
            <strong>{credits.policy}</strong>
          </div>
          <div>
            <span className="small-label">State</span>
            <StatusBadge status={{ label: credits.reconciliation === "ready" ? "Fixture reconciliation ready" : "Fixture attention required", tone: credits.reconciliation === "ready" ? "success" : "warning" }} />
          </div>
          <div>
            <span className="small-label">Settlement authority</span>
            <strong>Not implemented</strong>
          </div>
        </div>
      </section>
    </div>
  )
}

export default App
