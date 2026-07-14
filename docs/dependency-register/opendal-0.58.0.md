# Dependency record: Apache OpenDAL 0.58.0

- **Status:** approved for the local filesystem artifact-client slice only
- **Owner:** control-plane owner
- **Review date:** 2026-07-13
- **Source:** <https://github.com/apache/opendal>
- **Registry package:** <https://crates.io/crates/opendal/0.58.0>
- **Crate digest (SHA-256):** `77d02c6564e376d3670aaf66ad886cd34f83c0aca407b6364777c624a63e0e2d`
- **Licence:** Apache-2.0

## Role and feature boundary

OpenDAL is used only behind the `colibri-artifact-client` storage boundary. The
initial adapter uses its filesystem service to stage attempt-scoped candidate
bytes and promote verified, immutable content-addressed objects. MojoColib
continues to own artifact manifests, lineage, content verification, signatures,
fencing and acceptance decisions.

The adapter streams regular files in bounded 1 MiB chunks for hashing, upload,
verification, promotion and recovery. It deliberately has no whole-artifact
byte API, so the V1 20 GiB artifact limit does not imply a corresponding RAM
requirement. Storage keys use a versioned, portable layout with hex-encoded
identifier segments and raw digest hex; they do not embed the wire-format
`sha256:` prefix or unescaped identifier text.

The initial dependency pin is exactly `opendal = "=0.58.0"` with default
features disabled. The implementation may enable only the filesystem service
and the Tokio executor features required by that adapter. S3, HTTP transport,
credentials and presigned operations are not enabled in this slice.

## Distribution and compatibility

OpenDAL source is fetched by Cargo; it is not vendored or bundled as a separate
runtime component. Its declared Apache-2.0 licence is compatible with the
project's source licence. The resolved `Cargo.lock` remains the authoritative
record of the complete transitive graph.

OpenDAL 0.58.0 declares Rust 1.91 as its minimum supported Rust version. The
MojoColib workspace is pinned to Rust 1.97, so the local source build is within
the declared compatibility range. Before a public binary is distributed, the
release owner must generate an SBOM and a version-specific transitive licence
report as required by [the third-party runtime policy](../third-party-runtime-policy.md).

## Operational constraints

- Candidate objects are never accepted objects.
- The worker-facing stager capability and trusted promoter capability are
  separate Rust types. An executor process must receive neither capability nor
  the filesystem root/credentials; it returns a local output file only.
- The public promotion contract is immutable and content-addressed; it does not
  rely on filesystem rename semantics that an S3-compatible implementation may
  not provide.
- This is a local process-capability boundary, not a replacement for a durable
  coordinator authorization record. The later coordinator must hold promotion
  authority, re-check the active lease and atomically persist its acceptance
  decision before exposing an accepted manifest.
- A later S3/MinIO adapter must pass its own conditional-write, copy, replay and
  failure conformance suite before it is used for accepted artifacts.
- Disabling the adapter must fail closed before a stage can report an accepted
  artifact; it must not alter protocol or ledger state.

## Evidence

- [OpenDAL package manifest](https://github.com/apache/opendal/blob/main/core/Cargo.toml)
- [OpenDAL filesystem service](https://github.com/apache/opendal/tree/main/core/services/fs)
- [MojoColib runtime policy](../third-party-runtime-policy.md)
