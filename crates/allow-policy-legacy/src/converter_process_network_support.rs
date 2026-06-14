use allow_core::normalize_path;
use std::path::Path;

use crate::types::{LegacyNetworkRule, LegacyProcessRule};

pub(crate) fn process_scope(rule: &LegacyProcessRule) -> String {
    rule.called_by
        .first()
        .map(|path| normalize_path(Path::new(path)))
        .unwrap_or_else(|| "policy/process-allowlist.toml".to_string())
}

pub(crate) fn process_symbol(rule: &LegacyProcessRule) -> String {
    let args = rule.argv_shape.join(" ");
    if args.is_empty() {
        rule.binary.clone()
    } else {
        format!("{} {args}", rule.binary)
    }
}

pub(crate) fn process_fingerprint(rule: &LegacyProcessRule) -> String {
    format!("process:{}", process_symbol(rule))
}

pub(crate) fn network_symbol(rule: &LegacyNetworkRule) -> String {
    format!("{} lane {}", rule.destination, rule.lane)
}

pub(crate) fn network_fingerprint(rule: &LegacyNetworkRule) -> String {
    format!(
        "network:{}:auth:{}:lane:{}",
        rule.destination, rule.auth_required, rule.lane
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_scope_prefers_first_caller_and_normalizes_path() {
        let mut rule = process_rule();
        rule.called_by = vec![
            ".github\\workflows\\ci.yml".to_string(),
            "scripts/release.ps1".to_string(),
        ];

        assert_eq!(process_scope(&rule), ".github/workflows/ci.yml");
    }

    #[test]
    fn process_scope_uses_policy_file_when_no_caller_exists() {
        let mut rule = process_rule();
        rule.called_by.clear();

        assert_eq!(process_scope(&rule), "policy/process-allowlist.toml");
    }

    #[test]
    fn process_symbol_and_fingerprint_include_arguments_when_present() {
        let rule = process_rule();

        assert_eq!(process_symbol(&rule), "cargo test --workspace");
        assert_eq!(process_fingerprint(&rule), "process:cargo test --workspace");
    }

    #[test]
    fn process_symbol_uses_binary_when_arguments_are_empty() {
        let mut rule = process_rule();
        rule.argv_shape.clear();

        assert_eq!(process_symbol(&rule), "cargo");
        assert_eq!(process_fingerprint(&rule), "process:cargo");
    }

    #[test]
    fn network_symbol_and_fingerprint_preserve_lane_and_auth_state() {
        let rule = network_rule();

        assert_eq!(network_symbol(&rule), "crates.io lane release");
        assert_eq!(
            network_fingerprint(&rule),
            "network:crates.io:auth:true:lane:release"
        );
    }

    fn process_rule() -> LegacyProcessRule {
        LegacyProcessRule {
            id: "cargo-test-process".to_string(),
            binary: "cargo".to_string(),
            argv_shape: vec!["test".to_string(), "--workspace".to_string()],
            network_reach: false,
            called_by: vec![".github/workflows/ci.yml".to_string()],
            owner: "release/ci".to_string(),
            reason: "CI executes workspace tests.".to_string(),
            evidence: vec!["doc:docs/ci.md".to_string()],
            created: Some("2026-05-09".to_string()),
            review_after: Some("2026-09-09".to_string()),
            expires: Some("never".to_string()),
        }
    }

    fn network_rule() -> LegacyNetworkRule {
        LegacyNetworkRule {
            id: "crates-io-publish".to_string(),
            destination: "crates.io".to_string(),
            auth_required: true,
            auth_secret: Some("CARGO_REGISTRY_TOKEN".to_string()),
            lane: "release".to_string(),
            owner: "release".to_string(),
            reason: "Publish release artifacts.".to_string(),
            evidence: vec!["doc:docs/release.md".to_string()],
            created: Some("2026-05-09".to_string()),
            review_after: Some("2026-09-09".to_string()),
            expires: Some("never".to_string()),
        }
    }
}
