use super::*;
use allow_core::{
    AllowConfig, AllowEntry, Finding, FindingKind, Lifecycle, Selector, Span, StructuralIdentity,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

mod finding_posture;
mod policy_entry;
mod policy_lifecycle;
mod policy_metadata;
mod policy_requirements;
mod policy_scope;
mod policy_selector;
mod policy_strings;
mod policy_workspace;
mod revision_findings;
mod revision_git_parser;

fn config_with(entry: AllowEntry) -> AllowConfig {
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry);
    cfg
}

fn entry(id: &str) -> AllowEntry {
    AllowEntry {
        id: id.to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "core".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: "Range is validated before use.".to_string(),
        evidence: vec!["test:range_is_validated".to_string()],
        links: Vec::new(),
        occurrence_limit: Some(1),
        lifecycle: Lifecycle {
            created: Some("2026-05-26".to_string()),
            review_after: Some("2026-08-01".to_string()),
            expires: Some("2026-09-01".to_string()),
        },
        selector: Selector {
            ast_kind: Some("method_call".to_string()),
            container: Some("load".to_string()),
            callee: Some("unwrap".to_string()),
            normalized_snippet_hash: Some("fnv1a64:1234".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn dependency_surface_entry(path: &str) -> AllowEntry {
    AllowEntry {
        id: "dep-cargo-toml".to_string(),
        kind: FindingKind::PolicyException,
        family: Some("dependency_surface".to_string()),
        path: Some(PathBuf::from(path)),
        glob: None,
        owner: "release".to_string(),
        classification: "dependency_surface".to_string(),
        reason: "Dependency surface is governed by policy.".to_string(),
        evidence: vec!["legacy-policy:dependency-surface".to_string()],
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: Some("2026-05-26".to_string()),
            review_after: Some("2026-08-01".to_string()),
            expires: None,
        },
        selector: Selector {
            ast_kind: Some("dependency_surface".to_string()),
            symbol: Some(path.to_string()),
            target_fingerprint: Some("workspace_manifest".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn generated_code_entry(path: &str) -> AllowEntry {
    AllowEntry {
        id: "generated-schema".to_string(),
        kind: FindingKind::GeneratedCode,
        family: Some("generated_code".to_string()),
        path: Some(PathBuf::from(path)),
        glob: None,
        owner: "codegen".to_string(),
        classification: "generated_code".to_string(),
        reason: "Generated schema is tracked for review.".to_string(),
        evidence: vec!["legacy-policy:generated".to_string()],
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: Some("2026-05-26".to_string()),
            review_after: Some("2026-08-01".to_string()),
            expires: None,
        },
        selector: Selector {
            ast_kind: Some("tracked_file".to_string()),
            symbol: Some(path.to_string()),
            target_fingerprint: Some("json".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn workflow_entry(
    id: &str,
    family: &str,
    ast_kind: &str,
    path: &str,
    target_fingerprint: Option<&str>,
) -> AllowEntry {
    AllowEntry {
        id: id.to_string(),
        kind: FindingKind::PolicyException,
        family: Some(family.to_string()),
        path: Some(PathBuf::from(path)),
        glob: None,
        owner: "ci".to_string(),
        classification: family.to_string(),
        reason: "Workflow surface is governed by policy.".to_string(),
        evidence: vec!["legacy-policy:workflow".to_string()],
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: Some("2026-05-26".to_string()),
            review_after: Some("2026-08-01".to_string()),
            expires: None,
        },
        selector: Selector {
            ast_kind: Some(ast_kind.to_string()),
            symbol: Some(path.to_string()),
            target_fingerprint: target_fingerprint.map(str::to_string),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn executable_entry(path: &str) -> AllowEntry {
    AllowEntry {
        id: "exec-package-proof".to_string(),
        kind: FindingKind::PolicyException,
        family: Some("executable_file".to_string()),
        path: Some(PathBuf::from(path)),
        glob: None,
        owner: "release".to_string(),
        classification: "executable_file".to_string(),
        reason: "Release helper intentionally retains an executable bit.".to_string(),
        evidence: vec!["legacy-policy:executable".to_string()],
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: Some("2026-05-26".to_string()),
            review_after: Some("2026-08-01".to_string()),
            expires: None,
        },
        selector: Selector {
            ast_kind: Some("git_executable_file".to_string()),
            symbol: Some(path.to_string()),
            target_fingerprint: Some("git-mode:100755".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn config_policy_entry(
    id: &str,
    family: &str,
    path: &str,
    symbol: &str,
    target_fingerprint: &str,
) -> AllowEntry {
    AllowEntry {
        id: id.to_string(),
        kind: FindingKind::PolicyException,
        family: Some(family.to_string()),
        path: Some(PathBuf::from(path)),
        glob: None,
        owner: "infra".to_string(),
        classification: family.to_string(),
        reason: "Policy surface is retained for review.".to_string(),
        evidence: vec!["legacy-policy:policy".to_string()],
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: Some("2026-05-26".to_string()),
            review_after: Some("2026-08-01".to_string()),
            expires: None,
        },
        selector: Selector {
            ast_kind: Some(family.to_string()),
            symbol: Some(symbol.to_string()),
            target_fingerprint: Some(target_fingerprint.to_string()),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn finding(path: &str, line: u32, container: &str) -> Finding {
    let mut identity = StructuralIdentity::new("rust", "unsafe_fn");
    identity.container = Some(container.to_string());
    identity.normalized_snippet_hash = Some(format!("fnv1a64:{container}"));
    Finding {
        kind: FindingKind::Unsafe,
        family: Some("unsafe_fn".to_string()),
        path: PathBuf::from(path),
        span: Some(Span { line, column: 1 }),
        identity,
        message: "test finding".to_string(),
    }
}

fn temp_root(label: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_else(|err| std::panic::panic_any(format!("system clock: {err}")))
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-diff-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("temp root: {err}")));
    root
}

fn git(root: &PathBuf, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("git {args:?}: {err}")));
    if !output.status.success() {
        std::panic::panic_any(format!(
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
}
