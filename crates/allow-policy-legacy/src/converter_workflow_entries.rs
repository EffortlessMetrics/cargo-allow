use allow_core::AllowEntry;

use crate::converter_workflow_action_entries::workflow_action_entry;
use crate::converter_workflow_file_entries::workflow_file_entry;
use crate::types::LegacyWorkflowRule;

pub(crate) fn entries_from_workflow_rule(rule: &LegacyWorkflowRule) -> Vec<AllowEntry> {
    let mut entries = Vec::with_capacity(rule.external_actions.len() + 1);
    entries.push(workflow_file_entry(rule));
    entries.extend(
        rule.external_actions
            .iter()
            .map(|action| workflow_action_entry(rule, action)),
    );
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::FindingKind;
    use std::path::Path;

    #[test]
    fn workflow_rule_expands_file_entry_then_external_actions() {
        let rule = workflow_rule(vec![
            "actions/checkout@v6.0.2".to_string(),
            "dtolnay/rust-toolchain@stable".to_string(),
        ]);

        let entries = entries_from_workflow_rule(&rule);

        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].family.as_deref(), Some("github_workflow"));
        assert_eq!(
            entries[0].path.as_deref(),
            Some(Path::new(".github/workflows/ci.yml"))
        );
        assert_eq!(entries[0].kind, FindingKind::PolicyException);
        assert_eq!(
            entries[1].family.as_deref(),
            Some("workflow_external_action")
        );
        assert_eq!(
            entries[1].selector.target_fingerprint.as_deref(),
            Some("action:actions/checkout@v6.0.2")
        );
        assert_eq!(
            entries[2].selector.target_fingerprint.as_deref(),
            Some("action:dtolnay/rust-toolchain@stable")
        );
    }

    #[test]
    fn workflow_rule_without_actions_only_emits_file_entry() {
        let entries = entries_from_workflow_rule(&workflow_rule(Vec::new()));

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].family.as_deref(), Some("github_workflow"));
    }

    fn workflow_rule(external_actions: Vec<String>) -> LegacyWorkflowRule {
        LegacyWorkflowRule {
            path: ".github\\workflows\\ci.yml".to_string(),
            owner: "release/ci".to_string(),
            reason: "Primary CI lane.".to_string(),
            permissions: vec!["contents:read".to_string()],
            secrets_used: vec!["CARGO_REGISTRY_TOKEN".to_string()],
            external_actions,
            duplicate_of_lane: Some("ci-shadow".to_string()),
            evidence: vec!["doc:docs/ci.md".to_string()],
            created: Some("2026-05-09".to_string()),
            review_after: Some("2026-09-09".to_string()),
            expires: Some("never".to_string()),
        }
    }
}
