use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use colibri_protocol::{
    SCHEMA_VERSION_V1,
    artifact::{ArtifactLineageV1, ArtifactManifestV1, RetentionClass},
    digest::Sha256Digest,
    ids::{
        ActorId, ArtifactId, AttemptId, KeyId, MessageId, PipelineId, ProviderId, StageId, WorkerId,
    },
    lease::{FencingTokenV1, StageLeaseV1},
    pipeline::{ArtifactType, DataClass},
    signing::{
        Ed25519SignatureV1, SignedContract, SignedEnvelopeV1, SignerKind, TrustedEd25519KeyV1,
    },
    strict_json::parse_signed_envelope,
    verification::{VerificationDecisionV1, VerificationOutcome, VerificationPolicyV1},
};
use ed25519_dalek::{Signer as _, SigningKey};

fn stage_lease() -> StageLeaseV1 {
    StageLeaseV1 {
        schema_version: SCHEMA_VERSION_V1,
        pipeline_id: PipelineId::new("pipeline-1").unwrap(),
        pipeline_spec_digest: Sha256Digest::of_bytes(b"pipeline-spec"),
        stage_id: StageId::new("stage-1").unwrap(),
        attempt_id: AttemptId::new("attempt-1").unwrap(),
        worker_id: WorkerId::new("worker-1").unwrap(),
        template_digest: Sha256Digest::of_bytes(b"template"),
        input_manifest_digests: vec![Sha256Digest::of_bytes(b"input-manifest")],
        issued_at_unix_ms: 1_000,
        expires_at_unix_ms: 61_000,
        fencing_token: FencingTokenV1::new(7).unwrap(),
    }
}

fn artifact_manifest(lease: &StageLeaseV1) -> ArtifactManifestV1 {
    ArtifactManifestV1 {
        schema_version: SCHEMA_VERSION_V1,
        artifact_id: ArtifactId::new("artifact-1").unwrap(),
        content_digest: Sha256Digest::of_bytes(b"artifact-content"),
        byte_length: 128,
        logical_type: ArtifactType::Model,
        lineage: ArtifactLineageV1 {
            pipeline_id: lease.pipeline_id.clone(),
            stage_id: lease.stage_id.clone(),
            attempt_id: lease.attempt_id.clone(),
            provider_id: ProviderId::new("provider-1").unwrap(),
            worker_id: lease.worker_id.clone(),
            fencing_token: lease.fencing_token,
            input_manifest_digests: lease.input_manifest_digests.clone(),
        },
        environment_digest: Sha256Digest::of_bytes(b"environment"),
        data_class: DataClass::Confidential,
        retention: RetentionClass::PilotAccepted,
    }
}

fn signed_artifact(
    payload: ArtifactManifestV1,
) -> (SignedEnvelopeV1<ArtifactManifestV1>, TrustedEd25519KeyV1) {
    let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
    let trusted_key = TrustedEd25519KeyV1 {
        schema_version: SCHEMA_VERSION_V1,
        key_id: KeyId::new("key-1").unwrap(),
        signer_kind: SignerKind::Worker,
        signer_id: ActorId::new("worker-1").unwrap(),
        public_key_base64: STANDARD_NO_PAD.encode(signing_key.verifying_key().as_bytes()),
        valid_from_unix_ms: 1_000,
        expires_at_unix_ms: 10_000,
    };
    let mut envelope = SignedEnvelopeV1 {
        schema_version: SCHEMA_VERSION_V1,
        contract_type: ArtifactManifestV1::CONTRACT_TYPE.into(),
        message_id: MessageId::new("message-1").unwrap(),
        sequence: 1,
        issued_at_unix_ms: 2_000,
        expires_at_unix_ms: Some(9_000),
        payload,
        signature: Ed25519SignatureV1 {
            key_id: trusted_key.key_id.clone(),
            signer_kind: trusted_key.signer_kind,
            signer_id: trusted_key.signer_id.clone(),
            signature_base64: String::new(),
        },
    };
    let signature = signing_key.sign(&envelope.signing_bytes().unwrap());
    envelope.signature.signature_base64 = STANDARD_NO_PAD.encode(signature.to_bytes());
    (envelope, trusted_key)
}

#[test]
fn artifact_is_bound_to_its_exact_lease_and_registry_worker() {
    let lease = stage_lease();
    let artifact = artifact_manifest(&lease);
    let (envelope, trusted_key) = signed_artifact(artifact);

    assert_eq!(
        envelope.verify_with_registry_key(&trusted_key, 3_000, 1),
        Ok(())
    );
    assert_eq!(envelope.payload.validate_against_lease(&lease), Ok(()));
    assert_eq!(
        envelope.payload.validate_registry_signer(&trusted_key),
        Ok(())
    );

    let mut stale_lease = lease.clone();
    stale_lease.fencing_token = FencingTokenV1::new(8).unwrap();
    assert!(
        envelope
            .payload
            .validate_against_lease(&stale_lease)
            .is_err()
    );

    let mut wrong_inputs = lease;
    wrong_inputs.input_manifest_digests = vec![Sha256Digest::of_bytes(b"other-input")];
    assert!(
        envelope
            .payload
            .validate_against_lease(&wrong_inputs)
            .is_err()
    );

    let mut wrong_worker = trusted_key.clone();
    wrong_worker.signer_id = ActorId::new("worker-2").unwrap();
    assert!(
        envelope
            .payload
            .validate_registry_signer(&wrong_worker)
            .is_err()
    );

    let mut wrong_kind = trusted_key;
    wrong_kind.signer_kind = SignerKind::Provider;
    assert!(
        envelope
            .payload
            .validate_registry_signer(&wrong_kind)
            .is_err()
    );
}

#[test]
fn independent_verifier_policy_rejects_the_producing_provider() {
    let artifact = artifact_manifest(&stage_lease());
    let policy = VerificationPolicyV1 {
        schema_version: SCHEMA_VERSION_V1,
        policy_digest: Sha256Digest::of_bytes(b"verification-policy"),
        require_checkpoint_load: false,
        require_independent_provider: true,
        hidden_canary: false,
        duplicate_execution_basis_points: 0,
    };
    let mut decision = VerificationDecisionV1 {
        schema_version: SCHEMA_VERSION_V1,
        stage_id: artifact.lineage.stage_id.clone(),
        attempt_id: artifact.lineage.attempt_id.clone(),
        artifact_id: artifact.artifact_id.clone(),
        policy_digest: policy.policy_digest,
        verifier_provider_id: ProviderId::new("provider-2").unwrap(),
        producer_provider_id: artifact.lineage.provider_id.clone(),
        outcome: VerificationOutcome::Accepted,
        reason_codes: vec!["checks_passed".into()],
        evidence_manifest_digest: Sha256Digest::of_bytes(b"verification-evidence"),
    };

    assert_eq!(decision.validate_against(&policy, &artifact), Ok(()));

    decision.verifier_provider_id = artifact.lineage.provider_id.clone();
    assert!(decision.validate_against(&policy, &artifact).is_err());
}

#[test]
fn strict_json_accepts_valid_envelopes_and_rejects_ambiguous_or_extended_json() {
    let (envelope, trusted_key) = signed_artifact(artifact_manifest(&stage_lease()));
    let valid_json = serde_json::to_vec(&envelope).unwrap();
    let parsed = parse_signed_envelope::<ArtifactManifestV1>(&valid_json).unwrap();
    assert_eq!(parsed, envelope);
    assert_eq!(
        parsed.verify_with_registry_key(&trusted_key, 3_000, 1),
        Ok(())
    );

    let valid_text = String::from_utf8(valid_json.clone()).unwrap();
    let duplicate_top_level = format!(r#"{{"schema_version":1,{}"#, &valid_text[1..]);
    assert!(parse_signed_envelope::<ArtifactManifestV1>(duplicate_top_level.as_bytes()).is_err());

    let float_byte_length = valid_text.replace(r#""byte_length":128"#, r#""byte_length":128.0"#);
    assert_ne!(float_byte_length, valid_text);
    assert!(parse_signed_envelope::<ArtifactManifestV1>(float_byte_length.as_bytes()).is_err());

    let mut unknown_envelope_field: serde_json::Value =
        serde_json::from_slice(&valid_json).unwrap();
    unknown_envelope_field
        .as_object_mut()
        .unwrap()
        .insert("unexpected".into(), true.into());
    assert!(
        parse_signed_envelope::<ArtifactManifestV1>(
            &serde_json::to_vec(&unknown_envelope_field).unwrap()
        )
        .is_err()
    );

    let mut unknown_lineage_field: serde_json::Value = serde_json::from_slice(&valid_json).unwrap();
    unknown_lineage_field["payload"]["lineage"]["unexpected"] = true.into();
    assert!(
        parse_signed_envelope::<ArtifactManifestV1>(
            &serde_json::to_vec(&unknown_lineage_field).unwrap()
        )
        .is_err()
    );
}
