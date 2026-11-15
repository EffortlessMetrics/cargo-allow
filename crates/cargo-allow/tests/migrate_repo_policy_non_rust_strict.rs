//! #4817 end-to-end negative control.
//!
//! The compat read path tolerates unmapped non-Rust entry fields, but
//! `migrate --repo-policy` is a write path: it expands current findings and
//! must still refuse an entry whose field the canonical shape would drop.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Owns the temporary repository so a panic in setup or in an assertion still
/// removes it during unwinding.
struct TempRoot(PathBuf);

impl TempRoot {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!("{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        Self(root)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn git_init(root: &Path) {
    let git_invocations: [&[&str]; 5] = [
        &["init", "-q"],
        &["config", "user.email", "cargo-allow@example.invalid"],
        &["config", "user.name", "cargo-allow test"],
        &["add", "."],
        &["commit", "-m", "migrate strictness fixture"],
    ];
    for git_args in git_invocations {
        let output = Command::new("git")
            .args(git_args)
            .current_dir(root)
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("git {git_args:?}: {err}")));
        assert!(
            output.status.success(),
            "git {git_args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn non_rust_policy_with_language() -> &'static str {
    r#"schema_version = 1
policy = "non-rust-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "shiplog-language"
path = "README.md"
language = "markdown"
owner = "shiplog"
reason = "Language field from the legacy schema."
"#
}

#[test]
fn migrate_repo_policy_refuses_unmapped_non_rust_field_and_writes_nothing() {
    let temp = TempRoot::new("migrate-repo-policy-strict-non-rust");
    let root = temp.path();
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy dir: {err}")));
    fs::write(root.join("README.md"), "# readme\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write readme: {err}")));
    fs::write(
        root.join("policy/non-rust-allowlist.toml"),
        non_rust_policy_with_language(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write legacy policy: {err}")));
    git_init(&root);

    let out = root.join("policy/candidate.toml");
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .current_dir(&root)
        .args(["migrate", "--repo-policy", "policy", "--out"])
        .arg(&out)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run repo-policy migrate: {err}")));
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "directory migration must refuse an unmapped non-Rust field: {stderr}"
    );
    assert!(
        stderr.contains("unsupported field `language`"),
        "the refusal must name the offending field: {stderr}"
    );
    assert!(
        !out.exists(),
        "a refused migration must not write an output policy"
    );
}
