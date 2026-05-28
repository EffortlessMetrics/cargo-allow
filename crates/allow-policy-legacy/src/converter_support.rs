use allow_core::{Finding, Lifecycle, normalize_path};
use std::path::Path;

use crate::types::{
    LegacyDependencySurfaceRule, LegacyExecutableRule, LegacyGeneratedRule, LegacyNetworkRule,
    LegacyNonRustRule, LegacyProcessRule, LegacyUnsafeRule, LegacyWorkflowRule,
};

pub(crate) fn best_rule_index(rules: &[LegacyNonRustRule], finding: &Finding) -> Option<usize> {
    rules
        .iter()
        .enumerate()
        .filter(|(_, rule)| rule.matches(finding))
        .max_by_key(|(_, rule)| rule.specificity())
        .map(|(index, _)| index)
}

pub(crate) fn generated_evidence(rule: &LegacyGeneratedRule) -> Vec<String> {
    let mut evidence = Vec::new();
    if let Some(generator) = &rule.generator {
        evidence.push(format!("generator:{generator}"));
    }
    if let Some(command) = &rule.regenerate_command {
        evidence.push(format!("cargo:{command}"));
    }
    evidence
}

pub(crate) fn unsafe_evidence(rule: &LegacyUnsafeRule) -> Vec<String> {
    if rule.evidence.is_empty() {
        vec!["TODO: add unsafe-review or boundary-test evidence".to_string()]
    } else {
        rule.evidence.clone()
    }
}

pub(crate) fn executable_evidence(rule: &LegacyExecutableRule) -> Vec<String> {
    rule.interpreter
        .as_ref()
        .map(|interpreter| vec![format!("interpreter:{interpreter}")])
        .unwrap_or_default()
}

pub(crate) fn workflow_evidence(rule: &LegacyWorkflowRule) -> Vec<String> {
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

pub(crate) fn dependency_surface_evidence(rule: &LegacyDependencySurfaceRule) -> Vec<String> {
    let mut evidence = vec![format!("surface:{}", rule.surface)];
    if let Some(count) = rule.dep_count_at_baseline {
        evidence.push(format!("dep_count_at_baseline:{count}"));
    }
    evidence
}

pub(crate) fn process_evidence(rule: &LegacyProcessRule) -> Vec<String> {
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

pub(crate) fn network_evidence(rule: &LegacyNetworkRule) -> Vec<String> {
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

pub(crate) fn cargo_allow_panic_family(family: &str) -> String {
    if family == "panic" {
        "panic_macro".to_string()
    } else {
        family.to_string()
    }
}

pub(crate) fn normalize_selector_kind(kind: &str) -> String {
    kind.replace('-', "_")
}

pub(crate) fn no_panic_macro_name(family: &str) -> String {
    if family == "panic" {
        "panic".to_string()
    } else {
        family.to_string()
    }
}

pub(crate) fn no_panic_method_callee(family: &str, selector_callee: Option<&str>) -> String {
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

pub(crate) fn lifecycle_from_rule(rule: &LegacyNonRustRule) -> Lifecycle {
    Lifecycle {
        created: rule.created.clone(),
        review_after: rule.review_after.clone(),
        expires: rule.expires.clone(),
    }
}

pub(crate) fn lifecycle_from_workflow_rule(rule: &LegacyWorkflowRule) -> Lifecycle {
    Lifecycle {
        created: rule.created.clone(),
        review_after: rule.review_after.clone(),
        expires: rule.expires.clone(),
    }
}

pub(crate) fn slug_id(input: &str) -> String {
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
