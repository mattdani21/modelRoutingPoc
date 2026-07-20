mod config;
mod domain;
mod grader;
mod provider;
mod store;

use std::{path::PathBuf, sync::Arc};

use anyhow::{Context, Result};
use axum::{Json, Router, extract::{Path, State}, http::StatusCode, response::IntoResponse, routing::{get, post}};
use chrono::Utc;
use clap::Parser;
use reqwest::Client;
use tower_http::{services::ServeDir, trace::TraceLayer};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

use crate::{domain::{BenchmarkCatalog, ModelCatalog, ReviewRequest, RunResult, StartRunRequest}, store::Store};

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
    let state = Arc::new(AppState {
        benchmarks: config::load_benchmarks(&args.benchmarks)?,
        models: config::load_models(&args.models)?,
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
    let mut results = Vec::new();
    for task_id in &request.task_ids {
        let task = state.benchmarks.tasks.iter().find(|item| &item.id == task_id)
            .with_context(|| format!("Unknown task: {task_id}"))?;
        for (index, model_id) in request.model_ids.iter().enumerate() {
            let model = state.models.models.iter().find(|item| &item.id == model_id && item.enabled)
                .with_context(|| format!("Unknown or disabled model: {model_id}"))?;
            if !model.allowed_data.contains(&task.data_classification) {
                return Err(ApiError::bad_request(&format!(
                    "Model {model_id} is not approved for the data class used by task {task_id}"
                )));
            }
            let answer = if request.demo {
                provider::demo_answer(task, index)?
            } else {
                provider::call_model(&state.client, model, task).await?
            };
            let (pass, detail) = grader::grade(&task.grader, &answer.text).unwrap_or_else(|error| (false, error.to_string()));
            let cost = ((answer.tokens_in as f64 * model.input_cost_per_million)
                + (answer.tokens_out as f64 * model.output_cost_per_million)) / 1_000_000.0 * 1_000.0;
            let local = model.provider == "local";
            let result = RunResult {
                run_id: Uuid::new_v4().to_string(),
                created_at: Utc::now().to_rfc3339(),
                task_id: task.id.clone(),
                department: task.department.clone(),
                team: task.team.clone(),
                business_value: task.business_value.clone(),
                process: task.process.clone(),
                task_class: task.task_class.clone(),
                model_id: model.id.clone(),
                quantisation: model.quantisation.clone(),
                runtime: model.runtime.clone(),
                hardware: model.hardware.clone(),
                deterministic_pass: pass,
                grader_detail: detail,
                human_quality_score: None,
                latency_ms: answer.latency_ms,
                tokens_in: answer.tokens_in,
                tokens_out: answer.tokens_out,
                estimated_cost_per_1000_tasks: cost,
                data_classification: task.data_classification.clone(),
                sovereignty_note: if local { "Data stays in the approved local environment".into() } else { "Confirm the provider region, contract, and data controls before use".into() },
                response_preview: answer.text.chars().take(240).collect(),
            };
            state.store.save(&result)?;
            results.push(result);
        }
    }
    Ok(Json(results))
}

async fn review_run(State(state): State<Arc<AppState>>, Path(id): Path<String>, Json(review): Json<ReviewRequest>) -> Result<StatusCode, ApiError> {
    state.store.review(&id, review.score)?;
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
        (self.1, Json(serde_json::json!({"error": self.0.to_string()}))).into_response()
    }
}
