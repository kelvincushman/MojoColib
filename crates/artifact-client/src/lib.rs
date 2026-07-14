//! Attempt-scoped artifact staging and immutable content-addressed promotion.
//!
//! The worker-side staging capability and the trusted promotion capability are
//! separate Rust types. Executor processes receive neither capability: they
//! write an output file, while the worker supervisor owns byte transfer and
//! the coordinator/verifier side owns promotion.

#![forbid(unsafe_code)]
#![allow(clippy::missing_errors_doc)]

use std::path::{Path, PathBuf};

use colibri_protocol::{
    ProtocolError, artifact::MAX_ARTIFACT_BYTES, digest::Sha256Digest, lease::StageLeaseV1,
};
use opendal::{ErrorKind, Operator, Writer, services::Fs};
use sha2::{Digest as _, Sha256};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const STORAGE_LAYOUT_V1: &str = "v1";
const STREAM_CHUNK_BYTES: usize = 1024 * 1024;

/// A candidate object produced under one fenced stage lease.
///
/// Candidates are deliberately not visible through the accepted-object API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateArtifactV1 {
    lease: StageLeaseV1,
    storage_key: String,
    content_digest: Sha256Digest,
    byte_length: u64,
}

impl CandidateArtifactV1 {
    /// Returns the source lease that scopes the candidate.
    #[must_use]
    pub const fn lease(&self) -> &StageLeaseV1 {
        &self.lease
    }

    /// Returns the backend-relative, attempt-scoped candidate key.
    #[must_use]
    pub fn storage_key(&self) -> &str {
        &self.storage_key
    }

    /// Returns the digest calculated from the staged bytes.
    #[must_use]
    pub const fn content_digest(&self) -> Sha256Digest {
        self.content_digest
    }

    /// Returns the byte length calculated from the staged bytes.
    #[must_use]
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }
}

/// An accepted, immutable object identified solely by its content digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedArtifactV1 {
    storage_key: String,
    content_digest: Sha256Digest,
    byte_length: u64,
}

impl AcceptedArtifactV1 {
    /// Returns the backend-relative immutable object key.
    #[must_use]
    pub fn storage_key(&self) -> &str {
        &self.storage_key
    }

    /// Returns the digest that identifies the accepted object.
    #[must_use]
    pub const fn content_digest(&self) -> Sha256Digest {
        self.content_digest
    }

    /// Returns the verified byte length.
    #[must_use]
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }
}

/// Errors raised by a storage adapter before an artifact can be accepted.
#[derive(Debug, Error)]
pub enum ArtifactStoreError {
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    #[error("artifact storage root is not valid UTF-8: {0}")]
    NonUtf8Root(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Storage(#[from] opendal::Error),
    #[error("artifact source is not a regular file: {path}")]
    SourceNotRegularFile { path: PathBuf },
    #[error("artifacts must contain at least one byte")]
    EmptyArtifact,
    #[error("artifact byte length cannot be represented by the V1 contract")]
    LengthOverflow,
    #[error("artifact is {actual} bytes; V1 permits at most {MAX_ARTIFACT_BYTES} bytes")]
    ArtifactTooLarge { actual: u64 },
    #[error(
        "stored object {storage_key} does not match expected digest {expected_digest} and length {expected_length}"
    )]
    IntegrityMismatch {
        storage_key: String,
        expected_digest: Sha256Digest,
        expected_length: u64,
    },
}

/// Capability held by a worker supervisor while it moves an executor output
/// into attempt-scoped candidate storage.
#[allow(async_fn_in_trait)]
pub trait ArtifactStager {
    /// Streams a regular local file under the supplied attempt lease.
    async fn stage_candidate_file(
        &self,
        lease: &StageLeaseV1,
        source: &Path,
    ) -> Result<CandidateArtifactV1, ArtifactStoreError>;

    /// Re-reads and validates a candidate's bytes.
    async fn verify_candidate(
        &self,
        candidate: &CandidateArtifactV1,
    ) -> Result<(), ArtifactStoreError>;

    /// Streams a candidate into a new local file without loading it all into
    /// memory. The destination must not already exist.
    async fn copy_candidate_to_file(
        &self,
        candidate: &CandidateArtifactV1,
        destination: &Path,
    ) -> Result<(), ArtifactStoreError>;

    /// Removes a candidate without affecting any accepted object.
    async fn discard_candidate(
        &self,
        candidate: &CandidateArtifactV1,
    ) -> Result<(), ArtifactStoreError>;
}

/// Capability held only by the trusted verifier/coordinator side of a worker
/// flow. Possession of a candidate alone is insufficient to call `promote`.
#[allow(async_fn_in_trait)]
pub trait ArtifactPromoter {
    /// Promotes an already semantically-verified candidate to an immutable key.
    async fn promote(
        &self,
        candidate: &CandidateArtifactV1,
    ) -> Result<AcceptedArtifactV1, ArtifactStoreError>;

    /// Re-reads and validates an accepted object by digest.
    async fn verify_accepted(
        &self,
        digest: Sha256Digest,
    ) -> Result<AcceptedArtifactV1, ArtifactStoreError>;

    /// Streams an accepted object into a new local file without loading it all
    /// into memory. The destination must not already exist.
    async fn copy_accepted_to_file(
        &self,
        digest: Sha256Digest,
        destination: &Path,
    ) -> Result<AcceptedArtifactV1, ArtifactStoreError>;
}

/// `OpenDAL` filesystem implementation used by local development and tests.
///
/// The public key layout is portable to object stores and Windows-safe: every
/// user-controlled identifier is hex encoded, and digests use raw lowercase
/// hexadecimal rather than their wire-format `sha256:` representation. It does
/// not rely on filesystem rename semantics.
#[derive(Clone)]
pub struct FilesystemArtifactStore {
    operator: Operator,
}

/// The staging half of a [`FilesystemArtifactStore`].
#[derive(Clone)]
pub struct FilesystemArtifactStager {
    store: FilesystemArtifactStore,
}

/// The trusted promotion half of a [`FilesystemArtifactStore`].
#[derive(Clone)]
pub struct FilesystemArtifactPromoter {
    store: FilesystemArtifactStore,
}

impl FilesystemArtifactStore {
    /// Creates an artifact store rooted at a trusted local directory.
    pub fn new(root: impl AsRef<Path>) -> Result<Self, ArtifactStoreError> {
        std::fs::create_dir_all(root.as_ref())?;
        let root = root.as_ref().canonicalize()?;
        let root_text = root
            .to_str()
            .ok_or_else(|| ArtifactStoreError::NonUtf8Root(root.display().to_string()))?;
        let operator = Operator::new(Fs::default().root(root_text))?;
        Ok(Self { operator })
    }

    /// Splits the store into the worker staging and trusted promotion
    /// capabilities. Do not pass the promotion capability into an executor.
    #[must_use]
    pub fn into_capabilities(self) -> (FilesystemArtifactStager, FilesystemArtifactPromoter) {
        (
            FilesystemArtifactStager {
                store: self.clone(),
            },
            FilesystemArtifactPromoter { store: self },
        )
    }

    fn storage_segment(value: &str) -> String {
        format!("id-{}", hex::encode(value))
    }

    fn digest_segment(digest: Sha256Digest) -> String {
        hex::encode(digest.as_bytes())
    }

    fn candidate_key(lease: &StageLeaseV1, digest: Sha256Digest) -> String {
        format!(
            "{STORAGE_LAYOUT_V1}/candidates/{}/{}/{}/{}/{}",
            Self::storage_segment(lease.pipeline_id.as_str()),
            Self::storage_segment(lease.stage_id.as_str()),
            Self::storage_segment(lease.attempt_id.as_str()),
            lease.fencing_token.value(),
            Self::digest_segment(digest),
        )
    }

    fn accepted_key(digest: Sha256Digest) -> String {
        format!(
            "{STORAGE_LAYOUT_V1}/accepted/{}",
            Self::digest_segment(digest)
        )
    }

    fn mismatch(
        storage_key: &str,
        expected_digest: Sha256Digest,
        expected_length: u64,
    ) -> ArtifactStoreError {
        ArtifactStoreError::IntegrityMismatch {
            storage_key: storage_key.to_owned(),
            expected_digest,
            expected_length,
        }
    }

    fn is_condition_not_match(error: &ArtifactStoreError) -> bool {
        matches!(
            error,
            ArtifactStoreError::Storage(storage_error)
                if storage_error.kind() == ErrorKind::ConditionNotMatch
        )
    }

    fn validate_length(length: u64) -> Result<(), ArtifactStoreError> {
        if length == 0 {
            return Err(ArtifactStoreError::EmptyArtifact);
        }
        if length > MAX_ARTIFACT_BYTES {
            return Err(ArtifactStoreError::ArtifactTooLarge { actual: length });
        }
        Ok(())
    }

    fn next_length(current: u64, chunk_length: usize) -> Result<u64, ArtifactStoreError> {
        let chunk_length =
            u64::try_from(chunk_length).map_err(|_| ArtifactStoreError::LengthOverflow)?;
        let next = current
            .checked_add(chunk_length)
            .ok_or(ArtifactStoreError::LengthOverflow)?;
        Self::validate_length(next)?;
        Ok(next)
    }

    fn final_digest(
        hasher: Sha256,
        length: u64,
    ) -> Result<(Sha256Digest, u64), ArtifactStoreError> {
        Self::validate_length(length)?;
        let raw_bytes: [u8; 32] = hasher.finalize().into();
        Ok((Sha256Digest::from_raw_bytes(raw_bytes), length))
    }

    async fn validate_source_file(source: &Path) -> Result<(), ArtifactStoreError> {
        let metadata = tokio::fs::symlink_metadata(source).await?;
        if !metadata.file_type().is_file() {
            return Err(ArtifactStoreError::SourceNotRegularFile {
                path: source.to_owned(),
            });
        }
        Ok(())
    }

    async fn hash_source_file(source: &Path) -> Result<(Sha256Digest, u64), ArtifactStoreError> {
        Self::validate_source_file(source).await?;
        let mut source_file = tokio::fs::File::open(source).await?;
        let mut buffer = vec![0_u8; STREAM_CHUNK_BYTES];
        let mut hasher = Sha256::new();
        let mut length = 0_u64;

        loop {
            let read = source_file.read(&mut buffer).await?;
            if read == 0 {
                break;
            }
            length = Self::next_length(length, read)?;
            hasher.update(&buffer[..read]);
        }
        Self::final_digest(hasher, length)
    }

    async fn write_source_to_writer(
        source: &Path,
        writer: &mut Writer,
        expected_digest: Sha256Digest,
        expected_length: u64,
        storage_key: &str,
    ) -> Result<(), ArtifactStoreError> {
        Self::validate_source_file(source).await?;
        let mut source_file = tokio::fs::File::open(source).await?;
        let mut buffer = vec![0_u8; STREAM_CHUNK_BYTES];
        let mut hasher = Sha256::new();
        let mut length = 0_u64;

        loop {
            let read = source_file.read(&mut buffer).await?;
            if read == 0 {
                break;
            }
            length = Self::next_length(length, read)?;
            hasher.update(&buffer[..read]);
            writer.write(buffer[..read].to_vec()).await?;
        }

        let (actual_digest, actual_length) = Self::final_digest(hasher, length)?;
        if actual_digest != expected_digest || actual_length != expected_length {
            return Err(Self::mismatch(
                storage_key,
                expected_digest,
                expected_length,
            ));
        }
        Ok(())
    }

    async fn verify_object(
        &self,
        storage_key: &str,
        expected_digest: Sha256Digest,
        expected_length: u64,
    ) -> Result<(), ArtifactStoreError> {
        let metadata = self.operator.stat(storage_key).await?;
        if metadata.content_length() != expected_length {
            return Err(Self::mismatch(
                storage_key,
                expected_digest,
                expected_length,
            ));
        }
        Self::validate_length(expected_length)?;
        let reader = self.operator.reader(storage_key).await?;
        let mut hasher = Sha256::new();
        let mut length = 0_u64;

        while length < expected_length {
            let end = length
                .saturating_add(u64::try_from(STREAM_CHUNK_BYTES).unwrap())
                .min(expected_length);
            let bytes = reader.read(length..end).await?.to_vec();
            if bytes.is_empty() {
                return Err(Self::mismatch(
                    storage_key,
                    expected_digest,
                    expected_length,
                ));
            }
            length = Self::next_length(length, bytes.len())?;
            if length > expected_length {
                return Err(Self::mismatch(
                    storage_key,
                    expected_digest,
                    expected_length,
                ));
            }
            hasher.update(&bytes);
        }

        let (actual_digest, actual_length) = Self::final_digest(hasher, length)?;
        if actual_digest != expected_digest || actual_length != expected_length {
            return Err(Self::mismatch(
                storage_key,
                expected_digest,
                expected_length,
            ));
        }
        Ok(())
    }

    async fn write_object_to_writer(
        &self,
        source_key: &str,
        writer: &mut Writer,
        expected_digest: Sha256Digest,
        expected_length: u64,
        destination_key: &str,
    ) -> Result<(), ArtifactStoreError> {
        let metadata = self.operator.stat(source_key).await?;
        if metadata.content_length() != expected_length {
            return Err(Self::mismatch(source_key, expected_digest, expected_length));
        }
        Self::validate_length(expected_length)?;
        let reader = self.operator.reader(source_key).await?;
        let mut hasher = Sha256::new();
        let mut length = 0_u64;

        while length < expected_length {
            let end = length
                .saturating_add(u64::try_from(STREAM_CHUNK_BYTES).unwrap())
                .min(expected_length);
            let bytes = reader.read(length..end).await?.to_vec();
            if bytes.is_empty() {
                return Err(Self::mismatch(source_key, expected_digest, expected_length));
            }
            length = Self::next_length(length, bytes.len())?;
            if length > expected_length {
                return Err(Self::mismatch(source_key, expected_digest, expected_length));
            }
            hasher.update(&bytes);
            writer.write(bytes).await?;
        }

        let (actual_digest, actual_length) = Self::final_digest(hasher, length)?;
        if actual_digest != expected_digest || actual_length != expected_length {
            return Err(Self::mismatch(
                destination_key,
                expected_digest,
                expected_length,
            ));
        }
        Ok(())
    }

    async fn write_immutable_from_source(
        &self,
        source: &Path,
        storage_key: &str,
        expected_digest: Sha256Digest,
        expected_length: u64,
    ) -> Result<(), ArtifactStoreError> {
        let mut writer = match self
            .operator
            .writer_with(storage_key)
            .if_not_exists(true)
            .await
        {
            Ok(writer) => writer,
            Err(error) if error.kind() == ErrorKind::ConditionNotMatch => {
                return self
                    .verify_object(storage_key, expected_digest, expected_length)
                    .await;
            }
            Err(error) => return Err(ArtifactStoreError::Storage(error)),
        };

        if let Err(error) = Self::write_source_to_writer(
            source,
            &mut writer,
            expected_digest,
            expected_length,
            storage_key,
        )
        .await
        {
            let _ = writer.abort().await;
            if Self::is_condition_not_match(&error) {
                return self
                    .verify_object(storage_key, expected_digest, expected_length)
                    .await;
            }
            return Err(error);
        }
        match writer.close().await {
            Ok(_) => {
                self.verify_object(storage_key, expected_digest, expected_length)
                    .await
            }
            Err(error) if error.kind() == ErrorKind::ConditionNotMatch => {
                self.verify_object(storage_key, expected_digest, expected_length)
                    .await
            }
            Err(error) => Err(ArtifactStoreError::Storage(error)),
        }
    }

    async fn write_immutable_from_object(
        &self,
        source_key: &str,
        destination_key: &str,
        expected_digest: Sha256Digest,
        expected_length: u64,
    ) -> Result<(), ArtifactStoreError> {
        let mut writer = match self
            .operator
            .writer_with(destination_key)
            .if_not_exists(true)
            .await
        {
            Ok(writer) => writer,
            Err(error) if error.kind() == ErrorKind::ConditionNotMatch => {
                return self
                    .verify_object(destination_key, expected_digest, expected_length)
                    .await;
            }
            Err(error) => return Err(ArtifactStoreError::Storage(error)),
        };

        if let Err(error) = self
            .write_object_to_writer(
                source_key,
                &mut writer,
                expected_digest,
                expected_length,
                destination_key,
            )
            .await
        {
            let _ = writer.abort().await;
            if Self::is_condition_not_match(&error) {
                return self
                    .verify_object(destination_key, expected_digest, expected_length)
                    .await;
            }
            return Err(error);
        }
        match writer.close().await {
            Ok(_) => {
                self.verify_object(destination_key, expected_digest, expected_length)
                    .await
            }
            Err(error) if error.kind() == ErrorKind::ConditionNotMatch => {
                self.verify_object(destination_key, expected_digest, expected_length)
                    .await
            }
            Err(error) => Err(ArtifactStoreError::Storage(error)),
        }
    }

    async fn delete_if_present(&self, storage_key: &str) -> Result<(), ArtifactStoreError> {
        match self.operator.delete(storage_key).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(error) => Err(ArtifactStoreError::Storage(error)),
        }
    }

    async fn copy_object_to_file(
        &self,
        storage_key: &str,
        destination: &Path,
        expected_digest: Sha256Digest,
        expected_length: u64,
    ) -> Result<(), ArtifactStoreError> {
        let metadata = self.operator.stat(storage_key).await?;
        if metadata.content_length() != expected_length {
            return Err(Self::mismatch(
                storage_key,
                expected_digest,
                expected_length,
            ));
        }
        Self::validate_length(expected_length)?;
        let reader = self.operator.reader(storage_key).await?;
        let mut destination_file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)
            .await?;
        let mut hasher = Sha256::new();
        let mut length = 0_u64;

        while length < expected_length {
            let end = length
                .saturating_add(u64::try_from(STREAM_CHUNK_BYTES).unwrap())
                .min(expected_length);
            let bytes = reader.read(length..end).await?.to_vec();
            if bytes.is_empty() {
                return Err(Self::mismatch(
                    storage_key,
                    expected_digest,
                    expected_length,
                ));
            }
            length = Self::next_length(length, bytes.len())?;
            if length > expected_length {
                return Err(Self::mismatch(
                    storage_key,
                    expected_digest,
                    expected_length,
                ));
            }
            hasher.update(&bytes);
            destination_file.write_all(&bytes).await?;
        }
        destination_file.flush().await?;

        let (actual_digest, actual_length) = Self::final_digest(hasher, length)?;
        if actual_digest != expected_digest || actual_length != expected_length {
            return Err(Self::mismatch(
                storage_key,
                expected_digest,
                expected_length,
            ));
        }
        Ok(())
    }
}

impl ArtifactStager for FilesystemArtifactStager {
    async fn stage_candidate_file(
        &self,
        lease: &StageLeaseV1,
        source: &Path,
    ) -> Result<CandidateArtifactV1, ArtifactStoreError> {
        lease.validate()?;
        let (content_digest, byte_length) =
            FilesystemArtifactStore::hash_source_file(source).await?;
        let storage_key = FilesystemArtifactStore::candidate_key(lease, content_digest);
        self.store
            .write_immutable_from_source(source, &storage_key, content_digest, byte_length)
            .await?;
        Ok(CandidateArtifactV1 {
            lease: lease.clone(),
            storage_key,
            content_digest,
            byte_length,
        })
    }

    async fn verify_candidate(
        &self,
        candidate: &CandidateArtifactV1,
    ) -> Result<(), ArtifactStoreError> {
        candidate.lease.validate()?;
        self.store
            .verify_object(
                candidate.storage_key(),
                candidate.content_digest(),
                candidate.byte_length(),
            )
            .await
    }

    async fn copy_candidate_to_file(
        &self,
        candidate: &CandidateArtifactV1,
        destination: &Path,
    ) -> Result<(), ArtifactStoreError> {
        let result = self
            .store
            .copy_object_to_file(
                candidate.storage_key(),
                destination,
                candidate.content_digest(),
                candidate.byte_length(),
            )
            .await;
        if let Err(error) = result {
            let _ = tokio::fs::remove_file(destination).await;
            return Err(error);
        }
        Ok(())
    }

    async fn discard_candidate(
        &self,
        candidate: &CandidateArtifactV1,
    ) -> Result<(), ArtifactStoreError> {
        self.store.delete_if_present(candidate.storage_key()).await
    }
}

impl ArtifactPromoter for FilesystemArtifactPromoter {
    async fn promote(
        &self,
        candidate: &CandidateArtifactV1,
    ) -> Result<AcceptedArtifactV1, ArtifactStoreError> {
        self.store
            .verify_object(
                candidate.storage_key(),
                candidate.content_digest(),
                candidate.byte_length(),
            )
            .await?;
        let storage_key = FilesystemArtifactStore::accepted_key(candidate.content_digest());
        self.store
            .write_immutable_from_object(
                candidate.storage_key(),
                &storage_key,
                candidate.content_digest(),
                candidate.byte_length(),
            )
            .await?;
        Ok(AcceptedArtifactV1 {
            storage_key,
            content_digest: candidate.content_digest(),
            byte_length: candidate.byte_length(),
        })
    }

    async fn verify_accepted(
        &self,
        digest: Sha256Digest,
    ) -> Result<AcceptedArtifactV1, ArtifactStoreError> {
        let storage_key = FilesystemArtifactStore::accepted_key(digest);
        let metadata = self.store.operator.stat(&storage_key).await?;
        let byte_length = metadata.content_length();
        self.store
            .verify_object(&storage_key, digest, byte_length)
            .await?;
        Ok(AcceptedArtifactV1 {
            storage_key,
            content_digest: digest,
            byte_length,
        })
    }

    async fn copy_accepted_to_file(
        &self,
        digest: Sha256Digest,
        destination: &Path,
    ) -> Result<AcceptedArtifactV1, ArtifactStoreError> {
        let storage_key = FilesystemArtifactStore::accepted_key(digest);
        let metadata = self.store.operator.stat(&storage_key).await?;
        let byte_length = metadata.content_length();
        let result = self
            .store
            .copy_object_to_file(&storage_key, destination, digest, byte_length)
            .await;
        if let Err(error) = result {
            let _ = tokio::fs::remove_file(destination).await;
            return Err(error);
        }
        Ok(AcceptedArtifactV1 {
            storage_key,
            content_digest: digest,
            byte_length,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        ArtifactPromoter, ArtifactStager, ArtifactStoreError, FilesystemArtifactPromoter,
        FilesystemArtifactStager, FilesystemArtifactStore, STREAM_CHUNK_BYTES,
    };
    use colibri_protocol::{
        SCHEMA_VERSION_V1,
        digest::Sha256Digest,
        ids::{AttemptId, PipelineId, StageId, WorkerId},
        lease::{FencingTokenV1, StageLeaseV1},
    };

    fn lease(attempt: &str, fence: u64) -> StageLeaseV1 {
        StageLeaseV1 {
            schema_version: SCHEMA_VERSION_V1,
            pipeline_id: PipelineId::new("pipeline-local").unwrap(),
            pipeline_spec_digest: Sha256Digest::of_bytes(b"pipeline"),
            stage_id: StageId::new("dataset-stage").unwrap(),
            attempt_id: AttemptId::new(attempt).unwrap(),
            worker_id: WorkerId::new("worker-local").unwrap(),
            template_digest: Sha256Digest::of_bytes(b"template"),
            input_manifest_digests: Vec::new(),
            issued_at_unix_ms: 1_000,
            expires_at_unix_ms: 61_000,
            fencing_token: FencingTokenV1::new(fence).unwrap(),
        }
    }

    fn store() -> (
        tempfile::TempDir,
        FilesystemArtifactStager,
        FilesystemArtifactPromoter,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let store = FilesystemArtifactStore::new(directory.path()).unwrap();
        let (stager, promoter) = store.into_capabilities();
        (directory, stager, promoter)
    }

    async fn source(directory: &tempfile::TempDir, name: &str, bytes: &[u8]) -> PathBuf {
        let path = directory.path().join(name);
        tokio::fs::write(&path, bytes).await.unwrap();
        path
    }

    #[tokio::test]
    async fn candidate_is_not_visible_until_it_is_promoted() {
        let (directory, stager, promoter) = store();
        let source = source(&directory, "dataset.bin", b"dataset-shard").await;
        let candidate = stager
            .stage_candidate_file(&lease("attempt-1", 1), &source)
            .await
            .unwrap();

        assert!(
            promoter
                .verify_accepted(candidate.content_digest())
                .await
                .is_err()
        );

        let accepted = promoter.promote(&candidate).await.unwrap();
        assert_eq!(accepted.content_digest(), candidate.content_digest());
        assert_eq!(accepted.byte_length(), 13);
        let destination = directory.path().join("accepted.bin");
        promoter
            .copy_accepted_to_file(candidate.content_digest(), &destination)
            .await
            .unwrap();
        assert_eq!(
            tokio::fs::read(destination).await.unwrap(),
            b"dataset-shard"
        );
    }

    #[tokio::test]
    async fn staging_and_promotion_are_idempotent_for_the_same_content() {
        let (directory, stager, promoter) = store();
        let source = source(&directory, "dataset.bin", b"same bytes").await;
        let lease = lease("attempt-1", 1);
        let first = stager.stage_candidate_file(&lease, &source).await.unwrap();
        let second = stager.stage_candidate_file(&lease, &source).await.unwrap();
        assert_eq!(first, second);

        let first_promotion = promoter.promote(&first).await.unwrap();
        let second_promotion = promoter.promote(&second).await.unwrap();
        assert_eq!(first_promotion, second_promotion);
    }

    #[tokio::test]
    async fn attempt_scoping_keeps_distinct_attempt_candidates_separate() {
        let (directory, stager, _promoter) = store();
        let source = source(&directory, "dataset.bin", b"same bytes").await;
        let first = stager
            .stage_candidate_file(&lease("attempt-1", 1), &source)
            .await
            .unwrap();
        let second = stager
            .stage_candidate_file(&lease("attempt-2", 2), &source)
            .await
            .unwrap();

        assert_ne!(first.storage_key(), second.storage_key());
        assert!(first.storage_key().starts_with("v1/candidates/id-"));
        assert!(!first.storage_key().contains(".."));
    }

    #[tokio::test]
    async fn candidate_and_accepted_keys_use_portable_segments() {
        let (directory, stager, _promoter) = store();
        let source = source(&directory, "dataset.bin", b"same bytes").await;
        let mut colon_lease = lease("attempt:1", 1);
        colon_lease.pipeline_id = PipelineId::new("pipeline:local").unwrap();
        colon_lease.stage_id = StageId::new("dataset:stage").unwrap();
        let candidate = stager
            .stage_candidate_file(&colon_lease, &source)
            .await
            .unwrap();
        let accepted_key = FilesystemArtifactStore::accepted_key(candidate.content_digest());

        for key in [candidate.storage_key(), accepted_key.as_str()] {
            assert!(key.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'/' | b'-')
            }));
            assert!(!key.contains(':'));
        }
    }

    #[tokio::test]
    async fn tampered_candidate_cannot_be_promoted() {
        let (directory, stager, promoter) = store();
        let source = source(&directory, "dataset.bin", b"original").await;
        let candidate = stager
            .stage_candidate_file(&lease("attempt-1", 1), &source)
            .await
            .unwrap();
        stager
            .store
            .operator
            .write(candidate.storage_key(), b"tampered".to_vec())
            .await
            .unwrap();

        assert!(matches!(
            promoter.promote(&candidate).await,
            Err(ArtifactStoreError::IntegrityMismatch { .. })
        ));
        assert!(
            promoter
                .verify_accepted(candidate.content_digest())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn a_conflicting_accepted_key_is_never_overwritten() {
        let (directory, stager, promoter) = store();
        let source = source(&directory, "dataset.bin", b"trusted bytes").await;
        let candidate = stager
            .stage_candidate_file(&lease("attempt-1", 1), &source)
            .await
            .unwrap();
        let accepted_key = FilesystemArtifactStore::accepted_key(candidate.content_digest());
        stager
            .store
            .operator
            .write(&accepted_key, b"forged bytes".to_vec())
            .await
            .unwrap();

        assert!(matches!(
            promoter.promote(&candidate).await,
            Err(ArtifactStoreError::IntegrityMismatch { .. })
        ));
        let forged = stager
            .store
            .operator
            .read(&accepted_key)
            .await
            .unwrap()
            .to_vec();
        assert_eq!(forged, b"forged bytes");
    }

    #[tokio::test]
    async fn discarding_a_candidate_is_idempotent_and_preserves_accepted_bytes() {
        let (directory, stager, promoter) = store();
        let source = source(&directory, "dataset.bin", b"checkpoint").await;
        let candidate = stager
            .stage_candidate_file(&lease("attempt-1", 1), &source)
            .await
            .unwrap();
        promoter.promote(&candidate).await.unwrap();

        stager.discard_candidate(&candidate).await.unwrap();
        stager.discard_candidate(&candidate).await.unwrap();
        assert!(stager.verify_candidate(&candidate).await.is_err());
        let destination = directory.path().join("checkpoint.bin");
        promoter
            .copy_accepted_to_file(candidate.content_digest(), &destination)
            .await
            .unwrap();
        assert_eq!(tokio::fs::read(destination).await.unwrap(), b"checkpoint");
    }

    #[tokio::test]
    async fn file_backed_transfer_crosses_stream_chunk_boundaries() {
        let (directory, stager, promoter) = store();
        let bytes = vec![0xA5; STREAM_CHUNK_BYTES * 3 + 17];
        let source = source(&directory, "large-dataset.bin", &bytes).await;
        let candidate = stager
            .stage_candidate_file(&lease("attempt-1", 1), &source)
            .await
            .unwrap();
        promoter.promote(&candidate).await.unwrap();

        let destination = directory.path().join("large-accepted.bin");
        let accepted = promoter
            .copy_accepted_to_file(candidate.content_digest(), &destination)
            .await
            .unwrap();
        assert_eq!(accepted.byte_length(), u64::try_from(bytes.len()).unwrap());
        assert_eq!(
            Sha256Digest::of_bytes(&tokio::fs::read(destination).await.unwrap()),
            candidate.content_digest()
        );
    }

    #[tokio::test]
    async fn invalid_leases_and_empty_artifacts_are_rejected_before_storage() {
        let (directory, stager, _promoter) = store();
        let empty = source(&directory, "empty.bin", b"").await;
        assert!(matches!(
            stager
                .stage_candidate_file(&lease("attempt-1", 1), &empty)
                .await,
            Err(ArtifactStoreError::EmptyArtifact)
        ));

        let source = source(&directory, "dataset.bin", b"data").await;
        let mut invalid_lease = lease("attempt-2", 2);
        invalid_lease.expires_at_unix_ms = invalid_lease.issued_at_unix_ms;
        assert!(matches!(
            stager.stage_candidate_file(&invalid_lease, &source).await,
            Err(ArtifactStoreError::Protocol(_))
        ));
    }
}
