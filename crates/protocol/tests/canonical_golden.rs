use colibri_protocol::{canonical, digest::Sha256Digest};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    schema_version: u16,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    value: serde_json::Value,
    canonical: String,
    sha256: Sha256Digest,
}

#[test]
fn rust_matches_language_neutral_golden_bytes() {
    let fixture: Fixture = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/protocol/canonical_cases.json"
    )))
    .unwrap();
    assert_eq!(fixture.schema_version, 1);
    for case in fixture.cases {
        let encoded = canonical::to_vec(&case.value).unwrap();
        assert_eq!(encoded, case.canonical.as_bytes(), "case {}", case.name);
        assert_eq!(
            Sha256Digest::of_bytes(&encoded),
            case.sha256,
            "case {}",
            case.name
        );
    }
}
