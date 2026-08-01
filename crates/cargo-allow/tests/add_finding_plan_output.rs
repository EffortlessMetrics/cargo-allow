mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use support::{
    assert_saved_json_artifact, assert_status, assert_stderr_empty, assert_stdout_empty,
    cargo_allow_command, remove_temp_root, temp_root,
};

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap_or_else(|error| std::panic::panic_any(format!("git {args:?}: {error}")));
    assert_status("git fixture", &output, true);
}

#[test]
fn why_writes_a_new_only_bound_plan_without_overwriting_it() {
    let root = temp_root("add-finding-plan");
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|error| std::panic::panic_any(format!("create source dir: {error}")));
    git(&root, &["init"]);
    let init = cargo_allow_command()
        .args(["init", "--root"])
        .arg(&root)
        .output()
        .unwrap_or_else(|error| std::panic::panic_any(format!("init fixture: {error}")));
    assert_status("init fixture", &init, true);
    fs::write(
        root.join("src/lib.rs"),
        "pub fn load() -> usize { Some(1).unwrap() }\n",
    )
    .unwrap_or_else(|error| std::panic::panic_any(format!("write source: {error}")));
    git(&root, &["add", "policy/allow.toml", "src/lib.rs"]);

    let plan_path = root.join("add-plan.json");
    let why_path = root.join("why.json");
    let output = cargo_allow_command()
        .args(["why", "--root"])
        .arg(&root)
        .args(["--kind", "panic", "--path", "src/lib.rs", "--line", "1"])
        .args(["--format", "json", "--output"])
        .arg(&why_path)
        .arg("--include-untracked")
        .arg("--plan")
        .arg(&plan_path)
        .output()
        .unwrap_or_else(|error| std::panic::panic_any(format!("why plan: {error}")));
    assert_status("why plan", &output, true);
    assert_stdout_empty("why plan", &output, "should save explanation output");
    assert_stderr_empty("why plan", &output, "should not emit diagnostics");

    let plan = assert_saved_json_artifact(
        &plan_path,
        "add-finding-plan",
        "cargo-allow.add-finding-plan.v1",
        "why",
    );
    assert_eq!(
        plan.pointer("/outcome/status")
            .and_then(serde_json::Value::as_str),
        Some("new")
    );
    assert_eq!(
        plan.pointer("/evaluation/scope")
            .and_then(serde_json::Value::as_str),
        Some("scoped")
    );
    assert_eq!(
        plan.pointer("/evaluation/locality")
            .and_then(serde_json::Value::as_str),
        Some("proven")
    );
    assert_eq!(
        plan.pointer("/proof_plans/0/args/0")
            .and_then(serde_json::Value::as_str),
        Some("add")
    );
    let proof_args = plan
        .pointer("/proof_plans/0/args")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("add proof args should be an array"));
    for required in ["--root", "--config", "--include-untracked"] {
        assert!(
            proof_args
                .iter()
                .any(|argument| argument.as_str() == Some(required)),
            "add proof argv should preserve {required}"
        );
    }
    for pointer in [
        "/repository/identity",
        "/inventory_basis_identity",
        "/policy/digest",
        "/finding/digest",
        "/finding/source_file_digest",
    ] {
        assert!(
            plan.pointer(pointer)
                .and_then(serde_json::Value::as_str)
                .is_some_and(|value| value.starts_with("sha256:v1:") && value.len() == 74),
            "{pointer} should be a versioned SHA-256 binding"
        );
    }

    let original = fs::read(&plan_path)
        .unwrap_or_else(|error| std::panic::panic_any(format!("read plan: {error}")));
    let overwrite = cargo_allow_command()
        .args(["why", "--root"])
        .arg(&root)
        .args(["--kind", "panic", "--path", "src/lib.rs", "--line", "1"])
        .arg("--plan")
        .arg(&plan_path)
        .output()
        .unwrap_or_else(|error| std::panic::panic_any(format!("repeat why plan: {error}")));
    assert_status("repeat why plan", &overwrite, false);
    assert_eq!(
        fs::read(&plan_path)
            .unwrap_or_else(|error| std::panic::panic_any(format!("reread plan: {error}"))),
        original
    );

    let alias = cargo_allow_command()
        .args(["why", "--root"])
        .arg(&root)
        .args(["--kind", "panic", "--path", "src/lib.rs", "--line", "1"])
        .arg("--plan")
        .arg(root.join("alias.json"))
        .arg("--output")
        .arg(root.join("./alias.json"))
        .output()
        .unwrap_or_else(|error| std::panic::panic_any(format!("alias why plan: {error}")));
    assert_status("alias why plan", &alias, false);

    let add = cargo_allow_command()
        .args(["add", "--root"])
        .arg(&root)
        .args([
            "--kind",
            "panic",
            "--path",
            "src/lib.rs",
            "--line",
            "1",
            "--owner",
            "fixture",
            "--reason",
            "covered by the add-finding-plan output test",
            "--update",
        ])
        .output()
        .unwrap_or_else(|error| std::panic::panic_any(format!("add fixture entry: {error}")));
    assert_status("add fixture entry", &add, true);
    let matched_plan_path = root.join("matched-plan.json");
    let matched = cargo_allow_command()
        .args(["why", "--root"])
        .arg(&root)
        .args(["--kind", "panic", "--path", "src/lib.rs", "--line", "1"])
        .arg("--plan")
        .arg(&matched_plan_path)
        .output()
        .unwrap_or_else(|error| std::panic::panic_any(format!("matched why plan: {error}")));
    assert_status("matched why plan", &matched, false);
    assert!(
        !matched_plan_path.exists(),
        "matched plan must not be written"
    );

    remove_temp_root(root);
}
