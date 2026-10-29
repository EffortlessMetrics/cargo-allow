use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn unique_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    root
}

fn remove_root(root: &Path) {
    let _ = fs::remove_dir_all(root);
}

/// A bare fixture root: no policy, one tracked file. The #4632 guard fires
/// before any scan, so the commands never reach policy discovery.
fn write_guard_fixture(label: &str) -> PathBuf {
    let root = unique_root(label);
    fs::create_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create fixture dir: {err}")));
    fs::write(root.join("readme.md"), "# fixture\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write fixture file: {err}")));
    root
}

fn cargo_allow_in(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow {args:?}: {err}")))
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// #4632: a compat-only `--kind` filtered into a non-compat gate used to be
/// accepted and produced a permanently green gate that scanned nothing for
/// that kind. Every compat-only kind must instead fail closed with a typed
/// usage error (exit 2) naming the kind and the compat lane.
#[test]
fn non_compat_check_rejects_every_compat_only_kind() {
    let root = write_guard_fixture("compat-only-guard-check");

    for kind in [
        "workflow",
        "executable",
        "process",
        "network",
        "dependency-surface",
    ] {
        let output = cargo_allow_in(&root, &["check", "--kind", kind]);
        assert_eq!(
            output.status.code(),
            Some(2),
            "check --kind {kind} must exit 2 (typed usage error): {}",
            stderr_of(&output)
        );
        let stderr = stderr_of(&output);
        assert!(
            stderr.contains(&format!("--kind {kind}")),
            "error must name the kind: {stderr}"
        );
        assert!(
            stderr.contains("--compat lane for check"),
            "error must name the compat lane: {stderr}"
        );
        assert!(
            stderr.contains("permanently green"),
            "error must state the green-gate hazard: {stderr}"
        );
    }

    remove_root(&root);
}

/// The four other guarded commands reject the compat-only kinds the same way;
/// `audit` names its own lane, and `diff`/`why` (no compat lane at all) point
/// at the compat check/audit commands.
#[test]
fn other_non_compat_lanes_reject_compat_only_kinds() {
    let root = write_guard_fixture("compat-only-guard-lanes");

    let output = cargo_allow_in(&root, &["audit", "--kind", "workflow"]);
    assert_eq!(output.status.code(), Some(2), "{}", stderr_of(&output));
    assert!(
        stderr_of(&output).contains("--compat lane for audit"),
        "{}",
        stderr_of(&output)
    );

    let output = cargo_allow_in(&root, &["diff", "--kind", "workflow"]);
    assert_eq!(output.status.code(), Some(2), "{}", stderr_of(&output));
    assert!(
        stderr_of(&output).contains("--compat lane for diff")
            && stderr_of(&output).contains("check --compat --kind workflow"),
        "{}",
        stderr_of(&output)
    );

    let output = cargo_allow_in(
        &root,
        &[
            "why",
            "--kind",
            "workflow",
            "--path",
            "readme.md",
            "--line",
            "1",
        ],
    );
    assert_eq!(output.status.code(), Some(2), "{}", stderr_of(&output));
    assert!(
        stderr_of(&output).contains("--compat lane for why"),
        "{}",
        stderr_of(&output)
    );

    remove_root(&root);
}

/// `--compat --kind workflow` is untouched (#4632): the compat lane still
/// wires the workflow sensor in, and a missing legacy policy surfaces as the
/// loader's E0009 policy-demand error rather than the new usage guard.
#[test]
fn compat_check_kind_workflow_keeps_policy_demand_path() {
    let root = write_guard_fixture("compat-only-guard-compat-path");

    let output = cargo_allow_in(&root, &["check", "--compat", "--kind", "workflow"]);
    assert_ne!(
        output.status.code(),
        Some(2),
        "the compat lane must not hit the usage guard: {}",
        stderr_of(&output)
    );
    let stderr = stderr_of(&output);
    assert!(
        stderr.contains("E0009") && stderr.contains("workflow-allowlist.toml"),
        "the compat policy-demand path must survive: {stderr}"
    );

    remove_root(&root);
}

/// Kinds with a real non-compat sensor keep working; a compat-only kind must
/// not turn their rejections into the new guard's message.
#[test]
fn non_compat_kinds_are_not_intercepted_by_the_guard() {
    let root = write_guard_fixture("compat-only-guard-non-compat-kinds");

    // Without a policy this fails with the policy-discovery error (exit 1),
    // never the compat-lane guard (exit 2).
    let output = cargo_allow_in(&root, &["check", "--kind", "non-rust"]);
    assert_ne!(
        output.status.code(),
        Some(2),
        "check --kind non-rust must not hit the compat guard: {}",
        stderr_of(&output)
    );
    assert!(
        !stderr_of(&output).contains("--compat lane"),
        "{}",
        stderr_of(&output)
    );

    remove_root(&root);
}
