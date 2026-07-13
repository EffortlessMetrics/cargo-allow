use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

#[test]
fn concurrent_adds_reload_policy_under_the_mutation_lock() -> Result<(), Box<dyn std::error::Error>>
{
    let root = TempRoot::new("concurrent-add")?;
    fs::create_dir_all(root.path().join("src"))?;
    fs::create_dir_all(root.path().join("policy"))?;
    fs::write(
        root.path().join("src/a.rs"),
        "pub fn a() -> u32 { let value: Option<u32> = None; value.unwrap() }\n",
    )?;
    fs::write(
        root.path().join("src/b.rs"),
        "pub fn b() -> u32 { let value: Option<u32> = None; value.unwrap() }\n",
    )?;
    fs::write(root.path().join("policy/allow.toml"), base_policy())?;
    let git_init = Command::new("git")
        .arg("init")
        .arg("--quiet")
        .current_dir(root.path())
        .status()?;
    if !git_init.success() {
        return Err("git init failed".into());
    }
    let git_add = Command::new("git")
        .args(["add", "src", "policy/allow.toml"])
        .current_dir(root.path())
        .status()?;
    if !git_add.success() {
        return Err("git add failed".into());
    }

    let first = add_command(&root, "src/a.rs").spawn()?;
    let second = add_command(&root, "src/b.rs").spawn()?;
    wait_success(first, "first concurrent add")?;
    wait_success(second, "second concurrent add")?;

    let policy = fs::read_to_string(root.path().join("policy/allow.toml"))?;
    if !policy.contains("glob = \"src/a.rs\"") || !policy.contains("glob = \"src/b.rs\"") {
        return Err("concurrent adds did not preserve both policy mutations".into());
    }
    Ok(())
}

fn add_command(root: &TempRoot, glob: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-allow"));
    command
        .arg("add")
        .arg("--root")
        .arg(root.path())
        .arg("--config")
        .arg("policy/allow.toml")
        .arg("--kind")
        .arg("panic")
        .arg("--family")
        .arg("unwrap")
        .arg("--callee")
        .arg("unwrap")
        .arg("--glob")
        .arg(glob)
        .arg("--owner")
        .arg("concurrency-test")
        .arg("--reason")
        .arg("concurrent mutation regression")
        .arg("--write")
        .arg(root.path().join("policy/allow.toml"))
        .arg("--force");
    command
}

fn wait_success(child: Child, label: &str) -> Result<(), Box<dyn std::error::Error>> {
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(format!(
            "{label} failed with status {:?}: stdout=`{}` stderr=`{}`",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(())
}

fn base_policy() -> &'static str {
    r#"schema_version = "0.1"
policy = "cargo-allow"
owner = "core"
status = "active"

[workspace]
root = "."
inventory = "git-tracked"
default_mode = "no-new"
ignored = ["policy/**"]
generated = ["target/**"]

[requirements]
owner_required = true
reason_required = true
classification_required = true
evidence_required = false
expires_or_review_after_required = true
allow_bare_allow_attributes = false
lint_policy_id_required = false
stale_entries_fail = false

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
"#
}

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(label: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "cargo-allow-{label}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
