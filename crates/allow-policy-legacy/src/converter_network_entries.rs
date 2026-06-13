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
    let mut evidence = rule.evidence.clone();
    evidence.push(format!("legacy-policy:{}", rule.id));
    evidence.push(format!("destination:{}", rule.destination));
    evidence.push(format!("lane:{}", rule.lane));
    evidence.push(format!("auth_required:{}", rule.auth_required));
    if let Some(secret) = &rule.auth_secret {
        evidence.push(format!("auth_secret:{secret}"));
    }
    evidence
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_network_rule_records_policy_scope_and_selector_identity() {
        let rule = LegacyNetworkRule {
            id: "net-crates-io".to_string(),
            destination: "crates.io".to_string(),
            auth_required: false,
            auth_secret: None,
            lane: "build".to_string(),
            owner: "ci".to_string(),
            reason: "Build lane fetches public crates.".to_string(),
            evidence: Vec::new(),
            created: Some("2026-03-01".to_string()),
            review_after: None,
            expires: Some("never".to_string()),
        };

        let entry = entry_from_network_rule(&rule);

        assert_eq!(entry.id, "net-crates-io");
        assert_eq!(entry.kind, FindingKind::PolicyException);
        assert_eq!(entry.family.as_deref(), Some("network_destination"));
        assert_eq!(
            entry.path,
            Some(PathBuf::from("policy/network-allowlist.toml"))
        );
        assert_eq!(entry.glob, None);
        assert_eq!(entry.owner, "ci");
        assert_eq!(entry.classification, "public_network");
        assert_eq!(entry.reason, "Build lane fetches public crates.");
        assert_eq!(
            entry.evidence,
            vec![
                "legacy-policy:net-crates-io".to_string(),
                "destination:crates.io".to_string(),
                "lane:build".to_string(),
                "auth_required:false".to_string(),
            ]
        );
        assert_eq!(entry.links, vec!["legacy-policy:net-crates-io".to_string()]);
        assert_eq!(entry.occurrence_limit, None);
        assert_eq!(entry.lifecycle.created.as_deref(), Some("2026-03-01"));
        assert_eq!(entry.lifecycle.review_after.as_deref(), Some("2026-03-01"));
        assert_eq!(entry.lifecycle.expires.as_deref(), Some("never"));
        assert_eq!(
            entry.selector.ast_kind.as_deref(),
            Some("network_destination")
        );
        assert_eq!(
            entry.selector.symbol.as_deref(),
            Some("crates.io lane build")
        );
        assert_eq!(
            entry.selector.target_fingerprint.as_deref(),
            Some("network:crates.io:auth:false:lane:build")
        );
        assert_eq!(
            entry.selector.glob.as_deref(),
            Some("policy/network-allowlist.toml")
        );
        assert!(entry.last_seen.is_none());
    }

    #[test]
    fn authenticated_network_rule_preserves_secret_evidence_and_lifecycle() {
        let rule = LegacyNetworkRule {
            id: "net-github-api".to_string(),
            destination: "api.github.com".to_string(),
            auth_required: true,
            auth_secret: Some("GITHUB_TOKEN".to_string()),
            lane: "release".to_string(),
            owner: "release".to_string(),
            reason: "Release lane publishes GitHub releases.".to_string(),
            evidence: vec!["doc:docs/release.md".to_string()],
            created: Some("2026-04-01".to_string()),
            review_after: Some("2026-10-01".to_string()),
            expires: Some("2027-04-01".to_string()),
        };

        let entry = entry_from_network_rule(&rule);

        assert_eq!(entry.classification, "authenticated_network");
        assert_eq!(
            entry.evidence,
            vec![
                "doc:docs/release.md".to_string(),
                "legacy-policy:net-github-api".to_string(),
                "destination:api.github.com".to_string(),
                "lane:release".to_string(),
                "auth_required:true".to_string(),
                "auth_secret:GITHUB_TOKEN".to_string(),
            ]
        );
        assert_eq!(entry.lifecycle.created.as_deref(), Some("2026-04-01"));
        assert_eq!(entry.lifecycle.review_after.as_deref(), Some("2026-10-01"));
        assert_eq!(entry.lifecycle.expires.as_deref(), Some("2027-04-01"));
        assert_eq!(
            entry.selector.symbol.as_deref(),
            Some("api.github.com lane release")
        );
        assert_eq!(
            entry.selector.target_fingerprint.as_deref(),
            Some("network:api.github.com:auth:true:lane:release")
        );
    }
}
