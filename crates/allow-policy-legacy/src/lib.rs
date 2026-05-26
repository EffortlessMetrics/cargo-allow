use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowResult, Finding, FindingKind, LastSeen,
    Lifecycle, Requirements, Selector, WorkspaceConfig, glob_matches, normalize_path,
};
use allow_policy::{parse_policy, validate_policy};
use std::fs;
use std::path::{Path, PathBuf};
use toml::Value;

pub fn load_legacy_or_canonical(path: impl AsRef<Path>) -> CargoAllowResult<AllowConfig> {
    let text = read_policy(path.as_ref())?;
    if let Some(table) = legacy_table(&text)?
        && table.get("policy").and_then(Value::as_str) == Some("non-rust-allowlist")
    {
        let rules = parse_non_rust_rules(&table)?;
        return config_from_non_rust_rules(&table, &rules);
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

pub fn migration_notes() -> &'static str {
    "Legacy migration accepts canonical cargo-allow policies and shiplog-style policy/non-rust-allowlist.toml files. Non-Rust compat mode expands matching legacy globs to exact current file entries so cargo-allow can run side-by-side with existing xtask file-policy checks."
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

fn config_from_non_rust_rules(
    table: &toml::Table,
    rules: &[LegacyNonRustRule],
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = rules.iter().map(entry_from_rule).collect();
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

fn lifecycle_from_rule(rule: &LegacyNonRustRule) -> Lifecycle {
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

fn normalize_legacy_expires(expires: Option<String>) -> Option<String> {
    expires.map(|value| {
        if value == "permanent" {
            "never".to_string()
        } else {
            value
        }
    })
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

    static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

    fn policy_fixture_path() -> PathBuf {
        let id = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "cargo-allow-policy-legacy-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&dir)
            .unwrap_or_else(|err| std::panic::panic_any(format!("fixture dir: {err}")));
        let path = dir.join("non-rust-allowlist.toml");
        fs::write(&path, policy_fixture_text())
            .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
        path
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

    fn push_allow(text: &mut String, body: &str) {
        text.push_str("[[");
        text.push_str("allow]]\n");
        text.push_str(body);
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
