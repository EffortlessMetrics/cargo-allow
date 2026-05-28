use allow_core::{AllowEntry, FindingKind, Lifecycle, Selector, normalize_path};
use std::path::PathBuf;

use crate::converter_support::{
    dependency_surface_evidence, executable_evidence, lifecycle_from_workflow_rule,
    network_evidence, network_fingerprint, network_symbol, process_evidence, process_fingerprint,
    process_scope, process_symbol, slug_id, workflow_evidence,
};
use crate::findings::workflow_action_symbol;
use crate::types::{
    LegacyDependencySurfaceRule, LegacyExecutableRule, LegacyNetworkRule, LegacyProcessRule,
    LegacyWorkflowRule,
};

pub(crate) fn entry_from_executable_rule(rule: &LegacyExecutableRule) -> AllowEntry {
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

pub(crate) fn entries_from_workflow_rule(rule: &LegacyWorkflowRule) -> Vec<AllowEntry> {
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

pub(crate) fn entry_from_dependency_surface_rule(rule: &LegacyDependencySurfaceRule) -> AllowEntry {
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

pub(crate) fn entry_from_process_rule(rule: &LegacyProcessRule) -> AllowEntry {
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

pub(crate) fn entry_from_network_rule(rule: &LegacyNetworkRule) -> AllowEntry {
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
