use std::fs;
use std::path::Path;
use std::process::{Command, Output};

/// The plan-then-apply route promoted in README, getting-started, onboarding,
/// and how-to/manage-an-exception (#2456).
///
/// This runs the documented argv against a real binary and a real repository,
/// so the guides cannot drift from what the tool actually accepts. It also
/// pins the boundary the docs claim: `why --plan` does not mutate policy, a
/// stale plan is refused, the targeted recheck reports one finding as matched,
/// and that recheck is not a repository proof.
/// Single fallible boundary: spawning the binary.
///
/// Runs from inside the fixture, the way an operator runs it in their own
/// repository, so the relative artifact paths in the documented argv resolve
/// the same way they do for a reader following the guide.
fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap_or_else(|err| panic!("run cargo-allow {args:?}: {err}"))
}

fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .unwrap_or_else(|err| panic!("git {args:?}: {err}"));
    assert!(out.status.success(), "git {args:?} failed");
}

fn write(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|err| panic!("create dir: {err}"));
    }
    fs::write(path, body).unwrap_or_else(|err| panic!("write: {err}"));
}

fn fixture(label: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("cargo-allow-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);

    write(
        &root.join("src/lib.rs"),
        "pub fn parse(text: &str) -> u32 {\n    text.parse().unwrap()\n}\n",
    );
    write(&root.join("docs/design.md"), "# design\n");
    write(&root.join(".gitignore"), "target/\n");
    fs::create_dir_all(root.join("target/cargo-allow"))
        .unwrap_or_else(|err| panic!("create artifact dir: {err}"));

    git(&root, &["init"]);
    git(&root, &["config", "user.email", "t@example.invalid"]);
    git(&root, &["config", "user.name", "cargo-allow test"]);

    let init = run(&root, &["init"]);
    assert!(init.status.success(), "init should succeed");
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "fixture"]);

    root
}

/// The four documented commands, in order, with the documented flags.
#[test]
fn documented_plan_then_apply_route_receipts_one_finding() {
    let root = fixture("receipt-route");
    let plan = "target/cargo-allow/add-plan.json";

    // 1. why --plan: read-only, New-only.
    let planned = run(
        &root,
        &[
            "why",
            "--kind",
            "panic",
            "--path",
            "src/lib.rs",
            "--line",
            "2",
            "--plan",
            plan,
        ],
    );
    assert!(planned.status.success(), "why --plan should succeed");
    assert!(root.join(plan).is_file(), "why --plan should write a plan");

    let policy = root.join("policy/allow.toml");
    let before = fs::read_to_string(&policy).unwrap_or_default();

    // 2. add --from-plan --update: one atomic ledger write.
    let applied = run(
        &root,
        &[
            "add",
            "--from-plan",
            plan,
            "--update",
            "--owner",
            "core",
            "--reason",
            "bounded fixture exception",
            "--evidence",
            "doc:docs/design.md",
            "--summary-format",
            "json",
            "--summary-output",
            "target/cargo-allow/add-application.json",
        ],
    );
    assert!(applied.status.success(), "add --from-plan should succeed");

    let after = fs::read_to_string(&policy).unwrap_or_default();
    assert_ne!(before, after, "add --from-plan should mutate the ledger");
    assert!(after.contains("bounded fixture exception"), "reason stored");

    // 3. targeted recheck: that one finding is now matched.
    let recheck = run(
        &root,
        &[
            "why",
            "--kind",
            "panic",
            "--path",
            "src/lib.rs",
            "--line",
            "2",
        ],
    );
    let text = String::from_utf8_lossy(&recheck.stdout);
    assert!(
        text.contains("status: matched"),
        "finding should be matched"
    );

    // 4. the full check is a separate, stronger claim than the recheck.
    let checked = run(&root, &["check", "--mode", "no-new"]);
    let report = String::from_utf8_lossy(&checked.stdout);
    assert!(report.contains("Result:"), "check should report a result");

    let _ = fs::remove_dir_all(&root);
}

/// The docs claim `why --plan` does not touch policy. Prove it, since that is
/// the whole reason the plan step is safe to run on a dirty tree.
#[test]
fn why_plan_does_not_mutate_policy() {
    let root = fixture("receipt-readonly");
    let policy = root.join("policy/allow.toml");
    let before = fs::read_to_string(&policy).unwrap_or_default();

    let planned = run(
        &root,
        &[
            "why",
            "--kind",
            "panic",
            "--path",
            "src/lib.rs",
            "--line",
            "2",
            "--plan",
            "target/cargo-allow/add-plan.json",
        ],
    );

    assert!(planned.status.success(), "why --plan should succeed");
    let after = fs::read_to_string(&policy).unwrap_or_default();
    assert_eq!(before, after, "why --plan must not mutate policy");

    let _ = fs::remove_dir_all(&root);
}

/// The docs promise a stale plan is refused rather than applied. That refusal
/// is the "stale-safe" property the whole route exists for.
#[test]
fn a_plan_stale_against_the_tree_is_refused() {
    let root = fixture("receipt-stale");
    let plan = "target/cargo-allow/add-plan.json";

    let planned = run(
        &root,
        &[
            "why",
            "--kind",
            "panic",
            "--path",
            "src/lib.rs",
            "--line",
            "2",
            "--plan",
            plan,
        ],
    );
    assert!(planned.status.success(), "why --plan should succeed");

    // Move the source out from under the plan.
    write(
        &root.join("src/lib.rs"),
        "pub fn parse(text: &str) -> u32 {\n    // shifted\n    text.parse().unwrap()\n}\n",
    );
    git(&root, &["add", "."]);

    let applied = run(
        &root,
        &[
            "add",
            "--from-plan",
            plan,
            "--update",
            "--owner",
            "core",
            "--reason",
            "bounded fixture exception",
            "--evidence",
            "doc:docs/design.md",
        ],
    );

    assert!(!applied.status.success(), "a stale plan must be refused");
    let err = String::from_utf8_lossy(&applied.stderr);
    assert!(
        err.contains("why --plan"),
        "refusal should name the regen fix"
    );

    let _ = fs::remove_dir_all(&root);
}
