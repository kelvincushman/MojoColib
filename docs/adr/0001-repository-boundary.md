# ADR 0001: Keep MojoColib and the imported Colibri engine in one monorepo

- Status: Accepted
- Date: 2026-07-12
- Decision owners: Product owner and technical owner
- Applies to: repository layout, upstream Colibri synchronization and component ownership

## Context

MojoColib needs one release and verification boundary for its Rust control plane, Python training templates, schemas, deployment assets and optional inference backends. It also needs to preserve and extend the existing [JustVugg/colibri](https://github.com/JustVugg/colibri) engine without obscuring where that source came from or making routine upstream updates indistinguishable from local feature work.

Separate repositories would force protocol, fixture and release changes to be coordinated before the interfaces are stable. A Git submodule would preserve upstream identity but would make atomic changes across the scheduler, worker, fixtures and engine cumbersome and would make a normal checkout incomplete unless the submodule were initialized.

## Decision

This repository is the authoritative monorepo for MojoColib.

The upstream Colibri source is imported at `engine/colibri/` as a pinned, upstream-derived vendor tree. It is not a submodule and MUST NOT contain a nested `.git` directory. Its exact initial provenance and licence are recorded in [`docs/upstream-colibri-provenance.md`](../upstream-colibri-provenance.md).

The repository boundaries are:

- `engine/colibri/` contains the imported Colibri engine and the smallest engine-local adaptations required to compile, test or expose a versioned backend interface.
- `crates/`, `services/`, `python/`, `schemas/`, `fixtures/`, `deploy/` and root-level `tests/` contain MojoColib product code and contracts.
- Engine consumers MUST use a documented, versioned command, manifest or C ABI boundary. They MUST NOT depend directly on private engine structs or unversioned internal file formats.
- Mojo code that is inseparable from the engine build may live under `engine/colibri/`, but Mojo remains an optional extension. The upstream CPU path and the core training marketplace MUST continue to build and operate without the Mojo toolchain.

One repository does not imply one licence. Upstream-derived files remain governed by their upstream licence. The licence for new project-owned work is a separate project decision and MUST NOT be inferred from this ADR.

## Ownership rules

| Area | Primary owner | Required review |
| --- | --- | --- |
| `engine/colibri/` upstream synchronization, provenance and merge conflicts | Colibri vendor maintainer | Technical owner |
| Upstream-origin engine behavior and compatibility | Colibri vendor maintainer | Engine/backend owner for affected code |
| Mojo backend, shim, ABI and Mojo-specific tests | Mojo/backend owner | Colibri vendor maintainer |
| Rust control plane, worker and shared protocol crates | Control-plane owner | Protocol or security owner when their boundary changes |
| Python templates and ML fixtures | ML/template owner | Security owner for executor or dependency changes |
| Cross-boundary schemas, manifests and artifact formats | Protocol owner | Every affected producer and consumer owner |
| Release provenance and third-party licence inventory | Technical owner | Product owner before distribution |

Upstream authors retain authorship and ownership of imported work. Local maintainers own only their contributions. Any change to an upstream-origin file MUST be identifiable in review and carry the prominent modification notice required by Apache-2.0 section 4(b). A change that crosses the engine/product boundary requires approval from both owning areas; directory ownership rules established later MUST encode this table.

## Upstream update method

Upstream updates are deliberate vendor synchronizations, never an automatic pull from a moving branch.

1. Select one full 40-character upstream commit ID. Do not pin a branch name, tag alone or abbreviated hash.
2. Fetch that commit from the remote named `colibri-upstream`, whose canonical URL is `https://github.com/JustVugg/colibri.git`, and verify the commit and tree IDs against GitHub.
3. Compare the old and proposed upstream commits, including `LICENSE`, any `NOTICE` file, bundled dependencies, generated assets, build behavior and public interfaces. A licence or attribution change blocks the update pending technical-owner review.
4. Materialize the proposed commit as a clean tree without `.git`. Use the previous pinned upstream tree as the merge base when applying its delta to `engine/colibri/`; do not overwrite local adaptations silently.
5. Resolve conflicts in favor of an explicit, tested local decision. Keep the vendor synchronization in a dedicated commit, separate from unrelated product features and from optional follow-up adaptations whenever practical.
6. Run the upstream baseline tests first, then the MojoColib engine, protocol and integration checks affected by the update. A failing compatibility or provenance check blocks the new pin.
7. Update `docs/upstream-colibri-provenance.md` in the same synchronization commit. Preserve the previous pin in its import history and record the new commit, tree, licence evidence, local changes and verification performed.

The remote is read-only for synchronization purposes. Project commits are never pushed to the upstream repository unless a maintainer intentionally prepares a separate upstream contribution.

## Consequences

- Product and engine changes can be reviewed and tested atomically.
- A normal clone contains the complete build inputs without submodule setup.
- Upstream history is represented by immutable provenance records and dedicated vendor-update commits rather than as a second repository nested in the workspace.
- Local engine changes increase synchronization cost, so integration belongs outside `engine/colibri/` unless it genuinely must participate in the engine build or ABI.
- Licence and attribution checks are release gates, not documentation added after distribution.

## Rejected alternatives

### Separate marketplace and engine repositories

Rejected for the MVP because the protocol and backend boundaries are not stable enough to release independently. The decision can be revisited when the engine ABI and artifact contracts have independent versioning and release cadence.

### Git submodule at `engine/colibri/`

Rejected because it prevents a single atomic commit from changing an engine contract and all of its consumers, and because an ordinary clone would not contain the complete source tree.

### Untracked copy of upstream source

Rejected because it cannot demonstrate which source was imported, what changed locally or which licence obligations apply.
