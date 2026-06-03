use allow_core::{AllowEntry, FindingKind, Selector, normalize_path};
use std::path::PathBuf;

use crate::converter_workflow_support::{lifecycle_from_workflow_rule, slug_id};
use crate::types::LegacyWorkflowRule;

pub(crate) fn workflow_file_entry(rule: &LegacyWorkflowRule) -> AllowEntry {
    let path = normalize_path(&rule.path);
    AllowEntry {
        id: format!("workflow-file-{}", slug_id(&path)),
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
