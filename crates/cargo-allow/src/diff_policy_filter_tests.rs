use super::*;
use allow_core::{AllowConfig, AllowEntry, FindingKind, Lifecycle, Selector};
use std::path::PathBuf;

#[test]
fn diff_policy_changes_respect_kind_filter_for_base_and_head() {
    let mut base = AllowConfig::empty();
    base.allow.push(entry("allow-panic", FindingKind::Panic));
    base.allow.push(entry("allow-unsafe", FindingKind::Unsafe));

    let mut head = AllowConfig::empty();
    head.allow.push(entry("allow-panic", FindingKind::Panic));
    head.allow
        .push(entry("allow-non-rust", FindingKind::NonRustFile));

    let changes = policy_changes_for_diff(Some(base.clone()), &head, Some("panic"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy diff: {err}")));

    assert!(
        changes.is_empty(),
        "kind-filtered diff should not report unrelated policy removals/additions: {changes:?}"
    );

    let unfiltered_changes = policy_changes_for_diff(Some(base), &head, None)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy diff: {err}")));

    assert!(unfiltered_changes.iter().any(|change| {
        change.allow_id == "allow-unsafe"
            && change.kind == allow_diff::PolicyChangeKind::RemovedAllow
    }));
    assert!(unfiltered_changes.iter().any(|change| {
        change.allow_id == "allow-non-rust"
            && change.kind == allow_diff::PolicyChangeKind::AddedAllow
    }));
}

fn entry(id: &str, kind: FindingKind) -> AllowEntry {
    AllowEntry {
        id: id.to_string(),
        kind,
        family: Some(family_for(kind).to_string()),
        path: Some(PathBuf::from(path_for(kind))),
        glob: None,
        owner: "core".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: "Retained for policy diff regression coverage.".to_string(),
        evidence: vec!["test:policy_diff_filter".to_string()],
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: Some("2026-05-26".to_string()),
            review_after: Some("2026-08-01".to_string()),
            expires: Some("2026-12-01".to_string()),
        },
        selector: Selector {
            ast_kind: Some(ast_kind_for(kind).to_string()),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn family_for(kind: FindingKind) -> &'static str {
    match kind {
        FindingKind::Panic => "unwrap",
        FindingKind::Unsafe => "unsafe_block",
        FindingKind::NonRustFile => "script",
        FindingKind::LintException => "allow",
        FindingKind::GeneratedCode => "generated_code",
        FindingKind::PolicyException => "policy",
    }
}

fn ast_kind_for(kind: FindingKind) -> &'static str {
    match kind {
        FindingKind::Panic => "method_call",
        FindingKind::Unsafe => "unsafe_block",
        FindingKind::NonRustFile => "tracked_file",
        FindingKind::LintException => "attribute",
        FindingKind::GeneratedCode => "tracked_file",
        FindingKind::PolicyException => "policy_exception",
    }
}

fn path_for(kind: FindingKind) -> &'static str {
    match kind {
        FindingKind::Panic => "src/lib.rs",
        FindingKind::Unsafe => "src/ffi.rs",
        FindingKind::NonRustFile => "scripts/release.sh",
        FindingKind::LintException => "src/lints.rs",
        FindingKind::GeneratedCode => "generated/schema.rs",
        FindingKind::PolicyException => "policy/allow.toml",
    }
}
