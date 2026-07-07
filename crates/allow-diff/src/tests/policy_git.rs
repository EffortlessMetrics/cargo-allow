use super::*;
use allow_policy::render_policy;

fn init_git_repo(root: &PathBuf) {
    git(root, &["init"]);
    git(
        root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(root, &["config", "user.name", "cargo-allow test"]);
}

fn commit_all(root: &PathBuf, message: &str) {
    git(root, &["add", "."]);
    git(root, &["commit", "--allow-empty", "-m", message]);
}

fn write_policy(root: &Path, policy_path: &str, cfg: &AllowConfig) {
    let path = root.join(policy_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|err| std::panic::panic_any(format!("policy parent: {err}")));
    }
    fs::write(&path, render_policy(cfg))
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy file: {err}")));
}

#[test]
fn policy_changes_from_git_reports_additions_when_base_policy_is_missing() {
    let root = temp_root("policy-base-missing");
    init_git_repo(&root);
    commit_all(&root, "base without policy");
    let mut head = AllowConfig::empty();
    head.allow.push(entry("allow-added"));

    let changes = policy_changes_from_git(&root, "HEAD", "policy/allow.toml", &head)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy diff: {err}")));

    assert_eq!(changes.len(), 1);
    let change = changes
        .iter()
        .find(|change| change.allow_id == "allow-added")
        .unwrap_or_else(|| std::panic::panic_any("added allow should be reported"));
    assert_eq!(change.kind, PolicyChangeKind::AddedAllow);
    assert_eq!(change.severity, PolicyChangeSeverity::Review);
    assert!(
        change.message.contains("allow-added"),
        "added allow message should name the new entry: {change:?}"
    );
    fs::remove_dir_all(root).unwrap_or_else(|err| std::panic::panic_any(format!("cleanup: {err}")));
}

#[test]
fn policy_config_at_revision_returns_none_for_missing_policy() {
    let root = temp_root("policy-revision-missing");
    init_git_repo(&root);
    commit_all(&root, "base without policy");

    let cfg = policy_config_at_revision(&root, "HEAD", "policy/allow.toml")
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy lookup: {err}")));

    assert!(cfg.is_none(), "missing policy at revision should be None");
    fs::remove_dir_all(root).unwrap_or_else(|err| std::panic::panic_any(format!("cleanup: {err}")));
}

#[test]
fn policy_config_at_revision_parses_committed_policy_not_worktree() {
    let root = temp_root("policy-revision-present");
    init_git_repo(&root);
    write_policy(
        &root,
        "policy/allow.toml",
        &config_with(entry("allow-base")),
    );
    commit_all(&root, "base policy");
    write_policy(
        &root,
        "policy/allow.toml",
        &config_with(entry("allow-worktree")),
    );

    let cfg = policy_config_at_revision(&root, "HEAD", "policy/allow.toml")
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy lookup: {err}")))
        .unwrap_or_else(|| std::panic::panic_any("committed policy should exist"));
    let entry = cfg
        .allow
        .iter()
        .find(|entry| entry.id == "allow-base")
        .unwrap_or_else(|| std::panic::panic_any("committed allow entry should parse"));

    assert_eq!(entry.kind, FindingKind::Panic);
    assert_eq!(entry.selector.callee.as_deref(), Some("unwrap"));
    assert!(
        cfg.allow.iter().all(|entry| entry.id != "allow-worktree"),
        "revision lookup should ignore uncommitted worktree policy"
    );
    fs::remove_dir_all(root).unwrap_or_else(|err| std::panic::panic_any(format!("cleanup: {err}")));
}

#[test]
fn policy_config_at_revision_preserves_reportable_local_evidence_paths() {
    let root = temp_root("policy-revision-reportable-evidence");
    init_git_repo(&root);
    let path = root.join("policy/allow.toml");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|err| std::panic::panic_any(format!("policy parent: {err}")));
    }
    fs::write(
        &path,
        r#"
policy = "cargo-allow"

[[allow]]
id = "allow-reportable"
kind = "panic"
path = "src/lib.rs"
owner = "core"
classification = "reviewed"
reason = "fixture"
evidence = ["doc:../outside.md"]
expires = "2026-08-01"
[allow.selector]
ast_kind = "method_call"
callee = "unwrap"
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy file: {err}")));
    commit_all(&root, "policy with reportable local evidence");

    let cfg = policy_config_at_revision(&root, "HEAD", "policy/allow.toml")
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy lookup: {err}")))
        .unwrap_or_else(|| std::panic::panic_any("committed policy should exist"));
    let entry = cfg
        .allow
        .iter()
        .find(|entry| entry.id == "allow-reportable")
        .unwrap_or_else(|| std::panic::panic_any("committed allow entry should parse"));

    assert_eq!(entry.evidence, vec!["doc:../outside.md".to_string()]);
    fs::remove_dir_all(root).unwrap_or_else(|err| std::panic::panic_any(format!("cleanup: {err}")));
}
