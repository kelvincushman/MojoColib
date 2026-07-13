# Upstream Colibri provenance

This document is the authoritative source record for the upstream-derived tree at `engine/colibri/`. Repository-boundary and synchronization policy are defined by [`ADR 0001`](adr/0001-repository-boundary.md).

## Current upstream baseline

| Field | Recorded value |
| --- | --- |
| Upstream project | [JustVugg/colibri](https://github.com/JustVugg/colibri) |
| Canonical Git remote | `https://github.com/JustVugg/colibri.git` |
| Imported path | `engine/colibri/` |
| Import form | File-for-file vendor snapshot; upstream `.git` metadata excluded |
| Commit | [`92011def0746a2b53d7a625f320a89e29db882e5`](https://github.com/JustVugg/colibri/commit/92011def0746a2b53d7a625f320a89e29db882e5) |
| Parent commit | [`8e526c68a9a942af4280eba230f19a428536dcfc`](https://github.com/JustVugg/colibri/commit/8e526c68a9a942af4280eba230f19a428536dcfc) |
| Git tree | `c651e031ed3f8f91da6a37f47e6573c3022ab556` |
| Author and committer | `JustVugg <JustVugg@users.noreply.github.com>` |
| Commit time | `2026-07-12T21:12:46Z` (`2026-07-12T23:12:46+02:00` in the commit) |
| Commit subject | `README: generalize the reproducibility note (#100) — quantized-kernel rounding forks greedy across MTP/CUDA/batched, not just MTP; exact-mode recipe DRAFT=0 IDOT=0 COLI_CUDA=0` |
| GitHub signature status | Unsigned (`verified: false`, reason `unsigned`) |
| Snapshot verification | No differences from `git archive 92011def0746a2b53d7a625f320a89e29db882e5` |

The unsigned status is a provenance fact, not a claim that the snapshot is unsafe. The full commit ID, tree ID, GitHub metadata and local byte comparison are the identity checks for this import.

## Licence record

| Field | Recorded value |
| --- | --- |
| Declared upstream licence | Apache License 2.0 |
| SPDX identifier | `Apache-2.0` |
| Licence at pinned commit | [immutable GitHub copy](https://github.com/JustVugg/colibri/blob/92011def0746a2b53d7a625f320a89e29db882e5/LICENSE) |
| Local preserved copy | [`engine/colibri/LICENSE`](../engine/colibri/LICENSE) |
| Git blob ID | `261eeb9e9f8b2b4b0d119366dda99c6fd7d35c64` |
| File size | `11,357` bytes |
| SHA-256 | `c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4` |
| Upstream `NOTICE` file | None present in the pinned tree |

The imported `LICENSE` file is unchanged from the pinned upstream blob. Apache-2.0 permits use, modification and redistribution subject to its conditions. In particular, distributions of this source or derivative work must include the licence, retain applicable notices and mark modified upstream files prominently. If a later upstream commit adds a `NOTICE` file, the synchronization review must preserve and distribute its applicable notices.

This record covers the imported Colibri source only. Project-authored MojoColib source, documentation and synthetic fixture material are licensed under the root Apache-2.0 licence as recorded in the project licensing ADR. Model weights, datasets, tokenizers, Mojo/Modular components and other third-party dependencies retain their own terms and require separate immutable licence records before download or distribution.

## Local-change policy

- The baseline above was imported without content changes.
- Future changes beneath `engine/colibri/` must distinguish upstream-derived edits from new project-owned files in review.
- Modified upstream files must carry an appropriate modification notice and remain attributable to upstream contributors.
- Vendor updates must retain local changes through a reviewed three-way merge, run upstream and local verification, and update this document in the same dedicated synchronization commit.
- The project must not describe local modifications as endorsed by JustVugg or use upstream names or marks beyond reasonable source attribution.

## Import history

| Import date | Upstream commit | Upstream tree | Local state | Verification |
| --- | --- | --- | --- | --- |
| 2026-07-12 | `92011def0746a2b53d7a625f320a89e29db882e5` | `c651e031ed3f8f91da6a37f47e6573c3022ab556` | Clean initial snapshot at `engine/colibri/` | Local Git object metadata matched GitHub; recursive comparison with the commit archive reported no differences; licence blob and SHA-256 matched the preserved file |
