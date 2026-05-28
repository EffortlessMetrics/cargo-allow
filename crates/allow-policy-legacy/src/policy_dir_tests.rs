use super::*;
use crate::test_support::*;
use std::fs;
use std::path::Path;

#[test]
fn migrates_legacy_policy_directory_to_one_config() {
    let dir = fixture_dir();
    fs::write(
        dir.join("process-allowlist.toml"),
        process_policy_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("process fixture write: {err}")));
    fs::write(
        dir.join("network-allowlist.toml"),
        network_policy_fixture_text(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("network fixture write: {err}")));

    let cfg = load_legacy_policy_dir(&dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy directory migrates: {err}")));

    assert_eq!(cfg.policy, "cargo-allow");
    assert_eq!(cfg.owner.as_deref(), Some("EffortlessMetrics"));
    assert_eq!(cfg.allow.len(), 4);
    assert!(
        cfg.allow
            .iter()
            .any(|entry| entry.family.as_deref() == Some("process_spawn"))
    );
    assert!(
        cfg.allow
            .iter()
            .any(|entry| entry.family.as_deref() == Some("network_destination"))
    );
}

#[test]
fn policy_directory_can_expand_non_rust_globs_with_findings() {
    let dir = fixture_dir();
    fs::write(dir.join("non-rust-allowlist.toml"), policy_fixture_text())
        .unwrap_or_else(|err| std::panic::panic_any(format!("non-rust fixture write: {err}")));
    let findings = vec![finding(".github/workflows/ci.yml", "tracked_file")];

    let cfg =
        load_legacy_policy_dir_with_non_rust_findings(&dir, &findings).unwrap_or_else(|err| {
            std::panic::panic_any(format!("policy directory with findings migrates: {err}"))
        });

    assert_eq!(cfg.allow.len(), 1);
    let entry = cfg
        .allow
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected expanded non-rust entry"));
    assert_eq!(entry.id, "non-rust-github-workflows--0001");
    assert_eq!(
        entry.path.as_deref(),
        Some(Path::new(".github/workflows/ci.yml"))
    );
    assert_eq!(entry.links, vec!["legacy-policy:non-rust-github-workflows"]);
}

#[test]
fn legacy_policy_directory_requires_supported_files() {
    let dir = fixture_dir();
    let err = load_legacy_policy_dir(&dir).expect_err("empty policy directory should not migrate");
    assert!(
        err.to_string()
            .contains("contains no supported legacy policy files")
    );
}
