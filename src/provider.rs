use std::{env, time::Instant};

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::domain::{BenchmarkTask, GraderSpec, ModelConfig};

#[derive(Debug)]
pub struct ModelAnswer {
    pub text: String,
    pub latency_ms: u64,
    pub tokens_in: Option<u64>,
    pub tokens_out: Option<u64>,
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
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Choice {
    message: AnswerMessage,
}

#[derive(Deserialize)]
struct AnswerMessage {
    content: String,
}

#[derive(Deserialize)]
struct Usage {
    #[serde(default)]
    prompt_tokens: u64,
    #[serde(default)]
    completion_tokens: u64,
}

/// Build the chat completions URL for an OpenAI-compatible base URL, tolerating
/// a trailing slash.
pub fn chat_url(base_url: &str) -> String {
    format!("{}/chat/completions", base_url.trim_end_matches('/'))
}

pub async fn call_model(client: &Client, model: &ModelConfig, task: &BenchmarkTask) -> Result<ModelAnswer> {
    let url = chat_url(&model.base_url);
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
    let (tokens_in, tokens_out) = parsed
        .usage
        .map(|usage| (Some(usage.prompt_tokens), Some(usage.completion_tokens)))
        .unwrap_or((None, None));
    let answer = parsed.choices.into_iter().next().context("The provider returned no answer")?;

    Ok(ModelAnswer {
        text: answer.message.content,
        latency_ms: started.elapsed().as_millis() as u64,
        tokens_in,
        tokens_out,
    })
}

pub fn demo_answer(task: &BenchmarkTask, model_id: &str, model_index: usize) -> Result<ModelAnswer> {
    let mut text = match &task.grader {
        GraderSpec::Exact { expected } => expected.clone(),
        GraderSpec::ContainsAll { values } => values.join("; "),
        GraderSpec::Regex { .. } => "ERROR policy=POL123 source=ACE severity=high".into(),
        GraderSpec::JsonFields { fields } => {
            let object = fields.iter().map(|field| (field.clone(), serde_json::Value::String("demo".into()))).collect();
            serde_json::to_string(&serde_json::Value::Object(object))?
        }
        GraderSpec::JsonEquals { expected } => serde_json::to_string(expected)?,
        GraderSpec::ExactLines { expected, .. } => expected.join("\n"),
    };

    // Demo data must show both pass and fail states. It is not model evidence.
    if task.id == "ACT-EAC2-001" && model_id == "gpt-oss-20b-local" {
        text = r#"{"finding_code":"TERM_MATCH","required_action_code":"NONE","risk":"low"}"#.into();
    }
    Ok(ModelAnswer {
        text,
        latency_ms: 680 + (model_index as u64 * 410) + (task.id.len() as u64 * 7),
        tokens_in: Some(180 + task.prompt.len() as u64 / 4),
        tokens_out: Some(70 + model_index as u64 * 9),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_url_appends_the_completions_path() {
        assert_eq!(chat_url("http://127.0.0.1:11434/v1"), "http://127.0.0.1:11434/v1/chat/completions");
    }

    #[test]
    fn chat_url_tolerates_a_trailing_slash() {
        assert_eq!(chat_url("http://127.0.0.1:11434/v1/"), "http://127.0.0.1:11434/v1/chat/completions");
    }
}
