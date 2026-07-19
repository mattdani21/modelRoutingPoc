use std::{fs, path::Path};

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
