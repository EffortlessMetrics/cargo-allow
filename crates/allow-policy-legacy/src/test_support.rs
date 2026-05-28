use super::*;
use allow_core::Lifecycle;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

pub(super) use crate::test_findings::*;
pub(super) use crate::test_fixture_text::*;

pub(super) fn assert_current_baseline_window(lifecycle: &Lifecycle) {
    let created = lifecycle
        .created
        .as_deref()
        .and_then(SimpleDate::parse)
        .unwrap_or_else(|| std::panic::panic_any("baseline should have valid created date"));
    let expires = lifecycle
        .expires
        .as_deref()
        .and_then(SimpleDate::parse)
        .unwrap_or_else(|| std::panic::panic_any("baseline should have valid expires date"));
    let today = SimpleDate::today_utc_approx();

    assert!(
        today.add_days(-1) <= created && created <= today.add_days(1),
        "baseline created date should track the current UTC day"
    );
    assert_eq!(created.days_until(expires), BASELINE_DEBT_DEFAULT_DAYS);
}

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

pub(super) fn policy_fixture_path() -> PathBuf {
    let path = fixture_dir().join("non-rust-allowlist.toml");
    fs::write(&path, policy_fixture_text())
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
    path
}

pub(super) fn non_rust_policy_with_entry(entry: &str) -> PathBuf {
    let path = fixture_dir().join("non-rust-allowlist.toml");
    let text = format!(
        r#"schema_version = 1
policy = "non-rust-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
{entry}
"#
    );
    fs::write(&path, text)
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
    path
}

pub(super) fn generated_policy_fixture_path() -> PathBuf {
    let path = fixture_dir().join("generated-allowlist.toml");
    fs::write(&path, generated_policy_fixture_text())
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
    path
}

pub(super) fn no_panic_baseline_fixture_path() -> PathBuf {
    let path = fixture_dir().join("no-panic-baseline.toml");
    fs::write(&path, no_panic_baseline_fixture_text())
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
    path
}

pub(super) fn no_panic_allowlist_fixture_path() -> PathBuf {
    let path = fixture_dir().join("no-panic-allowlist.toml");
    fs::write(&path, no_panic_allowlist_fixture_text())
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
    path
}

pub(super) fn clippy_policy_fixture_path() -> PathBuf {
    let path = fixture_dir().join("clippy-exceptions.toml");
    fs::write(&path, clippy_policy_fixture_text())
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
    path
}

pub(super) fn unsafe_policy_fixture_path() -> PathBuf {
    let path = fixture_dir().join("unsafe-allowlist.toml");
    fs::write(&path, unsafe_policy_fixture_text())
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
    path
}

pub(super) fn executable_policy_fixture_path() -> PathBuf {
    let path = fixture_dir().join("executable-allowlist.toml");
    fs::write(&path, executable_policy_fixture_text())
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
    path
}

pub(super) fn workflow_policy_fixture_path() -> PathBuf {
    let path = fixture_dir().join("workflow-allowlist.toml");
    fs::write(&path, workflow_policy_fixture_text())
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
    path
}

pub(super) fn dependency_policy_fixture_path() -> PathBuf {
    let path = fixture_dir().join("dependency-surface-allowlist.toml");
    fs::write(&path, dependency_policy_fixture_text())
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
    path
}

pub(super) fn process_policy_fixture_path() -> PathBuf {
    let path = fixture_dir().join("process-allowlist.toml");
    fs::write(&path, process_policy_fixture_text())
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
    path
}

pub(super) fn malformed_process_policy_fixture_path() -> PathBuf {
    let path = fixture_dir().join("process-allowlist.toml");
    fs::write(
        &path,
        r#"schema_version = 1
policy = "process-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "proc-missing"
binary = "cargo"
argv_shape = ["install", "cargo-deny", "--locked"]
owner = "release/ci"
reason = "Intentionally incomplete fixture."
created = "2026-05-09"
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
    path
}

pub(super) fn network_policy_fixture_path() -> PathBuf {
    let path = fixture_dir().join("network-allowlist.toml");
    fs::write(&path, network_policy_fixture_text())
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
    path
}

pub(super) fn malformed_network_policy_fixture_path() -> PathBuf {
    let path = fixture_dir().join("network-allowlist.toml");
    fs::write(
        &path,
        r#"schema_version = 1
policy = "network-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "net-missing"
destination = "crates.io"
lane = "build"
owner = "release"
reason = "Intentionally incomplete fixture."
created = "2026-05-09"
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("fixture write: {err}")));
    path
}

pub(super) fn generated_fixture_root() -> PathBuf {
    let dir = fixture_dir();
    fs::write(
            dir.join(".gitattributes"),
            "# generated files\npolicy/no-panic-baseline.toml text linguist-generated=true\nREADME.md text\n",
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("gitattributes write: {err}")));
    dir
}

pub(super) fn workflow_fixture_root() -> PathBuf {
    let dir = fixture_dir();
    let workflows = dir.join(".github").join("workflows");
    fs::create_dir_all(&workflows)
        .unwrap_or_else(|err| std::panic::panic_any(format!("workflow dir: {err}")));
    fs::write(
            workflows.join("ci.yml"),
            "name: ci\njobs:\n  test:\n    steps:\n      - uses: actions/checkout@v6.0.2\n      - uses: Swatinem/rust-cache@v2 # cache\n      # - uses: ignored/comment@v1\n",
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("workflow write: {err}")));
    dir
}

pub(super) fn fixture_dir() -> PathBuf {
    let id = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "cargo-allow-policy-legacy-{}-{id}",
        std::process::id()
    ));
    fs::create_dir_all(&dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture dir: {err}")));
    dir
}
