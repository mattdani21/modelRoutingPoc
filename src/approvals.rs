//! Counter-signed live approvals.
//!
//! `approved_for_live` in the model catalog is a request, not an authority. A
//! model may only run against a live endpoint when a separate approvals ledger
//! (`config/approvals.yaml`) carries a matching record for the exact model ID
//! and artifact digest, naming the approver and the approval time. Keeping the
//! approval in a second file enforces separation of duties: a person who edits
//! the model catalog cannot promote a model to live on their own.

use std::{collections::HashMap, path::Path};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::domain::ModelCatalog;

#[derive(Debug, Deserialize)]
struct ApprovalFile {
    #[serde(default)]
    approvals: Vec<ApprovalEntry>,
}

#[derive(Clone, Debug, Deserialize)]
struct ApprovalEntry {
    model_id: String,
    artifact_digest: String,
    approver: String,
    approved_at: String,
}

pub struct ApprovalLedger {
    by_model: HashMap<String, ApprovalEntry>,
}

impl ApprovalLedger {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self { by_model: HashMap::new() });
        }
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("Could not read approvals file: {}", path.display()))?;
        let parsed: ApprovalFile = serde_yaml::from_str(&raw).context("Could not parse the approvals file")?;
        let mut by_model = HashMap::new();
        for entry in parsed.approvals {
            if entry.approver.trim().is_empty() || entry.approved_at.trim().is_empty() {
                bail!("Approval for {} must name an approver and an approval time", entry.model_id);
            }
            if entry.artifact_digest.trim().is_empty() {
                bail!("Approval for {} must record an artifact digest", entry.model_id);
            }
            if by_model.insert(entry.model_id.clone(), entry.clone()).is_some() {
                bail!("Duplicate approval record for model {}", entry.model_id);
            }
        }
        Ok(Self { by_model })
    }

    /// Every live-approved model must have a matching signed approval whose
    /// artifact digest equals the digest declared in the catalog.
    pub fn verify(&self, models: &ModelCatalog) -> Result<()> {
        for model in &models.models {
            if !model.approved_for_live {
                continue;
            }
            let digest = model
                .artifact_digest
                .as_deref()
                .context("A live-approved model must declare an artifact digest")?;
            match self.by_model.get(&model.id) {
                None => bail!(
                    "Model {} is marked approved_for_live but has no signed approval in the approvals ledger",
                    model.id
                ),
                Some(entry) if entry.artifact_digest != digest => bail!(
                    "Model {} approval digest does not match the catalog digest. Re-approve the exact artifact.",
                    model.id
                ),
                Some(_) => {}
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{DataClassification, ModelConfig};

    fn model(approved: bool, digest: Option<&str>) -> ModelConfig {
        ModelConfig {
            id: "m1".into(),
            display_name: "M1".into(),
            provider: "local".into(),
            model: "m1".into(),
            base_url: "http://127.0.0.1:1/v1".into(),
            api_key_env: None,
            quantisation: "Q8".into(),
            runtime: "llama.cpp".into(),
            hardware: "sandbox".into(),
            input_cost_per_million: None,
            output_cost_per_million: None,
            cost_basis: None,
            license: "Apache-2.0".into(),
            registry_source: "hf".into(),
            artifact_digest: digest.map(str::to_owned),
            approved_for_live: approved,
            allowed_data: vec![DataClassification::Internal],
            enabled: true,
        }
    }

    fn ledger(entry: Option<ApprovalEntry>) -> ApprovalLedger {
        let mut by_model = HashMap::new();
        if let Some(entry) = entry {
            by_model.insert(entry.model_id.clone(), entry);
        }
        ApprovalLedger { by_model }
    }

    fn entry(digest: &str) -> ApprovalEntry {
        ApprovalEntry {
            model_id: "m1".into(),
            artifact_digest: digest.into(),
            approver: "Risk Owner".into(),
            approved_at: "2026-07-22T10:00:00Z".into(),
        }
    }

    #[test]
    fn live_model_without_approval_is_rejected() {
        let models = ModelCatalog { version: "v".into(), models: vec![model(true, Some("sha256:a"))] };
        assert!(ledger(None).verify(&models).is_err());
    }

    #[test]
    fn digest_mismatch_is_rejected() {
        let models = ModelCatalog { version: "v".into(), models: vec![model(true, Some("sha256:a"))] };
        assert!(ledger(Some(entry("sha256:b"))).verify(&models).is_err());
    }

    #[test]
    fn matching_approval_passes() {
        let models = ModelCatalog { version: "v".into(), models: vec![model(true, Some("sha256:a"))] };
        assert!(ledger(Some(entry("sha256:a"))).verify(&models).is_ok());
    }

    #[test]
    fn unapproved_model_needs_no_record() {
        let models = ModelCatalog { version: "v".into(), models: vec![model(false, None)] };
        assert!(ledger(None).verify(&models).is_ok());
    }
}
