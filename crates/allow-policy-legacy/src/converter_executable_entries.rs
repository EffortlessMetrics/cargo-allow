use allow_core::{AllowEntry, FindingKind, Selector, normalize_path};
use std::path::PathBuf;

use crate::converter_lifecycle_support::lifecycle_from_legacy_fields;
use crate::types::LegacyExecutableRule;

pub(crate) fn entry_from_executable_rule(rule: &LegacyExecutableRule) -> AllowEntry {
    let path = normalize_path(&rule.path);
    AllowEntry {
        id: rule.id.clone(),
        kind: FindingKind::PolicyException,
        family: Some("executable_file".to_string()),
        path: Some(PathBuf::from(&path)),
        glob: None,
        owner: rule.owner.clone(),
        classification: "executable_file".to_string(),
        reason: rule.reason.clone(),
        evidence: executable_evidence(rule),
        links: vec![format!("legacy-policy:{}", rule.id)],
        occurrence_limit: None,
        lifecycle: lifecycle_from_legacy_fields(
            rule.created.clone(),
            rule.review_after.clone(),
            rule.expires.clone(),
        ),
        selector: Selector {
            ast_kind: Some("git_executable_file".to_string()),
            symbol: Some(path.clone()),
            target_fingerprint: Some("git-mode:100755".to_string()),
            glob: Some(path),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn executable_evidence(rule: &LegacyExecutableRule) -> Vec<String> {
    let mut evidence = rule.evidence.clone();
    evidence.push(format!("legacy-policy:{}", rule.id));
    if let Some(interpreter) = &rule.interpreter {
        evidence.push(format!("interpreter:{interpreter}"));
    }
    evidence
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_from_executable_rule_maps_legacy_rule_to_allow_entry() {
        let rule = executable_rule();
        let entry = entry_from_executable_rule(&rule);

        assert_eq!(entry.id, "executable-script");
        assert_eq!(entry.kind, FindingKind::PolicyException);
        assert_eq!(entry.family.as_deref(), Some("executable_file"));
        assert_eq!(entry.path, Some(PathBuf::from("scripts/release.ps1")));
        assert_eq!(entry.glob, None);
        assert_eq!(entry.owner, "release");
        assert_eq!(entry.classification, "executable_file");
        assert_eq!(entry.reason, "release script is intentionally executable");
        assert_eq!(
            entry.evidence,
            vec![
                "docs/release/automation.md".to_string(),
                "legacy-policy:executable-script".to_string(),
                "interpreter:powershell".to_string(),
            ]
        );
        assert_eq!(
            entry.links,
            vec!["legacy-policy:executable-script".to_string()]
        );
        assert_eq!(entry.occurrence_limit, None);
        assert_eq!(entry.lifecycle.created.as_deref(), Some("2026-05-11"));
        assert_eq!(entry.lifecycle.review_after.as_deref(), Some("2026-07-01"));
        assert_eq!(entry.lifecycle.expires.as_deref(), Some("2026-08-01"));
        assert_eq!(
            entry.selector.ast_kind.as_deref(),
            Some("git_executable_file")
        );
        assert_eq!(
            entry.selector.symbol.as_deref(),
            Some("scripts/release.ps1")
        );
        assert_eq!(
            entry.selector.target_fingerprint.as_deref(),
            Some("git-mode:100755")
        );
        assert_eq!(entry.selector.glob.as_deref(), Some("scripts/release.ps1"));
        assert_eq!(entry.last_seen, None);
    }

    #[test]
    fn executable_evidence_keeps_minimal_legacy_policy_link() {
        let rule = LegacyExecutableRule {
            id: "script-without-interpreter".to_string(),
            path: "scripts/tool".to_string(),
            owner: "tools".to_string(),
            reason: "checked in executable helper".to_string(),
            interpreter: None,
            evidence: Vec::new(),
            created: None,
            review_after: None,
            expires: None,
        };

        assert_eq!(
            executable_evidence(&rule),
            vec!["legacy-policy:script-without-interpreter".to_string()]
        );
    }

    fn executable_rule() -> LegacyExecutableRule {
        LegacyExecutableRule {
            id: "executable-script".to_string(),
            path: "scripts\\release.ps1".to_string(),
            owner: "release".to_string(),
            reason: "release script is intentionally executable".to_string(),
            interpreter: Some("powershell".to_string()),
            evidence: vec!["docs/release/automation.md".to_string()],
            created: Some("2026-05-11".to_string()),
            review_after: Some("2026-07-01".to_string()),
            expires: Some("2026-08-01".to_string()),
        }
    }
}
