//! Immutable pipeline DAG contracts and bounded validation.

use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::{
    ProtocolError, SCHEMA_VERSION_V1, canonical,
    digest::Sha256Digest,
    ids::{PipelineId, StageId, TemplateId},
    ledger::Microcredits,
    signing::SignedContract,
    verification::VerificationPolicyV1,
};

pub const MAX_STAGES: usize = 100;
pub const MAX_FAN_OUT: usize = 32;
pub const MAX_DEPENDENCY_DEPTH: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeKind {
    Python,
    NativeRust,
    ColibriC,
    Mojo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactType {
    Dataset,
    Checkpoint,
    Adapter,
    Model,
    Metrics,
    Evaluation,
    Report,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataClass {
    Public,
    Licensed,
    Confidential,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceRequirementsV1 {
    pub cpu_cores: u16,
    pub ram_bytes: u64,
    pub accelerator_memory_bytes: u64,
    pub scratch_bytes: u64,
    pub wall_time_seconds: u32,
    pub required_precisions: Vec<String>,
}

impl ResourceRequirementsV1 {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        const MIB: u64 = 1_048_576;
        const TIB: u64 = 1_099_511_627_776;
        if !(1..=128).contains(&self.cpu_cores) {
            return Err(ProtocolError::Invariant(
                "cpu_cores must be between 1 and 128".into(),
            ));
        }
        if !(256 * MIB..=TIB).contains(&self.ram_bytes) {
            return Err(ProtocolError::Invariant(
                "ram_bytes is outside MVP bounds".into(),
            ));
        }
        if self.accelerator_memory_bytes > TIB || self.scratch_bytes > 2 * TIB {
            return Err(ProtocolError::Invariant(
                "accelerator or scratch storage exceeds MVP bounds".into(),
            ));
        }
        if !(1..=7 * 24 * 60 * 60).contains(&self.wall_time_seconds) {
            return Err(ProtocolError::Invariant(
                "wall_time_seconds is outside MVP bounds".into(),
            ));
        }
        if self.required_precisions.len() > 8 {
            return Err(ProtocolError::Invariant(
                "too many required precision modes".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointPolicyV1 {
    pub optimizer_steps: u32,
    pub interval_seconds: u32,
    pub retain_latest: u16,
}

impl CheckpointPolicyV1 {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.optimizer_steps == 0 || self.interval_seconds == 0 || self.retain_latest == 0 {
            return Err(ProtocolError::Invariant(
                "checkpoint intervals and retention must be non-zero".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StageSpecV1 {
    pub stage_id: StageId,
    pub depends_on: Vec<StageId>,
    pub template_id: TemplateId,
    pub template_digest: Sha256Digest,
    pub runtime: RuntimeKind,
    pub input_manifests: Vec<Sha256Digest>,
    pub expected_outputs: Vec<ArtifactType>,
    pub resources: ResourceRequirementsV1,
    pub max_attempts: u16,
    pub quote_cap: Microcredits,
    pub checkpoint_policy: Option<CheckpointPolicyV1>,
    pub verification_policy: VerificationPolicyV1,
    pub data_class: DataClass,
}

impl StageSpecV1 {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        self.resources.validate()?;
        self.verification_policy.validate()?;
        if self.max_attempts == 0 {
            return Err(ProtocolError::Invariant(
                "max_attempts must be non-zero".into(),
            ));
        }
        if self.quote_cap < Microcredits::ZERO {
            return Err(ProtocolError::Invariant(
                "stage quote cap must be non-negative".into(),
            ));
        }
        if self.expected_outputs.is_empty() {
            return Err(ProtocolError::Invariant(
                "stage must declare at least one output type".into(),
            ));
        }
        if let Some(policy) = &self.checkpoint_policy {
            policy.validate()?;
        }
        Ok(())
    }
}

/// The canonical, immutable pipeline submitted and funded by a customer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PipelineSpecV1 {
    pub schema_version: u16,
    pub pipeline_id: PipelineId,
    pub revision: u32,
    pub scheduler_policy_version: u32,
    pub stages: Vec<StageSpecV1>,
}

impl SignedContract for PipelineSpecV1 {
    const CONTRACT_TYPE: &'static str = "pipeline_spec_v1";

    fn validate_signed_contract(&self, _now_unix_ms: u64) -> Result<(), ProtocolError> {
        self.validate()
    }
}

impl PipelineSpecV1 {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.schema_version != SCHEMA_VERSION_V1 {
            return Err(ProtocolError::UnsupportedSchemaVersion {
                actual: self.schema_version,
                expected: SCHEMA_VERSION_V1,
            });
        }
        if self.revision == 0 || self.scheduler_policy_version == 0 {
            return Err(ProtocolError::Invariant(
                "pipeline and scheduler policy revisions must be non-zero".into(),
            ));
        }
        if self.stages.is_empty() || self.stages.len() > MAX_STAGES {
            return Err(ProtocolError::Invariant(format!(
                "pipeline must contain between 1 and {MAX_STAGES} stages"
            )));
        }

        let mut indices = HashMap::with_capacity(self.stages.len());
        for (index, stage) in self.stages.iter().enumerate() {
            stage.validate()?;
            if indices.insert(stage.stage_id.clone(), index).is_some() {
                return Err(ProtocolError::Invariant(format!(
                    "duplicate stage id {}",
                    stage.stage_id
                )));
            }
        }

        let mut indegree = vec![0_usize; self.stages.len()];
        let mut successors = vec![Vec::<usize>::new(); self.stages.len()];
        for (index, stage) in self.stages.iter().enumerate() {
            let mut unique_dependencies = HashSet::with_capacity(stage.depends_on.len());
            for dependency in &stage.depends_on {
                if !unique_dependencies.insert(dependency) {
                    return Err(ProtocolError::Invariant(format!(
                        "stage {} repeats dependency {dependency}",
                        stage.stage_id
                    )));
                }
                let dependency_index = *indices.get(dependency).ok_or_else(|| {
                    ProtocolError::Invariant(format!(
                        "stage {} references missing dependency {dependency}",
                        stage.stage_id
                    ))
                })?;
                if dependency_index == index {
                    return Err(ProtocolError::Invariant(format!(
                        "stage {} depends on itself",
                        stage.stage_id
                    )));
                }
                indegree[index] += 1;
                successors[dependency_index].push(index);
                if successors[dependency_index].len() > MAX_FAN_OUT {
                    return Err(ProtocolError::Invariant(format!(
                        "stage {dependency} exceeds fan-out {MAX_FAN_OUT}"
                    )));
                }
            }
        }

        let mut queue = VecDeque::new();
        let mut depth = vec![1_usize; self.stages.len()];
        for (index, degree) in indegree.iter().enumerate() {
            if *degree == 0 {
                queue.push_back(index);
            }
        }

        let mut visited = 0_usize;
        while let Some(index) = queue.pop_front() {
            visited += 1;
            for &successor in &successors[index] {
                depth[successor] = depth[successor].max(depth[index] + 1);
                if depth[successor] > MAX_DEPENDENCY_DEPTH {
                    return Err(ProtocolError::Invariant(format!(
                        "dependency depth exceeds {MAX_DEPENDENCY_DEPTH}"
                    )));
                }
                indegree[successor] -= 1;
                if indegree[successor] == 0 {
                    queue.push_back(successor);
                }
            }
        }
        if visited != self.stages.len() {
            return Err(ProtocolError::Invariant(
                "pipeline dependency graph contains a cycle".into(),
            ));
        }
        Ok(())
    }

    pub fn canonical_digest(&self) -> Result<Sha256Digest, ProtocolError> {
        self.validate()?;
        canonical::digest(self)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ArtifactType, DataClass, PipelineSpecV1, ResourceRequirementsV1, RuntimeKind, StageSpecV1,
    };
    use crate::{
        SCHEMA_VERSION_V1,
        digest::Sha256Digest,
        ids::{PipelineId, StageId, TemplateId},
        ledger::Microcredits,
        verification::VerificationPolicyV1,
    };

    fn stage(id: &str, dependencies: &[&str]) -> StageSpecV1 {
        StageSpecV1 {
            stage_id: StageId::new(id).unwrap(),
            depends_on: dependencies
                .iter()
                .map(|dependency| StageId::new(*dependency).unwrap())
                .collect(),
            template_id: TemplateId::new("dataset-prepare-v1").unwrap(),
            template_digest: Sha256Digest::of_bytes(b"template"),
            runtime: RuntimeKind::Python,
            input_manifests: Vec::new(),
            expected_outputs: vec![ArtifactType::Dataset],
            resources: ResourceRequirementsV1 {
                cpu_cores: 2,
                ram_bytes: 1_073_741_824,
                accelerator_memory_bytes: 0,
                scratch_bytes: 1_073_741_824,
                wall_time_seconds: 300,
                required_precisions: Vec::new(),
            },
            max_attempts: 2,
            quote_cap: Microcredits::new(1_000_000),
            checkpoint_policy: None,
            verification_policy: VerificationPolicyV1 {
                schema_version: SCHEMA_VERSION_V1,
                policy_digest: Sha256Digest::of_bytes(b"policy"),
                require_checkpoint_load: false,
                require_independent_provider: false,
                hidden_canary: false,
                duplicate_execution_basis_points: 0,
            },
            data_class: DataClass::Public,
        }
    }

    fn pipeline(stages: Vec<StageSpecV1>) -> PipelineSpecV1 {
        PipelineSpecV1 {
            schema_version: SCHEMA_VERSION_V1,
            pipeline_id: PipelineId::new("pipeline-1").unwrap(),
            revision: 1,
            scheduler_policy_version: 1,
            stages,
        }
    }

    #[test]
    fn accepts_a_bounded_dag() {
        assert!(
            pipeline(vec![stage("a", &[]), stage("b", &["a"])])
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn rejects_cycles_and_missing_dependencies() {
        assert!(
            pipeline(vec![stage("a", &["b"]), stage("b", &["a"])])
                .validate()
                .is_err()
        );
        assert!(pipeline(vec![stage("a", &["missing"])]).validate().is_err());
    }

    #[test]
    fn canonical_hash_is_stable_across_json_round_trip() {
        let pipeline = pipeline(vec![stage("a", &[]), stage("b", &["a"])]);
        let json = serde_json::to_vec(&pipeline).unwrap();
        let decoded: PipelineSpecV1 = serde_json::from_slice(&json).unwrap();
        assert_eq!(
            pipeline.canonical_digest().unwrap(),
            decoded.canonical_digest().unwrap()
        );
    }
}
