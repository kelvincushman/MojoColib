//! A deliberately narrow local worker/executor vertical slice.
//!
//! It proves the trusted boundary before `PyTorch` or a GPU is involved: a
//! repository-owned child process normalises bounded public JSONL input, while
//! the Rust worker owns the signed assignment, manifest, artifact staging and
//! verifier-side promotion.

#![forbid(unsafe_code)]
#![allow(clippy::missing_errors_doc)]

use std::{
    collections::BTreeSet,
    io::Read as _,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use colibri_artifact_client::{
    AcceptedArtifactV1, ArtifactPromoter, ArtifactStager, ArtifactStoreError, CandidateArtifactV1,
    FilesystemArtifactPromoter, FilesystemArtifactStager,
};
use colibri_protocol::{
    ProtocolError, SCHEMA_VERSION_V1,
    artifact::{ArtifactLineageV1, ArtifactManifestV1, RetentionClass},
    canonical,
    digest::Sha256Digest,
    executor::{CompletionStatus, ExecutorCompletionV1, ExecutorRequestV1, NetworkPolicy},
    ids::{ActorId, ArtifactId, KeyId, MessageId, ProviderId, WorkerId},
    pipeline::{ArtifactType, DataClass, RuntimeKind},
    signing::{
        Ed25519SignatureV1, SignedContract, SignedEnvelopeV1, SignerKind, TrustedEd25519KeyV1,
    },
    template::TemplateManifestV1,
};
use ed25519_dalek::{Signer as _, SigningKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
    sync::Mutex,
    time::timeout,
};

/// The local fixture is intentionally bounded; real datasets use direct
/// object-store transfers in a later slice.
pub const MAX_LOCAL_DATASET_BYTES: usize = 1024 * 1024;
/// Largest JSON request the local supervisor will pass to its child.
pub const MAX_LOCAL_EXECUTOR_REQUEST_BYTES: usize = MAX_LOCAL_DATASET_BYTES * 2;
const MAX_CHILD_OUTPUT_BYTES: usize = 1024 * 1024;
const MAX_LOCAL_EXECUTOR_BYTES: u64 = 64 * 1024 * 1024;
const DATASET_PREPARE_TEMPLATE_ID: &str = "dataset-prepare-v1";
const EXECUTOR_TIMEOUT: Duration = Duration::from_secs(10);

/// Trusted source of wall-clock time for lease and signature validation.
pub trait Clock: Send + Sync {
    /// Returns the current Unix timestamp in milliseconds.
    fn now_unix_ms(&self) -> Result<u64, LocalWorkerError>;
}

/// The production clock used by a local worker outside deterministic tests.
#[derive(Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_unix_ms(&self) -> Result<u64, LocalWorkerError> {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| LocalWorkerError::ClockBeforeUnixEpoch)?;
        u64::try_from(elapsed.as_millis()).map_err(|_| LocalWorkerError::ClockOverflow)
    }
}

/// Fixed parameters committed by the local `dataset-prepare-v1` template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatasetPrepareParametersV1 {
    pub schema_version: u16,
    pub max_records: u16,
}

impl DatasetPrepareParametersV1 {
    /// Validates bounded, deterministic local-template parameters.
    pub fn validate(&self) -> Result<(), LocalWorkerError> {
        if self.schema_version != SCHEMA_VERSION_V1 {
            return Err(LocalWorkerError::Protocol(
                ProtocolError::UnsupportedSchemaVersion {
                    actual: self.schema_version,
                    expected: SCHEMA_VERSION_V1,
                },
            ));
        }
        if self.max_records == 0 || usize::from(self.max_records) > 100 {
            return Err(LocalWorkerError::InvalidParameters);
        }
        Ok(())
    }

    /// Returns the canonical commitment required in an executor request.
    pub fn canonical_digest(&self) -> Result<Sha256Digest, LocalWorkerError> {
        self.validate()?;
        Ok(canonical::digest(self)?)
    }
}

/// Trusted local file and its already-admitted input manifest.
#[derive(Debug, Clone)]
pub struct LocalDatasetInputV1 {
    pub source_path: PathBuf,
    pub manifest: ArtifactManifestV1,
}

/// The worker's signing identity. It stays in the supervisor, never in the
/// executor child process.
#[derive(Clone)]
pub struct LocalWorkerIdentity {
    provider_id: ProviderId,
    worker_id: WorkerId,
    key_id: KeyId,
    signing_key: SigningKey,
}

impl LocalWorkerIdentity {
    /// Creates the local fixture identity.
    #[must_use]
    pub fn new(
        provider_id: ProviderId,
        worker_id: WorkerId,
        key_id: KeyId,
        signing_key: SigningKey,
    ) -> Self {
        Self {
            provider_id,
            worker_id,
            key_id,
            signing_key,
        }
    }

    /// Returns the worker identity used in lease and manifest validation.
    #[must_use]
    pub const fn worker_id(&self) -> &WorkerId {
        &self.worker_id
    }

    /// Returns the provider identity attached to produced artifacts.
    #[must_use]
    pub const fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }

    /// Builds the registry record a verifier needs for this local worker.
    pub fn trusted_registry_key(
        &self,
        valid_from_unix_ms: u64,
        expires_at_unix_ms: u64,
    ) -> Result<TrustedEd25519KeyV1, LocalWorkerError> {
        Ok(TrustedEd25519KeyV1 {
            schema_version: SCHEMA_VERSION_V1,
            key_id: self.key_id.clone(),
            signer_kind: SignerKind::Worker,
            signer_id: ActorId::new(self.worker_id.as_str())?,
            public_key_base64: STANDARD_NO_PAD.encode(self.signing_key.verifying_key().as_bytes()),
            valid_from_unix_ms,
            expires_at_unix_ms,
        })
    }

    fn sign_artifact_manifest(
        &self,
        manifest: ArtifactManifestV1,
        sequence: u64,
        now_unix_ms: u64,
    ) -> Result<SignedEnvelopeV1<ArtifactManifestV1>, LocalWorkerError> {
        if sequence == 0 {
            return Err(LocalWorkerError::InvalidWorkerSequence);
        }
        // The manifest commitment includes full lineage and content; using it
        // avoids collisions between independent stages with the same fence and
        // deterministic output bytes.
        let message_id = MessageId::new(format!(
            "manifest-{}",
            hex::encode(manifest.canonical_digest()?.as_bytes()),
        ))?;
        let mut envelope = SignedEnvelopeV1 {
            schema_version: SCHEMA_VERSION_V1,
            contract_type: ArtifactManifestV1::CONTRACT_TYPE.into(),
            message_id,
            sequence,
            issued_at_unix_ms: now_unix_ms,
            expires_at_unix_ms: None,
            payload: manifest,
            signature: Ed25519SignatureV1 {
                key_id: self.key_id.clone(),
                signer_kind: SignerKind::Worker,
                signer_id: ActorId::new(self.worker_id.as_str())?,
                signature_base64: String::new(),
            },
        };
        let signature = self.signing_key.sign(&envelope.signing_bytes()?);
        envelope.signature.signature_base64 = STANDARD_NO_PAD.encode(signature.to_bytes());
        Ok(envelope)
    }
}

/// Candidate bytes plus the worker's signed semantic result.
#[derive(Debug, Clone)]
pub struct DatasetSubmissionV1 {
    pub candidate: CandidateArtifactV1,
    pub signed_manifest: SignedEnvelopeV1<ArtifactManifestV1>,
    pub completion: ExecutorCompletionV1,
}

/// A local verifier result. It is not a durable coordinator acceptance record.
#[derive(Debug, Clone)]
pub struct AcceptedDatasetRunV1 {
    pub accepted_artifact: AcceptedArtifactV1,
    pub signed_manifest: SignedEnvelopeV1<ArtifactManifestV1>,
    pub completion: ExecutorCompletionV1,
}

/// Supervisor for the repository-owned native dataset executor.
#[derive(Clone)]
pub struct LocalWorker {
    identity: LocalWorkerIdentity,
    operator_registry_key: TrustedEd25519KeyV1,
    template: TemplateManifestV1,
    parameters: DatasetPrepareParametersV1,
    environment_digest: Sha256Digest,
    clock: Arc<dyn Clock>,
    executor_path: PathBuf,
    stager: FilesystemArtifactStager,
    replay: Arc<Mutex<WorkerReplayState>>,
}

/// Trusted configuration retained by one local worker supervisor.
pub struct LocalWorkerConfig {
    pub identity: LocalWorkerIdentity,
    pub operator_registry_key: TrustedEd25519KeyV1,
    pub template: TemplateManifestV1,
    pub parameters: DatasetPrepareParametersV1,
    pub environment_digest: Sha256Digest,
    pub clock: Arc<dyn Clock>,
    pub executor_path: PathBuf,
    pub stager: FilesystemArtifactStager,
}

/// Trusted verifier/coordinator stand-in for the local vertical slice.
#[derive(Clone)]
pub struct LocalDatasetVerifier {
    template: TemplateManifestV1,
    parameters: DatasetPrepareParametersV1,
    operator_registry_key: TrustedEd25519KeyV1,
    signed_assignment: SignedEnvelopeV1<ExecutorRequestV1>,
    source: LocalDatasetInputV1,
    worker_registry_key: TrustedEd25519KeyV1,
    clock: Arc<dyn Clock>,
    stager: FilesystemArtifactStager,
    promoter: FilesystemArtifactPromoter,
    replay: Arc<Mutex<ReplayGuard>>,
}

/// Trusted configuration retained by the local verifier/coordinator stand-in.
pub struct LocalDatasetVerifierConfig {
    pub template: TemplateManifestV1,
    pub parameters: DatasetPrepareParametersV1,
    pub operator_registry_key: TrustedEd25519KeyV1,
    pub signed_assignment: SignedEnvelopeV1<ExecutorRequestV1>,
    pub source: LocalDatasetInputV1,
    pub worker_registry_key: TrustedEd25519KeyV1,
    pub clock: Arc<dyn Clock>,
    pub stager: FilesystemArtifactStager,
    pub promoter: FilesystemArtifactPromoter,
}

/// Development-only in-memory replay protection. The service implementation
/// replaces this with a durable, transactional replay store.
#[derive(Debug)]
struct ReplayGuard {
    seen_message_ids: BTreeSet<MessageId>,
    next_sequence: u64,
}

impl Default for ReplayGuard {
    fn default() -> Self {
        Self {
            seen_message_ids: BTreeSet::new(),
            next_sequence: 1,
        }
    }
}

impl ReplayGuard {
    fn claim(&mut self, message_id: &MessageId, sequence: u64) -> Result<(), LocalWorkerError> {
        if sequence < self.next_sequence || !self.seen_message_ids.insert(message_id.clone()) {
            return Err(LocalWorkerError::ReplayDetected);
        }
        self.next_sequence = sequence
            .checked_add(1)
            .ok_or(LocalWorkerError::ReplayDetected)?;
        Ok(())
    }
}

#[derive(Debug, Default)]
struct WorkerReplayState {
    inbound_assignments: ReplayGuard,
    next_manifest_sequence: u64,
}

impl WorkerReplayState {
    fn claim_assignment_and_allocate_manifest_sequence(
        &mut self,
        message_id: &MessageId,
        sequence: u64,
    ) -> Result<u64, LocalWorkerError> {
        self.inbound_assignments.claim(message_id, sequence)?;
        let manifest_sequence = self.next_manifest_sequence.max(1);
        self.next_manifest_sequence = manifest_sequence
            .checked_add(1)
            .ok_or(LocalWorkerError::ReplayDetected)?;
        Ok(manifest_sequence)
    }
}

/// Errors that prevent the local worker path from staging or accepting output.
#[derive(Debug, Error)]
pub enum LocalWorkerError {
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    #[error(transparent)]
    Artifact(#[from] ArtifactStoreError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Base64(#[from] base64::DecodeError),
    #[error("dataset-prepare parameters are invalid")]
    InvalidParameters,
    #[error("the local worker only accepts the approved native dataset-prepare-v1 template")]
    UnsupportedTemplate,
    #[error("executor request does not match the approved local template")]
    RequestTemplateMismatch,
    #[error("executor request exceeds the template's declared resource bound")]
    ResourceLimitExceeded,
    #[error("executor request has an unsupported network, environment or resume policy")]
    RequestPolicyMismatch,
    #[error("executor request is assigned to a different worker")]
    WrongWorker,
    #[error("local dataset input is not a regular file")]
    InputNotRegularFile,
    #[error("local dataset input exceeds the {MAX_LOCAL_DATASET_BYTES}-byte fixture limit")]
    InputTooLarge,
    #[error("local dataset input does not match its manifest or lease lineage")]
    InputMismatch,
    #[error("child executor did not finish before its deadline")]
    ExecutorTimedOut,
    #[error("child executor exited unsuccessfully")]
    ExecutorFailed,
    #[error("executor bundle is not a regular file: {path}")]
    ExecutorNotRegularFile { path: PathBuf },
    #[error("executor bundle exceeds the {MAX_LOCAL_EXECUTOR_BYTES}-byte local limit")]
    ExecutorTooLarge,
    #[error("executor bundle no longer matches the approved template digest")]
    ExecutorDigestMismatch,
    #[error("child executor output exceeds the {MAX_CHILD_OUTPUT_BYTES}-byte local limit")]
    ExecutorOutputTooLarge,
    #[error("child executor output is not the deterministic dataset-prepare result")]
    ExecutorOutputMismatch,
    #[error("dataset output is malformed or not canonical")]
    NonCanonicalDatasetOutput,
    #[error("worker manifest sequence must be non-zero")]
    InvalidWorkerSequence,
    #[error("operator registry authority must be an operator key")]
    InvalidOperatorAuthority,
    #[error("a signed assignment or worker result was already claimed")]
    ReplayDetected,
    #[error("local dataset candidates must be at most {MAX_LOCAL_DATASET_BYTES} bytes")]
    LocalCandidateTooLarge,
    #[error("the system clock predates the Unix epoch")]
    ClockBeforeUnixEpoch,
    #[error("the system clock cannot be represented in Unix milliseconds")]
    ClockOverflow,
    #[error("submission does not match its signed manifest, completion or active lease")]
    SubmissionMismatch,
}

impl LocalWorker {
    /// Builds a local worker with one hard-coded, approved template shape.
    pub fn new(config: LocalWorkerConfig) -> Result<Self, LocalWorkerError> {
        let LocalWorkerConfig {
            identity,
            operator_registry_key,
            template,
            parameters,
            environment_digest,
            clock,
            executor_path,
            stager,
        } = config;
        if operator_registry_key.signer_kind != SignerKind::Operator {
            return Err(LocalWorkerError::InvalidOperatorAuthority);
        }
        validate_dataset_template(&template, &parameters)?;
        verify_executor_bundle(&executor_path, template.bundle_digest)?;
        Ok(Self {
            identity,
            operator_registry_key,
            template,
            parameters,
            environment_digest,
            clock,
            executor_path,
            stager,
            replay: Arc::new(Mutex::new(WorkerReplayState::default())),
        })
    }

    /// Runs the constrained child, stages its output and signs the manifest.
    ///
    /// Development-only replay state is owned by the supervisor. A durable
    /// worker-session store replaces it before any remote worker is admitted.
    pub async fn execute_dataset_prepare(
        &self,
        signed_request: &SignedEnvelopeV1<ExecutorRequestV1>,
        source: &LocalDatasetInputV1,
    ) -> Result<DatasetSubmissionV1, LocalWorkerError> {
        let now_unix_ms = self.clock.now_unix_ms()?;
        verify_operator_assignment(signed_request, &self.operator_registry_key, now_unix_ms)?;
        let request = &signed_request.payload;
        validate_dataset_request(
            request,
            &self.template,
            &self.parameters,
            Some(self.identity.worker_id()),
            Some(self.environment_digest),
            now_unix_ms,
        )?;
        let source_bytes = load_and_validate_input(source, &request.lease).await?;
        let worker_sequence = self
            .replay
            .lock()
            .await
            .claim_assignment_and_allocate_manifest_sequence(
                &signed_request.message_id,
                signed_request.sequence,
            )?;
        let expected_output = normalise_dataset_jsonl(&source_bytes, self.parameters.max_records)?;
        let executor_output = self.run_child_executor(&source_bytes).await?;
        if executor_output != expected_output {
            return Err(LocalWorkerError::ExecutorOutputMismatch);
        }

        let output_directory = tempfile::tempdir()?;
        let output_path = output_directory.path().join("dataset.jsonl");
        tokio::fs::write(&output_path, &executor_output).await?;
        let candidate = self
            .stager
            .stage_candidate_file(&request.lease, &output_path)
            .await?;

        let completion_time_unix_ms = self.clock.now_unix_ms()?;
        if let Err(error) = request
            .lease
            .accepts_fencing_token(request.lease.fencing_token, completion_time_unix_ms)
        {
            let _ = self.stager.discard_candidate(&candidate).await;
            return Err(LocalWorkerError::Protocol(error));
        }

        let manifest = ArtifactManifestV1 {
            schema_version: SCHEMA_VERSION_V1,
            artifact_id: ArtifactId::new(format!(
                "dataset-{}",
                hex::encode(candidate.content_digest().as_bytes())
            ))?,
            content_digest: candidate.content_digest(),
            byte_length: candidate.byte_length(),
            logical_type: ArtifactType::Dataset,
            lineage: ArtifactLineageV1 {
                pipeline_id: request.lease.pipeline_id.clone(),
                stage_id: request.lease.stage_id.clone(),
                attempt_id: request.lease.attempt_id.clone(),
                provider_id: self.identity.provider_id.clone(),
                worker_id: self.identity.worker_id.clone(),
                fencing_token: request.lease.fencing_token,
                input_manifest_digests: request.lease.input_manifest_digests.clone(),
            },
            environment_digest: request.environment_digest,
            data_class: DataClass::Public,
            retention: RetentionClass::PilotAccepted,
        };
        manifest.validate_against_lease(&request.lease)?;
        let manifest_digest = manifest.canonical_digest()?;
        let signed_manifest = self.identity.sign_artifact_manifest(
            manifest,
            worker_sequence,
            completion_time_unix_ms,
        )?;
        let completion = ExecutorCompletionV1 {
            schema_version: SCHEMA_VERSION_V1,
            pipeline_id: request.lease.pipeline_id.clone(),
            stage_id: request.lease.stage_id.clone(),
            attempt_id: request.lease.attempt_id.clone(),
            fencing_token: request.lease.fencing_token,
            status: CompletionStatus::Succeeded,
            output_manifest_digests: vec![manifest_digest],
            final_metrics_digest: None,
            reason_code: "dataset_prepare_v1_succeeded".into(),
        };
        completion.validate_against(&request.lease, completion_time_unix_ms)?;
        Ok(DatasetSubmissionV1 {
            candidate,
            signed_manifest,
            completion,
        })
    }

    async fn run_child_executor(&self, source: &[u8]) -> Result<Vec<u8>, LocalWorkerError> {
        let request = DatasetExecutorInputV1 {
            schema_version: SCHEMA_VERSION_V1,
            max_records: self.parameters.max_records,
            source_base64: STANDARD_NO_PAD.encode(source),
        };
        let encoded_request = serde_json::to_vec(&request)?;
        if encoded_request.len() > MAX_LOCAL_EXECUTOR_REQUEST_BYTES {
            return Err(LocalWorkerError::InputTooLarge);
        }
        verify_executor_bundle(&self.executor_path, self.template.bundle_digest)?;

        let attempt_directory = tempfile::tempdir()?;
        let mut child = Command::new(&self.executor_path)
            .arg("--dataset-prepare-v1")
            .current_dir(attempt_directory.path())
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let mut stdin = child.stdin.take().ok_or(LocalWorkerError::ExecutorFailed)?;
        let stdout = child
            .stdout
            .take()
            .ok_or(LocalWorkerError::ExecutorFailed)?;
        let execution = timeout(EXECUTOR_TIMEOUT, async {
            let write_stdin = async {
                stdin.write_all(&encoded_request).await?;
                stdin.shutdown().await?;
                drop(stdin);
                Ok::<(), LocalWorkerError>(())
            };
            let read_stdout = async {
                let limit = u64::try_from(MAX_CHILD_OUTPUT_BYTES + 1)
                    .map_err(|_| LocalWorkerError::ExecutorOutputTooLarge)?;
                let mut bounded_stdout = stdout.take(limit);
                let mut output = Vec::with_capacity(MAX_CHILD_OUTPUT_BYTES + 1);
                bounded_stdout.read_to_end(&mut output).await?;
                if output.len() > MAX_CHILD_OUTPUT_BYTES {
                    return Err(LocalWorkerError::ExecutorOutputTooLarge);
                }
                Ok::<Vec<u8>, LocalWorkerError>(output)
            };
            let wait_for_exit = async { Ok::<_, LocalWorkerError>(child.wait().await?) };
            let ((), output, status) = tokio::try_join!(write_stdin, read_stdout, wait_for_exit)?;
            if !status.success() {
                return Err(LocalWorkerError::ExecutorFailed);
            }
            Ok(output)
        })
        .await;
        match execution {
            Ok(Ok(output)) => Ok(output),
            Ok(Err(error)) => {
                let _ = child.kill().await;
                Err(error)
            }
            Err(_) => {
                let _ = child.kill().await;
                Err(LocalWorkerError::ExecutorTimedOut)
            }
        }
    }
}

impl LocalDatasetVerifier {
    /// Creates the trusted local verifier side of the vertical slice.
    pub fn new(config: LocalDatasetVerifierConfig) -> Result<Self, LocalWorkerError> {
        let LocalDatasetVerifierConfig {
            template,
            parameters,
            operator_registry_key,
            signed_assignment,
            source,
            worker_registry_key,
            clock,
            stager,
            promoter,
        } = config;
        validate_dataset_template(&template, &parameters)?;
        if operator_registry_key.signer_kind != SignerKind::Operator {
            return Err(LocalWorkerError::InvalidOperatorAuthority);
        }
        Ok(Self {
            template,
            parameters,
            operator_registry_key,
            signed_assignment,
            source,
            worker_registry_key,
            clock,
            stager,
            promoter,
            replay: Arc::new(Mutex::new(ReplayGuard::default())),
        })
    }

    /// Independently validates and promotes a local dataset candidate.
    pub async fn verify_and_promote(
        &self,
        submission: DatasetSubmissionV1,
    ) -> Result<AcceptedDatasetRunV1, LocalWorkerError> {
        let now_unix_ms = self.clock.now_unix_ms()?;
        let validation = self.validate_submission(&submission, now_unix_ms).await;
        if let Err(error) = validation {
            let _ = self.stager.discard_candidate(&submission.candidate).await;
            return Err(error);
        }
        if let Err(error) = self.validate_assignment_binding(&submission, self.clock.now_unix_ms()?)
        {
            let _ = self.stager.discard_candidate(&submission.candidate).await;
            return Err(error);
        }
        let accepted_artifact = match self.promoter.promote(&submission.candidate).await {
            Ok(accepted) => accepted,
            Err(error) => {
                let _ = self.stager.discard_candidate(&submission.candidate).await;
                return Err(LocalWorkerError::Artifact(error));
            }
        };
        // Candidate bytes are only an attempt-scoped handoff. The immutable
        // accepted copy is now the retained object; cleanup must not turn a
        // successful promotion into a retry if the local delete races or fails.
        let _ = self.stager.discard_candidate(&submission.candidate).await;
        Ok(AcceptedDatasetRunV1 {
            accepted_artifact,
            signed_manifest: submission.signed_manifest,
            completion: submission.completion,
        })
    }

    async fn validate_submission(
        &self,
        submission: &DatasetSubmissionV1,
        now_unix_ms: u64,
    ) -> Result<(), LocalWorkerError> {
        self.validate_assignment_binding(submission, now_unix_ms)?;
        let request = &self.signed_assignment.payload;
        let source_bytes = load_and_validate_input(&self.source, &request.lease).await?;
        let expected_output = normalise_dataset_jsonl(&source_bytes, self.parameters.max_records)?;
        self.stager.verify_candidate(&submission.candidate).await?;

        let temporary = tempfile::tempdir()?;
        let output_path = temporary.path().join("candidate.jsonl");
        self.stager
            .copy_candidate_to_file(&submission.candidate, &output_path)
            .await?;
        let output = tokio::fs::read(output_path).await?;
        if output != expected_output {
            return Err(LocalWorkerError::SubmissionMismatch);
        }
        self.replay.lock().await.claim(
            &submission.signed_manifest.message_id,
            submission.signed_manifest.sequence,
        )?;
        Ok(())
    }

    fn validate_assignment_binding(
        &self,
        submission: &DatasetSubmissionV1,
        now_unix_ms: u64,
    ) -> Result<(), LocalWorkerError> {
        verify_operator_assignment(
            &self.signed_assignment,
            &self.operator_registry_key,
            now_unix_ms,
        )?;
        let request = &self.signed_assignment.payload;
        validate_dataset_request(
            request,
            &self.template,
            &self.parameters,
            None,
            None,
            now_unix_ms,
        )?;
        submission.signed_manifest.verify_with_registry_key(
            &self.worker_registry_key,
            now_unix_ms,
            1,
        )?;
        let manifest = &submission.signed_manifest.payload;
        manifest.validate_registry_signer(&self.worker_registry_key)?;
        if submission.candidate.lease() != &request.lease {
            return Err(LocalWorkerError::SubmissionMismatch);
        }
        if submission.candidate.byte_length()
            > u64::try_from(MAX_LOCAL_DATASET_BYTES)
                .map_err(|_| LocalWorkerError::LocalCandidateTooLarge)?
        {
            return Err(LocalWorkerError::LocalCandidateTooLarge);
        }
        manifest.validate_against_lease(&request.lease)?;
        if manifest.logical_type != ArtifactType::Dataset
            || manifest.data_class != DataClass::Public
            || manifest.retention != RetentionClass::PilotAccepted
            || manifest.environment_digest != request.environment_digest
            || manifest.content_digest != submission.candidate.content_digest()
            || manifest.byte_length != submission.candidate.byte_length()
            || submission.completion.status != CompletionStatus::Succeeded
            || submission.completion.output_manifest_digests != vec![manifest.canonical_digest()?]
        {
            return Err(LocalWorkerError::SubmissionMismatch);
        }
        submission
            .completion
            .validate_against(&request.lease, now_unix_ms)?;
        Ok(())
    }
}

fn validate_dataset_template(
    template: &TemplateManifestV1,
    parameters: &DatasetPrepareParametersV1,
) -> Result<(), LocalWorkerError> {
    parameters.validate()?;
    template.validate()?;
    if template.template_id.as_str() != DATASET_PREPARE_TEMPLATE_ID
        || template.runtime != RuntimeKind::NativeRust
        || template.network_policy != NetworkPolicy::DenyAll
        || !template.input_types.contains(&ArtifactType::Dataset)
        || !template.output_types.contains(&ArtifactType::Dataset)
        || template.parameter_schema_digest != parameters.canonical_digest()?
    {
        return Err(LocalWorkerError::UnsupportedTemplate);
    }
    Ok(())
}

fn verify_operator_assignment(
    signed_assignment: &SignedEnvelopeV1<ExecutorRequestV1>,
    operator_registry_key: &TrustedEd25519KeyV1,
    now_unix_ms: u64,
) -> Result<(), LocalWorkerError> {
    if operator_registry_key.signer_kind != SignerKind::Operator {
        return Err(LocalWorkerError::InvalidOperatorAuthority);
    }
    signed_assignment.verify_with_registry_key(operator_registry_key, now_unix_ms, 1)?;
    Ok(())
}

fn validate_dataset_request(
    request: &ExecutorRequestV1,
    template: &TemplateManifestV1,
    parameters: &DatasetPrepareParametersV1,
    expected_worker: Option<&WorkerId>,
    expected_environment: Option<Sha256Digest>,
    now_unix_ms: u64,
) -> Result<(), LocalWorkerError> {
    request.validate()?;
    request
        .lease
        .accepts_fencing_token(request.lease.fencing_token, now_unix_ms)?;
    if let Some(worker_id) = expected_worker
        && &request.lease.worker_id != worker_id
    {
        return Err(LocalWorkerError::WrongWorker);
    }
    if request.lease.template_digest != template.bundle_digest
        || request.parameter_digest != parameters.canonical_digest()?
    {
        return Err(LocalWorkerError::RequestTemplateMismatch);
    }
    if expected_environment.is_some_and(|digest| request.environment_digest != digest)
        || request.network_policy != NetworkPolicy::DenyAll
        || request.resume_checkpoint_manifest.is_some()
    {
        return Err(LocalWorkerError::RequestPolicyMismatch);
    }
    if !resources_fit(&request.resources, &template.maximum_resources) {
        return Err(LocalWorkerError::ResourceLimitExceeded);
    }
    Ok(())
}

async fn load_and_validate_input(
    source: &LocalDatasetInputV1,
    lease: &colibri_protocol::lease::StageLeaseV1,
) -> Result<Vec<u8>, LocalWorkerError> {
    source.manifest.validate()?;
    let manifest_digest = source.manifest.canonical_digest()?;
    if source.manifest.logical_type != ArtifactType::Dataset
        || source.manifest.data_class != DataClass::Public
        || lease.input_manifest_digests.len() != 1
        || lease.input_manifest_digests.first() != Some(&manifest_digest)
    {
        return Err(LocalWorkerError::InputMismatch);
    }
    let metadata = tokio::fs::symlink_metadata(&source.source_path).await?;
    if !metadata.file_type().is_file() {
        return Err(LocalWorkerError::InputNotRegularFile);
    }
    let source_length =
        usize::try_from(metadata.len()).map_err(|_| LocalWorkerError::InputTooLarge)?;
    if source_length > MAX_LOCAL_DATASET_BYTES {
        return Err(LocalWorkerError::InputTooLarge);
    }
    let bytes = tokio::fs::read(&source.source_path).await?;
    if bytes.len() != source_length
        || u64::try_from(bytes.len()).map_err(|_| LocalWorkerError::InputTooLarge)?
            != source.manifest.byte_length
        || Sha256Digest::of_bytes(&bytes) != source.manifest.content_digest
    {
        return Err(LocalWorkerError::InputMismatch);
    }
    Ok(bytes)
}

fn verify_executor_bundle(
    executor_path: &Path,
    expected_digest: Sha256Digest,
) -> Result<(), LocalWorkerError> {
    let metadata = std::fs::symlink_metadata(executor_path)?;
    if !metadata.file_type().is_file() {
        return Err(LocalWorkerError::ExecutorNotRegularFile {
            path: executor_path.to_owned(),
        });
    }
    if metadata.len() > MAX_LOCAL_EXECUTOR_BYTES {
        return Err(LocalWorkerError::ExecutorTooLarge);
    }
    let mut file = std::fs::File::open(executor_path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    let mut observed_length = 0_u64;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        observed_length = observed_length
            .checked_add(u64::try_from(read).map_err(|_| LocalWorkerError::ExecutorTooLarge)?)
            .ok_or(LocalWorkerError::ExecutorTooLarge)?;
        if observed_length > MAX_LOCAL_EXECUTOR_BYTES {
            return Err(LocalWorkerError::ExecutorTooLarge);
        }
        hasher.update(&buffer[..read]);
    }
    let final_metadata = std::fs::symlink_metadata(executor_path)?;
    if !final_metadata.file_type().is_file() || final_metadata.len() != observed_length {
        return Err(LocalWorkerError::ExecutorDigestMismatch);
    }
    let raw_digest: [u8; 32] = hasher.finalize().into();
    if Sha256Digest::from_raw_bytes(raw_digest) != expected_digest {
        return Err(LocalWorkerError::ExecutorDigestMismatch);
    }
    Ok(())
}

fn resources_fit(
    requested: &colibri_protocol::pipeline::ResourceRequirementsV1,
    maximum: &colibri_protocol::pipeline::ResourceRequirementsV1,
) -> bool {
    requested.cpu_cores <= maximum.cpu_cores
        && requested.ram_bytes <= maximum.ram_bytes
        && requested.accelerator_memory_bytes <= maximum.accelerator_memory_bytes
        && requested.scratch_bytes <= maximum.scratch_bytes
        && requested.wall_time_seconds <= maximum.wall_time_seconds
        && requested
            .required_precisions
            .iter()
            .all(|precision| maximum.required_precisions.contains(precision))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatasetExecutorInputV1 {
    pub schema_version: u16,
    pub max_records: u16,
    pub source_base64: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DatasetRecordV1 {
    id: String,
    input: String,
    target: String,
    task: String,
}

/// Executes the repository-owned child payload without filesystem, storage or
/// credential access. The binary wrapper passes it bytes from stdin and writes
/// only the normalised data bytes to stdout.
pub fn execute_dataset_prepare_child(
    input: DatasetExecutorInputV1,
) -> Result<Vec<u8>, LocalWorkerError> {
    if input.schema_version != SCHEMA_VERSION_V1 {
        return Err(LocalWorkerError::Protocol(
            ProtocolError::UnsupportedSchemaVersion {
                actual: input.schema_version,
                expected: SCHEMA_VERSION_V1,
            },
        ));
    }
    let source = STANDARD_NO_PAD.decode(input.source_base64)?;
    if source.len() > MAX_LOCAL_DATASET_BYTES {
        return Err(LocalWorkerError::InputTooLarge);
    }
    normalise_dataset_jsonl(&source, input.max_records)
}

/// Validates records and emits canonical JSON Lines in their original order.
pub fn normalise_dataset_jsonl(
    source: &[u8],
    max_records: u16,
) -> Result<Vec<u8>, LocalWorkerError> {
    if source.is_empty() || source.len() > MAX_LOCAL_DATASET_BYTES {
        return Err(LocalWorkerError::NonCanonicalDatasetOutput);
    }
    if max_records == 0 || usize::from(max_records) > 100 {
        return Err(LocalWorkerError::InvalidParameters);
    }

    let mut output = Vec::with_capacity(source.len());
    let mut identifiers = BTreeSet::new();
    let mut count = 0_usize;
    for line in source.split_inclusive(|byte| *byte == b'\n') {
        if !line.ends_with(b"\n") || line.len() == 1 {
            return Err(LocalWorkerError::NonCanonicalDatasetOutput);
        }
        let record: DatasetRecordV1 = serde_json::from_slice(&line[..line.len() - 1])?;
        if record.id.is_empty()
            || record.id.len() > 96
            || record.input.is_empty()
            || record.target.is_empty()
            || record.task.is_empty()
            || !identifiers.insert(record.id.clone())
        {
            return Err(LocalWorkerError::NonCanonicalDatasetOutput);
        }
        count += 1;
        if count > usize::from(max_records) {
            return Err(LocalWorkerError::NonCanonicalDatasetOutput);
        }
        output.extend(serde_jcs::to_vec(&record).map_err(|error| {
            LocalWorkerError::Protocol(ProtocolError::CanonicalEncoding(error.to_string()))
        })?);
        output.push(b'\n');
    }
    if count == 0 || output.len() > MAX_CHILD_OUTPUT_BYTES {
        return Err(LocalWorkerError::NonCanonicalDatasetOutput);
    }
    Ok(output)
}
