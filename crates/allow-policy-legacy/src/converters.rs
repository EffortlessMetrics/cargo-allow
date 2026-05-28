use allow_core::{
    AllowConfig, AllowEntry, CargoAllowResult, Finding, FindingKind, LastSeen, Lifecycle,
    Requirements, Selector, WorkspaceConfig, normalize_path, normalize_snippet, stable_hash_hex,
};
use allow_policy::validate_policy;
use std::path::{Path, PathBuf};

use crate::converter_support::{
    best_rule_index, cargo_allow_panic_family, dependency_surface_evidence, executable_evidence,
    generated_evidence, lifecycle_from_rule, lifecycle_from_workflow_rule, network_evidence,
    network_fingerprint, network_symbol, no_panic_macro_name, no_panic_method_callee,
    normalize_selector_kind, process_evidence, process_fingerprint, process_scope, process_symbol,
    slug_id, unsafe_evidence, workflow_evidence,
};
use crate::fields::string_field;
use crate::findings::{file_fingerprint, workflow_action_symbol};
use crate::types::{
    LegacyClippyRule, LegacyDependencySurfaceRule, LegacyExecutableRule, LegacyGeneratedRule,
    LegacyNetworkRule, LegacyNoPanicAllowEntry, LegacyNoPanicBaselineEntry, LegacyNonRustRule,
    LegacyProcessRule, LegacyUnsafeRule, LegacyWorkflowRule,
};
use crate::{default_baseline_created, default_baseline_expires};

pub(crate) fn config_from_non_rust_rules(
    table: &toml::Table,
    rules: &[LegacyNonRustRule],
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = rules.iter().map(entry_from_rule).collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

pub(crate) fn config_from_generated_rules(
    table: &toml::Table,
    rules: &[LegacyGeneratedRule],
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = rules.iter().map(entry_from_generated_rule).collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

pub(crate) fn config_from_no_panic_baseline_entries(
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

pub(crate) fn config_from_no_panic_allowlist_entries(
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

pub(crate) fn config_from_clippy_rules(
    table: &toml::Table,
    rules: &[LegacyClippyRule],
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = rules.iter().map(entry_from_clippy_rule).collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

pub(crate) fn config_from_unsafe_rules(
    table: &toml::Table,
    rules: &[LegacyUnsafeRule],
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = rules.iter().map(entry_from_unsafe_rule).collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

pub(crate) fn config_from_executable_rules(
    table: &toml::Table,
    rules: &[LegacyExecutableRule],
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = rules.iter().map(entry_from_executable_rule).collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

pub(crate) fn config_from_workflow_rules(
    table: &toml::Table,
    rules: &[LegacyWorkflowRule],
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = rules.iter().flat_map(entries_from_workflow_rule).collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

pub(crate) fn config_from_dependency_surface_rules(
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

pub(crate) fn config_from_process_rules(
    table: &toml::Table,
    rules: &[LegacyProcessRule],
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = rules.iter().map(entry_from_process_rule).collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

pub(crate) fn config_from_network_rules(
    table: &toml::Table,
    rules: &[LegacyNetworkRule],
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = rules.iter().map(entry_from_network_rule).collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

pub(crate) fn config_from_current_non_rust_findings(
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
