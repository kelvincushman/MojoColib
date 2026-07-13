# Third-party runtime and redistribution policy

**Effective:** 2026-07-12

**Applies to:** source, builds, test artefacts, release packages and hosted deployments of MojoColib

This policy converts ADR-0009 into an engineering gate. Its default is conservative: a third-party artefact is **not redistributable until an authoritative licence or written permission positively says that the exact artefact is redistributable in the intended form**.

## Dependency classes

Each dependency must be classified before it is pinned or shipped:

| Class | Examples | Default treatment |
| --- | --- | --- |
| Imported source | Colibri C sources | May be distributed only under the source licence and its notice obligations |
| Build tool | Mojo compiler, C compiler | Developer/CI installation; do not include in releases without separate approval |
| Runtime component | Shared libraries, device runtimes, loaders | User/system supplied unless expressly approved for redistribution |
| Driver or toolkit | NVIDIA driver/CUDA toolkit, ROCm/HIP, Apple frameworks | System supplied and governed by vendor/platform terms |
| Precompiled output | Mojo executable, shared library, `.mojopkg` or other compiled package | Inspect composition and compatibility before release |
| Model or data asset | Weights, tokenizer, dataset, evaluation corpus | Separate licence, provenance and acceptable-use review |

## Permitted now

The following are permitted within the research-first posture:

- local and private-CI installation and use of a third-party toolchain by a person or entity that has accepted its applicable terms;
- distribution of the imported or modified Colibri source when Apache-2.0 conditions are met;
- distribution of MojoColib-owned source, build instructions and version pins that tell users how to obtain dependencies from official upstream channels;
- local research builds and benchmark artefacts that are not redistributed to third parties;
- use of system-provided accelerator drivers and frameworks without copying them into the repository or release; and
- a public binary that passes the dependency review gate and is shown not to redistribute an unapproved third-party component.

For Colibri, every source or object distribution must include the Apache-2.0 licence, retain applicable notices, mark modified files prominently, and include applicable upstream `NOTICE` content if such a file exists.

## Prohibited without explicit approval

Do not:

- commit, mirror, rehost or attach the Modular SDK, Mojo compiler, Modular runtime libraries or vendor driver/toolkit files to a project release;
- redistribute a Modular component merely because a build succeeds or the component is available at no charge;
- ship a project-built Mojo binary before verifying whether it embeds, links to, dynamically loads or requires a Modular component and whether that component is expressly identified as redistributable;
- redistribute any SDK component as a standalone library or tool;
- strip notices, labels, marks, licence text or required attribution from third-party code or artefacts;
- bypass licence capacity or usage restrictions;
- claim that the Mojo compiler/toolchain, model weights, CUDA, ROCm or other independent assets are licensed under Colibri's Apache-2.0 licence;
- publish mutable `latest` dependency references as a reproducible release lock; or
- start a public commercial/production deployment using Modular infrastructure before the current Modular terms and associated operational obligations are approved.

## Dependency review gate

Before a dependency is pinned for a release, and again before any public binary or production deployment, record the following in the release decision or dependency register:

1. **Identity:** exact package and artefact names, version, cryptographic digest, source URL and supplier.
2. **Terms:** governing licence version/date, licence URL or bundled text, and any separate terms accepted during download or installation.
3. **Role:** build-only, statically linked, embedded, dynamically linked, loaded at runtime, downloaded on demand, or system-provided.
4. **Composition:** software bill of materials plus platform-appropriate dependency inspection (`ldd`, `readelf`, `otool -L`, package metadata, or equivalent). Record all copied notices and licence files.
5. **Redistribution grant:** the exact authoritative documentation clause that identifies each shipped third-party file as redistributable for this application. Silence or marketing language does not pass this check.
6. **Use restrictions:** supported hardware, device-count/capacity limits, production/commercial conditions, telemetry, export controls, attribution, branding and notification requirements.
7. **Compatibility:** compiler, runtime, driver and target versions; clean-machine build/run evidence; and a version pin. Mojo precompiled artefacts require particular care because official guidance warns that compiler versions may be incompatible.
8. **Fallback:** evidence that disabling the dependency leaves the core workflow usable and produces a clear capability error rather than corrupting an accepted artefact.
9. **Approval:** engineering owner approval for source-only research use; project-owner approval for public releases; and qualified legal review before shipping uncertain proprietary components or beginning a commercial/production deployment.

Failure of any mandatory item blocks the affected release mode; it does not block unrelated research or the core runtime.

## Release-mode matrix

| Mode | Mojo/Modular posture | Decision |
| --- | --- | --- |
| Public source research release | Instructions and version pins; users install from an official channel | Allowed after normal source-licence review |
| Local or private-CI prototype | Toolchain installed by the licensed user/entity; no third-party redistribution | Allowed under the applicable terms |
| Public binary with no unapproved third-party component | Composition and clean-machine checks passed | Allowed after the review gate |
| Public binary containing or packaging Modular components | Every component needs an express redistribution grant and all distribution requirements | Blocked until positively approved |
| Hosted non-production research service | Review current use terms, privacy/telemetry and model/data licences | Case-by-case approval |
| Commercial or production deployment | Review current capacity, notification, branding and other operational terms | Blocked until project-owner and legal approval |

## Mojo-specific posture

As of this policy date, the Modular Community License:

- permits non-production, non-commercial SDK use without a device-capacity restriction;
- sets production/commercial capacity rules by device type;
- imposes notification, logo-reference and “Powered by Modular” obligations on entity production deployments;
- prohibits SDK redistribution except as expressly permitted; and
- permits redistribution only for SDK components identified in official documentation as Redistributable Components and only when the stated distribution requirements are met.

These statements are a release checklist summary, not a substitute for reading the current [Modular Community License](https://www.modular.com/legal/community). The gate must be repeated if the terms, SDK version or distribution shape changes.

Mojo source portability also does not remove vendor requirements. The official [system requirements](https://mojolang.org/docs/requirements/) describe runtime dependencies for NVIDIA, AMD and Apple GPU targets, while the official [packaging guidance](https://mojolang.org/docs/tools/packaging/) requires attention to compiler/runtime version compatibility.

## Fallback if Mojo cannot be shipped

The runtime boundary must preserve these options in order:

1. Run the imported Colibri C/CPU backend as the compatibility baseline.
2. Permit Colibri's existing optional CUDA backend where the user supplies a compatible driver/toolkit under its own terms.
3. Keep Python/PyTorch as the training and correctness-reference path described by the MVP PRD.
4. Mark Mojo capability unavailable for that worker or extension stage without failing the coordinator, corrupting accepted checkpoints, or invalidating other providers.
5. Evaluate another backend only through this same dependency gate; a fallback is not automatically approved merely because it is open source or vendor-neutral.

A Mojo licensing, packaging or compatibility failure therefore blocks only the Mojo extension and its release artefacts, not the core marketplace.

## Change and incident handling

- Re-run the gate for every major dependency upgrade, licence change, new target family, packaging change or hosted-service launch.
- Quarantine a release immediately if an unapproved third-party binary or missing notice is discovered; preserve the evidence and replace the artefact with a source-only or system-dependency release.
- Do not silently substitute a different runtime version after a release candidate has passed review.
- Record unresolved questions as blockers, not assumptions.

This policy is an engineering control and is not legal advice.
