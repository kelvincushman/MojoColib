# ADR-0004: Rootless Linux executors and trusted-machine macOS executors

- Status: Accepted for the trusted-provider MVP
- Date: 2026-07-12
- Owners: Worker and security owners

## Decision

The Rust worker owns identity keys, leases, artifact promotion and metering. Training code runs in a disposable child executor and never receives the worker key, provider settlement key or coordinator credentials.

Linux GPU jobs use a rootless OCI container with a read-only image pinned by digest, a non-root UID, dropped capabilities, `no-new-privileges`, bounded CPU/RAM/PIDs/storage/wall time, explicit accelerator visibility and a writable attempt directory only. Network egress is denied by default; an approved template may receive a narrow destination allowlist. The worker kills the entire process group on expiry and deletes temporary credentials and mounts.

macOS cannot provide an equivalent general rootless GPU-container boundary. Apple jobs are therefore limited to project-owned, dedicated machines or dedicated OS accounts with no personal files. They run a signed, locked environment in a bounded subprocess with a fresh attempt directory, empty user caches, explicit environment allowlist and outbound-network denial. The executor cannot read Keychain items or worker identity material. General third-party Macs are not admitted until filesystem, process, Keychain and network enforcement tests pass. Product reports must disclose that this is weaker isolation than the Linux path.

Mojo, Python, Rust-native and Colibri C executors all use the same supervisor contract. A runtime's presence never grants it broader filesystem, network or secret access.

## Failure behavior

- An invalid signature, digest, compatibility key or lease is rejected before executor launch.
- Lease expiry or cancellation requests a checkpoint/drain, then force-terminates after 30 seconds.
- Outputs remain attempt-temporary until the coordinator accepts the signed manifest and current fencing token.
- An executor crash affects only its attempt; the worker process and identity session remain alive.
- Any isolation-enforcement failure quarantines the worker rather than silently falling back.

## Pilot gate

Admission requires automated tests for secret denial, path traversal, symlink escape, process cleanup, resource caps, network denial and stale-lease artifact rejection. Container escape or worker-key exposure is a pilot stop condition.
