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
        ledger: None,
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
        ledger: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{Lifecycle, Requirements, Selector, WorkspaceConfig};

    #[test]
    fn process_findings_from_config_keeps_only_process_policy_entries() {
        let process = process_entry("proc-cargo-install");
        let network = network_entry("net-github-api");
        let unsafe_entry = allow_entry(
            "unsafe-boundary",
            FindingKind::Unsafe,
            Some("unsafe_block"),
            Some("src/ffi.rs"),
            None,
            Selector {
                ast_kind: Some("unsafe_block".to_string()),
                ..Selector::default()
            },
        );
        let cfg = config_with_entries(vec![process, network, unsafe_entry]);

        let findings = process_findings_from_config(&cfg);

        let [finding] = findings.as_slice() else {
            std::panic::panic_any(format!(
                "expected one process finding, got {}",
                findings.len()
            ));
        };
        assert_eq!(finding.kind, FindingKind::PolicyException);
        assert_eq!(finding.family.as_deref(), Some("process_spawn"));
        assert_eq!(finding.path, PathBuf::from(".github/workflows/ci.yml"));
        assert_eq!(finding.span, Some(allow_core::Span { line: 1, column: 1 }));
        assert_eq!(finding.identity.language, "policy");
        assert_eq!(finding.identity.ast_kind, "process_spawn");
        assert_eq!(
            finding.identity.symbol.as_deref(),
            Some("cargo install cargo-deny --locked")
        );
        assert_eq!(
            finding.identity.target_fingerprint.as_deref(),
            Some("process:cargo install cargo-deny --locked")
        );
        assert_eq!(
            finding.message,
            "retained process policy entry cargo install cargo-deny --locked"
        );
    }

    #[test]
    fn network_findings_from_config_keeps_only_network_policy_entries() {
        let process = process_entry("proc-cargo-install");
        let network = network_entry("net-github-api");
        let dependency = allow_entry(
            "dependency-workspace",
            FindingKind::PolicyException,
            Some("dependency_surface"),
            Some("Cargo.toml"),
            None,
            Selector {
                ast_kind: Some("dependency_surface".to_string()),
                ..Selector::default()
            },
        );
        let cfg = config_with_entries(vec![process, network, dependency]);

        let findings = network_findings_from_config(&cfg);

        let [finding] = findings.as_slice() else {
            std::panic::panic_any(format!(
                "expected one network finding, got {}",
                findings.len()
            ));
        };
        assert_eq!(finding.kind, FindingKind::PolicyException);
        assert_eq!(finding.family.as_deref(), Some("network_destination"));
        assert_eq!(finding.path, PathBuf::from("policy/network-allowlist.toml"));
        assert_eq!(finding.span, Some(allow_core::Span { line: 1, column: 1 }));
        assert_eq!(finding.identity.language, "policy");
        assert_eq!(finding.identity.ast_kind, "network_destination");
        assert_eq!(
            finding.identity.symbol.as_deref(),
            Some("api.github.com lane release")
        );
        assert_eq!(
            finding.identity.target_fingerprint.as_deref(),
            Some("network:api.github.com:auth:true:lane:release")
        );
        assert_eq!(
            finding.message,
            "retained network policy entry api.github.com lane release"
        );
    }

    #[test]
    fn process_finding_from_entry_falls_back_to_entry_scope_and_id() {
        let entry = allow_entry(
            "proc-local-tool",
            FindingKind::PolicyException,
            Some("process_spawn"),
            None,
            Some("policy/process-allowlist.toml"),
            Selector::default(),
        );

        let finding = process_finding_from_entry(&entry);

        assert_eq!(finding.path, PathBuf::from("policy/process-allowlist.toml"));
        assert_eq!(finding.identity.symbol.as_deref(), Some("proc-local-tool"));
        assert_eq!(finding.identity.target_fingerprint, None);
        assert_eq!(
            finding.message,
            "retained process policy entry proc-local-tool"
        );
    }

    #[test]
    fn network_finding_from_entry_falls_back_to_entry_scope_and_id() {
        let entry = allow_entry(
            "net-public",
            FindingKind::PolicyException,
            Some("network_destination"),
            None,
            Some("policy/network-allowlist.toml"),
            Selector::default(),
        );

        let finding = network_finding_from_entry(&entry);

        assert_eq!(finding.path, PathBuf::from("policy/network-allowlist.toml"));
        assert_eq!(finding.identity.symbol.as_deref(), Some("net-public"));
        assert_eq!(finding.identity.target_fingerprint, None);
        assert_eq!(finding.message, "retained network policy entry net-public");
    }

    fn config_with_entries(entries: Vec<AllowEntry>) -> AllowConfig {
        AllowConfig {
            schema_version: "0.1".to_string(),
            policy: "cargo-allow".to_string(),
            owner: Some("policy".to_string()),
            status: Some("active".to_string()),
            workspace: WorkspaceConfig::default(),
            requirements: Requirements::default(),
            lanes: std::collections::BTreeMap::new(),
            allow: entries,
        }
    }

    fn process_entry(id: &str) -> AllowEntry {
        allow_entry(
            id,
            FindingKind::PolicyException,
            Some("process_spawn"),
            Some(".github/workflows/ci.yml"),
            None,
            Selector {
                ast_kind: Some("process_spawn".to_string()),
                symbol: Some("cargo install cargo-deny --locked".to_string()),
                target_fingerprint: Some("process:cargo install cargo-deny --locked".to_string()),
                glob: Some(".github/workflows/ci.yml".to_string()),
                ..Selector::default()
            },
        )
    }

    fn network_entry(id: &str) -> AllowEntry {
        allow_entry(
            id,
            FindingKind::PolicyException,
            Some("network_destination"),
            Some("policy/network-allowlist.toml"),
            None,
            Selector {
                ast_kind: Some("network_destination".to_string()),
                symbol: Some("api.github.com lane release".to_string()),
                target_fingerprint: Some(
                    "network:api.github.com:auth:true:lane:release".to_string(),
                ),
                glob: Some("policy/network-allowlist.toml".to_string()),
                ..Selector::default()
            },
        )
    }

    fn allow_entry(
        id: &str,
        kind: FindingKind,
        family: Option<&str>,
        path: Option<&str>,
        glob: Option<&str>,
        selector: Selector,
    ) -> AllowEntry {
        AllowEntry {
            id: id.to_string(),
            kind,
            family: family.map(str::to_string),
            path: path.map(PathBuf::from),
            glob: glob.map(str::to_string),
            owner: "policy".to_string(),
            classification: "policy".to_string(),
            reason: "test fixture".to_string(),
            evidence: Vec::new(),
            links: Vec::new(),
            occurrence_limit: None,
            lifecycle: Lifecycle::empty(),
            selector,
            last_seen: None,
        }
    }
}
