# ADR-0006: No automatic payment for unaccepted partial checkpoints

- Status: Accepted for the MVP
- Date: 2026-07-12
- Owners: Product and ledger owners

## Decision

The core MVP settles only an accepted stage. Promoted intermediate checkpoints provide recovery value but do not independently earn provider credit. This gives the ledger one objective payment boundary and prevents providers from maximizing checkpoint count instead of completing useful work.

A retry on another admitted worker may consume the latest compatible promoted checkpoint. The final accepted-stage settlement goes to the provider that produced the accepted final artifact under its accepted quote. Checkpoint provenance still records every contributing worker and measured execution so the pilot can quantify uncompensated recovery work.

If the coordinator or object store fails after a valid checkpoint promotion and the stage cannot complete for reasons outside the provider's control, the operator may issue an explicitly labelled test-credit adjustment. It is not a stage settlement, cannot consume the customer's reservation, requires operator identity and reason, and remains visible in the audit report. Provider disconnect, invalid output, incompatible state and resource exhaustion do not qualify.

## Revisit gate

Checkpoint compensation becomes a V2 pricing experiment only after the pilot measures interruption frequency, checkpoint reuse and contribution attribution. Any later design needs a customer cap, unique checkpoint-acceptance commitment and protection against checkpoint spam, collusion and duplicate claims.
