# ADR-0009: Research-first licensing and distribution posture

- **Status:** Accepted for the public source MVP
- **Date:** 2026-07-13
- **Owners:** Product owner and runtime owner
- **Review trigger:** Before any public binary, hosted production service, commercial deployment, or new model/runtime dependency

## Context

MojoColib combines components with different legal boundaries:

- the imported Colibri source under `engine/colibri/` is licensed under Apache License 2.0;
- Mojo and the Modular SDK are governed by Modular's current terms, which are not the Apache-2.0 terms;
- accelerator drivers and toolkits have their own terms; and
- model weights, tokenizers, datasets and generated artefacts may each have separate licences or acceptable-use conditions.

Apache-2.0 permits source and object redistribution of Colibri and derivative works, subject to its licence-copy, notice-retention and modified-file-notice conditions. It does not grant a general right to redistribute unrelated runtimes, model weights or vendor software.

As reviewed on 2026-07-13 against the Modular Community License last modified 2026-03-20, the licence permits SDK use subject to its requirements, but does not permit redistribution of the SDK or its parts except for components that Modular expressly identifies as redistributable. Redistribution of such a component is further limited to an application with material additional functionality and the other distribution requirements in that licence. The project has not yet found authoritative documentation that positively identifies the exact Mojo compiler/runtime files a MojoColib binary would need as Redistributable Components.

Technical ability to emit a native library or executable does not establish a redistribution right. Mojo also has compiler/runtime compatibility constraints: the official [packaging guidance](https://mojolang.org/docs/tools/packaging/) warns that precompiled Mojo files may not be compatible across compiler versions and recommends pinning the compiler and runtime dependency.

## Decision

MojoColib adopts Apache License 2.0 for project-authored source, documentation and synthetic fixtures, alongside a **research-first, source-first, bring-your-own-runtime** posture.

1. Project-authored source, documentation and the deterministic synthetic fixture are distributed under the root Apache License 2.0. The imported or modified Colibri source remains under its retained Apache-2.0 licence and notice obligations.
2. The default public deliverable is source, build recipes, test fixtures and dependency metadata. Users install any required Mojo/Modular toolchain from an official Modular channel and accept its terms directly.
3. The project will not check in, rehost, attach to a release, or embed Modular SDK, compiler or runtime artefacts unless the dependency review in `docs/third-party-runtime-policy.md` establishes an express redistribution right for every included file.
4. A project-built Mojo executable or shared library is not automatically releasable. It must pass an artefact-composition review that establishes whether it contains, links to, loads or otherwise redistributes Modular components.
5. Public production or commercial deployment that uses Modular infrastructure is blocked until the current capacity, notification, branding, attribution and distribution obligations have been reviewed and accepted by the project owner, with legal advice where appropriate.
6. Mojo remains an optional executor behind the runtime-neutral stage contract. The established Colibri C/CPU path remains the supported fallback, and Mojo unavailability must not block the core training marketplace.
7. Model, tokenizer, dataset, CUDA/driver and other third-party licences are reviewed independently. Approval of Colibri or Mojo does not approve those assets.

## Colibri Apache-2.0 obligations

Any distribution containing `engine/colibri/` or a derivative must:

- include a copy of the Apache License 2.0;
- add prominent change notices to modified Colibri files;
- retain applicable copyright, patent, trademark and attribution notices;
- carry forward applicable notices from an upstream `NOTICE` file if one is present; and
- avoid implying trademark permission or upstream endorsement.

The Apache patent grant and its patent-litigation termination clause also remain applicable. The imported licence is the controlling evidence for the imported snapshot: [`engine/colibri/LICENSE`](../../engine/colibri/LICENSE).

## Consequences

This posture allows local research and an Apache-2.0 source release without treating an emerging toolchain as a distributable project asset. It also keeps the scheduler and marketplace usable if Mojo cannot be installed or shipped.

The trade-off is that the initial Mojo path may require user-side installation and exact version pinning instead of a one-command universal binary. Public binary releases will take longer because they need a dependency inventory and a positive redistribution determination.

## Rejected alternatives

- **Bundle the Modular toolchain for convenience.** Rejected because the project has not established that every required file is an expressly identified Redistributable Component.
- **Treat a compiled Mojo artefact as self-authored and therefore automatically distributable.** Rejected because compilation may embed or require third-party runtime components and because compiler-version compatibility remains material.
- **Make Mojo a required core runtime.** Rejected because licensing or toolchain changes would then disable the core MVP.
- **Assume Apache-2.0 covers the complete application stack.** Rejected because it governs the imported Colibri work, not independent runtimes, drivers, weights or data.

## Evidence and limits

This decision is based on the root [Apache-2.0 licence](../../LICENSE), imported [Colibri Apache-2.0 licence](../../engine/colibri/LICENSE), Modular [Community License](https://www.modular.com/legal/community), Modular [trademark policy](https://www.modular.com/legal/trademark), Mojo [packaging guidance](https://mojolang.org/docs/tools/packaging/), and Mojo [system requirements](https://mojolang.org/docs/requirements/) as available on 2026-07-13.

This ADR is an engineering distribution policy, not legal advice. Terms can change; release approval must use the terms attached to the exact versions and artefacts being shipped.
