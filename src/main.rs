mod config;
mod domain;
mod grader;
mod provider;
mod store;

use std::{collections::HashSet, path::PathBuf, sync::Arc};

use anyhow::Result;
use axum::{Json, Router, extract::{Path, State}, http::StatusCode, response::IntoResponse, routing::{get, post}};
use chrono::Utc;
use clap::Parser;
use reqwest::Client;
use tower_http::{services::ServeDir, trace::TraceLayer};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

use crate::{domain::{
    evaluate_gate, BenchmarkCatalog, DataClassification, ExecutionMode, ExecutionStatus,
    ModelCatalog, ReviewRequest, RunResult, StartRunRequest,
}, store::Store};

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "127.0.0.1:8080")]
    bind: String,
    #[arg(long, default_value = "config/benchmarks.yaml")]
    benchmarks: PathBuf,
    #[arg(long, default_value = "config/models.yaml")]
    models: PathBuf,
    #[arg(long, default_value = "model-gate.db")]
    database: PathBuf,
}

struct AppState {
    benchmarks: BenchmarkCatalog,
    models: ModelCatalog,
    store: Store,
    client: Client,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter(EnvFilter::from_default_env()).init();
    let args = Args::parse();
    let benchmarks = config::load_benchmarks(&args.benchmarks)?;
    let models = config::load_models(&args.models)?;
    config::validate(&benchmarks, &models)?;
    let state = Arc::new(AppState {
        benchmarks,
        models,
        store: Store::open(&args.database)?,
        client: Client::builder().timeout(std::time::Duration::from_secs(120)).build()?,
    });

    let app = Router::new()
        .route("/api/health", get(|| async { Json(serde_json::json!({"status":"ok"})) }))
        .route("/api/benchmarks", get(list_benchmarks))
        .route("/api/models", get(list_models))
        .route("/api/runs", get(list_runs).post(start_run))
        .route("/api/runs/{id}/review", post(review_run))
        .fallback_service(ServeDir::new("web").append_index_html_on_directories(true))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&args.bind).await?;
    tracing::info!(address = %args.bind, "Tessera Model Gate is ready");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn list_benchmarks(State(state): State<Arc<AppState>>) -> Json<BenchmarkCatalog> {
    Json(state.benchmarks.clone())
}

async fn list_models(State(state): State<Arc<AppState>>) -> Json<ModelCatalog> {
    Json(state.models.clone())
}

async fn list_runs(State(state): State<Arc<AppState>>) -> Result<Json<Vec<RunResult>>, ApiError> {
    Ok(Json(state.store.list()?))
}

async fn start_run(State(state): State<Arc<AppState>>, Json(request): Json<StartRunRequest>) -> Result<Json<Vec<RunResult>>, ApiError> {
    if request.task_ids.is_empty() || request.model_ids.is_empty() {
        return Err(ApiError::bad_request("Select at least one task and one model"));
    }
    reject_duplicates("task", &request.task_ids)?;
    reject_duplicates("model", &request.model_ids)?;

    // Validate the full request before the first model call or ledger write.
    for task_id in &request.task_ids {
        let task = state.benchmarks.tasks.iter().find(|item| &item.id == task_id)
            .ok_or_else(|| ApiError::bad_request(&format!("Unknown task: {task_id}")))?;
        for model_id in &request.model_ids {
            let model = state.models.models.iter().find(|item| &item.id == model_id && item.enabled)
                .ok_or_else(|| ApiError::bad_request(&format!("Unknown or disabled model: {model_id}")))?;
            if !model.allowed_data.contains(&task.data_classification) {
                return Err(ApiError::bad_request(&format!(
                    "Model {model_id} is not approved for the data class used by task {task_id}"
                )));
            }
            if !request.demo && !model.approved_for_live {
                return Err(ApiError::bad_request(&format!(
                    "Model {model_id} is not approved for live runs"
                )));
            }
        }
    }

    let evaluation_id = Uuid::new_v4().to_string();
    let execution_mode = if request.demo { ExecutionMode::Demo } else { ExecutionMode::Live };
    let mut results = Vec::new();
    for task_id in &request.task_ids {
        let task = state.benchmarks.tasks.iter().find(|item| &item.id == task_id).expect("validated task");
        for (index, model_id) in request.model_ids.iter().enumerate() {
            let model = state.models.models.iter().find(|item| &item.id == model_id && item.enabled).expect("validated model");
            let answer = if request.demo {
                provider::demo_answer(task, &model.id, index)
            } else {
                provider::call_model(&state.client, model, task).await
            };

            let (execution_status, pass, detail, response, latency_ms, tokens_in, tokens_out) = match answer {
                Ok(answer) => {
                    let (pass, detail) = grader::grade(&task.grader, &answer.text)
                        .unwrap_or_else(|error| (false, format!("Grader failure: {error}")));
                    (ExecutionStatus::Completed, pass, detail, Some(answer.text), Some(answer.latency_ms), answer.tokens_in, answer.tokens_out)
                }
                Err(error) => {
                    tracing::warn!(task_id = %task.id, model_id = %model.id, error = %error, "Model call failed");
                    (ExecutionStatus::ProviderError, false, "Model call failed. Inspect the service log.".into(), None, None, None, None)
                }
            };

            let cost = estimate_cost(model.input_cost_per_million, model.output_cost_per_million, tokens_in, tokens_out);
            let local = model.provider == "local";
            let gate_status = evaluate_gate(&task.quality_gate, &execution_mode, &execution_status, pass, latency_ms, None);
            let response_preview = match task.data_classification {
                DataClassification::Confidential | DataClassification::Restricted => None,
                _ => response.map(|text| text.chars().take(240).collect()),
            };
            let result = RunResult {
                run_id: Uuid::new_v4().to_string(),
                evaluation_id: evaluation_id.clone(),
                created_at: Utc::now().to_rfc3339(),
                execution_mode: execution_mode.clone(),
                execution_status,
                benchmark_version: state.benchmarks.version.clone(),
                model_catalog_version: state.models.version.clone(),
                task_id: task.id.clone(),
                prompt_version: task.prompt_version.clone(),
                department: task.department.clone(),
                team: task.team.clone(),
                business_value: task.business_value.clone(),
                process: task.process.clone(),
                task_class: task.task_class.clone(),
                model_id: model.id.clone(),
                provider_model_id: model.model.clone(),
                quantisation: model.quantisation.clone(),
                runtime: model.runtime.clone(),
                hardware: model.hardware.clone(),
                license: model.license.clone(),
                registry_source: model.registry_source.clone(),
                artifact_digest: model.artifact_digest.clone(),
                deterministic_pass: pass,
                grader_detail: detail,
                quality_gate: task.quality_gate.clone(),
                gate_status,
                human_quality_score: None,
                reviewer: None,
                reviewed_at: None,
                latency_ms,
                tokens_in,
                tokens_out,
                estimated_cost_per_1000_tasks: cost,
                data_classification: task.data_classification.clone(),
                sovereignty_note: if local { "Declared local route. Endpoint location is not independently verified.".into() } else { "Hosted route. Confirm the provider region, contract, and data controls before use.".into() },
                response_preview,
            };
            state.store.save(&result)?;
            results.push(result);
        }
    }
    Ok(Json(results))
}

async fn review_run(State(state): State<Arc<AppState>>, Path(id): Path<String>, Json(review): Json<ReviewRequest>) -> Result<StatusCode, ApiError> {
    if !(1..=5).contains(&review.score) {
        return Err(ApiError::bad_request("The human score must be from 1 to 5"));
    }
    if review.reviewer.trim().is_empty() || review.reviewer.trim().chars().count() > 80 {
        return Err(ApiError::bad_request("The reviewer name must contain 1 to 80 characters"));
    }
    state.store.review(&id, review.score, &review.reviewer)
        .map_err(|error| ApiError::bad_request(&error.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

struct ApiError(anyhow::Error, StatusCode);

impl ApiError {
    fn bad_request(message: &str) -> Self {
        Self(anyhow::anyhow!(message.to_string()), StatusCode::BAD_REQUEST)
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(error: anyhow::Error) -> Self { Self(error, StatusCode::INTERNAL_SERVER_ERROR) }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let message = if self.1 == StatusCode::INTERNAL_SERVER_ERROR {
            tracing::error!(error = %self.0, "Request failed");
            "The request failed. Inspect the service log.".to_string()
        } else {
            self.0.to_string()
        };
        (self.1, Json(serde_json::json!({"error": message}))).into_response()
    }
}

fn reject_duplicates(label: &str, values: &[String]) -> Result<(), ApiError> {
    let mut seen = HashSet::new();
    if values.iter().any(|value| !seen.insert(value)) {
        return Err(ApiError::bad_request(&format!("Duplicate {label} IDs are not allowed")));
    }
    Ok(())
}

fn estimate_cost(
    input_price: Option<f64>,
    output_price: Option<f64>,
    tokens_in: Option<u64>,
    tokens_out: Option<u64>,
) -> Option<f64> {
    Some(((tokens_in? as f64 * input_price?) + (tokens_out? as f64 * output_price?)) / 1_000_000.0 * 1_000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_is_unknown_when_price_is_not_configured() {
        assert_eq!(estimate_cost(None, None, Some(100), Some(50)), None);
    }

    #[test]
    fn cost_uses_input_and_output_prices() {
        assert_eq!(estimate_cost(Some(2.0), Some(10.0), Some(1_000), Some(100)), Some(3.0));
    }

    #[test]
    fn duplicate_ids_are_rejected_before_a_run() {
        let values = vec!["one".to_string(), "one".to_string()];
        assert!(reject_duplicates("task", &values).is_err());
    }
}
