use allow_core::{AllowEntry, Finding, LastSeen, Selector, normalize_path};
use std::path::PathBuf;

use crate::converter_file_support::{evidence_from_rule, lifecycle_from_rule};
use crate::types::LegacyNonRustRule;

pub(crate) fn entry_from_finding(
    rule: &LegacyNonRustRule,
    finding: &Finding,
    index: usize,
) -> AllowEntry {
    let path = normalize_path(&finding.path);
    AllowEntry {
        id: format!("{}--{index:04}", rule.id),
        kind: finding.kind,
        family: None,
        path: Some(PathBuf::from(&path)),
        glob: None,
        owner: rule.owner.clone(),
        classification: rule.classification.clone(),
        reason: rule.reason.clone(),
        evidence: evidence_from_rule(rule),
        links: vec![format!("legacy-policy:{}", rule.id)],
        occurrence_limit: None,
        lifecycle: lifecycle_from_rule(rule),
        selector: Selector {
            ast_kind: Some(finding.identity.ast_kind.clone()),
            symbol: Some(path.clone()),
            glob: Some(path),
            ..Selector::default()
        },
        last_seen: finding.span.as_ref().map(|span| LastSeen {
            line: span.line,
            column: span.column,
        }),
    }
}
