use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use colibri_artifact_client::{ArtifactPromoter, ArtifactStager, FilesystemArtifactStore};
use colibri_local_worker::{
    Clock, DatasetPrepareParametersV1, LocalDatasetInputV1, LocalDatasetVerifier,
    LocalDatasetVerifierConfig, LocalWorker, LocalWorkerConfig, LocalWorkerError,
    LocalWorkerIdentity,
};
use colibri_protocol::{
    SCHEMA_VERSION_V1,
    artifact::{ArtifactLineageV1, ArtifactManifestV1, RetentionClass},
    digest::Sha256Digest,
    executor::{ExecutorRequestV1, NetworkPolicy},
    ids::{
        ActorId, ArtifactId, AttemptId, KeyId, MessageId, PipelineId, ProviderId, StageId,
        TemplateId, WorkerId,
    },
    lease::{FencingTokenV1, StageLeaseV1},
    pipeline::{ArtifactType, DataClass, ResourceRequirementsV1, RuntimeKind},
    signing::{
        Ed25519SignatureV1, SignedContract, SignedEnvelopeV1, SignerKind, TrustedEd25519KeyV1,
    },
    template::TemplateManifestV1,
    verification::VerificationPolicyV1,
};
use colibri_state_machine::{
    EventEnvelope,
    stage::{StageEvent, StageMachine, StagePhase},
};
use ed25519_dalek::{Signer as _, SigningKey};

const NOW: u64 = 2_000;

#[derive(Clone)]
struct TestClock {
    now: Arc<AtomicU64>,
}

impl TestClock {
    fn new(now_unix_ms: u64) -> Self {
        Self {
            now: Arc::new(AtomicU64::new(now_unix_ms)),
        }
    }

    fn set(&self, now_unix_ms: u64) {
        self.now.store(now_unix_ms, Ordering::SeqCst);
    }
}

impl Clock for TestClock {
    fn now_unix_ms(&self) -> Result<u64, LocalWorkerError> {
        Ok(self.now.load(Ordering::SeqCst))
    }
}

struct Harness {
    directory: tempfile::TempDir,
    stager: colibri_artifact_client::FilesystemArtifactStager,
    promoter: colibri_artifact_client::FilesystemArtifactPromoter,
    worker: LocalWorker,
    verifier: LocalDatasetVerifier,
    operator_key: TrustedEd25519KeyV1,
    signed_request: SignedEnvelopeV1<ExecutorRequestV1>,
    request: ExecutorRequestV1,
    template: TemplateManifestV1,
    parameters: DatasetPrepareParametersV1,
    worker_registry_key: TrustedEd25519KeyV1,
    source: LocalDatasetInputV1,
    clock: TestClock,
}

fn executor_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mojocolib-dataset-prepare-executor"))
}

fn fresh_worker_identity() -> LocalWorkerIdentity {
    LocalWorkerIdentity::new(
        ProviderId::new("provider-local").unwrap(),
        WorkerId::new("worker-local").unwrap(),
        KeyId::new("worker-key").unwrap(),
        SigningKey::from_bytes(&[7; 32]),
    )
}

fn local_worker_for(
    harness: &Harness,
    operator_key: TrustedEd25519KeyV1,
    template: TemplateManifestV1,
    parameters: DatasetPrepareParametersV1,
) -> Result<LocalWorker, LocalWorkerError> {
    LocalWorker::new(LocalWorkerConfig {
        identity: fresh_worker_identity(),
        operator_registry_key: operator_key,
        template,
        parameters,
        environment_digest: harness.request.environment_digest,
        clock: Arc::new(harness.clock.clone()),
        executor_path: executor_path(),
        stager: harness.stager.clone(),
    })
}

fn resources() -> ResourceRequirementsV1 {
    ResourceRequirementsV1 {
        cpu_cores: 1,
        ram_bytes: 512 * 1024 * 1024,
        accelerator_memory_bytes: 0,
        scratch_bytes: 512 * 1024 * 1024,
        wall_time_seconds: 30,
        required_precisions: Vec::new(),
    }
}

fn template(
    parameters: &DatasetPrepareParametersV1,
    executor_path: &PathBuf,
) -> TemplateManifestV1 {
    TemplateManifestV1 {
        schema_version: SCHEMA_VERSION_V1,
        template_id: TemplateId::new("dataset-prepare-v1").unwrap(),
        bundle_digest: Sha256Digest::of_bytes(&std::fs::read(executor_path).unwrap()),
        runtime: RuntimeKind::NativeRust,
        supported_operating_systems: vec!["macos".into(), "linux".into()],
        supported_architectures: vec!["aarch64".into(), "x86_64".into()],
        supported_backends: vec!["cpu".into()],
        parameter_schema_digest: parameters.canonical_digest().unwrap(),
        input_types: vec![ArtifactType::Dataset],
        output_types: vec![ArtifactType::Dataset],
        maximum_resources: ResourceRequirementsV1 {
            cpu_cores: 2,
            ram_bytes: 1024 * 1024 * 1024,
            accelerator_memory_bytes: 0,
            scratch_bytes: 1024 * 1024 * 1024,
            wall_time_seconds: 60,
            required_precisions: Vec::new(),
        },
        network_policy: NetworkPolicy::DenyAll,
        network_allowlist_digest: None,
        verification_policy: VerificationPolicyV1 {
            schema_version: SCHEMA_VERSION_V1,
            policy_digest: Sha256Digest::of_bytes(b"dataset-prepare-policy"),
            require_checkpoint_load: false,
            require_independent_provider: false,
            hidden_canary: false,
            duplicate_execution_basis_points: 0,
        },
        graceful_drain_seconds: 5,
    }
}

fn operator_key(signing_key: &SigningKey) -> TrustedEd25519KeyV1 {
    TrustedEd25519KeyV1 {
        schema_version: SCHEMA_VERSION_V1,
        key_id: KeyId::new("operator-key").unwrap(),
        signer_kind: SignerKind::Operator,
        signer_id: ActorId::new("operator-local").unwrap(),
        public_key_base64: STANDARD_NO_PAD.encode(signing_key.verifying_key().as_bytes()),
        valid_from_unix_ms: 1,
        expires_at_unix_ms: 100_000,
    }
}

fn sign_request(
    request: ExecutorRequestV1,
    signing_key: &SigningKey,
    operator_key: &TrustedEd25519KeyV1,
) -> SignedEnvelopeV1<ExecutorRequestV1> {
    let mut envelope = SignedEnvelopeV1 {
        schema_version: SCHEMA_VERSION_V1,
        contract_type: ExecutorRequestV1::CONTRACT_TYPE.into(),
        message_id: MessageId::new("assignment-1").unwrap(),
        sequence: 1,
        issued_at_unix_ms: 1_500,
        expires_at_unix_ms: Some(60_000),
        payload: request,
        signature: Ed25519SignatureV1 {
            key_id: operator_key.key_id.clone(),
            signer_kind: SignerKind::Operator,
            signer_id: operator_key.signer_id.clone(),
            signature_base64: String::new(),
        },
    };
    envelope.signature.signature_base64 = STANDARD_NO_PAD.encode(
        signing_key
            .sign(&envelope.signing_bytes().unwrap())
            .to_bytes(),
    );
    envelope
}

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("fixtures/tiny_training/train.jsonl")
}

fn fixture_source() -> LocalDatasetInputV1 {
    let source_path = fixture_path();
    let source_bytes = std::fs::read(&source_path).unwrap();
    LocalDatasetInputV1 {
        source_path,
        manifest: ArtifactManifestV1 {
            schema_version: SCHEMA_VERSION_V1,
            artifact_id: ArtifactId::new("input-fixture").unwrap(),
            content_digest: Sha256Digest::of_bytes(&source_bytes),
            byte_length: u64::try_from(source_bytes.len()).unwrap(),
            logical_type: ArtifactType::Dataset,
            lineage: ArtifactLineageV1 {
                pipeline_id: PipelineId::new("input-pipeline").unwrap(),
                stage_id: StageId::new("input-stage").unwrap(),
                attempt_id: AttemptId::new("input-attempt").unwrap(),
                provider_id: ProviderId::new("fixture-provider").unwrap(),
                worker_id: WorkerId::new("fixture-worker").unwrap(),
                fencing_token: FencingTokenV1::new(1).unwrap(),
                input_manifest_digests: Vec::new(),
            },
            environment_digest: Sha256Digest::of_bytes(b"fixture-environment"),
            data_class: DataClass::Public,
            retention: RetentionClass::PilotAccepted,
        },
    }
}

fn harness() -> Harness {
    let parameters = DatasetPrepareParametersV1 {
        schema_version: SCHEMA_VERSION_V1,
        max_records: 32,
    };
    let child_path = executor_path();
    let template = template(&parameters, &child_path);
    let source = fixture_source();
    let source_manifest_digest = source.manifest.canonical_digest().unwrap();
    let lease = StageLeaseV1 {
        schema_version: SCHEMA_VERSION_V1,
        pipeline_id: PipelineId::new("local-pipeline").unwrap(),
        pipeline_spec_digest: Sha256Digest::of_bytes(b"local-pipeline-spec"),
        stage_id: StageId::new("dataset-stage").unwrap(),
        attempt_id: AttemptId::new("attempt-1").unwrap(),
        worker_id: WorkerId::new("worker-local").unwrap(),
        template_digest: template.bundle_digest,
        input_manifest_digests: vec![source_manifest_digest],
        issued_at_unix_ms: 1_000,
        expires_at_unix_ms: 61_000,
        fencing_token: FencingTokenV1::new(1).unwrap(),
    };
    let request = ExecutorRequestV1 {
        schema_version: SCHEMA_VERSION_V1,
        lease,
        parameter_digest: parameters.canonical_digest().unwrap(),
        environment_digest: Sha256Digest::of_bytes(b"local-environment"),
        resources: resources(),
        network_policy: NetworkPolicy::DenyAll,
        resume_checkpoint_manifest: None,
    };
    let directory = tempfile::tempdir().unwrap();
    let store = FilesystemArtifactStore::new(directory.path()).unwrap();
    let (stager, promoter) = store.into_capabilities();
    let worker_identity = fresh_worker_identity();
    let worker_registry_key = worker_identity.trusted_registry_key(1, 100_000).unwrap();
    let operator_signing_key = SigningKey::from_bytes(&[9; 32]);
    let operator_key = operator_key(&operator_signing_key);
    let clock = TestClock::new(NOW);
    let signed_request = sign_request(request.clone(), &operator_signing_key, &operator_key);
    let worker = LocalWorker::new(LocalWorkerConfig {
        identity: worker_identity,
        operator_registry_key: operator_key.clone(),
        template: template.clone(),
        parameters: parameters.clone(),
        environment_digest: request.environment_digest,
        clock: Arc::new(clock.clone()),
        executor_path: executor_path(),
        stager: stager.clone(),
    })
    .unwrap();
    let verifier = LocalDatasetVerifier::new(LocalDatasetVerifierConfig {
        template: template.clone(),
        parameters: parameters.clone(),
        operator_registry_key: operator_key.clone(),
        signed_assignment: signed_request.clone(),
        source: source.clone(),
        worker_registry_key: worker_registry_key.clone(),
        clock: Arc::new(clock.clone()),
        stager: stager.clone(),
        promoter: promoter.clone(),
    })
    .unwrap();
    Harness {
        directory,
        stager,
        promoter,
        worker,
        verifier,
        operator_key,
        signed_request,
        request,
        template,
        parameters,
        worker_registry_key,
        source,
        clock,
    }
}

#[tokio::test]
async fn local_native_executor_stages_signs_verifies_and_promotes_dataset() {
    let harness = harness();
    let submission = harness
        .worker
        .execute_dataset_prepare(&harness.signed_request, &harness.source)
        .await
        .unwrap();
    let candidate = submission.candidate.clone();
    let manifest_digest = submission
        .signed_manifest
        .payload
        .canonical_digest()
        .unwrap();
    let accepted = harness
        .verifier
        .verify_and_promote(submission)
        .await
        .unwrap();

    assert_eq!(
        accepted.completion.output_manifest_digests,
        vec![manifest_digest]
    );
    assert_eq!(
        accepted.accepted_artifact.content_digest(),
        accepted.signed_manifest.payload.content_digest
    );
    assert!(harness.stager.verify_candidate(&candidate).await.is_err());
    let destination = harness.directory.path().join("accepted.jsonl");
    harness
        .promoter
        .copy_accepted_to_file(accepted.accepted_artifact.content_digest(), &destination)
        .await
        .unwrap();
    assert_eq!(
        tokio::fs::read(destination).await.unwrap(),
        tokio::fs::read(fixture_path()).await.unwrap()
    );

    let lease = harness.request.lease.clone();
    let mut stage = StageMachine::new(lease.stage_id.clone(), 1).unwrap();
    stage
        .apply_event(&EventEnvelope::new(
            MessageId::new("stage-dependencies").unwrap(),
            StageEvent::DependenciesAccepted,
        ))
        .unwrap();
    stage
        .apply_event(&EventEnvelope::new(
            MessageId::new("stage-lease").unwrap(),
            StageEvent::LeaseIssued {
                lease: lease.clone(),
            },
        ))
        .unwrap();
    stage
        .apply_event(&EventEnvelope::new(
            MessageId::new("stage-start").unwrap(),
            StageEvent::LeaseAccepted {
                attempt_id: lease.attempt_id.clone(),
                fencing_token: lease.fencing_token,
                observed_at_unix_ms: NOW,
            },
        ))
        .unwrap();
    stage
        .apply_event(&EventEnvelope::new(
            MessageId::new("stage-output").unwrap(),
            StageEvent::ExecutionSucceeded {
                attempt_id: lease.attempt_id.clone(),
                fencing_token: lease.fencing_token,
                observed_at_unix_ms: NOW,
                manifest_digest,
            },
        ))
        .unwrap();
    stage
        .apply_event(&EventEnvelope::new(
            MessageId::new("stage-verified").unwrap(),
            StageEvent::VerificationAccepted {
                attempt_id: lease.attempt_id,
                manifest_digest,
            },
        ))
        .unwrap();
    assert_eq!(stage.phase(), StagePhase::Accepted);
}

#[tokio::test]
async fn expired_verification_discards_candidate_without_promoting_it() {
    let harness = harness();
    let submission = harness
        .worker
        .execute_dataset_prepare(&harness.signed_request, &harness.source)
        .await
        .unwrap();
    let candidate = submission.candidate.clone();
    harness.clock.set(61_000);
    assert!(
        harness
            .verifier
            .verify_and_promote(submission)
            .await
            .is_err()
    );
    assert!(harness.stager.verify_candidate(&candidate).await.is_err());
    assert!(
        harness
            .promoter
            .verify_accepted(candidate.content_digest())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn wrong_template_digest_is_rejected_before_a_candidate_exists() {
    let harness = harness();
    let mut wrong_request = harness.request.clone();
    wrong_request.lease.template_digest = Sha256Digest::of_bytes(b"unapproved-template");
    let signing_key = SigningKey::from_bytes(&[9; 32]);
    let signed = sign_request(wrong_request, &signing_key, &harness.operator_key);
    assert!(
        harness
            .worker
            .execute_dataset_prepare(&signed, &harness.source)
            .await
            .is_err()
    );
}

#[test]
fn worker_admission_rejects_wrong_parameter_or_operator_contracts() {
    let harness = harness();
    let mut missing_dataset_input = harness.template.clone();
    missing_dataset_input.input_types.clear();
    assert!(
        local_worker_for(
            &harness,
            harness.operator_key.clone(),
            missing_dataset_input,
            harness.parameters.clone(),
        )
        .is_err()
    );

    let mut mismatched_parameters = harness.template.clone();
    mismatched_parameters.parameter_schema_digest = Sha256Digest::of_bytes(b"wrong-parameters");
    assert!(
        local_worker_for(
            &harness,
            harness.operator_key.clone(),
            mismatched_parameters,
            harness.parameters.clone(),
        )
        .is_err()
    );

    let mut unattested_bundle = harness.template.clone();
    unattested_bundle.bundle_digest = Sha256Digest::of_bytes(b"wrong-bundle");
    assert!(
        local_worker_for(
            &harness,
            harness.operator_key.clone(),
            unattested_bundle,
            harness.parameters.clone(),
        )
        .is_err()
    );

    let mut wrong_role = harness.operator_key.clone();
    wrong_role.signer_kind = SignerKind::Worker;
    assert!(
        local_worker_for(
            &harness,
            wrong_role,
            harness.template.clone(),
            harness.parameters.clone(),
        )
        .is_err()
    );
}

#[tokio::test]
async fn duplicate_signed_assignment_is_claimed_only_once() {
    let harness = harness();
    let first = harness
        .worker
        .execute_dataset_prepare(&harness.signed_request, &harness.source)
        .await
        .unwrap();
    assert!(
        harness
            .worker
            .execute_dataset_prepare(&harness.signed_request, &harness.source)
            .await
            .is_err()
    );
    harness
        .stager
        .discard_candidate(&first.candidate)
        .await
        .unwrap();
}

#[tokio::test]
async fn extra_input_lineage_is_rejected_before_child_execution() {
    let harness = harness();
    let mut request = harness.request.clone();
    request
        .lease
        .input_manifest_digests
        .push(Sha256Digest::of_bytes(b"unused-input"));
    let signed = sign_request(
        request,
        &SigningKey::from_bytes(&[9; 32]),
        &harness.operator_key,
    );
    assert!(
        harness
            .worker
            .execute_dataset_prepare(&signed, &harness.source)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn superseded_assignment_cannot_promote_an_old_candidate() {
    let harness = harness();
    let submission = harness
        .worker
        .execute_dataset_prepare(&harness.signed_request, &harness.source)
        .await
        .unwrap();
    let candidate = submission.candidate.clone();
    let mut replacement_request = harness.request.clone();
    replacement_request.lease.attempt_id = AttemptId::new("attempt-2").unwrap();
    replacement_request.lease.fencing_token = FencingTokenV1::new(2).unwrap();
    let replacement_assignment = sign_request(
        replacement_request,
        &SigningKey::from_bytes(&[9; 32]),
        &harness.operator_key,
    );
    let replacement_verifier = LocalDatasetVerifier::new(LocalDatasetVerifierConfig {
        template: harness.template.clone(),
        parameters: harness.parameters.clone(),
        operator_registry_key: harness.operator_key.clone(),
        signed_assignment: replacement_assignment,
        source: harness.source.clone(),
        worker_registry_key: harness.worker_registry_key.clone(),
        clock: Arc::new(harness.clock.clone()),
        stager: harness.stager.clone(),
        promoter: harness.promoter.clone(),
    })
    .unwrap();
    assert!(
        replacement_verifier
            .verify_and_promote(submission)
            .await
            .is_err()
    );
    assert!(harness.stager.verify_candidate(&candidate).await.is_err());
}

#[cfg(unix)]
#[tokio::test]
async fn oversized_child_stdout_is_bounded_and_terminated() {
    use std::os::unix::fs::PermissionsExt;

    let harness = harness();
    let script_path = harness.directory.path().join("stdout-spammer.sh");
    std::fs::write(&script_path, "#!/bin/sh\nwhile :; do printf x; done\n").unwrap();
    let mut permissions = std::fs::metadata(&script_path).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&script_path, permissions).unwrap();

    let mut template = harness.template.clone();
    template.bundle_digest = Sha256Digest::of_bytes(&std::fs::read(&script_path).unwrap());
    let mut request = harness.request.clone();
    request.lease.template_digest = template.bundle_digest;
    let signed = sign_request(
        request,
        &SigningKey::from_bytes(&[9; 32]),
        &harness.operator_key,
    );
    let worker = LocalWorker::new(LocalWorkerConfig {
        identity: fresh_worker_identity(),
        operator_registry_key: harness.operator_key.clone(),
        template,
        parameters: harness.parameters.clone(),
        environment_digest: harness.request.environment_digest,
        clock: Arc::new(harness.clock.clone()),
        executor_path: script_path,
        stager: harness.stager.clone(),
    })
    .unwrap();
    assert!(matches!(
        worker
            .execute_dataset_prepare(&signed, &harness.source)
            .await,
        Err(LocalWorkerError::ExecutorOutputTooLarge)
    ));
}
