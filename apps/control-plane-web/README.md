# MojoColib Control

Product-owned operator-console preview for the MojoColib trusted-pilot training marketplace.

This application is deliberately separate from the imported Colibri web client in
`engine/colibri/web/`. It uses deterministic fixture data today and does **not**
connect to a coordinator, schedule work, expose provider controls, promote
artifacts, settle credits or handle secrets.

## Local commands

Requires Node.js 22.22.2, matching CI.

```sh
npm ci
npm run dev
npm test
npm run build
```

## Current UX scope

- Operator overview and attention queue.
- Immutable reference pipeline, stage/attempt and checkpoint visibility.
- Provider fleet health and qualification states.
- Candidate versus accepted artifact lineage.
- Verification decisions and policy evidence.
- Fixed-integer, valueless pilot test-credit presentation.

The design contract is [`DESIGN.md`](../../DESIGN.md) and the implementation
plan is [`docs/plans/mojocolib-control-plane-ui.md`](../../docs/plans/mojocolib-control-plane-ui.md).

## Future API boundary

When coordinator services exist, an authenticated data-source adapter should
replace `src/data/dashboard.ts` without leaking worker identity keys,
object-storage grants, executor credentials, host paths or settlement secrets to
the browser.
