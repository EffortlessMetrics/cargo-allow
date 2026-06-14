use allow_core::{AllowConfig, CargoAllowResult};
use toml::Value;

use crate::converter_file_configs::{config_from_generated_rules, config_from_non_rust_rules};
use crate::converter_panic_configs::{
    config_from_no_panic_allowlist_entries, config_from_no_panic_baseline_entries,
};
use crate::converter_policy_configs::{
    config_from_dependency_surface_rules, config_from_executable_rules, config_from_network_rules,
    config_from_process_rules, config_from_workflow_rules,
};
use crate::converter_source_configs::{config_from_clippy_rules, config_from_unsafe_rules};
use crate::parsers::{
    is_clippy_exceptions_policy, parse_clippy_rules, parse_dependency_surface_rules,
    parse_executable_rules, parse_generated_rules, parse_network_rules,
    parse_no_panic_allowlist_entries, parse_no_panic_baseline_entries, parse_non_rust_rules,
    parse_process_rules, parse_unsafe_rules, parse_workflow_rules,
};

pub(crate) fn config_from_legacy_table(
    table: &toml::Table,
) -> CargoAllowResult<Option<AllowConfig>> {
    match table.get("policy").and_then(Value::as_str) {
        Some("non-rust-allowlist") => {
            let rules = parse_non_rust_rules(table)?;
            config_from_non_rust_rules(table, &rules).map(Some)
        }
        Some("generated-allowlist") => {
            let rules = parse_generated_rules(table)?;
            config_from_generated_rules(table, &rules).map(Some)
        }
        Some("no-panic-allowlist") => {
            let entries = parse_no_panic_allowlist_entries(table)?;
            config_from_no_panic_allowlist_entries(table, &entries).map(Some)
        }
        Some("no-panic-baseline") => {
            let entries = parse_no_panic_baseline_entries(table)?;
            config_from_no_panic_baseline_entries(table, &entries).map(Some)
        }
        _ if is_clippy_exceptions_policy(table) => {
            let rules = parse_clippy_rules(table)?;
            config_from_clippy_rules(table, &rules).map(Some)
        }
        Some("unsafe-allowlist") => {
            let rules = parse_unsafe_rules(table)?;
            config_from_unsafe_rules(table, &rules).map(Some)
        }
        Some("executable-allowlist") => {
            let rules = parse_executable_rules(table)?;
            config_from_executable_rules(table, &rules).map(Some)
        }
        Some("workflow-allowlist") => {
            let rules = parse_workflow_rules(table)?;
            config_from_workflow_rules(table, &rules).map(Some)
        }
        Some("dependency-surface-allowlist") => {
            let rules = parse_dependency_surface_rules(table)?;
            config_from_dependency_surface_rules(table, &rules).map(Some)
        }
        Some("process-allowlist") => {
            let rules = parse_process_rules(table)?;
            config_from_process_rules(table, &rules).map(Some)
        }
        Some("network-allowlist") => {
            let rules = parse_network_rules(table)?;
            config_from_network_rules(table, &rules).map(Some)
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::FindingKind;
    use std::path::Path;

    fn parse_table(input: &str) -> toml::Table {
        toml::from_str::<toml::Table>(input)
            .unwrap_or_else(|err| std::panic::panic_any(format!("test TOML parses: {err}")))
    }

    fn migrated_config(input: &str) -> AllowConfig {
        let table = parse_table(input);
        config_from_legacy_table(&table)
            .unwrap_or_else(|err| {
                std::panic::panic_any(format!("legacy table dispatch succeeds: {err}"))
            })
            .unwrap_or_else(|| std::panic::panic_any("legacy table should dispatch"))
    }

    #[test]
    fn config_from_legacy_table_dispatches_each_policy_to_expected_entry_shape() {
        for case in dispatch_cases() {
            let config = migrated_config(case.policy);
            let entry = config
                .allow
                .iter()
                .find(|entry| entry.id == case.entry_id)
                .unwrap_or_else(|| {
                    std::panic::panic_any(format!(
                        "{} dispatch should emit entry {}",
                        case.label, case.entry_id
                    ))
                });

            assert_eq!(entry.kind, case.kind, "{} kind", case.label);
            assert_eq!(
                entry.family.as_deref(),
                case.family,
                "{} family",
                case.label
            );
            assert_eq!(
                entry.path.as_deref(),
                case.path.map(Path::new),
                "{} path",
                case.label
            );
        }
    }

    #[test]
    fn config_from_legacy_table_returns_none_for_unrecognized_policy_values() {
        for input in [
            "policy = \"unknown-policy\"",
            "policy = \"\"",
            "owner = \"repo\"",
        ] {
            let table = parse_table(input);
            let result = config_from_legacy_table(&table).unwrap_or_else(|err| {
                std::panic::panic_any(format!("unrecognized policy checks cleanly: {err}"))
            });

            assert!(result.is_none(), "{input} should not dispatch");
        }
    }

    struct DispatchCase {
        label: &'static str,
        policy: &'static str,
        entry_id: &'static str,
        kind: FindingKind,
        family: Option<&'static str>,
        path: Option<&'static str>,
    }

    fn dispatch_cases() -> [DispatchCase; 11] {
        [
            DispatchCase {
                label: "non-rust",
                policy: r#"
policy = "non-rust-allowlist"
owner = "repo"
status = "advisory"

[[allow]]
id = "non-rust-readme"
path = "README.md"
owner = "docs"
classification = "documentation"
reason = "Front door docs."
evidence = ["doc:README.md"]
created = "2026-05-09"
expires = "permanent"
"#,
                entry_id: "non-rust-readme",
                kind: FindingKind::NonRustFile,
                family: None,
                path: Some("README.md"),
            },
            DispatchCase {
                label: "generated",
                policy: r#"
policy = "generated-allowlist"
owner = "repo"
status = "advisory"

[[allow]]
id = "generated-policy"
path = "policy/generated.toml"
owner = "release"
reason = "Generated fixture."
evidence = ["test:generated_policy"]
created = "2026-05-09"
expires = "permanent"
"#,
                entry_id: "generated-policy",
                kind: FindingKind::GeneratedCode,
                family: Some("generated_code"),
                path: Some("policy/generated.toml"),
            },
            DispatchCase {
                label: "no-panic allowlist",
                policy: r#"
policy = "no-panic-allowlist"
owner = "repo"
status = "advisory"

[[allow]]
id = "panic-unwrap"
path = "src/lib.rs"
family = "unwrap"
owner = "runtime"
classification = "reviewed_panic_exception"
reason = "Input was checked."
evidence = ["test:panic_coverage"]
created = "2026-05-09"
review_after = "2026-09-09"

[allow.selector]
kind = "method-call"
callee = "unwrap"
"#,
                entry_id: "panic-unwrap",
                kind: FindingKind::Panic,
                family: Some("unwrap"),
                path: Some("src/lib.rs"),
            },
            DispatchCase {
                label: "no-panic baseline",
                policy: r#"
policy = "no-panic-baseline"
owner = "repo"
status = "advisory"

[[entry]]
path = "src/lib.rs"
family = "panic"
selector_kind = "macro-call"
selector_callee = "panic"
snippet = "panic!(\"bad\")"
count = 2
"#,
                entry_id: "panic-baseline-0001",
                kind: FindingKind::Panic,
                family: Some("panic_macro"),
                path: Some("src/lib.rs"),
            },
            DispatchCase {
                label: "clippy",
                policy: r#"
policy = "clippy-exceptions"
owner = "repo"
status = "advisory"

[[allow]]
id = "clippy-unwrap"
path = "src/lib.rs"
lint = "clippy::unwrap_used"
family = "expect"
owner = "lint"
classification = "reviewed_lint_exception"
reason = "Intentional suppression."
evidence = ["test:lint_coverage"]
created = "2026-05-09"
review_after = "2026-09-09"
"#,
                entry_id: "clippy-unwrap",
                kind: FindingKind::LintException,
                family: Some("expect_attribute"),
                path: Some("src/lib.rs"),
            },
            DispatchCase {
                label: "unsafe",
                policy: r#"
policy = "unsafe-allowlist"
owner = "repo"
status = "advisory"

[[allow]]
id = "unsafe-read"
path = "src/lib.rs"
family = "unsafe-block"
owner = "runtime"
classification = "reviewed_unsafe_boundary"
reason = "Caller checks pointer."
evidence = ["unsafe-review:read.json"]
created = "2026-05-09"
review_after = "2026-09-09"

[allow.selector]
kind = "unsafe-block"
container = "read"
"#,
                entry_id: "unsafe-read",
                kind: FindingKind::Unsafe,
                family: Some("unsafe_block"),
                path: Some("src/lib.rs"),
            },
            DispatchCase {
                label: "executable",
                policy: r#"
policy = "executable-allowlist"
owner = "repo"
status = "advisory"

[[allow]]
id = "exec-script"
path = "scripts/package-proof.sh"
interpreter = "bash"
owner = "release"
reason = "Release helper."
evidence = ["test:executable_policy"]
created = "2026-05-09"
review_after = "2026-09-09"
"#,
                entry_id: "exec-script",
                kind: FindingKind::PolicyException,
                family: Some("executable_file"),
                path: Some("scripts/package-proof.sh"),
            },
            DispatchCase {
                label: "workflow",
                policy: r#"
policy = "workflow-allowlist"
owner = "repo"
status = "advisory"

[[entry]]
path = ".github/workflows/ci.yml"
owner = "release/ci"
reason = "Primary CI lane."
permissions = ["contents:read"]
secrets_used = []
external_actions = ["actions/checkout@v6.0.2"]
evidence = ["doc:docs/ci.md"]
created = "2026-05-09"
review_after = "2026-09-09"
"#,
                entry_id: "workflow-file-github-workflows-ci-yml",
                kind: FindingKind::PolicyException,
                family: Some("github_workflow"),
                path: Some(".github/workflows/ci.yml"),
            },
            DispatchCase {
                label: "dependency",
                policy: r#"
policy = "dependency-surface-allowlist"
owner = "repo"
status = "advisory"

[[allow]]
id = "workspace-manifest"
path = "Cargo.toml"
surface = "dependency_surface"
owner = "deps"
reason = "Workspace manifest owns dependency declarations."
evidence = ["test:dependency_surface"]
created = "2026-05-09"
review_after = "2026-09-09"
dep_count_at_baseline = 4
"#,
                entry_id: "workspace-manifest",
                kind: FindingKind::PolicyException,
                family: Some("dependency_surface"),
                path: Some("Cargo.toml"),
            },
            DispatchCase {
                label: "process",
                policy: r#"
policy = "process-allowlist"
owner = "repo"
status = "advisory"

[[allow]]
id = "cargo-test-process"
binary = "cargo"
argv_shape = ["cargo", "test"]
network_reach = false
called_by = ["ci"]
owner = "release/ci"
reason = "CI executes workspace tests."
evidence = ["doc:docs/ci.md"]
created = "2026-05-09"
review_after = "2026-09-09"
expires = "permanent"
"#,
                entry_id: "cargo-test-process",
                kind: FindingKind::PolicyException,
                family: Some("process_spawn"),
                path: Some("ci"),
            },
            DispatchCase {
                label: "network",
                policy: r#"
policy = "network-allowlist"
owner = "repo"
status = "advisory"

[[allow]]
id = "crates-io-publish"
destination = "crates.io"
auth_required = true
auth_secret = "CARGO_REGISTRY_TOKEN"
lane = "release"
owner = "release"
reason = "Publish release artifacts."
evidence = ["doc:docs/release.md"]
created = "2026-05-09"
review_after = "2026-09-09"
"#,
                entry_id: "crates-io-publish",
                kind: FindingKind::PolicyException,
                family: Some("network_destination"),
                path: Some("policy/network-allowlist.toml"),
            },
        ]
    }

    #[test]
    fn dispatch_boundary_returns_some_for_each_supported_policy() {
        let non_rust = parse_table(
            r#"
policy = "non-rust-allowlist"
[[allow]]
id = "non-rust-readme"
path = "README.md"
owner = "docs"
reason = "Front door docs."
evidence = ["doc:README.md"]
expires = "permanent"
"#,
        );
        assert!(
            config_from_legacy_table(&non_rust)
                .unwrap_or_else(|err| {
                    std::panic::panic_any(format!("non-rust dispatch succeeds: {err}"))
                })
                .is_some()
        );

        let generated = parse_table(
            r#"
policy = "generated-allowlist"
[[allow]]
id = "generated-policy"
path = "policy/generated.toml"
owner = "release"
reason = "Generated fixture."
evidence = ["test:generated_policy"]
expires = "permanent"
"#,
        );
        assert!(
            config_from_legacy_table(&generated)
                .unwrap_or_else(|err| {
                    std::panic::panic_any(format!("generated dispatch succeeds: {err}"))
                })
                .is_some()
        );

        let no_panic_allowlist = parse_table(
            r#"
policy = "no-panic-allowlist"
[[allow]]
id = "panic-unwrap"
path = "src/lib.rs"
family = "unwrap"
owner = "runtime"
classification = "reviewed_panic_exception"
reason = "Input was checked."
evidence = ["test:panic_coverage"]
[allow.selector]
kind = "method-call"
"#,
        );
        assert!(
            config_from_legacy_table(&no_panic_allowlist)
                .unwrap_or_else(|err| {
                    std::panic::panic_any(format!("no-panic allowlist dispatch succeeds: {err}"))
                })
                .is_some()
        );

        let no_panic_baseline = parse_table(
            r#"
policy = "no-panic-baseline"
[[entry]]
path = "src/lib.rs"
family = "panic"
selector_kind = "macro-call"
selector_callee = "panic"
snippet = "panic!(\"bad\")"
count = 1
"#,
        );
        assert!(
            config_from_legacy_table(&no_panic_baseline)
                .unwrap_or_else(|err| {
                    std::panic::panic_any(format!("no-panic baseline dispatch succeeds: {err}"))
                })
                .is_some()
        );

        let clippy = parse_table(
            r#"
policy = "clippy-exceptions"
[[allow]]
id = "clippy-unwrap"
path = "src/lib.rs"
lint = "clippy::unwrap_used"
reason = "Intentional suppression."
"#,
        );
        assert!(
            config_from_legacy_table(&clippy)
                .unwrap_or_else(|err| {
                    std::panic::panic_any(format!("clippy dispatch succeeds: {err}"))
                })
                .is_some()
        );

        let unsafe_allowlist = parse_table(
            r#"
policy = "unsafe-allowlist"
[[allow]]
id = "unsafe-read"
path = "src/lib.rs"
family = "unsafe-block"
owner = "runtime"
classification = "reviewed_unsafe_boundary"
reason = "Caller checks pointer."
evidence = ["unsafe-review:read.json"]
"#,
        );
        assert!(
            config_from_legacy_table(&unsafe_allowlist)
                .unwrap_or_else(|err| {
                    std::panic::panic_any(format!("unsafe dispatch succeeds: {err}"))
                })
                .is_some()
        );

        let executable = parse_table(
            r#"
policy = "executable-allowlist"
[[allow]]
id = "exec-script"
path = "scripts/package-proof.sh"
owner = "release"
reason = "Release helper."
evidence = ["test:executable_policy"]
review_after = "2026-09-09"
"#,
        );
        assert!(
            config_from_legacy_table(&executable)
                .unwrap_or_else(|err| {
                    std::panic::panic_any(format!("executable dispatch succeeds: {err}"))
                })
                .is_some()
        );

        let workflow = parse_table(
            r#"
policy = "workflow-allowlist"
[[entry]]
path = ".github/workflows/ci.yml"
owner = "release/ci"
reason = "Primary CI lane."
external_actions = ["actions/checkout@v6.0.2"]
evidence = ["doc:docs/ci.md"]
review_after = "2026-09-09"
"#,
        );
        assert!(
            config_from_legacy_table(&workflow)
                .unwrap_or_else(|err| {
                    std::panic::panic_any(format!("workflow dispatch succeeds: {err}"))
                })
                .is_some()
        );

        let dependency = parse_table(
            r#"
policy = "dependency-surface-allowlist"
[[allow]]
id = "workspace-manifest"
path = "Cargo.toml"
owner = "deps"
reason = "Workspace manifest owns dependency declarations."
evidence = ["test:dependency_surface"]
review_after = "2026-09-09"
"#,
        );
        assert!(
            config_from_legacy_table(&dependency)
                .unwrap_or_else(|err| {
                    std::panic::panic_any(format!("dependency dispatch succeeds: {err}"))
                })
                .is_some()
        );

        let process = parse_table(
            r#"
policy = "process-allowlist"
[[allow]]
id = "cargo-test-process"
binary = "cargo"
argv_shape = ["cargo", "test"]
network_reach = false
owner = "release/ci"
reason = "CI executes workspace tests."
evidence = ["doc:docs/ci.md"]
created = "2026-05-09"
review_after = "2026-09-09"
"#,
        );
        assert!(
            config_from_legacy_table(&process)
                .unwrap_or_else(|err| {
                    std::panic::panic_any(format!("process dispatch succeeds: {err}"))
                })
                .is_some()
        );

        let network = parse_table(
            r#"
policy = "network-allowlist"
[[allow]]
id = "crates-io-publish"
destination = "crates.io"
auth_required = true
lane = "release"
owner = "release"
reason = "Publish release artifacts."
evidence = ["doc:docs/release.md"]
created = "2026-05-09"
review_after = "2026-09-09"
"#,
        );
        assert!(
            config_from_legacy_table(&network)
                .unwrap_or_else(|err| {
                    std::panic::panic_any(format!("network dispatch succeeds: {err}"))
                })
                .is_some()
        );
    }

    #[test]
    fn dispatches_file_policy_tables() {
        let non_rust = migrated_config(
            r#"
policy = "non-rust-allowlist"
owner = "repo"
status = "advisory"

[[allow]]
id = "non-rust-readme"
path = "README.md"
category = "documentation"
owner = "docs"
reason = "Front door docs."
evidence = ["doc:README.md"]
created = "2026-05-09"
expires = "permanent"
"#,
        );

        assert!(non_rust.allow.iter().any(|entry| {
            entry.id == "non-rust-readme"
                && entry.kind == FindingKind::NonRustFile
                && entry.path.as_deref() == Some(Path::new("README.md"))
        }));

        let generated = migrated_config(
            r#"
policy = "generated-allowlist"
owner = "repo"
status = "advisory"

[[allow]]
id = "generated-policy"
path = "policy/generated.toml"
owner = "release"
reason = "Generated fixture."
generator = "cargo xtask generate"
regenerate_command = "cargo xtask generate"
evidence = ["test:generated_policy"]
created = "2026-05-09"
expires = "permanent"
"#,
        );

        assert!(generated.allow.iter().any(|entry| {
            entry.id == "generated-policy"
                && entry.kind == FindingKind::GeneratedCode
                && entry.family.as_deref() == Some("generated_code")
        }));
    }

    #[test]
    fn dispatches_panic_policy_tables() {
        let allowlist = migrated_config(
            r#"
policy = "no-panic-allowlist"
owner = "repo"
status = "advisory"

[[allow]]
id = "panic-unwrap"
path = "src/lib.rs"
family = "unwrap"
owner = "runtime"
classification = "reviewed_panic_exception"
reason = "Input was checked."
evidence = ["test:panic_coverage"]
created = "2026-05-09"
review_after = "2026-09-09"

[allow.selector]
kind = "method-call"
callee = "unwrap"
"#,
        );

        assert!(allowlist.allow.iter().any(|entry| {
            entry.id == "panic-unwrap"
                && entry.kind == FindingKind::Panic
                && entry.family.as_deref() == Some("unwrap")
                && entry.selector.ast_kind.as_deref() == Some("method_call")
        }));

        let baseline = migrated_config(
            r#"
policy = "no-panic-baseline"
owner = "repo"
status = "advisory"

[[entry]]
path = "src/lib.rs"
family = "panic"
selector_kind = "macro-call"
selector_callee = "panic"
snippet = "panic!(\"bad\")"
count = 2
"#,
        );

        assert!(baseline.allow.iter().any(|entry| {
            entry.id == "panic-baseline-0001"
                && entry.kind == FindingKind::Panic
                && entry.occurrence_limit == Some(2)
        }));
    }

    #[test]
    fn dispatches_source_policy_tables() {
        let clippy = migrated_config(
            r#"
policy = "clippy-exceptions"
owner = "repo"
status = "advisory"

[[allow]]
id = "clippy-unwrap"
path = "src/lib.rs"
lint = "clippy::unwrap_used"
family = "expect"
owner = "lint"
classification = "reviewed_lint_exception"
reason = "Intentional suppression."
evidence = ["test:lint_coverage"]
created = "2026-05-09"
review_after = "2026-09-09"
"#,
        );

        assert!(clippy.allow.iter().any(|entry| {
            entry.id == "clippy-unwrap"
                && entry.kind == FindingKind::LintException
                && entry.selector.lint.as_deref() == Some("clippy::unwrap_used")
        }));

        let unsafe_allowlist = migrated_config(
            r#"
policy = "unsafe-allowlist"
owner = "repo"
status = "advisory"

[[allow]]
id = "unsafe-read"
path = "src/lib.rs"
family = "unsafe-block"
owner = "runtime"
classification = "reviewed_unsafe_boundary"
reason = "Caller checks pointer."
evidence = ["unsafe-review:read.json"]
created = "2026-05-09"
review_after = "2026-09-09"

[allow.selector]
kind = "unsafe-block"
container = "read"
"#,
        );

        assert!(unsafe_allowlist.allow.iter().any(|entry| {
            entry.id == "unsafe-read"
                && entry.kind == FindingKind::Unsafe
                && entry.family.as_deref() == Some("unsafe_block")
        }));
    }

    #[test]
    fn dispatches_policy_exception_tables() {
        let executable = migrated_config(
            r#"
policy = "executable-allowlist"
owner = "repo"
status = "advisory"

[[allow]]
id = "exec-script"
path = "scripts/package-proof.sh"
interpreter = "bash"
owner = "release"
reason = "Release helper."
evidence = ["test:executable_policy"]
created = "2026-05-09"
review_after = "2026-09-09"
"#,
        );

        assert!(executable.allow.iter().any(|entry| {
            entry.id == "exec-script"
                && entry.kind == FindingKind::PolicyException
                && entry.family.as_deref() == Some("executable_file")
        }));

        let workflow = migrated_config(
            r#"
policy = "workflow-allowlist"
owner = "repo"
status = "advisory"

[[entry]]
path = ".github/workflows/ci.yml"
owner = "release/ci"
reason = "Primary CI lane."
permissions = ["contents:read"]
secrets_used = []
external_actions = ["actions/checkout@v6.0.2"]
evidence = ["doc:docs/ci.md"]
created = "2026-05-09"
review_after = "2026-09-09"
"#,
        );

        assert!(workflow.allow.iter().any(|entry| {
            entry.kind == FindingKind::PolicyException
                && entry.family.as_deref() == Some("github_workflow")
                && entry.path.as_deref() == Some(Path::new(".github/workflows/ci.yml"))
        }));
        assert!(workflow.allow.iter().any(|entry| {
            entry.kind == FindingKind::PolicyException
                && entry.family.as_deref() == Some("workflow_external_action")
                && entry.selector.target_fingerprint.as_deref()
                    == Some("action:actions/checkout@v6.0.2")
        }));

        let dependency = migrated_config(
            r#"
policy = "dependency-surface-allowlist"
owner = "repo"
status = "advisory"

[[allow]]
id = "workspace-manifest"
path = "Cargo.toml"
surface = "dependency_surface"
owner = "deps"
reason = "Workspace manifest owns dependency declarations."
evidence = ["test:dependency_surface"]
created = "2026-05-09"
review_after = "2026-09-09"
dep_count_at_baseline = 4
"#,
        );

        assert!(dependency.allow.iter().any(|entry| {
            entry.id == "workspace-manifest"
                && entry.kind == FindingKind::PolicyException
                && entry.family.as_deref() == Some("dependency_surface")
        }));
    }

    #[test]
    fn dispatches_process_and_network_tables() {
        let process = migrated_config(
            r#"
policy = "process-allowlist"
owner = "repo"
status = "advisory"

[[allow]]
id = "cargo-test-process"
binary = "cargo"
argv_shape = ["cargo", "test"]
network_reach = false
called_by = ["ci"]
owner = "release/ci"
reason = "CI executes workspace tests."
evidence = ["doc:docs/ci.md"]
created = "2026-05-09"
review_after = "2026-09-09"
expires = "permanent"
"#,
        );

        assert!(process.allow.iter().any(|entry| {
            entry.id == "cargo-test-process"
                && entry.kind == FindingKind::PolicyException
                && entry.family.as_deref() == Some("process_spawn")
        }));

        let network = migrated_config(
            r#"
policy = "network-allowlist"
owner = "repo"
status = "advisory"

[[allow]]
id = "crates-io-publish"
destination = "crates.io"
auth_required = true
auth_secret = "CARGO_REGISTRY_TOKEN"
lane = "release"
owner = "release"
reason = "Publish release artifacts."
evidence = ["doc:docs/release.md"]
created = "2026-05-09"
review_after = "2026-09-09"
"#,
        );

        assert!(network.allow.iter().any(|entry| {
            entry.id == "crates-io-publish"
                && entry.kind == FindingKind::PolicyException
                && entry.family.as_deref() == Some("network_destination")
        }));
    }

    #[test]
    fn returns_none_for_unknown_or_missing_policy() {
        let unknown = parse_table("policy = \"unknown-policy\"");
        let unknown_result = config_from_legacy_table(&unknown)
            .unwrap_or_else(|err| std::panic::panic_any(format!("unknown policy checks: {err}")));
        assert!(unknown_result.is_none());

        let missing = parse_table("owner = \"repo\"");
        let missing_result = config_from_legacy_table(&missing)
            .unwrap_or_else(|err| std::panic::panic_any(format!("missing policy checks: {err}")));
        assert!(missing_result.is_none());
    }
}
