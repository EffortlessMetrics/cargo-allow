use allow_core::{AllowConfig, AllowEntry, CargoAllowError, CargoAllowResult};
use std::collections::BTreeMap;

use crate::converter_config::config_from_entries;
use crate::converter_dependency_entries::entry_from_dependency_surface_rule;
use crate::converter_executable_entries::entry_from_executable_rule;
use crate::converter_process_network_entries::{entry_from_network_rule, entry_from_process_rule};
use crate::converter_workflow_entries::entries_from_workflow_rule;
use crate::types::{
    LegacyDependencySurfaceRule, LegacyExecutableRule, LegacyNetworkRule, LegacyProcessRule,
    LegacyWorkflowRule,
};

pub(crate) fn config_from_executable_rules(
    table: &toml::Table,
    rules: &[LegacyExecutableRule],
) -> CargoAllowResult<AllowConfig> {
    config_from_entries(table, rules.iter().map(entry_from_executable_rule))
}

pub(crate) fn config_from_workflow_rules(
    table: &toml::Table,
    rules: &[LegacyWorkflowRule],
) -> CargoAllowResult<AllowConfig> {
    let entries: Vec<AllowEntry> = rules.iter().flat_map(entries_from_workflow_rule).collect();
    validate_unique_workflow_entry_ids(&entries)?;
    config_from_entries(table, entries)
}

fn validate_unique_workflow_entry_ids(entries: &[AllowEntry]) -> CargoAllowResult<()> {
    let mut ids = BTreeMap::<String, String>::new();
    for entry in entries {
        let source_path = entry
            .path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "<missing path>".to_string());
        if let Some(first_path) = ids.insert(entry.id.clone(), source_path.clone()) {
            return Err(CargoAllowError::new(format!(
                "duplicate workflow allow id `{}` from source paths `{}` and `{}`",
                entry.id, first_path, source_path
            )));
        }
    }
    Ok(())
}

pub(crate) fn config_from_dependency_surface_rules(
    table: &toml::Table,
    rules: &[LegacyDependencySurfaceRule],
) -> CargoAllowResult<AllowConfig> {
    config_from_entries(table, rules.iter().map(entry_from_dependency_surface_rule))
}

pub(crate) fn config_from_process_rules(
    table: &toml::Table,
    rules: &[LegacyProcessRule],
) -> CargoAllowResult<AllowConfig> {
    config_from_entries(table, rules.iter().map(entry_from_process_rule))
}

pub(crate) fn config_from_network_rules(
    table: &toml::Table,
    rules: &[LegacyNetworkRule],
) -> CargoAllowResult<AllowConfig> {
    config_from_entries(table, rules.iter().map(entry_from_network_rule))
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::FindingKind;
    use std::path::{Path, PathBuf};

    fn parse_table(input: &str) -> toml::Table {
        toml::from_str::<toml::Table>(input)
            .unwrap_or_else(|err| std::panic::panic_any(format!("test TOML parses: {err}")))
    }

    fn policy_table() -> toml::Table {
        parse_table(
            r#"
owner = "policy"
status = "active"
"#,
        )
    }

    #[test]
    fn config_from_executable_rules_preserves_config_and_converted_entry() {
        let table = policy_table();
        let rules = vec![LegacyExecutableRule {
            id: "executable-script".to_string(),
            path: "scripts\\release.ps1".to_string(),
            owner: "release".to_string(),
            reason: "release script is intentionally executable".to_string(),
            interpreter: Some("powershell".to_string()),
            evidence: vec!["docs/release/automation.md".to_string()],
            created: Some("2026-05-11".to_string()),
            review_after: Some("2026-07-01".to_string()),
            expires: Some("2026-08-01".to_string()),
        }];

        let cfg = config_from_executable_rules(&table, &rules)
            .unwrap_or_else(|err| std::panic::panic_any(format!("config converts: {err}")));

        assert_config_metadata(&cfg, 1);
        let entry = single_entry(&cfg);
        assert_eq!(entry.id, "executable-script");
        assert_eq!(entry.kind, FindingKind::PolicyException);
        assert_eq!(entry.family.as_deref(), Some("executable_file"));
        assert_eq!(
            entry.path.as_deref(),
            Some(Path::new("scripts/release.ps1"))
        );
        assert_eq!(entry.owner, "release");
        assert_eq!(entry.classification, "executable_file");
        assert_eq!(
            entry.selector.ast_kind.as_deref(),
            Some("git_executable_file")
        );
        assert_eq!(
            entry.selector.target_fingerprint.as_deref(),
            Some("git-mode:100755")
        );
        assert_eq!(
            entry.evidence,
            vec![
                "docs/release/automation.md".to_string(),
                "legacy-policy:executable-script".to_string(),
                "interpreter:powershell".to_string(),
            ]
        );
    }

    #[test]
    fn config_from_workflow_rules_preserves_file_and_action_entries() {
        let table = policy_table();
        let rules = vec![LegacyWorkflowRule {
            path: ".github\\workflows\\ci.yml".to_string(),
            owner: "platform".to_string(),
            reason: "required CI lane".to_string(),
            permissions: vec!["contents:read".to_string()],
            secrets_used: vec!["CARGO_REGISTRY_TOKEN".to_string()],
            external_actions: vec![
                "actions/checkout@v4".to_string(),
                "dtolnay/rust-toolchain@stable".to_string(),
            ],
            duplicate_of_lane: Some("ci-shadow".to_string()),
            evidence: vec!["docs/ci.md".to_string()],
            created: Some("2026-01-02".to_string()),
            review_after: Some("2026-07-02".to_string()),
            expires: Some("never".to_string()),
        }];

        let cfg = config_from_workflow_rules(&table, &rules)
            .unwrap_or_else(|err| std::panic::panic_any(format!("config converts: {err}")));

        assert_config_metadata(&cfg, 3);
        let file_entry = cfg
            .allow
            .iter()
            .find(|entry| entry.id == "workflow-file-github-workflows-ci-yml-a8f3817ec78236ac")
            .unwrap_or_else(|| std::panic::panic_any("workflow file entry exists"));
        assert_eq!(file_entry.family.as_deref(), Some("github_workflow"));
        assert_eq!(
            file_entry.path.as_deref(),
            Some(Path::new(".github/workflows/ci.yml"))
        );
        assert_eq!(
            file_entry.evidence,
            vec![
                "docs/ci.md".to_string(),
                "legacy-policy:workflow:.github/workflows/ci.yml".to_string(),
                "permission:contents:read".to_string(),
                "secret:CARGO_REGISTRY_TOKEN".to_string(),
                "duplicate_of_lane:ci-shadow".to_string(),
            ]
        );

        let checkout_action = cfg
            .allow
            .iter()
            .find(|entry| {
                entry.id
                    == "workflow-action-github-workflows-ci-yml-a8f3817ec78236ac--actions-checkout-v4"
            })
            .unwrap_or_else(|| std::panic::panic_any("checkout action entry exists"));
        assert_eq!(
            checkout_action.family.as_deref(),
            Some("workflow_external_action")
        );
        assert_eq!(
            checkout_action.selector.ast_kind.as_deref(),
            Some("github_action_uses")
        );
        assert_eq!(
            checkout_action.selector.target_fingerprint.as_deref(),
            Some("action:actions/checkout@v4")
        );
    }

    #[test]
    fn config_from_workflow_rules_disambiguates_same_slug_workflow_paths() {
        let table = policy_table();
        let rules = vec![
            workflow_rule_for_path(".github/workflows/ci-main.yml"),
            workflow_rule_for_path(".github/workflows/ci_main.yml"),
        ];

        let cfg = config_from_workflow_rules(&table, &rules)
            .unwrap_or_else(|err| std::panic::panic_any(format!("config converts: {err}")));

        assert_config_metadata(&cfg, 4);
        let file_ids: Vec<&str> = cfg
            .allow
            .iter()
            .map(|entry| entry.id.as_str())
            .filter(|id| id.starts_with("workflow-file-github-workflows-ci-main-yml-"))
            .collect();
        assert_eq!(file_ids.len(), 2);
        assert_ne!(
            file_ids[0].to_ascii_lowercase(),
            file_ids[1].to_ascii_lowercase()
        );
        let action_ids: Vec<&str> = cfg
            .allow
            .iter()
            .map(|entry| entry.id.as_str())
            .filter(|id| id.starts_with("workflow-action-github-workflows-ci-main-yml-"))
            .collect();
        assert_eq!(action_ids.len(), 2);
        assert_ne!(
            action_ids[0].to_ascii_lowercase(),
            action_ids[1].to_ascii_lowercase()
        );
    }

    #[test]
    fn config_from_workflow_rules_duplicate_error_names_source_paths() {
        let table = policy_table();
        let rules = vec![
            workflow_rule_for_path(".github/workflows/ci.yml"),
            workflow_rule_for_path(".github\\workflows\\ci.yml"),
        ];

        let err = config_from_workflow_rules(&table, &rules)
            .expect_err("duplicate workflow IDs should fail conversion");
        let message = err.to_string();

        assert!(message.contains("duplicate workflow allow id"));
        assert!(message.matches(".github/workflows/ci.yml").count() >= 2);
    }

    #[test]
    fn config_from_dependency_surface_rules_preserves_config_and_converted_entry() {
        let table = policy_table();
        let rules = vec![LegacyDependencySurfaceRule {
            id: "dependency-workspace".to_string(),
            pattern: "crates\\core\\Cargo.toml".to_string(),
            is_glob: false,
            surface: "workspace_manifest".to_string(),
            owner: "build".to_string(),
            reason: "Workspace manifest owns dependency declarations.".to_string(),
            broad_glob_reason: None,
            dep_count_at_baseline: Some(42),
            evidence: vec!["test:dependency_surface".to_string()],
            created: Some("2026-01-01".to_string()),
            review_after: Some("2026-10-01".to_string()),
            expires: Some("2027-01-01".to_string()),
        }];

        let cfg = config_from_dependency_surface_rules(&table, &rules)
            .unwrap_or_else(|err| std::panic::panic_any(format!("config converts: {err}")));

        assert_config_metadata(&cfg, 1);
        let entry = single_entry(&cfg);
        assert_eq!(entry.id, "dependency-workspace");
        assert_eq!(entry.family.as_deref(), Some("dependency_surface"));
        assert_eq!(entry.path, Some(PathBuf::from("crates/core/Cargo.toml")));
        assert_eq!(entry.classification, "workspace_manifest");
        assert_eq!(
            entry.selector.ast_kind.as_deref(),
            Some("dependency_surface")
        );
        assert_eq!(
            entry.evidence,
            vec![
                "test:dependency_surface".to_string(),
                "legacy-policy:dependency-workspace".to_string(),
                "surface:workspace_manifest".to_string(),
                "dep_count_at_baseline:42".to_string(),
            ]
        );
    }

    #[test]
    fn config_from_process_rules_preserves_config_and_converted_entry() {
        let table = policy_table();
        let rules = vec![LegacyProcessRule {
            id: "proc-cargo-install".to_string(),
            binary: "cargo".to_string(),
            argv_shape: vec![
                "install".to_string(),
                "cargo-deny".to_string(),
                "--locked".to_string(),
            ],
            network_reach: true,
            called_by: vec![".github\\workflows\\ci.yml".to_string()],
            owner: "ci".to_string(),
            reason: "CI installs a pinned tool.".to_string(),
            evidence: vec!["doc:docs/ci.md".to_string()],
            created: Some("2026-04-01".to_string()),
            review_after: Some("2026-10-01".to_string()),
            expires: Some("2027-04-01".to_string()),
        }];

        let cfg = config_from_process_rules(&table, &rules)
            .unwrap_or_else(|err| std::panic::panic_any(format!("config converts: {err}")));

        assert_config_metadata(&cfg, 1);
        let entry = single_entry(&cfg);
        assert_eq!(entry.id, "proc-cargo-install");
        assert_eq!(entry.family.as_deref(), Some("process_spawn"));
        assert_eq!(
            entry.path.as_deref(),
            Some(Path::new(".github/workflows/ci.yml"))
        );
        assert_eq!(entry.classification, "network_process");
        assert_eq!(
            entry.selector.symbol.as_deref(),
            Some("cargo install cargo-deny --locked")
        );
        assert_eq!(
            entry.selector.target_fingerprint.as_deref(),
            Some("process:cargo install cargo-deny --locked")
        );
        assert_eq!(
            entry.evidence,
            vec![
                "doc:docs/ci.md".to_string(),
                "legacy-policy:proc-cargo-install".to_string(),
                "binary:cargo".to_string(),
                "argv_shape:install cargo-deny --locked".to_string(),
                "network_reach:true".to_string(),
                "called_by:.github/workflows/ci.yml".to_string(),
            ]
        );
    }

    #[test]
    fn config_from_network_rules_preserves_config_and_converted_entry() {
        let table = policy_table();
        let rules = vec![LegacyNetworkRule {
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
        }];

        let cfg = config_from_network_rules(&table, &rules)
            .unwrap_or_else(|err| std::panic::panic_any(format!("config converts: {err}")));

        assert_config_metadata(&cfg, 1);
        let entry = single_entry(&cfg);
        assert_eq!(entry.id, "net-github-api");
        assert_eq!(entry.family.as_deref(), Some("network_destination"));
        assert_eq!(entry.classification, "authenticated_network");
        assert_eq!(
            entry.path,
            Some(PathBuf::from("policy/network-allowlist.toml"))
        );
        assert_eq!(
            entry.selector.symbol.as_deref(),
            Some("api.github.com lane release")
        );
        assert_eq!(
            entry.selector.target_fingerprint.as_deref(),
            Some("network:api.github.com:auth:true:lane:release")
        );
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
    }

    fn assert_config_metadata(cfg: &AllowConfig, expected_entries: usize) {
        assert_eq!(cfg.schema_version, "0.1");
        assert_eq!(cfg.policy, "cargo-allow");
        assert_eq!(cfg.owner.as_deref(), Some("policy"));
        assert_eq!(cfg.status.as_deref(), Some("active"));
        assert_eq!(cfg.allow.len(), expected_entries);
    }

    fn single_entry(cfg: &AllowConfig) -> &allow_core::AllowEntry {
        let [entry] = cfg.allow.as_slice() else {
            std::panic::panic_any(format!("expected one allow entry, got {}", cfg.allow.len()));
        };
        entry
    }

    fn workflow_rule_for_path(path: &str) -> LegacyWorkflowRule {
        LegacyWorkflowRule {
            path: path.to_string(),
            owner: "platform".to_string(),
            reason: "required CI lane".to_string(),
            permissions: Vec::new(),
            secrets_used: Vec::new(),
            external_actions: vec!["actions/checkout@v4".to_string()],
            duplicate_of_lane: None,
            evidence: vec!["docs/ci.md".to_string()],
            created: Some("2026-01-02".to_string()),
            review_after: Some("2026-07-02".to_string()),
            expires: Some("never".to_string()),
        }
    }
}
