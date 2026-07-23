use std::{collections::HashSet, fs, path::Path};

use anyhow::{Context, Result};

use crate::domain::{BenchmarkCatalog, ModelCatalog};

pub fn load_benchmarks(path: &Path) -> Result<BenchmarkCatalog> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("Could not read benchmark file: {}", path.display()))?;
    serde_yaml::from_str(&raw).context("Could not parse the benchmark file")
}

pub fn load_models(path: &Path) -> Result<ModelCatalog> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("Could not read model file: {}", path.display()))?;
    serde_yaml::from_str(&raw).context("Could not parse the model file")
}

pub fn validate(benchmarks: &BenchmarkCatalog, models: &ModelCatalog) -> Result<()> {
    let mut task_ids = HashSet::new();
    for task in &benchmarks.tasks {
        if task.id.trim().is_empty() || !task_ids.insert(&task.id) {
            anyhow::bail!("Benchmark task IDs must be non-empty and unique");
        }
        if task.prompt_version.trim().is_empty() {
            anyhow::bail!("Task {} has no prompt version", task.id);
        }
        if let Some(score) = task.quality_gate.minimum_human_score {
            if !(1..=5).contains(&score) {
                anyhow::bail!("Task {} has an invalid human score gate", task.id);
            }
        }
    }

    let mut model_ids = HashSet::new();
    for model in &models.models {
        if model.id.trim().is_empty() || !model_ids.insert(&model.id) {
            anyhow::bail!("Model IDs must be non-empty and unique");
        }
        match model.provider.as_str() {
            "local" => {
                let local_url = model.base_url.starts_with("http://127.0.0.1")
                    || model.base_url.starts_with("http://localhost")
                    || model.base_url.starts_with("http://[::1]");
                if !local_url {
                    anyhow::bail!("Local model {} must use a loopback endpoint in this POC", model.id);
                }
            }
            "hosted" => {
                if !model.base_url.starts_with("https://") {
                    anyhow::bail!("Hosted model {} must use HTTPS", model.id);
                }
            }
            _ => anyhow::bail!("Model {} has an unknown provider type", model.id),
        }
        if model.approved_for_live && model.artifact_digest.is_none() {
            anyhow::bail!("Live-approved model {} must have an artifact digest", model.id);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_catalogs_are_valid() {
        let benchmarks = load_benchmarks(Path::new("config/benchmarks.yaml")).unwrap();
        let models = load_models(Path::new("config/models.yaml")).unwrap();
        validate(&benchmarks, &models).unwrap();
    }
}
