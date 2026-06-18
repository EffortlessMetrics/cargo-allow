use super::*;
use std::fs;
use std::path::PathBuf;

#[test]
fn semantic_selector_import_preserves_receiver_container_and_callee() {
    let policy_path = stage_semantic_selector_fixture();
    let cfg = load_legacy_or_canonical(&policy_path).unwrap_or_else(|err| {
        std::panic::panic_any(format!("semantic selector fixture migration: {err}"))
    });

    assert_eq!(cfg.policy, "cargo-allow");

    let entry = cfg
        .allow
        .iter()
        .find(|entry| entry.id == "fixture-semantic-unwrap")
        .unwrap_or_else(|| std::panic::panic_any("expected fixture-semantic-unwrap entry"));

    assert_eq!(entry.kind, allow_core::FindingKind::Panic);
    assert_eq!(entry.family.as_deref(), Some("unwrap"));
    assert_eq!(entry.selector.ast_kind.as_deref(), Some("method_call"));
    assert_eq!(entry.selector.container.as_deref(), Some("load"));
    assert_eq!(entry.selector.callee.as_deref(), Some("unwrap"));
    assert_eq!(
        entry.selector.receiver_fingerprint.as_deref(),
        Some("optional_value")
    );
    assert!(
        entry.selector.has_structural_identity(),
        "semantic selector import should not fall back to path-only identity"
    );
}

#[test]
fn semantic_selector_import_preserves_nested_clippy_target_fingerprint() {
    let dir = crate::test_support::fixture_dir();
    let source = migration_fixture_path("lint-exception.toml");
    let text = fs::read_to_string(&source).unwrap_or_else(|err| {
        std::panic::panic_any(format!("read lint-exception fixture: {err}"))
    });
    let path = dir.join("clippy-exceptions.toml");
    fs::write(&path, text).unwrap_or_else(|err| {
        std::panic::panic_any(format!("stage lint-exception fixture: {err}"))
    });

    let cfg = load_clippy_exceptions_compat_config(&path).unwrap_or_else(|err| {
        std::panic::panic_any(format!("lint-exception semantic selector migration: {err}"))
    });

    let entry = cfg
        .allow
        .iter()
        .find(|entry| entry.id == "fixture-clippy")
        .unwrap_or_else(|| std::panic::panic_any("expected fixture-clippy entry"));

    assert_eq!(
        entry.selector.target_fingerprint.as_deref(),
        Some("policy:fixture-clippy")
    );
    assert!(
        entry.selector.has_structural_identity(),
        "lint selector target should import from nested allow.selector table"
    );
}

fn migration_fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/migration")
}

fn migration_fixture_path(fixture_file: &str) -> PathBuf {
    migration_fixture_root().join(fixture_file)
}

fn stage_semantic_selector_fixture() -> PathBuf {
    let dir = crate::test_support::fixture_dir();
    let source = migration_fixture_path("no-panic-allowlist-semantic-selectors.toml");
    let text = fs::read_to_string(&source).unwrap_or_else(|err| {
        std::panic::panic_any(format!("read semantic selector fixture: {err}"))
    });
    let path = dir.join("no-panic-allowlist.toml");
    fs::write(&path, text).unwrap_or_else(|err| {
        std::panic::panic_any(format!("stage semantic selector fixture: {err}"))
    });
    path
}
