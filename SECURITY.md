# Security policy

MojoColib is a research preview. It has not been audited and must not be used to
run untrusted workloads, protect confidential data or settle real-value
payments.

## Supported versions

Only the current `main` branch receives security fixes during the research
preview. There are no supported production releases yet.

## Reporting a vulnerability

Please use GitHub's private
[security advisory form](https://github.com/kelvincushman/MojoColib/security/advisories/new).
Do not include exploit details, credentials, private model data or personal data
in a public issue.

Include enough information to reproduce the problem safely:

- affected commit and component;
- expected and observed behavior;
- a minimal proof of concept or malformed input;
- the trust boundary crossed; and
- any suggested mitigation.

Relevant areas include signature or canonicalisation bypasses, lease fencing,
artifact and checkpoint integrity, path traversal, executor escape, credential
exposure, ledger double settlement and unsafe model/data handling.

We will acknowledge a useful report as soon as practical and coordinate a fix
and disclosure timeline based on severity. This project does not currently
operate a bug-bounty programme or promise a fixed response window.

Read the full [threat model](docs/threat-model.md) for the intended boundary and
known exclusions.
