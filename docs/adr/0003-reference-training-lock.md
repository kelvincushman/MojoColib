# ADR-0003: Reference training lock

- Status: Proposed; evidence gate not yet passed
- Decision deadline: immediately before Slice 3 source work begins
- Owners: ML/platform lead and licence reviewer
- Scope: the first Apple 16 GiB and NVIDIA LoRA baseline

## Context

Slice 3 needs one reproducible model, tokenizer, dataset, training recipe and evaluation policy. A mutable model name is not a reproducibility boundary, and a model that runs for inference is not automatically suitable for training on both reference backends.

This ADR therefore defines the selection procedure now and defers the immutable lock until benchmark and licence evidence exists. It does not authorize model downloads or claim that either candidate already fits the Apple reference machine.

## Candidate order

Candidates are evaluated in this fixed order:

1. `Qwen/Qwen2.5-1.5B-Instruct` (default). The official model card reports 1.54B parameters, native Transformers support and an Apache-2.0 licence.
2. `Qwen/Qwen2.5-0.5B-Instruct` (fallback). The official model card reports 0.49B parameters, the same `qwen2` Transformers architecture and an Apache-2.0 licence.

Both remain candidates until their exact repository commit, files and licence are captured. No candidate may exceed 2B parameters, require `trust_remote_code=True`, or use a licence other than Apache-2.0 for the baseline. A new fallback requires an ADR amendment and the same gate; it cannot be selected ad hoc after a failed run.

## Decision

Use ordinary LoRA, not QLoRA, for the cross-vendor baseline. Select the 1.5B candidate if and only if it passes every gate below on both the Apple and NVIDIA references. If it fails any hard gate, run the 0.5B fallback through the identical procedure. If the fallback also fails, Slice 3 is blocked; quantization or a higher-memory machine is a separately approved scope change.

### Gate A: immutable provenance and licence

Before any benchmark is accepted, record:

- the Hugging Face repository commit SHA, model and tokenizer revision;
- SHA-256 and byte size for every consumed config, tokenizer and safetensors file;
- the exact Apache-2.0 licence bytes and a completed redistribution/fine-tuning review;
- parameter count from the resolved config/weights, proving it is no larger than 2B;
- `trust_remote_code=False` loading in the locked Transformers release;
- exact Python, PyTorch, Transformers, PEFT, safetensors and OS versions;
- the dataset source or generator revision, licence, content manifest and SHA-256;
- immutable train/evaluation record IDs and proof that no private, personal or regulated data is present.

Any changed byte invalidates the evidence and reopens this gate.

### Gate B: proposed benchmark recipe

The following is the initial recipe. The accepted evidence packet must replace every `TBD` with a value and then freeze it:

| Field | Proposed value |
| --- | --- |
| Template | `hf-peft-sft-v1` |
| Model | candidate commit SHA, not `main` |
| Sequence length | 512 tokens |
| Micro-batch | 1 |
| Gradient accumulation | 8 |
| LoRA targets | `q_proj`, `v_proj` |
| LoRA rank / alpha / dropout | 8 / 16 / 0.0 |
| Optimizer | AdamW; learning rate and all optimizer fields `TBD` |
| Precision | Apple MPS FP16; NVIDIA CUDA BF16 when admitted, otherwise FP16 |
| Quantization | none |
| Gradient checkpointing | enabled |
| Seed | 1729 |
| Train/evaluation steps | `TBD`, long enough to include at least two promoted checkpoints |

The benchmark runs from a pre-populated read-only cache with outbound networking disabled. Each backend performs one uninterrupted run and one interrupted/resumed run from a promoted checkpoint. The evidence packet includes raw logs, environment lock, capability report, metrics, resource samples, checkpoint manifest and adapter manifest.

### Gate C: hardware and runtime admission

The Apple reference is an Apple Silicon machine with 16 GiB unified memory using the PyTorch MPS backend. The NVIDIA reference records GPU model, VRAM, driver and CUDA runtime. Both must use versions admitted by their fixture; backend family alone is not sufficient.

A candidate passes only when both machines meet all of these conditions:

- no OOM, process crash, non-finite value or unsupported-operation fallback;
- no CPU fallback unless the locked template explicitly declares and times that operation;
- Apple memory pressure remains green, process plus MPS allocations remain below the ADR limit, and the controlled run records zero swap-out growth;
- NVIDIA peak allocated and reserved VRAM is recorded and leaves at least 1 GiB or 10% physical VRAM free, whichever is larger;
- the final adapter is below 500 MB and only declared LoRA modules change;
- checkpoint promotion, reload and second-run resume complete with optimizer, scheduler, step and RNG state restored;
- base and adapter load with outbound networking disabled.

Throughput and energy are recorded for later economics but are not hard selection gates in this ADR.

### Gate D: metrics and tolerances

The primary training metric is held-out mean token negative log-likelihood (NLL), computed over a fixed ordered evaluation manifest with padding and prompt tokens masked exactly as declared. Perplexity may be reported as `exp(NLL)` but is not the canonical comparison value.

Hard correctness rules:

- processed record IDs, optimizer-step count and checkpoint lineage match exactly;
- all losses, gradients and adapter tensors are finite;
- the learnable canary subset's final training NLL is at least 10% below its initial NLL;
- on the same backend and locked environment, uninterrupted versus resumed final evaluation NLL differs by at most `1e-3` and adapter tensors by `atol=1e-3, rtol=1e-2`;
- Apple versus NVIDIA final evaluation NLL differs by at most 5% relative to the lower value;
- cross-backend adapter hashes and tensors are not required to match; each adapter is judged under the same frozen evaluation policy.

If two model candidates were ever eligible, the fixed order decides: the 1.5B default is selected when it passes. There is no post-hoc metric cherry-picking. Within the later eight-trial sweep, lowest held-out NLL wins; an NLL difference of at most 0.5% is a tie, broken by lower peak memory and then higher measured tokens/second.

## Immutable lock record

Immediately before Slice 3, change this ADR to `Accepted` and append one evidence-backed lock record containing:

- selected model repository and commit SHA;
- model/config/tokenizer file hashes;
- dataset generator/source revision, licence, manifest hash and ordered split IDs;
- exact prompt/chat template and label-masking policy;
- every recipe value in Gate B;
- dependency lock and platform compatibility keys;
- primary metric implementation hash, tolerances and tie-breaker;
- Apple and NVIDIA evidence artifact hashes;
- licence-review approver and date.

Slice 3 code must reject a job whose compatibility key differs from this record. Updating any locked field requires a new ADR decision and new baseline evidence.

## Tiny fixture relationship

`fixtures/tiny_training/` is the offline protocol, checkpoint and chaos-test fixture. It proves deterministic mechanics without external downloads. It does not establish Qwen quality, memory fit or licence admission and cannot satisfy this ADR's hardware gate by itself.

## Primary sources

- [Qwen2.5-1.5B-Instruct model card](https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct) and [Apache-2.0 licence](https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct/blob/main/LICENSE)
- [Qwen2.5-0.5B-Instruct model card](https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct) and [Apache-2.0 licence](https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct/blob/main/LICENSE)
- [Transformers Qwen2 implementation](https://huggingface.co/docs/transformers/model_doc/qwen2)
- [PEFT LoRA reference](https://huggingface.co/docs/peft/main/en/package_reference/lora)
- [PyTorch reproducibility guidance](https://docs.pytorch.org/docs/stable/notes/randomness.html)
- [Apple: accelerated PyTorch training with MPS](https://developer.apple.com/metal/pytorch/)
