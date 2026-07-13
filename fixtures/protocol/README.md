# Protocol golden fixtures

`canonical_cases.json` is the first language-neutral commitment corpus for ADR-0002. Rust and Python must reproduce every canonical byte string and SHA-256 value exactly. Cases deliberately avoid floating point because signed protocol objects prohibit it.

Any change to an existing expected byte string is a V1 protocol break. Add new cases freely; change encoding rules only under a new schema/commitment version.
