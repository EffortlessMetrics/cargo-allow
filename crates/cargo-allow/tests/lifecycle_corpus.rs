mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use allow_core::allow_entry_content_fingerprint;
use serde_json::{Value, json};
use support::{
    assert_saved_json_artifact, assert_status, assert_stderr_empty, assert_stdout_empty,
    cargo_allow_command, remove_temp_root, temp_root,
};

const EXPIRED_ID: &str = "allow-expired";
const REVIEW_DUE_ID: &str = "allow-review";
const STALE_ID: &str = "allow-stale";
const DRIFT_ID: &str = "allow-drift";
const HEADROOM_ID: &str = "allow-headroom";
const MISSING_EVIDENCE_ID: &str = "allow-missing-evidence";
const BROKEN_EVIDENCE_ID: &str = "allow-broken-evidence";
const WEAK_EVIDENCE_ID: &str = "allow-weak-evidence";
const BASELINE_DEBT_ID: &str = "allow-baseline-debt";
const MIRROR_LEDGER_ID: &str = "source-policy-mirror";
const MIRROR_LEDGER_PATH: &str = ".allow/mirror/policy.toml";
const AUDIT_ARGS: &[&str] = &["audit"];
const CHECK_NO_NEW_ARGS: &[&str] = &["check", "--mode", "no-new"];
const DIFF_ARGS: &[&str] = &["diff", "--base", "HEAD"];

#[test]
fn policy_change_notes_pin_exact_transition_and_inverse_is_improvement() {
    let weakening_root = create_policy_change_fixture("policy-weakening", "src/lib.rs");
    let weakening_base = policy_with_occurrence_limit(1);
    git(&weakening_root, &["add", "policy/allow.toml"]);
    fs::write(weakening_root.join("policy/allow.toml"), &weakening_base)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write weakening base policy: {err}")));
    git(&weakening_root, &["add", "policy/allow.toml"]);
    git(
        &weakening_root,
        &["commit", "--no-gpg-sign", "-m", "set weakening base"],
    );
    let weakening_head = policy_with_occurrence_limit(3);
    fs::write(weakening_root.join("policy/allow.toml"), &weakening_head)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write weakening policy: {err}")));

    let missing_output = weakening_root.join("target/cargo-allow/weakening-missing.json");
    let missing = run_diff_with_note_requirement(
        &weakening_root,
        &weakening_base,
        &weakening_head,
        &missing_output,
    );
    assert_status("weakening without note", &missing, false);
    assert_saved_json_artifact(
        &missing_output,
        "weakening without note",
        "cargo-allow.report.v1",
        "diff",
    );
    let missing_changes = policy_changes(&missing_output);
    assert_eq!(
        missing_changes.len(),
        1,
        "unexpected weakening rows: {missing_changes:?}"
    );
    assert_policy_change_fields(
        &missing_output,
        "occurrence_limit_loosened",
        "allow-transition",
        "fail",
        "retained",
        "worsened",
    );
    let (before_fingerprint, after_fingerprint) =
        transition_fingerprints(&weakening_base, &weakening_head);
    // #3218 keeps stderr clean in JSON mode so pipelines are not disturbed, so
    // the #2075 change-note diagnostic is asserted on the human render instead.
    // The JSON run above still pins the machine-readable transition rows.
    let missing_rendered = run_diff_rendered(&weakening_root, "human", true);
    assert_status("weakening without note (human)", &missing_rendered, false);
    let missing_stderr = String::from_utf8_lossy(&missing_rendered.stderr);
    assert!(
        missing_stderr.contains("change note required: allow-transition occurrence_limit_loosened"),
        "missing-note diagnostic should identify the transition: {missing_stderr}"
    );
    assert!(
        missing_stderr.contains(&before_fingerprint) && missing_stderr.contains(&after_fingerprint),
        "missing-note diagnostic should provide the exact fingerprint route: {missing_stderr}"
    );

    let template_path = weakening_root.join(".allow/revisions/generated.toml");
    let template_output = weakening_root.join("target/cargo-allow/weakening-template.json");
    let template = run_diff_with_template(
        &weakening_root,
        &weakening_base,
        &weakening_head,
        &template_output,
        Path::new(".allow/revisions/generated.toml"),
    );
    assert_status("weakening template generation", &template, false);
    let template_text = fs::read_to_string(&template_path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read generated template: {err}")));
    assert!(template_text.contains("allow_ids = [\"allow-transition\"]"));
    assert!(template_text.contains(&before_fingerprint));
    assert!(template_text.contains(&after_fingerprint));

    let dogfood_receipt =
        weakening_root.join("target/cargo-allow/change-control-dogfood.receipt.json");
    fs::write(
        &dogfood_receipt,
        serde_json::to_vec_pretty(&json!({
            "repository": weakening_root.display().to_string(),
            "base": "HEAD",
            "merge_base": "HEAD",
            "tested_head": "working-tree-policy",
            "policy": "policy/allow.toml",
            "revisions_dir": ".allow/revisions",
            "allow_id": "allow-transition",
            "change_kind": "occurrence_limit_loosened",
            "before_fingerprint": before_fingerprint,
            "after_fingerprint": after_fingerprint,
            "missing_note": "blocked",
            "template": ".allow/revisions/generated.toml",
            "satisfied_note": "passed",
            "stale_note": "blocked",
            "claim_boundary": "fixture-only exact weakening-note-repair journey"
        }))
        .unwrap_or_else(|err| std::panic::panic_any(format!("serialize dogfood receipt: {err}"))),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write dogfood receipt: {err}")));
    write_revision_note(&weakening_root, &before_fingerprint, &after_fingerprint);
    let matching_output = weakening_root.join("target/cargo-allow/weakening-matching.json");
    let matching = run_diff_with_note_requirement(
        &weakening_root,
        &weakening_base,
        &weakening_head,
        &matching_output,
    );
    assert_status("weakening with exact note", &matching, true);
    assert_saved_json_artifact(
        &matching_output,
        "weakening with exact note",
        "cargo-allow.report.v1",
        "diff",
    );
    assert_eq!(policy_changes(&matching_output).len(), 1);
    fs::remove_file(&template_path).unwrap_or_else(|err| {
        std::panic::panic_any(format!("remove completed template fixture: {err}"))
    });

    write_revision_note(&weakening_root, "sha256:v1:stale", &after_fingerprint);
    let stale_output = weakening_root.join("target/cargo-allow/weakening-stale.json");
    let stale = run_diff_with_note_requirement(
        &weakening_root,
        &weakening_base,
        &weakening_head,
        &stale_output,
    );
    assert_status("weakening with stale note", &stale, false);
    let stale_rendered = run_diff_rendered(&weakening_root, "human", true);
    assert_status("weakening with stale note (human)", &stale_rendered, false);
    assert!(
        String::from_utf8_lossy(&stale_rendered.stderr).contains("change note required"),
        "stale fingerprint should reopen the note obligation"
    );
    remove_temp_root(weakening_root);

    let improvement_root = create_policy_change_fixture("policy-improvement", "src/lib.rs");
    let improvement_base = policy_with_occurrence_limit(3);
    fs::write(
        improvement_root.join("policy/allow.toml"),
        &improvement_base,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write improvement base policy: {err}")));
    git(&improvement_root, &["add", "policy/allow.toml"]);
    git(
        &improvement_root,
        &["commit", "--no-gpg-sign", "-m", "set improvement base"],
    );
    let improvement_head = policy_with_occurrence_limit(1);
    fs::write(
        improvement_root.join("policy/allow.toml"),
        &improvement_head,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write improvement policy: {err}")));
    let improvement_output = improvement_root.join("target/cargo-allow/improvement.json");
    let improvement = run_diff(&improvement_root, &improvement_output, false);
    assert_status("policy improvement", &improvement, true);
    assert_saved_json_artifact(
        &improvement_output,
        "policy improvement",
        "cargo-allow.report.v1",
        "diff",
    );
    assert_eq!(policy_changes(&improvement_output).len(), 1);
    assert_policy_change_fields(
        &improvement_output,
        "occurrence_limit_tightened",
        "allow-transition",
        "improvement",
        "retained",
        "improved",
    );
    remove_temp_root(improvement_root);

    let formatting_root = create_policy_change_fixture("policy-formatting", "src/lib.rs");
    let formatting_output = formatting_root.join("target/cargo-allow/formatting.json");
    let formatting_policy = policy_with_scope("src/lib.rs").replace(
        "reason = \"fixture transition\"",
        "reason = \"fixture transition\"\n\n",
    );
    fs::write(formatting_root.join("policy/allow.toml"), formatting_policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write formatting policy: {err}")));
    let formatting = run_diff(&formatting_root, &formatting_output, true);
    assert_status("formatting-only policy edit", &formatting, true);
    assert_eq!(policy_changes(&formatting_output).len(), 0);
    remove_temp_root(formatting_root);
}

#[test]
fn policy_change_projections_agree_across_human_markdown_and_json() {
    // #2248: one weakening and one improvement must be classified once and
    // projected identically across the human, markdown, and json renders — and
    // the improvement must not raise a weakening change-note obligation. The
    // existing note-contract test asserts only the json projection, so this
    // pins the human/markdown counts and identities to the same transition.

    // --- Weakening: occurrence_limit 1 -> 3 ---
    let weakening_root = create_policy_change_fixture("policy-projection-weakening", "src/lib.rs");
    fs::write(
        weakening_root.join("policy/allow.toml"),
        policy_with_occurrence_limit(1),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write weakening base: {err}")));
    git(&weakening_root, &["add", "policy/allow.toml"]);
    git(
        &weakening_root,
        &["commit", "--no-gpg-sign", "-m", "weakening base"],
    );
    fs::write(
        weakening_root.join("policy/allow.toml"),
        policy_with_occurrence_limit(3),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write weakening head: {err}")));

    // json is the projection the corpus already trusts: exactly one worsened
    // occurrence_limit_loosened transition.
    let weakening_json = weakening_root.join("target/cargo-allow/projection-weakening.json");
    let weakening = run_diff(&weakening_root, &weakening_json, false);
    assert_status("weakening json", &weakening, false);
    assert_eq!(
        policy_changes(&weakening_json).len(),
        1,
        "weakening json must record exactly one policy change"
    );
    assert_policy_change_fields(
        &weakening_json,
        "occurrence_limit_loosened",
        "allow-transition",
        "fail",
        "retained",
        "worsened",
    );

    // markdown must describe the same single transition, count, and posture.
    let weakening_md = run_diff_rendered(&weakening_root, "markdown", false);
    let weakening_md_text = String::from_utf8_lossy(&weakening_md.stdout);
    assert!(
        weakening_md_text.contains("`occurrence_limit_loosened`")
            && weakening_md_text.contains("`allow-transition`"),
        "markdown must name the weakening transition identity: {weakening_md_text}"
    );
    assert!(
        weakening_md_text.contains("| Policy failures | 1 |"),
        "markdown count must agree with the single json weakening: {weakening_md_text}"
    );
    assert!(
        weakening_md_text.contains("**Net posture:** `worse`"),
        "markdown net posture must agree with the json worsened delta: {weakening_md_text}"
    );
    assert!(
        !weakening_md_text.contains("occurrence_limit_tightened"),
        "a weakening must never render as an improvement: {weakening_md_text}"
    );

    // human must describe the same single transition and posture.
    let weakening_human = run_diff_rendered(&weakening_root, "human", false);
    let weakening_human_text = String::from_utf8_lossy(&weakening_human.stdout);
    assert!(
        weakening_human_text.contains("occurrence_limit_loosened")
            && weakening_human_text.contains("allow-transition"),
        "human must name the weakening transition identity: {weakening_human_text}"
    );
    assert!(
        weakening_human_text.contains("net_posture: worse")
            && weakening_human_text.contains("posture_delta=worsened"),
        "human net posture must agree with the json worsened delta: {weakening_human_text}"
    );
    assert!(
        !weakening_human_text.contains("occurrence_limit_tightened"),
        "a weakening must never render as an improvement: {weakening_human_text}"
    );
    remove_temp_root(weakening_root);

    // --- Improvement: occurrence_limit 3 -> 1 ---
    let improvement_root =
        create_policy_change_fixture("policy-projection-improvement", "src/lib.rs");
    fs::write(
        improvement_root.join("policy/allow.toml"),
        policy_with_occurrence_limit(3),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write improvement base: {err}")));
    git(&improvement_root, &["add", "policy/allow.toml"]);
    git(
        &improvement_root,
        &["commit", "--no-gpg-sign", "-m", "improvement base"],
    );
    fs::write(
        improvement_root.join("policy/allow.toml"),
        policy_with_occurrence_limit(1),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write improvement head: {err}")));

    let improvement_json = improvement_root.join("target/cargo-allow/projection-improvement.json");
    let improvement = run_diff(&improvement_root, &improvement_json, false);
    assert_status("improvement json", &improvement, true);
    assert_eq!(
        policy_changes(&improvement_json).len(),
        1,
        "improvement json must record exactly one policy change"
    );
    assert_policy_change_fields(
        &improvement_json,
        "occurrence_limit_tightened",
        "allow-transition",
        "improvement",
        "retained",
        "improved",
    );

    let improvement_md = run_diff_rendered(&improvement_root, "markdown", false);
    let improvement_md_text = String::from_utf8_lossy(&improvement_md.stdout);
    assert!(
        improvement_md_text.contains("`occurrence_limit_tightened`")
            && improvement_md_text.contains("`allow-transition`"),
        "markdown must name the improvement transition identity: {improvement_md_text}"
    );
    assert!(
        improvement_md_text.contains("| Policy improvements | 1 |"),
        "markdown improvement count must agree with the single json record: {improvement_md_text}"
    );
    assert!(
        improvement_md_text.contains("**Net posture:** `improved`"),
        "markdown net posture must agree with the json improved delta: {improvement_md_text}"
    );
    assert!(
        !improvement_md_text.contains("occurrence_limit_loosened")
            && !improvement_md_text.contains("### Policy Failures"),
        "an improvement must never render a false weakening: {improvement_md_text}"
    );

    let improvement_human = run_diff_rendered(&improvement_root, "human", false);
    let improvement_human_text = String::from_utf8_lossy(&improvement_human.stdout);
    assert!(
        improvement_human_text.contains("occurrence_limit_tightened")
            && improvement_human_text.contains("allow-transition"),
        "human must name the improvement transition identity: {improvement_human_text}"
    );
    assert!(
        improvement_human_text.contains("net_posture: improved")
            && improvement_human_text.contains("posture_delta=improved"),
        "human net posture must agree with the json improved delta: {improvement_human_text}"
    );
    assert!(
        !improvement_human_text.contains("occurrence_limit_loosened"),
        "an improvement must never render a false weakening: {improvement_human_text}"
    );

    // The improvement must not demand a weakening change note: --require-change-note
    // only gates worsened/review transitions, so this still passes cleanly.
    let improvement_gated =
        improvement_root.join("target/cargo-allow/projection-improvement-gated.json");
    let gated = run_diff(&improvement_root, &improvement_gated, true);
    assert_status("improvement under --require-change-note", &gated, true);
    assert!(
        !String::from_utf8_lossy(&gated.stderr).contains("change note required"),
        "an improvement must not raise a change-note obligation: {}",
        String::from_utf8_lossy(&gated.stderr)
    );
    assert_eq!(
        policy_changes(&improvement_gated).len(),
        1,
        "the gated improvement must still record its single improvement"
    );
    remove_temp_root(improvement_root);
}

#[test]
fn lifecycle_statuses_converge_across_read_artifacts() {
    let root = create_fixture("lifecycle-corpus", true);

    let (list_path, list_result) = run_report(&root, "list", &["list"]);
    assert_status("list", &list_result, true);
    assert_quiet("list", &list_result);
    let list = assert_saved_json_artifact(&list_path, "list", "cargo-allow.list.v1", "list");
    assert_entry_status(&list, "/allow_entries", EXPIRED_ID, "expired");
    assert_entry_status(&list, "/allow_entries", REVIEW_DUE_ID, "review_due");
    assert_entry_status(&list, "/allow_entries", STALE_ID, "stale");
    assert_entry_status(&list, "/allow_entries", DRIFT_ID, "location_drift");
    assert_entry_matches(&list, HEADROOM_ID, 2);
    assert_entry_status(&list, "/allow_entries", MISSING_EVIDENCE_ID, "matched");
    assert_entry_count(&list, MISSING_EVIDENCE_ID, "evidence_count", 0);
    assert_entry_count(&list, BROKEN_EVIDENCE_ID, "broken_evidence_references", 1);
    assert_entry_count(&list, WEAK_EVIDENCE_ID, "weak_evidence_references", 1);
    assert_entry_status(&list, "/allow_entries", BASELINE_DEBT_ID, "baseline_debt");

    let (expired_path, expired_result) =
        run_report(&root, "explain-expired", &["explain", EXPIRED_ID]);
    assert_status("explain expired", &expired_result, true);
    assert_quiet("explain expired", &expired_result);
    let expired = assert_saved_json_artifact(
        &expired_path,
        "explain expired",
        "cargo-allow.explain.v1",
        "explain",
    );
    assert_explain_status(&expired, EXPIRED_ID, "expired");

    let (review_path, review_result) =
        run_report(&root, "explain-review", &["explain", REVIEW_DUE_ID]);
    assert_status("explain review", &review_result, true);
    assert_quiet("explain review", &review_result);
    let review = assert_saved_json_artifact(
        &review_path,
        "explain review",
        "cargo-allow.explain.v1",
        "explain",
    );
    assert_explain_status(&review, REVIEW_DUE_ID, "review_due");

    let (headroom_path, headroom_result) =
        run_report(&root, "explain-headroom", &["explain", HEADROOM_ID]);
    assert_status("explain headroom", &headroom_result, true);
    assert_quiet("explain headroom", &headroom_result);
    let headroom = assert_saved_json_artifact(
        &headroom_path,
        "explain headroom",
        "cargo-allow.explain.v1",
        "explain",
    );
    assert_explain_status(&headroom, HEADROOM_ID, "matched");
    assert_eq!(
        headroom
            .pointer("/allow_entry/occurrence_limit")
            .and_then(Value::as_u64),
        Some(3),
        "explain should expose the configured occurrence limit"
    );
    assert_eq!(
        headroom
            .pointer("/summary/current_matches")
            .and_then(Value::as_u64),
        Some(2),
        "explain should expose the current matched count"
    );

    let (missing_evidence_path, missing_evidence_result) = run_report(
        &root,
        "explain-missing-evidence",
        &["explain", MISSING_EVIDENCE_ID],
    );
    assert_status("explain missing evidence", &missing_evidence_result, true);
    assert_quiet("explain missing evidence", &missing_evidence_result);
    let missing_evidence = assert_saved_json_artifact(
        &missing_evidence_path,
        "explain missing evidence",
        "cargo-allow.explain.v1",
        "explain",
    );
    assert_explain_status(&missing_evidence, MISSING_EVIDENCE_ID, "matched");
    assert_eq!(
        missing_evidence
            .pointer("/evidence_references")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0),
        "explain should show that the entry has no evidence references"
    );

    let (baseline_path, baseline_result) = run_report(
        &root,
        "explain-baseline-debt",
        &["explain", BASELINE_DEBT_ID],
    );
    assert_status("explain baseline debt", &baseline_result, true);
    assert_quiet("explain baseline debt", &baseline_result);
    let baseline = assert_saved_json_artifact(
        &baseline_path,
        "explain baseline debt",
        "cargo-allow.explain.v1",
        "explain",
    );
    assert_explain_status(&baseline, BASELINE_DEBT_ID, "baseline_debt");

    for (allow_id, evidence_status) in [
        (BROKEN_EVIDENCE_ID, "local_file_missing"),
        (WEAK_EVIDENCE_ID, "unstructured"),
    ] {
        let (path, result) = run_report(
            &root,
            &format!("explain-{allow_id}"),
            &["explain", allow_id],
        );
        assert_status(&format!("explain {allow_id}"), &result, true);
        assert_quiet(&format!("explain {allow_id}"), &result);
        let explanation = assert_saved_json_artifact(
            &path,
            &format!("explain {allow_id}"),
            "cargo-allow.explain.v1",
            "explain",
        );
        assert_explain_evidence_status(&explanation, evidence_status);
    }

    for (allow_id, status) in [(STALE_ID, "stale"), (DRIFT_ID, "location_drift")] {
        let (path, result) = run_report(
            &root,
            &format!("explain-{allow_id}"),
            &["explain", allow_id],
        );
        assert_status(&format!("explain {allow_id}"), &result, true);
        assert_quiet(&format!("explain {allow_id}"), &result);
        let explanation = assert_saved_json_artifact(
            &path,
            &format!("explain {allow_id}"),
            "cargo-allow.explain.v1",
            "explain",
        );
        assert_explain_status(&explanation, allow_id, status);
    }

    let (worklist_path, worklist_result) = run_report(&root, "worklist", &["worklist"]);
    assert_status("worklist", &worklist_result, true);
    assert_quiet("worklist", &worklist_result);
    let worklist = assert_saved_json_artifact(
        &worklist_path,
        "worklist",
        "cargo-allow.worklist.v1",
        "worklist",
    );
    assert_entry_status(&worklist, "/work_items", EXPIRED_ID, "expired");
    assert_entry_status(&worklist, "/work_items", REVIEW_DUE_ID, "review_due");
    assert_entry_status(&worklist, "/work_items", STALE_ID, "stale");
    assert_entry_status(&worklist, "/work_items", DRIFT_ID, "location_drift");
    assert_work_item_kind(&worklist, HEADROOM_ID, "occurrence_headroom");
    assert_work_item_message(&worklist, HEADROOM_ID, "1 remaining");
    assert_work_item_kind(&worklist, MISSING_EVIDENCE_ID, "missing_evidence");
    assert_work_item_kind(&worklist, BROKEN_EVIDENCE_ID, "broken_evidence_link");
    assert_work_item_kind(&worklist, WEAK_EVIDENCE_ID, "weak_evidence_reference");
    assert_work_item_kind(&worklist, BASELINE_DEBT_ID, "baseline_debt");

    for (command, args, should_succeed) in [
        ("audit", AUDIT_ARGS, true),
        ("check", CHECK_NO_NEW_ARGS, false),
        ("diff", DIFF_ARGS, false),
    ] {
        let (path, result) = run_report(&root, command, args);
        assert_status(command, &result, should_succeed);
        assert_quiet(command, &result);
        let report = assert_saved_json_artifact(&path, command, "cargo-allow.report.v1", command);
        assert_entry_status(&report, "/outcomes", EXPIRED_ID, "expired");
        assert_entry_status(&report, "/outcomes", REVIEW_DUE_ID, "review_due");
        assert_entry_status(&report, "/outcomes", STALE_ID, "stale");
        assert_entry_status(&report, "/outcomes", DRIFT_ID, "location_drift");
        assert_report_advisory_count(&report, "occurrence_headroom", 1);
        assert_report_advisory_count(&report, "policy_missing_evidence", 1);
        assert_report_advisory_count(&report, "broken_evidence_links", 1);
        assert_report_advisory_count(&report, "weak_evidence_references", 1);
        assert_report_advisory_count(&report, "baseline_debt", 1);
        assert_report_summary_count(&report, "policy_baseline_debt", 1);
    }

    remove_temp_root(root);
}

#[test]
fn stale_is_blocking_only_in_strict_while_location_drift_is_advisory() {
    let root = create_fixture("stale-drift-mode", false);
    fs::write(
        root.join("src/lib.rs"),
        "pub fn relocate(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write stale/drift source: {err}")));
    fs::write(root.join("policy/allow.toml"), stale_drift_policy())
        .unwrap_or_else(|err| std::panic::panic_any(format!("write stale/drift policy: {err}")));

    let no_new_output = root.join("target/cargo-allow/no-new.json");
    let no_new = run_command(&root, &["check", "--mode", "no-new"], &no_new_output);
    assert_status("stale/drift no-new", &no_new, true);
    assert_quiet("stale/drift no-new", &no_new);
    let no_new_report = assert_saved_json_artifact(
        &no_new_output,
        "stale/drift no-new",
        "cargo-allow.report.v1",
        "check",
    );
    assert_eq!(
        no_new_report.pointer("/failed").and_then(Value::as_bool),
        Some(false),
        "stale and location_drift are advisory in no-new mode"
    );
    assert_entry_status(&no_new_report, "/outcomes", STALE_ID, "stale");
    assert_entry_status(&no_new_report, "/outcomes", DRIFT_ID, "location_drift");

    let strict_output = root.join("target/cargo-allow/strict.json");
    let strict = run_command(&root, &["check", "--mode", "strict"], &strict_output);
    assert_status("stale/drift strict", &strict, false);
    assert_quiet("stale/drift strict", &strict);
    let strict_report = assert_saved_json_artifact(
        &strict_output,
        "stale/drift strict",
        "cargo-allow.report.v1",
        "check",
    );
    assert_eq!(
        strict_report.pointer("/failed").and_then(Value::as_bool),
        Some(true),
        "stale is blocking in strict mode"
    );
    assert_entry_status(&strict_report, "/outcomes", STALE_ID, "stale");
    assert_entry_status(&strict_report, "/outcomes", DRIFT_ID, "location_drift");

    remove_temp_root(root);
}

#[test]
fn review_due_is_advisory_in_no_new_and_blocking_in_strict() {
    let root = create_fixture("review-due-mode", false);

    let no_new_output = root.join("target/cargo-allow/no-new.json");
    let no_new = run_command(&root, &["check", "--mode", "no-new"], &no_new_output);
    assert_status("review-due no-new", &no_new, true);
    assert_quiet("review-due no-new", &no_new);
    let no_new_report = assert_saved_json_artifact(
        &no_new_output,
        "review-due no-new",
        "cargo-allow.report.v1",
        "check",
    );
    assert_eq!(
        no_new_report.pointer("/failed").and_then(Value::as_bool),
        Some(false),
        "review_due is advisory in no-new mode"
    );
    assert_entry_status(&no_new_report, "/outcomes", REVIEW_DUE_ID, "review_due");

    let strict_output = root.join("target/cargo-allow/strict.json");
    let strict = run_command(&root, &["check", "--mode", "strict"], &strict_output);
    assert_status("review-due strict", &strict, false);
    assert_quiet("review-due strict", &strict);
    let strict_report = assert_saved_json_artifact(
        &strict_output,
        "review-due strict",
        "cargo-allow.report.v1",
        "check",
    );
    assert_eq!(
        strict_report.pointer("/failed").and_then(Value::as_bool),
        Some(true),
        "review_due is blocking in strict mode"
    );
    assert_entry_status(&strict_report, "/outcomes", REVIEW_DUE_ID, "review_due");

    remove_temp_root(root);
}

#[test]
fn baseline_debt_is_advisory_in_no_new_and_blocking_in_strict() {
    let root = create_fixture("baseline-debt-mode", false);
    fs::write(
        root.join("src/lib.rs"),
        "pub fn baseline(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write baseline source: {err}")));
    fs::write(root.join("policy/allow.toml"), baseline_debt_policy())
        .unwrap_or_else(|err| std::panic::panic_any(format!("write baseline policy: {err}")));

    let no_new_output = root.join("target/cargo-allow/no-new.json");
    let no_new = run_command(&root, &["check", "--mode", "no-new"], &no_new_output);
    assert_status("baseline debt no-new", &no_new, true);
    assert_quiet("baseline debt no-new", &no_new);
    let no_new_report = assert_saved_json_artifact(
        &no_new_output,
        "baseline debt no-new",
        "cargo-allow.report.v1",
        "check",
    );
    assert_eq!(
        no_new_report.pointer("/failed").and_then(Value::as_bool),
        Some(false),
        "baseline debt is advisory in no-new mode"
    );
    assert_entry_status(&no_new_report, "/outcomes", BASELINE_DEBT_ID, "matched");

    let strict_output = root.join("target/cargo-allow/strict.json");
    let strict = run_command(&root, &["check", "--mode", "strict"], &strict_output);
    assert_status("baseline debt strict", &strict, false);
    assert_quiet("baseline debt strict", &strict);
    let strict_report = assert_saved_json_artifact(
        &strict_output,
        "baseline debt strict",
        "cargo-allow.report.v1",
        "check",
    );
    assert_eq!(
        strict_report.pointer("/failed").and_then(Value::as_bool),
        Some(true),
        "baseline debt is blocking in strict mode"
    );
    assert_entry_status(
        &strict_report,
        "/outcomes",
        BASELINE_DEBT_ID,
        "baseline_debt",
    );

    remove_temp_root(root);
}

#[test]
fn mirror_divergence_projects_across_check_worklist_and_doctor() {
    let root = create_mirror_divergence_fixture();
    let check_output = root.join("target/cargo-allow/check.json");
    let check_receipt = root.join("target/cargo-allow/check.receipt.json");
    let check = run_command_with_receipt(&root, CHECK_NO_NEW_ARGS, &check_output, &check_receipt);
    assert_status("mirror divergence no-new", &check, true);
    assert_quiet("mirror divergence no-new", &check);
    let check_report = assert_saved_json_artifact(
        &check_output,
        "mirror divergence no-new",
        "cargo-allow.report.v1",
        "check",
    );
    assert_eq!(
        check_report.pointer("/failed").and_then(Value::as_bool),
        Some(false),
        "mirror divergence remains advisory in no-new mode"
    );

    let check_receipt = assert_saved_json_artifact(
        &check_receipt,
        "mirror divergence receipt",
        allow_report::RECEIPT_SCHEMA_ID,
        "check",
    );
    assert_eq!(
        check_receipt
            .pointer("/federation/divergence_summary/counts_by_kind/0/kind")
            .and_then(Value::as_str),
        Some("mirror_divergence"),
        "check receipt should retain the divergence kind"
    );
    assert_eq!(
        check_receipt
            .pointer("/federation/divergence_summary/counts_by_kind/0/count")
            .and_then(Value::as_u64),
        Some(1),
        "check receipt should retain the divergence count"
    );
    assert_eq!(
        check_receipt
            .pointer("/advisory/mirror_divergence")
            .and_then(Value::as_u64),
        Some(1),
        "check receipt should report mirror divergence as advisory"
    );

    let (worklist_path, worklist_result) = run_report(&root, "mirror-worklist", &["worklist"]);
    assert_status("mirror divergence worklist", &worklist_result, true);
    assert_quiet("mirror divergence worklist", &worklist_result);
    let worklist = assert_saved_json_artifact(
        &worklist_path,
        "mirror divergence worklist",
        "cargo-allow.worklist.v1",
        "worklist",
    );
    assert_work_item_ledger(
        &worklist,
        "mirror_divergence",
        MIRROR_LEDGER_ID,
        MIRROR_LEDGER_PATH,
        "mirror",
        "advisory",
    );

    let (doctor_path, doctor_result) = run_report(&root, "mirror-doctor", &["doctor"]);
    assert_status("mirror divergence doctor", &doctor_result, true);
    assert_quiet("mirror divergence doctor", &doctor_result);
    let doctor = assert_saved_json_artifact(
        &doctor_path,
        "mirror divergence doctor",
        "cargo-allow.doctor.v1",
        "doctor",
    );
    assert_eq!(
        doctor
            .pointer("/federation/divergences/0/kind")
            .and_then(Value::as_str),
        Some("mirror_divergence"),
        "doctor should report the runtime divergence kind"
    );
    let ledger_ids = doctor
        .pointer("/federation/divergences/0/ledger_ids")
        .and_then(Value::as_array)
        .unwrap_or_else(|| {
            std::panic::panic_any("doctor divergence ledger IDs should be an array")
        });
    assert!(
        ledger_ids
            .iter()
            .any(|id| id.as_str() == Some(MIRROR_LEDGER_ID)),
        "doctor should retain mirror ledger provenance: {doctor}"
    );

    remove_temp_root(root);
}

fn create_fixture(label: &str, include_expired: bool) -> PathBuf {
    let root = temp_root(label);
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create source directory: {err}")));
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy directory: {err}")));
    let source = if include_expired {
        "pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\npub fn reload(value: Option<u8>) -> u8 { value.unwrap() }\npub fn relocate(value: Option<u8>) -> u8 { value.unwrap() }\npub fn reserve(value: Option<u8>) -> u8 { let first = value.unwrap(); first + value.unwrap() }\npub fn missing_evidence(value: Option<u8>) -> u8 { value.unwrap() }\npub fn broken_evidence(value: Option<u8>) -> u8 { value.unwrap() }\npub fn weak_evidence(value: Option<u8>) -> u8 { value.unwrap() }\npub fn baseline(value: Option<u8>) -> u8 { value.unwrap() }\n"
    } else {
        "pub fn reload(value: Option<u8>) -> u8 { value.unwrap() }\n"
    };
    fs::write(root.join("src/lib.rs"), source)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write source fixture: {err}")));
    fs::write(root.join("policy/allow.toml"), policy(include_expired))
        .unwrap_or_else(|err| std::panic::panic_any(format!("write policy fixture: {err}")));

    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "."]);
    git(
        &root,
        &["commit", "--no-gpg-sign", "-m", "lifecycle corpus fixture"],
    );
    root
}

fn create_mirror_divergence_fixture() -> PathBuf {
    let root = temp_root("mirror-divergence");
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create source directory: {err}")));
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy directory: {err}")));
    fs::create_dir_all(root.join(".allow/mirror"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create mirror directory: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write source fixture: {err}")));
    fs::write(root.join("policy/allow.toml"), mirror_canonical_policy())
        .unwrap_or_else(|err| std::panic::panic_any(format!("write canonical policy: {err}")));
    fs::write(
        root.join(MIRROR_LEDGER_PATH),
        "schema_version = \"0.1\"\npolicy = \"cargo-allow\"\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write mirror policy: {err}")));
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/federation/canonical-mirror-drain-config.toml"),
        root.join(".allow/config.toml"),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("copy federation config: {err}")));

    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "."]);
    git(
        &root,
        &["commit", "--no-gpg-sign", "-m", "mirror divergence fixture"],
    );
    root
}

fn create_policy_change_fixture(label: &str, base_scope: &str) -> PathBuf {
    let root = temp_root(label);
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create source directory: {err}")));
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create policy directory: {err}")));
    fs::write(
        root.join("src/lib.rs"),
        "pub fn load(value: Option<u8>) -> u8 { value.unwrap() }\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("write transition source: {err}")));
    let policy = if base_scope.contains('*') {
        policy_with_glob(base_scope)
    } else {
        policy_with_scope(base_scope)
    };
    fs::write(root.join("policy/allow.toml"), policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write transition policy: {err}")));
    git(&root, &["init"]);
    git(
        &root,
        &["config", "user.email", "cargo-allow@example.invalid"],
    );
    git(&root, &["config", "user.name", "cargo-allow test"]);
    git(&root, &["add", "."]);
    git(
        &root,
        &["commit", "--no-gpg-sign", "-m", "policy transition fixture"],
    );
    root
}

fn policy_with_scope(scope: &str) -> String {
    transition_policy(&format!("path = \"{scope}\""))
}

fn policy_with_glob(glob: &str) -> String {
    transition_policy(&format!("glob = \"{glob}\""))
}

fn policy_with_occurrence_limit(limit: u32) -> String {
    policy_with_scope("src/lib.rs").replace(
        "reason = \"fixture transition\"",
        &format!("occurrence_limit = {limit}\nreason = \"fixture transition\""),
    )
}

fn transition_policy(scope: &str) -> String {
    format!(
        r#"schema_version = "0.1"
policy = "cargo-allow"

[workspace]
ignored = ["policy/**", "target/**"]

[[allow]]
id = "allow-transition"
kind = "panic"
family = "unwrap"
{scope}
owner = "core"
classification = "reviewed_exception"
reason = "fixture transition"
evidence = ["test:lifecycle_corpus"]
created = "2026-06-01"
review_after = "2099-01-01"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"
"#
    )
}

fn transition_fingerprints(base_policy: &str, head_policy: &str) -> (String, String) {
    let base = allow_policy::parse_policy(base_policy).unwrap_or_else(|err| {
        std::panic::panic_any(format!("parse base transition policy: {err}"))
    });
    let head = allow_policy::parse_policy(head_policy).unwrap_or_else(|err| {
        std::panic::panic_any(format!("parse head transition policy: {err}"))
    });
    let base_entry = base
        .allow
        .first()
        .unwrap_or_else(|| std::panic::panic_any("base transition policy has no entry"));
    let head_entry = head
        .allow
        .first()
        .unwrap_or_else(|| std::panic::panic_any("head transition policy has no entry"));
    (
        allow_entry_content_fingerprint(base_entry),
        allow_entry_content_fingerprint(head_entry),
    )
}

fn write_revision_note(root: &Path, before_fingerprint: &str, after_fingerprint: &str) {
    let note = format!(
        r#"[[records]]
allow_ids = ["allow-transition"]
change_kinds = ["occurrence_limit_loosened"]
before_fingerprint = "{before_fingerprint}"
after_fingerprint = "{after_fingerprint}"
"#
    );
    fs::write(root.join(".allow/revisions/transition.toml"), note)
        .unwrap_or_else(|err| std::panic::panic_any(format!("write revision note: {err}")));
}

fn run_diff_with_note_requirement(
    root: &Path,
    _base_policy: &str,
    _head_policy: &str,
    output: &Path,
) -> Output {
    run_diff(root, output, true)
}

fn run_diff_with_template(
    root: &Path,
    _base_policy: &str,
    _head_policy: &str,
    output: &Path,
    template: &Path,
) -> Output {
    let mut command = cargo_allow_command();
    command
        .args(["diff", "--base", "HEAD"])
        .arg("--root")
        .arg(root)
        .arg("--config")
        .arg(root.join("policy/allow.toml"))
        .args(["--format", "json", "--output"])
        .arg(output)
        .args([
            "--require-change-note",
            "--revisions-dir",
            ".allow/revisions",
        ])
        .arg("--write-change-note-template")
        .arg(template);
    command
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run template diff: {err}")))
}

fn run_diff(root: &Path, output: &Path, require_change_note: bool) -> Output {
    let mut command = cargo_allow_command();
    command
        .args(["diff", "--base", "HEAD"])
        .arg("--root")
        .arg(root)
        .arg("--config")
        .arg(root.join("policy/allow.toml"))
        .args(["--format", "json", "--output"])
        .arg(output);
    if require_change_note {
        command
            .arg("--require-change-note")
            .args(["--revisions-dir", ".allow/revisions"]);
    }
    command
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run transition diff: {err}")))
}

/// Run `diff` in a non-JSON render format, capturing stdout. Unlike
/// [`run_diff`], no `--output` file is written, so the human/markdown render is
/// returned on stdout for cross-format agreement checks.
fn run_diff_rendered(root: &Path, format: &str, require_change_note: bool) -> Output {
    let mut command = cargo_allow_command();
    command
        .args(["diff", "--base", "HEAD"])
        .arg("--root")
        .arg(root)
        .arg("--config")
        .arg(root.join("policy/allow.toml"))
        .args(["--format", format]);
    if require_change_note {
        command
            .arg("--require-change-note")
            .args(["--revisions-dir", ".allow/revisions"]);
    }
    command
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run {format} diff: {err}")))
}

fn policy_changes(output: &Path) -> Vec<Value> {
    let text = fs::read_to_string(output)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read diff output: {err}")));
    let value: Value = serde_json::from_str(&text)
        .unwrap_or_else(|err| std::panic::panic_any(format!("parse diff output: {err}")));
    value
        .pointer("/diff/policy_changes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_else(|| std::panic::panic_any("diff policy_changes should be an array"))
}

fn assert_policy_change_fields(
    output: &Path,
    kind: &str,
    allow_id: &str,
    severity: &str,
    movement: &str,
    posture_delta: &str,
) {
    let changes = policy_changes(output);
    let change = changes
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one policy change"));
    assert_eq!(change.get("kind").and_then(Value::as_str), Some(kind));
    assert_eq!(
        change.get("allow_id").and_then(Value::as_str),
        Some(allow_id)
    );
    assert_eq!(
        change.get("severity").and_then(Value::as_str),
        Some(severity)
    );
    assert_eq!(
        change.get("movement").and_then(Value::as_str),
        Some(movement)
    );
    assert_eq!(
        change.get("posture_delta").and_then(Value::as_str),
        Some(posture_delta)
    );
}

fn mirror_canonical_policy() -> &'static str {
    r#"schema_version = "0.1"
policy = "cargo-allow"
owner = "core/policy"
status = "active"

[workspace]
root = "."
inventory = "git-tracked"
ignored = [".allow/**", "policy/**", "target/**"]
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

[[allow]]
id = "canonical-only"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "The canonical fixture entry is intentionally absent from the mirror."
evidence = ["test:lifecycle_corpus"]
created = "2026-07-14"
review_after = "2099-01-01"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"
"#
}

fn policy(include_expired: bool) -> String {
    let expired = if include_expired {
        format!(
            r#"
[[allow]]
id = "{EXPIRED_ID}"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "The fixture intentionally unwraps after callers provide Some values."
evidence = ["test:lifecycle_corpus"]
created = "2019-01-01"
expires = "2020-01-01"

[allow.selector]
ast_kind = "method_call"
container = "load"
callee = "unwrap"
"#
        )
    } else {
        String::new()
    };
    let additional = if include_expired {
        format!(
            r#"
[[allow]]
id = "{STALE_ID}"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "The fixture keeps one stale policy entry for lifecycle review."
evidence = ["test:lifecycle_corpus"]
created = "2019-01-01"
review_after = "2099-01-01"

[allow.selector]
ast_kind = "method_call"
container = "gone"
callee = "unwrap"

[[allow]]
id = "{DRIFT_ID}"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "The fixture moves one allow entry away from its recorded location."
evidence = ["test:lifecycle_corpus"]
created = "2019-01-01"
review_after = "2099-01-01"

[allow.selector]
ast_kind = "method_call"
container = "relocate"
callee = "unwrap"

[allow.last_seen]
line = 99
column = 1

[[allow]]
id = "{HEADROOM_ID}"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "The fixture reserves one additional occurrence for a later match."
evidence = ["test:lifecycle_corpus"]
occurrence_limit = 3
created = "2019-01-01"
review_after = "2099-01-01"

[allow.selector]
ast_kind = "method_call"
container = "reserve"
callee = "unwrap"

[[allow]]
id = "{MISSING_EVIDENCE_ID}"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "The fixture keeps one matched entry without evidence for lifecycle repair."
created = "2019-01-01"
review_after = "2099-01-01"

[allow.selector]
ast_kind = "method_call"
container = "missing_evidence"
callee = "unwrap"

[[allow]]
id = "{BROKEN_EVIDENCE_ID}"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "The fixture keeps one broken local evidence reference for lifecycle repair."
evidence = ["doc:docs/missing-evidence.md"]
created = "2019-01-01"
review_after = "2099-01-01"

[allow.selector]
ast_kind = "method_call"
container = "broken_evidence"
callee = "unwrap"

[[allow]]
id = "{WEAK_EVIDENCE_ID}"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "The fixture keeps one weak evidence reference for lifecycle repair."
evidence = ["spreadsheet:manual-review"]
created = "2019-01-01"
review_after = "2099-01-01"

[allow.selector]
ast_kind = "method_call"
container = "weak_evidence"
callee = "unwrap"

[[allow]]
id = "{BASELINE_DEBT_ID}"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "generated"
classification = "baseline_debt"
reason = "The fixture keeps generated baseline debt for human review."
created = "2026-07-14"
expires = "2026-10-01"

[allow.selector]
ast_kind = "method_call"
container = "baseline"
callee = "unwrap"
"#
        )
    } else {
        String::new()
    };
    format!(
        r#"schema_version = "0.1"
policy = "cargo-allow"
owner = "core/policy"
status = "active"

[workspace]
root = "."
inventory = "git-tracked"
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
{expired}
{additional}
[[allow]]
id = "{REVIEW_DUE_ID}"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "The fixture intentionally unwraps after callers provide Some values."
evidence = ["test:lifecycle_corpus"]
created = "2019-01-01"
review_after = "2020-01-01"

[allow.selector]
ast_kind = "method_call"
container = "reload"
callee = "unwrap"
"#
    )
}

fn stale_drift_policy() -> String {
    format!(
        r#"schema_version = "0.1"
policy = "cargo-allow"
owner = "core/policy"
status = "active"

[workspace]
root = "."
inventory = "git-tracked"
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

[[allow]]
id = "{STALE_ID}"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "The fixture keeps one stale policy entry for lifecycle review."
evidence = ["test:lifecycle_corpus"]
created = "2019-01-01"
review_after = "2099-01-01"

[allow.selector]
ast_kind = "method_call"
container = "gone"
callee = "unwrap"

[[allow]]
id = "{DRIFT_ID}"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "The fixture moves one allow entry away from its recorded location."
evidence = ["test:lifecycle_corpus"]
created = "2019-01-01"
review_after = "2099-01-01"

[allow.selector]
ast_kind = "method_call"
container = "relocate"
callee = "unwrap"

[allow.last_seen]
line = 99
column = 1
"#
    )
}

fn baseline_debt_policy() -> String {
    format!(
        r#"schema_version = "0.1"
policy = "cargo-allow"
owner = "core/policy"
status = "active"

[workspace]
root = "."
inventory = "git-tracked"
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

[[allow]]
id = "{BASELINE_DEBT_ID}"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "generated"
classification = "baseline_debt"
reason = "Generated baseline debt remains for human review."
created = "2026-07-14"
expires = "2026-10-01"

[allow.selector]
ast_kind = "method_call"
container = "baseline"
callee = "unwrap"
"#
    )
}

fn run_report(root: &Path, name: &str, args: &[&str]) -> (PathBuf, Output) {
    let output = root.join(format!("target/cargo-allow/{name}.json"));
    let result = run_command(root, args, &output);
    (output, result)
}

fn run_report_with_common_summary(
    root: &Path,
    name: &str,
    args: &[&str],
) -> (PathBuf, PathBuf, Output) {
    let output = root.join(format!("target/cargo-allow/{name}.json"));
    let summary = root.join(format!("target/cargo-allow/{name}-common.json"));
    let result = cargo_allow_command()
        .arg("--command-summary-output")
        .arg(&summary)
        .args(args)
        .arg("--root")
        .arg(root)
        .arg("--config")
        .arg(root.join("policy/allow.toml"))
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow {args:?}: {err}")));
    (output, summary, result)
}

fn run_command(root: &Path, args: &[&str], output: &Path) -> Output {
    cargo_allow_command()
        .args(args)
        .arg("--root")
        .arg(root)
        .arg("--config")
        .arg(root.join("policy/allow.toml"))
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(output)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("run cargo-allow {args:?}: {err}")))
}

fn run_command_with_receipt(root: &Path, args: &[&str], output: &Path, receipt: &Path) -> Output {
    cargo_allow_command()
        .args(args)
        .arg("--root")
        .arg(root)
        .arg("--config")
        .arg(root.join("policy/allow.toml"))
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(output)
        .arg("--receipt")
        .arg(receipt)
        .output()
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!("run cargo-allow {args:?} with receipt: {err}"))
        })
}

fn assert_quiet(command: &str, result: &Output) {
    assert_stdout_empty(command, result, "--output should not emit report JSON");
    assert_stderr_empty(command, result, "--output should not emit status text");
}

fn assert_explain_status(value: &Value, allow_id: &str, status: &str) {
    assert_eq!(
        value.pointer("/allow_entry/id").and_then(Value::as_str),
        Some(allow_id),
        "explain should retain the allow ID"
    );
    assert_eq!(
        value
            .pointer("/summary/current_status")
            .and_then(Value::as_str),
        Some(status),
        "{allow_id} explain status"
    );
}

fn assert_entry_status(value: &Value, collection_pointer: &str, allow_id: &str, status: &str) {
    let entries = value
        .pointer(collection_pointer)
        .and_then(Value::as_array)
        .unwrap_or_else(|| {
            std::panic::panic_any(format!("{collection_pointer} should be an array"))
        });
    let entry = entries
        .iter()
        .find(|entry| {
            entry.get("allow_id").and_then(Value::as_str) == Some(allow_id)
                || entry.get("id").and_then(Value::as_str) == Some(allow_id)
        })
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "{allow_id} missing from {collection_pointer}: {entries:?}"
            ))
        });
    assert_eq!(
        entry.get("status").and_then(Value::as_str),
        Some(status),
        "{allow_id} status in {collection_pointer}"
    );
}

fn assert_entry_matches(value: &Value, allow_id: &str, matches: u64) {
    let entries = value
        .pointer("/allow_entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("/allow_entries should be an array"));
    let entry = entries
        .iter()
        .find(|entry| entry.get("id").and_then(Value::as_str) == Some(allow_id))
        .unwrap_or_else(|| {
            std::panic::panic_any(format!("{allow_id} missing from /allow_entries"))
        });
    assert_eq!(
        entry.get("matches").and_then(Value::as_u64),
        Some(matches),
        "{allow_id} current match count"
    );
}

fn assert_entry_count(value: &Value, allow_id: &str, field: &str, count: u64) {
    let entries = value
        .pointer("/allow_entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("/allow_entries should be an array"));
    let entry = entries
        .iter()
        .find(|entry| entry.get("id").and_then(Value::as_str) == Some(allow_id))
        .unwrap_or_else(|| {
            std::panic::panic_any(format!("{allow_id} missing from /allow_entries"))
        });
    assert_eq!(
        entry.get(field).and_then(Value::as_u64),
        Some(count),
        "{allow_id} {field}"
    );
}

fn assert_explain_evidence_status(value: &Value, status: &str) {
    assert_eq!(
        value
            .pointer("/evidence_references/0/status")
            .and_then(Value::as_str),
        Some(status),
        "explain evidence status"
    );
}

fn assert_work_item_kind(value: &Value, allow_id: &str, kind: &str) {
    let items = value
        .pointer("/work_items")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("/work_items should be an array"));
    let item = items
        .iter()
        .find(|item| item.get("allow_id").and_then(Value::as_str) == Some(allow_id))
        .unwrap_or_else(|| std::panic::panic_any(format!("{allow_id} missing from /work_items")));
    assert_eq!(
        item.get("kind").and_then(Value::as_str),
        Some(kind),
        "{allow_id} work item kind"
    );
}

fn assert_work_item_ledger(
    value: &Value,
    kind: &str,
    ledger_id: &str,
    ledger_path: &str,
    role: &str,
    mode: &str,
) {
    let items = value
        .pointer("/work_items")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("/work_items should be an array"));
    let item = items
        .iter()
        .find(|item| item.get("kind").and_then(Value::as_str) == Some(kind))
        .unwrap_or_else(|| {
            std::panic::panic_any(format!("{kind} missing from /work_items: {items:?}"))
        });
    assert_eq!(
        item.get("ledger_id").and_then(Value::as_str),
        Some(ledger_id)
    );
    assert_eq!(
        item.get("ledger_path").and_then(Value::as_str),
        Some(ledger_path)
    );
    assert_eq!(item.get("role").and_then(Value::as_str), Some(role));
    assert_eq!(item.get("mode").and_then(Value::as_str), Some(mode));
}

fn assert_work_item_message(value: &Value, allow_id: &str, fragment: &str) {
    let items = value
        .pointer("/work_items")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("/work_items should be an array"));
    let item = items
        .iter()
        .find(|item| item.get("allow_id").and_then(Value::as_str) == Some(allow_id))
        .unwrap_or_else(|| std::panic::panic_any(format!("{allow_id} missing from /work_items")));
    let message = item
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or_else(|| std::panic::panic_any(format!("{allow_id} work item has no message")));
    assert!(
        message.contains(fragment),
        "{allow_id} work item message should contain {fragment:?}: {message}"
    );
}

fn assert_report_advisory_count(value: &Value, advisory: &str, count: u64) {
    assert_eq!(
        value
            .pointer(&format!("/trend/{advisory}"))
            .and_then(Value::as_u64),
        Some(count),
        "report trend should expose {advisory} count"
    );
}

fn assert_report_summary_count(value: &Value, field: &str, count: u64) {
    assert_eq!(
        value
            .pointer(&format!("/summary/{field}"))
            .and_then(Value::as_u64),
        Some(count),
        "report summary should expose {field} count"
    );
}

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap_or_else(|err| std::panic::panic_any(format!("git {args:?}: {err}")));
    if !output.status.success() {
        std::panic::panic_any(format!(
            "git {args:?} failed: stdout=`{}` stderr=`{}`",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
}

#[test]
fn repair_routes_converge_into_refresh_and_prune_mutation_previews() {
    let root = create_fixture("repair-route-convergence", true);
    let policy_path = root.join("policy/allow.toml");
    let policy_text = fs::read_to_string(&policy_path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read repair policy: {err}")));
    fs::write(
        &policy_path,
        policy_text.replace(
            "evidence = [\"doc:docs/missing-evidence.md\"]",
            "evidence = [\"test:lifecycle_corpus\"]",
        ),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("prepare repair policy: {err}")));

    let (worklist_path, worklist_result) = run_report(&root, "repair-worklist", &["worklist"]);
    assert_status("repair route worklist", &worklist_result, true);
    assert_quiet("repair route worklist", &worklist_result);
    let worklist = assert_saved_json_artifact(
        &worklist_path,
        "repair route worklist",
        "cargo-allow.worklist.v1",
        "worklist",
    );
    let stale_item = worklist
        .pointer("/work_items")
        .and_then(Value::as_array)
        .and_then(|items| {
            items
                .iter()
                .find(|item| item.get("allow_id").and_then(Value::as_str) == Some(STALE_ID))
        })
        .unwrap_or_else(|| std::panic::panic_any("stale work item should be projected"));
    assert_eq!(
        stale_item.get("status").and_then(Value::as_str),
        Some("stale")
    );
    assert!(
        stale_item
            .pointer("/proof_commands")
            .and_then(Value::as_array)
            .is_some_and(|commands| {
                commands.iter().any(|command| {
                    command
                        .as_str()
                        .is_some_and(|command| command.contains("prune --stale --dry-run"))
                })
            }),
        "stale work item should route to prune preview"
    );

    let (refresh_preview_path, refresh_preview_result) = run_report(
        &root,
        "refresh-preview",
        &["refresh", "--allow-id", DRIFT_ID, "--dry-run"],
    );
    assert_status("refresh preview", &refresh_preview_result, true);
    assert_quiet("refresh preview", &refresh_preview_result);
    let refresh_preview = assert_saved_json_artifact(
        &refresh_preview_path,
        "refresh preview",
        "cargo-allow.refresh.v1",
        "refresh",
    );
    assert_eq!(
        refresh_preview
            .pointer("/summary/entry_id")
            .and_then(Value::as_str),
        Some(DRIFT_ID)
    );
    assert_eq!(
        refresh_preview
            .pointer("/mode/write_requested")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        refresh_preview
            .pointer("/mutation_receipt/changed_allow_ids/0")
            .and_then(Value::as_str),
        Some(DRIFT_ID)
    );

    let (refresh_write_path, refresh_write_result) = run_report(
        &root,
        "refresh-write",
        &["refresh", "--allow-id", DRIFT_ID, "--write"],
    );
    assert_status("refresh write", &refresh_write_result, true);
    assert_quiet("refresh write", &refresh_write_result);
    let refresh_write = assert_saved_json_artifact(
        &refresh_write_path,
        "refresh write",
        "cargo-allow.refresh.v1",
        "refresh",
    );
    assert_eq!(
        refresh_write
            .pointer("/mutation_receipt/result")
            .and_then(Value::as_str),
        Some("written")
    );
    assert_eq!(
        refresh_write
            .pointer("/mutation_receipt/after_fingerprints/0")
            .and_then(Value::as_str)
            .map(|fingerprint| fingerprint.starts_with("sha256:v1:")),
        Some(true)
    );

    let (prune_preview_path, prune_preview_summary_path, prune_preview_result) =
        run_report_with_common_summary(&root, "prune-preview", &["prune", "--stale", "--dry-run"]);
    assert_status("prune preview", &prune_preview_result, true);
    assert_quiet("prune preview", &prune_preview_result);
    let prune_preview = assert_saved_json_artifact(
        &prune_preview_path,
        "prune preview",
        "cargo-allow.prune.v1",
        "prune",
    );
    let stale_entries = prune_preview
        .pointer("/stale_entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("prune preview should list stale entries"));
    assert!(
        stale_entries
            .iter()
            .any(|entry| { entry.get("id").and_then(Value::as_str) == Some(STALE_ID) }),
        "prune preview should select the stale worklist subject"
    );
    assert!(
        prune_preview
            .pointer("/mutation_receipt/changed_allow_ids")
            .and_then(Value::as_array)
            .is_some_and(|ids| ids.iter().any(|id| id.as_str() == Some(STALE_ID))),
        "prune receipt should retain the stale worklist subject identity"
    );

    let prune_preview_summary: Value = serde_json::from_str(
        &fs::read_to_string(&prune_preview_summary_path).unwrap_or_else(|err| {
            std::panic::panic_any(format!("read prune preview summary: {err}"))
        }),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("parse prune preview summary: {err}")));
    assert_eq!(
        prune_preview_summary
            .get("schema_id")
            .and_then(Value::as_str),
        Some("cargo-allow.core-command-summary.v1")
    );
    assert_eq!(
        prune_preview_summary
            .pointer("/operation")
            .and_then(Value::as_str),
        Some("prune")
    );
    assert_eq!(
        prune_preview_summary
            .pointer("/posture")
            .and_then(Value::as_str),
        Some("advisory")
    );
    assert_eq!(
        prune_preview_summary
            .pointer("/primary_action/args/0")
            .and_then(Value::as_str),
        Some("prune")
    );

    let (prune_write_path, prune_write_summary_path, prune_write_result) =
        run_report_with_common_summary(&root, "prune-write", &["prune", "--stale", "--write"]);
    assert_status("prune write", &prune_write_result, true);
    assert_quiet("prune write", &prune_write_result);
    let prune_write = assert_saved_json_artifact(
        &prune_write_path,
        "prune write",
        "cargo-allow.prune.v1",
        "prune",
    );
    assert_eq!(
        prune_write
            .pointer("/mutation_receipt/result")
            .and_then(Value::as_str),
        Some("written")
    );
    let prune_write_summary: Value = serde_json::from_str(
        &fs::read_to_string(&prune_write_summary_path).unwrap_or_else(|err| {
            std::panic::panic_any(format!("read prune write summary: {err}"))
        }),
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("parse prune write summary: {err}")));
    assert_eq!(
        prune_write_summary
            .pointer("/posture")
            .and_then(Value::as_str),
        Some("satisfied")
    );
    assert_eq!(
        prune_write_summary
            .pointer("/operation_effects/writes_repository")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        prune_write_summary
            .pointer("/next_proof/args/0")
            .and_then(Value::as_str),
        Some("check")
    );

    let (final_list_path, final_list_result) = run_report(&root, "repair-final-list", &["list"]);
    assert_status("repair final list", &final_list_result, true);
    assert_quiet("repair final list", &final_list_result);
    let final_list = assert_saved_json_artifact(
        &final_list_path,
        "repair final list",
        "cargo-allow.list.v1",
        "list",
    );
    assert_entry_absent(&final_list, STALE_ID);
    assert_entry_status(&final_list, "/allow_entries", DRIFT_ID, "matched");

    remove_temp_root(root);
}

fn assert_entry_absent(value: &Value, allow_id: &str) {
    let entries = value
        .pointer("/allow_entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("allow_entries should be an array"));
    assert!(
        entries
            .iter()
            .all(|entry| { entry.get("id").and_then(Value::as_str) != Some(allow_id) }),
        "{allow_id} should be absent after its repair mutation"
    );
}
