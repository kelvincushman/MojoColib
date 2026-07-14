# MojoColib documentation

Use the documents in this order:

1. [Training marketplace PRD](mojocolib-training-prd.md) — authoritative MVP product requirements, priorities and acceptance tests.
2. [Training marketplace implementation plan](plans/mojocolib-training-mvp-plan.md) — sequenced engineering work, planned files, verification and release gates.
3. [Training and ML pipeline engineering brief](decentralized-ml-training-marketplace-mvp.md) — broader product and architecture rationale.
4. [Inference marketplace companion](decentralized-inference-marketplace-mvp.md) — later inference-network direction.
5. [Mojo/Colibri backend plan](plans/mojo-colibri-olmoe-plan.md) — independently gated Mojo inference and NVMe-streaming work.
6. [Control-plane UI/UX plan](plans/mojocolib-control-plane-ui.md) — the product-owned, fixture-backed operator-console increment.
7. [Network topology ADR](adr/0010-network-topology-and-progressive-decentralisation.md) — progressive federation, authority and evidence gates.
8. [V2 settlement policy ADR](adr/0011-platform-fee-and-settlement-policy.md) — quote-bound platform-fee, funding and financial-finality policy.

Key implementation decisions are in [`docs/adr`](adr), with the exact Rust contract names in the [V1 protocol inventory](protocol-contracts-v1.md) and [V2 settlement inventory](protocol-contracts-v2.md), the current security boundary in the [MVP threat model](threat-model.md), the imported engine pin in [upstream provenance](upstream-colibri-provenance.md), and runtime distribution rules in the [third-party runtime policy](third-party-runtime-policy.md).

## Current status

The implemented surface is a repository bootstrap plus Rust protocol, replay-safe state machines (including V2 quote/funding/reservation/settlement lifecycle rules), a signed single-writer V2 reservation journal, deterministic compatibility scheduler, a runnable in-memory happy-path simulator, local artifact/worker foundations and a product-owned fixture-backed operator console. The console deliberately has no coordinator connection, mutations or production data. CI checks the Rust components, local documentation links, deterministic fixture structure and the imported Colibri CPU baseline.

Slice 0 is not yet complete: the tiny fixture does not train, checkpoint and resume; named product/technical owners and the real-model evidence lock are still required. The public source and synthetic fixture are Apache-2.0 licensed. No production coordinator or worker service, PyTorch training template, marketplace service, Solana program or Mojo backend is implemented. The local V2 journal is not a payment rail. The [root README](../README.md) is the concise built-versus-blocked status.

The core MVP is the checkpointed eight-trial LoRA sweep. Mojo, Solana and Rust-native training experiments are extensions and must not block the core training-marketplace proof.
