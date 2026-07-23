//! Champion registry.
//!
//! Each task class keeps one approved champion model. A new model is a
//! challenger. The POC records champion status on every run and flags a
//! challenger that regresses against the champion inside the same evaluation.
//! Promotion is not automatic: the plan requires two consecutive passing runs
//! and an owner's approval before the champion changes.

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ChampionRegistry {
    #[serde(default)]
    pub champions: Vec<ChampionEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ChampionEntry {
    pub task_class: String,
    pub model_id: String,
    #[serde(default)]
    pub since: Option<String>,
}

impl ChampionRegistry {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("Could not read champion file: {}", path.display()))?;
        serde_yaml::from_str(&raw).context("Could not parse the champion file")
    }

    /// The champion model for a task class, if one is registered.
    pub fn champion_for(&self, task_class: &str) -> Option<&str> {
        self.champions
            .iter()
            .find(|entry| entry.task_class == task_class)
            .map(|entry| entry.model_id.as_str())
    }

    pub fn is_champion(&self, task_class: &str, model_id: &str) -> bool {
        self.champion_for(task_class) == Some(model_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> ChampionRegistry {
        ChampionRegistry {
            champions: vec![ChampionEntry {
                task_class: "log_extraction".into(),
                model_id: "qwen36-27b-q8".into(),
                since: Some("2026-07-20".into()),
            }],
        }
    }

    #[test]
    fn champion_is_matched_by_class() {
        assert_eq!(registry().champion_for("log_extraction"), Some("qwen36-27b-q8"));
        assert_eq!(registry().champion_for("unknown_class"), None);
    }

    #[test]
    fn is_champion_checks_model() {
        assert!(registry().is_champion("log_extraction", "qwen36-27b-q8"));
        assert!(!registry().is_champion("log_extraction", "gpt-oss-20b-local"));
    }
}
