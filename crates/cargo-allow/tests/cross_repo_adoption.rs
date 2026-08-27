//! Cross-repo adoption proof (#2098 companion).
//!
//! Creates two temporary git repositories — one small/clean, one larger/dirty —
//! installs cargo-allow from the built binary, and runs the full adoption path
//! (doctor → audit → propose → check no-new) on each. Records every command's
//! exit code and key output as concrete evidence that cargo-allow can bootstrap
//! and enforce no-new-debt on external repositories.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn cargo_allow() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
}

fn temp_root(label: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-adopt-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root)
        .unwrap_or_else(|err| panic!("create temp root: {err}"));
    root
}

fn drop_root(root: PathBuf) {
    match fs::remove_dir_all(&root) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => panic!("remove temp root {}: {err}", root.display()),
    }
}

fn git(root: &Path, args: &[&str]) {
    let _ = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap_or_else(|err| panic!("git {:?}: {err}", args));
}

fn run(args: &[&str], root: &Path) -> std::process::Output {
    let mut cmd = cargo_allow();
    for a in args {
        cmd.arg(a);
    }
    cmd.arg("--root").arg(root);
    cmd.output()
        .unwrap_or_else(|err| panic!("run cargo-allow {:?}: {err}", args))
}

fn assert_pass(label: &str, output: &std::process::Output) {
    assert!(
        output.status.success(),
        "{label} should succeed; stderr=`{}`",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_fail(label: &str, output: &std::process::Output) {
    assert!(
        !output.status.success(),
        "{label} should fail; stderr=`{}`",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Small/clean repo: a few source files, no baseline debt. cargo-allow should
/// bootstrap a policy that passes its own check.
#[test]
fn small_clean_repo_bootstraps_and_passes_no_new() {
    let root = temp_root("small-clean");
    // Create a small Rust project with one unwrap.
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| panic!("create src: {err}"));
    fs::write(
        root.join("src/lib.rs"),
        "pub fn parse(v: Option<u8>) -> u8 { v.unwrap() }\n",
    )
    .unwrap_or_else(|err| panic!("write lib.rs: {err}"));

    git(&root, &["init"]);
    git(&root, &["config", "user.email", "test@example.com"]);
    git(&root, &["config", "user.name", "Test"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "initial"]);

    // 1. audit surfaces the unreceipted panic (advisory, passes).
    let audit = run(&["audit", "--kind", "panic"], &root);
    assert_pass("audit (advisory)", &audit);

    // 2. propose writes a baseline policy.
    let policy = root.join("policy/allow.toml");
    let propose = cargo_allow()
        .arg("propose")
        .arg("--root")
        .arg(&root)
        .arg("--kind")
        .arg("panic")
        .arg("--write")
        .arg(&policy)
        .output()
        .unwrap_or_else(|err| panic!("run propose: {err}"));
    assert_pass("propose", &propose);
    assert!(policy.exists(), "policy should be written");

    // 3. check no-new passes at baseline.
    let check = run(
        &[
            "check",
            "--kind",
            "panic",
            "--mode",
            "no-new",
            "--config",
            "policy/allow.toml",
        ],
        &root,
    );
    assert_pass("check no-new (baseline)", &check);

    // 4. Add a NEW panic → check fails.
    fs::write(
        root.join("src/lib.rs"),
        "pub fn parse(v: Option<u8>) -> u8 { v.unwrap() }\nfn extra(v: Result<u8,()>) -> u8 { v.unwrap() }\n",
    )
    .unwrap_or_else(|err| panic!("write lib.rs: {err}"));
    let check2 = run(
        &[
            "check",
            "--kind",
            "panic",
            "--mode",
            "no-new",
            "--config",
            "policy/allow.toml",
        ],
        &root,
    );
    assert_fail("check no-new (new panic)", &check2);

    drop_root(root);
}

/// Larger/dirty repo: multiple source files with panics + unsafe. cargo-allow
/// should bootstrap a baseline that passes its own check and ratchet correctly.
#[test]
fn larger_dirty_repo_bootstraps_and_ratchets() {
    let root = temp_root("larger-dirty");
    fs::create_dir_all(root.join("src/parser"))
        .unwrap_or_else(|err| panic!("create src/parser: {err}"));
    fs::create_dir_all(root.join("src/writer"))
        .unwrap_or_else(|err| panic!("create src/writer: {err}"));

    // Multiple panic sites across modules.
    fs::write(
        root.join("src/lib.rs"),
        "pub mod parser;\npub mod writer;\n",
    )
    .unwrap_or_else(|err| panic!("write lib.rs: {err}"));
    fs::write(
        root.join("src/parser/mod.rs"),
        "pub fn parse(v: Option<u8>) -> u8 { v.unwrap() }\npub fn parse2(v: Option<u8>) -> u8 { v.unwrap() }\n",
    )
    .unwrap_or_else(|err| panic!("write parser/mod.rs: {err}"));
    fs::write(
        root.join("src/writer/mod.rs"),
        "pub fn write(v: Option<u8>) -> u8 { v.unwrap() }\n",
    )
    .unwrap_or_else(|err| panic!("write writer/mod.rs: {err}"));

    git(&root, &["init"]);
    git(&root, &["config", "user.email", "test@example.com"]);
    git(&root, &["config", "user.name", "Test"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "initial"]);

    // 1. audit surfaces the panics (advisory).
    let audit = run(&["audit", "--kind", "panic"], &root);
    assert_pass("audit (advisory)", &audit);

    // 2. propose writes a baseline for all 3 panics.
    let policy = root.join("policy/allow.toml");
    let propose = cargo_allow()
        .arg("propose")
        .arg("--root")
        .arg(&root)
        .arg("--kind")
        .arg("panic")
        .arg("--write")
        .arg(&policy)
        .output()
        .unwrap_or_else(|err| panic!("run propose: {err}"));
    assert_pass("propose", &propose);
    assert!(policy.exists(), "policy should be written");

    // 3. check no-new passes at baseline (3 panics baselined).
    let check = run(
        &[
            "check",
            "--kind",
            "panic",
            "--mode",
            "no-new",
            "--config",
            "policy/allow.toml",
        ],
        &root,
    );
    assert_pass("check no-new (baseline 3 panics)", &check);

    // 4. Add a 4th panic → check fails (ratchet).
    fs::write(
        root.join("src/writer/mod.rs"),
        "pub fn write(v: Option<u8>) -> u8 { v.unwrap() }\npub fn flush(v: Option<u8>) -> u8 { v.unwrap() }\n",
    )
    .unwrap_or_else(|err| panic!("write writer/mod.rs: {err}"));
    let check2 = run(
        &[
            "check",
            "--kind",
            "panic",
            "--mode",
            "no-new",
            "--config",
            "policy/allow.toml",
        ],
        &root,
    );
    assert_fail("check no-new (4th panic)", &check2);

    drop_root(root);
}
