mod approvals;
mod auth;
mod champions;
mod config;
mod crypto;
mod domain;
mod grader;
mod provider;
mod store;

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::Result;
use axum::{
    extract::{Path, Request, State},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use clap::Parser;
use reqwest::Client;
use tower_http::{limit::RequestBodyLimitLayer, services::ServeDir, trace::TraceLayer};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

use crate::{
    approvals::ApprovalLedger,
    auth::{AccessControl, AuthDecision},
    champions::ChampionRegistry,
    crypto::Cipher,
    domain::{
        evaluate_gate, BenchmarkCatalog, DataClassification, ExecutionMode, ExecutionStatus,
        ModelCatalog, ReviewRequest, RunResult, StartRunRequest,
    },
    store::Store,
};

const DB_KEY_ENV: &str = "MODEL_GATE_DB_KEY";

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "127.0.0.1:8080")]
    bind: String,
    #[arg(long, default_value = "config/benchmarks.yaml")]
    benchmarks: PathBuf,
    #[arg(long, default_value = "config/models.yaml")]
    models: PathBuf,
    #[arg(long, default_value = "config/access.yaml")]
    access: PathBuf,
    #[arg(long, default_value = "config/approvals.yaml")]
    approvals: PathBuf,
    #[arg(long, default_value = "config/champions.yaml")]
    champions: PathBuf,
    #[arg(long, default_value = "model-gate.db")]
    database: PathBuf,
    /// Delete evidence older than this many days. Unset keeps every record.
    #[arg(long, env = "MODEL_GATE_RETENTION_DAYS")]
    retention_days: Option<i64>,
    /// Maximum API requests accepted per minute across all callers.
    #[arg(long, default_value_t = 240)]
    rate_limit_per_minute: u32,
    /// Maximum accepted request body size in bytes.
    #[arg(long, default_value_t = 65_536)]
    max_body_bytes: usize,
}

struct RateState {
    window_minute: i64,
    count: u32,
}

struct AppState {
    benchmarks: BenchmarkCatalog,
    models: ModelCatalog,
    champions: ChampionRegistry,
    access: AccessControl,
    store: Store,
    client: Client,
    rate_limit_per_minute: u32,
    rate: Mutex<RateState>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter(EnvFilter::from_default_env()).init();
    let args = Args::parse();

    let benchmarks = config::load_benchmarks(&args.benchmarks)?;
    let models = config::load_models(&args.models)?;
    config::validate(&benchmarks, &models)?;

    // A model may only run live when a separate approvals ledger counter-signs
    // its exact artifact digest.
    ApprovalLedger::load(&args.approvals)?.verify(&models)?;

    let access = AccessControl::load(&args.access)?;
    if access.enforced() {
        tracing::info!(principals = access.principal_count(), "Access control is enforced");
    } else {
        tracing::warn!(
            "No access tokens are configured. The API is open. Bind to loopback only and add config/access.yaml before any shared deployment."
        );
    }

    let champions = ChampionRegistry::load(&args.champions)?;

    let cipher = Cipher::from_env(DB_KEY_ENV)?;
    if cipher.enabled() {
        tracing::info!("Evidence ledger encryption is enabled");
    } else {
        tracing::warn!(
            "The evidence ledger is not encrypted. Set {DB_KEY_ENV} to a base64 32-byte key before storing sensitive evidence."
        );
    }

    let store = Store::open(&args.database, cipher)?;

    if let Some(days) = args.retention_days {
        let removed = store.purge_before(&retention_cutoff(days))?;
        if removed > 0 {
            tracing::info!(removed, days, "Purged evidence older than the retention window at startup");
        }
    }

    let state = Arc::new(AppState {
        benchmarks,
        models,
        champions,
        access,
        store,
        client: Client::builder().timeout(Duration::from_secs(120)).build()?,
        rate_limit_per_minute: args.rate_limit_per_minute,
        rate: Mutex::new(RateState { window_minute: 0, count: 0 }),
    });

    if let Some(days) = args.retention_days {
        spawn_retention_task(state.clone(), days);
    }

    let app = Router::new()
        .route("/api/health", get(|| async { Json(serde_json::json!({"status":"ok"})) }))
        .route("/api/session", get(session))
        .route("/api/benchmarks", get(list_benchmarks))
        .route("/api/models", get(list_models))
        .route("/api/champions", get(list_champions))
        .route("/api/runs", get(list_runs).post(start_run))
        .route("/api/runs/{id}/review", post(review_run))
        .fallback_service(ServeDir::new("web").append_index_html_on_directories(true))
        .layer(middleware::from_fn_with_state(state.clone(), authorize))
        .layer(middleware::from_fn_with_state(state.clone(), rate_limit))
        .layer(RequestBodyLimitLayer::new(args.max_body_bytes))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&args.bind).await?;
    tracing::info!(address = %args.bind, "Tessera Model Gate is ready");
    axum::serve(listener, app).await?;
    Ok(())
}

fn retention_cutoff(days: i64) -> String {
    (Utc::now() - chrono::Duration::days(days)).to_rfc3339()
}

fn spawn_retention_task(state: Arc<AppState>, days: i64) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(3_600));
        ticker.tick().await; // consume the immediate first tick
        loop {
            ticker.tick().await;
            match state.store.purge_before(&retention_cutoff(days)) {
                Ok(removed) if removed > 0 => tracing::info!(removed, "Purged evidence past the retention window"),
                Ok(_) => {}
                Err(error) => tracing::error!(error = %error, "Retention purge failed"),
            }
        }
    });
}

// --- Middleware -----------------------------------------------------------

fn presented_token(request: &Request) -> Option<String> {
    if let Some(token) = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| auth::bearer(Some(value)))
    {
        return Some(token.to_string());
    }
    request
        .headers()
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(str::to_string)
}

async fn authorize(State(state): State<Arc<AppState>>, request: Request, next: Next) -> Response {
    let Some(required) = auth::required_role(request.method().as_str(), request.uri().path()) else {
        return next.run(request).await;
    };
    let token = presented_token(&request);
    match state.access.decide(token.as_deref(), required) {
        AuthDecision::Allow(_) => next.run(request).await,
        AuthDecision::Unauthenticated => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "A valid access token is required"})),
        )
            .into_response(),
        AuthDecision::Forbidden => (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "This token does not have the required role"})),
        )
            .into_response(),
    }
}

async fn rate_limit(State(state): State<Arc<AppState>>, request: Request, next: Next) -> Response {
    let minute = Utc::now().timestamp() / 60;
    let over = {
        let mut rate = state.rate.lock().expect("rate limiter lock");
        if rate.window_minute != minute {
            rate.window_minute = minute;
            rate.count = 0;
        }
        rate.count += 1;
        rate.count > state.rate_limit_per_minute
    };
    if over {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({"error": "The request rate limit was exceeded. Retry shortly."})),
        )
            .into_response();
    }
    next.run(request).await
}

// --- Handlers -------------------------------------------------------------

async fn session(State(state): State<Arc<AppState>>, request: Request) -> Json<serde_json::Value> {
    let token = presented_token(&request);
    let (role, principal_name) = match state.access.decide(token.as_deref(), auth::Role::Viewer) {
        AuthDecision::Allow(Some(principal)) => (
            Some(format!("{:?}", principal.role).to_lowercase()),
            Some(principal.display_name),
        ),
        _ => (None, None),
    };
    Json(serde_json::json!({
        "auth_required": state.access.enforced(),
        "role": role,
        "principal": principal_name,
    }))
}

async fn list_benchmarks(State(state): State<Arc<AppState>>) -> Json<BenchmarkCatalog> {
    Json(state.benchmarks.clone())
}

async fn list_models(State(state): State<Arc<AppState>>) -> Json<ModelCatalog> {
    Json(state.models.clone())
}

async fn list_champions(State(state): State<Arc<AppState>>) -> Json<ChampionRegistry> {
    Json(state.champions.clone())
}

async fn list_runs(State(state): State<Arc<AppState>>) -> Result<Json<Vec<RunResult>>, ApiError> {
    Ok(Json(state.store.list()?))
}

async fn start_run(
    State(state): State<Arc<AppState>>,
    Json(request): Json<StartRunRequest>,
) -> Result<Json<Vec<RunResult>>, ApiError> {
    if request.task_ids.is_empty() || request.model_ids.is_empty() {
        return Err(ApiError::bad_request("Select at least one task and one model"));
    }
    reject_duplicates("task", &request.task_ids)?;
    reject_duplicates("model", &request.model_ids)?;
    let repetitions = request.repetition_count();

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
            let is_champion = state.champions.is_champion(&task.task_class, &model.id);
            for repetition in 1..=repetitions {
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
                    cost_basis: model.cost_basis.clone(),
                    repetition,
                    is_champion,
                    regressed_vs_champion: false,
                    data_classification: task.data_classification.clone(),
                    sovereignty_note: if local { "Declared local route. Endpoint location is not independently verified.".into() } else { "Hosted route. Confirm the provider region, contract, and data controls before use.".into() },
                    response_preview,
                };
                results.push(result);
            }
        }
    }

    flag_regressions(&mut results);

    for result in &results {
        state.store.save(result)?;
    }
    Ok(Json(results))
}

/// A challenger that fails a task the champion passed in the same evaluation is
/// a regression. The champion must pass every repetition of the task to count.
fn flag_regressions(results: &mut [RunResult]) {
    let mut champion_passed: HashMap<String, bool> = HashMap::new();
    for result in results.iter() {
        if result.is_champion {
            let entry = champion_passed.entry(result.task_id.clone()).or_insert(true);
            *entry = *entry && result.deterministic_pass;
        }
    }
    for result in results.iter_mut() {
        if !result.is_champion
            && champion_passed.get(&result.task_id) == Some(&true)
            && !result.deterministic_pass
        {
            result.regressed_vs_champion = true;
        }
    }
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
    use crate::domain::{ExecutionMode, ExecutionStatus, GateStatus, QualityGate};

    fn run(task_id: &str, is_champion: bool, pass: bool) -> RunResult {
        RunResult {
            run_id: Uuid::new_v4().to_string(),
            evaluation_id: "e".into(),
            created_at: "2026-07-23T00:00:00Z".into(),
            execution_mode: ExecutionMode::Demo,
            execution_status: ExecutionStatus::Completed,
            benchmark_version: "v".into(),
            model_catalog_version: "v".into(),
            task_id: task_id.into(),
            prompt_version: "1".into(),
            department: "d".into(),
            team: "t".into(),
            business_value: "b".into(),
            process: "p".into(),
            task_class: "c".into(),
            model_id: "m".into(),
            provider_model_id: "m".into(),
            quantisation: "Q8".into(),
            runtime: "r".into(),
            hardware: "h".into(),
            license: "l".into(),
            registry_source: "s".into(),
            artifact_digest: None,
            deterministic_pass: pass,
            grader_detail: "d".into(),
            quality_gate: QualityGate::default(),
            gate_status: GateStatus::default(),
            human_quality_score: None,
            reviewer: None,
            reviewed_at: None,
            latency_ms: None,
            tokens_in: None,
            tokens_out: None,
            estimated_cost_per_1000_tasks: None,
            cost_basis: None,
            repetition: 1,
            is_champion,
            regressed_vs_champion: false,
            data_classification: DataClassification::Internal,
            sovereignty_note: "n".into(),
            response_preview: None,
        }
    }

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

    #[test]
    fn challenger_failure_after_champion_pass_is_a_regression() {
        let mut results = vec![run("T1", true, true), run("T1", false, false)];
        flag_regressions(&mut results);
        assert!(!results[0].regressed_vs_champion);
        assert!(results[1].regressed_vs_champion);
    }

    #[test]
    fn challenger_failure_without_champion_pass_is_not_a_regression() {
        let mut results = vec![run("T1", true, false), run("T1", false, false)];
        flag_regressions(&mut results);
        assert!(!results[1].regressed_vs_champion);
    }

    #[test]
    fn repetition_count_is_clamped() {
        let request = StartRunRequest { task_ids: vec![], model_ids: vec![], demo: true, repetitions: Some(99) };
        assert_eq!(request.repetition_count(), 5);
        let default = StartRunRequest { task_ids: vec![], model_ids: vec![], demo: true, repetitions: None };
        assert_eq!(default.repetition_count(), 1);
    }
}
