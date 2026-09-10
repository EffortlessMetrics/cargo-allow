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
    let script = read_workspace_file(&root, "scripts/project-review-readiness.sh");
    // The Actions job check is an implementation detail under a
    // different name; the one stable `review-readiness` context is
    // the published typed check run only, so branch protection can
    // never observe a job-level success standing in for a neutral
    // review.
    assert!(
        workflow.contains("  review-readiness-publisher:"),
        "the publisher job is named distinctly from the published context"
    );
    assert!(
        !workflow.contains("  review-readiness:\n"),
        "the workflow must not create a competing check under the stable context name"
    );
    assert!(
        script.contains(concat!("CHECK_NAME=", "\"review-readiness\"")),
        "the adapter publishes the one stable check context"
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
        "edited",
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

fn validate_readiness_api_calls(script: &str) -> Result<(), String> {
    let mut found_check_run = false;
    for call in script
        .lines()
        .filter(|line| line.contains("gh api"))
        .map(|line| {
            line.split_once("gh api ")
                .map_or(line, |(_, call)| call)
                .trim()
        })
    {
        // Exact retained command-start shapes, not a general shell parser.
        // In particular, ref acquisition has no continuation or flags that
        // could override the default GET or implicitly select POST.
        match call {
            r#""${API}/git/ref/heads/${encoded}" 2>/dev/null)"; then"# => {}
            r#""${API}/commits/${head_sha}/check-runs?check_name=${CHECK_NAME}" \"#
            | r#""${API}/check-runs/${existing_id}" -X PATCH \"#
            | r#""${API}/check-runs" -X POST \"# => found_check_run = true,
            _ => return Err(format!("unexpected readiness API command: {call}")),
        }
    }
    if found_check_run {
        Ok(())
    } else {
        Err("the adapter must retain its check-run API surface".to_owned())
    }
}

#[test]
fn repository_release_controls_never_mutate_or_self_require() -> Result<(), String> {
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
    validate_readiness_api_calls(&script)?;
    // The workflow cannot make itself a required check: it publishes
    // a check run but configures no required context; live required-
    // context configuration is #2284's alone.
    let workflow = read_workspace_file(&root, ".github/workflows/review-readiness.yml");
    assert!(
        !workflow.contains("required_status_check"),
        "a source workflow cannot self-require"
    );
    Ok(())
}

#[test]
fn repository_release_controls_reject_ref_writes_and_unrelated_api_calls() -> Result<(), String> {
    let root = workspace_root();
    let script = read_workspace_file(&root, "scripts/project-review-readiness.sh");
    let read = r#"gh api "${API}/git/ref/heads/${encoded}" 2>/dev/null)"; then"#;
    if script.matches(read).count() != 1 {
        return Err("the ref-read control must replace exactly one real acquisition".to_owned());
    }
    validate_readiness_api_calls(&script)?;
    for replacement in [
        r#"gh api "${API}/git/ref/heads/${encoded}" -X PATCH 2>/dev/null)"; then"#,
        r#"gh api "${API}/git/ref/heads/${encoded}" --method POST 2>/dev/null)"; then"#,
        r#"gh api "${API}/git/ref/heads/${encoded}" -f sha=changed 2>/dev/null)"; then"#,
        r#"gh api "${API}/git/ref/heads/${encoded}" --input payload.json 2>/dev/null)"; then"#,
        "gh api \"${API}/git/ref/heads/${encoded}\" \\\n  -X PATCH 2>/dev/null)\"; then",
        r#"gh api "${API}/git/matching-refs/heads/${encoded}" 2>/dev/null)"; then"#,
        r#"gh api "${API}/git/ref/tags/${encoded}" 2>/dev/null)"; then"#,
        r#"gh api "${API}/issues?label=check-runs" 2>/dev/null)"; then"#,
    ] {
        let changed = script.replace(read, replacement);
        if validate_readiness_api_calls(&changed).is_ok() {
            return Err(format!("API surface guard admitted: {replacement}"));
        }
    }
    for missing_check_run in ["no API calls", read] {
        if validate_readiness_api_calls(missing_check_run).is_ok() {
            return Err("API surface guard admitted a missing check-run surface".to_owned());
        }
    }
    Ok(())
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
    // Ancestor-bound records feed the ledger bootstrap; exact-head
    // records take precedence; base-changing edits recompute; the
    // check run is updated in place instead of accumulating stale
    // duplicates; and enumeration failures fail the run.
    assert!(
        script.contains("git merge-base --is-ancestor"),
        "ancestor-bound records feed the ledger bootstrap"
    );
    assert!(
        script.contains("check-runs/${existing_id}") && script.contains("-X PATCH"),
        "the authoritative check run is updated in place"
    );
    assert!(
        script.contains("PR_BASE_CHANGED"),
        "base-changing edits map to the base-moved event"
    );
    assert!(
        script.contains(r#"open_prs="$(gh pr list"#),
        "enumeration failure must fail the run instead of looking empty"
    );
    // An unreadable retained record is review evidence that fails
    // closed; it never degrades to a missing review.
    assert!(
        script.contains("unreadable retained disposition records"),
        "malformed ledger records fail closed"
    );
}
