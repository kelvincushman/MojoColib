# Contributing to MojoColib

MojoColib is an early research project. Contributions should make claims more
reproducible, narrow trust boundaries or add a measured capability without
weakening the reference paths.

## Before starting

- Read the [README](README.md), [product requirements](docs/mojocolib-training-prd.md)
  and [threat model](docs/threat-model.md).
- Open an issue before a broad protocol change, new runtime dependency or large
  hardware-specific implementation.
- Keep Mojo, CUDA, ROCm, Solana and model downloads optional. The default checks
  must remain offline and runnable without proprietary tooling.
- Do not commit credentials, private data, downloaded model weights, datasets,
  checkpoints or generated build outputs.

## Development workflow

1. Create a focused branch from `main`.
2. Add or update tests before changing protocol or state-machine behavior.
3. Keep identifiers, manifests and signed payloads deterministic and versioned.
4. Run the complete local check before opening a pull request:

   ```sh
   make check
   ```

5. Explain the change, its trust or compatibility impact, and any hardware or
   runtime path you could not test.

Changes beneath `engine/colibri/` must preserve upstream attribution, identify
local modifications and update the provenance record when the upstream pin
changes.

## Contribution terms

Unless a file states otherwise, contributions submitted to this repository are
licensed under the [Apache License 2.0](LICENSE), without additional terms or a
contributor licence agreement. By submitting a contribution, you confirm that
you have the right to provide it under those terms.

Third-party code, datasets, model assets and vendor binaries must not be copied
into the repository without a documented licence and provenance review. Mojo
or Modular components must also pass the distribution policy in
[ADR-0009](docs/adr/0009-licensing-posture.md).

## Security reports

Do not disclose a suspected vulnerability in a public issue. Follow
[SECURITY.md](SECURITY.md).
