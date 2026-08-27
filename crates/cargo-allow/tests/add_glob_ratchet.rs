//! #2056: `add --glob` pins the in-scope occurrence count as `occurrence_limit`
//! so a broad baseline is a true ratchet floor — the N+1th in-scope occurrence
//! fails `check --mode no-new`.
//!
//! Focused test: inlines its own subprocess helpers (version_output /
//! policy_discovery / check_output_path_containment convention) and does not
//! pull in the shared `tests/support` module.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn cargo_allow_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
}

fn temp_root(label: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root)
        .unwrap_or_else(|err| panic!("create temp root: {err}"));
    root
}

fn remove_temp_root(root: PathBuf) {
    match fs::remove_dir_all(&root) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => panic!("remove temp root {}: {err}", root.display()),
    }
}

fn run(args: &[&str]) -> Command {
    let mut cmd = cargo_allow_command();
    for arg in args {
        cmd.arg(arg);
    }
    cmd
}

fn write_source(root: &std::path::Path, unwraps: usize) {
    let body = (0..unwraps)
        .map(|i| format!("pub fn f{i}() -> u32 {{ let v: Option<u32> = None; v.unwrap() }}\n"))
        .collect::<String>();
    std::fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| panic!("create src dir: {err}"));
    std::fs::write(root.join("src/foo.rs"), body)
        .unwrap_or_else(|err| panic!("write src/foo.rs: {err}"));
}

fn base_policy(root: &std::path::Path) {
    // A minimal valid policy that receipt its own directory (any policy/*.toml,
    // including the baselined output) and sets no occurrence pinning — the
    // broad baseline is added by `add --glob`.
    std::fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| panic!("create policy dir: {err}"));
    std::fs::write(
        root.join("policy/allow.toml"),
        r#"schema_version = "0.1"
policy = "cargo-allow"
owner = "core"
status = "active"

[[allow]]
id = "allow-policy"
kind = "non_rust_file"
family = "configuration"
glob = "policy/*.toml"
owner = "core"
classification = "fixture"
reason = "policy files"
created = "2026-01-01"
review_after = "2026-12-01"

[allow.selector]
ast_kind = "tracked_file"
target_fingerprint = "toml"
glob = "policy/*.toml"
"#,
    )
    .unwrap_or_else(|err| panic!("write policy: {err}"));
}

/// Baseline one unwrap via `add --glob`, then add a second unwrap in the same
/// scope: `check --mode no-new` must fail (new debt). This is the exact #2056
/// acceptance scenario.
#[test]
fn add_glob_pins_count_and_blocks_new_in_scope_occurrence() {
    let root = temp_root("add-glob-ratchet");
    write_source(&root, 1);
    base_policy(&root);
    // Write the broad-baselined policy to a NEW file (not --force over the
    // base) so no .bak artifact is created that would itself surface as a new
    // finding. Check then runs against this combined policy.
    let policy_with_baseline = root.join("policy/allow.baselined.toml");

    // Pin the current (1) in-scope unwrap via a broad glob baseline.
    let add = run(&[
        "add",
        "--root",
        root.to_str().unwrap_or_default(),
        "--config",
        "policy/allow.toml",
        "--kind",
        "panic",
        "--family",
        "unwrap",
        "--callee",
        "unwrap",
        "--glob",
        "src/foo.rs",
        "--owner",
        "core",
        "--reason",
        "baseline one unwrap",
        "--classification",
        "reviewed_exception",
        "--review-after",
        "2027-12-01",
        "--write",
        policy_with_baseline.to_str().unwrap_or_default(),
    ])
    .output()
    .unwrap_or_else(|err| panic!("run add --glob: {err}"));
    assert!(
        add.status.success(),
        "add --glob should succeed; stderr=`{}`",
        String::from_utf8_lossy(&add.stderr)
    );

    // At the baseline count (1), no-new passes.
    let check_one = run(&[
        "check",
        "--root",
        root.to_str().unwrap_or_default(),
        "--config",
        "policy/allow.baselined.toml",
        "--mode",
        "no-new",
    ])
    .output()
    .unwrap_or_else(|err| panic!("run check: {err}"));
    assert!(
        check_one.status.success(),
        "no-new should pass at the pinned baseline; stderr=`{}`",
        String::from_utf8_lossy(&check_one.stderr)
    );

    // Add a SECOND unwrap in the same scope.
    write_source(&root, 2);

    // Now no-new must fail — the broad baseline is a ratchet floor.
    let check_two = run(&[
        "check",
        "--root",
        root.to_str().unwrap_or_default(),
        "--config",
        "policy/allow.baselined.toml",
        "--mode",
        "no-new",
    ])
    .output()
    .unwrap_or_else(|err| panic!("run check: {err}"));
    assert!(
        !check_two.status.success(),
        "no-new should fail when an in-scope occurrence is added past the pinned count; stderr=`{}`",
        String::from_utf8_lossy(&check_two.stderr)
    );
    let human =
        String::from_utf8_lossy(&check_two.stdout) + String::from_utf8_lossy(&check_two.stderr);
    assert!(
        human.contains("occurrence_limit exceeded"),
        "expected occurrence_limit exceeded in the failure output: `{human}`"
    );

    remove_temp_root(root);
}

/// `add --glob` on a scope with zero current findings must fail fail-closed
/// (cannot baseline an empty scope; occurrence_limit=0 is rejected anyway).
#[test]
fn add_glob_rejects_empty_scope_fail_closed() {
    let root = temp_root("add-glob-empty");
    // No source file at all -> zero matches for the glob.
    base_policy(&root);

    let add = run(&[
        "add",
        "--root",
        root.to_str().unwrap_or_default(),
        "--config",
        "policy/allow.toml",
        "--kind",
        "panic",
        "--family",
        "unwrap",
        "--callee",
        "unwrap",
        "--glob",
        "src/foo.rs",
        "--owner",
        "core",
        "--reason",
        "baseline",
        "--classification",
        "reviewed_exception",
        "--review-after",
        "2027-12-01",
        "--write",
        root.join("policy/allow.baselined.toml")
            .to_str()
            .unwrap_or_default(),
    ])
    .output()
    .unwrap_or_else(|err| panic!("run add --glob: {err}"));

    assert!(
        !add.status.success(),
        "add --glob on an empty scope should fail; stdout=`{}` stderr=`{}`",
        String::from_utf8_lossy(&add.stdout),
        String::from_utf8_lossy(&add.stderr)
    );
    let stderr = String::from_utf8_lossy(&add.stderr);
    assert!(
        stderr.contains("cannot baseline an empty scope"),
        "expected empty-scope error: `{stderr}`"
    );

    remove_temp_root(root);
}

/// `add --glob --summary-format json` must carry the shared `mutation_receipt`
/// envelope too, not just the `--path`/`--line` JSON path (the envelope must
/// not be reinvented or omitted per add mode).
#[test]
fn add_glob_json_summary_includes_mutation_receipt() {
    let root = temp_root("add-glob-mutation-receipt");
    write_source(&root, 1);
    base_policy(&root);

    let add = run(&[
        "add",
        "--root",
        root.to_str().unwrap_or_default(),
        "--config",
        "policy/allow.toml",
        "--kind",
        "panic",
        "--family",
        "unwrap",
        "--callee",
        "unwrap",
        "--glob",
        "src/foo.rs",
        "--owner",
        "core",
        "--reason",
        "baseline one unwrap",
        "--classification",
        "reviewed_exception",
        "--review-after",
        "2027-12-01",
        "--summary-format",
        "json",
        "--summary-output",
        root.join("add-summary.json").to_str().unwrap_or_default(),
    ])
    .output()
    .unwrap_or_else(|err| panic!("run add --glob: {err}"));
    assert!(
        add.status.success(),
        "add --glob should succeed; stderr=`{}`",
        String::from_utf8_lossy(&add.stderr)
    );

    let summary = fs::read_to_string(root.join("add-summary.json"))
        .unwrap_or_else(|err| panic!("read add summary: {err}"));
    assert!(
        summary.contains("\"mutation_receipt\":"),
        "add --glob JSON summary should carry the shared mutation_receipt envelope: `{summary}`"
    );
    assert!(
        summary.contains("\"schema_id\": \"cargo-allow.mutation-receipt.v1\""),
        "mutation_receipt should be schema-identified: `{summary}`"
    );
    assert!(
        summary.contains("\"operation\": \"add\""),
        "mutation_receipt should name the add operation: `{summary}`"
    );

    remove_temp_root(root);
}
