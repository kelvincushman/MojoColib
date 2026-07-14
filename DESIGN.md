# Design

## Source of truth

- Status: Active — first operator-console increment
- Last refreshed: 2026-07-13
- Primary product surfaces: MojoColib Control (product-owned web console), MojoColib CLI (future authoritative operator interface), and the separately maintained Colibri local inference console.
- Evidence reviewed:
  - `docs/mojocolib-training-prd.md` — MVP users, reference journey, requirements and acceptance tests.
  - `docs/decentralized-ml-training-marketplace-mvp.md` — product thesis, execution model and pilot constraints.
  - `docs/adr/0001-repository-boundary.md` — `engine/colibri/` is an imported vendor tree; product UI belongs outside it.
  - `docs/adr/0007-operator-interface.md` — CLI remains the authoritative operational surface for early slices.
  - `docs/adr/0005-ledger-units.md` — integer test-credit terminology and settlement boundary.
  - `engine/colibri/web/src/App.tsx` and `engine/colibri/web/src/index.css` — existing dark visual language and reusable interaction patterns, not a product integration target.

## Brand

- Personality: calm, exacting, technically literate and evidence-first. The product should feel like a trustworthy operations room, not a speculative crypto marketplace.
- Trust signals: immutable IDs and digests, explicit pilot/demo status, visible provenance, precise state terminology, timestamps, reason codes and clear distinction between candidate and accepted work.
- Avoid: fake real-time metrics, payout language, anonymous-worker rhetoric, vague progress percentages, neon "futurist" ornament, or anything that suggests Mojo, Solana, cross-vendor training or public P2P execution is already live.

## Product goals

- Goals:
  - Let a pilot operator understand what needs attention without reading raw event logs.
  - Make pipeline, stage, attempt, checkpoint, artifact and verification lineage intelligible at a glance.
  - Make provider readiness and safety states visible without exposing secrets or workstation internals.
  - Establish a clean API-adapter seam for the future Rust coordinator while delivering a useful fixture-backed preview now.
  - Preserve the imported Colibri chat experience as a separate inference console.
- Non-goals:
  - Replace the CLI or allow browser code to bypass coordinator state machines.
  - Claim a live distributed marketplace, perform settlement, expose wallet/object-store/worker credentials, or provide arbitrary execution controls.
  - Build customer self-service, public provider onboarding, real payments or a universal model/inference UI in this increment.
- Success signals:
  - An operator can answer "what needs attention?", "which work is safe to schedule?", and "why was an artifact accepted or rejected?" within one navigation step.
  - Every fixture-backed page visibly says that it is a local pilot preview until the coordinator API is connected.
  - The UI remains usable at keyboard-only, narrow-screen and reduced-motion settings.

## Personas and jobs

- Primary personas:
  - Marketplace operator: admits providers, follows failures, manages pilot safety and reconciles evidence.
  - Verification operator: reviews candidate artifacts and policy evidence.
  - Pipeline author: follows a submitted reproducible sweep and retrieves accepted provenance.
  - Compute provider: later views one machine's enrolment, benchmark, availability and assigned work.
- User jobs:
  - Identify expiring leases, stale/offline workers, rejected artifacts and blocked verification work.
  - Inspect a pipeline from immutable specification through attempts, checkpoints, verification and accepted artifacts.
  - Check whether a provider is ready, draining, quarantined or unsuitable for a workload.
  - Understand fixed-integer pilot credit reservations without confusing them with money or on-chain settlement.
- Key contexts of use: desktop-first operator work during a trusted 5–10-machine pilot; tablet and phone review of health/attention items; no assumption of permanent connectivity or production APIs.

## Information architecture

- Primary navigation:
  - Overview
  - Pipelines
  - Providers
  - Artifacts
  - Verification
  - Credits (feature-gated pilot accounting)
  - Separate Colibri inference-console link, not a merged workspace.
- Core routes/screens:
  - Overview: attention queue, stage state counts, pilot health, active pipeline and activity timeline.
  - Pipelines: pipeline summaries; detail exposes stage/trial progress, attempts/checkpoints, accepted artifacts and audit events.
  - Providers: invited provider/worker inventory, readiness, capabilities, backend, storage and availability state.
  - Artifacts: lineage-first candidate/accepted views with content digests, type, retention and promotion/verification status.
  - Verification: queue, policy version, producer/verifier separation, decision and reason codes.
  - Credits: test-credit reservations and reconciliation only; no real payment or Solana authority.
- Content hierarchy: attention and safety signals first; state and provenance second; technical identifiers/details on demand. A checkpoint is shown as a timeline milestone, never as a stage outcome.

## Design principles

- Evidence before optimism: report signed/observed state, not an inferred promise. Do not invent ETA or completion percentage when the executor has not emitted it.
- State names are product contracts: use pipeline, stage, attempt, trial, candidate artifact, accepted artifact, verification decision and test credits consistently.
- Make unsafe actions hard to mistake: later mutation views need explicit scope, reason code, confirmation and an audit-event result. The initial console is read-only.
- Reveal detail progressively: show a clear operator summary first and precise digests/timestamps/lineage where inspection is needed.
- Tradeoffs: desktop density helps operators inspect a small fleet, but every dense table must collapse into readable cards on small screens; visual polish cannot imply backend capability.

## Visual language

- Color: retain the existing graphite foundation and mint trust accent as visual inspiration, but use a semantic palette: blue for active, green for accepted/healthy, amber for waiting/retry/draining, red for failed/rejected/quarantined and neutral grey for draft/offline/cancelled. Pair every colour with text and an icon.
- Typography: system sans-serif for dense interface text, tabular/monospace figures for digests, IDs and metrics. Use small uppercase labels sparingly for scanning.
- Spacing/layout rhythm: 8px base rhythm; 24–32px desktop page padding; cards and panels favour breathing room over a wall of telemetry.
- Shape/radius/elevation: modest 10–16px radii, subtle borders, low-elevation panels and no decorative glass effects that reduce legibility.
- Motion: short, purpose-led transitions only; reduced-motion preference removes nonessential animation.
- Imagery/iconography: use clear line icons and data glyphs; no hardware-brand imagery, wallets, token imagery or generated art in the operator console.

## Components

- Existing components to reuse: the product console may reuse the visual concepts of the vendored Colibri UI (dark shell, status dot, compact metric, responsive side rail) but not its chat-specific code or API client.
- New/changed components:
  - App shell and responsive navigation rail.
  - Environment/pilot notice, status badge, metric card, attention item, activity timeline and empty-state card.
  - Pipeline/trial table, provider health card, artifact-lineage card and verification decision panel.
  - Future: confirmation dialog, audit-event link, capability matrix and scoped intervention form.
- Variants and states: every status-bearing component accepts explicit label, semantic tone and optional supporting detail; no colour-only status representation.
- Token/component ownership: product-owned tokens and components live under `apps/control-plane-web/`; no product component is added to `engine/colibri/`.

## Accessibility

- Target standard: WCAG 2.2 AA for the product-owned console.
- Keyboard/focus behavior: all navigation and interactive cards/buttons are reachable by keyboard with a visible focus ring; no hover-only meaning.
- Contrast/readability: text and status combinations meet AA contrast, have readable line height and never depend on colour alone.
- Screen-reader semantics: labelled navigation, headings in hierarchy, real tables for tabular fleet/pipeline data, live status only for meaningful changes and explicit text for state badges.
- Reduced motion and sensory considerations: honour `prefers-reduced-motion`; no auto-playing charts or blinking alerts.

## Responsive behavior

- Supported breakpoints/devices: desktop-first from 1280px, compact desktop/tablet at 960px, one-column mobile below 700px.
- Layout adaptations: rail collapses into a horizontal navigation control; overview cards stack; tables become labelled cards; identifiers wrap safely or truncate with accessible full values.
- Touch/hover differences: target sizes stay at least 40px; explanatory content does not require hover.

## Interaction states

- Loading: a structured skeleton or a named loading state; never a page of blank cards.
- Empty: state why a list is empty and identify whether it is normal, filtered, disconnected or not implemented.
- Error: name the failed operation, preserve prior read-only data where safe and offer a retry path when an API exists.
- Success: show the accepted/updated state, timestamp and audit reference rather than a generic toast only.
- Disabled: say why the control is unavailable; initial mutations are visibly absent/read-only rather than silently nonfunctional.
- Offline/slow network: distinguish the local fixture preview from a future disconnected coordinator. Never display stale data as live.

## Content voice

- Tone: concise, factual and candid about pilot boundaries.
- Terminology: pipeline is an immutable DAG; stage is a schedulable node; attempt is a fenced execution; trial is a sweep variant; checkpoint is resumable progress; candidate is not accepted; test credits are valueless pilot accounting.
- Microcopy rules: prefer an observable fact and its implication ("Lease expires in 08m — reassignment blocked until renewal") over a broad alarm ("warning"). Use full, meaningful labels rather than unexplained abbreviations.

## Implementation constraints

- Framework/styling system: a new product-owned React/Vite application at `apps/control-plane-web/`, using plain TypeScript and CSS for this first increment. It remains separate from the vendored `engine/colibri/web` application.
- Design-token constraints: use CSS custom properties within the new app; do not create a cross-tree dependency on vendored styles or components.
- Performance constraints: fixture preview must render without network access; future API data must be paginated/virtualised before large pilot volumes make tables slow.
- Compatibility constraints: current desktop Node tooling, modern evergreen browsers and no browser exposure of worker identity keys, object-store grants, executor credentials, presigned URLs or settlement secrets.
- Test/screenshot expectations: unit-test data/derived state helpers; run a production TypeScript/Vite build; add browser-level/accessibility coverage alongside real API screens. No visual-reference baseline exists yet.

## Open questions

- [ ] Coordinator API resource shapes, authentication flow and streaming/refresh strategy — coordinator owner — blocks live data integration.
- [ ] Exact operator roles/permissions and customer/provider role separation — product and security owners — blocks mutation controls and role-specific UI.
- [ ] Final first-pilot model, dataset and pricing policy — product/ML owners — affects pipeline and credit copy.
- [ ] Whether the console eventually lives at a separate host or shares a gateway with the coordinator API — platform owner — affects deployment and session policy.
- [ ] Visual brand assets/logo beyond the text wordmark — product owner — low impact for first operator increment.
