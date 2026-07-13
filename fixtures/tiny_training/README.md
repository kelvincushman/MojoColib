# Tiny offline training fixture

This directory contains the generated, deterministic input fixture for protocol, checkpoint/resume and chaos tests. It requires no model or dataset downloads and remains small enough for CPU CI. It is not the Slice 3 reference model and must not be used as evidence that Qwen fits a 16 GiB machine.

`generate.py` currently produces the 92 KB vocabulary, splits, recipe, expected preprocessing values and valid safetensors initial weights using only the Python standard library. `test_fixture.py` verifies reproducibility, manifest hashes, split separation, safetensors offsets and the 2 MiB limit. The actual PyTorch training/checkpoint golden is intentionally marked pending until a supported pinned Python/PyTorch environment is placed on the external development drive; the host currently has Python 3.14, no PyTorch and insufficient internal disk for that dependency stack.

## Fixture contract

The generator must produce only repository-owned data and weights from a reviewed script and fixed seed `1729`. Generated outputs are committed; normal tests invoke `python3 generate.py --check` and never rewrite them.

Required files:

| File | Contract |
| --- | --- |
| `manifest.json` | schema version, generator revision, seed, ordered file list, byte sizes and SHA-256 hashes |
| `model_config.json` | exact tiny causal-transformer architecture |
| `model.safetensors` | deterministic initial weights; no pickle |
| `vocab.json` | explicit 64-token ASCII vocabulary and special-token IDs; no external tokenizer code |
| `train.jsonl` | 32 ordered, project-generated algorithmic examples with stable IDs |
| `eval.jsonl` | 8 disjoint ordered examples with stable IDs |
| `recipe.json` | all training, LoRA, optimizer, scheduler, checkpoint and determinism fields |
| `expected.json` | golden input IDs, masks, step metrics, checkpoint manifest and final hashes/tolerances |
| `LICENSE` | project licence covering every generated fixture byte |

No file may contain personal data, prose copied from another dataset, a network URL required at runtime, or an opaque executable payload.

## Proposed tiny model and recipe

- decoder-only causal transformer;
- 2 layers, hidden size 32, 4 attention heads, context length 32 and vocabulary size 64;
- tied input/output embeddings, no dropout, FP32 CPU baseline;
- LoRA only on attention query and value projections, rank 2, alpha 4;
- AdamW with every field explicit, constant learning rate and no fused optimizer;
- micro-batch 2, gradient accumulation 2, exactly 12 optimizer steps;
- checkpoint after step 5; resumed run completes steps 6–12;
- deterministic algorithms enabled and all Python, NumPy and PyTorch RNG states captured.

The generator must fail if the model exceeds 100,000 parameters or the complete fixture exceeds 2 MiB.

## Required test paths

1. **Golden preprocessing:** JSONL records produce the exact token IDs, attention masks and labels in `expected.json`.
2. **Uninterrupted:** train steps 1–12 and emit adapter, optimizer, scheduler, RNG, metrics and lineage artifacts.
3. **Resume:** train steps 1–5, atomically write and promote a checkpoint, start a fresh process, restore all state and complete steps 6–12.
4. **Network denial:** repeat loading and execution with outbound networking disabled and empty user caches.
5. **Corruption:** reject a changed hash, missing tensor, wrong shape/dtype, unlisted adapter module and non-finite value.
6. **Compatibility:** reject a changed model/config/tokenizer hash, recipe field, dependency lock or backend compatibility key before loading optimizer state.

## Golden assertions

For the locked CPU environment, uninterrupted and resumed runs must have:

- identical ordered record IDs, optimizer steps and metric-step keys;
- identical promoted-checkpoint lineage;
- exact final adapter, optimizer, scheduler and RNG artifact hashes;
- final training NLL at least 20% below initial training NLL;
- finite training and evaluation NLL at every recorded step.

Accelerator smoke runs may use `atol=1e-3, rtol=1e-2` for adapter tensors and an absolute final evaluation-NLL tolerance of `1e-3` against an uninterrupted run on the same backend. They must not replace the CPU golden test. Cross-backend byte equality is not expected.

## Reproducibility boundary

PyTorch does not promise identical results across releases, commits or platforms. The golden hashes are therefore valid only for the pinned dependency and platform lock recorded in `manifest.json`. Regeneration after a dependency change requires a reviewed manifest diff and a new golden record; tests must never silently rewrite expected outputs.

See [ADR-0003](../../docs/adr/0003-reference-training-lock.md) for the real-model gate and [PyTorch reproducibility guidance](https://docs.pytorch.org/docs/stable/notes/randomness.html) for the upstream limitation.
