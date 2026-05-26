use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowResult, Finding, FindingKind, LastSeen,
    Lifecycle, Requirements, Selector, WorkspaceConfig, glob_matches, normalize_path,
};
use allow_policy::{parse_policy, validate_policy};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use toml::Value;

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
    parse_policy(&text)
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

pub fn generated_findings_from_gitattributes(
    root: impl AsRef<Path>,
) -> CargoAllowResult<Vec<Finding>> {
    let path = root.as_ref().join(".gitattributes");
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(&path)
        .map_err(|e| CargoAllowError::new(format!("failed to read {}: {e}", path.display())))?;
    Ok(generated_paths_from_gitattributes(&text)
        .into_iter()
        .map(generated_finding)
        .collect())
}

pub fn executable_findings_from_git(root: impl AsRef<Path>) -> CargoAllowResult<Vec<Finding>> {
    let output = Command::new("git")
        .args(["ls-files", "--stage"])
        .current_dir(root.as_ref())
        .output()
        .map_err(|e| CargoAllowError::new(format!("failed to run git ls-files --stage: {e}")))?;
    if !output.status.success() {
        return Err(CargoAllowError::new(format!(
            "git ls-files --stage failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|e| CargoAllowError::new(format!("git ls-files output was not UTF-8: {e}")))?;
    Ok(executable_findings_from_git_stage(&text))
}

fn git_ls_files(root: impl AsRef<Path>) -> CargoAllowResult<Vec<PathBuf>> {
    let output = Command::new("git")
        .args(["ls-files"])
        .current_dir(root.as_ref())
        .output()
        .map_err(|e| CargoAllowError::new(format!("failed to run git ls-files: {e}")))?;
    if !output.status.success() {
        return Err(CargoAllowError::new(format!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|e| CargoAllowError::new(format!("git ls-files output was not UTF-8: {e}")))?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .collect())
}

pub fn workflow_findings_from_files(root: impl AsRef<Path>) -> CargoAllowResult<Vec<Finding>> {
    let root = root.as_ref();
    let workflows_dir = root.join(".github").join("workflows");
    if !workflows_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    for entry in fs::read_dir(&workflows_dir).map_err(|e| {
        CargoAllowError::new(format!("failed to read {}: {e}", workflows_dir.display()))
    })? {
        let entry = entry.map_err(|e| {
            CargoAllowError::new(format!(
                "failed to read {} entry: {e}",
                workflows_dir.display()
            ))
        })?;
        let path = entry.path();
        if is_workflow_path(&path) {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            paths.push(PathBuf::from(rel));
        }
    }
    paths.sort();

    let mut findings = Vec::new();
    for path in paths {
        findings.push(workflow_file_finding(path.clone()));
        let full_path = root.join(
            path.to_string_lossy()
                .replace('/', std::path::MAIN_SEPARATOR_STR),
        );
        let text = fs::read_to_string(&full_path).map_err(|e| {
            CargoAllowError::new(format!("failed to read {}: {e}", full_path.display()))
        })?;
        let uses = text
            .lines()
            .filter_map(extract_workflow_uses)
            .collect::<BTreeSet<_>>();
        findings.extend(
            uses.into_iter()
                .map(|action| workflow_action_finding(path.clone(), action)),
        );
    }
    Ok(findings)
}

pub fn dependency_surface_findings_from_git(
    root: impl AsRef<Path>,
    cfg: &AllowConfig,
) -> CargoAllowResult<Vec<Finding>> {
    let tracked = git_ls_files(root)?;
    let mut paths = BTreeSet::new();
    for entry in &cfg.allow {
        if entry.kind != FindingKind::PolicyException
            || entry.family.as_deref() != Some("dependency_surface")
        {
            continue;
        }
        for path in &tracked {
            if dependency_entry_matches_path(entry, path) {
                paths.insert(path.clone());
            }
        }
    }
    Ok(paths.into_iter().map(dependency_surface_finding).collect())
}

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

pub fn migration_notes() -> &'static str {
    "Legacy migration accepts canonical cargo-allow policies plus shiplog-style non-rust, generated, executable, workflow, dependency-surface, and process allowlists. Non-Rust compat expands matching legacy globs to exact current file entries; generated compat compares .gitattributes generated paths with policy/generated-allowlist.toml; executable compat compares git tree mode 100755 paths with policy/executable-allowlist.toml; workflow compat compares .github/workflows files and uses: actions with policy/workflow-allowlist.toml; dependency-surface compat preserves the legacy pattern-matches-tracked-file check; process compat validates retained process policy entries and does not scan source code for process spawns."
}

fn read_policy(path: &Path) -> CargoAllowResult<String> {
    fs::read_to_string(path).map_err(|e| {
        CargoAllowError::new(format!(
            "failed to read legacy policy {}: {e}",
            path.display()
        ))
    })
}

fn legacy_table(input: &str) -> CargoAllowResult<Option<toml::Table>> {
    toml::from_str::<toml::Table>(input)
        .map(Some)
        .map_err(|e| CargoAllowError::new(format!("failed to parse legacy policy TOML: {e}")))
}

#[derive(Debug, Clone)]
struct LegacyNonRustRule {
    id: String,
    pattern: String,
    is_path: bool,
    owner: String,
    classification: String,
    reason: String,
    created: Option<String>,
    review_after: Option<String>,
    expires: Option<String>,
}

#[derive(Debug, Clone)]
struct LegacyGeneratedRule {
    id: String,
    path: String,
    owner: String,
    reason: String,
    generator: Option<String>,
    regenerate_command: Option<String>,
    created: Option<String>,
    expires: Option<String>,
}

#[derive(Debug, Clone)]
struct LegacyExecutableRule {
    id: String,
    path: String,
    owner: String,
    reason: String,
    interpreter: Option<String>,
    created: Option<String>,
    review_after: Option<String>,
    expires: Option<String>,
}

#[derive(Debug, Clone)]
struct LegacyWorkflowRule {
    path: String,
    owner: String,
    reason: String,
    permissions: Vec<String>,
    secrets_used: Vec<String>,
    external_actions: Vec<String>,
    duplicate_of_lane: Option<String>,
    created: Option<String>,
    review_after: Option<String>,
    expires: Option<String>,
}

#[derive(Debug, Clone)]
struct LegacyDependencySurfaceRule {
    id: String,
    pattern: String,
    is_glob: bool,
    surface: String,
    owner: String,
    reason: String,
    broad_glob_reason: Option<String>,
    dep_count_at_baseline: Option<i64>,
    created: Option<String>,
    review_after: Option<String>,
    expires: Option<String>,
}

#[derive(Debug, Clone)]
struct LegacyProcessRule {
    id: String,
    binary: String,
    argv_shape: Vec<String>,
    network_reach: bool,
    called_by: Vec<String>,
    owner: String,
    reason: String,
    created: Option<String>,
    review_after: Option<String>,
    expires: Option<String>,
}

impl LegacyNonRustRule {
    fn matches(&self, finding: &Finding) -> bool {
        if !matches!(
            finding.kind,
            FindingKind::NonRustFile | FindingKind::GeneratedCode
        ) {
            return false;
        }
        if self.is_path {
            normalize_path(&self.pattern) == normalize_path(&finding.path)
        } else {
            glob_matches(&self.pattern, &finding.path)
        }
    }

    fn specificity(&self) -> usize {
        let literal_chars = self
            .pattern
            .chars()
            .filter(|ch| !matches!(ch, '*' | '?' | '[' | ']' | '{' | '}' | ',' | '!'))
            .count();
        literal_chars + if self.is_path { 10_000 } else { 0 }
    }
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
    let reason = match (
        string_field(table, "reason"),
        string_field(table, "broad_glob_reason"),
    ) {
        (Some(reason), Some(scope_reason)) if !scope_reason.trim().is_empty() => {
            format!("{reason} Scope note: {scope_reason}")
        }
        (Some(reason), _) => reason,
        (None, Some(scope_reason)) => scope_reason,
        (None, None) => String::new(),
    };
    Ok(LegacyNonRustRule {
        id,
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

fn generated_paths_from_gitattributes(input: &str) -> Vec<PathBuf> {
    input
        .lines()
        .filter_map(generated_path_from_gitattributes_line)
        .map(PathBuf::from)
        .collect()
}

fn generated_path_from_gitattributes_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('#')
        || !trimmed.contains("linguist-generated=true")
    {
        return None;
    }
    trimmed
        .split_whitespace()
        .next()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(str::to_string)
}

fn generated_finding(path: PathBuf) -> Finding {
    let normalized = normalize_path(&path);
    let mut identity = allow_core::StructuralIdentity::new("file", "tracked_file");
    identity.symbol = Some(normalized);
    identity.target_fingerprint = file_fingerprint(&path);
    Finding {
        kind: FindingKind::GeneratedCode,
        family: Some("generated_code".to_string()),
        path,
        span: Some(allow_core::Span { line: 1, column: 1 }),
        identity,
        message: "tracked generated file from .gitattributes".to_string(),
    }
}

fn executable_findings_from_git_stage(input: &str) -> Vec<Finding> {
    input
        .lines()
        .filter_map(executable_path_from_git_stage_line)
        .map(executable_finding)
        .collect()
}

fn executable_path_from_git_stage_line(line: &str) -> Option<PathBuf> {
    let (meta, path) = line.split_once('\t')?;
    let mode = meta.split_whitespace().next()?;
    if mode == "100755" && !path.trim().is_empty() {
        Some(PathBuf::from(path.trim()))
    } else {
        None
    }
}

fn executable_finding(path: PathBuf) -> Finding {
    let normalized = normalize_path(&path);
    let mut identity = allow_core::StructuralIdentity::new("file", "git_executable_file");
    identity.symbol = Some(normalized);
    identity.target_fingerprint = Some("git-mode:100755".to_string());
    Finding {
        kind: FindingKind::PolicyException,
        family: Some("executable_file".to_string()),
        path,
        span: Some(allow_core::Span { line: 1, column: 1 }),
        identity,
        message: "tracked file has git executable bit".to_string(),
    }
}

fn workflow_file_finding(path: PathBuf) -> Finding {
    let normalized = normalize_path(&path);
    let mut identity = allow_core::StructuralIdentity::new("workflow", "github_workflow");
    identity.symbol = Some(normalized);
    Finding {
        kind: FindingKind::PolicyException,
        family: Some("github_workflow".to_string()),
        path,
        span: Some(allow_core::Span { line: 1, column: 1 }),
        identity,
        message: "GitHub Actions workflow file".to_string(),
    }
}

fn workflow_action_finding(path: PathBuf, action: String) -> Finding {
    let normalized = normalize_path(&path);
    let mut identity = allow_core::StructuralIdentity::new("workflow", "github_action_uses");
    identity.symbol = Some(workflow_action_symbol(&normalized, &action));
    identity.target_fingerprint = Some(format!("action:{action}"));
    Finding {
        kind: FindingKind::PolicyException,
        family: Some("workflow_external_action".to_string()),
        path,
        span: Some(allow_core::Span { line: 1, column: 1 }),
        identity,
        message: format!("GitHub Actions workflow uses external action {action}"),
    }
}

fn dependency_surface_finding(path: PathBuf) -> Finding {
    let normalized = normalize_path(&path);
    let mut identity = allow_core::StructuralIdentity::new("file", "dependency_surface");
    identity.symbol = Some(normalized.clone());
    identity.target_fingerprint = Some(dependency_surface_family(&path));
    Finding {
        kind: FindingKind::PolicyException,
        family: Some("dependency_surface".to_string()),
        path,
        span: Some(allow_core::Span { line: 1, column: 1 }),
        identity,
        message: format!("tracked dependency surface {normalized}"),
    }
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

fn dependency_surface_family(path: &Path) -> String {
    let normalized = normalize_path(path);
    match normalized.as_str() {
        "Cargo.toml" => "workspace_manifest".to_string(),
        "Cargo.lock" => "workspace_lockfile".to_string(),
        "rust-toolchain.toml" => "toolchain_pin".to_string(),
        "deny.toml" => "policy_config".to_string(),
        text if text.ends_with("/Cargo.toml") => "crate_manifest".to_string(),
        text if text.ends_with("/Cargo.lock") => "lockfile".to_string(),
        text if text.ends_with("/rust-toolchain.toml") => "toolchain_pin".to_string(),
        _ => "dependency_surface".to_string(),
    }
}

fn dependency_entry_matches_path(entry: &AllowEntry, path: &Path) -> bool {
    entry
        .path
        .as_ref()
        .is_some_and(|scope| normalize_path(scope) == normalize_path(path))
        || entry
            .glob
            .as_ref()
            .is_some_and(|glob| glob_matches(glob, path))
        || entry
            .selector
            .glob
            .as_ref()
            .is_some_and(|glob| glob_matches(glob, path))
}

fn extract_workflow_uses(line: &str) -> Option<String> {
    let trimmed = line.trim().trim_start_matches('-').trim_start();
    let stripped = trimmed.strip_prefix("uses:")?;
    let value = stripped.trim();
    if value.is_empty() {
        return None;
    }
    let no_comment = value.split('#').next().unwrap_or(value).trim();
    if no_comment.is_empty() {
        None
    } else {
        Some(no_comment.to_string())
    }
}

fn workflow_action_symbol(path: &str, action: &str) -> String {
    format!("{path} uses {action}")
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

fn is_workflow_path(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("yml" | "yaml")
    )
}

fn file_fingerprint(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .or_else(|| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.to_ascii_lowercase())
        })
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

fn string_field(table: &toml::Table, field: &str) -> Option<String> {
    table
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn string_array_field(table: &toml::Table, field: &str) -> Vec<String> {
    table
        .get(field)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn required_string_field(
    table: &toml::Table,
    field: &str,
    context: &str,
) -> CargoAllowResult<String> {
    string_field(table, field)
        .ok_or_else(|| CargoAllowError::new(format!("{context} missing {field}")))
}

fn required_string_array_field(
    table: &toml::Table,
    field: &str,
    context: &str,
) -> CargoAllowResult<Vec<String>> {
    let values = string_array_field(table, field);
    if values.is_empty() {
        Err(CargoAllowError::new(format!("{context} missing {field}")))
    } else {
        Ok(values)
    }
}

fn required_bool_field(table: &toml::Table, field: &str, context: &str) -> CargoAllowResult<bool> {
    table
        .get(field)
        .and_then(Value::as_bool)
        .ok_or_else(|| CargoAllowError::new(format!("{context} missing {field}")))
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

    static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

    fn policy_fixture_path() -> PathBuf {
        let path = fixture_dir().join("non-rust-allowlist.toml");
        fs::write(&path, policy_fixture_text())
            .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
        path
    }

    fn generated_policy_fixture_path() -> PathBuf {
        let path = fixture_dir().join("generated-allowlist.toml");
        fs::write(&path, generated_policy_fixture_text())
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
