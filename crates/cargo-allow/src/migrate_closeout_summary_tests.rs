use std::fs;
use std::path::{Path, PathBuf};

use allow_policy_legacy::load_legacy_or_canonical;

use super::migrate_render::render_migrate_summary_json;
use super::migrate_types::MigrateContext;

#[test]
fn migrate_closeout_summary_panic_baseline_with_evidence_preserves_legacy_proof() {
    let root = stage_panic_fixture("panic-baseline.toml", "no-panic-baseline.toml");
    let policy_path = root.join("no-panic-baseline.toml");
    let cfg = load_legacy_or_canonical(&policy_path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("load panic baseline: {err}")));
    let context = migrate_context(
        &root,
        policy_path.display().to_string(),
        vec!["no-panic-baseline.toml".to_string()],
        vec!["panic"],
    );

    let json = render_migrate_summary_json(&cfg, &context, Path::new("policy/allow.toml"), false);
    let closeout = closeout_value(&json);

    assert_eq!(closeout_preserved_u64(&closeout, "allow_entries"), 1);
    assert_eq!(
        closeout_preserved_u64(&closeout, "entries_with_evidence"),
        1
    );
    assert!(closeout_preserved_u64(&closeout, "evidence_entries") >= 2);
    assert_eq!(closeout_pointer_u64(&closeout, "/baseline_debt/entries"), 1);
    assert_eq!(closeout_preserved_u64(&closeout, "reviewed_entries"), 0);
    assert!(
        closeout_blockers(&closeout).contains(&"baseline_debt".to_string()),
        "panic baseline entries remain baseline debt even with legacy evidence"
    );
    assert_legacy_source(&closeout, "no-panic-baseline.toml", "panic", "blocked");
    assert!(
        json.contains("\"item_kind\": \"baseline_debt\""),
        "panic baseline with evidence should still route baseline-debt closeout"
    );
    remove_dir(&root);
}

#[test]
fn migrate_closeout_summary_panic_baseline_without_evidence_routes_baseline_debt() {
    let root = stage_panic_fixture("panic-baseline-no-evidence.toml", "no-panic-baseline.toml");
    let policy_path = root.join("no-panic-baseline.toml");
    let cfg = load_legacy_or_canonical(&policy_path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("load panic baseline: {err}")));
    let context = migrate_context(
        &root,
        policy_path.display().to_string(),
        vec!["no-panic-baseline.toml".to_string()],
        vec!["panic"],
    );

    let json = render_migrate_summary_json(&cfg, &context, Path::new("policy/allow.toml"), false);
    let closeout = closeout_value(&json);

    assert_eq!(closeout_pointer_u64(&closeout, "/baseline_debt/entries"), 1);
    assert!(
        closeout_blockers(&closeout).contains(&"baseline_debt".to_string()),
        "missing legacy evidence should keep baseline debt visible"
    );
    assert!(
        closeout_blockers(&closeout).contains(&"weak_evidence_reference".to_string()),
        "generated baseline markers should route weak-evidence closeout"
    );
    assert_eq!(
        closeout_pointer_bool(&closeout, "/legacy_retirement/ready"),
        Some(false)
    );
    assert_legacy_source(&closeout, "no-panic-baseline.toml", "panic", "blocked");
    assert!(
        json.contains("\"item_kind\": \"baseline_debt\""),
        "panic baseline without evidence should route baseline-debt closeout queue"
    );
    assert!(
        json.contains("\"signal\": \"no_new_gate\""),
        "closeout should end with the no-new guard after routed queues"
    );
    remove_dir(&root);
}

#[test]
fn migrate_closeout_summary_lint_exception_minimal_routes_baseline_debt() {
    let root = stage_lane_fixture("lint-exception-minimal.toml", "clippy-exceptions.toml");
    let policy_path = root.join("clippy-exceptions.toml");
    let cfg = load_legacy_or_canonical(&policy_path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("load lint exception: {err}")));
    let context = migrate_context(
        &root,
        policy_path.display().to_string(),
        vec!["clippy-exceptions.toml".to_string()],
        vec!["lint-exception"],
    );

    let json = render_migrate_summary_json(&cfg, &context, Path::new("policy/allow.toml"), false);
    let closeout = closeout_value(&json);

    assert_eq!(closeout_pointer_u64(&closeout, "/baseline_debt/entries"), 1);
    assert!(
        closeout_blockers(&closeout).contains(&"baseline_debt".to_string()),
        "minimal lint entries should remain baseline debt"
    );
    assert_legacy_source(
        &closeout,
        "clippy-exceptions.toml",
        "lint-exception",
        "blocked",
    );
    assert!(
        json.contains("\"label\": \"baseline debt entries\""),
        "lint-exception closeout should use generic baseline-debt queue label"
    );
    assert!(
        json.contains("\"signal\": \"no_new_gate\""),
        "closeout should end with the no-new guard after routed queues"
    );
    remove_dir(&root);
}

#[test]
fn migrate_closeout_summary_unsafe_without_evidence_routes_weak_evidence() {
    let root = stage_lane_fixture("unsafe-no-evidence.toml", "unsafe-allowlist.toml");
    let policy_path = root.join("unsafe-allowlist.toml");
    let cfg = load_legacy_or_canonical(&policy_path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("load unsafe allowlist: {err}")));
    let context = migrate_context(
        &root,
        policy_path.display().to_string(),
        vec!["unsafe-allowlist.toml".to_string()],
        vec!["unsafe"],
    );

    let json = render_migrate_summary_json(&cfg, &context, Path::new("policy/allow.toml"), false);
    let closeout = closeout_value(&json);

    assert!(
        closeout_blockers(&closeout).contains(&"weak_evidence_reference".to_string()),
        "unsafe lane without legacy evidence should route weak-evidence closeout"
    );
    assert_legacy_source(&closeout, "unsafe-allowlist.toml", "unsafe", "blocked");
    assert!(
        json.contains("\"item_kind\": \"weak_evidence_reference\""),
        "unsafe without evidence should route weak-evidence closeout queue"
    );
    assert!(
        json.contains("\"signal\": \"no_new_gate\""),
        "closeout should end with the no-new guard after routed queues"
    );
    remove_dir(&root);
}

fn closeout_value(json: &str) -> serde_json::Value {
    let value: serde_json::Value = serde_json::from_str(json)
        .unwrap_or_else(|err| std::panic::panic_any(format!("parse migrate json: {err}")));
    value
        .get("closeout")
        .cloned()
        .unwrap_or_else(|| std::panic::panic_any("migrate summary missing closeout section"))
}

fn closeout_preserved_u64(closeout: &serde_json::Value, field: &str) -> u64 {
    closeout
        .pointer(&format!("/preserved/{field}"))
        .and_then(|value| value.as_u64())
        .unwrap_or_else(|| std::panic::panic_any(format!("closeout preserved.{field} missing")))
}

fn closeout_pointer_u64(closeout: &serde_json::Value, pointer: &str) -> u64 {
    closeout
        .pointer(pointer)
        .and_then(|value| value.as_u64())
        .unwrap_or_else(|| std::panic::panic_any(format!("closeout{pointer} missing")))
}

fn closeout_pointer_bool(closeout: &serde_json::Value, pointer: &str) -> Option<bool> {
    closeout.pointer(pointer).and_then(|value| value.as_bool())
}

fn closeout_blockers(closeout: &serde_json::Value) -> Vec<String> {
    closeout
        .pointer("/legacy_retirement/blockers")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn assert_legacy_source(
    closeout: &serde_json::Value,
    file_name: &str,
    compat_kind: &str,
    status: &str,
) {
    let sources = closeout
        .pointer("/legacy_retirement/sources")
        .and_then(|value| value.as_array())
        .unwrap_or_else(|| std::panic::panic_any("closeout legacy sources missing"));
    assert!(
        sources.iter().any(|source| {
            source.get("file_name").and_then(|value| value.as_str()) == Some(file_name)
                && source.get("compat_kind").and_then(|value| value.as_str()) == Some(compat_kind)
                && source.get("status").and_then(|value| value.as_str()) == Some(status)
        }),
        "closeout should name the migrated legacy source file"
    );
}

fn migrate_context(
    root: &Path,
    input_path: String,
    legacy_source_files: Vec<String>,
    legacy_compat_kinds: Vec<&'static str>,
) -> MigrateContext {
    let (signal, label) = allow_policy_legacy::baseline_debt_projection(&legacy_compat_kinds);
    MigrateContext {
        inventory_source: "filesystem_fallback".to_string(),
        source_tree_root: Some(root.display().to_string()),
        inventory_files: Some(1),
        inventory_completeness: Some("complete".to_string()),
        repository_identity: Some("test".to_string()),
        input_kind: "from".to_string(),
        input_path,
        legacy_source_files,
        baseline_debt_projection: allow_report::MigrateBaselineDebtProjection { signal, label },
        legacy_compat_kinds,
    }
}

fn stage_panic_fixture(fixture_file: &str, legacy_filename: &str) -> PathBuf {
    stage_lane_fixture(fixture_file, legacy_filename)
}

fn stage_lane_fixture(fixture_file: &str, legacy_filename: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-migrate-closeout-{}-{}",
        fixture_file.replace('.', "-"),
        std::process::id()
    ));
    fs::create_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create fixture dir: {err}")));
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/migration")
        .join(fixture_file);
    let text = fs::read_to_string(&fixture)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read fixture {fixture_file}: {err}")));
    fs::write(root.join(legacy_filename), text)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write legacy policy: {err}")));
    root
}

fn remove_dir(path: &Path) {
    let _ = fs::remove_dir_all(path);
}
