use allow_core::{AllowEntry, FindingKind, Lifecycle, Selector};
use std::path::PathBuf;

use crate::converter_evidence::{network_evidence, process_evidence};
use crate::converter_process_network_support::{
    network_fingerprint, network_symbol, process_fingerprint, process_scope, process_symbol,
};
use crate::types::{LegacyNetworkRule, LegacyProcessRule};

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

pub(crate) fn entry_from_network_rule(rule: &LegacyNetworkRule) -> AllowEntry {
    let scope = "policy/network-allowlist.toml".to_string();
    let symbol = network_symbol(rule);
    AllowEntry {
        id: rule.id.clone(),
        kind: FindingKind::PolicyException,
        family: Some("network_destination".to_string()),
        path: Some(PathBuf::from(&scope)),
        glob: None,
        owner: rule.owner.clone(),
        classification: if rule.auth_required {
            "authenticated_network".to_string()
        } else {
            "public_network".to_string()
        },
        reason: rule.reason.clone(),
        evidence: network_evidence(rule),
        links: vec![format!("legacy-policy:{}", rule.id)],
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: rule.created.clone(),
            review_after: rule.review_after.clone(),
            expires: rule.expires.clone(),
        },
        selector: Selector {
            ast_kind: Some("network_destination".to_string()),
            symbol: Some(symbol.clone()),
            target_fingerprint: Some(network_fingerprint(rule)),
            glob: Some(scope),
            ..Selector::default()
        },
        last_seen: None,
    }
}
