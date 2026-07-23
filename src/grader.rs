use anyhow::{Context, Result};
use regex::Regex;
use serde_json::Value;

use crate::domain::GraderSpec;

pub fn grade(spec: &GraderSpec, answer: &str) -> Result<(bool, String)> {
    match spec {
        GraderSpec::Exact { expected } => {
            let pass = answer.trim() == expected.trim();
            Ok((pass, format!("Exact match: {pass}")))
        }
        GraderSpec::ContainsAll { values } => {
            let lower = answer.to_lowercase();
            let missing: Vec<&String> = values
                .iter()
                .filter(|value| !lower.contains(&value.to_lowercase()))
                .collect();
            Ok((
                missing.is_empty(),
                if missing.is_empty() {
                    "All required terms are present".into()
                } else {
                    format!("Missing terms: {}", missing.iter().map(|v| v.as_str()).collect::<Vec<_>>().join(", "))
                },
            ))
        }
        GraderSpec::Regex { pattern } => {
            let regex = Regex::new(pattern).context("The grader regular expression is invalid")?;
            let pass = regex.is_match(answer);
            Ok((pass, format!("Regular expression match: {pass}")))
        }
        GraderSpec::JsonFields { fields } => {
            let value: Value = serde_json::from_str(answer).context("The answer is not valid JSON")?;
            let missing: Vec<&String> = fields.iter().filter(|field| value.get(field.as_str()).is_none()).collect();
            Ok((
                missing.is_empty(),
                if missing.is_empty() {
                    "The required JSON fields are present".into()
                } else {
                    format!("Missing JSON fields: {}", missing.iter().map(|v| v.as_str()).collect::<Vec<_>>().join(", "))
                },
            ))
        }
        GraderSpec::JsonEquals { expected } => {
            let actual: Value = serde_json::from_str(answer).context("The answer is not valid JSON")?;
            let pass = &actual == expected;
            Ok((pass, format!("Expected JSON value match: {pass}")))
        }
        GraderSpec::ExactLines { expected, ignore_order } => {
            let mut actual: Vec<String> = answer
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect();
            let mut expected = expected.clone();
            if *ignore_order {
                actual.sort();
                expected.sort();
            }
            let pass = actual == expected;
            Ok((pass, format!("Exact line match: {pass}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_all_is_case_insensitive() {
        let spec = GraderSpec::ContainsAll { values: vec!["error".into(), "policy".into()] };
        assert!(grade(&spec, "POLICY error found").unwrap().0);
    }

    #[test]
    fn json_fields_rejects_missing_field() {
        let spec = GraderSpec::JsonFields { fields: vec!["risk".into(), "action".into()] };
        assert!(!grade(&spec, r#"{"risk":"low"}"#).unwrap().0);
    }

    #[test]
    fn json_equals_rejects_correct_fields_with_wrong_values() {
        let spec = GraderSpec::JsonEquals {
            expected: serde_json::json!({"risk":"high","action":"correct"}),
        };
        assert!(!grade(&spec, r#"{"risk":"low","action":"ignore"}"#).unwrap().0);
    }

    #[test]
    fn exact_lines_rejects_false_positive_lines() {
        let spec = GraderSpec::ExactLines {
            expected: vec!["ERROR one".into(), "ERROR two".into()],
            ignore_order: false,
        };
        assert!(!grade(&spec, "ERROR one\nERROR wrong\nERROR two").unwrap().0);
    }
}
