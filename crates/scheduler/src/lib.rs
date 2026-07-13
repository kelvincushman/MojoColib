//! Compatibility-first, deterministic scheduling without transport or storage concerns.

#![forbid(unsafe_code)]
#![allow(clippy::missing_errors_doc)]

use colibri_protocol::{
    ProtocolError, SCHEMA_VERSION_V1,
    ids::{PipelineId, ProviderId, StageId, WorkerId},
    ledger::QuoteV1,
    pipeline::{ResourceRequirementsV1, StageSpecV1},
    provider::{AcceleratorCapabilityV1, WorkerCapabilityV1},
    template::TemplateManifestV1,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The frozen hard-filter policy used to explain and replay an assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchedulerPolicyV1 {
    pub schema_version: u16,
    pub policy_version: u32,
    pub minimum_reliability_basis_points: u16,
}

impl SchedulerPolicyV1 {
    fn validate(self) -> Result<(), SchedulerError> {
        if self.schema_version != SCHEMA_VERSION_V1
            || self.policy_version == 0
            || self.minimum_reliability_basis_points > 10_000
        {
            return Err(SchedulerError::InvalidPolicy);
        }
        Ok(())
    }
}

/// All signed facts needed to consider one worker for a stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateV1 {
    pub capability: WorkerCapabilityV1,
    pub quote: QuoteV1,
    pub reliability_basis_points: u16,
}

/// The deterministic output recorded with a lease decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssignmentV1 {
    pub scheduler_policy_version: u32,
    pub pipeline_id: PipelineId,
    pub stage_id: StageId,
    pub provider_id: ProviderId,
    pub worker_id: WorkerId,
    pub selected_backend: String,
}

#[derive(Debug, Error)]
pub enum SchedulerError {
    #[error("invalid scheduler policy")]
    InvalidPolicy,
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
}

/// Selects the first eligible worker under a stable provider/stage/worker order.
///
/// Cost scoring is intentionally absent in V1. Budget is a hard filter, and all
/// remaining candidates are equivalent until a later versioned scoring policy
/// is introduced.
pub fn select_worker(
    pipeline_id: &PipelineId,
    stage: &StageSpecV1,
    template: &TemplateManifestV1,
    candidates: &[CandidateV1],
    policy: SchedulerPolicyV1,
    now_unix_ms: u64,
) -> Result<Option<AssignmentV1>, SchedulerError> {
    policy.validate()?;
    stage.validate()?;
    template.validate()?;
    if !stage_matches_template(stage, template) {
        return Ok(None);
    }

    let mut eligible = candidates
        .iter()
        .filter_map(|candidate| {
            eligible_backend(candidate, pipeline_id, stage, template, policy, now_unix_ms)
                .map(|backend| (candidate, backend))
        })
        .collect::<Vec<_>>();
    eligible.sort_by(|(left, _), (right, _)| {
        (
            &left.capability.provider_id,
            &stage.stage_id,
            &left.capability.worker_id,
        )
            .cmp(&(
                &right.capability.provider_id,
                &stage.stage_id,
                &right.capability.worker_id,
            ))
    });

    Ok(eligible.first().map(|(candidate, backend)| AssignmentV1 {
        scheduler_policy_version: policy.policy_version,
        pipeline_id: pipeline_id.clone(),
        stage_id: stage.stage_id.clone(),
        provider_id: candidate.capability.provider_id.clone(),
        worker_id: candidate.capability.worker_id.clone(),
        selected_backend: backend.clone(),
    }))
}

fn stage_matches_template(stage: &StageSpecV1, template: &TemplateManifestV1) -> bool {
    stage.template_id == template.template_id
        && stage.template_digest == template.bundle_digest
        && stage.runtime == template.runtime
        && stage.verification_policy == template.verification_policy
        && stage
            .expected_outputs
            .iter()
            .all(|output| template.output_types.contains(output))
        && resources_within(&stage.resources, &template.maximum_resources)
}

fn eligible_backend(
    candidate: &CandidateV1,
    pipeline_id: &PipelineId,
    stage: &StageSpecV1,
    template: &TemplateManifestV1,
    policy: SchedulerPolicyV1,
    now_unix_ms: u64,
) -> Option<String> {
    let capability = &candidate.capability;
    if capability.validate(now_unix_ms).is_err()
        || candidate.quote.validate(now_unix_ms).is_err()
        || !capability.available
        || candidate.reliability_basis_points < policy.minimum_reliability_basis_points
        || candidate.reliability_basis_points > 10_000
        || candidate.quote.policy_version != policy.policy_version
        || &candidate.quote.pipeline_id != pipeline_id
        || candidate.quote.provider_id != capability.provider_id
        || candidate.quote.stage_id != stage.stage_id
        || !candidate
            .quote
            .maximum_reservation()
            .is_ok_and(|amount| amount <= stage.quote_cap)
        || capability.cpu_cores < stage.resources.cpu_cores
        || capability.ram_bytes < stage.resources.ram_bytes
        || capability.available_storage_bytes < stage.resources.scratch_bytes
        || !template
            .supported_operating_systems
            .contains(&capability.operating_system)
        || !template
            .supported_architectures
            .contains(&capability.architecture)
    {
        return None;
    }

    if stage.resources.accelerator_memory_bytes == 0 {
        return template
            .supported_backends
            .iter()
            .find(|backend| backend.as_str() == "cpu")
            .cloned();
    }

    let mut accelerators = capability
        .accelerators
        .iter()
        .filter(|accelerator| accelerator_matches(accelerator, &stage.resources, template))
        .collect::<Vec<_>>();
    accelerators.sort_by(|left, right| {
        (&left.backend, &left.model, &left.driver_version).cmp(&(
            &right.backend,
            &right.model,
            &right.driver_version,
        ))
    });
    accelerators
        .first()
        .map(|accelerator| accelerator.backend.clone())
}

fn accelerator_matches(
    accelerator: &AcceleratorCapabilityV1,
    resources: &ResourceRequirementsV1,
    template: &TemplateManifestV1,
) -> bool {
    accelerator.memory_bytes >= resources.accelerator_memory_bytes
        && template.supported_backends.contains(&accelerator.backend)
        && resources
            .required_precisions
            .iter()
            .all(|precision| accelerator.supported_precisions.contains(precision))
}

const fn resources_within(
    requested: &ResourceRequirementsV1,
    maximum: &ResourceRequirementsV1,
) -> bool {
    requested.cpu_cores <= maximum.cpu_cores
        && requested.ram_bytes <= maximum.ram_bytes
        && requested.accelerator_memory_bytes <= maximum.accelerator_memory_bytes
        && requested.scratch_bytes <= maximum.scratch_bytes
        && requested.wall_time_seconds <= maximum.wall_time_seconds
}

#[cfg(test)]
mod tests {
    use colibri_protocol::{
        SCHEMA_VERSION_V1,
        digest::Sha256Digest,
        executor::NetworkPolicy,
        ids::{PipelineId, ProviderId, QuoteId, StageId, TemplateId, WorkerId},
        ledger::{Microcredits, QuoteV1},
        pipeline::{ArtifactType, DataClass, ResourceRequirementsV1, RuntimeKind, StageSpecV1},
        provider::{AcceleratorCapabilityV1, AcceleratorVendor, WorkerCapabilityV1},
        template::TemplateManifestV1,
        verification::VerificationPolicyV1,
    };
    use proptest::prelude::*;

    use super::{AssignmentV1, CandidateV1, SchedulerPolicyV1, select_worker};

    fn resources(memory: u64) -> ResourceRequirementsV1 {
        ResourceRequirementsV1 {
            cpu_cores: 4,
            ram_bytes: 2 * 1_048_576_000,
            accelerator_memory_bytes: memory,
            scratch_bytes: 1_048_576_000,
            wall_time_seconds: 3_600,
            required_precisions: vec!["fp16".into()],
        }
    }

    fn stage() -> StageSpecV1 {
        StageSpecV1 {
            stage_id: StageId::new("train").unwrap(),
            depends_on: Vec::new(),
            template_id: TemplateId::new("training-v1").unwrap(),
            template_digest: Sha256Digest::of_bytes(b"template"),
            runtime: RuntimeKind::Python,
            input_manifests: Vec::new(),
            expected_outputs: vec![ArtifactType::Adapter],
            resources: resources(8_000_000_000),
            max_attempts: 2,
            quote_cap: Microcredits::new(1_000),
            checkpoint_policy: None,
            verification_policy: verification_policy(),
            data_class: DataClass::Public,
        }
    }

    fn template() -> TemplateManifestV1 {
        TemplateManifestV1 {
            schema_version: SCHEMA_VERSION_V1,
            template_id: TemplateId::new("training-v1").unwrap(),
            bundle_digest: Sha256Digest::of_bytes(b"template"),
            runtime: RuntimeKind::Python,
            supported_operating_systems: vec!["linux".into()],
            supported_architectures: vec!["x86_64".into()],
            supported_backends: vec!["cuda".into()],
            parameter_schema_digest: Sha256Digest::of_bytes(b"parameters"),
            input_types: Vec::new(),
            output_types: vec![ArtifactType::Adapter],
            maximum_resources: resources(24_000_000_000),
            network_policy: NetworkPolicy::DenyAll,
            network_allowlist_digest: None,
            verification_policy: verification_policy(),
            graceful_drain_seconds: 30,
        }
    }

    fn verification_policy() -> VerificationPolicyV1 {
        VerificationPolicyV1 {
            schema_version: SCHEMA_VERSION_V1,
            policy_digest: Sha256Digest::of_bytes(b"verification"),
            require_checkpoint_load: true,
            require_independent_provider: true,
            hidden_canary: false,
            duplicate_execution_basis_points: 0,
        }
    }

    fn candidate(provider: &str, worker: &str, memory: u64, price: i64) -> CandidateV1 {
        let provider_id = ProviderId::new(provider).unwrap();
        CandidateV1 {
            capability: WorkerCapabilityV1 {
                schema_version: SCHEMA_VERSION_V1,
                provider_id: provider_id.clone(),
                worker_id: WorkerId::new(worker).unwrap(),
                machine_fingerprint_digest: Sha256Digest::of_bytes(worker.as_bytes()),
                operating_system: "linux".into(),
                architecture: "x86_64".into(),
                cpu_cores: 8,
                ram_bytes: 16_000_000_000,
                available_storage_bytes: 100_000_000_000,
                accelerators: vec![AcceleratorCapabilityV1 {
                    vendor: AcceleratorVendor::Nvidia,
                    model: "test-gpu".into(),
                    backend: "cuda".into(),
                    driver_version: "test-driver".into(),
                    memory_bytes: memory,
                    supported_precisions: vec!["fp16".into()],
                }],
                benchmark_manifest_digest: Sha256Digest::of_bytes(b"benchmark"),
                available: true,
                maximum_temperature_celsius: Some(85),
                power_limit_milliwatts: Some(250_000),
                expires_at_unix_ms: 100_000,
            },
            quote: QuoteV1 {
                schema_version: SCHEMA_VERSION_V1,
                quote_id: QuoteId::new(format!("quote-{worker}")).unwrap(),
                pipeline_id: PipelineId::new("pipeline-1").unwrap(),
                stage_id: StageId::new("train").unwrap(),
                provider_id,
                execution: Microcredits::new(price),
                verification_reserve: Microcredits::new(100),
                policy_version: 1,
                expires_at_unix_ms: 100_000,
            },
            reliability_basis_points: 9_900,
        }
    }

    const POLICY: SchedulerPolicyV1 = SchedulerPolicyV1 {
        schema_version: SCHEMA_VERSION_V1,
        policy_version: 1,
        minimum_reliability_basis_points: 9_500,
    };

    fn selected(candidates: &[CandidateV1]) -> Option<AssignmentV1> {
        select_worker(
            &PipelineId::new("pipeline-1").unwrap(),
            &stage(),
            &template(),
            candidates,
            POLICY,
            1_000,
        )
        .unwrap()
    }

    #[test]
    fn rejects_incompatible_or_over_budget_workers() {
        let too_small = candidate("provider-a", "worker-a", 4_000_000_000, 500);
        let too_expensive = candidate("provider-b", "worker-b", 24_000_000_000, 1_001);
        assert_eq!(selected(&[too_small, too_expensive]), None);
    }

    #[test]
    fn quote_must_be_bound_to_the_pipeline_being_scheduled() {
        let mut wrong_pipeline = candidate("provider-a", "worker-a", 24_000_000_000, 500);
        wrong_pipeline.quote.pipeline_id = PipelineId::new("pipeline-other").unwrap();
        assert_eq!(selected(&[wrong_pipeline]), None);
    }

    proptest! {
        #[test]
        fn selection_is_order_independent(reverse in any::<bool>()) {
            let mut candidates = vec![
                candidate("provider-z", "worker-z", 24_000_000_000, 700),
                candidate("provider-a", "worker-a", 24_000_000_000, 800),
            ];
            if reverse {
                candidates.reverse();
            }
            let assignment = selected(&candidates).expect("eligible worker");
            prop_assert_eq!(assignment.provider_id.as_str(), "provider-a");
            prop_assert_eq!(assignment.worker_id.as_str(), "worker-a");
            prop_assert_eq!(assignment.selected_backend, "cuda");
        }
    }
}
