use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn unique_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    root
}

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("git {args:?}: {err}")));
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
/// #4594: the main check path must fail no-new on a partial inventory the
/// same way the staged path does — a worktree-deleted tracked file makes
/// the scan incomplete, and a scan that silently missed tracked files must
/// not read as a green pass (the staged path already fails this exact
/// condition).
#[test]
fn check_no_new_fails_on_partial_inventory_from_deleted_tracked_file() {
    let root = unique_root("check-partial-inventory");
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::write(
        root.join("policy/allow.toml"),
        r#"schema_version = "0.1"
policy = "cargo-allow"

[[allow]]
id = "allow-policy"
kind = "non_rust_file"
family = "configuration"
path = "policy/allow.toml"
owner = "core"
classification = "fixture"
reason = "self-receipt for the fixture policy"
evidence = ["doc:policy/allow.toml"]
created = "2026-05-09"
review_after = "2027-08-01"

[allow.selector]
ast_kind = "tracked_file"
symbol = "policy/allow.toml"
target_fingerprint = "toml"
glob = "policy/allow.toml"
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    fs::write(root.join("src/lib.rs"), "pub fn ok() {}\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));

    git(&root, &["init", "-q"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "fixture"]);

    // Baseline: complete inventory, gate passes.
    let baseline = Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .arg("check")
        .arg("--mode")
        .arg("no-new")
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run baseline check: {err}")));
    assert!(
        baseline.status.success(),
        "a complete receipted fixture must pass: {}",
        String::from_utf8_lossy(&baseline.stderr)
    );

    // Delete a tracked source file: the git_tracked inventory can no longer
    // see it, so completeness drops to partial.
    fs::remove_file(root.join("src/lib.rs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("delete source: {err}")));

    let output = Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .arg("check")
        .arg("--mode")
        .arg("no-new")
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run check: {err}")));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !output.status.success(),
        "a partial-inventory no-new run must fail the gate: {stdout}"
    );
    assert!(
        stdout.contains("partial"),
        "the run must disclose the partial inventory: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// #4646: enforcing-mode completeness matrix.
//
// Every enforcing check mode (no-new, strict, release) must fail whenever the
// selected input was omitted or could not be parsed under the selected
// scanner, while evaluated audit results stay informational with truthful
// completeness. The fixtures drive the real CLI producer end to end and
// consume the JSON report and receipt, so exit code, report `failed`, and
// receipt status/`failed` must agree.
// ---------------------------------------------------------------------------

const FIXTURE_POLICY: &str = r#"schema_version = "0.1"
policy = "cargo-allow"

[[allow]]
id = "allow-policy"
kind = "non_rust_file"
family = "configuration"
path = "policy/allow.toml"
owner = "core"
classification = "fixture"
reason = "self-receipt for the fixture policy"
evidence = ["doc:policy/allow.toml"]
created = "2026-05-09"
review_after = "2027-08-01"

[allow.selector]
ast_kind = "tracked_file"
symbol = "policy/allow.toml"
target_fingerprint = "toml"
glob = "policy/allow.toml"
"#;

fn write_fixture_policy(root: &Path) {
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::write(root.join("policy/allow.toml"), FIXTURE_POLICY)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
}

struct CheckRun {
    exit: i32,
    report: serde_json::Value,
    receipt: serde_json::Value,
}

fn run_check(root: &Path, mode: &str, staged: bool, kind: Option<&str>) -> CheckRun {
    let receipt = root.join("target/cargo-allow/receipt.json");
    let _ = fs::remove_file(&receipt);
    let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-allow"));
    command.arg("check").arg("--mode").arg(mode);
    command.arg("--root").arg(root);
    if let Some(kind) = kind {
        command.arg("--kind").arg(kind);
    }
    command.arg("--format").arg("json");
    command.arg("--receipt").arg(&receipt);
    // Staged evaluation rejects `--persistent-cache off`; the worktree route
    // uses it so runs stay hermetic.
    if staged {
        command.arg("--staged").arg("--phase").arg("precommit");
    } else {
        command.arg("--persistent-cache").arg("off");
    }
    let output = command
        .current_dir(root)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run check {mode}: {err}")));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let report: serde_json::Value = if stdout.trim().is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_str(stdout.trim()).unwrap_or_else(|err| {
            std::panic::panic_any(format!("parse check report: {err}: {stdout}"))
        })
    };
    let receipt_text = fs::read_to_string(&receipt)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read receipt: {err}")));
    let receipt: serde_json::Value = serde_json::from_str(&receipt_text)
        .unwrap_or_else(|err| std::panic::panic_any(format!("parse receipt: {err}")));
    CheckRun {
        exit: output.status.code().unwrap_or(-1),
        report,
        receipt,
    }
}

fn report_completeness(run: &CheckRun) -> Option<&str> {
    run.report
        .pointer("/inventory/completeness")
        .and_then(serde_json::Value::as_str)
}

/// Assert exit code, report `failed`, and receipt status/`failed` agree.
fn assert_verdict_agreement(run: &CheckRun, label: &str, expected_failed: bool) {
    assert_eq!(
        run.exit,
        if expected_failed { 1 } else { 0 },
        "{label}: exit code disagrees with the expected verdict"
    );
    assert_eq!(
        run.report
            .get("failed")
            .and_then(serde_json::Value::as_bool),
        Some(expected_failed),
        "{label}: report failed flag"
    );
    assert_eq!(
        run.receipt
            .get("failed")
            .and_then(serde_json::Value::as_bool),
        Some(expected_failed),
        "{label}: receipt failed flag"
    );
    assert_eq!(
        run.receipt
            .get("status")
            .and_then(serde_json::Value::as_str),
        Some(if expected_failed { "failed" } else { "passed" }),
        "{label}: receipt status"
    );
}

/// #4646 mode × completeness table (tracked worktree route): a non-UTF8,
/// parse-error, or worktree-deleted tracked source must fail every enforcing
/// mode — not only no-new — while audit stays informational. The complete
/// control passes every mode.
#[test]
fn incomplete_tracked_worktree_inputs_fail_every_enforcing_mode() {
    let cases: [(&str, &[u8], bool); 4] = [
        ("complete-ctrl", b"pub fn ok() {}\n", false),
        ("non-utf8", b"\xff", true),
        ("parse-error", b"fn broken(\n", true),
        // Deleted from the worktree after the baseline commit, so the git
        // inventory discloses it as deleted-tracked.
        ("deleted-tracked", b"pub fn ok() {}\n", true),
    ];
    for (label, content, incomplete) in cases {
        let root = unique_root(&format!("check-enforcing-{label}"));
        fs::create_dir_all(root.join("src"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
        write_fixture_policy(&root);
        fs::write(root.join("src/lib.rs"), content)
            .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));
        git(&root, &["init", "-q"]);
        git(
            &root,
            &["config", "user.email", "cargo-allow@example.invalid"],
        );
        git(&root, &["config", "user.name", "cargo-allow test"]);
        git(&root, &["add", "."]);
        git(&root, &["commit", "-m", "fixture"]);
        if label == "deleted-tracked" {
            fs::remove_file(root.join("src/lib.rs"))
                .unwrap_or_else(|err| std::panic::panic_any(format!("delete source: {err}")));
        }
        for mode in ["audit", "no-new", "strict", "release"] {
            let expected_failed = incomplete && mode != "audit";
            let run = run_check(&root, mode, false, None);
            assert_verdict_agreement(&run, &format!("{label}/{mode}"), expected_failed);
            if expected_failed {
                assert_eq!(
                    report_completeness(&run),
                    Some("partial"),
                    "{label}/{mode}: the failing report must disclose the partial inventory"
                );
            }
        }
        let _ = fs::remove_dir_all(&root);
    }
}

/// #4646 staged route: a staged source with a tree-sitter parse error fails
/// every enforcing mode while audit stays informational. The invalid-UTF8
/// staged case intentionally stays an early loader refusal (E0005_SCAN) and
/// is not exercised here; that refusal must be preserved, not converted into
/// a gate failure.
#[test]
fn incomplete_staged_parse_error_fails_every_enforcing_mode() {
    let root = unique_root("check-enforcing-staged-parse-error");
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
    write_fixture_policy(&root);
    fs::write(root.join("src/lib.rs"), b"fn broken(\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));
    git(&root, &["init", "-q"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "fixture"]);
    for mode in ["audit", "no-new", "strict", "release"] {
        let expected_failed = mode != "audit";
        let run = run_check(&root, mode, true, None);
        assert_verdict_agreement(&run, &format!("staged/{mode}"), expected_failed);
        if expected_failed {
            assert_eq!(
                report_completeness(&run),
                Some("partial"),
                "staged/{mode}: the failing report must disclose the partial inventory"
            );
        }
    }
    let _ = fs::remove_dir_all(&root);
}

/// #4646 non-Git fixture: the configured inventory depth bound is 64. A
/// source file hidden below the cap truncates the filesystem walk, and the
/// recorded omission must fail every enforcing mode even though the hidden
/// finding cannot be emitted. The depth-0/64 controls find the unreceipted
/// panic and fail enforcing modes through the ordinary findings path.
#[test]
fn non_git_depth_capped_fallback_fails_every_enforcing_mode() {
    for (depth, label) in [(0usize, "depth-0"), (64, "depth-64"), (65, "depth-65")] {
        let root = unique_root(&format!("check-enforcing-{label}"));
        write_fixture_policy(&root);
        let mut nested = root.clone();
        for _ in 0..depth {
            nested = nested.join("d");
        }
        fs::create_dir_all(&nested)
            .unwrap_or_else(|err| std::panic::panic_any(format!("create nested dirs: {err}")));
        fs::write(
            nested.join("lib.rs"),
            "fn hidden() { panic!(\"retained\"); }\n",
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));
        for mode in ["audit", "no-new", "strict", "release"] {
            // Audit stays advisory at every depth; enforcing modes fail at
            // every depth — via findings at depth 0/64, via the recorded
            // omission at depth 65.
            let expected_failed = mode != "audit";
            let run = run_check(&root, mode, false, Some("panic"));
            assert_verdict_agreement(&run, &format!("{label}/{mode}"), expected_failed);
            if depth == 65 {
                assert_eq!(
                    report_completeness(&run),
                    Some("partial"),
                    "{label}/{mode}: the truncated fallback must be disclosed as partial"
                );
                // #4646 review: the omission count itself must reach both the
                // report JSON and the receipt inventory, so consumers read the
                // count and not just the `partial` label.
                assert_eq!(
                    run.report
                        .pointer("/inventory/inventory_skipped_paths")
                        .and_then(serde_json::Value::as_u64),
                    Some(1),
                    "{label}/{mode}: report inventory must carry the omission count"
                );
                assert_eq!(
                    run.receipt
                        .pointer("/inventory/inventory_skipped_paths")
                        .and_then(serde_json::Value::as_u64),
                    Some(1),
                    "{label}/{mode}: receipt inventory must carry the omission count"
                );
            }
        }
        let _ = fs::remove_dir_all(&root);
    }
}

/// #4646 review: the walker records skips before the configured ignore globs
/// are applied, so a truncation inside a policy-ignored subtree omits no
/// selected input. With `workspace.ignored = ["vendor/**"]`, a depth-capped
/// `vendor/...` walk is scope, not an omission — every mode passes, and the
/// artifacts carry no `partial` marker or omission count.
#[test]
fn ignored_subtree_truncation_is_scope_not_omission() {
    let root = unique_root("check-enforcing-ignored-truncation");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::write(
        root.join("policy/allow.toml"),
        r#"schema_version = "0.1"
policy = "cargo-allow"

[workspace]
ignored = [".git/**", "target/**", "vendor/**"]

[[allow]]
id = "allow-policy"
kind = "non_rust_file"
family = "configuration"
path = "policy/allow.toml"
owner = "core"
classification = "fixture"
reason = "self-receipt for the fixture policy"
evidence = ["doc:policy/allow.toml"]
created = "2026-05-09"
review_after = "2027-08-01"

[allow.selector]
ast_kind = "tracked_file"
symbol = "policy/allow.toml"
target_fingerprint = "toml"
glob = "policy/allow.toml"
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write policy: {err}")));
    let mut nested = root.join("vendor").join("private");
    for _ in 0..65 {
        nested = nested.join("d");
    }
    fs::create_dir_all(&nested)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create nested dirs: {err}")));
    fs::write(
        nested.join("lib.rs"),
        "fn hidden() { panic!(\"ignored subtree\"); }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));

    for mode in ["audit", "no-new", "strict", "release"] {
        let run = run_check(&root, mode, false, Some("panic"));
        assert_verdict_agreement(&run, &format!("ignored-truncation/{mode}"), false);
        assert_ne!(
            report_completeness(&run),
            Some("partial"),
            "ignored-truncation/{mode}: ignored-subtree scope must not read as partial"
        );
        assert_eq!(
            run.report
                .pointer("/inventory/inventory_skipped_paths")
                .and_then(serde_json::Value::as_u64),
            None,
            "ignored-truncation/{mode}: no omission count may be emitted for ignored scope"
        );
    }
    let _ = fs::remove_dir_all(&root);
}

/// #4646: a complete filesystem fallback with no findings is a supported
/// source-only route and must pass every mode — fallback provenance alone is
/// not a defect.
#[test]
fn complete_filesystem_fallback_with_no_findings_passes_every_mode() {
    let root = unique_root("check-enforcing-fallback-complete");
    write_fixture_policy(&root);
    fs::write(root.join("lib.rs"), "fn probe() {}\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));
    for mode in ["audit", "no-new", "strict", "release"] {
        let run = run_check(&root, mode, false, Some("panic"));
        assert_verdict_agreement(&run, &format!("fallback-complete/{mode}"), false);
        assert_eq!(
            run.report
                .pointer("/inventory/inventory_skipped_paths")
                .and_then(serde_json::Value::as_u64),
            None,
            "fallback-complete/{mode}: a complete fallback emits no omission count"
        );
    }
    let _ = fs::remove_dir_all(&root);
}
