use allow_core::{AllowEntry, FindingKind, Selector, normalize_path};
use std::path::PathBuf;

use crate::converter_workflow_support::{lifecycle_from_workflow_rule, path_hash_id, slug_id};
use crate::findings::workflow_action_symbol;
use crate::types::LegacyWorkflowRule;

pub(crate) fn workflow_action_entry(rule: &LegacyWorkflowRule, action: &str) -> AllowEntry {
    let path = normalize_path(&rule.path);
    let symbol = workflow_action_symbol(&path, action);
    AllowEntry {
        id: workflow_action_id(&path, action),
        kind: FindingKind::PolicyException,
        family: Some("workflow_external_action".to_string()),
        path: Some(PathBuf::from(&path)),
        glob: None,
        owner: rule.owner.clone(),
        classification: "workflow_external_action".to_string(),
        reason: rule.reason.clone(),
        evidence: workflow_action_evidence(rule, action),
        links: vec![format!("legacy-policy:workflow:{path}")],
        occurrence_limit: None,
        lifecycle: lifecycle_from_workflow_rule(rule),
        selector: Selector {
            ast_kind: Some("github_action_uses".to_string()),
            symbol: Some(symbol),
            target_fingerprint: Some(format!("action:{action}")),
            glob: Some(path),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn workflow_action_id(path: &str, action: &str) -> String {
    format!(
        "workflow-action-{}-{}--{}",
        slug_id(path),
        path_hash_id(path),
        slug_id(action)
    )
}

fn workflow_action_evidence(rule: &LegacyWorkflowRule, action: &str) -> Vec<String> {
    let mut evidence = rule.evidence.clone();
    evidence.push(format!(
        "legacy-policy:workflow:{}",
        normalize_path(&rule.path)
    ));
    evidence.push(format!("external_action:{action}"));
    evidence
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn workflow_action_entry_maps_legacy_rule_to_policy_exception() {
        let rule = workflow_rule();
        let entry = workflow_action_entry(&rule, "actions/checkout@v4");

        assert_eq!(
            entry.id,
            "workflow-action-github-workflows-ci-yml-a8f3817ec78236ac--actions-checkout-v4"
        );
        assert_eq!(entry.kind, FindingKind::PolicyException);
        assert_eq!(entry.family.as_deref(), Some("workflow_external_action"));
        assert_eq!(entry.path, Some(PathBuf::from(".github/workflows/ci.yml")));
        assert_eq!(entry.glob, None);
        assert_eq!(entry.owner, "platform");
        assert_eq!(entry.classification, "workflow_external_action");
        assert_eq!(entry.reason, "required CI lane");
        assert_eq!(
            entry.evidence,
            vec![
                "docs/ci.md".to_string(),
                "legacy-policy:workflow:.github/workflows/ci.yml".to_string(),
                "external_action:actions/checkout@v4".to_string(),
            ]
        );
        assert_eq!(
            entry.links,
            vec!["legacy-policy:workflow:.github/workflows/ci.yml".to_string()]
        );
        assert_eq!(entry.occurrence_limit, None);
        assert_eq!(entry.lifecycle.created.as_deref(), Some("2026-01-02"));
        assert_eq!(entry.lifecycle.review_after.as_deref(), Some("2026-07-02"));
        assert_eq!(entry.lifecycle.expires.as_deref(), Some("never"));
        assert_eq!(
            entry.selector.ast_kind.as_deref(),
            Some("github_action_uses")
        );
        assert_eq!(
            entry.selector.symbol.as_deref(),
            Some(".github/workflows/ci.yml uses actions/checkout@v4")
        );
        assert_eq!(
            entry.selector.target_fingerprint.as_deref(),
            Some("action:actions/checkout@v4")
        );
        assert_eq!(
            entry.selector.glob.as_deref(),
            Some(".github/workflows/ci.yml")
        );
        assert_eq!(entry.last_seen, None);
    }

    #[test]
    fn workflow_action_evidence_keeps_minimal_rule_evidence() {
        let rule = LegacyWorkflowRule {
            path: ".github\\workflows\\release.yml".to_string(),
            owner: "release".to_string(),
            reason: "publish lane".to_string(),
            permissions: Vec::new(),
            secrets_used: Vec::new(),
            external_actions: Vec::new(),
            duplicate_of_lane: None,
            evidence: Vec::new(),
            created: None,
            review_after: None,
            expires: None,
        };

        assert_eq!(
            workflow_action_evidence(&rule, "dtolnay/rust-toolchain@stable"),
            vec![
                "legacy-policy:workflow:.github/workflows/release.yml".to_string(),
                "external_action:dtolnay/rust-toolchain@stable".to_string(),
            ]
        );
    }

    #[test]
    fn workflow_action_entry_disambiguates_same_slug_workflow_paths() {
        let mut hyphen = workflow_rule();
        hyphen.path = ".github/workflows/ci-main.yml".to_string();
        let mut underscore = workflow_rule();
        underscore.path = ".github/workflows/ci_main.yml".to_string();

        let first = workflow_action_entry(&hyphen, "actions/checkout@v4");
        let second = workflow_action_entry(&underscore, "actions/checkout@v4");

        assert_ne!(first.id, second.id);
        assert!(
            first
                .id
                .starts_with("workflow-action-github-workflows-ci-main-yml-")
        );
        assert!(
            second
                .id
                .starts_with("workflow-action-github-workflows-ci-main-yml-")
        );
        assert_ne!(
            first.id.to_ascii_lowercase(),
            second.id.to_ascii_lowercase()
        );
    }

    fn workflow_rule() -> LegacyWorkflowRule {
        LegacyWorkflowRule {
            path: ".github\\workflows\\ci.yml".to_string(),
            owner: "platform".to_string(),
            reason: "required CI lane".to_string(),
            permissions: vec!["contents:read".to_string(), "id-token:write".to_string()],
            secrets_used: vec!["CARGO_REGISTRY_TOKEN".to_string()],
            external_actions: vec!["actions/checkout@v4".to_string()],
            duplicate_of_lane: Some("ci-shadow".to_string()),
            evidence: vec!["docs/ci.md".to_string()],
            created: Some("2026-01-02".to_string()),
            review_after: Some("2026-07-02".to_string()),
            expires: Some("never".to_string()),
        }
    }
}
