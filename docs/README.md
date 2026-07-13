# MojoColib documentation

Use the documents in this order:

1. [Training marketplace PRD](mojocolib-training-prd.md) — authoritative MVP product requirements, priorities and acceptance tests.
2. [Training marketplace implementation plan](plans/mojocolib-training-mvp-plan.md) — sequenced engineering work, planned files, verification and release gates.
3. [Training and ML pipeline engineering brief](decentralized-ml-training-marketplace-mvp.md) — broader product and architecture rationale.
4. [Inference marketplace companion](decentralized-inference-marketplace-mvp.md) — later inference-network direction.
5. [Mojo/Colibri backend plan](plans/mojo-colibri-olmoe-plan.md) — independently gated Mojo inference and NVMe-streaming work.

Key implementation decisions are in [`docs/adr`](adr), with the exact Rust contract names in the [V1 protocol inventory](protocol-contracts-v1.md), the current security boundary in the [MVP threat model](threat-model.md), the imported engine pin in [upstream provenance](upstream-colibri-provenance.md), and runtime distribution rules in the [third-party runtime policy](third-party-runtime-policy.md).

## Current status

The implemented surface is a repository bootstrap plus Rust protocol, replay-safe state machines, deterministic compatibility scheduler and a runnable in-memory happy-path simulator. CI checks those Rust components, local documentation links, deterministic fixture structure and the imported Colibri CPU baseline.

Slice 0 is not yet complete: the tiny fixture does not train, checkpoint and resume; named product/technical owners and the real-model evidence lock are still required. The public source and synthetic fixture are Apache-2.0 licensed. No coordinator, worker, PyTorch training template, marketplace service or Mojo backend is implemented. The [root README](../README.md) is the concise built-versus-blocked status.

The core MVP is the checkpointed eight-trial LoRA sweep. Mojo, Solana and Rust-native training experiments are extensions and must not block the core training-marketplace proof.
