use super::*;
use allow_core::{AllowConfig, AllowEntry, FindingKind, Lifecycle, Selector};
use std::fs;
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

#[test]
fn diff_policy_changes_keep_ledger_level_posture_under_kind_filter() {
    let mut base = AllowConfig::empty();
    base.status = Some("active".to_string());
    base.allow.push(entry("allow-panic", FindingKind::Panic));
    base.allow.push(entry("allow-unsafe", FindingKind::Unsafe));

    let mut head = AllowConfig::empty();
    head.status = Some("advisory".to_string());
    head.allow.push(entry("allow-panic", FindingKind::Panic));
    head.allow
        .push(entry("allow-non-rust", FindingKind::NonRustFile));

    let changes = policy_changes_for_diff(Some(base), &head, Some("panic"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy diff: {err}")));

    assert!(
        changes.iter().any(|change| {
            change.allow_id == "policy.status"
                && change.kind == allow_diff::PolicyChangeKind::PolicyStatusWeakened
                && change.severity == allow_diff::PolicyChangeSeverity::Fail
        }),
        "kind-filtered diff should keep ledger-level policy weakening: {changes:?}"
    );
    assert!(
        !changes
            .iter()
            .any(|change| change.allow_id == "allow-unsafe"),
        "kind-filtered diff should still suppress unrelated allow-entry removals: {changes:?}"
    );
    assert!(
        !changes
            .iter()
            .any(|change| change.allow_id == "allow-non-rust"),
        "kind-filtered diff should still suppress unrelated allow-entry additions: {changes:?}"
    );
}

#[test]
fn diff_policy_changes_report_added_entries_when_base_policy_is_missing() {
    let mut head = AllowConfig::empty();
    head.allow.push(entry("allow-reviewed", FindingKind::Panic));
    let mut baseline = entry("allow-baseline", FindingKind::Unsafe);
    baseline.classification = "baseline_debt".to_string();
    head.allow.push(baseline);

    let changes = policy_changes_for_diff(None, &head, None)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy diff: {err}")));

    assert!(
        changes.iter().any(|change| {
            change.allow_id == "allow-reviewed"
                && change.kind == allow_diff::PolicyChangeKind::AddedAllow
                && change.severity == allow_diff::PolicyChangeSeverity::Review
        }),
        "diff should report reviewed allow entries added with a new policy file: {changes:?}"
    );
    assert!(
        changes.iter().any(|change| {
            change.allow_id == "allow-baseline"
                && change.kind == allow_diff::PolicyChangeKind::BaselineDebtAdded
                && change.severity == allow_diff::PolicyChangeSeverity::Fail
        }),
        "diff should report added baseline_debt when a new policy file introduces it: {changes:?}"
    );
}

#[test]
fn diff_policy_changes_promote_broken_added_evidence_to_failure() {
    let root = diff_fixture_dir();
    let mut base_entry = entry("allow-panic", FindingKind::Panic);
    base_entry.evidence.clear();
    let base = config_with(base_entry);
    let mut head_entry = entry("allow-panic", FindingKind::Panic);
    head_entry.evidence = vec!["doc:docs/missing.md".to_string()];
    let head = config_with(head_entry);
    let mut changes = policy_changes_for_diff(Some(base), &head, None)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy diff: {err}")));

    promote_broken_added_local_reference_policy_changes(&root, None, &head, &mut changes)
        .unwrap_or_else(|err| std::panic::panic_any(format!("promote evidence changes: {err}")));

    let change = changes
        .iter()
        .find(|change| change.kind == allow_diff::PolicyChangeKind::EvidenceAdded)
        .unwrap_or_else(|| std::panic::panic_any("evidence addition should be reported"));
    assert_eq!(change.severity, allow_diff::PolicyChangeSeverity::Fail);
    assert!(
        change.message.contains("broken local evidence added"),
        "message should explain source-tree evidence failure: {change:?}"
    );
    let evidence = change
        .evidence
        .as_ref()
        .unwrap_or_else(|| std::panic::panic_any("evidence change should include added values"));
    assert_eq!(evidence.added, vec!["doc:docs/missing.md".to_string()]);
    remove_diff_fixture_dir(root);
}

#[test]
fn diff_policy_changes_explain_added_evidence_outside_compared_inventory() {
    let root = diff_fixture_dir();
    let mut base_entry = entry("allow-panic", FindingKind::Panic);
    base_entry.evidence.clear();
    let base = config_with(base_entry);
    let mut head_entry = entry("allow-panic", FindingKind::Panic);
    head_entry.evidence = vec!["doc:docs/untracked.md".to_string()];
    let head = config_with(head_entry);
    let mut changes = policy_changes_for_diff(Some(base), &head, None)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy diff: {err}")));
    let compared_files = std::collections::BTreeSet::new();

    promote_broken_added_local_reference_policy_changes(
        &root,
        Some(&compared_files),
        &head,
        &mut changes,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("promote evidence changes: {err}")));

    let change = changes
        .iter()
        .find(|change| change.kind == allow_diff::PolicyChangeKind::EvidenceAdded)
        .unwrap_or_else(|| std::panic::panic_any("evidence addition should be reported"));
    assert_eq!(change.severity, allow_diff::PolicyChangeSeverity::Fail);
    assert!(
        change
            .message
            .contains("outside compared source-tree inventory"),
        "message should explain source-tree inventory failure: {change:?}"
    );
    remove_diff_fixture_dir(root);
}

#[test]
fn diff_policy_changes_keep_present_added_evidence_as_improvement() {
    let root = diff_fixture_dir();
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create docs dir: {err}")));
    fs::write(root.join("docs/present.md"), "review notes")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write evidence: {err}")));
    let mut base_entry = entry("allow-panic", FindingKind::Panic);
    base_entry.evidence.clear();
    let base = config_with(base_entry);
    let mut head_entry = entry("allow-panic", FindingKind::Panic);
    head_entry.evidence = vec!["doc:docs/present.md".to_string()];
    let head = config_with(head_entry);
    let mut changes = policy_changes_for_diff(Some(base), &head, None)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy diff: {err}")));

    promote_broken_added_local_reference_policy_changes(&root, None, &head, &mut changes)
        .unwrap_or_else(|err| std::panic::panic_any(format!("promote evidence changes: {err}")));

    let change = changes
        .iter()
        .find(|change| change.kind == allow_diff::PolicyChangeKind::EvidenceAdded)
        .unwrap_or_else(|| std::panic::panic_any("evidence addition should be reported"));
    assert_eq!(
        change.severity,
        allow_diff::PolicyChangeSeverity::Improvement
    );
    assert!(
        change.message.contains("evidence added"),
        "present local evidence should stay improvement: {change:?}"
    );
    remove_diff_fixture_dir(root);
}

#[test]
fn diff_policy_changes_explain_added_link_outside_compared_inventory() {
    let root = diff_fixture_dir();
    let mut base_entry = entry("allow-panic", FindingKind::Panic);
    base_entry.links.clear();
    let base = config_with(base_entry);
    let mut head_entry = entry("allow-panic", FindingKind::Panic);
    head_entry.links = vec!["doc:docs/untracked-link.md".to_string()];
    let head = config_with(head_entry);
    let mut changes = policy_changes_for_diff(Some(base), &head, None)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy diff: {err}")));
    let compared_files = std::collections::BTreeSet::new();

    promote_broken_added_local_reference_policy_changes(
        &root,
        Some(&compared_files),
        &head,
        &mut changes,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("promote link changes: {err}")));

    let change = changes
        .iter()
        .find(|change| change.kind == allow_diff::PolicyChangeKind::LinkAdded)
        .unwrap_or_else(|| std::panic::panic_any("link addition should be reported"));
    assert_eq!(change.severity, allow_diff::PolicyChangeSeverity::Fail);
    assert!(
        change
            .message
            .contains("local link added outside compared source-tree inventory"),
        "message should explain source-tree inventory failure: {change:?}"
    );
    remove_diff_fixture_dir(root);
}

fn config_with(entry: AllowEntry) -> AllowConfig {
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry);
    cfg
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

fn diff_fixture_dir() -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("cargo-allow-diff-{}-{stamp}", std::process::id()));
    remove_diff_fixture_dir(dir.clone());
    fs::create_dir_all(&dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create fixture dir: {err}")));
    dir
}

fn remove_diff_fixture_dir(path: PathBuf) {
    match fs::remove_dir_all(&path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => std::panic::panic_any(format!("remove diff fixture {}: {err}", path.display())),
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
        _ => "unknown",
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
        _ => "unknown",
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
        _ => "unknown/unknown",
    }
}
