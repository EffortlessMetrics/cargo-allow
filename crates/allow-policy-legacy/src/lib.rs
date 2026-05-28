use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowResult, Finding, FindingKind, LastSeen,
    Lifecycle, Requirements, Selector, SimpleDate, WorkspaceConfig, normalize_path,
    normalize_snippet, stable_hash_hex,
};
use allow_policy::{parse_policy, validate_policy};
use std::path::{Path, PathBuf};
use toml::Value;

mod fields;
mod findings;
mod io;
mod types;
use fields::{
    legacy_evidence, optional_last_seen, optional_u32_field, raw_string_field, required_bool_field,
    required_string_array_field, required_string_field, string_array_field, string_field,
};
#[cfg(test)]
use findings::{
    dependency_surface_finding, executable_finding, executable_findings_from_git_stage,
    generated_finding, workflow_action_finding, workflow_file_finding,
};
pub use findings::{
    dependency_surface_findings_from_git, executable_findings_from_git,
    generated_findings_from_gitattributes, network_findings_from_config,
    process_findings_from_config, workflow_findings_from_files,
};
use findings::{file_fingerprint, workflow_action_symbol};
use io::{legacy_table, read_policy};
use types::{
    LegacyClippyRule, LegacyDependencySurfaceRule, LegacyExecutableRule, LegacyGeneratedRule,
    LegacyNetworkRule, LegacyNoPanicAllowEntry, LegacyNoPanicBaselineEntry, LegacyNonRustRule,
    LegacyProcessRule, LegacyUnsafeRule, LegacyWorkflowRule,
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
const BASELINE_DEBT_DEFAULT_DAYS: i64 = 67;

fn default_baseline_created() -> String {
    SimpleDate::today_utc_approx().to_string()
}

fn default_baseline_expires() -> String {
    SimpleDate::today_utc_approx()
        .add_days(BASELINE_DEBT_DEFAULT_DAYS)
        .to_string()
}

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

pub fn load_non_rust_compat_config(
    path: impl AsRef<Path>,
    findings: &[Finding],
) -> CargoAllowResult<AllowConfig> {
    let text = read_policy(path.as_ref())?;
    let table = legacy_table(&text)?.ok_or_else(|| {
        CargoAllowError::new(format!("{} is not a TOML table", path.as_ref().display()))
    })?;
    if table.get("policy").and_then(Value::as_str) != Some("non-rust-allowlist") {
        return Err(CargoAllowError::new(format!(
            "{} is not a non-rust-allowlist policy",
            path.as_ref().display()
        )));
    }
    let rules = parse_non_rust_rules(&table)?;
    let cfg = config_from_current_non_rust_findings(&table, &rules, findings)?;
    Ok(cfg)
}

pub fn load_generated_compat_config(path: impl AsRef<Path>) -> CargoAllowResult<AllowConfig> {
    let text = read_policy(path.as_ref())?;
    let table = legacy_table(&text)?.ok_or_else(|| {
        CargoAllowError::new(format!("{} is not a TOML table", path.as_ref().display()))
    })?;
    if table.get("policy").and_then(Value::as_str) != Some("generated-allowlist") {
        return Err(CargoAllowError::new(format!(
            "{} is not a generated-allowlist policy",
            path.as_ref().display()
        )));
    }
    let rules = parse_generated_rules(&table)?;
    config_from_generated_rules(&table, &rules)
}

pub fn load_no_panic_baseline_compat_config(
    path: impl AsRef<Path>,
) -> CargoAllowResult<AllowConfig> {
    let text = read_policy(path.as_ref())?;
    let table = legacy_table(&text)?.ok_or_else(|| {
        CargoAllowError::new(format!("{} is not a TOML table", path.as_ref().display()))
    })?;
    if table.get("policy").and_then(Value::as_str) != Some("no-panic-baseline") {
        return Err(CargoAllowError::new(format!(
            "{} is not a no-panic-baseline policy",
            path.as_ref().display()
        )));
    }
    let entries = parse_no_panic_baseline_entries(&table)?;
    config_from_no_panic_baseline_entries(&table, &entries)
}

pub fn load_no_panic_allowlist_compat_config(
    path: impl AsRef<Path>,
) -> CargoAllowResult<AllowConfig> {
    let text = read_policy(path.as_ref())?;
    let table = legacy_table(&text)?.ok_or_else(|| {
        CargoAllowError::new(format!("{} is not a TOML table", path.as_ref().display()))
    })?;
    if table.get("policy").and_then(Value::as_str) != Some("no-panic-allowlist") {
        return Err(CargoAllowError::new(format!(
            "{} is not a no-panic-allowlist policy",
            path.as_ref().display()
        )));
    }
    let entries = parse_no_panic_allowlist_entries(&table)?;
    config_from_no_panic_allowlist_entries(&table, &entries)
}

pub fn load_clippy_exceptions_compat_config(
    path: impl AsRef<Path>,
) -> CargoAllowResult<AllowConfig> {
    let text = read_policy(path.as_ref())?;
    let table = legacy_table(&text)?.ok_or_else(|| {
        CargoAllowError::new(format!("{} is not a TOML table", path.as_ref().display()))
    })?;
    if !is_clippy_exceptions_policy(&table) {
        return Err(CargoAllowError::new(format!(
            "{} is not a clippy-exceptions policy",
            path.as_ref().display()
        )));
    }
    let rules = parse_clippy_rules(&table)?;
    config_from_clippy_rules(&table, &rules)
}

pub fn load_unsafe_allowlist_compat_config(
    path: impl AsRef<Path>,
) -> CargoAllowResult<AllowConfig> {
    let text = read_policy(path.as_ref())?;
    let table = legacy_table(&text)?.ok_or_else(|| {
        CargoAllowError::new(format!("{} is not a TOML table", path.as_ref().display()))
    })?;
    if table.get("policy").and_then(Value::as_str) != Some("unsafe-allowlist") {
        return Err(CargoAllowError::new(format!(
            "{} is not an unsafe-allowlist policy",
            path.as_ref().display()
        )));
    }
    let rules = parse_unsafe_rules(&table)?;
    config_from_unsafe_rules(&table, &rules)
}

pub fn load_executable_compat_config(path: impl AsRef<Path>) -> CargoAllowResult<AllowConfig> {
    let text = read_policy(path.as_ref())?;
    let table = legacy_table(&text)?.ok_or_else(|| {
        CargoAllowError::new(format!("{} is not a TOML table", path.as_ref().display()))
    })?;
    if table.get("policy").and_then(Value::as_str) != Some("executable-allowlist") {
        return Err(CargoAllowError::new(format!(
            "{} is not an executable-allowlist policy",
            path.as_ref().display()
        )));
    }
    let rules = parse_executable_rules(&table)?;
    config_from_executable_rules(&table, &rules)
}

pub fn load_workflow_compat_config(path: impl AsRef<Path>) -> CargoAllowResult<AllowConfig> {
    let text = read_policy(path.as_ref())?;
    let table = legacy_table(&text)?.ok_or_else(|| {
        CargoAllowError::new(format!("{} is not a TOML table", path.as_ref().display()))
    })?;
    if table.get("policy").and_then(Value::as_str) != Some("workflow-allowlist") {
        return Err(CargoAllowError::new(format!(
            "{} is not a workflow-allowlist policy",
            path.as_ref().display()
        )));
    }
    let rules = parse_workflow_rules(&table)?;
    config_from_workflow_rules(&table, &rules)
}

pub fn load_dependency_surface_compat_config(
    path: impl AsRef<Path>,
) -> CargoAllowResult<AllowConfig> {
    let text = read_policy(path.as_ref())?;
    let table = legacy_table(&text)?.ok_or_else(|| {
        CargoAllowError::new(format!("{} is not a TOML table", path.as_ref().display()))
    })?;
    if table.get("policy").and_then(Value::as_str) != Some("dependency-surface-allowlist") {
        return Err(CargoAllowError::new(format!(
            "{} is not a dependency-surface-allowlist policy",
            path.as_ref().display()
        )));
    }
    let rules = parse_dependency_surface_rules(&table)?;
    config_from_dependency_surface_rules(&table, &rules)
}

pub fn load_process_compat_config(path: impl AsRef<Path>) -> CargoAllowResult<AllowConfig> {
    let text = read_policy(path.as_ref())?;
    let table = legacy_table(&text)?.ok_or_else(|| {
        CargoAllowError::new(format!("{} is not a TOML table", path.as_ref().display()))
    })?;
    if table.get("policy").and_then(Value::as_str) != Some("process-allowlist") {
        return Err(CargoAllowError::new(format!(
            "{} is not a process-allowlist policy",
            path.as_ref().display()
        )));
    }
    let rules = parse_process_rules(&table)?;
    config_from_process_rules(&table, &rules)
}

pub fn load_network_compat_config(path: impl AsRef<Path>) -> CargoAllowResult<AllowConfig> {
    let text = read_policy(path.as_ref())?;
    let table = legacy_table(&text)?.ok_or_else(|| {
        CargoAllowError::new(format!("{} is not a TOML table", path.as_ref().display()))
    })?;
    if table.get("policy").and_then(Value::as_str) != Some("network-allowlist") {
        return Err(CargoAllowError::new(format!(
            "{} is not a network-allowlist policy",
            path.as_ref().display()
        )));
    }
    let rules = parse_network_rules(&table)?;
    config_from_network_rules(&table, &rules)
}

pub fn migration_notes() -> &'static str {
    "Legacy migration accepts canonical cargo-allow policies plus shiplog-style non-rust, generated, no-panic-allowlist, no-panic-baseline, clippy-exceptions, unsafe-allowlist, executable, workflow, dependency-surface, process, and network allowlists. Non-Rust compat expands matching legacy globs to exact current file entries; generated compat compares .gitattributes generated paths with policy/generated-allowlist.toml; no-panic allowlist migration maps retained source exceptions to structural panic receipts and treats last_seen as a hint only; no-panic baseline migration emits count-limited baseline_debt entries; clippy-exceptions compat maps retained lint suppression entries to source-syntax lint_exception receipts and uses cargo-allow's Rust source scanner for current findings; unsafe compat maps retained unsafe entries to source-syntax unsafe receipts and keeps missing evidence as temporary baseline_debt TODO evidence; executable compat compares git tree mode 100755 paths with policy/executable-allowlist.toml; workflow compat compares .github/workflows files and uses: actions with policy/workflow-allowlist.toml; dependency-surface compat preserves the legacy pattern-matches-tracked-file check; process compat validates retained process policy entries and does not scan source code for process spawns; network compat validates retained network policy entries and does not scan source code or runtime traffic."
}

fn parse_non_rust_rules(table: &toml::Table) -> CargoAllowResult<Vec<LegacyNonRustRule>> {
    let entries = table
        .get("allow")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("non-rust-allowlist missing allow entries"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_non_rust_rule(index, entry))
        .collect()
}

fn parse_non_rust_rule(index: usize, entry: &Value) -> CargoAllowResult<LegacyNonRustRule> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("non-rust allow entry {index} is not a table"))
    })?;
    let id = string_field(table, "id").unwrap_or_else(|| format!("legacy-non-rust-{index:04}"));
    let (pattern, is_path) = match (string_field(table, "path"), string_field(table, "glob")) {
        (Some(path), None) => (path, true),
        (None, Some(glob)) => (glob, false),
        (Some(path), Some(_)) => (path, true),
        (None, None) => {
            return Err(CargoAllowError::new(format!("{id} missing path or glob")));
        }
    };
    let reason_field = string_field(table, "reason");
    let raw_broad_glob_reason = raw_string_field(table, "broad_glob_reason");
    let broad_glob_reason = raw_broad_glob_reason
        .as_deref()
        .map(str::trim)
        .filter(|reason| !reason.is_empty())
        .map(str::to_string);
    if !is_path && is_broad_legacy_glob(&pattern) {
        match raw_broad_glob_reason.as_deref() {
            None => {
                return Err(CargoAllowError::new(format!(
                    "{id} broad glob `{pattern}` requires broad_glob_reason"
                )));
            }
            Some(reason) if reason.trim().is_empty() => {
                return Err(CargoAllowError::new(format!(
                    "{id} broad glob `{pattern}` has empty broad_glob_reason"
                )));
            }
            Some(_) => {}
        }
    }
    let reason = match (reason_field, broad_glob_reason) {
        (Some(reason), Some(scope_reason)) if !scope_reason.trim().is_empty() => {
            format!("{reason} Scope note: {scope_reason}")
        }
        (Some(reason), _) => reason,
        (None, Some(scope_reason)) => scope_reason,
        (None, None) => String::new(),
    };
    Ok(LegacyNonRustRule {
        id: id.clone(),
        pattern,
        is_path,
        owner: string_field(table, "owner").unwrap_or_default(),
        classification: string_field(table, "category")
            .unwrap_or_else(|| "legacy_non_rust".to_string()),
        reason,
        created: string_field(table, "created"),
        review_after: string_field(table, "review_after"),
        expires: normalize_legacy_expires(string_field(table, "expires")),
    })
}

fn is_broad_legacy_glob(pattern: &str) -> bool {
    pattern.contains('*')
}

fn parse_generated_rules(table: &toml::Table) -> CargoAllowResult<Vec<LegacyGeneratedRule>> {
    let entries = table
        .get("allow")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("generated-allowlist missing allow entries"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_generated_rule(index, entry))
        .collect()
}

fn parse_generated_rule(index: usize, entry: &Value) -> CargoAllowResult<LegacyGeneratedRule> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("generated allow entry {index} is not a table"))
    })?;
    let id = string_field(table, "id").unwrap_or_else(|| format!("legacy-generated-{index:04}"));
    let path = string_field(table, "path")
        .ok_or_else(|| CargoAllowError::new(format!("{id} missing path")))?;
    Ok(LegacyGeneratedRule {
        id,
        path,
        owner: string_field(table, "owner").unwrap_or_default(),
        reason: string_field(table, "reason").unwrap_or_default(),
        generator: string_field(table, "generator"),
        regenerate_command: string_field(table, "regenerate_command"),
        created: string_field(table, "created"),
        expires: normalize_legacy_expires(string_field(table, "expires")),
    })
}

fn parse_no_panic_baseline_entries(
    table: &toml::Table,
) -> CargoAllowResult<Vec<LegacyNoPanicBaselineEntry>> {
    let entries = table
        .get("entry")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("no-panic-baseline missing entry records"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_no_panic_baseline_entry(index, entry))
        .collect()
}

fn parse_no_panic_baseline_entry(
    index: usize,
    entry: &Value,
) -> CargoAllowResult<LegacyNoPanicBaselineEntry> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("no-panic baseline entry {index} is not a table"))
    })?;
    let context = format!("no-panic baseline entry {index}");
    let count = table
        .get("count")
        .and_then(Value::as_integer)
        .filter(|value| *value > 0)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| CargoAllowError::new(format!("{context} missing count")))?;
    Ok(LegacyNoPanicBaselineEntry {
        index,
        path: required_string_field(table, "path", &context)?,
        family: required_string_field(table, "family", &context)?,
        selector_kind: required_string_field(table, "selector_kind", &context)?,
        selector_callee: required_string_field(table, "selector_callee", &context)?,
        snippet: required_string_field(table, "snippet", &context)?,
        count,
    })
}

fn parse_no_panic_allowlist_entries(
    table: &toml::Table,
) -> CargoAllowResult<Vec<LegacyNoPanicAllowEntry>> {
    let entries = table
        .get("allow")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("no-panic-allowlist missing allow entries"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_no_panic_allowlist_entry(index, entry))
        .collect()
}

fn parse_no_panic_allowlist_entry(
    index: usize,
    entry: &Value,
) -> CargoAllowResult<LegacyNoPanicAllowEntry> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("no-panic allow entry {index} is not a table"))
    })?;
    let id = string_field(table, "id").unwrap_or_else(|| format!("legacy-no-panic-{index:04}"));
    let selector = table.get("selector").and_then(Value::as_table);
    let last_seen_table = table.get("last_seen").and_then(Value::as_table);
    let review_after = string_field(table, "review_after");
    let expires = normalize_legacy_expires(string_field(table, "expires"))
        .or_else(|| review_after.is_none().then(default_baseline_expires));
    let last_seen = optional_last_seen(last_seen_table);
    Ok(LegacyNoPanicAllowEntry {
        index,
        id: id.clone(),
        path: required_string_field(table, "path", &id)?,
        family: required_string_field(table, "family", &id)?,
        selector_kind: selector
            .and_then(|selector| {
                string_field(selector, "kind").or_else(|| string_field(selector, "ast_kind"))
            })
            .ok_or_else(|| CargoAllowError::new(format!("{id} missing selector.kind")))?,
        selector_callee: selector.and_then(|selector| string_field(selector, "callee")),
        selector_container: selector.and_then(|selector| string_field(selector, "container")),
        owner: string_field(table, "owner").unwrap_or_else(|| "unowned".to_string()),
        classification: string_field(table, "classification")
            .unwrap_or_else(|| "baseline_debt".to_string()),
        reason: string_field(table, "reason")
            .or_else(|| string_field(table, "explanation"))
            .unwrap_or_else(|| {
                "Generated from legacy no-panic allowlist; requires human review.".to_string()
            }),
        created: string_field(table, "created").or_else(|| Some(default_baseline_created())),
        review_after,
        expires,
        line_hint: selector
            .and_then(|selector| optional_u32_field(selector, "line_hint"))
            .or_else(|| last_seen.as_ref().map(|seen| seen.line)),
        last_seen,
    })
}

fn parse_clippy_rules(table: &toml::Table) -> CargoAllowResult<Vec<LegacyClippyRule>> {
    let entries = table
        .get("allow")
        .or_else(|| table.get("entry"))
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("clippy-exceptions missing allow entries"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_clippy_rule(index, entry))
        .collect()
}

fn parse_clippy_rule(index: usize, entry: &Value) -> CargoAllowResult<LegacyClippyRule> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("clippy exception entry {index} is not a table"))
    })?;
    let id = string_field(table, "id").unwrap_or_else(|| format!("legacy-clippy-{index:04}"));
    let review_after = string_field(table, "review_after");
    let expires = normalize_legacy_expires(string_field(table, "expires"))
        .or_else(|| review_after.is_none().then(default_baseline_expires));
    Ok(LegacyClippyRule {
        path: required_string_field(table, "path", &id)?,
        lint: required_string_field(table, "lint", &id)?,
        family: string_field(table, "family")
            .or_else(|| string_field(table, "attribute"))
            .map(|family| normalize_lint_attribute_family(&family))
            .unwrap_or_else(|| "expect_attribute".to_string()),
        owner: string_field(table, "owner").unwrap_or_else(|| "unowned".to_string()),
        classification: string_field(table, "classification")
            .unwrap_or_else(|| "baseline_debt".to_string()),
        reason: string_field(table, "reason").unwrap_or_else(|| {
            "Generated from legacy Clippy exceptions policy; requires human review.".to_string()
        }),
        symbol: string_field(table, "symbol"),
        target_fingerprint: string_field(table, "target_fingerprint")
            .or_else(|| string_field(table, "policy_id").map(|id| format!("policy:{id}"))),
        created: string_field(table, "created").or_else(|| Some(default_baseline_created())),
        review_after,
        expires,
        id,
    })
}

fn parse_unsafe_rules(table: &toml::Table) -> CargoAllowResult<Vec<LegacyUnsafeRule>> {
    let entries = table
        .get("allow")
        .or_else(|| table.get("entry"))
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("unsafe-allowlist missing allow entries"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_unsafe_rule(index, entry))
        .collect()
}

fn parse_unsafe_rule(index: usize, entry: &Value) -> CargoAllowResult<LegacyUnsafeRule> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("unsafe allow entry {index} is not a table"))
    })?;
    let id = string_field(table, "id").unwrap_or_else(|| format!("legacy-unsafe-{index:04}"));
    let selector = table.get("selector").and_then(Value::as_table);
    let last_seen_table = table.get("last_seen").and_then(Value::as_table);
    let review_after = string_field(table, "review_after");
    let expires = normalize_legacy_expires(string_field(table, "expires"))
        .or_else(|| review_after.is_none().then(default_baseline_expires));
    let family = string_field(table, "family")
        .or_else(|| {
            selector.and_then(|selector| {
                string_field(selector, "kind").or_else(|| string_field(selector, "ast_kind"))
            })
        })
        .ok_or_else(|| CargoAllowError::new(format!("{id} missing family or selector.kind")))?;
    let family = normalize_unsafe_family(&family);
    let selector_kind = selector
        .and_then(|selector| {
            string_field(selector, "kind").or_else(|| string_field(selector, "ast_kind"))
        })
        .map(|kind| normalize_unsafe_family(&kind))
        .unwrap_or_else(|| family.clone());
    let last_seen = optional_last_seen(last_seen_table);
    Ok(LegacyUnsafeRule {
        id: id.clone(),
        path: required_string_field(table, "path", &id)?,
        family,
        selector_kind,
        selector_container: selector.and_then(|selector| string_field(selector, "container")),
        owner: string_field(table, "owner").unwrap_or_else(|| "unowned".to_string()),
        classification: string_field(table, "classification")
            .unwrap_or_else(|| "baseline_debt".to_string()),
        reason: string_field(table, "reason")
            .or_else(|| string_field(table, "explanation"))
            .unwrap_or_else(|| {
                "Generated from legacy unsafe allowlist; requires human review.".to_string()
            }),
        evidence: legacy_evidence(table),
        created: string_field(table, "created").or_else(|| Some(default_baseline_created())),
        review_after,
        expires,
        line_hint: selector
            .and_then(|selector| optional_u32_field(selector, "line_hint"))
            .or_else(|| last_seen.as_ref().map(|seen| seen.line)),
        last_seen,
    })
}

fn parse_executable_rules(table: &toml::Table) -> CargoAllowResult<Vec<LegacyExecutableRule>> {
    let entries = table
        .get("allow")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("executable-allowlist missing allow entries"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_executable_rule(index, entry))
        .collect()
}

fn parse_executable_rule(index: usize, entry: &Value) -> CargoAllowResult<LegacyExecutableRule> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("executable allow entry {index} is not a table"))
    })?;
    let id = string_field(table, "id").unwrap_or_else(|| format!("legacy-executable-{index:04}"));
    let path = string_field(table, "path")
        .ok_or_else(|| CargoAllowError::new(format!("{id} missing path")))?;
    Ok(LegacyExecutableRule {
        id,
        path,
        owner: string_field(table, "owner").unwrap_or_default(),
        reason: string_field(table, "reason").unwrap_or_default(),
        interpreter: string_field(table, "interpreter"),
        created: string_field(table, "created"),
        review_after: string_field(table, "review_after"),
        expires: normalize_legacy_expires(string_field(table, "expires")),
    })
}

fn parse_workflow_rules(table: &toml::Table) -> CargoAllowResult<Vec<LegacyWorkflowRule>> {
    let entries = table
        .get("entry")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("workflow-allowlist missing entry records"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_workflow_rule(index, entry))
        .collect()
}

fn parse_workflow_rule(index: usize, entry: &Value) -> CargoAllowResult<LegacyWorkflowRule> {
    let table = entry
        .as_table()
        .ok_or_else(|| CargoAllowError::new(format!("workflow entry {index} is not a table")))?;
    let path = string_field(table, "path")
        .ok_or_else(|| CargoAllowError::new(format!("workflow entry {index} missing path")))?;
    Ok(LegacyWorkflowRule {
        path,
        owner: string_field(table, "owner").unwrap_or_default(),
        reason: string_field(table, "reason").unwrap_or_default(),
        permissions: string_array_field(table, "permissions"),
        secrets_used: string_array_field(table, "secrets_used"),
        external_actions: string_array_field(table, "external_actions"),
        duplicate_of_lane: string_field(table, "duplicate_of_lane"),
        created: string_field(table, "created"),
        review_after: string_field(table, "review_after"),
        expires: normalize_legacy_expires(string_field(table, "expires")),
    })
}

fn parse_dependency_surface_rules(
    table: &toml::Table,
) -> CargoAllowResult<Vec<LegacyDependencySurfaceRule>> {
    let entries = table
        .get("allow")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            CargoAllowError::new("dependency-surface-allowlist missing allow entries")
        })?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_dependency_surface_rule(index, entry))
        .collect()
}

fn parse_dependency_surface_rule(
    index: usize,
    entry: &Value,
) -> CargoAllowResult<LegacyDependencySurfaceRule> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!(
            "dependency-surface allow entry {index} is not a table"
        ))
    })?;
    let id = string_field(table, "id").unwrap_or_else(|| format!("legacy-dependency-{index:04}"));
    let pattern = string_field(table, "path")
        .or_else(|| string_field(table, "glob"))
        .ok_or_else(|| CargoAllowError::new(format!("{id} missing path or glob")))?;
    Ok(LegacyDependencySurfaceRule {
        id,
        is_glob: has_glob_meta(&pattern),
        pattern,
        surface: string_field(table, "surface").unwrap_or_else(|| "dependency_surface".to_string()),
        owner: string_field(table, "owner").unwrap_or_default(),
        reason: string_field(table, "reason").unwrap_or_default(),
        broad_glob_reason: string_field(table, "broad_glob_reason"),
        dep_count_at_baseline: table
            .get("dep_count_at_baseline")
            .and_then(Value::as_integer),
        created: string_field(table, "created"),
        review_after: string_field(table, "review_after"),
        expires: normalize_legacy_expires(string_field(table, "expires")),
    })
}

fn parse_process_rules(table: &toml::Table) -> CargoAllowResult<Vec<LegacyProcessRule>> {
    let entries = table
        .get("allow")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("process-allowlist missing allow entries"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_process_rule(index, entry))
        .collect()
}

fn parse_process_rule(index: usize, entry: &Value) -> CargoAllowResult<LegacyProcessRule> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("process allow entry {index} is not a table"))
    })?;
    let id = required_string_field(table, "id", &format!("process allow entry {index}"))?;
    Ok(LegacyProcessRule {
        binary: required_string_field(table, "binary", &id)?,
        argv_shape: required_string_array_field(table, "argv_shape", &id)?,
        network_reach: required_bool_field(table, "network_reach", &id)?,
        called_by: string_array_field(table, "called_by"),
        owner: required_string_field(table, "owner", &id)?,
        reason: required_string_field(table, "reason", &id)?,
        created: Some(required_string_field(table, "created", &id)?),
        review_after: string_field(table, "review_after"),
        expires: normalize_legacy_expires(string_field(table, "expires")),
        id,
    })
}

fn parse_network_rules(table: &toml::Table) -> CargoAllowResult<Vec<LegacyNetworkRule>> {
    let entries = table
        .get("allow")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("network-allowlist missing allow entries"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_network_rule(index, entry))
        .collect()
}

fn parse_network_rule(index: usize, entry: &Value) -> CargoAllowResult<LegacyNetworkRule> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("network allow entry {index} is not a table"))
    })?;
    let id = required_string_field(table, "id", &format!("network allow entry {index}"))?;
    Ok(LegacyNetworkRule {
        destination: required_string_field(table, "destination", &id)?,
        auth_required: required_bool_field(table, "auth_required", &id)?,
        auth_secret: string_field(table, "auth_secret"),
        lane: required_string_field(table, "lane", &id)?,
        owner: required_string_field(table, "owner", &id)?,
        reason: required_string_field(table, "reason", &id)?,
        created: Some(required_string_field(table, "created", &id)?),
        review_after: string_field(table, "review_after"),
        expires: normalize_legacy_expires(string_field(table, "expires")),
        id,
    })
}

fn config_from_non_rust_rules(
    table: &toml::Table,
    rules: &[LegacyNonRustRule],
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = rules.iter().map(entry_from_rule).collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

fn config_from_generated_rules(
    table: &toml::Table,
    rules: &[LegacyGeneratedRule],
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = rules.iter().map(entry_from_generated_rule).collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

fn config_from_no_panic_baseline_entries(
    table: &toml::Table,
    entries: &[LegacyNoPanicBaselineEntry],
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = entries
        .iter()
        .map(entry_from_no_panic_baseline_entry)
        .collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

fn config_from_no_panic_allowlist_entries(
    table: &toml::Table,
    entries: &[LegacyNoPanicAllowEntry],
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = entries
        .iter()
        .map(entry_from_no_panic_allow_entry)
        .collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

fn config_from_clippy_rules(
    table: &toml::Table,
    rules: &[LegacyClippyRule],
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = rules.iter().map(entry_from_clippy_rule).collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

fn config_from_unsafe_rules(
    table: &toml::Table,
    rules: &[LegacyUnsafeRule],
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = rules.iter().map(entry_from_unsafe_rule).collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

fn config_from_executable_rules(
    table: &toml::Table,
    rules: &[LegacyExecutableRule],
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = rules.iter().map(entry_from_executable_rule).collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

fn config_from_workflow_rules(
    table: &toml::Table,
    rules: &[LegacyWorkflowRule],
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = rules.iter().flat_map(entries_from_workflow_rule).collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

fn config_from_dependency_surface_rules(
    table: &toml::Table,
    rules: &[LegacyDependencySurfaceRule],
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = rules
        .iter()
        .map(entry_from_dependency_surface_rule)
        .collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

fn config_from_process_rules(
    table: &toml::Table,
    rules: &[LegacyProcessRule],
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = rules.iter().map(entry_from_process_rule).collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

fn config_from_network_rules(
    table: &toml::Table,
    rules: &[LegacyNetworkRule],
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = rules.iter().map(entry_from_network_rule).collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

fn config_from_current_non_rust_findings(
    table: &toml::Table,
    rules: &[LegacyNonRustRule],
    findings: &[Finding],
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = findings
        .iter()
        .enumerate()
        .filter_map(|(index, finding)| {
            best_rule_index(rules, finding)
                .and_then(|rule_index| rules.get(rule_index))
                .map(|rule| entry_from_finding(rule, finding, index + 1))
        })
        .collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

fn base_config(table: &toml::Table) -> AllowConfig {
    AllowConfig {
        schema_version: "0.1".to_string(),
        policy: "cargo-allow".to_string(),
        owner: string_field(table, "owner"),
        status: string_field(table, "status"),
        workspace: WorkspaceConfig::default(),
        requirements: Requirements::default(),
        allow: Vec::new(),
    }
}

fn best_rule_index(rules: &[LegacyNonRustRule], finding: &Finding) -> Option<usize> {
    rules
        .iter()
        .enumerate()
        .filter(|(_, rule)| rule.matches(finding))
        .max_by_key(|(_, rule)| rule.specificity())
        .map(|(index, _)| index)
}

fn entry_from_rule(rule: &LegacyNonRustRule) -> AllowEntry {
    let (path, glob) = if rule.is_path {
        (Some(PathBuf::from(&rule.pattern)), None)
    } else {
        (None, Some(rule.pattern.clone()))
    };
    AllowEntry {
        id: rule.id.clone(),
        kind: FindingKind::NonRustFile,
        family: None,
        path,
        glob: glob.clone(),
        owner: rule.owner.clone(),
        classification: rule.classification.clone(),
        reason: rule.reason.clone(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: lifecycle_from_rule(rule),
        selector: Selector {
            glob: Some(rule.pattern.clone()),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn entry_from_finding(rule: &LegacyNonRustRule, finding: &Finding, index: usize) -> AllowEntry {
    let path = normalize_path(&finding.path);
    AllowEntry {
        id: format!("{}--{index:04}", rule.id),
        kind: finding.kind,
        family: None,
        path: Some(PathBuf::from(&path)),
        glob: None,
        owner: rule.owner.clone(),
        classification: rule.classification.clone(),
        reason: rule.reason.clone(),
        evidence: Vec::new(),
        links: vec![format!("legacy-policy:{}", rule.id)],
        occurrence_limit: None,
        lifecycle: lifecycle_from_rule(rule),
        selector: Selector {
            ast_kind: Some(finding.identity.ast_kind.clone()),
            symbol: Some(path.clone()),
            glob: Some(path),
            ..Selector::default()
        },
        last_seen: finding.span.as_ref().map(|span| LastSeen {
            line: span.line,
            column: span.column,
        }),
    }
}

fn entry_from_generated_rule(rule: &LegacyGeneratedRule) -> AllowEntry {
    let path = normalize_path(&rule.path);
    AllowEntry {
        id: rule.id.clone(),
        kind: FindingKind::GeneratedCode,
        family: Some("generated_code".to_string()),
        path: Some(PathBuf::from(&path)),
        glob: None,
        owner: rule.owner.clone(),
        classification: "generated_code".to_string(),
        reason: rule.reason.clone(),
        evidence: generated_evidence(rule),
        links: vec![format!("legacy-policy:{}", rule.id)],
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: rule.created.clone(),
            review_after: None,
            expires: rule.expires.clone(),
        },
        selector: Selector {
            ast_kind: Some("tracked_file".to_string()),
            symbol: Some(path.clone()),
            target_fingerprint: file_fingerprint(Path::new(&path)),
            glob: Some(path),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn entry_from_no_panic_baseline_entry(rule: &LegacyNoPanicBaselineEntry) -> AllowEntry {
    let path = normalize_path(&rule.path);
    let family = cargo_allow_panic_family(&rule.family);
    let ast_kind = normalize_selector_kind(&rule.selector_kind);
    let snippet_hash = stable_hash_hex(&normalize_snippet(&rule.snippet));
    AllowEntry {
        id: format!("panic-baseline-{:04}", rule.index + 1),
        kind: FindingKind::Panic,
        family: Some(family.clone()),
        path: Some(PathBuf::from(&path)),
        glob: None,
        owner: "unowned".to_string(),
        classification: "baseline_debt".to_string(),
        reason: "Generated from legacy no-panic baseline; requires human review.".to_string(),
        evidence: vec![
            "legacy_policy:no-panic-baseline".to_string(),
            format!("legacy_selector_callee:{}", rule.selector_callee),
            format!("baseline_count:{}", rule.count),
        ],
        links: vec!["legacy-policy:no-panic-baseline".to_string()],
        occurrence_limit: Some(rule.count),
        lifecycle: Lifecycle {
            created: Some(default_baseline_created()),
            review_after: None,
            expires: Some(default_baseline_expires()),
        },
        selector: Selector {
            ast_kind: Some(ast_kind.clone()),
            callee: (ast_kind == "method_call").then(|| family.clone()),
            macro_name: (ast_kind == "macro_call").then(|| no_panic_macro_name(&rule.family)),
            normalized_snippet_hash: Some(snippet_hash),
            glob: Some(path),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn entry_from_no_panic_allow_entry(rule: &LegacyNoPanicAllowEntry) -> AllowEntry {
    let path = normalize_path(&rule.path);
    let family = cargo_allow_panic_family(&rule.family);
    let ast_kind = normalize_selector_kind(&rule.selector_kind);
    AllowEntry {
        id: rule.id.clone(),
        kind: FindingKind::Panic,
        family: Some(family.clone()),
        path: Some(PathBuf::from(&path)),
        glob: None,
        owner: rule.owner.clone(),
        classification: rule.classification.clone(),
        reason: rule.reason.clone(),
        evidence: vec![
            "legacy_policy:no-panic-allowlist".to_string(),
            format!("legacy_index:{}", rule.index),
        ],
        links: vec!["legacy-policy:no-panic-allowlist".to_string()],
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: rule.created.clone(),
            review_after: rule.review_after.clone(),
            expires: rule.expires.clone(),
        },
        selector: Selector {
            ast_kind: Some(ast_kind.clone()),
            container: rule.selector_container.clone(),
            callee: (ast_kind == "method_call")
                .then(|| no_panic_method_callee(&family, rule.selector_callee.as_deref())),
            macro_name: (ast_kind == "macro_call")
                .then(|| no_panic_macro_name(rule.selector_callee.as_deref().unwrap_or(&family))),
            line_hint: rule.line_hint,
            glob: Some(path),
            ..Selector::default()
        },
        last_seen: rule.last_seen.clone(),
    }
}

fn entry_from_clippy_rule(rule: &LegacyClippyRule) -> AllowEntry {
    let path = normalize_path(&rule.path);
    AllowEntry {
        id: rule.id.clone(),
        kind: FindingKind::LintException,
        family: Some(rule.family.clone()),
        path: Some(PathBuf::from(&path)),
        glob: None,
        owner: rule.owner.clone(),
        classification: rule.classification.clone(),
        reason: rule.reason.clone(),
        evidence: vec![format!("lint:{}", rule.lint)],
        links: vec![format!("legacy-policy:{}", rule.id)],
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: rule.created.clone(),
            review_after: rule.review_after.clone(),
            expires: rule.expires.clone(),
        },
        selector: Selector {
            ast_kind: Some("attribute".to_string()),
            lint: Some(rule.lint.clone()),
            symbol: rule.symbol.clone(),
            target_fingerprint: rule.target_fingerprint.clone(),
            glob: Some(path),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn entry_from_unsafe_rule(rule: &LegacyUnsafeRule) -> AllowEntry {
    let path = normalize_path(&rule.path);
    AllowEntry {
        id: rule.id.clone(),
        kind: FindingKind::Unsafe,
        family: Some(rule.family.clone()),
        path: Some(PathBuf::from(&path)),
        glob: None,
        owner: rule.owner.clone(),
        classification: rule.classification.clone(),
        reason: rule.reason.clone(),
        evidence: unsafe_evidence(rule),
        links: vec![format!("legacy-policy:{}", rule.id)],
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: rule.created.clone(),
            review_after: rule.review_after.clone(),
            expires: rule.expires.clone(),
        },
        selector: Selector {
            ast_kind: Some(rule.selector_kind.clone()),
            container: rule.selector_container.clone(),
            line_hint: rule.line_hint,
            glob: Some(path),
            ..Selector::default()
        },
        last_seen: rule.last_seen.clone(),
    }
}

fn entry_from_executable_rule(rule: &LegacyExecutableRule) -> AllowEntry {
    let path = normalize_path(&rule.path);
    AllowEntry {
        id: rule.id.clone(),
        kind: FindingKind::PolicyException,
        family: Some("executable_file".to_string()),
        path: Some(PathBuf::from(&path)),
        glob: None,
        owner: rule.owner.clone(),
        classification: "executable_file".to_string(),
        reason: rule.reason.clone(),
        evidence: executable_evidence(rule),
        links: vec![format!("legacy-policy:{}", rule.id)],
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: rule.created.clone(),
            review_after: rule.review_after.clone(),
            expires: rule.expires.clone(),
        },
        selector: Selector {
            ast_kind: Some("git_executable_file".to_string()),
            symbol: Some(path.clone()),
            target_fingerprint: Some("git-mode:100755".to_string()),
            glob: Some(path),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn entries_from_workflow_rule(rule: &LegacyWorkflowRule) -> Vec<AllowEntry> {
    let mut entries = Vec::with_capacity(rule.external_actions.len() + 1);
    entries.push(workflow_file_entry(rule));
    entries.extend(
        rule.external_actions
            .iter()
            .map(|action| workflow_action_entry(rule, action)),
    );
    entries
}

fn workflow_file_entry(rule: &LegacyWorkflowRule) -> AllowEntry {
    let path = normalize_path(&rule.path);
    AllowEntry {
        id: format!("workflow-file-{}", slug_id(&path)),
        kind: FindingKind::PolicyException,
        family: Some("github_workflow".to_string()),
        path: Some(PathBuf::from(&path)),
        glob: None,
        owner: rule.owner.clone(),
        classification: "github_workflow".to_string(),
        reason: rule.reason.clone(),
        evidence: workflow_evidence(rule),
        links: vec![format!("legacy-policy:workflow:{path}")],
        occurrence_limit: None,
        lifecycle: lifecycle_from_workflow_rule(rule),
        selector: Selector {
            ast_kind: Some("github_workflow".to_string()),
            symbol: Some(path.clone()),
            glob: Some(path),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn workflow_action_entry(rule: &LegacyWorkflowRule, action: &str) -> AllowEntry {
    let path = normalize_path(&rule.path);
    let symbol = workflow_action_symbol(&path, action);
    AllowEntry {
        id: format!("workflow-action-{}--{}", slug_id(&path), slug_id(action)),
        kind: FindingKind::PolicyException,
        family: Some("workflow_external_action".to_string()),
        path: Some(PathBuf::from(&path)),
        glob: None,
        owner: rule.owner.clone(),
        classification: "workflow_external_action".to_string(),
        reason: rule.reason.clone(),
        evidence: vec![format!("external_action:{action}")],
        links: vec![format!("legacy-policy:workflow:{path}")],
        occurrence_limit: None,
        lifecycle: lifecycle_from_workflow_rule(rule),
        selector: Selector {
            ast_kind: Some("github_action_uses".to_string()),
            symbol: Some(symbol),
            target_fingerprint: Some(format!("action:{action}")),
            glob: Some(path),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn entry_from_dependency_surface_rule(rule: &LegacyDependencySurfaceRule) -> AllowEntry {
    let pattern = normalize_path(&rule.pattern);
    let reason = match &rule.broad_glob_reason {
        Some(scope_reason) if !scope_reason.trim().is_empty() => {
            format!("{} Scope note: {scope_reason}", rule.reason)
        }
        _ => rule.reason.clone(),
    };
    AllowEntry {
        id: rule.id.clone(),
        kind: FindingKind::PolicyException,
        family: Some("dependency_surface".to_string()),
        path: (!rule.is_glob).then(|| PathBuf::from(&pattern)),
        glob: rule.is_glob.then(|| pattern.clone()),
        owner: rule.owner.clone(),
        classification: rule.surface.clone(),
        reason,
        evidence: dependency_surface_evidence(rule),
        links: vec![format!("legacy-policy:{}", rule.id)],
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: rule.created.clone(),
            review_after: rule.review_after.clone(),
            expires: rule.expires.clone(),
        },
        selector: Selector {
            ast_kind: Some("dependency_surface".to_string()),
            symbol: (!rule.is_glob).then(|| pattern.clone()),
            glob: Some(pattern),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn entry_from_process_rule(rule: &LegacyProcessRule) -> AllowEntry {
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

fn entry_from_network_rule(rule: &LegacyNetworkRule) -> AllowEntry {
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

fn generated_evidence(rule: &LegacyGeneratedRule) -> Vec<String> {
    let mut evidence = Vec::new();
    if let Some(generator) = &rule.generator {
        evidence.push(format!("generator:{generator}"));
    }
    if let Some(command) = &rule.regenerate_command {
        evidence.push(format!("cargo:{command}"));
    }
    evidence
}

fn unsafe_evidence(rule: &LegacyUnsafeRule) -> Vec<String> {
    if rule.evidence.is_empty() {
        vec!["TODO: add unsafe-review or boundary-test evidence".to_string()]
    } else {
        rule.evidence.clone()
    }
}

fn executable_evidence(rule: &LegacyExecutableRule) -> Vec<String> {
    rule.interpreter
        .as_ref()
        .map(|interpreter| vec![format!("interpreter:{interpreter}")])
        .unwrap_or_default()
}

fn workflow_evidence(rule: &LegacyWorkflowRule) -> Vec<String> {
    let mut evidence = Vec::new();
    evidence.extend(
        rule.permissions
            .iter()
            .map(|permission| format!("permission:{permission}")),
    );
    evidence.extend(
        rule.secrets_used
            .iter()
            .map(|secret| format!("secret:{secret}")),
    );
    if let Some(lane) = &rule.duplicate_of_lane {
        evidence.push(format!("duplicate_of_lane:{lane}"));
    }
    evidence
}

fn dependency_surface_evidence(rule: &LegacyDependencySurfaceRule) -> Vec<String> {
    let mut evidence = vec![format!("surface:{}", rule.surface)];
    if let Some(count) = rule.dep_count_at_baseline {
        evidence.push(format!("dep_count_at_baseline:{count}"));
    }
    evidence
}

fn process_evidence(rule: &LegacyProcessRule) -> Vec<String> {
    let mut evidence = vec![
        format!("binary:{}", rule.binary),
        format!("argv_shape:{}", rule.argv_shape.join(" ")),
        format!("network_reach:{}", rule.network_reach),
    ];
    evidence.extend(
        rule.called_by
            .iter()
            .map(|path| format!("called_by:{}", normalize_path(Path::new(path)))),
    );
    evidence
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

fn cargo_allow_panic_family(family: &str) -> String {
    if family == "panic" {
        "panic_macro".to_string()
    } else {
        family.to_string()
    }
}

fn normalize_selector_kind(kind: &str) -> String {
    kind.replace('-', "_")
}

fn no_panic_macro_name(family: &str) -> String {
    if family == "panic" {
        "panic".to_string()
    } else {
        family.to_string()
    }
}

fn no_panic_method_callee(family: &str, selector_callee: Option<&str>) -> String {
    match selector_callee.map(str::trim) {
        Some(callee) if callee.ends_with("unwrap") || callee.contains("::unwrap") => {
            "unwrap".to_string()
        }
        Some(callee) if callee.ends_with("expect") || callee.contains("::expect") => {
            "expect".to_string()
        }
        Some(callee) if !callee.is_empty() => callee.to_string(),
        _ => family.to_string(),
    }
}

fn normalize_unsafe_family(kind: &str) -> String {
    match kind.trim() {
        "unsafe-block" | "unsafe block" => "unsafe_block".to_string(),
        "unsafe-fn" | "unsafe function" | "unsafe_fn" => "unsafe_fn".to_string(),
        "unsafe-impl" | "unsafe impl" => "unsafe_impl".to_string(),
        "unsafe-trait" | "unsafe trait" => "unsafe_trait".to_string(),
        "unsafe-extern" | "unsafe extern" | "unsafe-extern-block" | "unsafe extern block" => {
            "unsafe_extern_block".to_string()
        }
        "unsafe-attr" | "unsafe attribute" | "unsafe-attribute" => "unsafe_attr".to_string(),
        other => other.replace('-', "_"),
    }
}

fn process_scope(rule: &LegacyProcessRule) -> String {
    rule.called_by
        .first()
        .map(|path| normalize_path(Path::new(path)))
        .unwrap_or_else(|| "policy/process-allowlist.toml".to_string())
}

fn process_symbol(rule: &LegacyProcessRule) -> String {
    let args = rule.argv_shape.join(" ");
    if args.is_empty() {
        rule.binary.clone()
    } else {
        format!("{} {args}", rule.binary)
    }
}

fn process_fingerprint(rule: &LegacyProcessRule) -> String {
    format!("process:{}", process_symbol(rule))
}

fn network_symbol(rule: &LegacyNetworkRule) -> String {
    format!("{} lane {}", rule.destination, rule.lane)
}

fn network_fingerprint(rule: &LegacyNetworkRule) -> String {
    format!(
        "network:{}:auth:{}:lane:{}",
        rule.destination, rule.auth_required, rule.lane
    )
}

fn lifecycle_from_rule(rule: &LegacyNonRustRule) -> Lifecycle {
    Lifecycle {
        created: rule.created.clone(),
        review_after: rule.review_after.clone(),
        expires: rule.expires.clone(),
    }
}

fn lifecycle_from_workflow_rule(rule: &LegacyWorkflowRule) -> Lifecycle {
    Lifecycle {
        created: rule.created.clone(),
        review_after: rule.review_after.clone(),
        expires: rule.expires.clone(),
    }
}

fn has_glob_meta(input: &str) -> bool {
    input
        .chars()
        .any(|ch| matches!(ch, '*' | '?' | '[' | ']' | '{' | '}' | ','))
}

fn normalize_legacy_expires(expires: Option<String>) -> Option<String> {
    expires.map(|value| {
        if value == "permanent" {
            "never".to_string()
        } else {
            value
        }
    })
}

fn is_clippy_exceptions_policy(table: &toml::Table) -> bool {
    matches!(
        table.get("policy").and_then(Value::as_str),
        Some("clippy-exceptions" | "clippy-exception-allowlist" | "clippy-allowlist")
    )
}

fn normalize_lint_attribute_family(family: &str) -> String {
    match family.trim() {
        "allow" | "allow-attribute" | "allow_attribute" => "allow_attribute".to_string(),
        "expect" | "expect-attribute" | "expect_attribute" => "expect_attribute".to_string(),
        other => other.to_string(),
    }
}

fn slug_id(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{Span, StructuralIdentity};
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn assert_current_baseline_window(lifecycle: &Lifecycle) {
        let created = lifecycle
            .created
            .as_deref()
            .and_then(SimpleDate::parse)
            .unwrap_or_else(|| std::panic::panic_any("baseline should have valid created date"));
        let expires = lifecycle
            .expires
            .as_deref()
            .and_then(SimpleDate::parse)
            .unwrap_or_else(|| std::panic::panic_any("baseline should have valid expires date"));
        let today = SimpleDate::today_utc_approx();

        assert!(
            today.add_days(-1) <= created && created <= today.add_days(1),
            "baseline created date should track the current UTC day"
        );
        assert_eq!(created.days_until(expires), BASELINE_DEBT_DEFAULT_DAYS);
    }

    #[test]
    fn migrates_non_rust_allowlist_to_canonical_policy() {
        let policy = policy_fixture_path();
        let cfg = load_legacy_or_canonical(&policy)
            .unwrap_or_else(|err| std::panic::panic_any(format!("legacy policy migrates: {err}")));

        assert_eq!(cfg.policy, "cargo-allow");
        assert_eq!(cfg.allow.len(), 4);
        let docs = cfg
            .allow
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected docs allow entry"));
        assert_eq!(docs.id, "non-rust-docs");
        assert_eq!(docs.glob.as_deref(), Some("docs/**"));
        assert_eq!(docs.lifecycle.expires.as_deref(), Some("never"));
        assert!(docs.reason.contains("Scope note:"));
        let ripr = cfg
            .allow
            .get(3)
            .unwrap_or_else(|| std::panic::panic_any("expected ripr allow entry"));
        assert_eq!(ripr.path.as_deref(), Some(Path::new("ripr.toml")));
        assert_eq!(ripr.selector.glob.as_deref(), Some("ripr.toml"));
    }

    #[test]
    fn compat_config_expands_matching_findings_to_exact_entries() {
        let findings = vec![
            finding(".github/workflows/ci.yml", "tracked_file"),
            finding("unmatched/tool.py", "tracked_file"),
        ];

        let policy = policy_fixture_path();
        let cfg = load_non_rust_compat_config(&policy, &findings).unwrap_or_else(|err| {
            std::panic::panic_any(format!("legacy compat config loads: {err}"))
        });

        assert_eq!(cfg.allow.len(), 1);
        let entry = cfg
            .allow
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected one compat allow entry"));
        assert_eq!(
            entry.path.as_deref(),
            Some(Path::new(".github/workflows/ci.yml"))
        );
        assert_eq!(entry.owner, "release/ci");
        assert_eq!(entry.classification, "ci_declarative");
        assert_eq!(
            entry.selector.glob.as_deref(),
            Some(".github/workflows/ci.yml")
        );
        assert_eq!(entry.links, vec!["legacy-policy:non-rust-github-workflows"]);
    }

    #[test]
    fn compat_prefers_more_specific_rule_when_legacy_globs_overlap() {
        let findings = vec![finding(".github/workflows/ci.yml", "tracked_file")];

        let policy = policy_fixture_path();
        let cfg = load_non_rust_compat_config(&policy, &findings).unwrap_or_else(|err| {
            std::panic::panic_any(format!("legacy compat config loads: {err}"))
        });

        let entry = cfg
            .allow
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected one compat allow entry"));
        assert_eq!(entry.owner, "release/ci");
        assert_eq!(entry.classification, "ci_declarative");
    }

    #[test]
    fn non_rust_migration_rejects_broad_glob_without_reason() {
        let policy = non_rust_policy_with_entry(
            r#"id = "non-rust-docs"
glob = "docs/**"
category = "documentation"
owner = "docs"
reason = "Repository policy prose."
created = "2026-05-09"
expires = "permanent"
"#,
        );

        let err = load_legacy_or_canonical(&policy)
            .expect_err("broad non-rust glob without reason should fail");

        assert!(err.to_string().contains("requires broad_glob_reason"));
    }

    #[test]
    fn non_rust_migration_rejects_empty_broad_glob_reason() {
        let policy = non_rust_policy_with_entry(
            r#"id = "non-rust-docs"
glob = "docs/**"
category = "documentation"
owner = "docs"
reason = "Repository policy prose."
broad_glob_reason = "   "
created = "2026-05-09"
expires = "permanent"
"#,
        );

        let err = load_legacy_or_canonical(&policy)
            .expect_err("empty broad non-rust glob reason should fail");

        assert!(err.to_string().contains("empty broad_glob_reason"));
    }

    #[test]
    fn migrates_generated_allowlist_to_canonical_policy() {
        let policy = generated_policy_fixture_path();
        let cfg = load_legacy_or_canonical(&policy).unwrap_or_else(|err| {
            std::panic::panic_any(format!("generated policy migrates: {err}"))
        });

        assert_eq!(cfg.policy, "cargo-allow");
        assert_eq!(cfg.allow.len(), 1);
        let entry = cfg
            .allow
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected generated allow entry"));
        assert_eq!(entry.kind, FindingKind::GeneratedCode);
        assert_eq!(entry.family.as_deref(), Some("generated_code"));
        assert_eq!(
            entry.path.as_deref(),
            Some(Path::new("policy/no-panic-baseline.toml"))
        );
        assert_eq!(entry.lifecycle.expires.as_deref(), Some("never"));
        assert!(entry.evidence.iter().any(|item| item.starts_with("cargo:")));
    }

    #[test]
    fn migrates_no_panic_baseline_to_count_limited_baseline_debt() {
        let policy = no_panic_baseline_fixture_path();
        let cfg = load_legacy_or_canonical(&policy).unwrap_or_else(|err| {
            std::panic::panic_any(format!("no-panic baseline migrates: {err}"))
        });

        assert_eq!(cfg.policy, "cargo-allow");
        assert_eq!(cfg.allow.len(), 2);
        let unwrap = cfg
            .allow
            .iter()
            .find(|entry| entry.family.as_deref() == Some("unwrap"))
            .unwrap_or_else(|| std::panic::panic_any("expected unwrap baseline entry"));
        assert_eq!(unwrap.kind, FindingKind::Panic);
        assert_eq!(unwrap.classification, "baseline_debt");
        assert_eq!(unwrap.owner, "unowned");
        assert_eq!(unwrap.occurrence_limit, Some(2));
        assert_current_baseline_window(&unwrap.lifecycle);
        assert_eq!(unwrap.selector.ast_kind.as_deref(), Some("method_call"));
        assert_eq!(unwrap.selector.callee.as_deref(), Some("unwrap"));
        assert!(unwrap.selector.normalized_snippet_hash.is_some());
        assert!(
            unwrap
                .evidence
                .iter()
                .any(|item| item == "baseline_count:2")
        );

        let panic = cfg
            .allow
            .iter()
            .find(|entry| entry.family.as_deref() == Some("panic_macro"))
            .unwrap_or_else(|| std::panic::panic_any("expected panic macro baseline entry"));
        assert_eq!(panic.selector.ast_kind.as_deref(), Some("macro_call"));
        assert_eq!(panic.selector.macro_name.as_deref(), Some("panic"));
        assert_eq!(panic.occurrence_limit, Some(1));
    }

    #[test]
    fn no_panic_compat_loader_requires_no_panic_policy() {
        let policy = generated_policy_fixture_path();

        let err = load_no_panic_baseline_compat_config(&policy)
            .expect_err("generated policy should not load as no-panic compat");

        assert!(err.to_string().contains("not a no-panic-baseline policy"));
    }

    #[test]
    fn no_panic_baseline_occurrence_limit_prevents_unbounded_matches() {
        let policy = no_panic_baseline_fixture_path();
        let cfg = load_legacy_or_canonical(&policy).unwrap_or_else(|err| {
            std::panic::panic_any(format!("no-panic baseline migrates: {err}"))
        });
        let snippet = ["let value = maybe.", "unwrap();"].concat();
        let finding = panic_finding(
            "src/lib.rs",
            "unwrap",
            "method_call",
            Some("unwrap"),
            None,
            &snippet,
        );

        let outcomes = allow_match::evaluate(
            &cfg,
            &[finding.clone(), finding.clone(), finding],
            allow_match::CheckMode::NoNew,
        );

        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| outcome.status == allow_core::MatchStatus::Matched)
                .count(),
            2
        );
        assert!(
            outcomes
                .iter()
                .any(|outcome| outcome.status == allow_core::MatchStatus::New
                    && outcome.message.contains("occurrence_limit exceeded"))
        );
    }

    #[test]
    fn migrates_no_panic_allowlist_to_structural_panic_entries() {
        let policy = no_panic_allowlist_fixture_path();
        let cfg = load_legacy_or_canonical(&policy).unwrap_or_else(|err| {
            std::panic::panic_any(format!("no-panic allowlist migrates: {err}"))
        });

        assert_eq!(cfg.policy, "cargo-allow");
        assert_eq!(cfg.allow.len(), 2);
        let unwrap = cfg
            .allow
            .iter()
            .find(|entry| entry.id == "no-panic-unwrap")
            .unwrap_or_else(|| std::panic::panic_any("expected unwrap allow entry"));
        assert_eq!(unwrap.kind, FindingKind::Panic);
        assert_eq!(unwrap.family.as_deref(), Some("unwrap"));
        assert_eq!(unwrap.reason, "Parser validates the optional value.");
        assert_eq!(unwrap.selector.ast_kind.as_deref(), Some("method_call"));
        assert_eq!(unwrap.selector.callee.as_deref(), Some("unwrap"));
        assert_eq!(unwrap.selector.container.as_deref(), Some("load"));
        assert_eq!(unwrap.selector.line_hint, Some(7));
        assert_eq!(
            unwrap
                .last_seen
                .as_ref()
                .map(|seen| (seen.line, seen.column)),
            Some((7, 12))
        );
        assert_eq!(unwrap.lifecycle.review_after.as_deref(), Some("2026-09-09"));

        let generated = cfg
            .allow
            .iter()
            .find(|entry| entry.id.starts_with("legacy-no-panic-"))
            .unwrap_or_else(|| std::panic::panic_any("expected generated no-panic entry"));
        assert_eq!(generated.classification, "baseline_debt");
        assert_eq!(generated.owner, "unowned");
        assert_eq!(generated.selector.macro_name.as_deref(), Some("panic"));
        assert_current_baseline_window(&generated.lifecycle);
    }

    #[test]
    fn no_panic_allowlist_compat_preserves_matched_new_and_stale_drift() {
        let policy = no_panic_allowlist_fixture_path();
        let cfg = load_no_panic_allowlist_compat_config(&policy).unwrap_or_else(|err| {
            std::panic::panic_any(format!("no-panic allowlist compat config loads: {err}"))
        });

        let mut finding = panic_finding(
            "src/lib.rs",
            "unwrap",
            "method_call",
            Some("unwrap"),
            None,
            "let value = maybe.unwrap();",
        );
        finding.identity.container = Some("load".to_string());
        let matched = allow_match::evaluate(&cfg, &[finding], allow_match::CheckMode::NoNew);
        assert!(
            matched
                .iter()
                .any(|outcome| outcome.status == allow_core::MatchStatus::Matched)
        );

        let missing_allow = allow_match::evaluate(
            &cfg,
            &[panic_finding(
                "src/lib.rs",
                "expect",
                "method_call",
                Some("expect"),
                None,
                "let value = maybe.expect(\"value\");",
            )],
            allow_match::CheckMode::NoNew,
        );
        assert!(
            missing_allow
                .iter()
                .any(|outcome| outcome.status == allow_core::MatchStatus::New)
        );

        let stale_allow = allow_match::evaluate(&cfg, &[], allow_match::CheckMode::Audit);
        assert!(
            stale_allow
                .iter()
                .any(|outcome| outcome.status == allow_core::MatchStatus::Stale)
        );
    }

    #[test]
    fn no_panic_allowlist_loader_requires_allowlist_policy() {
        let policy = no_panic_baseline_fixture_path();

        let err = load_no_panic_allowlist_compat_config(&policy)
            .expect_err("baseline policy should not load as no-panic allowlist compat");

        assert!(err.to_string().contains("not a no-panic-allowlist policy"));
    }

    #[test]
    fn migrates_clippy_exceptions_to_lint_policy_entries() {
        let policy = clippy_policy_fixture_path();
        let cfg = load_legacy_or_canonical(&policy).unwrap_or_else(|err| {
            std::panic::panic_any(format!("clippy exceptions migrate: {err}"))
        });

        assert_eq!(cfg.policy, "cargo-allow");
        assert_eq!(cfg.allow.len(), 1);
        let entry = cfg
            .allow
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected clippy exception entry"));
        assert_eq!(entry.id, "clippy-unwrap-policy");
        assert_eq!(entry.kind, FindingKind::LintException);
        assert_eq!(entry.family.as_deref(), Some("expect_attribute"));
        assert_eq!(entry.path.as_deref(), Some(Path::new("src/lib.rs")));
        assert_eq!(entry.owner, "lint");
        assert_eq!(entry.classification, "reviewed_lint_exception");
        assert_eq!(entry.selector.ast_kind.as_deref(), Some("attribute"));
        assert_eq!(entry.selector.lint.as_deref(), Some("clippy::unwrap_used"));
        assert_eq!(
            entry.selector.target_fingerprint.as_deref(),
            Some("policy:clippy-unwrap-policy")
        );
        assert_eq!(entry.lifecycle.review_after.as_deref(), Some("2026-09-09"));
    }

    #[test]
    fn clippy_compat_preserves_matched_new_and_stale_drift() {
        let policy = clippy_policy_fixture_path();
        let cfg = load_clippy_exceptions_compat_config(&policy).unwrap_or_else(|err| {
            std::panic::panic_any(format!("clippy compat config loads: {err}"))
        });

        let matched = allow_match::evaluate(
            &cfg,
            &[lint_finding(
                "src/lib.rs",
                "expect_attribute",
                "clippy::unwrap_used",
                Some("clippy-unwrap-policy"),
            )],
            allow_match::CheckMode::NoNew,
        );
        assert!(
            matched
                .iter()
                .any(|outcome| outcome.status == allow_core::MatchStatus::Matched)
        );

        let missing_allow = allow_match::evaluate(
            &cfg,
            &[lint_finding(
                "src/lib.rs",
                "expect_attribute",
                "clippy::panic",
                None,
            )],
            allow_match::CheckMode::NoNew,
        );
        assert!(
            missing_allow
                .iter()
                .any(|outcome| outcome.status == allow_core::MatchStatus::New)
        );

        let stale_allow = allow_match::evaluate(&cfg, &[], allow_match::CheckMode::Audit);
        assert!(
            stale_allow
                .iter()
                .any(|outcome| outcome.status == allow_core::MatchStatus::Stale)
        );
    }

    #[test]
    fn clippy_compat_accepts_minimal_legacy_entries_as_baseline_debt() {
        let path = fixture_dir().join("clippy-exceptions.toml");
        fs::write(
            &path,
            r#"schema_version = 1
policy = "clippy-exceptions"

[[allow]]
path = "src/lib.rs"
lint = "clippy::unwrap_used"
"#,
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));

        let cfg = load_clippy_exceptions_compat_config(&path).unwrap_or_else(|err| {
            std::panic::panic_any(format!("minimal clippy compat config loads: {err}"))
        });

        let entry = cfg
            .allow
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected clippy exception entry"));
        assert_eq!(entry.owner, "unowned");
        assert_eq!(entry.classification, "baseline_debt");
        assert!(entry.reason.contains("requires human review"));
        assert_current_baseline_window(&entry.lifecycle);
    }

    #[test]
    fn clippy_compat_loader_requires_clippy_policy() {
        let policy = generated_policy_fixture_path();

        let err = load_clippy_exceptions_compat_config(&policy)
            .expect_err("generated policy should not load as clippy compat");

        assert!(err.to_string().contains("not a clippy-exceptions policy"));
    }

    #[test]
    fn migrates_unsafe_allowlist_to_structural_unsafe_entries() {
        let policy = unsafe_policy_fixture_path();
        let cfg = load_legacy_or_canonical(&policy).unwrap_or_else(|err| {
            std::panic::panic_any(format!("unsafe allowlist migrates: {err}"))
        });

        assert_eq!(cfg.policy, "cargo-allow");
        assert_eq!(cfg.allow.len(), 2);
        let reviewed = cfg
            .allow
            .iter()
            .find(|entry| entry.id == "unsafe-read")
            .unwrap_or_else(|| std::panic::panic_any("expected reviewed unsafe entry"));
        assert_eq!(reviewed.kind, FindingKind::Unsafe);
        assert_eq!(reviewed.family.as_deref(), Some("unsafe_block"));
        assert_eq!(reviewed.selector.ast_kind.as_deref(), Some("unsafe_block"));
        assert_eq!(reviewed.selector.container.as_deref(), Some("read"));
        assert_eq!(reviewed.selector.line_hint, Some(7));
        assert_eq!(
            reviewed
                .last_seen
                .as_ref()
                .map(|seen| (seen.line, seen.column)),
            Some((7, 12))
        );
        assert!(
            reviewed
                .evidence
                .iter()
                .any(|item| item == "unsafe-review:docs/evidence/unsafe/read.json")
        );

        let generated = cfg
            .allow
            .iter()
            .find(|entry| entry.id.starts_with("legacy-unsafe-"))
            .unwrap_or_else(|| std::panic::panic_any("expected generated unsafe entry"));
        assert_eq!(generated.family.as_deref(), Some("unsafe_fn"));
        assert_eq!(generated.classification, "baseline_debt");
        assert_eq!(generated.owner, "unowned");
        assert!(
            generated
                .evidence
                .iter()
                .any(|item| item.contains("TODO: add unsafe-review"))
        );
        assert_current_baseline_window(&generated.lifecycle);
    }

    #[test]
    fn unsafe_allowlist_compat_preserves_matched_new_and_stale_drift() {
        let policy = unsafe_policy_fixture_path();
        let cfg = load_unsafe_allowlist_compat_config(&policy).unwrap_or_else(|err| {
            std::panic::panic_any(format!("unsafe allowlist compat config loads: {err}"))
        });

        let matched = allow_match::evaluate(
            &cfg,
            &[unsafe_finding("src/lib.rs", "unsafe_block", Some("read"))],
            allow_match::CheckMode::NoNew,
        );
        assert!(
            matched
                .iter()
                .any(|outcome| outcome.status == allow_core::MatchStatus::Matched)
        );

        let missing_allow = allow_match::evaluate(
            &cfg,
            &[unsafe_finding("src/lib.rs", "unsafe_impl", None)],
            allow_match::CheckMode::NoNew,
        );
        assert!(
            missing_allow
                .iter()
                .any(|outcome| outcome.status == allow_core::MatchStatus::New)
        );

        let stale_allow = allow_match::evaluate(&cfg, &[], allow_match::CheckMode::Audit);
        assert!(
            stale_allow
                .iter()
                .any(|outcome| outcome.status == allow_core::MatchStatus::Stale)
        );
    }

    #[test]
    fn unsafe_allowlist_loader_requires_unsafe_policy() {
        let policy = generated_policy_fixture_path();

        let err = load_unsafe_allowlist_compat_config(&policy)
            .expect_err("generated policy should not load as unsafe compat");

        assert!(err.to_string().contains("not an unsafe-allowlist policy"));
    }

    #[test]
    fn generated_findings_read_linguist_generated_paths() {
        let root = generated_fixture_root();

        let findings = generated_findings_from_gitattributes(&root)
            .unwrap_or_else(|err| std::panic::panic_any(format!("generated findings load: {err}")));

        assert_eq!(findings.len(), 1);
        let finding = findings
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected generated finding"));
        assert_eq!(finding.kind, FindingKind::GeneratedCode);
        assert_eq!(finding.path, PathBuf::from("policy/no-panic-baseline.toml"));
    }

    #[test]
    fn generated_compat_preserves_missing_and_stale_drift() {
        let policy = generated_policy_fixture_path();
        let cfg = load_generated_compat_config(&policy).unwrap_or_else(|err| {
            std::panic::panic_any(format!("generated compat config loads: {err}"))
        });

        let matched = allow_match::evaluate(
            &cfg,
            &[generated_finding(PathBuf::from(
                "policy/no-panic-baseline.toml",
            ))],
            allow_match::CheckMode::NoNew,
        );
        assert!(matched.iter().any(|outcome| {
            outcome.status == allow_core::MatchStatus::Matched
                && outcome.allow_id.as_deref() == Some("generated-no-panic-baseline")
        }));

        let missing_allow = allow_match::evaluate(
            &cfg,
            &[generated_finding(PathBuf::from(
                "policy/extra-baseline.toml",
            ))],
            allow_match::CheckMode::NoNew,
        );
        assert!(
            missing_allow
                .iter()
                .any(|outcome| outcome.status == allow_core::MatchStatus::New)
        );

        let stale_allow = allow_match::evaluate(&cfg, &[], allow_match::CheckMode::Audit);
        assert!(
            stale_allow
                .iter()
                .any(|outcome| outcome.status == allow_core::MatchStatus::Stale)
        );
    }

    #[test]
    fn migrates_executable_allowlist_to_policy_exception_entries() {
        let policy = executable_policy_fixture_path();
        let cfg = load_legacy_or_canonical(&policy).unwrap_or_else(|err| {
            std::panic::panic_any(format!("executable policy migrates: {err}"))
        });

        assert_eq!(cfg.policy, "cargo-allow");
        assert_eq!(cfg.allow.len(), 1);
        let entry = cfg
            .allow
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected executable allow entry"));
        assert_eq!(entry.kind, FindingKind::PolicyException);
        assert_eq!(entry.family.as_deref(), Some("executable_file"));
        assert_eq!(entry.classification, "executable_file");
        assert_eq!(
            entry.path.as_deref(),
            Some(Path::new("scripts/package-proof.sh"))
        );
        assert_eq!(entry.lifecycle.expires.as_deref(), Some("never"));
        assert_eq!(entry.evidence, vec!["interpreter:bash"]);
        assert_eq!(
            entry.selector.target_fingerprint.as_deref(),
            Some("git-mode:100755")
        );
    }

    #[test]
    fn executable_findings_read_git_stage_executable_paths() {
        let stage = "\
100644 abc 0\tREADME.md\n\
100755 def 0\tscripts/package-proof.sh\n\
120000 ghi 0\tscripts/link.sh\n";

        let findings = executable_findings_from_git_stage(stage);

        assert_eq!(findings.len(), 1);
        let finding = findings
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected executable finding"));
        assert_eq!(finding.kind, FindingKind::PolicyException);
        assert_eq!(finding.family.as_deref(), Some("executable_file"));
        assert_eq!(finding.path, PathBuf::from("scripts/package-proof.sh"));
        assert_eq!(finding.identity.ast_kind, "git_executable_file");
        assert_eq!(
            finding.identity.target_fingerprint.as_deref(),
            Some("git-mode:100755")
        );
    }

    #[test]
    fn executable_compat_preserves_missing_and_stale_drift() {
        let policy = executable_policy_fixture_path();
        let cfg = load_executable_compat_config(&policy).unwrap_or_else(|err| {
            std::panic::panic_any(format!("executable compat config loads: {err}"))
        });

        let matched = allow_match::evaluate(
            &cfg,
            &[executable_finding(PathBuf::from(
                "scripts/package-proof.sh",
            ))],
            allow_match::CheckMode::NoNew,
        );
        assert!(matched.iter().any(|outcome| {
            outcome.status == allow_core::MatchStatus::Matched
                && outcome.allow_id.as_deref() == Some("exec-package-proof")
        }));

        let missing_allow = allow_match::evaluate(
            &cfg,
            &[executable_finding(PathBuf::from("scripts/new-tool.sh"))],
            allow_match::CheckMode::NoNew,
        );
        assert!(
            missing_allow
                .iter()
                .any(|outcome| outcome.status == allow_core::MatchStatus::New)
        );

        let stale_allow = allow_match::evaluate(&cfg, &[], allow_match::CheckMode::Audit);
        assert!(
            stale_allow
                .iter()
                .any(|outcome| outcome.status == allow_core::MatchStatus::Stale)
        );
    }

    #[test]
    fn migrates_workflow_allowlist_to_policy_exception_entries() {
        let policy = workflow_policy_fixture_path();
        let cfg = load_legacy_or_canonical(&policy).unwrap_or_else(|err| {
            std::panic::panic_any(format!("workflow policy migrates: {err}"))
        });

        assert_eq!(cfg.policy, "cargo-allow");
        assert_eq!(cfg.allow.len(), 3);
        assert!(cfg.allow.iter().any(|entry| {
            entry.kind == FindingKind::PolicyException
                && entry.family.as_deref() == Some("github_workflow")
                && entry.path.as_deref() == Some(Path::new(".github/workflows/ci.yml"))
        }));
        let action = cfg
            .allow
            .iter()
            .find(|entry| {
                entry.family.as_deref() == Some("workflow_external_action")
                    && entry
                        .selector
                        .target_fingerprint
                        .as_deref()
                        .is_some_and(|target| target == "action:actions/checkout@v6.0.2")
            })
            .unwrap_or_else(|| std::panic::panic_any("expected checkout action entry"));
        assert_eq!(action.classification, "workflow_external_action");
        assert_eq!(action.lifecycle.expires.as_deref(), Some("never"));
    }

    #[test]
    fn workflow_findings_read_workflow_files_and_uses_lines() {
        let root = workflow_fixture_root();

        let findings = workflow_findings_from_files(&root)
            .unwrap_or_else(|err| std::panic::panic_any(format!("workflow findings load: {err}")));

        assert!(findings.iter().any(|finding| {
            finding.family.as_deref() == Some("github_workflow")
                && finding.path == Path::new(".github/workflows/ci.yml")
        }));
        assert!(findings.iter().any(|finding| {
            finding.family.as_deref() == Some("workflow_external_action")
                && finding.identity.target_fingerprint.as_deref()
                    == Some("action:actions/checkout@v6.0.2")
        }));
        assert!(!findings.iter().any(|finding| {
            finding.identity.target_fingerprint.as_deref() == Some("action:ignored/comment@v1")
        }));
    }

    #[test]
    fn workflow_compat_preserves_missing_and_stale_drift() {
        let policy = workflow_policy_fixture_path();
        let cfg = load_workflow_compat_config(&policy).unwrap_or_else(|err| {
            std::panic::panic_any(format!("workflow compat config loads: {err}"))
        });

        let matched = allow_match::evaluate(
            &cfg,
            &[
                workflow_file_finding(PathBuf::from(".github/workflows/ci.yml")),
                workflow_action_finding(
                    PathBuf::from(".github/workflows/ci.yml"),
                    "actions/checkout@v6.0.2".to_string(),
                ),
                workflow_action_finding(
                    PathBuf::from(".github/workflows/ci.yml"),
                    "Swatinem/rust-cache@v2".to_string(),
                ),
            ],
            allow_match::CheckMode::NoNew,
        );
        assert_eq!(
            matched
                .iter()
                .filter(|outcome| outcome.status == allow_core::MatchStatus::Matched)
                .count(),
            3
        );

        let missing_allow = allow_match::evaluate(
            &cfg,
            &[
                workflow_file_finding(PathBuf::from(".github/workflows/ci.yml")),
                workflow_action_finding(
                    PathBuf::from(".github/workflows/ci.yml"),
                    "actions/setup-node@v5".to_string(),
                ),
            ],
            allow_match::CheckMode::NoNew,
        );
        assert!(
            missing_allow
                .iter()
                .any(|outcome| outcome.status == allow_core::MatchStatus::New)
        );

        let stale_allow = allow_match::evaluate(&cfg, &[], allow_match::CheckMode::Audit);
        assert!(
            stale_allow
                .iter()
                .any(|outcome| outcome.status == allow_core::MatchStatus::Stale)
        );
    }

    #[test]
    fn migrates_dependency_surface_allowlist_to_policy_exception_entries() {
        let policy = dependency_policy_fixture_path();
        let cfg = load_legacy_or_canonical(&policy).unwrap_or_else(|err| {
            std::panic::panic_any(format!("dependency policy migrates: {err}"))
        });

        assert_eq!(cfg.policy, "cargo-allow");
        assert_eq!(cfg.allow.len(), 2);
        let workspace = cfg
            .allow
            .iter()
            .find(|entry| entry.id == "dep-workspace-cargo-toml")
            .unwrap_or_else(|| std::panic::panic_any("expected workspace manifest entry"));
        assert_eq!(workspace.kind, FindingKind::PolicyException);
        assert_eq!(workspace.family.as_deref(), Some("dependency_surface"));
        assert_eq!(workspace.classification, "workspace_manifest");
        assert_eq!(workspace.path.as_deref(), Some(Path::new("Cargo.toml")));
        assert_eq!(workspace.lifecycle.expires.as_deref(), Some("never"));
        assert!(
            workspace
                .evidence
                .iter()
                .any(|item| item == "dep_count_at_baseline:22")
        );

        let crates = cfg
            .allow
            .iter()
            .find(|entry| entry.id == "dep-crate-cargo-toml")
            .unwrap_or_else(|| std::panic::panic_any("expected crate glob entry"));
        assert_eq!(crates.glob.as_deref(), Some("crates/*/Cargo.toml"));
        assert!(crates.reason.contains("Scope note:"));
    }

    #[test]
    fn dependency_surface_compat_preserves_matched_new_and_stale_drift() {
        let policy = dependency_policy_fixture_path();
        let cfg = load_dependency_surface_compat_config(&policy).unwrap_or_else(|err| {
            std::panic::panic_any(format!("dependency compat config loads: {err}"))
        });

        let matched = allow_match::evaluate(
            &cfg,
            &[
                dependency_surface_finding(PathBuf::from("Cargo.toml")),
                dependency_surface_finding(PathBuf::from("crates/core/Cargo.toml")),
            ],
            allow_match::CheckMode::NoNew,
        );
        assert_eq!(
            matched
                .iter()
                .filter(|outcome| outcome.status == allow_core::MatchStatus::Matched)
                .count(),
            2
        );

        let missing_allow = allow_match::evaluate(
            &cfg,
            &[dependency_surface_finding(PathBuf::from(
                "xtask/Cargo.toml",
            ))],
            allow_match::CheckMode::NoNew,
        );
        assert!(
            missing_allow
                .iter()
                .any(|outcome| outcome.status == allow_core::MatchStatus::New)
        );

        let stale_allow = allow_match::evaluate(&cfg, &[], allow_match::CheckMode::Audit);
        assert!(
            stale_allow
                .iter()
                .any(|outcome| outcome.status == allow_core::MatchStatus::Stale)
        );
    }

    #[test]
    fn migrates_process_allowlist_to_policy_exception_entries() {
        let policy = process_policy_fixture_path();
        let cfg = load_legacy_or_canonical(&policy)
            .unwrap_or_else(|err| std::panic::panic_any(format!("process policy migrates: {err}")));

        assert_eq!(cfg.policy, "cargo-allow");
        assert_eq!(cfg.allow.len(), 2);
        let install = cfg
            .allow
            .iter()
            .find(|entry| entry.id == "proc-cargo-install-cargo-deny")
            .unwrap_or_else(|| std::panic::panic_any("expected cargo install process entry"));
        assert_eq!(install.kind, FindingKind::PolicyException);
        assert_eq!(install.family.as_deref(), Some("process_spawn"));
        assert_eq!(install.classification, "network_process");
        assert_eq!(
            install.path.as_deref(),
            Some(Path::new(".github/workflows/ci.yml"))
        );
        assert_eq!(install.selector.ast_kind.as_deref(), Some("process_spawn"));
        assert_eq!(
            install.selector.symbol.as_deref(),
            Some("cargo install cargo-deny --locked")
        );
        assert_eq!(
            install.selector.target_fingerprint.as_deref(),
            Some("process:cargo install cargo-deny --locked")
        );
        assert_eq!(
            install.lifecycle.review_after.as_deref(),
            Some("2026-09-09")
        );
        assert!(
            install
                .evidence
                .iter()
                .any(|item| item == "network_reach:true")
        );

        let local = cfg
            .allow
            .iter()
            .find(|entry| entry.id == "proc-bash-package-proof")
            .unwrap_or_else(|| std::panic::panic_any("expected package proof process entry"));
        assert_eq!(local.classification, "local_process");
        assert_eq!(local.lifecycle.expires.as_deref(), Some("never"));
    }

    #[test]
    fn process_compat_synthesizes_matched_new_and_stale_drift() {
        let policy = process_policy_fixture_path();
        let cfg = load_process_compat_config(&policy).unwrap_or_else(|err| {
            std::panic::panic_any(format!("process compat config loads: {err}"))
        });
        let findings = process_findings_from_config(&cfg);

        let matched = allow_match::evaluate(&cfg, &findings, allow_match::CheckMode::NoNew);
        assert_eq!(
            matched
                .iter()
                .filter(|outcome| outcome.status == allow_core::MatchStatus::Matched)
                .count(),
            2
        );

        let missing_allow = allow_match::evaluate(
            &cfg,
            &[process_policy_finding(
                ".github/workflows/release.yml",
                "bash scripts/publish.sh",
            )],
            allow_match::CheckMode::NoNew,
        );
        assert!(
            missing_allow
                .iter()
                .any(|outcome| outcome.status == allow_core::MatchStatus::New)
        );

        let stale_allow = allow_match::evaluate(&cfg, &[], allow_match::CheckMode::Audit);
        assert!(
            stale_allow
                .iter()
                .any(|outcome| outcome.status == allow_core::MatchStatus::Stale)
        );
    }

    #[test]
    fn process_policy_requires_legacy_xtask_fields() {
        let policy = malformed_process_policy_fixture_path();
        let err = load_process_compat_config(&policy)
            .expect_err("process policy without network_reach should fail");
        assert!(
            err.to_string()
                .contains("proc-missing missing network_reach")
        );
    }

    #[test]
    fn migrates_network_allowlist_to_policy_exception_entries() {
        let policy = network_policy_fixture_path();
        let cfg = load_legacy_or_canonical(&policy)
            .unwrap_or_else(|err| std::panic::panic_any(format!("network policy migrates: {err}")));

        assert_eq!(cfg.policy, "cargo-allow");
        assert_eq!(cfg.allow.len(), 2);
        let public = cfg
            .allow
            .iter()
            .find(|entry| entry.id == "net-crates-io-fetch")
            .unwrap_or_else(|| std::panic::panic_any("expected crates.io network entry"));
        assert_eq!(public.kind, FindingKind::PolicyException);
        assert_eq!(public.family.as_deref(), Some("network_destination"));
        assert_eq!(public.classification, "public_network");
        assert_eq!(
            public.path.as_deref(),
            Some(Path::new("policy/network-allowlist.toml"))
        );
        assert_eq!(
            public.selector.ast_kind.as_deref(),
            Some("network_destination")
        );
        assert_eq!(
            public.selector.symbol.as_deref(),
            Some("crates.io lane build")
        );
        assert_eq!(
            public.selector.target_fingerprint.as_deref(),
            Some("network:crates.io:auth:false:lane:build")
        );
        assert_eq!(public.lifecycle.expires.as_deref(), Some("never"));

        let authenticated = cfg
            .allow
            .iter()
            .find(|entry| entry.id == "net-github-api")
            .unwrap_or_else(|| std::panic::panic_any("expected GitHub API network entry"));
        assert_eq!(authenticated.classification, "authenticated_network");
        assert!(
            authenticated
                .evidence
                .iter()
                .any(|item| item == "auth_secret:GITHUB_TOKEN")
        );
    }

    #[test]
    fn network_compat_synthesizes_matched_new_and_stale_drift() {
        let policy = network_policy_fixture_path();
        let cfg = load_network_compat_config(&policy).unwrap_or_else(|err| {
            std::panic::panic_any(format!("network compat config loads: {err}"))
        });
        let findings = network_findings_from_config(&cfg);

        let matched = allow_match::evaluate(&cfg, &findings, allow_match::CheckMode::NoNew);
        assert_eq!(
            matched
                .iter()
                .filter(|outcome| outcome.status == allow_core::MatchStatus::Matched)
                .count(),
            2
        );

        let missing_allow = allow_match::evaluate(
            &cfg,
            &[network_policy_finding("example.com lane test")],
            allow_match::CheckMode::NoNew,
        );
        assert!(
            missing_allow
                .iter()
                .any(|outcome| outcome.status == allow_core::MatchStatus::New)
        );

        let stale_allow = allow_match::evaluate(&cfg, &[], allow_match::CheckMode::Audit);
        assert!(
            stale_allow
                .iter()
                .any(|outcome| outcome.status == allow_core::MatchStatus::Stale)
        );
    }

    #[test]
    fn network_policy_requires_legacy_xtask_fields() {
        let policy = malformed_network_policy_fixture_path();
        let err = load_network_compat_config(&policy)
            .expect_err("network policy without auth_required should fail");
        assert!(
            err.to_string()
                .contains("net-missing missing auth_required")
        );
    }

    #[test]
    fn migrates_legacy_policy_directory_to_one_config() {
        let dir = fixture_dir();
        fs::write(
            dir.join("process-allowlist.toml"),
            process_policy_fixture_text(),
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("process fixture write: {err}")));
        fs::write(
            dir.join("network-allowlist.toml"),
            network_policy_fixture_text(),
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("network fixture write: {err}")));

        let cfg = load_legacy_policy_dir(&dir).unwrap_or_else(|err| {
            std::panic::panic_any(format!("policy directory migrates: {err}"))
        });

        assert_eq!(cfg.policy, "cargo-allow");
        assert_eq!(cfg.owner.as_deref(), Some("EffortlessMetrics"));
        assert_eq!(cfg.allow.len(), 4);
        assert!(
            cfg.allow
                .iter()
                .any(|entry| entry.family.as_deref() == Some("process_spawn"))
        );
        assert!(
            cfg.allow
                .iter()
                .any(|entry| entry.family.as_deref() == Some("network_destination"))
        );
    }

    #[test]
    fn policy_directory_can_expand_non_rust_globs_with_findings() {
        let dir = fixture_dir();
        fs::write(dir.join("non-rust-allowlist.toml"), policy_fixture_text())
            .unwrap_or_else(|err| std::panic::panic_any(format!("non-rust fixture write: {err}")));
        let findings = vec![finding(".github/workflows/ci.yml", "tracked_file")];

        let cfg =
            load_legacy_policy_dir_with_non_rust_findings(&dir, &findings).unwrap_or_else(|err| {
                std::panic::panic_any(format!("policy directory with findings migrates: {err}"))
            });

        assert_eq!(cfg.allow.len(), 1);
        let entry = cfg
            .allow
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected expanded non-rust entry"));
        assert_eq!(entry.id, "non-rust-github-workflows--0001");
        assert_eq!(
            entry.path.as_deref(),
            Some(Path::new(".github/workflows/ci.yml"))
        );
        assert_eq!(entry.links, vec!["legacy-policy:non-rust-github-workflows"]);
    }

    #[test]
    fn legacy_policy_directory_requires_supported_files() {
        let dir = fixture_dir();
        let err =
            load_legacy_policy_dir(&dir).expect_err("empty policy directory should not migrate");
        assert!(
            err.to_string()
                .contains("contains no supported legacy policy files")
        );
    }

    static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

    fn policy_fixture_path() -> PathBuf {
        let path = fixture_dir().join("non-rust-allowlist.toml");
        fs::write(&path, policy_fixture_text())
            .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
        path
    }

    fn non_rust_policy_with_entry(entry: &str) -> PathBuf {
        let path = fixture_dir().join("non-rust-allowlist.toml");
        let text = format!(
            r#"schema_version = 1
policy = "non-rust-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
{entry}
"#
        );
        fs::write(&path, text)
            .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
        path
    }

    fn generated_policy_fixture_path() -> PathBuf {
        let path = fixture_dir().join("generated-allowlist.toml");
        fs::write(&path, generated_policy_fixture_text())
            .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
        path
    }

    fn no_panic_baseline_fixture_path() -> PathBuf {
        let path = fixture_dir().join("no-panic-baseline.toml");
        fs::write(&path, no_panic_baseline_fixture_text())
            .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
        path
    }

    fn no_panic_allowlist_fixture_path() -> PathBuf {
        let path = fixture_dir().join("no-panic-allowlist.toml");
        fs::write(&path, no_panic_allowlist_fixture_text())
            .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
        path
    }

    fn clippy_policy_fixture_path() -> PathBuf {
        let path = fixture_dir().join("clippy-exceptions.toml");
        fs::write(&path, clippy_policy_fixture_text())
            .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
        path
    }

    fn unsafe_policy_fixture_path() -> PathBuf {
        let path = fixture_dir().join("unsafe-allowlist.toml");
        fs::write(&path, unsafe_policy_fixture_text())
            .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
        path
    }

    fn executable_policy_fixture_path() -> PathBuf {
        let path = fixture_dir().join("executable-allowlist.toml");
        fs::write(&path, executable_policy_fixture_text())
            .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
        path
    }

    fn workflow_policy_fixture_path() -> PathBuf {
        let path = fixture_dir().join("workflow-allowlist.toml");
        fs::write(&path, workflow_policy_fixture_text())
            .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
        path
    }

    fn dependency_policy_fixture_path() -> PathBuf {
        let path = fixture_dir().join("dependency-surface-allowlist.toml");
        fs::write(&path, dependency_policy_fixture_text())
            .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
        path
    }

    fn process_policy_fixture_path() -> PathBuf {
        let path = fixture_dir().join("process-allowlist.toml");
        fs::write(&path, process_policy_fixture_text())
            .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
        path
    }

    fn malformed_process_policy_fixture_path() -> PathBuf {
        let path = fixture_dir().join("process-allowlist.toml");
        fs::write(
            &path,
            r#"schema_version = 1
policy = "process-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "proc-missing"
binary = "cargo"
argv_shape = ["install", "cargo-deny", "--locked"]
owner = "release/ci"
reason = "Intentionally incomplete fixture."
created = "2026-05-09"
"#,
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
        path
    }

    fn network_policy_fixture_path() -> PathBuf {
        let path = fixture_dir().join("network-allowlist.toml");
        fs::write(&path, network_policy_fixture_text())
            .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
        path
    }

    fn malformed_network_policy_fixture_path() -> PathBuf {
        let path = fixture_dir().join("network-allowlist.toml");
        fs::write(
            &path,
            r#"schema_version = 1
policy = "network-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "net-missing"
destination = "crates.io"
lane = "build"
owner = "release"
reason = "Intentionally incomplete fixture."
created = "2026-05-09"
"#,
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
        path
    }

    fn generated_fixture_root() -> PathBuf {
        let dir = fixture_dir();
        fs::write(
            dir.join(".gitattributes"),
            "# generated files\npolicy/no-panic-baseline.toml text linguist-generated=true\nREADME.md text\n",
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("gitattributes write: {err}")));
        dir
    }

    fn workflow_fixture_root() -> PathBuf {
        let dir = fixture_dir();
        let workflows = dir.join(".github").join("workflows");
        fs::create_dir_all(&workflows)
            .unwrap_or_else(|err| std::panic::panic_any(format!("workflow dir: {err}")));
        fs::write(
            workflows.join("ci.yml"),
            "name: ci\njobs:\n  test:\n    steps:\n      - uses: actions/checkout@v6.0.2\n      - uses: Swatinem/rust-cache@v2 # cache\n      # - uses: ignored/comment@v1\n",
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("workflow write: {err}")));
        dir
    }

    fn fixture_dir() -> PathBuf {
        let id = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "cargo-allow-policy-legacy-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&dir)
            .unwrap_or_else(|err| std::panic::panic_any(format!("fixture dir: {err}")));
        dir
    }

    fn policy_fixture_text() -> String {
        let mut text = String::from(
            r#"schema_version = 1
policy = "non-rust-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

"#,
        );
        push_allow(
            &mut text,
            r#"id = "non-rust-docs"
glob = "docs/**"
category = "documentation"
owner = "docs"
reason = "Repository policy prose."
broad_glob_reason = "Docs are intentionally tree-shaped."
created = "2026-05-09"
expires = "permanent"

"#,
        );
        push_allow(
            &mut text,
            r#"id = "non-rust-github-meta"
glob = ".github/**"
category = "ci_meta"
owner = "release/meta"
reason = "GitHub metadata."
broad_glob_reason = "Covers ancillary GitHub configuration."
created = "2026-05-09"
expires = "permanent"

"#,
        );
        push_allow(
            &mut text,
            r#"id = "non-rust-github-workflows"
glob = ".github/workflows/*.yml"
category = "ci_declarative"
owner = "release/ci"
reason = "GitHub Actions workflows."
broad_glob_reason = "Workflow detail lives in a companion ledger."
created = "2026-05-09"
expires = "permanent"

"#,
        );
        push_allow(
            &mut text,
            r#"id = "non-rust-ripr-config"
path = "ripr.toml"
category = "policy_config"
owner = "policy"
reason = "ripr configuration."
created = "2026-05-09"
expires = "permanent"
"#,
        );
        text
    }

    fn generated_policy_fixture_text() -> String {
        let mut text = String::from(
            r#"schema_version = 1
policy = "generated-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

"#,
        );
        push_allow(
            &mut text,
            r#"id = "generated-no-panic-baseline"
path = "policy/no-panic-baseline.toml"
generator = "cargo xtask no-panic baseline --reset"
regenerate_command = "cargo xtask no-panic baseline --reset"
owner = "policy"
reason = "Generated by the no-panic classifier."
created = "2026-05-10"
expires = "permanent"
"#,
        );
        text
    }

    fn no_panic_baseline_fixture_text() -> String {
        let unwrap_snippet = ["let value = maybe.", "unwrap();"].concat();
        let panic_snippet = ["panic!", "(\"bad\");"].concat();
        format!(
            r#"schema_version = 1
policy = "no-panic-baseline"
owner = "EffortlessMetrics"
status = "advisory"

[policy_config]
mode = "no-new-debt"

[[entry]]
path = "src/lib.rs"
family = "unwrap"
selector_kind = "method-call"
selector_callee = "Option/Result::unwrap"
snippet = "{unwrap_snippet}"
count = 2

[[entry]]
path = "src/lib.rs"
family = "panic"
selector_kind = "macro-call"
selector_callee = "panic"
snippet = '{panic_snippet}'
count = 1
"#,
        )
    }

    fn no_panic_allowlist_fixture_text() -> String {
        r#"schema_version = 1
policy = "no-panic-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "no-panic-unwrap"
path = "src/lib.rs"
family = "unwrap"
owner = "parser"
classification = "reviewed_panic_exception"
explanation = "Parser validates the optional value."
created = "2026-05-09"
review_after = "2026-09-09"

[allow.selector]
kind = "method-call"
callee = "Option/Result::unwrap"
container = "load"
line_hint = 7

[allow.last_seen]
line = 7
column = 12

[[allow]]
path = "src/lib.rs"
family = "panic"

[allow.selector]
kind = "macro-call"
callee = "panic"
"#
        .to_string()
    }

    fn clippy_policy_fixture_text() -> String {
        let mut text = String::from(
            r#"schema_version = 1
policy = "clippy-exceptions"
owner = "EffortlessMetrics"
status = "advisory"

"#,
        );
        push_allow(
            &mut text,
            r#"id = "clippy-unwrap-policy"
path = "src/lib.rs"
lint = "clippy::unwrap_used"
family = "expect"
owner = "lint"
classification = "reviewed_lint_exception"
reason = "Fixture keeps an explicit lint suppression linked to policy."
policy_id = "clippy-unwrap-policy"
created = "2026-05-09"
review_after = "2026-09-09"
"#,
        );
        text
    }

    fn unsafe_policy_fixture_text() -> String {
        r#"schema_version = 1
policy = "unsafe-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "unsafe-read"
path = "src/lib.rs"
family = "unsafe_block"
owner = "runtime"
classification = "reviewed_unsafe_boundary"
reason = "Caller validates pointer before read."
evidence = ["unsafe-review:docs/evidence/unsafe/read.json"]
created = "2026-05-09"
review_after = "2026-09-09"

[allow.selector]
kind = "unsafe-block"
container = "read"
line_hint = 7

[allow.last_seen]
line = 7
column = 12

[[allow]]
path = "src/lib.rs"
family = "unsafe_fn"

[allow.selector]
kind = "unsafe-fn"
"#
        .to_string()
    }

    fn executable_policy_fixture_text() -> String {
        let mut text = String::from(
            r#"schema_version = 1
policy = "executable-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

"#,
        );
        push_allow(
            &mut text,
            r#"id = "exec-package-proof"
path = "scripts/package-proof.sh"
interpreter = "bash"
owner = "release"
reason = "Release preflight aggregator."
created = "2026-05-09"
expires = "permanent"
"#,
        );
        text
    }

    fn workflow_policy_fixture_text() -> String {
        let mut text = String::from(
            r#"schema_version = 1
policy = "workflow-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

"#,
        );
        text.push_str("[[entry]]\n");
        text.push_str(
            r#"path = ".github/workflows/ci.yml"
owner = "release/ci"
reason = "Primary PR correctness gate."
permissions = ["contents:read"]
secrets_used = []
external_actions = [
  "actions/checkout@v6.0.2",
  "Swatinem/rust-cache@v2",
]
created = "2026-05-09"
expires = "permanent"
"#,
        );
        text
    }

    fn dependency_policy_fixture_text() -> String {
        let mut text = String::from(
            r#"schema_version = 1
policy = "dependency-surface-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

"#,
        );
        push_allow(
            &mut text,
            r#"id = "dep-workspace-cargo-toml"
path = "Cargo.toml"
surface = "workspace_manifest"
owner = "release"
reason = "Workspace dependency block."
dep_count_at_baseline = 22
created = "2026-05-09"
expires = "permanent"

"#,
        );
        push_allow(
            &mut text,
            r#"id = "dep-crate-cargo-toml"
path = "crates/*/Cargo.toml"
surface = "crate_manifest"
owner = "release"
reason = "Per-crate manifests."
broad_glob_reason = "Per-crate enumeration would duplicate the workspace member list."
created = "2026-05-09"
expires = "permanent"
"#,
        );
        text
    }

    fn process_policy_fixture_text() -> String {
        let mut text = String::from(
            r#"schema_version = 1
policy = "process-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

"#,
        );
        push_allow(
            &mut text,
            r#"id = "proc-cargo-install-cargo-deny"
binary = "cargo"
argv_shape = ["install", "cargo-deny", "--locked"]
network_reach = true
called_by = [".github/workflows/ci.yml"]
owner = "release/ci"
reason = "Installs cargo-deny in the deny job."
created = "2026-05-09"
review_after = "2026-09-09"

"#,
        );
        push_allow(
            &mut text,
            r#"id = "proc-bash-package-proof"
binary = "bash"
argv_shape = ["scripts/package-proof.sh"]
network_reach = false
called_by = [".github/workflows/release.yml"]
owner = "release"
reason = "Release preflight package proof; pure local checks."
created = "2026-05-09"
expires = "permanent"
"#,
        );
        text
    }

    fn network_policy_fixture_text() -> String {
        let mut text = String::from(
            r#"schema_version = 1
policy = "network-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

"#,
        );
        push_allow(
            &mut text,
            r#"id = "net-crates-io-fetch"
destination = "crates.io"
auth_required = false
lane = "build"
owner = "release"
reason = "cargo fetch resolves and downloads crate dependencies."
created = "2026-05-09"
expires = "permanent"

"#,
        );
        push_allow(
            &mut text,
            r#"id = "net-github-api"
destination = "api.github.com"
auth_required = true
auth_secret = "GITHUB_TOKEN"
lane = "release"
owner = "release/ci"
reason = "Release uploads through the GitHub API."
created = "2026-05-09"
expires = "permanent"
"#,
        );
        text
    }

    fn push_allow(text: &mut String, body: &str) {
        text.push_str("[[");
        text.push_str("allow]]\n");
        text.push_str(body);
    }

    fn process_policy_finding(path: &str, symbol: &str) -> Finding {
        let mut identity = StructuralIdentity::new("policy", "process_spawn");
        identity.symbol = Some(symbol.to_string());
        identity.target_fingerprint = Some(format!("process:{symbol}"));
        Finding {
            kind: FindingKind::PolicyException,
            family: Some("process_spawn".to_string()),
            path: PathBuf::from(path),
            span: Some(Span { line: 1, column: 1 }),
            identity,
            message: String::new(),
        }
    }

    fn network_policy_finding(symbol: &str) -> Finding {
        let mut identity = StructuralIdentity::new("policy", "network_destination");
        identity.symbol = Some(symbol.to_string());
        identity.target_fingerprint = Some(format!("network:{symbol}"));
        Finding {
            kind: FindingKind::PolicyException,
            family: Some("network_destination".to_string()),
            path: PathBuf::from("policy/network-allowlist.toml"),
            span: Some(Span { line: 1, column: 1 }),
            identity,
            message: String::new(),
        }
    }

    fn panic_finding(
        path: &str,
        family: &str,
        ast_kind: &str,
        callee: Option<&str>,
        macro_name: Option<&str>,
        snippet: &str,
    ) -> Finding {
        let mut identity = StructuralIdentity::new("rust", ast_kind);
        identity.callee = callee.map(str::to_string);
        identity.macro_name = macro_name.map(str::to_string);
        identity.normalized_snippet_hash = Some(stable_hash_hex(&normalize_snippet(snippet)));
        Finding {
            kind: FindingKind::Panic,
            family: Some(family.to_string()),
            path: PathBuf::from(path),
            span: Some(Span { line: 1, column: 1 }),
            identity,
            message: String::new(),
        }
    }

    fn lint_finding(path: &str, family: &str, lint: &str, policy_id: Option<&str>) -> Finding {
        let mut identity = StructuralIdentity::new("rust", "attribute");
        identity.lint = Some(lint.to_string());
        identity.symbol = Some(format!(
            "#[expect({lint}, reason = \"policy:{}\")]",
            policy_id.unwrap_or("unlinked")
        ));
        identity.target_fingerprint = policy_id.map(|id| format!("policy:{id}"));
        Finding {
            kind: FindingKind::LintException,
            family: Some(family.to_string()),
            path: PathBuf::from(path),
            span: Some(Span { line: 1, column: 1 }),
            identity,
            message: String::new(),
        }
    }

    fn unsafe_finding(path: &str, family: &str, container: Option<&str>) -> Finding {
        let mut identity = StructuralIdentity::new("rust", family);
        identity.container = container.map(str::to_string);
        Finding {
            kind: FindingKind::Unsafe,
            family: Some(family.to_string()),
            path: PathBuf::from(path),
            span: Some(Span { line: 1, column: 1 }),
            identity,
            message: String::new(),
        }
    }

    fn finding(path: &str, ast_kind: &str) -> Finding {
        Finding {
            kind: FindingKind::NonRustFile,
            family: Some("configuration".to_string()),
            path: PathBuf::from(path),
            span: Some(Span { line: 1, column: 1 }),
            identity: StructuralIdentity::new("file", ast_kind),
            message: String::new(),
        }
    }
}
