use allow_core::{AllowEntry, FindingKind, Selector, normalize_path};
use std::path::{Path, PathBuf};

use crate::converter_lifecycle_support::lifecycle_from_legacy_fields;
use crate::converter_process_network_support::{
    process_fingerprint, process_scope, process_symbol,
};
use crate::types::LegacyProcessRule;

pub(crate) fn entry_from_process_rule(rule: &LegacyProcessRule) -> AllowEntry {
    let scope = process_scope(rule);
    let symbol = process_symbol(rule);
    AllowEntry {
        id: rule.id.clone(),
        kind: FindingKind::PolicyException,
        family: Some("process_spawn".to_string()),
        path: Some(PathBuf::from(&scope)),
        glob: None,
        owner: rule.owner.clone(),
        classification: if rule.network_reach {
            "network_process".to_string()
        } else {
            "local_process".to_string()
        },
        reason: rule.reason.clone(),
        evidence: process_evidence(rule),
        links: vec![format!("legacy-policy:{}", rule.id)],
        occurrence_limit: None,
        lifecycle: lifecycle_from_legacy_fields(
            rule.created.clone(),
            rule.review_after.clone(),
            rule.expires.clone(),
        ),
        selector: Selector {
            ast_kind: Some("process_spawn".to_string()),
            symbol: Some(symbol.clone()),
            target_fingerprint: Some(process_fingerprint(rule)),
            glob: Some(scope),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn process_evidence(rule: &LegacyProcessRule) -> Vec<String> {
    let mut evidence = rule.evidence.clone();
    evidence.push(format!("legacy-policy:{}", rule.id));
    evidence.push(format!("binary:{}", rule.binary));
    evidence.push(format!("argv_shape:{}", rule.argv_shape.join(" ")));
    evidence.push(format!("network_reach:{}", rule.network_reach));
    evidence.extend(
        rule.called_by
            .iter()
            .map(|path| format!("called_by:{}", normalize_path(Path::new(path)))),
    );
    evidence
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_rule_without_callers_uses_policy_scope_and_local_classification() {
        let rule = LegacyProcessRule {
            id: "proc-local-tool".to_string(),
            binary: "cargo".to_string(),
            argv_shape: Vec::new(),
            network_reach: false,
            called_by: Vec::new(),
            owner: "build".to_string(),
            reason: "Local build tool fixture.".to_string(),
            evidence: Vec::new(),
            created: Some("2026-03-01".to_string()),
            review_after: None,
            expires: Some("never".to_string()),
        };

        let entry = entry_from_process_rule(&rule);

        assert_eq!(entry.id, "proc-local-tool");
        assert_eq!(entry.kind, FindingKind::PolicyException);
        assert_eq!(entry.family.as_deref(), Some("process_spawn"));
        assert_eq!(
            entry.path,
            Some(PathBuf::from("policy/process-allowlist.toml"))
        );
        assert_eq!(entry.glob, None);
        assert_eq!(entry.owner, "build");
        assert_eq!(entry.classification, "local_process");
        assert_eq!(entry.reason, "Local build tool fixture.");
        assert_eq!(
            entry.evidence,
            vec![
                "legacy-policy:proc-local-tool".to_string(),
                "binary:cargo".to_string(),
                "argv_shape:".to_string(),
                "network_reach:false".to_string(),
            ]
        );
        assert_eq!(
            entry.links,
            vec!["legacy-policy:proc-local-tool".to_string()]
        );
        assert_eq!(entry.occurrence_limit, None);
        assert_eq!(entry.lifecycle.created.as_deref(), Some("2026-03-01"));
        assert_eq!(entry.lifecycle.review_after.as_deref(), Some("2026-03-01"));
        assert_eq!(entry.lifecycle.expires.as_deref(), Some("never"));
        assert_eq!(entry.selector.ast_kind.as_deref(), Some("process_spawn"));
        assert_eq!(entry.selector.symbol.as_deref(), Some("cargo"));
        assert_eq!(
            entry.selector.target_fingerprint.as_deref(),
            Some("process:cargo")
        );
        assert_eq!(
            entry.selector.glob.as_deref(),
            Some("policy/process-allowlist.toml")
        );
        assert!(entry.last_seen.is_none());
    }

    #[test]
    fn process_rule_with_callers_records_network_metadata_and_normalized_evidence() {
        let rule = LegacyProcessRule {
            id: "proc-cargo-install".to_string(),
            binary: "cargo".to_string(),
            argv_shape: vec![
                "install".to_string(),
                "cargo-deny".to_string(),
                "--locked".to_string(),
            ],
            network_reach: true,
            called_by: vec![
                ".github\\workflows\\ci.yml".to_string(),
                "scripts\\install.ps1".to_string(),
            ],
            owner: "ci".to_string(),
            reason: "CI installs a pinned tool.".to_string(),
            evidence: vec!["doc:docs/ci.md".to_string()],
            created: Some("2026-04-01".to_string()),
            review_after: Some("2026-10-01".to_string()),
            expires: Some("2027-04-01".to_string()),
        };

        let entry = entry_from_process_rule(&rule);

        assert_eq!(entry.path, Some(PathBuf::from(".github/workflows/ci.yml")));
        assert_eq!(entry.classification, "network_process");
        assert_eq!(
            entry.evidence,
            vec![
                "doc:docs/ci.md".to_string(),
                "legacy-policy:proc-cargo-install".to_string(),
                "binary:cargo".to_string(),
                "argv_shape:install cargo-deny --locked".to_string(),
                "network_reach:true".to_string(),
                "called_by:.github/workflows/ci.yml".to_string(),
                "called_by:scripts/install.ps1".to_string(),
            ]
        );
        assert_eq!(entry.lifecycle.created.as_deref(), Some("2026-04-01"));
        assert_eq!(entry.lifecycle.review_after.as_deref(), Some("2026-10-01"));
        assert_eq!(entry.lifecycle.expires.as_deref(), Some("2027-04-01"));
        assert_eq!(
            entry.selector.symbol.as_deref(),
            Some("cargo install cargo-deny --locked")
        );
        assert_eq!(
            entry.selector.target_fingerprint.as_deref(),
            Some("process:cargo install cargo-deny --locked")
        );
        assert_eq!(
            entry.selector.glob.as_deref(),
            Some(".github/workflows/ci.yml")
        );
    }
}
