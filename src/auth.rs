//! Token-based access control for the API surface.
//!
//! Access is defined in an optional `config/access.yaml`. Each token maps to a
//! named principal and a role. When the file is absent or lists no tokens the
//! control plane runs in open development mode: this is only safe on the
//! loopback interface and the service warns loudly at startup. As soon as one
//! token is defined every `/api` call except the health probe must present a
//! valid token with a sufficient role.

use std::{collections::HashMap, path::Path};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

/// Roles form a strict hierarchy: an operator can also review and read, and a
/// reviewer can also read.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Viewer,
    Reviewer,
    Operator,
}

impl Role {
    fn level(self) -> u8 {
        match self {
            Role::Viewer => 1,
            Role::Reviewer => 2,
            Role::Operator => 3,
        }
    }

    pub fn allows(self, required: Role) -> bool {
        self.level() >= required.level()
    }
}

#[derive(Clone, Debug)]
pub struct Principal {
    pub display_name: String,
    pub role: Role,
}

#[derive(Debug, Deserialize)]
struct AccessFile {
    #[serde(default)]
    tokens: Vec<TokenEntry>,
}

#[derive(Debug, Deserialize)]
struct TokenEntry {
    token: String,
    display_name: String,
    role: Role,
}

/// The outcome of an authorization check.
pub enum AuthDecision {
    Allow(Option<Principal>),
    Unauthenticated,
    Forbidden,
}

pub struct AccessControl {
    tokens: HashMap<String, Principal>,
}

impl AccessControl {
    /// Load access control from an optional file. A missing file yields open
    /// development mode.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self { tokens: HashMap::new() });
        }
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("Could not read access file: {}", path.display()))?;
        let parsed: AccessFile = serde_yaml::from_str(&raw).context("Could not parse the access file")?;
        let mut tokens = HashMap::new();
        for entry in parsed.tokens {
            if entry.token.trim().len() < 16 {
                bail!("Access token for {} must be at least 16 characters", entry.display_name);
            }
            if tokens.insert(entry.token, Principal { display_name: entry.display_name.clone(), role: entry.role }).is_some() {
                bail!("Duplicate access token for {}", entry.display_name);
            }
        }
        Ok(Self { tokens })
    }

    /// True when at least one token is configured. When false the service is in
    /// open development mode.
    pub fn enforced(&self) -> bool {
        !self.tokens.is_empty()
    }

    pub fn principal_count(&self) -> usize {
        self.tokens.len()
    }

    /// Decide whether a presented token may act with the required role.
    pub fn decide(&self, presented: Option<&str>, required: Role) -> AuthDecision {
        if !self.enforced() {
            return AuthDecision::Allow(None);
        }
        let Some(token) = presented else {
            return AuthDecision::Unauthenticated;
        };
        match self.tokens.get(token) {
            None => AuthDecision::Unauthenticated,
            Some(principal) if principal.role.allows(required) => AuthDecision::Allow(Some(principal.clone())),
            Some(_) => AuthDecision::Forbidden,
        }
    }
}

/// The role required to call a given method and path. `None` means the route is
/// open (the health probe and static assets).
pub fn required_role(method: &str, path: &str) -> Option<Role> {
    if !path.starts_with("/api/") || path == "/api/health" {
        return None;
    }
    match method {
        "GET" | "HEAD" => Some(Role::Viewer),
        _ if path.ends_with("/review") => Some(Role::Reviewer),
        _ => Some(Role::Operator),
    }
}

/// Extract a bearer token from an `Authorization` header value.
pub fn bearer(header: Option<&str>) -> Option<&str> {
    let value = header?;
    value
        .strip_prefix("Bearer ")
        .or_else(|| value.strip_prefix("bearer "))
        .map(str::trim)
        .filter(|token| !token.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn control() -> AccessControl {
        let mut tokens = HashMap::new();
        tokens.insert("operator-token-abcdef".into(), Principal { display_name: "Op".into(), role: Role::Operator });
        tokens.insert("viewer-token-abcdef123".into(), Principal { display_name: "View".into(), role: Role::Viewer });
        AccessControl { tokens }
    }

    #[test]
    fn open_mode_allows_everything() {
        let control = AccessControl { tokens: HashMap::new() };
        assert!(matches!(control.decide(None, Role::Operator), AuthDecision::Allow(None)));
    }

    #[test]
    fn missing_token_is_unauthenticated() {
        assert!(matches!(control().decide(None, Role::Viewer), AuthDecision::Unauthenticated));
    }

    #[test]
    fn viewer_cannot_operate() {
        assert!(matches!(control().decide(Some("viewer-token-abcdef123"), Role::Operator), AuthDecision::Forbidden));
    }

    #[test]
    fn operator_can_review_and_read() {
        assert!(matches!(control().decide(Some("operator-token-abcdef"), Role::Reviewer), AuthDecision::Allow(Some(_))));
        assert!(matches!(control().decide(Some("operator-token-abcdef"), Role::Viewer), AuthDecision::Allow(Some(_))));
    }

    #[test]
    fn role_hierarchy_is_ordered() {
        assert!(Role::Operator.allows(Role::Viewer));
        assert!(!Role::Viewer.allows(Role::Reviewer));
    }

    #[test]
    fn required_role_maps_routes() {
        assert_eq!(required_role("GET", "/api/runs"), Some(Role::Viewer));
        assert_eq!(required_role("POST", "/api/runs"), Some(Role::Operator));
        assert_eq!(required_role("POST", "/api/runs/abc/review"), Some(Role::Reviewer));
        assert_eq!(required_role("GET", "/api/health"), None);
        assert_eq!(required_role("GET", "/index.html"), None);
    }

    #[test]
    fn bearer_parsing() {
        assert_eq!(bearer(Some("Bearer abc")), Some("abc"));
        assert_eq!(bearer(Some("nonsense")), None);
        assert_eq!(bearer(None), None);
    }
}
