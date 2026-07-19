use std::{env, time::Instant};

use anyhow::{Context, Result, bail};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::domain::{BenchmarkTask, GraderSpec, ModelConfig};

#[derive(Debug)]
pub struct ModelAnswer {
    pub text: String,
    pub latency_ms: u64,
    pub tokens_in: u64,
    pub tokens_out: u64,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<Message<'a>>,
    temperature: f32,
}

#[derive(Serialize)]
struct Message<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
    #[serde(default)]
    usage: Usage,
}

#[derive(Deserialize)]
struct Choice {
    message: AnswerMessage,
}

#[derive(Deserialize)]
struct AnswerMessage {
    content: String,
}

#[derive(Default, Deserialize)]
struct Usage {
    #[serde(default)]
    prompt_tokens: u64,
    #[serde(default)]
    completion_tokens: u64,
}

pub async fn call_model(client: &Client, model: &ModelConfig, task: &BenchmarkTask) -> Result<ModelAnswer> {
    let url = format!("{}/chat/completions", model.base_url.trim_end_matches('/'));
    let body = ChatRequest {
        model: &model.model,
        messages: vec![
            Message { role: "system", content: "Follow the task. Return only the requested result. Do not include confidential data that is not in the prompt." },
            Message { role: "user", content: &task.prompt },
        ],
        temperature: 0.0,
    };

    let mut request = client.post(url).json(&body);
    if let Some(env_name) = &model.api_key_env {
        let key = env::var(env_name).with_context(|| format!("Missing API key: {env_name}"))?;
        request = request.bearer_auth(key);
    }

    let started = Instant::now();
    let response = request.send().await?.error_for_status()?;
    let parsed: ChatResponse = response.json().await?;
    let answer = parsed.choices.into_iter().next().context("The provider returned no answer")?;

    Ok(ModelAnswer {
        text: answer.message.content,
        latency_ms: started.elapsed().as_millis() as u64,
        tokens_in: parsed.usage.prompt_tokens,
        tokens_out: parsed.usage.completion_tokens,
    })
}

pub fn demo_answer(task: &BenchmarkTask, model_index: usize) -> Result<ModelAnswer> {
    let text = match &task.grader {
        GraderSpec::Exact { expected } => expected.clone(),
        GraderSpec::ContainsAll { values } => values.join("; "),
        GraderSpec::Regex { .. } => "ERROR policy=POL123 source=ACE severity=high".into(),
        GraderSpec::JsonFields { fields } => {
            let object = fields.iter().map(|field| (field.clone(), serde_json::Value::String("demo".into()))).collect();
            serde_json::to_string(&serde_json::Value::Object(object))?
        }
    };
    if model_index > 20 {
        bail!("The demo model index is invalid");
    }
    Ok(ModelAnswer {
        text,
        latency_ms: 680 + (model_index as u64 * 410) + (task.id.len() as u64 * 7),
        tokens_in: 180 + task.prompt.len() as u64 / 4,
        tokens_out: 70 + model_index as u64 * 9,
    })
}
