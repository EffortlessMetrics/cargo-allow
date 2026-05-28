use allow_core::{AllowConfig, AllowEntry, Finding, FindingKind};
use std::path::PathBuf;

pub fn process_findings_from_config(cfg: &AllowConfig) -> Vec<Finding> {
    cfg.allow
        .iter()
        .filter(|entry| {
            entry.kind == FindingKind::PolicyException
                && entry.family.as_deref() == Some("process_spawn")
        })
        .map(process_finding_from_entry)
        .collect()
}

pub fn network_findings_from_config(cfg: &AllowConfig) -> Vec<Finding> {
    cfg.allow
        .iter()
        .filter(|entry| {
            entry.kind == FindingKind::PolicyException
                && entry.family.as_deref() == Some("network_destination")
        })
        .map(network_finding_from_entry)
        .collect()
}

fn process_finding_from_entry(entry: &AllowEntry) -> Finding {
    let path = entry
        .path
        .clone()
        .unwrap_or_else(|| PathBuf::from(entry.path_or_glob()));
    let symbol = entry
        .selector
        .symbol
        .clone()
        .unwrap_or_else(|| entry.id.clone());
    let mut identity = allow_core::StructuralIdentity::new("policy", "process_spawn");
    identity.symbol = Some(symbol.clone());
    identity.target_fingerprint = entry.selector.target_fingerprint.clone();
    Finding {
        kind: FindingKind::PolicyException,
        family: Some("process_spawn".to_string()),
        path,
        span: Some(allow_core::Span { line: 1, column: 1 }),
        identity,
        message: format!("retained process policy entry {symbol}"),
    }
}

fn network_finding_from_entry(entry: &AllowEntry) -> Finding {
    let path = entry
        .path
        .clone()
        .unwrap_or_else(|| PathBuf::from(entry.path_or_glob()));
    let symbol = entry
        .selector
        .symbol
        .clone()
        .unwrap_or_else(|| entry.id.clone());
    let mut identity = allow_core::StructuralIdentity::new("policy", "network_destination");
    identity.symbol = Some(symbol.clone());
    identity.target_fingerprint = entry.selector.target_fingerprint.clone();
    Finding {
        kind: FindingKind::PolicyException,
        family: Some("network_destination".to_string()),
        path,
        span: Some(allow_core::Span { line: 1, column: 1 }),
        identity,
        message: format!("retained network policy entry {symbol}"),
    }
}
