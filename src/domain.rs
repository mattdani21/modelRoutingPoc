use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct BenchmarkCatalog {
    pub version: String,
    pub tasks: Vec<BenchmarkTask>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BenchmarkTask {
    pub id: String,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
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
    pub base_url: String,
    pub api_key_env: Option<String>,
    pub quantisation: String,
    pub runtime: String,
    pub hardware: String,
    pub input_cost_per_million: f64,
    pub output_cost_per_million: f64,
    pub allowed_data: Vec<DataClassification>,
    pub enabled: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct StartRunRequest {
    pub task_ids: Vec<String>,
    pub model_ids: Vec<String>,
    #[serde(default)]
    pub demo: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct RunResult {
    pub run_id: String,
    pub created_at: String,
    pub task_id: String,
    pub department: String,
    pub team: String,
    pub business_value: String,
    pub process: String,
    pub task_class: String,
    pub model_id: String,
    pub quantisation: String,
    pub runtime: String,
    pub hardware: String,
    pub deterministic_pass: bool,
    pub grader_detail: String,
    pub human_quality_score: Option<u8>,
    pub latency_ms: u64,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub estimated_cost_per_1000_tasks: f64,
    pub data_classification: DataClassification,
    pub sovereignty_note: String,
    pub response_preview: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ReviewRequest {
    pub score: u8,
}
