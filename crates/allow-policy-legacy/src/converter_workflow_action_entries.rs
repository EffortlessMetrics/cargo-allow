use allow_core::{AllowEntry, FindingKind, Selector, normalize_path};
use std::path::PathBuf;

use crate::converter_workflow_support::{lifecycle_from_workflow_rule, slug_id};
use crate::findings::workflow_action_symbol;
use crate::types::LegacyWorkflowRule;

pub(crate) fn workflow_action_entry(rule: &LegacyWorkflowRule, action: &str) -> AllowEntry {
    let path = normalize_path(&rule.path);
    let symbol = workflow_action_symbol(&path, action);
    AllowEntry {
        id: format!("workflow-action-{}--{}", slug_id(&path), slug_id(action)),
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

fn workflow_action_evidence(rule: &LegacyWorkflowRule, action: &str) -> Vec<String> {
    let mut evidence = rule.evidence.clone();
    evidence.push(format!("external_action:{action}"));
    evidence
}
