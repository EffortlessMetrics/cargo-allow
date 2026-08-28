use std::fs;
use std::path::Path;
use std::process::{Command, Output};

/// The passing result line must state the mode that produced it (#2832).
///
/// `check --mode no-new` enforces; `check --mode audit` and `audit` do not.
/// Both branches are asserted here on purpose: the previous label was wrong
/// for a long time precisely because only the advisory shape was ever
/// exercised, so a fix that covered one branch could regress the other
/// unnoticed.
fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow {args:?}: {err}")))
}

fn git_add(root: &Path) {
    let out = Command::new("git")
        .current_dir(root)
        .args(["add", "."])
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("git add: {err}")));
    assert!(out.status.success(), "git add should succeed");
}

fn stdout_of(result: &Output) -> String {
    String::from_utf8_lossy(&result.stdout).into_owned()
}

/// A clean repository with a policy, so every mode passes and the only thing
/// that varies between runs is the enforcement label.
fn fixture(label: &str) -> std::path::PathBuf {
    // Per-test path: these tests run in parallel and would otherwise delete
    // each other's fixture.
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-result-label-{label}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create fixture: {err}")));
    fs::write(root.join("src/lib.rs"), "pub fn ok() -> u32 {\n    1\n}\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write source: {err}")));

    for args in [
        vec!["init"],
        vec!["config", "user.email", "t@example.invalid"],
        vec!["config", "user.name", "cargo-allow test"],
    ] {
        let out = Command::new("git")
            .current_dir(&root)
            .args(&args)
            .output()
            .unwrap_or_else(|err| std::panic::panic_any(format!("git {args:?}: {err}")));
        assert!(out.status.success(), "git {args:?} should succeed");
    }

    assert!(
        run(&root, &["init"]).status.success(),
        "init should succeed"
    );
    git_add(&root);

    // The policy file `init` just wrote is itself a tracked non-Rust file, so
    // it needs a receipt before `--mode no-new` can reach a pass. Without it
    // the enforcing branch never renders and this file would only ever
    // exercise the advisory one.
    let receipt = run(
        &root,
        &[
            "add",
            "--update",
            "--kind",
            "non-rust",
            "--glob",
            "policy/*.toml",
            "--owner",
            "core/test",
            "--reason",
            "fixture policy file for result-label characterization",
        ],
    );
    assert!(receipt.status.success(), "receipting policy should succeed");
    git_add(&root);

    root
}

#[test]
fn an_enforcing_pass_and_an_advisory_pass_are_labelled_differently() {
    let root = fixture("modes");

    let enforcing = stdout_of(&run(&root, &["check", "--mode", "no-new"]));
    let advisory = stdout_of(&run(&root, &["check", "--mode", "audit"]));
    let audited = stdout_of(&run(&root, &["audit"]));

    assert!(enforcing.contains("passed (enforcing)"), "no-new enforces");
    assert!(
        !enforcing.contains("passed (advisory)"),
        "no-new is not advisory"
    );
    assert!(
        advisory.contains("passed (advisory)"),
        "audit mode is advisory"
    );
    assert!(audited.contains("passed (advisory)"), "audit is advisory");

    let _ = fs::remove_dir_all(&root);
}

/// Markdown and HTML must agree with human output. They previously carried
/// three independent copies of this string and disagreed.
#[test]
fn every_renderer_reports_the_same_enforcement() {
    let root = fixture("renderers");

    for format in ["human", "markdown", "html"] {
        let out = stdout_of(&run(
            &root,
            &["check", "--mode", "no-new", "--format", format],
        ));
        assert!(
            out.contains("passed (enforcing)"),
            "{format} states enforcing"
        );
    }

    let _ = fs::remove_dir_all(&root);
}
