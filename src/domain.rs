use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BenchmarkCatalog {
    pub version: String,
    pub tasks: Vec<BenchmarkTask>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BenchmarkTask {
    pub id: String,
    pub prompt_version: String,
    pub department: String,
    pub team: String,
    pub business_value: String,
    pub process: String,
    pub task_class: String,
    pub data_classification: DataClassification,
    pub prompt: String,
    pub grader: GraderSpec,
    pub quality_gate: QualityGate,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DataClassification {
    Public,
    Internal,
    Confidential,
    Restricted,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GraderSpec {
    Exact { expected: String },
    ContainsAll { values: Vec<String> },
    Regex { pattern: String },
    JsonFields { fields: Vec<String> },
    JsonEquals { expected: Value },
    ExactLines {
        expected: Vec<String>,
        #[serde(default)]
        ignore_order: bool,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct QualityGate {
    pub deterministic_pass_required: bool,
    pub minimum_human_score: Option<u8>,
    pub maximum_latency_ms: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModelCatalog {
    pub version: String,
    pub models: Vec<ModelConfig>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModelConfig {
    pub id: String,
    pub display_name: String,
    pub provider: String,
    pub model: String,
    #[serde(skip_serializing)]
    pub base_url: String,
    #[serde(skip_serializing)]
    pub api_key_env: Option<String>,
    pub quantisation: String,
    pub runtime: String,
    pub hardware: String,
    pub input_cost_per_million: Option<f64>,
    pub output_cost_per_million: Option<f64>,
    /// How the price above was established (for example "vendor price list
    /// 2026-07" or "measured on 1x RTX 6000, 2026-07-22"). `None` means the
    /// cost is not yet configured; the price fields must then also be `None`.
    #[serde(default)]
    pub cost_basis: Option<String>,
    pub license: String,
    pub registry_source: String,
    pub artifact_digest: Option<String>,
    pub approved_for_live: bool,
    pub allowed_data: Vec<DataClassification>,
    pub enabled: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct StartRunRequest {
    pub task_ids: Vec<String>,
    pub model_ids: Vec<String>,
    #[serde(default)]
    pub demo: bool,
    /// How many times to run each task and model pair. Repeating a stochastic
    /// case exposes variance; the worst result should decide eligibility.
    /// Defaults to one and is capped at five for the POC.
    #[serde(default)]
    pub repetitions: Option<u32>,
}

impl StartRunRequest {
    pub fn repetition_count(&self) -> u32 {
        self.repetitions.unwrap_or(1).clamp(1, 5)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RunResult {
    pub run_id: String,
    #[serde(default)]
    pub evaluation_id: String,
    pub created_at: String,
    #[serde(default)]
    pub execution_mode: ExecutionMode,
    #[serde(default)]
    pub execution_status: ExecutionStatus,
    #[serde(default)]
    pub benchmark_version: String,
    #[serde(default)]
    pub model_catalog_version: String,
    pub task_id: String,
    #[serde(default)]
    pub prompt_version: String,
    pub department: String,
    pub team: String,
    pub business_value: String,
    pub process: String,
    pub task_class: String,
    pub model_id: String,
    #[serde(default)]
    pub provider_model_id: String,
    pub quantisation: String,
    pub runtime: String,
    pub hardware: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub registry_source: String,
    #[serde(default)]
    pub artifact_digest: Option<String>,
    pub deterministic_pass: bool,
    pub grader_detail: String,
    #[serde(default)]
    pub quality_gate: QualityGate,
    #[serde(default)]
    pub gate_status: GateStatus,
    pub human_quality_score: Option<u8>,
    #[serde(default)]
    pub reviewer: Option<String>,
    #[serde(default)]
    pub reviewed_at: Option<String>,
    #[serde(default)]
    pub latency_ms: Option<u64>,
    #[serde(default)]
    pub tokens_in: Option<u64>,
    #[serde(default)]
    pub tokens_out: Option<u64>,
    #[serde(default)]
    pub estimated_cost_per_1000_tasks: Option<f64>,
    #[serde(default)]
    pub cost_basis: Option<String>,
    /// One-based repetition index within the evaluation for this task and model.
    #[serde(default = "one")]
    pub repetition: u32,
    /// True when this model is the recorded champion for the task class.
    #[serde(default)]
    pub is_champion: bool,
    /// True when a challenger failed a case that the champion passed in the same
    /// evaluation. A regression blocks promotion regardless of cost.
    #[serde(default)]
    pub regressed_vs_champion: bool,
    pub data_classification: DataClassification,
    pub sovereignty_note: String,
    #[serde(default)]
    pub response_preview: Option<String>,
}

fn one() -> u32 {
    1
}

#[derive(Clone, Debug, Deserialize)]
pub struct ReviewRequest {
    pub score: u8,
    pub reviewer: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    Demo,
    Live,
    #[default]
    LegacyUnknown,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    #[default]
    Completed,
    ProviderError,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GateStatus {
    Eligible,
    DemoOnly,
    #[default]
    PendingHumanReview,
    Rejected,
}

impl Default for QualityGate {
    fn default() -> Self {
        Self {
            deterministic_pass_required: true,
            minimum_human_score: None,
            maximum_latency_ms: None,
        }
    }
}

pub fn evaluate_gate(
    gate: &QualityGate,
    execution_mode: &ExecutionMode,
    execution_status: &ExecutionStatus,
    deterministic_pass: bool,
    latency_ms: Option<u64>,
    human_score: Option<u8>,
) -> GateStatus {
    if *execution_status != ExecutionStatus::Completed {
        return GateStatus::Rejected;
    }
    if gate.deterministic_pass_required && !deterministic_pass {
        return GateStatus::Rejected;
    }
    if gate.maximum_latency_ms.is_some_and(|limit| latency_ms.is_none_or(|latency| latency > limit)) {
        return GateStatus::Rejected;
    }
    if let Some(minimum) = gate.minimum_human_score {
        return match human_score {
            Some(score) if score >= minimum => {
                if *execution_mode == ExecutionMode::Demo { GateStatus::DemoOnly } else { GateStatus::Eligible }
            }
            Some(_) => GateStatus::Rejected,
            None => GateStatus::PendingHumanReview,
        };
    }
    if *execution_mode == ExecutionMode::Demo { GateStatus::DemoOnly } else { GateStatus::Eligible }
}

#[cfg(test)]
mod gate_tests {
    use super::*;

    fn gate() -> QualityGate {
        QualityGate {
            deterministic_pass_required: true,
            minimum_human_score: Some(4),
            maximum_latency_ms: Some(1_000),
        }
    }

    #[test]
    fn human_review_is_required_before_eligibility() {
        assert_eq!(
            evaluate_gate(&gate(), &ExecutionMode::Live, &ExecutionStatus::Completed, true, Some(500), None),
            GateStatus::PendingHumanReview
        );
    }

    #[test]
    fn latency_failure_cannot_be_overridden_by_human_score() {
        assert_eq!(
            evaluate_gate(&gate(), &ExecutionMode::Live, &ExecutionStatus::Completed, true, Some(1_001), Some(5)),
            GateStatus::Rejected
        );
    }

    #[test]
    fn demo_run_never_becomes_eligible() {
        assert_eq!(
            evaluate_gate(&gate(), &ExecutionMode::Demo, &ExecutionStatus::Completed, true, Some(500), Some(5)),
            GateStatus::DemoOnly
        );
    }
}
