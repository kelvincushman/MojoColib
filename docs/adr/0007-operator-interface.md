# ADR-0007: CLI-first operator interface

- Status: Accepted for Slices 1–2
- Date: 2026-07-12
- Owners: Operations and coordinator owners

## Decision

The first operator surface is a Rust CLI that talks to the same authenticated coordinator API used by automation. A dashboard is optional and cannot be the only way to inspect or control the pilot.

Minimum read commands expose pipeline, stage, attempt, lease, checkpoint, verification, provider and ledger histories; each supports JSON output and human-readable tables. Minimum controls pause/resume new assignments globally or by template/provider/backend, quarantine or reinstate a worker, request pipeline cancellation, and record a manual verification decision. Mutations require operator identity, reason code and an idempotency key and return the appended audit-event ID.

The CLI must never bypass coordinator state machines or write the database directly. Manual acceptance cannot bypass signature, lineage, current-fence or uniqueness checks and can never create a second settlement. Dangerous bulk operations show their exact scope and require an explicit non-interactive confirmation flag in automation.

Slice 2 reports include active/expired leases, stale workers, temporary uploads awaiting cleanup, rejected artifacts, open reservations, unsettled accepted stages and reconciliation differences. Secret values and presigned URLs are always redacted.
