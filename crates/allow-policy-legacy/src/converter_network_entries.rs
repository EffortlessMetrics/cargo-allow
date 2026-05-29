use allow_core::{AllowEntry, FindingKind, Selector};
use std::path::PathBuf;

use crate::converter_lifecycle_support::lifecycle_from_legacy_fields;
use crate::converter_process_network_support::{network_fingerprint, network_symbol};
use crate::types::LegacyNetworkRule;

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
        lifecycle: lifecycle_from_legacy_fields(
            rule.created.clone(),
            rule.review_after.clone(),
            rule.expires.clone(),
        ),
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

fn network_evidence(rule: &LegacyNetworkRule) -> Vec<String> {
    let mut evidence = vec![
        format!("destination:{}", rule.destination),
        format!("lane:{}", rule.lane),
        format!("auth_required:{}", rule.auth_required),
    ];
    if let Some(secret) = &rule.auth_secret {
        evidence.push(format!("auth_secret:{secret}"));
    }
    evidence
}
