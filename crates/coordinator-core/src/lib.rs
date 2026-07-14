//! Durable authority boundaries for the future coordinator service.
//!
//! This crate owns one single-writer V2 reservation journal. Its only public
//! mutation path accepts an Ed25519-signed sequencer event and, where required,
//! independently signed customer or rail evidence verified against a pinned
//! authority set. It is still not an HTTP
//! service, identity registry, multi-process lock, database, artifact-promotion
//! transaction, or settlement rail. A production coordinator must replace this
//! file with serializable database transactions, audit/outbox records and
//! reservation-scoped authority enforcement.

#![forbid(unsafe_code)]
#![allow(clippy::missing_errors_doc)]

use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use colibri_protocol::{
    ProtocolError, canonical,
    digest::Sha256Digest,
    ids::{KeyId, MessageId},
    settlement_v2::FundingAuthorizationV2,
    signing::{SignedEnvelopeV1, SignerKind, TrustedEd25519KeyV1},
};
use colibri_state_machine::{
    ApplyOutcome, EventEnvelope, TransitionError,
    settlement_v2::{SettlementEventV2, SettlementMachineV2},
};
use thiserror::Error;

const JOURNAL_MAGIC: [u8; 4] = *b"MCJ2";
const FRAME_HEADER_BYTES: usize = 12;
const FRAME_DIGEST_BYTES: usize = 32;
const MAX_EVENT_PAYLOAD_BYTES: u64 = 1_048_576;

/// Failure while authorizing, persisting, or reconstructing a V2 settlement stream.
#[derive(Debug, Error)]
pub enum SettlementSequencerError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    #[error(transparent)]
    Transition(#[from] TransitionError),
    #[error("configured sequencer key must have system signer kind")]
    InvalidSequencerKey,
    #[error("settlement event authority rejected: {0}")]
    Authority(String),
    #[error("settlement event admission time rejected: {0}")]
    Admission(String),
    #[error("journal record at byte {offset} has an invalid frame header")]
    InvalidFrameHeader { offset: u64 },
    #[error("journal record at byte {offset} exceeds the maximum event payload size")]
    OversizedRecord { offset: u64 },
    #[error("journal record at byte {offset} failed its integrity digest")]
    IntegrityMismatch { offset: u64 },
    #[error("journal record at byte {offset} is not valid signed settlement event JSON: {reason}")]
    InvalidEventJson { offset: u64, reason: String },
    #[error("journal record at byte {offset} duplicates or conflicts with a prior signed event")]
    DuplicateRecordedEvent { offset: u64 },
    #[error("signed event sequence {actual} is not the expected next sequence {expected}")]
    UnexpectedSequence { actual: u64, expected: u64 },
    #[error("signed event message id conflicts with a prior durable record")]
    SignedEventConflict,
    #[error("signed event sequence space is exhausted")]
    SequenceExhausted,
}

/// Trusted customer and rail keys pinned for one reservation stream.
///
/// The set is supplied from the identity and rail registries, never from an
/// event envelope. It intentionally retains historic rotated keys so journal
/// replay can verify the authority that existed when each event was issued.
#[derive(Debug, Clone, Default)]
pub struct SettlementAuthoritySet {
    customer_keys: BTreeMap<KeyId, TrustedEd25519KeyV1>,
    rail_keys: BTreeMap<KeyId, RailAuthorityKey>,
    key_material: BTreeMap<[u8; 32], KeyId>,
}

#[derive(Debug, Clone)]
struct RailAuthorityKey {
    rail_id: String,
    trusted_key: TrustedEd25519KeyV1,
}

impl SettlementAuthoritySet {
    /// Creates an empty, fail-closed authority set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a trusted customer signing key.
    pub fn register_customer_key(
        &mut self,
        trusted_key: TrustedEd25519KeyV1,
    ) -> Result<(), SettlementSequencerError> {
        if trusted_key.signer_kind != SignerKind::Customer {
            return Err(SettlementSequencerError::Authority(
                "registered customer key must have customer signer kind".into(),
            ));
        }
        let key_material = self.validate_new_authority_key(&trusted_key)?;
        self.customer_keys
            .insert(trusted_key.key_id.clone(), trusted_key.clone());
        self.key_material.insert(key_material, trusted_key.key_id);
        Ok(())
    }

    /// Adds a trusted system attestor key for one named settlement rail.
    pub fn register_rail_key(
        &mut self,
        rail_id: impl Into<String>,
        trusted_key: TrustedEd25519KeyV1,
    ) -> Result<(), SettlementSequencerError> {
        let rail_id = rail_id.into();
        if !is_valid_reference(&rail_id) {
            return Err(SettlementSequencerError::Authority(
                "registered rail id is invalid".into(),
            ));
        }
        if trusted_key.signer_kind != SignerKind::System {
            return Err(SettlementSequencerError::Authority(
                "registered rail key must have system signer kind".into(),
            ));
        }
        let key_material = self.validate_new_authority_key(&trusted_key)?;
        self.rail_keys.insert(
            trusted_key.key_id.clone(),
            RailAuthorityKey {
                rail_id,
                trusted_key: trusted_key.clone(),
            },
        );
        self.key_material.insert(key_material, trusted_key.key_id);
        Ok(())
    }

    fn customer_key(
        &self,
        key_id: &KeyId,
    ) -> Result<&TrustedEd25519KeyV1, SettlementSequencerError> {
        self.customer_keys.get(key_id).ok_or_else(|| {
            SettlementSequencerError::Authority(
                "customer authorization key is not pinned for this reservation".into(),
            )
        })
    }

    fn rail_key(
        &self,
        rail_id: &str,
        key_id: &KeyId,
    ) -> Result<&TrustedEd25519KeyV1, SettlementSequencerError> {
        let authority = self.rail_keys.get(key_id).ok_or_else(|| {
            SettlementSequencerError::Authority(
                "rail attestor key is not pinned for this reservation".into(),
            )
        })?;
        if authority.rail_id != rail_id {
            return Err(SettlementSequencerError::Authority(
                "rail attestor key is not registered for the attested rail".into(),
            ));
        }
        Ok(&authority.trusted_key)
    }

    fn contains_key(&self, key_id: &KeyId) -> bool {
        self.customer_keys.contains_key(key_id) || self.rail_keys.contains_key(key_id)
    }

    fn contains_key_material(&self, key_material: &[u8; 32]) -> bool {
        self.key_material.contains_key(key_material)
    }

    fn validate_new_authority_key(
        &self,
        trusted_key: &TrustedEd25519KeyV1,
    ) -> Result<[u8; 32], SettlementSequencerError> {
        if self.rail_keys.contains_key(&trusted_key.key_id)
            || self.customer_keys.contains_key(&trusted_key.key_id)
        {
            return Err(SettlementSequencerError::Authority(
                "authority key id is already registered for this reservation".into(),
            ));
        }
        let key_material = trusted_key.public_key_bytes().map_err(|_| {
            SettlementSequencerError::Authority(
                "registered authority key has invalid Ed25519 public-key material".into(),
            )
        })?;
        if self.contains_key_material(&key_material) {
            return Err(SettlementSequencerError::Authority(
                "authority public-key material is already registered for this reservation".into(),
            ));
        }
        Ok(key_material)
    }
}

/// The public authority boundary for one reservation-pinned V2 settlement stream.
///
/// All events are signed by one configured system sequencer. Funding
/// authorization also requires a customer key bound to the `CustomerId`; rail
/// finality requires a distinct system key bound to the attesting rail. The
/// durable journal is deliberately private so callers cannot append raw reducer
/// events.
pub struct SettlementSequencer {
    journal: SettlementJournal,
    sequencer_key: TrustedEd25519KeyV1,
    authorities: SettlementAuthoritySet,
}

impl SettlementSequencer {
    /// Opens and verifies one journal with its pinned sequencer and authority keys.
    pub fn open(
        path: impl AsRef<Path>,
        sequencer_key: TrustedEd25519KeyV1,
        authorities: SettlementAuthoritySet,
    ) -> Result<Self, SettlementSequencerError> {
        if sequencer_key.signer_kind != SignerKind::System {
            return Err(SettlementSequencerError::InvalidSequencerKey);
        }
        let sequencer_key_material = sequencer_key.public_key_bytes().map_err(|_| {
            SettlementSequencerError::Authority(
                "configured sequencer key has invalid Ed25519 public-key material".into(),
            )
        })?;
        if authorities.contains_key(&sequencer_key.key_id)
            || authorities.contains_key_material(&sequencer_key_material)
        {
            return Err(SettlementSequencerError::Authority(
                "sequencer public-key material must be distinct from every customer and rail authority key".into(),
            ));
        }
        let journal = SettlementJournal::open(path, &sequencer_key, &authorities)?;
        Ok(Self {
            journal,
            sequencer_key,
            authorities,
        })
    }

    /// Returns the journal path for operator diagnostics.
    #[must_use]
    pub fn path(&self) -> &Path {
        self.journal.path()
    }

    /// Returns state reconstructed from the signed durable event stream.
    #[must_use]
    pub const fn machine(&self) -> &SettlementMachineV2 {
        self.journal.machine()
    }

    /// Returns the sequence required for the next distinct signed event.
    pub fn next_sequence(&self) -> Result<u64, SettlementSequencerError> {
        next_sequence(self.journal.highest_sequencer_sequence)
    }

    /// Verifies all required authority, appends one signed event durably, and materializes it.
    ///
    /// An exact retry of a previously stored signed envelope returns
    /// [`ApplyOutcome::DuplicateIgnored`]. A caller may not append raw reducer
    /// events through this crate. `admitted_at_unix_ms` must come from the
    /// coordinator's admission clock; it is required to equal the signed outer
    /// event timestamp and every lifecycle observation timestamp.
    pub fn apply_signed(
        &mut self,
        envelope: &SignedEnvelopeV1<SettlementEventV2>,
        admitted_at_unix_ms: u64,
    ) -> Result<ApplyOutcome, SettlementSequencerError> {
        if let Some(outcome) = self.journal.existing_event_outcome(envelope)? {
            return Ok(outcome);
        }
        // The journal owns durable sequence replay protection. Passing one here
        // lets a fresh signed event enter the persistent sequence check below.
        envelope.verify_with_registry_key(&self.sequencer_key, admitted_at_unix_ms, 1)?;
        verify_event_admission_time(
            &envelope.payload,
            envelope.issued_at_unix_ms,
            admitted_at_unix_ms,
        )?;
        verify_event_authority(&envelope.payload, admitted_at_unix_ms, &self.authorities)?;
        self.journal.apply_verified(envelope)
    }
}

fn verify_event_admission_time(
    event: &SettlementEventV2,
    issued_at_unix_ms: u64,
    admitted_at_unix_ms: u64,
) -> Result<(), SettlementSequencerError> {
    if issued_at_unix_ms != admitted_at_unix_ms {
        return Err(SettlementSequencerError::Admission(
            "signed event issue time must equal the coordinator admission time".into(),
        ));
    }
    if event
        .observed_at_unix_ms()
        .is_some_and(|observed_at_unix_ms| observed_at_unix_ms != admitted_at_unix_ms)
    {
        return Err(SettlementSequencerError::Admission(
            "lifecycle observation time must equal the coordinator admission time".into(),
        ));
    }
    Ok(())
}

fn verify_event_authority(
    event: &SettlementEventV2,
    now_unix_ms: u64,
    authorities: &SettlementAuthoritySet,
) -> Result<(), SettlementSequencerError> {
    match event {
        SettlementEventV2::QuoteAuthorised { authorization, .. } => {
            let trusted_key = authorities.customer_key(&authorization.signature.key_id)?;
            FundingAuthorizationV2::verify_customer_envelope(
                authorization,
                trusted_key,
                now_unix_ms,
                1,
            )?;
        }
        SettlementEventV2::ChainConfirmationRecorded { finality }
        | SettlementEventV2::ChainFinalityReconciled { finality } => {
            verify_rail_finality(finality, now_unix_ms, authorities)?;
        }
        SettlementEventV2::ChainRetryAuthorised { reconciliation } => {
            verify_rail_finality(reconciliation, now_unix_ms, authorities)?;
        }
        _ => {}
    }
    Ok(())
}

fn verify_rail_finality(
    finality: &SignedEnvelopeV1<colibri_protocol::settlement_v2::RailFinalityAttestationV2>,
    now_unix_ms: u64,
    authorities: &SettlementAuthoritySet,
) -> Result<(), SettlementSequencerError> {
    let trusted_key =
        authorities.rail_key(&finality.payload.rail_id, &finality.signature.key_id)?;
    finality.verify_with_registry_key(trusted_key, now_unix_ms, 1)?;
    Ok(())
}

/// Durable storage is intentionally not public: only [`SettlementSequencer`]
/// may append after authority verification.
struct SettlementJournal {
    path: PathBuf,
    file: File,
    machine: SettlementMachineV2,
    highest_sequencer_sequence: u64,
    signed_event_digests: BTreeMap<MessageId, Sha256Digest>,
}

impl SettlementJournal {
    fn open(
        path: impl AsRef<Path>,
        sequencer_key: &TrustedEd25519KeyV1,
        authorities: &SettlementAuthoritySet,
    ) -> Result<Self, SettlementSequencerError> {
        let path = path.as_ref().to_owned();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&path)?;
        let mut journal = Self {
            path,
            file,
            machine: SettlementMachineV2::new(),
            highest_sequencer_sequence: 0,
            signed_event_digests: BTreeMap::new(),
        };
        journal.replay_and_recover(sequencer_key, authorities)?;
        Ok(journal)
    }

    fn path(&self) -> &Path {
        &self.path
    }

    const fn machine(&self) -> &SettlementMachineV2 {
        &self.machine
    }

    fn existing_event_outcome(
        &self,
        envelope: &SignedEnvelopeV1<SettlementEventV2>,
    ) -> Result<Option<ApplyOutcome>, SettlementSequencerError> {
        let signed_digest = canonical::digest(envelope)?;
        match self.signed_event_digests.get(&envelope.message_id) {
            Some(previous_digest) if *previous_digest == signed_digest => {
                Ok(Some(ApplyOutcome::DuplicateIgnored))
            }
            Some(_) => Err(SettlementSequencerError::SignedEventConflict),
            None => Ok(None),
        }
    }

    fn apply_verified(
        &mut self,
        envelope: &SignedEnvelopeV1<SettlementEventV2>,
    ) -> Result<ApplyOutcome, SettlementSequencerError> {
        let signed_digest = canonical::digest(envelope)?;
        if let Some(previous_digest) = self.signed_event_digests.get(&envelope.message_id) {
            return if *previous_digest == signed_digest {
                Ok(ApplyOutcome::DuplicateIgnored)
            } else {
                Err(SettlementSequencerError::SignedEventConflict)
            };
        }

        let expected_sequence = next_sequence(self.highest_sequencer_sequence)?;
        if envelope.sequence != expected_sequence {
            return Err(SettlementSequencerError::UnexpectedSequence {
                actual: envelope.sequence,
                expected: expected_sequence,
            });
        }
        let raw_event = EventEnvelope::new(envelope.message_id.clone(), envelope.payload.clone());
        let mut next_machine = self.machine.clone();
        let outcome = next_machine.apply_trusted(&raw_event)?;
        if outcome != ApplyOutcome::Applied {
            return Err(SettlementSequencerError::DuplicateRecordedEvent { offset: 0 });
        }
        self.append_frame(envelope)?;
        self.machine = next_machine;
        self.highest_sequencer_sequence = envelope.sequence;
        self.signed_event_digests
            .insert(envelope.message_id.clone(), signed_digest);
        Ok(outcome)
    }

    fn append_frame(
        &mut self,
        envelope: &SignedEnvelopeV1<SettlementEventV2>,
    ) -> Result<(), SettlementSequencerError> {
        let payload = canonical::to_vec(envelope)?;
        let payload_length = u64::try_from(payload.len())
            .map_err(|_| SettlementSequencerError::OversizedRecord { offset: 0 })?;
        if payload_length > MAX_EVENT_PAYLOAD_BYTES {
            return Err(SettlementSequencerError::OversizedRecord {
                offset: self.file.seek(SeekFrom::End(0))?,
            });
        }
        let digest = Sha256Digest::of_bytes(&payload);
        self.file.seek(SeekFrom::End(0))?;
        self.file.write_all(&JOURNAL_MAGIC)?;
        self.file.write_all(&payload_length.to_be_bytes())?;
        self.file.write_all(&payload)?;
        self.file.write_all(digest.as_bytes())?;
        self.file.sync_data()?;
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn replay_and_recover(
        &mut self,
        sequencer_key: &TrustedEd25519KeyV1,
        authorities: &SettlementAuthoritySet,
    ) -> Result<(), SettlementSequencerError> {
        self.file.seek(SeekFrom::Start(0))?;
        let mut rebuilt = SettlementMachineV2::new();
        let mut rebuilt_highest_sequence = 0_u64;
        let mut rebuilt_signed_event_digests = BTreeMap::new();
        let mut last_valid_offset = 0_u64;

        loop {
            let frame_offset = self.file.stream_position()?;
            let mut header = [0_u8; FRAME_HEADER_BYTES];
            match read_exact_or_end(&mut self.file, &mut header)? {
                FrameRead::End => break,
                FrameRead::Truncated => {
                    self.truncate_interrupted_tail(last_valid_offset)?;
                    break;
                }
                FrameRead::Complete => {}
            }
            if header[..4] != JOURNAL_MAGIC {
                return Err(SettlementSequencerError::InvalidFrameHeader {
                    offset: frame_offset,
                });
            }
            let payload_length = u64::from_be_bytes([
                header[4], header[5], header[6], header[7], header[8], header[9], header[10],
                header[11],
            ]);
            if payload_length > MAX_EVENT_PAYLOAD_BYTES {
                return Err(SettlementSequencerError::OversizedRecord {
                    offset: frame_offset,
                });
            }
            let payload_length = usize::try_from(payload_length).map_err(|_| {
                SettlementSequencerError::OversizedRecord {
                    offset: frame_offset,
                }
            })?;
            let mut payload = vec![0_u8; payload_length];
            if read_exact_or_end(&mut self.file, &mut payload)? != FrameRead::Complete {
                self.truncate_interrupted_tail(last_valid_offset)?;
                break;
            }
            let mut expected_digest = [0_u8; FRAME_DIGEST_BYTES];
            if read_exact_or_end(&mut self.file, &mut expected_digest)? != FrameRead::Complete {
                self.truncate_interrupted_tail(last_valid_offset)?;
                break;
            }
            if Sha256Digest::of_bytes(&payload).as_bytes() != &expected_digest {
                return Err(SettlementSequencerError::IntegrityMismatch {
                    offset: frame_offset,
                });
            }
            let envelope = serde_json::from_slice::<SignedEnvelopeV1<SettlementEventV2>>(&payload)
                .map_err(|error| SettlementSequencerError::InvalidEventJson {
                    offset: frame_offset,
                    reason: error.to_string(),
                })?;
            let expected_sequence = next_sequence(rebuilt_highest_sequence)?;
            if envelope.sequence != expected_sequence {
                return Err(SettlementSequencerError::UnexpectedSequence {
                    actual: envelope.sequence,
                    expected: expected_sequence,
                });
            }
            // A historical record is verified at its own issuance time. This
            // permits a retired registry key to authenticate an otherwise valid
            // historic event while still validating its key validity interval.
            envelope.verify_with_registry_key(
                sequencer_key,
                envelope.issued_at_unix_ms,
                expected_sequence,
            )?;
            verify_event_admission_time(
                &envelope.payload,
                envelope.issued_at_unix_ms,
                envelope.issued_at_unix_ms,
            )?;
            verify_event_authority(&envelope.payload, envelope.issued_at_unix_ms, authorities)?;
            let signed_digest = canonical::digest(&envelope)?;
            if rebuilt_signed_event_digests
                .insert(envelope.message_id.clone(), signed_digest)
                .is_some()
            {
                return Err(SettlementSequencerError::DuplicateRecordedEvent {
                    offset: frame_offset,
                });
            }
            let raw_event = EventEnvelope::new(envelope.message_id.clone(), envelope.payload);
            if rebuilt.apply_trusted(&raw_event)? != ApplyOutcome::Applied {
                return Err(SettlementSequencerError::DuplicateRecordedEvent {
                    offset: frame_offset,
                });
            }
            rebuilt_highest_sequence = envelope.sequence;
            last_valid_offset = self.file.stream_position()?;
        }

        self.machine = rebuilt;
        self.highest_sequencer_sequence = rebuilt_highest_sequence;
        self.signed_event_digests = rebuilt_signed_event_digests;
        self.file.seek(SeekFrom::End(0))?;
        Ok(())
    }

    fn truncate_interrupted_tail(
        &mut self,
        last_valid_offset: u64,
    ) -> Result<(), SettlementSequencerError> {
        self.file.set_len(last_valid_offset)?;
        self.file.sync_data()?;
        self.file.seek(SeekFrom::End(0))?;
        Ok(())
    }
}

fn next_sequence(highest_sequence: u64) -> Result<u64, SettlementSequencerError> {
    highest_sequence
        .checked_add(1)
        .ok_or(SettlementSequencerError::SequenceExhausted)
}

fn is_valid_reference(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrameRead {
    Complete,
    End,
    Truncated,
}

fn read_exact_or_end(file: &mut File, buffer: &mut [u8]) -> Result<FrameRead, std::io::Error> {
    let mut filled = 0_usize;
    while filled < buffer.len() {
        let read = file.read(&mut buffer[filled..])?;
        if read == 0 {
            return Ok(if filled == 0 {
                FrameRead::End
            } else {
                FrameRead::Truncated
            });
        }
        filled = filled
            .checked_add(read)
            .expect("bytes read cannot exceed the supplied buffer length");
    }
    Ok(FrameRead::Complete)
}

#[cfg(test)]
mod tests {
    use std::{
        fs::OpenOptions,
        io::Write as _,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
    use colibri_protocol::{
        SCHEMA_VERSION_V1, SCHEMA_VERSION_V2,
        digest::Sha256Digest,
        ids::{
            ActorId, ArtifactId, AttemptId, ChainReceiptId, CustomerId, FundingAuthorizationId,
            KeyId, MessageId, PipelineId, ProviderId, QuoteId, ReservationId, SettlementId,
            SettlementNonceId, StageId,
        },
        settlement_v2::{
            AcceptedArtifactV2, AtomicAmount, ChainFeeReceiptV2, FundingAuthorizationV2,
            PayoutRecipientV2, QuoteV2, RailFinalityAttestationV2, RailFinalityOutcomeV2,
            ReservationV2, SettlementAssetV2, SettlementV2,
        },
        signing::{
            Ed25519SignatureV1, SignedContract, SignedEnvelopeV1, SignerKind, TrustedEd25519KeyV1,
        },
    };
    use colibri_state_machine::{
        ApplyOutcome,
        settlement_v2::{SettlementEventV2, SettlementPhaseV2},
    };
    use ed25519_dalek::{Signer as _, SigningKey};

    use super::{SettlementAuthoritySet, SettlementSequencer, SettlementSequencerError};

    const NOW: u64 = 1_700;

    struct TestKeys {
        sequencer_signing: SigningKey,
        sequencer_registry: TrustedEd25519KeyV1,
        customer_signing: SigningKey,
        customer_registry: TrustedEd25519KeyV1,
        rail_signing: SigningKey,
        rail_registry: TrustedEd25519KeyV1,
        authorities: SettlementAuthoritySet,
    }

    fn journal_path(test_name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir()
            .join(format!(
                "mojocolib-coordinator-{test_name}-{}-{nonce}",
                std::process::id()
            ))
            .join("settlement.journal")
    }

    fn trusted_key(
        seed: u8,
        key_id: &str,
        signer_kind: SignerKind,
        signer_id: &str,
    ) -> (SigningKey, TrustedEd25519KeyV1) {
        let signing = SigningKey::from_bytes(&[seed; 32]);
        let registry = TrustedEd25519KeyV1 {
            schema_version: SCHEMA_VERSION_V1,
            key_id: KeyId::new(key_id).unwrap(),
            signer_kind,
            signer_id: ActorId::new(signer_id).unwrap(),
            public_key_base64: STANDARD_NO_PAD.encode(signing.verifying_key().as_bytes()),
            valid_from_unix_ms: 1_000,
            expires_at_unix_ms: 3_000,
        };
        (signing, registry)
    }

    fn keys() -> TestKeys {
        let (sequencer_signing, sequencer_registry) =
            trusted_key(7, "sequencer-key-1", SignerKind::System, "sequencer-1");
        let (customer_signing, customer_registry) =
            trusted_key(8, "customer-key-1", SignerKind::Customer, "customer-1");
        let (rail_signing, rail_registry) =
            trusted_key(9, "rail-key-1", SignerKind::System, "rail-attestor-1");
        let mut authorities = SettlementAuthoritySet::new();
        authorities
            .register_customer_key(customer_registry.clone())
            .unwrap();
        authorities
            .register_rail_key("test:rail", rail_registry.clone())
            .unwrap();
        TestKeys {
            sequencer_signing,
            sequencer_registry,
            customer_signing,
            customer_registry,
            rail_signing,
            rail_registry,
            authorities,
        }
    }

    fn sign<T: SignedContract>(
        signing_key: &SigningKey,
        registry_key: &TrustedEd25519KeyV1,
        message_id: &str,
        sequence: u64,
        issued_at_unix_ms: u64,
        payload: T,
    ) -> SignedEnvelopeV1<T> {
        let mut envelope = SignedEnvelopeV1 {
            schema_version: SCHEMA_VERSION_V1,
            contract_type: T::CONTRACT_TYPE.into(),
            message_id: MessageId::new(message_id).unwrap(),
            sequence,
            issued_at_unix_ms,
            expires_at_unix_ms: Some(2_500),
            payload,
            signature: Ed25519SignatureV1 {
                key_id: registry_key.key_id.clone(),
                signer_kind: registry_key.signer_kind,
                signer_id: registry_key.signer_id.clone(),
                signature_base64: String::new(),
            },
        };
        let signature = signing_key.sign(&envelope.signing_bytes().unwrap());
        envelope.signature.signature_base64 = STANDARD_NO_PAD.encode(signature.to_bytes());
        envelope
    }

    fn recipient(actor: &str, destination: &str) -> PayoutRecipientV2 {
        PayoutRecipientV2 {
            actor_id: ActorId::new(actor).unwrap(),
            destination_ref: destination.into(),
        }
    }

    fn quote() -> QuoteV2 {
        QuoteV2 {
            schema_version: SCHEMA_VERSION_V2,
            quote_id: QuoteId::new("quote-v2-1").unwrap(),
            pipeline_id: PipelineId::new("pipeline-1").unwrap(),
            stage_id: StageId::new("stage-1").unwrap(),
            provider_id: ProviderId::new("provider-1").unwrap(),
            asset: SettlementAssetV2 {
                asset_id: "test:credits".into(),
                decimals: 6,
                accounting_unit: "usd-micro".into(),
                conversion_source: "fixed:pilot-v1".into(),
                conversion_observed_at_unix_ms: 900,
            },
            accepted_execution_cap_atomic: AtomicAmount::new(1_000),
            verification_reserve_cap_atomic: AtomicAmount::new(200),
            chain_fee_reserve_cap_atomic: AtomicAmount::new(50),
            provider_payout_cap_atomic: AtomicAmount::new(950),
            verifier_payout_cap_atomic: AtomicAmount::new(150),
            operator_fee_bps: 500,
            operator_fee_cap_atomic: AtomicAmount::new(60),
            provider_recipient: recipient("provider-actor", "wallet:provider-1"),
            verifier_recipient: recipient("verifier-actor", "wallet:verifier-1"),
            operator_recipient: recipient("operator-actor", "wallet:operator-1"),
            chain_fee_payer: recipient("operator-actor", "wallet:operator-1"),
            pricing_policy_version: 1,
            expires_at_unix_ms: 2_000,
        }
    }

    fn authorization(quote: &QuoteV2) -> FundingAuthorizationV2 {
        FundingAuthorizationV2 {
            schema_version: SCHEMA_VERSION_V2,
            authorization_id: FundingAuthorizationId::new("authorization-1").unwrap(),
            customer_id: CustomerId::new("customer-1").unwrap(),
            reservation_id: ReservationId::new("reservation-v2-1").unwrap(),
            quote_id: quote.quote_id.clone(),
            quote_digest: quote.canonical_digest().unwrap(),
            funding_account_ref: "wallet:customer-1".into(),
            funded_reservation_atomic: quote.funded_reservation_atomic().unwrap(),
            issued_at_unix_ms: 1_000,
            expires_at_unix_ms: 1_900,
        }
    }

    fn reservation(quote: &QuoteV2, authorization: &FundingAuthorizationV2) -> ReservationV2 {
        ReservationV2 {
            schema_version: SCHEMA_VERSION_V2,
            reservation_id: authorization.reservation_id.clone(),
            customer_id: authorization.customer_id.clone(),
            quote_id: quote.quote_id.clone(),
            quote_digest: quote.canonical_digest().unwrap(),
            authorization_id: authorization.authorization_id.clone(),
            funded_reservation_atomic: quote.funded_reservation_atomic().unwrap(),
            funded_at_unix_ms: 1_100,
            expires_at_unix_ms: 1_800,
        }
    }

    fn artifact(quote: &QuoteV2, reservation: &ReservationV2) -> AcceptedArtifactV2 {
        AcceptedArtifactV2 {
            schema_version: SCHEMA_VERSION_V2,
            reservation_id: reservation.reservation_id.clone(),
            quote_id: quote.quote_id.clone(),
            quote_digest: quote.canonical_digest().unwrap(),
            pipeline_id: quote.pipeline_id.clone(),
            stage_id: quote.stage_id.clone(),
            attempt_id: AttemptId::new("attempt-1").unwrap(),
            artifact_id: ArtifactId::new("artifact-1").unwrap(),
            artifact_manifest_digest: Sha256Digest::of_bytes(b"manifest"),
            provider_id: quote.provider_id.clone(),
            verification_decision_digest: Sha256Digest::of_bytes(b"verification"),
        }
    }

    fn settlement(
        quote: &QuoteV2,
        reservation: &ReservationV2,
        artifact: &AcceptedArtifactV2,
    ) -> SettlementV2 {
        SettlementV2 {
            schema_version: SCHEMA_VERSION_V2,
            settlement_id: SettlementId::new("settlement-v2-1").unwrap(),
            settlement_nonce: SettlementNonceId::new("settlement-nonce-1").unwrap(),
            rail_id: "test:rail".into(),
            reservation_id: reservation.reservation_id.clone(),
            quote_id: quote.quote_id.clone(),
            quote_digest: quote.canonical_digest().unwrap(),
            pipeline_id: artifact.pipeline_id.clone(),
            stage_id: artifact.stage_id.clone(),
            attempt_id: artifact.attempt_id.clone(),
            artifact_id: artifact.artifact_id.clone(),
            artifact_manifest_digest: artifact.artifact_manifest_digest,
            provider_id: artifact.provider_id.clone(),
            verification_decision_digest: artifact.verification_decision_digest,
            accepted_execution_component_atomic: AtomicAmount::new(1_000),
            provider_payout_atomic: AtomicAmount::new(950),
            verifier_payout_atomic: AtomicAmount::new(100),
            operator_fee_atomic: AtomicAmount::new(50),
            chain_fee_reimbursement_atomic: AtomicAmount::new(20),
            chain_fee_receipt: Some(ChainFeeReceiptV2 {
                schema_version: SCHEMA_VERSION_V2,
                receipt_id: ChainReceiptId::new("receipt-1").unwrap(),
                rail_id: "test:rail".into(),
                asset_id: quote.asset.asset_id.clone(),
                reservation_id: reservation.reservation_id.clone(),
                quote_id: quote.quote_id.clone(),
                settlement_id: SettlementId::new("settlement-v2-1").unwrap(),
                settlement_nonce: SettlementNonceId::new("settlement-nonce-1").unwrap(),
                payer: quote.chain_fee_payer.clone(),
                transaction_reference: "solana:transaction-1".into(),
                paid_atomic: AtomicAmount::new(20),
                observed_at_unix_ms: 1_500,
            }),
        }
    }

    fn signed_finality(
        keys: &TestKeys,
        settlement: &SettlementV2,
        transaction_reference: &str,
        outcome: RailFinalityOutcomeV2,
    ) -> SignedEnvelopeV1<RailFinalityAttestationV2> {
        sign(
            &keys.rail_signing,
            &keys.rail_registry,
            "rail-attestation-1",
            1,
            NOW,
            RailFinalityAttestationV2 {
                schema_version: SCHEMA_VERSION_V2,
                rail_id: settlement.rail_id.clone(),
                settlement_id: settlement.settlement_id.clone(),
                settlement_nonce: settlement.settlement_nonce.clone(),
                settlement_digest: settlement.canonical_digest().unwrap(),
                transaction_reference: transaction_reference.into(),
                outcome,
                rail_proof_digest: Sha256Digest::of_bytes(b"rail-proof"),
                observed_at_unix_ms: NOW,
            },
        )
    }

    fn signed_event(
        keys: &TestKeys,
        message_id: &str,
        sequence: u64,
        payload: SettlementEventV2,
    ) -> SignedEnvelopeV1<SettlementEventV2> {
        sign(
            &keys.sequencer_signing,
            &keys.sequencer_registry,
            message_id,
            sequence,
            NOW,
            payload,
        )
    }

    fn advance_to_intent(sequencer: &mut SettlementSequencer, keys: &TestKeys) -> SettlementV2 {
        let quote = quote();
        let authorization = authorization(&quote);
        let reservation = reservation(&quote, &authorization);
        let artifact = artifact(&quote, &reservation);
        let settlement = settlement(&quote, &reservation, &artifact);
        let customer_authorization = sign(
            &keys.customer_signing,
            &keys.customer_registry,
            "customer-authorization-1",
            1,
            1_000,
            authorization,
        );
        let event = signed_event(
            keys,
            "event-1",
            1,
            SettlementEventV2::QuoteAuthorised {
                quote,
                authorization: customer_authorization,
                observed_at_unix_ms: NOW,
            },
        );
        assert_eq!(
            sequencer.apply_signed(&event, NOW).unwrap(),
            ApplyOutcome::Applied
        );
        for (sequence, message_id, event) in [
            (
                2,
                "event-2",
                SettlementEventV2::ReservationCreated {
                    reservation,
                    observed_at_unix_ms: NOW,
                },
            ),
            (
                3,
                "event-3",
                SettlementEventV2::WorkAccepted {
                    accepted_artifact: artifact,
                    observed_at_unix_ms: NOW,
                },
            ),
            (
                4,
                "event-4",
                SettlementEventV2::SettlementIntentCreated {
                    settlement: settlement.clone(),
                    observed_at_unix_ms: NOW,
                },
            ),
        ] {
            let event = signed_event(keys, message_id, sequence, event);
            assert_eq!(
                sequencer.apply_signed(&event, NOW).unwrap(),
                ApplyOutcome::Applied
            );
        }
        settlement
    }

    fn cleanup(path: &Path) {
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn signed_customer_and_rail_evidence_survive_restart() {
        let path = journal_path("signed-restart");
        let keys = keys();
        let mut sequencer = SettlementSequencer::open(
            &path,
            keys.sequencer_registry.clone(),
            keys.authorities.clone(),
        )
        .unwrap();
        let settlement = advance_to_intent(&mut sequencer, &keys);
        let submitted = signed_event(
            &keys,
            "event-5",
            5,
            SettlementEventV2::ChainSubmitted {
                transaction_reference: "solana:transaction-1".into(),
            },
        );
        assert_eq!(
            sequencer.apply_signed(&submitted, NOW).unwrap(),
            ApplyOutcome::Applied
        );
        assert_eq!(
            sequencer.apply_signed(&submitted, NOW + 1).unwrap(),
            ApplyOutcome::DuplicateIgnored
        );
        let confirmation = signed_event(
            &keys,
            "event-6",
            6,
            SettlementEventV2::ChainConfirmationRecorded {
                finality: signed_finality(
                    &keys,
                    &settlement,
                    "solana:transaction-1",
                    RailFinalityOutcomeV2::Final,
                ),
            },
        );
        sequencer.apply_signed(&confirmation, NOW).unwrap();
        let journal_length = std::fs::metadata(&path).unwrap().len();
        drop(sequencer);

        let mut without_rail = SettlementAuthoritySet::new();
        without_rail
            .register_customer_key(keys.customer_registry.clone())
            .unwrap();
        assert!(matches!(
            SettlementSequencer::open(&path, keys.sequencer_registry.clone(), without_rail),
            Err(SettlementSequencerError::Authority(_))
        ));

        let reopened = SettlementSequencer::open(
            &path,
            keys.sequencer_registry.clone(),
            keys.authorities.clone(),
        )
        .unwrap();
        assert_eq!(
            reopened.machine().phase(),
            SettlementPhaseV2::FinancialFinal
        );
        assert_eq!(reopened.next_sequence().unwrap(), 7);
        assert_eq!(std::fs::metadata(&path).unwrap().len(), journal_length);
        drop(reopened);
        cleanup(&path);
    }

    #[test]
    fn pinned_customer_and_rail_authorities_are_required_and_distinct() {
        let path = journal_path("authority-missing-customer");
        let keys = keys();
        let mut sequencer = SettlementSequencer::open(
            &path,
            keys.sequencer_registry.clone(),
            SettlementAuthoritySet::new(),
        )
        .unwrap();
        let quote = quote();
        let authorization = authorization(&quote);
        let customer_authorization = sign(
            &keys.customer_signing,
            &keys.customer_registry,
            "customer-authorization-1",
            1,
            1_000,
            authorization,
        );
        let quote_event = signed_event(
            &keys,
            "event-1",
            1,
            SettlementEventV2::QuoteAuthorised {
                quote,
                authorization: customer_authorization,
                observed_at_unix_ms: NOW,
            },
        );
        assert!(matches!(
            sequencer.apply_signed(&quote_event, NOW),
            Err(SettlementSequencerError::Authority(_))
        ));
        assert_eq!(sequencer.machine().phase(), SettlementPhaseV2::Draft);
        drop(sequencer);
        cleanup(&path);

        let path = journal_path("authority-missing-rail");
        let mut customer_only = SettlementAuthoritySet::new();
        customer_only
            .register_customer_key(keys.customer_registry.clone())
            .unwrap();
        let mut sequencer =
            SettlementSequencer::open(&path, keys.sequencer_registry.clone(), customer_only)
                .unwrap();
        let settlement = advance_to_intent(&mut sequencer, &keys);
        let submitted = signed_event(
            &keys,
            "event-5",
            5,
            SettlementEventV2::ChainSubmitted {
                transaction_reference: "solana:transaction-1".into(),
            },
        );
        sequencer.apply_signed(&submitted, NOW).unwrap();
        let confirmation = signed_event(
            &keys,
            "event-6",
            6,
            SettlementEventV2::ChainConfirmationRecorded {
                finality: signed_finality(
                    &keys,
                    &settlement,
                    "solana:transaction-1",
                    RailFinalityOutcomeV2::Final,
                ),
            },
        );
        assert!(matches!(
            sequencer.apply_signed(&confirmation, NOW),
            Err(SettlementSequencerError::Authority(_))
        ));
        assert_eq!(sequencer.machine().phase(), SettlementPhaseV2::ChainPending);
        drop(sequencer);
        cleanup(&path);

        let mut invalid_authorities = SettlementAuthoritySet::new();
        invalid_authorities
            .register_rail_key("test:rail", keys.sequencer_registry.clone())
            .unwrap();
        assert!(matches!(
            SettlementSequencer::open(
                journal_path("authority-key-reuse"),
                keys.sequencer_registry.clone(),
                invalid_authorities,
            ),
            Err(SettlementSequencerError::Authority(_))
        ));

        let mut sequencer_key_alias = keys.sequencer_registry.clone();
        sequencer_key_alias.key_id = KeyId::new("sequencer-key-alias").unwrap();
        sequencer_key_alias.signer_id = ActorId::new("rail-attestor-alias").unwrap();
        let mut aliased_authorities = SettlementAuthoritySet::new();
        aliased_authorities
            .register_rail_key("test:rail", sequencer_key_alias)
            .unwrap();
        assert!(matches!(
            SettlementSequencer::open(
                journal_path("authority-public-key-reuse"),
                keys.sequencer_registry.clone(),
                aliased_authorities,
            ),
            Err(SettlementSequencerError::Authority(_))
        ));
    }

    #[test]
    fn coordinator_admission_time_cannot_be_backdated_or_split_from_lifecycle_time() {
        let path = journal_path("admission-time");
        let keys = keys();
        let mut sequencer = SettlementSequencer::open(
            &path,
            keys.sequencer_registry.clone(),
            keys.authorities.clone(),
        )
        .unwrap();
        let quote = quote();
        let customer_authorization = sign(
            &keys.customer_signing,
            &keys.customer_registry,
            "customer-authorization-1",
            1,
            1_000,
            authorization(&quote),
        );
        let observation_backdated = signed_event(
            &keys,
            "event-1",
            1,
            SettlementEventV2::QuoteAuthorised {
                quote: quote.clone(),
                authorization: customer_authorization.clone(),
                observed_at_unix_ms: NOW - 1,
            },
        );
        assert!(matches!(
            sequencer.apply_signed(&observation_backdated, NOW),
            Err(SettlementSequencerError::Admission(_))
        ));

        let issue_time_backdated = sign(
            &keys.sequencer_signing,
            &keys.sequencer_registry,
            "event-2",
            1,
            NOW - 1,
            SettlementEventV2::QuoteAuthorised {
                quote,
                authorization: customer_authorization,
                observed_at_unix_ms: NOW,
            },
        );
        assert!(matches!(
            sequencer.apply_signed(&issue_time_backdated, NOW),
            Err(SettlementSequencerError::Admission(_))
        ));
        assert_eq!(sequencer.machine().phase(), SettlementPhaseV2::Draft);
        drop(sequencer);
        cleanup(&path);
    }

    #[test]
    fn restart_discards_only_an_interrupted_trailing_frame() {
        let path = journal_path("tail-recovery");
        let keys = keys();
        let mut sequencer = SettlementSequencer::open(
            &path,
            keys.sequencer_registry.clone(),
            keys.authorities.clone(),
        )
        .unwrap();
        advance_to_intent(&mut sequencer, &keys);
        let valid_length = std::fs::metadata(&path).unwrap().len();
        drop(sequencer);

        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(b"MCJ2\x00\x00\x00").unwrap();
        file.sync_data().unwrap();
        drop(file);

        let reopened = SettlementSequencer::open(
            &path,
            keys.sequencer_registry.clone(),
            keys.authorities.clone(),
        )
        .unwrap();
        assert_eq!(
            reopened.machine().phase(),
            SettlementPhaseV2::SettlementIntent
        );
        assert_eq!(std::fs::metadata(&path).unwrap().len(), valid_length);
        drop(reopened);
        cleanup(&path);
    }

    #[test]
    fn a_corrupt_complete_frame_fails_closed_without_truncating_history() {
        let path = journal_path("corrupt-frame");
        let keys = keys();
        let mut sequencer = SettlementSequencer::open(
            &path,
            keys.sequencer_registry.clone(),
            keys.authorities.clone(),
        )
        .unwrap();
        advance_to_intent(&mut sequencer, &keys);
        drop(sequencer);
        let length_before = std::fs::metadata(&path).unwrap().len();

        let mut bytes = std::fs::read(&path).unwrap();
        let last = bytes.last_mut().unwrap();
        *last ^= 0xFF;
        std::fs::write(&path, bytes).unwrap();

        assert!(matches!(
            SettlementSequencer::open(
                &path,
                keys.sequencer_registry.clone(),
                keys.authorities.clone(),
            ),
            Err(SettlementSequencerError::IntegrityMismatch { .. })
        ));
        assert_eq!(std::fs::metadata(&path).unwrap().len(), length_before);
        cleanup(&path);
    }
}
