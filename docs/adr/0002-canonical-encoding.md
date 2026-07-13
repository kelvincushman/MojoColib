# ADR-0002: Canonical JSON commitments

- Status: Accepted
- Date: 2026-07-12
- Owners: Protocol and security owners

## Decision

All V1 protocol hashes and Ed25519 signatures cover UTF-8 JSON encoded with RFC 8785 JSON Canonicalization Scheme (JCS), followed by SHA-256 where a digest is required. The Rust implementation is `colibri-protocol::canonical`; every non-Rust implementation must pass the same golden byte fixtures before it can sign or verify production records.

Digests use exactly `sha256:` plus 64 lowercase hexadecimal characters. Schema objects reject unknown versions, invalid identifiers and values outside the declared bounds before hashing. Maps with application-defined keys use deterministic key ordering. Floating-point values are prohibited in signed protocol objects: metrics use separately defined integer/fixed-point fields or content-addressed metric artifacts.

Although monetary values are signed 64-bit integers internally, JSON carries microcredits as canonical decimal strings. This prevents JavaScript and database adapters from silently rounding values beyond IEEE-754's exact integer range. Parsers must reject fractional, exponent, whitespace-padded and out-of-range forms.

Signatures cover a domain-separated commitment containing the envelope schema version, key ID, worker ID, public key and canonical payload. The signature bytes alone remain outside that commitment, avoiding self-reference while preventing identity metadata from being swapped. Key-registry validation must additionally prove that the claimed public key is active for the worker. Any envelope or payload schema change creates a new version instead of changing V1 encoding rules.

## Trust-boundary parsing rule

Every untrusted signed V1 JSON message entering Rust MUST be parsed with `colibri_protocol::strict_json::parse_signed_envelope`. Code at HTTP, queue, file-import, worker/coordinator and executor control boundaries MUST NOT deserialize signed input directly with permissive `serde_json::from_slice`, `from_str` or an intermediate generic `Value`.

The strict entry point rejects duplicate object keys at any depth, all floating-point JSON numbers, trailing non-whitespace data and unknown fields on V1 envelope/contract structs. Validated identifier types, schema versions, contract-type tags and each contract's semantic invariants are then checked before a state transition. Signature verification operates on the canonical typed commitment only after this parsing and validation path; it must never authenticate bytes that a different parser could interpret differently.

Unsigned future API payloads that can fund work, mutate state, admit a provider or select an artifact MUST provide equivalent duplicate-key, unknown-field, numeric and trailing-data rejection before reaching a state machine. Adding such an endpoint does not weaken this rule.

## Why JCS

JSON is inspectable, has mature Rust/Python/TypeScript support and maps directly to the planned API and schema artifacts. JCS removes object-order and number-format ambiguity without introducing a second binary codec during the MVP.

Deterministic CBOR would produce smaller payloads but adds diagnostic friction and cross-language profile choices. Deterministic protobuf was rejected because unknown-field behavior and generated-code versioning make "the bytes being signed" less obvious during an early, security-sensitive protocol.

## Verification gates

- Rust/Python/TypeScript fixtures must produce identical bytes and SHA-256 values.
- Changed key order must not change a commitment; changed values must.
- Invalid Unicode, non-finite numbers, duplicate keys and out-of-range integers must be rejected.
- Signature tests must reject payload, identity and signature tampering.

Relevant implementation: [`crates/protocol/src/canonical.rs`](../../crates/protocol/src/canonical.rs), [`strict_json.rs`](../../crates/protocol/src/strict_json.rs), [`digest.rs`](../../crates/protocol/src/digest.rs) and [`signing.rs`](../../crates/protocol/src/signing.rs).
