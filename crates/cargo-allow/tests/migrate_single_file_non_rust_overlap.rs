//! #4840: a single-file non-Rust migration refuses an overlapping selector
//! pair it cannot rank, resolves the mechanical path-shadows-glob case, and
//! leaves the compat read path unchanged.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn unique_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    root
}

fn git_init(root: &Path) {
    let git_invocations: [&[&str]; 5] = [
        &["init", "-q"],
        &["config", "user.email", "cargo-allow@example.invalid"],
        &["config", "user.name", "cargo-allow test"],
        &["add", "."],
        &["commit", "-m", "non-rust overlap fixture"],
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

fn overlapping_policy() -> &'static str {
    r#"schema_version = 1
policy = "non-rust-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "non-rust-github-workflows"
glob = ".github/workflows/*.yml"
owner = "ci"
reason = "Workflow files."
broad_glob_reason = "Workflow definitions are tree-shaped."
expires = "never"

[[allow]]
id = "non-rust-github-meta"
glob = ".github/**"
owner = "ci"
reason = "Repository automation metadata."
broad_glob_reason = "Repository automation metadata is tree-shaped."
expires = "never"
"#
}

fn shadow_policy() -> &'static str {
    r#"schema_version = 1
policy = "non-rust-allowlist"
owner = "EffortlessMetrics"
status = "advisory"

[[allow]]
id = "non-rust-guide"
path = "docs/guide.md"
owner = "docs"
reason = "One reviewed guide."
expires = "never"

[[allow]]
id = "non-rust-docs"
glob = "docs/**"
owner = "docs"
reason = "Documentation tree."
broad_glob_reason = "Documentation is intentionally tree-shaped."
expires = "never"
"#
}

#[test]
fn migrate_single_file_refuses_overlapping_globs_and_names_both() {
    let root = unique_root("migrate-overlap-refusal");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    fs::create_dir_all(root.join(".github/workflows"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("workflow dir: {err}")));
    fs::write(root.join(".github/workflows/ci.yml"), "name: ci\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("workflow fixture: {err}")));
    fs::write(
        root.join("policy/non-rust-allowlist.toml"),
        overlapping_policy(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy fixture: {err}")));
    git_init(&root);

    let out = root.join("policy/candidate.toml");
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .current_dir(&root)
        .args([
            "migrate",
            "--from",
            "policy/non-rust-allowlist.toml",
            "--out",
        ])
        .arg(&out)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run migrate: {err}")));
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "overlapping selectors must refuse: {stderr}"
    );
    for selector in [".github/workflows/*.yml", ".github/**"] {
        assert!(
            stderr.contains(selector),
            "refusal must name `{selector}`: {stderr}"
        );
    }
    assert!(
        stderr.contains("overlap"),
        "refusal must name the overlap: {stderr}"
    );
    assert!(
        !out.exists(),
        "a refused migration must not write an output policy"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn migrate_single_file_resolves_path_shadowing_a_glob() {
    let root = unique_root("migrate-shadow-resolve");
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("docs dir: {err}")));
    fs::write(root.join("docs/guide.md"), "# guide\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("guide fixture: {err}")));
    fs::write(root.join("policy/non-rust-allowlist.toml"), shadow_policy())
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy fixture: {err}")));
    git_init(&root);

    let out = root.join("policy/candidate.toml");
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .current_dir(&root)
        .args([
            "migrate",
            "--from",
            "policy/non-rust-allowlist.toml",
            "--out",
        ])
        .arg(&out)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run migrate: {err}")));
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "a path shadowing one glob must resolve: {stderr}"
    );
    let generated = fs::read_to_string(&out)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read migrated policy: {err}")));
    assert!(
        generated.contains("docs/guide.md"),
        "resolved policy must carry the shadowed path: {generated}"
    );
    assert!(
        !generated.contains("ambiguous"),
        "migration output is policy text, not a scan result: {generated}"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compat_read_still_reads_the_overlapping_legacy_file() {
    let root = unique_root("migrate-overlap-compat");
    fs::create_dir_all(root.join("target"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("target dir: {err}")));
    fs::create_dir_all(root.join(".github/workflows"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("workflow dir: {err}")));
    fs::write(root.join(".github/workflows/ci.yml"), "name: ci\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("workflow fixture: {err}")));
    // Keep the legacy file under the inventory-ignored `target/**` so the
    // compat read is judged on the overlapping rule pair, not on the policy
    // file counting as an unreceipted non-Rust finding.
    fs::write(
        root.join("target/non-rust-allowlist.toml"),
        overlapping_policy(),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy fixture: {err}")));
    git_init(&root);

    let output = Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .current_dir(&root)
        .args([
            "check",
            "--compat",
            "--kind",
            "non-rust",
            "--config",
            "target/non-rust-allowlist.toml",
            "--mode",
            "no-new",
        ])
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run compat check: {err}")));
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "the compat read path must still read the legacy file: {stderr}\n{stdout}"
    );

    let _ = fs::remove_dir_all(&root);
}
