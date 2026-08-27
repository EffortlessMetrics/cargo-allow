use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

#[test]
fn add_rejects_unsafe_entry_without_evidence() {
    let root = temp_root("add-unsafe-evidence");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| panic!("create policy dir: {err}"));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| panic!("create source dir: {err}"));
    fs::write(root.join("src/lib.rs"), "pub fn read() { unsafe {} }\n")
        .unwrap_or_else(|err| panic!("write source: {err}"));
    fs::write(
        root.join("policy/allow.toml"),
        r#"schema_version = "0.1"
policy = "cargo-allow"
owner = "core/policy"
status = "active"

[workspace]
root = "."
inventory = "git-tracked"
default_mode = "no-new"
ignored = ["policy/**", "target/**"]
generated = ["target/**", "vendor/**"]

[requirements]
owner_required = true
reason_required = true
classification_required = true
evidence_required = false
expires_or_review_after_required = true
allow_bare_allow_attributes = false
lint_policy_id_required = false
stale_entries_fail = false

[requirements.unsafe]
evidence_required = true
safety_comment_required = false
"#,
    )
    .unwrap_or_else(|err| panic!("write policy: {err}"));

    let output_policy = root.join("policy/allow.added.toml");
    let result = cargo_allow_command()
        .arg("add")
        .arg("--root")
        .arg(&root)
        .arg("--config")
        .arg("policy/allow.toml")
        .arg("--kind")
        .arg("unsafe")
        .arg("--path")
        .arg("src/lib.rs")
        .arg("--line")
        .arg("1")
        .arg("--owner")
        .arg("core/tests")
        .arg("--reason")
        .arg("Unsafe boundary is reviewed before retention.")
        .arg("--write")
        .arg(&output_policy)
        .output()
        .unwrap_or_else(|err| panic!("run add: {err}"));

    assert_status("add unsafe without evidence", &result, false);
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("unsafe allow entries require at least one --evidence reference"),
        "stderr should explain the unsafe evidence requirement: {stderr}"
    );
    assert!(
        !output_policy.exists(),
        "add should not write a reviewed policy when unsafe evidence is missing"
    );

    remove_temp_root(root);
}

fn cargo_allow_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
}

fn assert_status(command: &str, result: &Output, should_succeed: bool) {
    assert_eq!(
        result.status.success(),
        should_succeed,
        "{command} status mismatch: stdout=`{}` stderr=`{}`",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

fn temp_root(label: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_else(|err| panic!("system clock: {err}"))
        .as_nanos();
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
