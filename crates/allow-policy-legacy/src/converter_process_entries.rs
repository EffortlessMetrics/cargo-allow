use allow_core::{AllowEntry, FindingKind, Lifecycle, Selector, normalize_path};
use std::path::{Path, PathBuf};

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
        lifecycle: Lifecycle {
            created: rule.created.clone(),
            review_after: rule.review_after.clone(),
            expires: rule.expires.clone(),
        },
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
    let mut evidence = vec![
        format!("binary:{}", rule.binary),
        format!("argv_shape:{}", rule.argv_shape.join(" ")),
        format!("network_reach:{}", rule.network_reach),
    ];
    evidence.extend(
        rule.called_by
            .iter()
            .map(|path| format!("called_by:{}", normalize_path(Path::new(path)))),
    );
    evidence
}
