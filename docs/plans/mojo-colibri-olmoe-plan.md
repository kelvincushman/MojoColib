# Mojo-Colibri implementation plan

Status: planning complete; no source implementation has started.

## 1. Outcome

Build an experimental fork of Colibri that keeps its proven C model/runtime and NVMe expert-streaming design, but adds an optional Mojo accelerator backend. The first real checkpoint will be `allenai/OLMoE-1B-7B-0125-Instruct`, followed by a second model adapter only after the backend is correct and measurably useful.

The MVP succeeds when the same converted OLMoE checkpoint can run in three reproducible modes:

1. C/CPU reference.
2. Mojo/Metal with resident hot experts.
3. Mojo/Metal with a deliberately constrained expert cache and external-NVMe streaming.

The point of the MVP is not to claim that Mojo automatically beats CUDA or Metal. It is to establish a portable accelerator boundary, measure where Mojo helps, and identify where disk I/O or dispatch overhead dominates.

## 2. Confirmed environment and constraints

- Host: Apple M2, 8 GPU cores, 16 GiB unified memory.
- OS: macOS 15.2; Mojo currently requires macOS 15+ on Apple silicon.
- Toolchain present: Xcode, Apple Clang, Metal compiler and Homebrew OpenMP.
- Mojo is not installed yet.
- Internal free storage is only about 4.8 GiB, so checkpoints and converted weights must live on the external NVMe.
- A 500 GB USB4 40 Gb/s external NVMe has been ordered as the dedicated model and benchmark volume; its usable capacity and sustained performance remain to be measured after arrival.
- The 16 GiB is shared by macOS, CPU and GPU; it must not be treated as 16 GiB of dedicated VRAM.
- Safe initial process target: <=10 GiB resident/Metal allocation, with an absolute experimental ceiling of 11 GiB. Runs that cause sustained swap are invalid benchmarks.
- External-drive requirement: USB4 advertised at 40 **Gb/s**, not 40 Mb/s. Benchmark actual performance rather than trusting the label.

Recommended drive setup after arrival:

- Use APFS if the drive will primarily stay with the Mac; avoid exFAT for the main benchmark volume unless cross-platform mounting is essential.
- Keep at least 15% free space so conversion, temporary files and SSD garbage collection have room.
- Store source checkpoints, converted checkpoints, fixture data and benchmark results in separate directories.
- Run a short benchmark and a 20–30 minute sustained-read soak before downloading models; record throughput, latency, temperature and disconnects.
- Begin with OLMoE. The nominal 500 GB capacity is ample for OLMoE and Qwen experiments, but a roughly 370 GB converted GLM-5.2 image would leave inadequate working headroom for comfortable conversion and benchmarking.

## 3. Why OLMoE is the first model

`allenai/OLMoE-1B-7B-0125-Instruct` is the best first real checkpoint because:

- It is a sparse MoE with 7B total and about 1B active parameters per token.
- Its Apache-2.0 model licence is compatible with an experimental open-source fork.
- Colibri already contains `c/olmoe.c` and `c/tools/convert_olmoe.py`, so model correctness and backend correctness can be separated.
- The converter already creates row-wise int8 expert weights with float scales, giving the Mojo backend one initial quantized format to implement.
- The checkpoint is small enough for the M2 experiment but large enough to exercise routing, expert caching and streaming.
- Because it can mostly fit locally, the same model can provide a fully resident control and an artificially constrained streaming test. That is much cleaner than beginning with a model that only works when every subsystem is perfect.

Primary references:

- Colibri overview and existing OLMoE target: <https://github.com/JustVugg/colibri>
- OLMoE checkpoint and licence: <https://huggingface.co/allenai/OLMoE-1B-7B-0125-Instruct>
- Mojo Apple/Metal requirements: <https://mojolang.org/docs/requirements/>
- Mojo compilation targets: <https://mojolang.org/docs/tools/compilation/>

## 4. Scope boundaries

### Included in the MVP

- Fork and preserve upstream Colibri behaviour.
- Build and validate the existing CPU OLMoE path.
- Add an optional Mojo shared-library backend using a C ABI.
- Implement row-wise q8 expert kernels first.
- Fuse a complete expert MLP before attempting end-to-end performance claims.
- Add resident expert caching on the Mojo side.
- Add an experimental streamed/on-demand path and I/O-compute overlap.
- Create deterministic correctness fixtures and repeatable benchmarks.
- Run the real OLMoE instruct model and evaluate basic capabilities.
- Design the adapter seam needed for a second MoE architecture.

### Explicitly excluded from the MVP

- Rewriting all of `glm.c` or `olmoe.c` in Mojo.
- Supporting GLM-5.2 on the 16 GiB Mac.
- Claiming one universal binary for Apple, NVIDIA and AMD.
- Coral Edge TPU support; Mojo does not target Coral.
- Training or fine-tuning.
- Dense-model disk streaming; dense models do not have inactive expert weights to skip.
- A GUI or hosted service before the engine is correct and benchmarked.
- Production redistribution of the Mojo runtime until its current licence and redistributable components have been reviewed.

## 5. Architectural decision

Keep C as the host runtime and make Mojo an optional accelerator plugin.

```text
Tokenizer / model loop / routing / KV cache / disk I/O       C
                         |
                         | stable C ABI
                         v
Device context / expert handles / fused MoE kernels        Mojo
                         |
                         v
                    Apple Metal GPU
```

C continues to own:

- model configuration and tensor indexing;
- attention and KV state during the MVP;
- router decisions and route weights;
- LRU policy, disk reads, statistics and CPU fallback;
- CLI, sampling and deterministic replay.

Mojo owns:

- the accelerator context;
- Mojo/Metal buffers and their lifetimes;
- persistent resident expert handles;
- q8 matmul and fused expert kernels;
- batched routed-expert execution;
- synchronization and backend error reporting.

This preserves Colibri's core value while creating a backend that can later be rebuilt for Metal, CUDA and HIP from the same Mojo source. Performance-specific branches are allowed; source portability does not imply identical kernels on every GPU.

Upstream implementation anchors for the execution phase:

- [`c/backend_cuda.h` lines 13–43](https://github.com/JustVugg/colibri/blob/main/c/backend_cuda.h#L13-L43) is the existing opaque C backend pattern to generalize.
- [`c/backend_cuda.cu` lines 191–207](https://github.com/JustVugg/colibri/blob/main/c/backend_cuda.cu#L191-L207) shows why a per-matmul boundary is only suitable for bring-up: each call transfers activations in and results out.
- [`c/glm.c` lines 475–488](https://github.com/JustVugg/colibri/blob/main/c/glm.c#L475-L488) limits acceleration to persistent resident tensors today.
- [`c/glm.c` lines 1325–1382](https://github.com/JustVugg/colibri/blob/main/c/glm.c#L1325-L1382) contains the routed expert-block boundary that motivates the fused Mojo call.
- [`c/glm.c` lines 2443–2493](https://github.com/JustVugg/colibri/blob/main/c/glm.c#L2443-L2493) implements the current hot/pinned expert tier.
- [`c/olmoe.c`](https://github.com/JustVugg/colibri/blob/main/c/olmoe.c) already owns the OLMoE model loop, row-wise quantized expert cache and on-demand reads; the MVP should add only narrow backend calls there.
- [`c/tools/convert_olmoe.py`](https://github.com/JustVugg/colibri/blob/main/c/tools/convert_olmoe.py) defines the initial on-disk q8 expert representation the Mojo kernel must match.
- [`c/Makefile` lines 503–527](https://github.com/JustVugg/colibri/blob/main/c/Makefile#L503-L527) contains the existing Apple Clang/OpenMP build path that the optional Mojo target must preserve.

## 6. Planned repository structure

The exact names can adjust to upstream conventions after cloning, but ownership should remain clear:

```text
c/
  backend_accel.h              generic host-facing accelerator contract
  backend_mojo_shim.c          optional dynamic loading and C-side validation
  olmoe.c                      minimal integration points only
  Makefile                     optional MOJO=1 build; default build unchanged
  mojo/
    backend.mojo               exported C ABI and context lifecycle
    types.mojo                 fixed-width ABI descriptors
    q8_matmul.mojo             correctness-first q8 kernel
    expert_mlp.mojo            gate/up/activation/down fusion
    routed_moe.mojo            batched routed-expert execution
  tests/
    test_backend_mojo.c        ABI, lifecycle and negative tests
    test_mojo_q8.c             numerical parity fixtures
    test_mojo_moe.c            fused expert and routing parity
  tools/
    make_olmoe_fixture.py      deterministic tiny model and expected outputs
    benchmark_mojo.py          reproducible CPU/Mojo benchmark runner
    evaluate_capabilities.py   fixed prompt suite and result capture
docs/
  MOJO_BACKEND.md              build, limitations, measurements and licences
```

## 7. C ABI design

The ABI must expose only fixed-width scalars, pointers, sizes and opaque handles. No Mojo collections or object layouts cross the boundary.

Initial operations:

```c
typedef struct ColiMojoContext ColiMojoContext;
typedef struct ColiMojoExpert ColiMojoExpert;

int coli_mojo_init(const ColiMojoInit *cfg, ColiMojoContext **out);
void coli_mojo_shutdown(ColiMojoContext *ctx);
int coli_mojo_mem_info(ColiMojoContext *ctx, ColiMojoMemInfo *out);

int coli_mojo_expert_upload(
    ColiMojoContext *ctx,
    const ColiQ8Tensor *gate,
    const ColiQ8Tensor *up,
    const ColiQ8Tensor *down,
    ColiMojoExpert **out);

void coli_mojo_expert_free(ColiMojoContext *ctx, ColiMojoExpert *expert);

int coli_mojo_moe_block(
    ColiMojoContext *ctx,
    ColiMojoExpert *const *experts,
    const float *activations,
    const ColiRoute *routes,
    float *output,
    const ColiMoeShape *shape);

const char *coli_mojo_last_error(ColiMojoContext *ctx);
```

ABI rules:

- Include `abi_version` and `struct_size` in public descriptor structures.
- Define alignment, row order, quantization packing and scale ownership explicitly.
- Return status codes; never throw through the C boundary.
- Make every allocation owner and destructor explicit.
- Reject invalid shapes, null pointers, overflow and unsupported quantization before dispatch.
- Keep the default C build independent of Mojo by loading the Mojo library only when requested.
- Do not assume arbitrary C allocations can be wrapped as zero-copy Metal buffers; treat any such capability as an optimization that must be proven.

## 8. Implementation phases and gates

### Phase 0 — Reproducible baseline

Work:

- Clone/fork Colibri and record the upstream commit.
- Build the unmodified default C targets on Apple silicon.
- Run C and Python tests plus the existing OLMoE reference path.
- Attach and benchmark the external NVMe with Colibri's I/O benchmark and an expert-sized random-read trace.
- Install a pinned Mojo compiler release, not nightly.
- Verify `DeviceContext` sees the Apple GPU and compile a minimal shared library callable from C.
- Record baseline OS, compiler, Mojo, drive, filesystem and thermal state.

Gate:

- Existing tests pass before source changes.
- Mojo can return a value through a C ABI shared library.
- The external drive has at least 40 GiB free for OLMoE development; 80 GiB is recommended so source, converted weights, fixtures and results can coexist.
- No benchmark proceeds if the drive is actually limited to roughly 40 Mb/s.

Estimated effort: 1–2 days.

### Phase 1 — Deterministic fixtures and backend contract

Work:

- Generate tiny q8 tensors with awkward dimensions as well as OLMoE-shaped tensors.
- Capture CPU reference outputs for matmul, one expert MLP and a routed MoE block.
- Add lifecycle, invalid-shape, allocation-failure and repeated-init tests.
- Add a backend selector such as `COLI_ACCEL=cpu|mojo`; `cpu` remains the default.

Gate:

- Fixture generation is deterministic from a recorded seed.
- Running without the Mojo library behaves exactly as upstream CPU Colibri.
- Requesting Mojo with an unavailable library gives an actionable startup error, not a crash or silent fallback.

Estimated effort: 2–3 days.

### Phase 2 — q8 matmul bring-up

Work:

- Implement the exact row-wise q8 + float-scale format used by `convert_olmoe.py`.
- Begin with a simple correctness kernel, then tile work for Apple GPU threadgroups.
- Benchmark host-to-device copy, dispatch, compute and device-to-host copy separately.
- Cover single-token decode shapes and batched prefill shapes.

Gate:

- Numerical error versus the C q8 path is <=2e-4 relative to `max(1, |reference|)` across the fixture suite.
- Address Sanitizer passes on the C shim; Mojo negative tests report errors cleanly.
- No speed claim is made at this stage. Per-matmul dispatch is a bring-up seam, not the intended production seam.

Estimated effort: 4–7 days.

### Phase 3 — Fused expert MLP

Work:

- Fuse gate projection, up projection, SiLU, elementwise product and down projection.
- Keep activations on the GPU for the entire expert operation.
- Add persistent opaque expert handles so weights are uploaded once and reused.
- Specialize for the OLMoE hidden/intermediate dimensions while retaining a generic fallback.
- Measure memory allocation and reuse; prohibit per-token allocation in the hot path.

Gate:

- Fused output meets the same numerical tolerance as the staged CPU reference.
- After warm-up, no new device allocations occur during token decode.
- Resident OLMoE-shaped expert execution is at least 1.25x faster than the C/NEON expert path, or the project records a failed performance gate and profiles the cause before integration expands.

Estimated effort: 5–8 days.

### Phase 4 — Batched routed MoE block

Work:

- Batch all tokens/routes for one MoE layer into a single backend call.
- Resolve duplicate expert IDs once per batch.
- Scatter weighted expert outputs without returning intermediate gate/up buffers to the CPU.
- Keep routing in C initially; only the expert block crosses the ABI.
- Add resident-set admission based on actual unified-memory availability.

Gate:

- Tiny OLMoE fixture produces the same selected experts and the same greedy tokens as the CPU reference for at least 32 generated tokens.
- A ten-prompt deterministic replay produces matching top-1 tokens for at least 99.5% of positions; every mismatch includes logit-error and routing-margin diagnostics.
- Warm end-to-end decode is not more than 5% slower than CPU. The performance target for continuing optimization is >=1.20x warm decode speedup.

Estimated effort: 5–10 days.

### Phase 5 — Real OLMoE checkpoint

Work:

- Download source weights directly to the external NVMe.
- Convert with the existing OLMoE converter and generate a manifest containing hashes, tensor shapes and byte counts.
- Add a read-only memory/storage planner for OLMoE if the existing planner does not cover it.
- Run short-context CPU and Mojo generation before longer benchmarks.
- Validate chat-template handling and stop tokens against the model card.

Gate:

- Startup remains below the 10 GiB target or automatically reduces the resident expert tier until it does.
- No sustained swap; no GPU allocation failure; no corrupted or partially converted shard is accepted.
- The model completes a fixed capability suite covering instruction following, summarization, extraction, arithmetic, simple coding and multi-turn chat.
- Mojo and CPU use the same converted weights, prompts, sampler and token limits.

Estimated effort: 3–5 days after kernel integration.

### Phase 6 — NVMe streaming and overlap experiment

Work:

- Force cache budgets below the full expert set so the small checkpoint exercises genuine cache misses.
- Compare three policies: CPU cold expert, upload-on-miss Mojo expert, and pinned Mojo hot expert.
- Implement asynchronous `begin/end` or equivalent only after traces show overlap is possible.
- Pipeline next-expert disk reads with current resident-expert GPU work.
- Record bytes read/token, cache hits, upload bytes and wait time.

Gate:

- Streaming mode stays within its configured RAM/GPU budget for a 30-minute run.
- Output correctness is independent of cache size and eviction order.
- Overlap demonstrably reduces blocked wall time in traces; otherwise the asynchronous complexity is not retained.
- Cold performance is reported honestly as I/O-limited when appropriate. Mojo is not considered a failure merely because it cannot beat the NVMe ceiling.

Estimated effort: 1–2 weeks.

### Phase 7 — Second architecture: Qwen1.5-MoE-A2.7B

Only start this after OLMoE passes correctness and the backend boundary is stable.

Why this model:

- 14.3B total, 2.7B active parameters.
- 60 routed experts with top-4 activation plus four shared experts.
- At q4/q8 mixed precision it is plausible on a 16 GiB unified-memory machine with streaming.
- It exercises shared experts and a different router/attention family, proving whether the design is becoming a framework rather than an OLMoE patch.

Work:

- Extract model-independent tensor storage, cache and accelerator interfaces from OLMoE without changing behaviour.
- Add a model descriptor/adapter for tensor names, attention, router normalization, shared experts and chat template.
- Extend the converter with resumable per-shard q4/q8 conversion.
- Reuse the same Mojo expert handle and routed-block ABI.

Gate:

- No Qwen-specific fields enter the generic backend ABI.
- OLMoE regression tests remain unchanged and passing.
- A tiny Qwen-shaped oracle reaches token parity before downloading the full checkpoint.
- Full-model work starts only if the converted storage forecast fits the external drive and the <=10 GiB runtime budget.

Estimated effort: 2–4 weeks.

### Phase 8 — Legacy NVIDIA revival experiment

This is a separate compatibility/performance lane after the Apple backend is correct. It must not block the M2 MVP.

Primary target: Tesla P40 24 GB (Pascal, `sm_61`). It is attractive for Colibri because NVIDIA specifies 346 GB/s memory bandwidth and 47 INT8 TOPS, but it is a passive 250 W server card without modern Tensor Cores.

Control target: Tesla T4 16 GB (Turing, `sm_75`). T4 is the oldest NVIDIA generation currently listed as known-compatible by Mojo and provides the control needed to distinguish a general backend bug from a Pascal/toolchain problem.

Compatibility constraints:

- Mojo documents pre-Turing cards as not supported out of the box, but provides an experimental route through `MODULAR_NVPTX_COMPILER_PATH` and a system `ptxas` compatible with the GPU.
- CUDA 13 removed offline compilation and library support for Maxwell, Pascal and Volta. Keep an isolated CUDA 12.x toolchain for P40/P100/M40-era tests; do not make the main project depend on it.
- Mojo on NVIDIA still loads NVIDIA's `libcuda` driver and generates PTX. This reduces CUDA-language/toolkit coupling in our source, but it does not eliminate dependence on NVIDIA's driver stack.
- Apple silicon Macs cannot host or drive an NVIDIA PCIe card. Legacy tests require a separate Linux x86-64 machine with a full PCIe slot, suitable power delivery and forced airflow for passive Tesla cards.

Initial card matrix:

| Card | Architecture | Memory | Role | Initial verdict |
|---|---|---:|---|---|
| Tesla P40 | Pascal `sm_61` | 24 GB | primary legacy INT8 experiment | worthwhile, experimental Mojo path |
| Tesla T4 | Turing `sm_75` | 16 GB | oldest supported Mojo control | recommended control |
| Tesla P100 | Pascal `sm_60` | 16 GB | FP16/HBM comparison | secondary; less aligned with q8 |
| Tesla M40 | Maxwell | 24 GB | capacity-only legacy case | low priority; weak modern INT8 path |
| Tesla K80 | Kepler, dual GPU | 2 x 12 GB | historical only | reject; not one coherent 24 GB device |

Work:

- Build the exact same q8 and fused-MoE fixtures for Apple Metal, T4 and P40.
- Establish a direct CUDA 12.x result on P40 as a hardware baseline, then run the Mojo/PTX build against the same fixtures.
- Compile separate target artifacts; never pretend one `.dylib`/`.so` is universal.
- Record driver, CUDA toolkit, `ptxas`, Mojo compiler and generated PTX versions.
- Benchmark q8 resident experts first; only then test hot-expert tiers and streaming.
- Add an optional open Vulkan/SPIR-V backend investigation later if the objective becomes running old NVIDIA hardware without a CUDA userspace toolchain. This would be a separate backend, not a Mojo feature promised by the current toolchain.

Gate:

- T4 must pass the full fixture suite before diagnosing P40-specific failures.
- P40 success requires deterministic q8/fused-MoE correctness and a documented reproducible toolchain container or environment lock.
- P40 must beat the host CPU on resident fused-expert shapes to justify continued tuning.
- Power, cooling and complete system cost must be reported alongside tokens/s; cheap cards are not useful if deployment requirements erase the saving.
- No claim that NVIDIA intentionally throttles a card is made without direct evidence. Report the observable issue accurately: deprecated compiler/library support, missing new instructions, driver requirements or measured performance.

Official references:

- Mojo NVIDIA compatibility and pre-Turing workaround: <https://mojolang.org/docs/requirements/>
- NVIDIA CUDA 13 architecture removals: <https://docs.nvidia.com/cuda/cuda-features-archive/index.html>
- NVIDIA Tesla P40 specifications: <https://www.nvidia.com/content/dam/en-zz/Solutions/Data-Center/tesla-product-literature/184427-Tesla-P40-Datasheet-NV-Final-Letter-Web.pdf>

### Phase 9 — Cross-vendor proof, later

- Rebuild the same Mojo kernel source for one NVIDIA CUDA target and one AMD HIP target when hardware is available.
- Keep vendor-specific tuning behind compile-time branches.
- Treat each platform as a separate build and benchmark artifact.
- Do not call the framework portable until correctness tests pass on at least Apple plus one non-Apple GPU.

## 9. Benchmark protocol

Every performance claim must include:

- exact git commit and Mojo compiler version;
- model/converted-manifest hash;
- power mode, thermal state and drive filesystem;
- memory/cache budgets;
- prompt and generated-token counts;
- five warm-up runs and at least ten measured runs;
- median and p95, not only the fastest result.

Required scenarios:

| Scenario | Prompt/output | Cache condition | Purpose |
|---|---:|---|---|
| Kernel q8 | OLMoE shapes | resident | compute and dispatch ceiling |
| Fused expert | 1 and 8 tokens | resident | fusion benefit |
| Warm decode | 128/128 tokens | hot resident tier | normal interactive path |
| Prefill | 512/32 tokens | warm | batching behaviour |
| Forced streaming | 128/64 tokens | constrained cache | NVMe/cache policy |
| Cold replay | fixed routes | empty caches | I/O ceiling |
| Stability | repeated generation for 30 min | mixed | leaks, thermals, eviction |

Metrics:

- time to first token;
- prefill tokens/s and decode tokens/s;
- median/p95 token latency;
- kernel, dispatch, copy, disk-wait and synchronization time;
- peak RSS and Metal allocation;
- expert cache hit rate and evictions;
- bytes read and uploaded per token;
- output/routing mismatch count;
- thermal throttling indicators when available.

## 10. Capability evaluation

Performance and model quality must be kept separate.

Create a versioned prompt suite with at least 50 prompts:

- 10 instruction-following/format constraints;
- 10 summarization and extraction;
- 10 factual/common-sense questions;
- 10 arithmetic/reasoning tasks;
- 5 short coding tasks;
- 5 multi-turn chat continuations.

For each run, save prompt, settings, token IDs, decoded output, latency and backend. Use greedy decoding for backend parity and a fixed seed for qualitative sampled runs.

Acceptance:

- CPU and Mojo complete all prompts without crashes or malformed UTF-8.
- Greedy backend parity meets the 99.5% token target, with all differences diagnosed.
- Capability results are described as properties of OLMoE, not evidence that Mojo improves intelligence.
- A backend is rejected if numerical changes materially alter routing or output quality even when it is faster.

## 11. Test matrix

### Unit

- ABI versioning and descriptor sizes.
- q8 unpack/scale semantics and awkward dimensions.
- SiLU/product/down fusion.
- route accumulation, repeated expert IDs and zero-route cases.
- overflow, invalid shapes, null buffers and allocation failures.
- expert upload/free and repeated context lifecycle.

### Integration

- C program loads Mojo `.dylib`, uploads an expert and matches CPU output.
- CPU-only build does not require Mojo.
- tiny OLMoE fixture runs CPU and Mojo from identical converted tensors.
- cache eviction never frees an expert still referenced by an in-flight operation.
- forced cache misses return correct output.

### End-to-end

- real OLMoE chat generation on CPU and Mojo.
- resident, constrained-cache and cold-cache benchmark modes.
- interrupted conversion resumes safely.
- missing/corrupt model files fail before inference.
- 30-minute stability run stays within budget and produces no increasing allocation trend.

### Observability

- structured per-run JSON summary;
- optional per-layer timing trace;
- counters for disk reads, uploads, cache hits, evictions and Mojo calls;
- one command that reproduces every table in the benchmark report.

## 12. Stop/go rules

Proceed from q8 bring-up to fusion only after numerical parity.

Proceed from fusion to full OLMoE only if:

- persistent resident execution works without per-token allocations; and
- profiling shows a credible path to end-to-end improvement.

Proceed to async streaming only if traces show GPU work and disk reads can overlap materially.

Proceed to Qwen only if:

- OLMoE is correct and stable;
- the ABI has survived the real checkpoint without model-specific leakage; and
- the storage and memory forecast fits the actual external drive and M2 budget.

Pause Mojo performance work and publish a negative result if, after fused-block implementation:

- dispatch/copy/synchronization remains more than 40% of warm token time;
- warm end-to-end decode is consistently >10% slower than the CPU path; and
- profiling reveals no additional removable round trips.

A negative performance result does not invalidate the portable ABI or the model-adapter work; it changes the next priority to CPU/NEON or direct Metal comparison.

## 13. Principal risks and mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| 16 GiB unified memory is overcommitted | swap destroys measurements or process dies | 10 GiB target, admission planner, external weights, no full-cache assumption |
| Internal disk lacks space | downloads fail or macOS becomes unstable | store all checkpoints/build caches/results on external NVMe; require free-space preflight |
| Per-matmul GPU calls are slower than CPU | false conclusion that Mojo cannot help | use matmul only for bring-up; performance seam is fused routed MoE block |
| Disk dominates cold inference | GPU work shows little end-to-end gain | report resident/warm/cold separately and measure bytes/wait time |
| Mojo APIs/compiler change | build breaks | pin compiler, lock manifest, CI against pinned release, upgrade intentionally |
| Mojo redistribution terms conflict with “open-source framework” language | release/legal risk | distribute source and build instructions first; keep Mojo optional; review Community Licence before binaries |
| Floating-point reduction changes routing | token divergence | route-margin diagnostics, deterministic fixtures, conservative tolerances |
| OLMoE-specific assumptions pollute the backend | second model becomes a rewrite | generic ABI contains only expert tensors/routes/shapes; Qwen gate tests this explicitly |
| Apple optimization does not transfer to CUDA/HIP | portability claim is overstated | separate correctness portability from performance portability; test other hardware later |
| Pascal support requires frozen legacy tooling | fragile builds and security/maintenance burden | isolate CUDA 12.x/`ptxas`, lock versions, keep T4 as supported control, never make legacy support the default |
| Passive Tesla cards are installed without server airflow | overheating, throttling or hardware damage | require forced airflow, temperature logging and suitable power delivery before sustained tests |
| External enclosure throttles thermally | inconsistent cold results | long I/O soak, temperature monitoring and median/p95 reporting |

## 14. Deliverables

MVP deliverables:

1. Fork pinned to an upstream Colibri commit.
2. Optional Mojo/Metal shared-library backend with a versioned C ABI.
3. q8 and fused routed-expert tests.
4. Deterministic tiny OLMoE oracle.
5. Converted OLMoE instructions and manifest tooling.
6. CPU/resident-Mojo/streamed-Mojo benchmark report.
7. Capability suite results.
8. Documentation stating measured limits and non-claims.
9. A written go/no-go decision for the Qwen adapter.

## 15. Effort forecast

- Correct Mojo/C backend prototype: approximately 2–3 focused weeks.
- Real OLMoE integration and trustworthy benchmarks: approximately 3–5 weeks total.
- Streaming overlap and robust tooling: approximately 5–7 weeks total.
- Second-model Qwen adapter and framework extraction: approximately 7–11 weeks total.

These are engineering ranges, not promises. Kernel profiling, Mojo API instability and model-oracle work are the largest sources of variance.

## 16. Recommended execution order

1. Clone and baseline upstream without changes.
2. Prepare external NVMe and validate actual I/O.
3. Pin/install Mojo and prove C-to-Mojo-to-Metal execution.
4. Build deterministic q8 fixtures.
5. Implement q8 matmul.
6. Fuse one expert MLP.
7. Batch a routed MoE block with persistent expert handles.
8. Integrate the tiny OLMoE fixture.
9. Convert and run the real OLMoE checkpoint.
10. Benchmark resident and streaming modes.
11. Decide from evidence whether to tune Mojo further.
12. Only then extract a model adapter and start Qwen1.5-MoE-A2.7B.

## 17. Definition of MVP completion

The MVP is complete only when all of the following are true:

- the unmodified CPU path still passes its tests;
- the Mojo backend is optional and fails cleanly when unavailable;
- q8, fused-expert and routed-block numerical tests pass;
- the tiny OLMoE oracle meets the token-parity gate;
- the real OLMoE model runs within the M2 memory budget;
- resident and streamed benchmarks are reproducible from one command;
- capability results are captured for CPU and Mojo;
- performance conclusions distinguish compute, dispatch, memory and NVMe limits;
- remaining portability and licensing limits are documented;
- a go/no-go decision for the second-model adapter is recorded.
