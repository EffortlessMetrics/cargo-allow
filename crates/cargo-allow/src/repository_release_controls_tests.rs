//! Repository release-controls surface for the #3844 review-readiness
//! check: the stable context name, the minimal permission posture, the
//! event coverage, and the no-mutation/no-self-requirement law are all
//! checked against the source-controlled workflow and adapter.

use allow_report::REVIEW_READINESS_CHECK_CONTEXT;

fn workspace_root() -> std::path::PathBuf {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set for cargo tests");
    std::path::PathBuf::from(manifest_dir)
        .join("../..")
        .canonicalize()
        .expect("workspace root resolves")
}

fn read_workspace_file(root: &std::path::Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel))
        .expect("the release-controls surface is retained in the tree")
}

#[test]
fn repository_release_controls_publish_one_stable_check_context() {
    // #2283 names the check; the context string is the contract.
    assert_eq!(REVIEW_READINESS_CHECK_CONTEXT, "review-readiness");

    let root = workspace_root();
    let workflow = read_workspace_file(&root, ".github/workflows/review-readiness.yml");
    assert!(
        workflow.contains("  review-readiness:"),
        "the job id is the visible check context"
    );
    assert!(
        workflow.contains("name: Review readiness"),
        "the workflow is named for the readiness lane"
    );
}

#[test]
fn repository_release_controls_cover_every_readiness_relevant_event() {
    let root = workspace_root();
    let workflow = read_workspace_file(&root, ".github/workflows/review-readiness.yml");
    for event in [
        "opened",
        "reopened",
        "synchronize",
        "ready_for_review",
        "converted_to_draft",
    ] {
        assert!(
            workflow.contains(event),
            "the readiness workflow must trigger on {event}"
        );
    }
    // Base-branch movement fires no pull_request event; a push to the
    // base branch must recompute open pull requests against the new
    // merge base so a stale green cannot survive base movement.
    assert!(
        workflow.contains("branches: [main]"),
        "the readiness workflow must recompute on base-branch movement"
    );
}

#[test]
fn repository_release_controls_use_minimum_permissions() {
    let root = workspace_root();
    let workflow = read_workspace_file(&root, ".github/workflows/review-readiness.yml");
    let permissions_block = workflow
        .split("permissions:")
        .nth(1)
        .and_then(|rest| rest.split("jobs:").next())
        .expect("a permissions block before jobs");
    assert!(
        permissions_block.contains("contents: read"),
        "contents stays read-only: {permissions_block}"
    );
    assert!(
        permissions_block.contains("pull-requests: read"),
        "pull-requests stays read-only: {permissions_block}"
    );
    // checks: write is the one granted write: publishing the typed
    // check run is what makes a neutral (missing-disposition) result
    // visible instead of a green check. Everything else stays
    // read-only.
    assert!(
        permissions_block.contains("checks: write"),
        "publishing the check conclusion requires checks: write: {permissions_block}"
    );
    for forbidden in ["issues:", "contents: write", "pull-requests: write"] {
        assert!(
            !permissions_block.contains(forbidden),
            "the readiness workflow must not request '{forbidden}'"
        );
    }
}

#[test]
fn repository_release_controls_never_mutate_or_self_require() {
    let root = workspace_root();
    let script = read_workspace_file(&root, "scripts/project-review-readiness.sh");
    // The only GitHub write is publishing the review-readiness check
    // run itself; every other mutation verb is absent.
    for mutation in [
        "gh pr merge",
        "gh pr ready",
        "gh pr edit",
        "gh pr close",
        "gh pr update-branch",
        "gh release",
        "ruleset",
    ] {
        assert!(
            !script.contains(mutation),
            "the readiness adapter must not mutate state via '{mutation}'"
        );
    }
    assert!(
        script.contains("review-readiness project"),
        "the adapter runs the typed projection"
    );
    let api_calls: Vec<&str> = script
        .lines()
        .filter(|line| line.contains("gh api"))
        .collect();
    assert_eq!(
        api_calls.len(),
        1,
        "exactly one gh api call is permitted (the check-run publish)"
    );
    assert!(
        api_calls[0].contains("check-runs"),
        "the single gh api call publishes the readiness check run"
    );
    // The workflow cannot make itself a required check: it publishes
    // a check run but configures no required context; live required-
    // context configuration is #2284's alone.
    let workflow = read_workspace_file(&root, ".github/workflows/review-readiness.yml");
    assert!(
        !workflow.contains("required_status_check"),
        "a source workflow cannot self-require"
    );
}

#[test]
fn repository_release_controls_bind_the_disposition_location() {
    let root = workspace_root();
    let script = read_workspace_file(&root, "scripts/project-review-readiness.sh");
    assert!(
        script.contains(".allow/review-dispositions"),
        "the checked adapter reads retained dispositions from the declared location"
    );
    assert!(
        script.contains("sha256sum"),
        "the adapter binds the diff digest with the documented recipe"
    );
    assert!(
        script.contains("git merge-base"),
        "the adapter binds the effective merge base"
    );
    // Ambiguous disposition records fail closed, and the head delta
    // is delivered so the projection can prove the review-ledger
    // bootstrap.
    assert!(
        script.contains("ambiguous retained dispositions"),
        "duplicate dispositions fail closed"
    );
    assert!(
        script.contains("--head-delta-path"),
        "the adapter passes the head delta for the ledger-bootstrap proof"
    );
}
