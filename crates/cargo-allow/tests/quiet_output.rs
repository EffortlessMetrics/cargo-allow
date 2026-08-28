// Each test binary compiles `support` separately, so the helpers this one
// does not call would otherwise warn as dead code under `-D warnings`.
#[expect(
    dead_code,
    reason = "policy:allow-0494 shared test support module; this binary uses a subset"
)]
mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use support::{assert_status, cargo_allow_command, remove_temp_root, temp_root};

/// `--quiet` promises to suppress the claim boundary, matched inventory, and
/// non-matched advisory outcomes, showing only result and counts.
///
/// It shipped without a test, and only the claim boundary was actually
/// suppressed: the per-file matched inventory listing and the advisory
/// outcomes both printed regardless. These tests pin each half of the
/// contract — what quiet drops, and what it must never drop (#2785).
#[test]
fn quiet_drops_matched_inventory_listing_and_claim_boundary() {
    let root = temp_root("quiet-suppresses");
    init_fixture_repo(&root);

    let loud = run_check(&root, false);
    let quiet = run_check(&root, true);

    // Counts survive in both: they are the "result + counts" quiet promises.
    assert!(
        loud.contains("Non-Rust file inventory:") && quiet.contains("Non-Rust file inventory:"),
        "quiet should keep the non-Rust counts block\nquiet:\n{quiet}"
    );

    // The per-file listing is the matched inventory quiet exists to drop.
    assert!(
        loud.contains("  files:\n"),
        "fixture should produce a per-file listing without --quiet\nloud:\n{loud}"
    );
    assert!(
        !quiet.contains("  files:\n"),
        "quiet should drop the per-file matched inventory listing\nquiet:\n{quiet}"
    );

    assert!(
        loud.contains("Claim boundary:"),
        "claim boundary should print without --quiet\nloud:\n{loud}"
    );
    assert!(
        !quiet.contains("Claim boundary:"),
        "quiet should drop the claim boundary\nquiet:\n{quiet}"
    );

    assert!(
        quiet.lines().count() < loud.lines().count(),
        "quiet should be shorter than the default report"
    );

    remove_temp_root(root);
}

/// Blocking outcomes are the reason a run failed. Suppressing them would leave
/// an operator with a bare non-zero exit and no explanation, so `--quiet` must
/// keep them even though they are non-matched.
#[test]
fn quiet_keeps_blocking_outcomes_and_failure_reason() {
    let root = temp_root("quiet-keeps-blocking");
    init_fixture_repo(&root);

    // A tracked file outside every receipted glob is `new`, which fails the
    // check. `.sh` rather than `.md` so it is genuinely unreceipted rather
    // than an occurrence-limit overflow on the fixture's `**/*.md` baseline.
    fs::write(root.join("run.sh"), "#!/usr/bin/env bash\necho hi\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write run.sh: {err}")));
    git(&root, &["add", "."]);

    let quiet = run_check_expecting(&root, true, false);

    assert!(
        quiet.contains("new: unreceipted"),
        "quiet must keep blocking `new` outcomes\nquiet:\n{quiet}"
    );
    assert!(
        quiet.contains("Result: failed"),
        "quiet must keep the failure result line\nquiet:\n{quiet}"
    );

    remove_temp_root(root);
}

fn run_check(root: &Path, quiet: bool) -> String {
    run_check_expecting(root, quiet, true)
}

fn run_check_expecting(root: &Path, quiet: bool, should_succeed: bool) -> String {
    let mut command = cargo_allow_command();
    command.arg("check").arg("--root").arg(root);
    if quiet {
        command.arg("--quiet");
    }
    let output = command
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run check: {err}")));
    assert_status("check", &output, should_succeed);
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// A git repo with enough tracked non-Rust files to produce a per-file
/// listing, all receipted so the default run passes.
fn init_fixture_repo(root: &Path) {
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create docs dir: {err}")));
    for index in 0..3 {
        fs::write(
            root.join(format!("docs/guide-{index}.md")),
            format!("# guide {index}\n"),
        )
        .unwrap_or_else(|err| std::panic::panic_any(format!("write guide: {err}")));
    }

    git(root, &["init"]);
    git(
        root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(root, &["config", "user.name", "cargo-allow test"]);

    let init = cargo_allow_command()
        .arg("init")
        .arg("--root")
        .arg(root)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run init: {err}")));
    assert_status("init", &init, true);

    git(root, &["add", "."]);

    // Receipt every current finding so the default run passes and the only
    // difference between loud and quiet is presentation, not posture. The
    // policy file `init` just wrote is itself a tracked non-Rust file, so it
    // needs a receipt too.
    for glob in ["**/*.md", "policy/*.toml"] {
        receipt_glob(root, glob);
        git(root, &["add", "."]);
    }
}

fn receipt_glob(root: &Path, glob: &str) {
    let added = cargo_allow_command()
        .arg("add")
        .arg("--root")
        .arg(root)
        .arg("--update")
        .arg("--kind")
        .arg("non-rust")
        .arg("--glob")
        .arg(glob)
        .arg("--owner")
        .arg("core/test")
        .arg("--reason")
        .arg("fixture file for quiet-output characterization")
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run add {glob}: {err}")));
    assert_status("add", &added, true);
}

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run git {args:?}: {err}")));
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
