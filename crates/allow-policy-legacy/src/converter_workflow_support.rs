use allow_core::{Lifecycle, stable_hash_hex};

use crate::converter_lifecycle_support::lifecycle_from_legacy_fields;
use crate::types::LegacyWorkflowRule;

pub(crate) fn lifecycle_from_workflow_rule(rule: &LegacyWorkflowRule) -> Lifecycle {
    lifecycle_from_legacy_fields(
        rule.created.clone(),
        rule.review_after.clone(),
        rule.expires.clone(),
    )
}

pub(crate) fn slug_id(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

pub(crate) fn path_hash_id(path: &str) -> String {
    let hash = stable_hash_hex(path);
    hash.strip_prefix("fnv1a64:")
        .unwrap_or(hash.as_str())
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_from_workflow_rule_preserves_explicit_lifecycle_fields() {
        let rule = workflow_rule(Some("2026-01-02"), Some("2026-07-02"), Some("2027-01-02"));

        let lifecycle = lifecycle_from_workflow_rule(&rule);

        assert_eq!(lifecycle.created.as_deref(), Some("2026-01-02"));
        assert_eq!(lifecycle.review_after.as_deref(), Some("2026-07-02"));
        assert_eq!(lifecycle.expires.as_deref(), Some("2027-01-02"));
    }

    #[test]
    fn lifecycle_from_workflow_rule_uses_created_as_review_when_expiry_is_never() {
        let rule = workflow_rule(Some("2026-03-04"), None, Some("never"));

        let lifecycle = lifecycle_from_workflow_rule(&rule);

        assert_eq!(lifecycle.created.as_deref(), Some("2026-03-04"));
        assert_eq!(lifecycle.review_after.as_deref(), Some("2026-03-04"));
        assert_eq!(lifecycle.expires.as_deref(), Some("never"));
    }

    #[test]
    fn slug_id_lowercases_ascii_and_collapses_separator_runs() {
        assert_eq!(
            slug_id(".github/workflows/CI.yml::actions/Checkout@v4"),
            "github-workflows-ci-yml-actions-checkout-v4"
        );
    }

    #[test]
    fn slug_id_trims_edges_and_drops_inputs_without_ascii_alphanumerics() {
        assert_eq!(slug_id("---Release Lane---"), "release-lane");
        assert_eq!(slug_id("///"), "");
    }

    fn workflow_rule(
        created: Option<&str>,
        review_after: Option<&str>,
        expires: Option<&str>,
    ) -> LegacyWorkflowRule {
        LegacyWorkflowRule {
            path: ".github\\workflows\\ci.yml".to_string(),
            owner: "platform".to_string(),
            reason: "required CI lane".to_string(),
            permissions: Vec::new(),
            secrets_used: Vec::new(),
            external_actions: Vec::new(),
            duplicate_of_lane: None,
            evidence: Vec::new(),
            created: created.map(str::to_string),
            review_after: review_after.map(str::to_string),
            expires: expires.map(str::to_string),
        }
    }
}
