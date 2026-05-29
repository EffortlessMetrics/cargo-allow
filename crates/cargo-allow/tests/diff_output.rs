use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn diff_json_with_output_file_does_not_emit_human_posture_to_stderr() {
    let root = temp_root("diff-output");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create src dir: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));
    fs::write(
        root.join("policy/allow.toml"),
        policy_with_scope("path = \"src/lib.rs\""),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write base policy: {err}")));
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "base"]);
    fs::write(
        root.join("policy/allow.toml"),
        policy_with_scope("glob = \"src/**\""),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write head policy: {err}")));
    let output = root.join("diff.json");

    let result = Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .arg("diff")
        .arg("--root")
        .arg(&root)
        .arg("--base")
        .arg("HEAD")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow diff: {err}")));

    assert!(
        !result.status.success(),
        "scope broadening should keep diff failing in no-new posture checks"
    );
    assert!(
        result.stderr.is_empty(),
        "diff --output should not emit human posture rows to stderr: `{}`",
        String::from_utf8_lossy(&result.stderr)
    );
    let json = fs::read_to_string(&output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read diff output: {err}")));
    assert!(json.contains("\"schema_id\": \"cargo-allow.report.v1\""));
    assert!(json.contains("\"scope_broadened\""));

    remove_temp_root(root);
}

fn policy_with_scope(scope: &str) -> String {
    format!(
        r#"policy = "cargo-allow"

[[allow]]
id = "allow-unwrap"
kind = "panic"
family = "unwrap"
{scope}
owner = "core"
classification = "reviewed_exception"
reason = "fixture"
created = "2026-05-29"
review_after = "2026-08-01"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"
"#
    )
}

fn temp_root(label: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_else(|err| std::panic::panic_any(format!("system clock: {err}")))
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create temp root: {err}")));
    root
}

fn remove_temp_root(root: PathBuf) {
    match fs::remove_dir_all(&root) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => std::panic::panic_any(format!("remove temp root {}: {err}", root.display())),
    }
}

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("git {args:?}: {err}")));
    if !output.status.success() {
        std::panic::panic_any(format!(
            "git {args:?} failed: stdout=`{}` stderr=`{}`",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
}
