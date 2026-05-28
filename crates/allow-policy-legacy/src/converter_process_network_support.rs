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
