use super::*;

#[test]
fn policy_changes_from_git_reports_additions_when_base_policy_is_missing() {
    let root = temp_root("policy-base-missing");
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(
        &root,
        &["commit", "--allow-empty", "-m", "base without policy"],
    );
    let mut head = AllowConfig::empty();
    head.allow.push(entry("allow-added"));

    let changes = policy_changes_from_git(&root, "HEAD", "policy/allow.toml", &head)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy diff: {err}")));

    assert!(
        changes.iter().any(|change| {
            change.allow_id == "allow-added"
                && change.kind == PolicyChangeKind::AddedAllow
                && change.severity == PolicyChangeSeverity::Review
        }),
        "missing base policy should be treated as an empty ledger: {changes:?}"
    );
    fs::remove_dir_all(root).unwrap_or_else(|err| std::panic::panic_any(format!("cleanup: {err}")));
}
