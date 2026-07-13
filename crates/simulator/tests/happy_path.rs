use colibri_simulator::run_happy_path_json;

#[test]
fn happy_path_is_deterministic_and_replay_safe() {
    let first = run_happy_path_json().unwrap();
    let second = run_happy_path_json().unwrap();

    assert_eq!(first, second);
    assert_eq!(
        first,
        r#"{
  "simulation": "mojocolib-step-1-happy-path-v1",
  "pipeline": {
    "id": "pipeline-step-1",
    "phase": "completed",
    "sequence": 4
  },
  "stage": {
    "id": "stage-training-1",
    "phase": "accepted",
    "sequence": 5,
    "attempts_started": 1,
    "artifact_manifest_digest": "sha256:5a57cd8252401196492412e5dd8234064c460b218b44e9ac0e1bcc0b6a04166f"
  },
  "ledger": {
    "customer_id": "customer-demo",
    "customer_available_microcredits": 200,
    "provider_id": "provider-demo",
    "provider_credit_microcredits": 650,
    "operator_fee_microcredits": 150
  },
  "replay": {
    "exact_replays_ignored": 3,
    "state_unchanged": true
  }
}"#
    );
}
