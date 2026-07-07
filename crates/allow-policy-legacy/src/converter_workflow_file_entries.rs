use allow_core::{AllowEntry, FindingKind, Selector, normalize_path};
use std::path::PathBuf;

use crate::converter_workflow_support::{lifecycle_from_workflow_rule, path_hash_id, slug_id};
use crate::types::LegacyWorkflowRule;

pub(crate) fn workflow_file_entry(rule: &LegacyWorkflowRule) -> AllowEntry {
    let path = normalize_path(&rule.path);
    AllowEntry {
        id: workflow_file_id(&path),
        kind: FindingKind::PolicyException,
        family: Some("github_workflow".to_string()),
        path: Some(PathBuf::from(&path)),
        glob: None,
        owner: rule.owner.clone(),
        classification: "github_workflow".to_string(),
        reason: rule.reason.clone(),
        evidence: workflow_evidence(rule),
        links: vec![format!("legacy-policy:workflow:{path}")],
        occurrence_limit: None,
        lifecycle: lifecycle_from_workflow_rule(rule),
        selector: Selector {
            ast_kind: Some("github_workflow".to_string()),
            symbol: Some(path.clone()),
            glob: Some(path),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn workflow_file_id(path: &str) -> String {
    format!("workflow-file-{}-{}", slug_id(path), path_hash_id(path))
}

fn workflow_evidence(rule: &LegacyWorkflowRule) -> Vec<String> {
    let mut evidence = rule.evidence.clone();
    evidence.push(format!(
        "legacy-policy:workflow:{}",
        normalize_path(&rule.path)
    ));
    evidence.extend(
        rule.permissions
            .iter()
            .map(|permission| format!("permission:{permission}")),
    );
    evidence.extend(
        rule.secrets_used
            .iter()
            .map(|secret| format!("secret:{secret}")),
    );
    if let Some(lane) = &rule.duplicate_of_lane {
        evidence.push(format!("duplicate_of_lane:{lane}"));
    }
    evidence
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn workflow_file_entry_maps_legacy_rule_to_policy_exception() {
        let rule = workflow_rule();
        let entry = workflow_file_entry(&rule);

        assert_eq!(
            entry.id,
            "workflow-file-github-workflows-ci-yml-a8f3817ec78236ac"
        );
        assert_eq!(entry.kind, FindingKind::PolicyException);
        assert_eq!(entry.family.as_deref(), Some("github_workflow"));
        assert_eq!(entry.path, Some(PathBuf::from(".github/workflows/ci.yml")));
        assert_eq!(entry.glob, None);
        assert_eq!(entry.owner, "platform");
        assert_eq!(entry.classification, "github_workflow");
        assert_eq!(entry.reason, "required CI lane");
        assert_eq!(
            entry.evidence,
            vec![
                "docs/ci.md".to_string(),
                "legacy-policy:workflow:.github/workflows/ci.yml".to_string(),
                "permission:contents:read".to_string(),
                "permission:id-token:write".to_string(),
                "secret:CARGO_REGISTRY_TOKEN".to_string(),
                "duplicate_of_lane:ci-shadow".to_string(),
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
        assert_eq!(entry.selector.ast_kind.as_deref(), Some("github_workflow"));
        assert_eq!(
            entry.selector.symbol.as_deref(),
            Some(".github/workflows/ci.yml")
        );
        assert_eq!(
            entry.selector.glob.as_deref(),
            Some(".github/workflows/ci.yml")
        );
        assert_eq!(entry.selector.target_fingerprint, None);
        assert_eq!(entry.last_seen, None);
    }

    #[test]
    fn workflow_evidence_keeps_minimal_rule_evidence() {
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
            workflow_evidence(&rule),
            vec!["legacy-policy:workflow:.github/workflows/release.yml".to_string()]
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
