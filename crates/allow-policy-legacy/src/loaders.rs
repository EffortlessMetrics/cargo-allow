use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult, Finding};
use allow_policy::{parse_policy, validate_policy};
use std::path::Path;
use toml::Value;

use crate::converters::{
    config_from_clippy_rules, config_from_dependency_surface_rules, config_from_executable_rules,
    config_from_generated_rules, config_from_network_rules, config_from_no_panic_allowlist_entries,
    config_from_no_panic_baseline_entries, config_from_non_rust_rules, config_from_process_rules,
    config_from_unsafe_rules, config_from_workflow_rules,
};
use crate::io::{legacy_table, read_policy};
pub use crate::loader_compat::{
    load_clippy_exceptions_compat_config, load_dependency_surface_compat_config,
    load_executable_compat_config, load_generated_compat_config, load_network_compat_config,
    load_no_panic_allowlist_compat_config, load_no_panic_baseline_compat_config,
    load_non_rust_compat_config, load_process_compat_config, load_unsafe_allowlist_compat_config,
    load_workflow_compat_config,
};
use crate::parsers::{
    is_clippy_exceptions_policy, parse_clippy_rules, parse_dependency_surface_rules,
    parse_executable_rules, parse_generated_rules, parse_network_rules,
    parse_no_panic_allowlist_entries, parse_no_panic_baseline_entries, parse_non_rust_rules,
    parse_process_rules, parse_unsafe_rules, parse_workflow_rules,
};

const LEGACY_POLICY_FILES: &[&str] = &[
    "non-rust-allowlist.toml",
    "generated-allowlist.toml",
    "no-panic-allowlist.toml",
    "no-panic-baseline.toml",
    "clippy-exceptions.toml",
    "unsafe-allowlist.toml",
    "executable-allowlist.toml",
    "workflow-allowlist.toml",
    "dependency-surface-allowlist.toml",
    "process-allowlist.toml",
    "network-allowlist.toml",
];

pub fn load_legacy_or_canonical(path: impl AsRef<Path>) -> CargoAllowResult<AllowConfig> {
    let text = read_policy(path.as_ref())?;
    if let Some(table) = legacy_table(&text)?
        && table.get("policy").and_then(Value::as_str) == Some("non-rust-allowlist")
    {
        let rules = parse_non_rust_rules(&table)?;
        return config_from_non_rust_rules(&table, &rules);
    }
    if let Some(table) = legacy_table(&text)?
        && table.get("policy").and_then(Value::as_str) == Some("generated-allowlist")
    {
        let rules = parse_generated_rules(&table)?;
        return config_from_generated_rules(&table, &rules);
    }
    if let Some(table) = legacy_table(&text)?
        && table.get("policy").and_then(Value::as_str) == Some("no-panic-allowlist")
    {
        let entries = parse_no_panic_allowlist_entries(&table)?;
        return config_from_no_panic_allowlist_entries(&table, &entries);
    }
    if let Some(table) = legacy_table(&text)?
        && table.get("policy").and_then(Value::as_str) == Some("no-panic-baseline")
    {
        let entries = parse_no_panic_baseline_entries(&table)?;
        return config_from_no_panic_baseline_entries(&table, &entries);
    }
    if let Some(table) = legacy_table(&text)?
        && is_clippy_exceptions_policy(&table)
    {
        let rules = parse_clippy_rules(&table)?;
        return config_from_clippy_rules(&table, &rules);
    }
    if let Some(table) = legacy_table(&text)?
        && table.get("policy").and_then(Value::as_str) == Some("unsafe-allowlist")
    {
        let rules = parse_unsafe_rules(&table)?;
        return config_from_unsafe_rules(&table, &rules);
    }
    if let Some(table) = legacy_table(&text)?
        && table.get("policy").and_then(Value::as_str) == Some("executable-allowlist")
    {
        let rules = parse_executable_rules(&table)?;
        return config_from_executable_rules(&table, &rules);
    }
    if let Some(table) = legacy_table(&text)?
        && table.get("policy").and_then(Value::as_str) == Some("workflow-allowlist")
    {
        let rules = parse_workflow_rules(&table)?;
        return config_from_workflow_rules(&table, &rules);
    }
    if let Some(table) = legacy_table(&text)?
        && table.get("policy").and_then(Value::as_str) == Some("dependency-surface-allowlist")
    {
        let rules = parse_dependency_surface_rules(&table)?;
        return config_from_dependency_surface_rules(&table, &rules);
    }
    if let Some(table) = legacy_table(&text)?
        && table.get("policy").and_then(Value::as_str) == Some("process-allowlist")
    {
        let rules = parse_process_rules(&table)?;
        return config_from_process_rules(&table, &rules);
    }
    if let Some(table) = legacy_table(&text)?
        && table.get("policy").and_then(Value::as_str) == Some("network-allowlist")
    {
        let rules = parse_network_rules(&table)?;
        return config_from_network_rules(&table, &rules);
    }
    parse_policy(&text)
}

pub fn load_legacy_policy_dir(dir: impl AsRef<Path>) -> CargoAllowResult<AllowConfig> {
    load_legacy_policy_dir_inner(dir.as_ref(), None)
}

pub fn load_legacy_policy_dir_with_non_rust_findings(
    dir: impl AsRef<Path>,
    findings: &[Finding],
) -> CargoAllowResult<AllowConfig> {
    load_legacy_policy_dir_inner(dir.as_ref(), Some(findings))
}

fn load_legacy_policy_dir_inner(
    dir: &Path,
    non_rust_findings: Option<&[Finding]>,
) -> CargoAllowResult<AllowConfig> {
    if !dir.is_dir() {
        return Err(CargoAllowError::new(format!(
            "{} is not a policy directory",
            dir.display()
        )));
    }

    let mut merged = AllowConfig::empty();
    let mut loaded = 0usize;
    for file_name in LEGACY_POLICY_FILES {
        let path = dir.join(file_name);
        if !path.is_file() {
            continue;
        }
        let cfg = if *file_name == "non-rust-allowlist.toml" {
            if let Some(findings) = non_rust_findings {
                load_non_rust_compat_config(&path, findings)?
            } else {
                load_legacy_or_canonical(&path)?
            }
        } else {
            load_legacy_or_canonical(&path)?
        };
        if loaded == 0 {
            merged.owner = cfg.owner.clone();
            merged.status = cfg.status.clone();
            merged.workspace = cfg.workspace.clone();
            merged.requirements = cfg.requirements.clone();
        }
        loaded += 1;
        merged.allow.extend(cfg.allow);
    }

    if loaded == 0 {
        return Err(CargoAllowError::new(format!(
            "{} contains no supported legacy policy files",
            dir.display()
        )));
    }

    validate_policy(&merged)?;
    Ok(merged)
}

pub fn migration_notes() -> &'static str {
    "Legacy migration accepts canonical cargo-allow policies plus shiplog-style non-rust, generated, no-panic-allowlist, no-panic-baseline, clippy-exceptions, unsafe-allowlist, executable, workflow, dependency-surface, process, and network allowlists. Non-Rust compat expands matching legacy globs to exact current file entries; generated compat compares .gitattributes generated paths with policy/generated-allowlist.toml; no-panic allowlist migration maps retained source exceptions to structural panic receipts and treats last_seen as a hint only; no-panic baseline migration emits count-limited baseline_debt entries; clippy-exceptions compat maps retained lint suppression entries to source-syntax lint_exception receipts and uses cargo-allow's Rust source scanner for current findings; unsafe compat maps retained unsafe entries to source-syntax unsafe receipts and keeps missing evidence as temporary baseline_debt TODO evidence; executable compat compares git tree mode 100755 paths with policy/executable-allowlist.toml; workflow compat compares .github/workflows files and uses: actions with policy/workflow-allowlist.toml; dependency-surface compat preserves the legacy pattern-matches-tracked-file check; process compat validates retained process policy entries and does not scan source code for process spawns; network compat validates retained network policy entries and does not scan source code or runtime traffic."
}
