//! End-to-end proof that the supported first-hour commands present one
//! operator grammar (#3149 PR A).
//!
//! These run the real binary rather than the in-process adapters, so they also
//! cover argv routing, `--command-summary-output` acceptance, and the human/machine
//! parity an automation consumer depends on.

use allow_core::SOURCE_FILE_READ_MAX_BYTES;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Every human summary block in the shared grammar, in order.
const GRAMMAR_FIELDS: [&str; 8] = [
    "Result:",
    "Why:",
    "Subject:",
    "Coverage:",
    "Next:",
    "Writes:",
    "Then:",
    "Not proven:",
];

/// Stable argv fixtures for summary commands that need a subject or explicit
/// mode to exercise their normal route.
const EXPLAIN_ARGV: &[&str] = &["explain", "allow-0001"];
const AUDIT_ARGV: &[&str] = &["audit"];
const CHECK_ARGV: &[&str] = &["check", "--mode", "no-new"];
const LIST_ARGV: &[&str] = &["list"];
const WHY_ARGV: &[&str] = &[
    "why",
    "--kind",
    "panic",
    "--path",
    "src/lib.rs",
    "--line",
    "1",
];
const WORKLIST_ARGV: &[&str] = &["worklist"];

/// Every command that projects the summary.
const GRAMMAR_COMMANDS: [&[&str]; 8] = [
    &["adopt"],
    &["doctor"],
    AUDIT_ARGV,
    CHECK_ARGV,
    LIST_ARGV,
    EXPLAIN_ARGV,
    WHY_ARGV,
    WORKLIST_ARGV,
];

#[test]
fn core_command_summary_router() -> Result<(), String> {
    let root = temp_root("summary-grammar")?;
    // `explain` and `why` need a ledger and an unreceipted finding to inspect,
    // so the fixture carries both. `adopt`, `doctor`, `audit`, `check`, and `worklist`
    // are unaffected by their presence.
    write_source(&root, "pub fn value(v: Option<u8>) -> u8 { v.unwrap() }\n")?;
    run(&root, &["init"])?;

    for command in GRAMMAR_COMMANDS {
        let output = run(&root, command)?;
        let text = stdout(&output)?;
        // Assert the *first eight lines*, not merely that the labels appear
        // somewhere. Several commands repeat words like `Why:` inside their
        // detailed section, so a search-anywhere check would still pass if the
        // summary were not prepended at all.
        let mut lines = text.lines();
        for field in GRAMMAR_FIELDS {
            let line = lines.next().ok_or_else(|| {
                format!(
                    "`{command:?}` human output ended before summary line `{field}`; got:\n{text}"
                )
            })?;
            require(
                line.starts_with(field),
                format!("`{command:?}` summary line must start with `{field}`; got `{line}`"),
            )?;
        }
    }

    remove_temp_root(root)
}

#[test]
fn core_command_summary_mutation_init() -> Result<(), String> {
    let root = temp_root("summary-init")?;
    let sidecar = root.join("init-summary.json");
    let sidecar_text = sidecar.to_string_lossy().to_string();
    let output = run(
        &root,
        &[
            "--command-summary-output",
            &sidecar_text,
            "init",
            "--config",
            "policy/allow.toml",
        ],
    )?;
    require(
        output.status.success(),
        format!("init failed: {}", String::from_utf8_lossy(&output.stderr)),
    )?;
    require(
        stdout(&output)?.starts_with("Result: satisfied"),
        format!(
            "init summary missing from human output: {}",
            stdout(&output)?
        ),
    )?;
    let summary: Value = serde_json::from_str(
        &fs::read_to_string(&sidecar).map_err(|error| format!("read init summary: {error}"))?,
    )
    .map_err(|error| format!("parse init summary: {error}"))?;
    require(
        field(&summary, &["operation"]) == Some(&Value::from("init"))
            && field(&summary, &["operation_effects", "writes_repository"])
                == Some(&Value::Bool(true))
            && field(&summary, &["operation_effects", "write_paths"])
                == Some(&Value::from(vec!["policy/allow.toml"])),
        format!("init summary lost live-write posture: {summary}"),
    )?;

    let preview_sidecar = root.join("init-preview-summary.json");
    let preview_text = preview_sidecar.to_string_lossy().to_string();
    let preview = run(
        &root,
        &[
            "--command-summary-output",
            &preview_text,
            "init",
            "--dry-run",
            "--config",
            "policy/allow.toml",
        ],
    )?;
    require(
        preview.status.success(),
        format!(
            "init dry-run failed: {}",
            String::from_utf8_lossy(&preview.stderr)
        ),
    )?;
    require(
        stdout(&preview)?.starts_with("Result: completed (advisory)"),
        format!("init preview summary missing: {}", stdout(&preview)?),
    )?;
    let preview_summary: Value = serde_json::from_str(
        &fs::read_to_string(&preview_sidecar)
            .map_err(|error| format!("read init preview summary: {error}"))?,
    )
    .map_err(|error| format!("parse init preview summary: {error}"))?;
    require(
        field(
            &preview_summary,
            &["operation_effects", "writes_repository"],
        ) == Some(&Value::Bool(false)),
        format!("init preview must remain read-only: {preview_summary}"),
    )?;
    remove_temp_root(root)
}

#[test]
fn core_command_summary_mutation_propose_preserves_candidate_boundary() -> Result<(), String> {
    let root = temp_root("summary-propose")?;
    write_source(&root, "pub fn value(v: Option<u8>) -> u8 { v.unwrap() }\n")?;
    run(&root, &["init"])?;
    git_commit_fixture(&root)?;

    let written_sidecar = root.join("propose-written-summary.json");
    let written_sidecar_text = written_sidecar.to_string_lossy().to_string();
    let written_target = root.join("policy/allow.proposed.toml");
    let written_target_text = written_target.to_string_lossy().to_string();
    let written = run(
        &root,
        &[
            "--command-summary-output",
            &written_sidecar_text,
            "propose",
            "--write",
            &written_target_text,
            "--force",
        ],
    )?;
    require(
        written.status.success(),
        format!(
            "propose write failed: {}",
            String::from_utf8_lossy(&written.stderr)
        ),
    )?;
    require(
        String::from_utf8_lossy(&written.stderr).starts_with("Result: completed (advisory)"),
        format!(
            "propose write must expose the candidate summary on stderr: {}",
            String::from_utf8_lossy(&written.stderr)
        ),
    )?;
    let written_summary: Value = serde_json::from_str(
        &fs::read_to_string(&written_sidecar)
            .map_err(|error| format!("read written propose summary: {error}"))?,
    )
    .map_err(|error| format!("parse written propose summary: {error}"))?;
    require(
        field(
            &written_summary,
            &["operation_effects", "writes_repository"],
        ) == Some(&Value::Bool(true))
            && field(&written_summary, &["operation_effects", "write_paths"])
                == Some(&Value::from(vec!["policy/allow.proposed.toml"])),
        format!("propose write lost candidate target posture: {written_summary}"),
    )?;

    let stdout_sidecar = root.join("propose-stdout-summary.json");
    let stdout_sidecar_text = stdout_sidecar.to_string_lossy().to_string();
    let stdout_candidate = run(
        &root,
        &["--command-summary-output", &stdout_sidecar_text, "propose"],
    )?;
    require(
        stdout_candidate.status.success(),
        format!(
            "propose stdout candidate failed: {}",
            String::from_utf8_lossy(&stdout_candidate.stderr)
        ),
    )?;
    let stdout_summary: Value = serde_json::from_str(
        &fs::read_to_string(&stdout_sidecar)
            .map_err(|error| format!("read stdout propose summary: {error}"))?,
    )
    .map_err(|error| format!("parse stdout propose summary: {error}"))?;
    require(
        field(&stdout_summary, &["operation_effects", "writes_repository"])
            == Some(&Value::Bool(false))
            && field(&stdout_summary, &["posture"]) == Some(&Value::from("advisory")),
        format!("propose stdout must remain read-only advisory output: {stdout_summary}"),
    )?;

    remove_temp_root(root)
}

#[test]
fn core_command_summary_mutation_add_separates_candidate_and_live_entry() -> Result<(), String> {
    let root = temp_root("summary-add")?;
    write_source(&root, "pub fn value(v: Option<u8>) -> u8 { v.unwrap() }\n")?;
    run(&root, &["init"])?;
    git_commit_fixture(&root)?;

    let candidate_sidecar = root.join("add-candidate-summary.json");
    let candidate_sidecar_text = candidate_sidecar.to_string_lossy().to_string();
    let candidate_target = root.join("policy/allow.proposed.toml");
    let candidate_target_text = candidate_target.to_string_lossy().to_string();
    let candidate = run(
        &root,
        &[
            "--command-summary-output",
            &candidate_sidecar_text,
            "add",
            "--kind",
            "panic",
            "--path",
            "src/lib.rs",
            "--line",
            "1",
            "--owner",
            "fixture",
            "--reason",
            "candidate review",
            "--write",
            &candidate_target_text,
            "--force",
        ],
    )?;
    require(
        candidate.status.success(),
        format!(
            "add candidate failed: {}",
            String::from_utf8_lossy(&candidate.stderr)
        ),
    )?;
    require(
        String::from_utf8_lossy(&candidate.stderr).starts_with("Result: completed (advisory)"),
        format!(
            "candidate add must be advisory: {}",
            String::from_utf8_lossy(&candidate.stderr)
        ),
    )?;
    let candidate_summary: Value = serde_json::from_str(
        &fs::read_to_string(&candidate_sidecar)
            .map_err(|error| format!("read add candidate summary: {error}"))?,
    )
    .map_err(|error| format!("parse add candidate summary: {error}"))?;
    require(
        field(
            &candidate_summary,
            &["operation_effects", "writes_repository"],
        ) == Some(&Value::Bool(true))
            && field(&candidate_summary, &["operation_effects", "write_paths"])
                == Some(&Value::from(vec!["policy/allow.proposed.toml"])),
        format!("candidate add lost its exact write posture: {candidate_summary}"),
    )?;

    let live_sidecar = root.join("add-live-summary.json");
    let live_sidecar_text = live_sidecar.to_string_lossy().to_string();
    let live = run(
        &root,
        &[
            "--command-summary-output",
            &live_sidecar_text,
            "add",
            "--kind",
            "panic",
            "--path",
            "src/lib.rs",
            "--line",
            "1",
            "--owner",
            "fixture",
            "--reason",
            "live review",
            "--update",
        ],
    )?;
    require(
        live.status.success(),
        format!(
            "add update failed: {}",
            String::from_utf8_lossy(&live.stderr)
        ),
    )?;
    let live_summary: Value = serde_json::from_str(
        &fs::read_to_string(&live_sidecar)
            .map_err(|error| format!("read add live summary: {error}"))?,
    )
    .map_err(|error| format!("parse add live summary: {error}"))?;
    require(
        field(&live_summary, &["posture"]) == Some(&Value::from("satisfied"))
            && field(&live_summary, &["operation_effects", "writes_repository"])
                == Some(&Value::Bool(true))
            && field(&live_summary, &["operation_effects", "write_paths"])
                == Some(&Value::from(vec!["policy/allow.toml"])),
        format!("live add must name its exact ledger write: {live_summary}"),
    )?;

    remove_temp_root(root)
}

#[test]
fn summary_commands_emit_a_read_only_summary_sidecar() -> Result<(), String> {
    let root = temp_root("summary-inspection")?;
    write_source(&root, "pub fn value(v: Option<u8>) -> u8 { v.unwrap() }\n")?;
    run(&root, &["init"])?;

    for (label, command) in [
        ("audit", AUDIT_ARGV),
        ("check", CHECK_ARGV),
        ("explain", EXPLAIN_ARGV),
        ("why", WHY_ARGV),
        ("worklist", WORKLIST_ARGV),
    ] {
        let sidecar = root.join(format!("{label}-summary.json"));
        let mut argv = vec!["--command-summary-output"];
        let sidecar_text = sidecar.to_string_lossy().to_string();
        argv.push(&sidecar_text);
        argv.extend(command.iter().copied());
        let text = stdout(&run(&root, &argv)?)?;
        let summary: Value = serde_json::from_str(
            &fs::read_to_string(&sidecar)
                .map_err(|error| format!("read {label} summary: {error}"))?,
        )
        .map_err(|error| format!("parse {label} summary: {error}"))?;

        require(
            field(&summary, &["operation"]) == Some(&Value::from(label)),
            format!("{label} summary must name its own operation"),
        )?;
        let reason = field(&summary, &["reason", "message"])
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{label} summary needs a human reason"))?;
        require(
            text.contains(reason),
            format!("{label} human `Why:` must match the summary reason"),
        )?;
        // None of these three writes anything without `why --plan`.
        require(
            field(&summary, &["operation_effects", "writes_repository"])
                == Some(&Value::Bool(false)),
            format!("{label} is read-only"),
        )?;
        require(
            text.contains("Writes: nothing in this operation"),
            format!("{label} must state its read-only posture in the summary"),
        )?;
    }

    remove_temp_root(root)
}

#[test]
fn triage_summary_matrix_preserves_read_only_and_judgment_boundaries() -> Result<(), String> {
    let root = temp_root("summary-triage-matrix")?;
    write_source(&root, "pub fn value(v: Option<u8>) -> u8 { v.unwrap() }\n")?;
    run(&root, &["init"])?;
    git_commit_fixture(&root)?;

    let list = run(&root, &["list", "--format", "json"])?;
    let list_json: Value = serde_json::from_slice(&list.stdout)
        .map_err(|error| format!("parse list triage summary: {error}"))?;
    let list_summary = list_json
        .get("core_command_summary")
        .ok_or_else(|| "list JSON omitted the common summary".to_string())?;
    require(
        field(list_summary, &["operation"]) == Some(&Value::from("list"))
            && field(list_summary, &["operation_effects", "writes_repository"])
                == Some(&Value::Bool(false)),
        format!("list must remain a read-only common summary: {list_summary}"),
    )?;

    for (label, command) in [("why", WHY_ARGV), ("worklist", WORKLIST_ARGV)] {
        let sidecar = root.join(format!("{label}-triage-summary.json"));
        let sidecar_text = sidecar.to_string_lossy().to_string();
        let mut argv = vec!["--command-summary-output", &sidecar_text];
        argv.extend(command.iter().copied());
        let output = run(&root, &argv)?;
        let summary: Value = serde_json::from_str(
            &fs::read_to_string(&sidecar)
                .map_err(|error| format!("read {label} triage summary: {error}"))?,
        )
        .map_err(|error| format!("parse {label} triage summary: {error}"))?;

        require(
            field(&summary, &["operation"]) == Some(&Value::from(label))
                && field(&summary, &["operation_effects", "writes_repository"])
                    == Some(&Value::Bool(false)),
            format!("{label} must remain read-only: {summary}"),
        )?;
        if let Some(action) = summary.get("primary_action") {
            require(
                field(action, &["kind"]) == Some(&Value::from("decision"))
                    && field(action, &["program"]).is_none(),
                format!("{label} repository-controlled next step must remain a decision: {action}"),
            )?;
        }
        require(
            !output.stdout.windows(2).any(|pair| pair == [0x1b, b'[']),
            format!("{label} summary unexpectedly emitted an ANSI escape"),
        )?;
    }

    remove_temp_root(root)
}

#[test]
fn list_summary_distinguishes_empty_ledger_from_empty_filter_result() -> Result<(), String> {
    let root = temp_root("summary-list-filter-matrix")?;
    write_source(&root, "pub fn value() -> u8 { 1 }\n")?;
    run(&root, &["init"])?;
    fs::write(
        root.join("policy/allow.toml"),
        r#"schema_version = "0.1"
policy = "cargo-allow"

[requirements]
owner_required = true
reason_required = true
classification_required = true
evidence_required = false
expires_or_review_after_required = true
stale_entries_fail = false
allow_bare_allow_attributes = false
lint_policy_id_required = false

[requirements.unsafe]
evidence_required = true
safety_comment_required = false
"#,
    )
    .map_err(|error| format!("write empty policy fixture: {error}"))?;
    git_commit_fixture(&root)?;

    for (label, extra_args, expected_reason, expected_action) in [
        ("empty", Vec::<&str>::new(), "list.no_entries", "adopt"),
        (
            "filtered-empty",
            vec!["--owner", "no-such-owner"],
            "list.no_filter_matches",
            "list",
        ),
    ] {
        let mut argv = vec!["list", "--format", "json"];
        argv.extend(extra_args);
        let output = run(&root, &argv)?;
        let json: Value = serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("parse {label} list output: {error}"))?;
        let summary = json
            .get("core_command_summary")
            .ok_or_else(|| format!("{label} list output omitted common summary"))?;
        require(
            field(summary, &["reason", "code"]) == Some(&Value::from(expected_reason))
                && field(summary, &["operation_effects", "writes_repository"])
                    == Some(&Value::Bool(false))
                && summary.pointer("/primary_action/args/0") == Some(&Value::from(expected_action)),
            format!("{label} list summary lost its distinction: {summary}"),
        )?;
    }

    remove_temp_root(root)
}

#[test]
fn why_summary_keeps_a_skipped_target_partial_and_non_green() -> Result<(), String> {
    let root = temp_root("summary-why-partial-target")?;
    write_source(&root, "pub fn value() -> u8 { 1 }\n")?;
    run(&root, &["init"])?;
    fs::write(
        root.join("src/large.rs"),
        vec![b' '; (SOURCE_FILE_READ_MAX_BYTES as usize).saturating_add(1)],
    )
    .map_err(|error| format!("write oversized target: {error}"))?;
    git_commit_fixture(&root)?;

    let sidecar = root.join("why-partial-summary.json");
    let sidecar_text = sidecar.to_string_lossy().to_string();
    let mut argv = vec!["--command-summary-output", &sidecar_text];
    argv.extend([
        "why",
        "--kind",
        "panic",
        "--path",
        "src/large.rs",
        "--line",
        "1",
    ]);
    let output = run(&root, &argv)?;
    let summary: Value = serde_json::from_str(
        &fs::read_to_string(&sidecar)
            .map_err(|error| format!("read partial why summary: {error}"))?,
    )
    .map_err(|error| format!("parse partial why summary: {error}"))?;

    require(
        field(&summary, &["result_class"]) == Some(&Value::from("partial_data"))
            && field(&summary, &["posture"]) == Some(&Value::from("blocking"))
            && field(&summary, &["completeness"]) == Some(&Value::from("partial"))
            && field(&summary, &["next_proof"]).is_none()
            && field(&summary, &["operation_effects", "writes_repository"])
                == Some(&Value::Bool(false))
            && field(&summary, &["primary_action", "kind"]) == Some(&Value::from("decision")),
        format!("partial why summary made an unsafe claim: {summary}"),
    )?;
    require(
        stdout(&output)?.contains("partial") && stdout(&output)?.contains("large.rs"),
        format!(
            "partial why human output lost its coverage context: {}",
            stdout(&output)?
        ),
    )
}

#[test]
fn why_summary_preserves_ambiguous_candidates_and_read_only_posture() -> Result<(), String> {
    let root = temp_root("summary-why-ambiguous")?;
    write_source(&root, "pub fn value(v: Option<u8>) -> u8 { v.unwrap() }\n")?;
    run(&root, &["init"])?;
    fs::write(
        root.join("policy/allow.toml"),
        r#"schema_version = "0.1"
policy = "cargo-allow"

[requirements]
owner_required = true
reason_required = true
classification_required = true
evidence_required = false
expires_or_review_after_required = true
stale_entries_fail = false
allow_bare_allow_attributes = false
lint_policy_id_required = false

[requirements.unsafe]
evidence_required = true
safety_comment_required = false

[[allow]]
id = "allow-tied-a"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "First equally matching reviewed exception."
evidence = ["test:tied_a"]
created = "2026-01-01"
review_after = "2027-01-01"

[allow.selector]
ast_kind = "method_call"
callee = "unwrap"

[[allow]]
id = "allow-tied-b"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "Second equally matching reviewed exception."
evidence = ["test:tied_b"]
created = "2026-01-01"
review_after = "2027-01-01"

[allow.selector]
ast_kind = "method_call"
callee = "unwrap"
"#,
    )
    .map_err(|error| format!("write ambiguous policy: {error}"))?;
    git_commit_fixture(&root)?;

    let sidecar = root.join("why-ambiguous-summary.json");
    let sidecar_text = sidecar.to_string_lossy().to_string();
    let mut summary_argv = vec!["--command-summary-output", &sidecar_text];
    summary_argv.extend(WHY_ARGV.iter().copied());
    let summary_output = run(&root, &summary_argv)?;
    require(
        summary_output.status.success(),
        format!(
            "ambiguous why should be inspectable: {}",
            String::from_utf8_lossy(&summary_output.stderr)
        ),
    )?;
    let human = stdout(&summary_output)?;
    let summary: Value = serde_json::from_str(
        &fs::read_to_string(&sidecar)
            .map_err(|error| format!("read ambiguous why summary: {error}"))?,
    )
    .map_err(|error| format!("parse ambiguous why summary: {error}"))?;
    require(
        field(&summary, &["result_class"]) == Some(&Value::from("findings"))
            && field(&summary, &["posture"]) == Some(&Value::from("decision_required"))
            && field(&summary, &["reason", "code"]) == Some(&Value::from("why.ambiguous"))
            && field(&summary, &["primary_action", "kind"]) == Some(&Value::from("decision"))
            && field(&summary, &["additional_action_count"])
                .and_then(Value::as_u64)
                .is_some_and(|count| count >= 2)
            && field(&summary, &["additional_actions_ref"])
                == Some(&Value::from("cargo-allow.why.v1.next.suggested_actions"))
            && field(&summary, &["operation_effects", "writes_repository"])
                == Some(&Value::Bool(false))
            && field(&summary, &["operation_effects", "write_paths"]).is_none(),
        format!("ambiguous why summary lost judgment boundaries: {summary}"),
    )?;
    let human_lines: Vec<&str> = human.lines().collect();
    require(
        human_lines.len() >= GRAMMAR_FIELDS.len(),
        format!(
            "ambiguous why human summary must contain all {} grammar lines: {human}",
            GRAMMAR_FIELDS.len()
        ),
    )?;
    require(
        GRAMMAR_FIELDS
            .iter()
            .zip(human_lines.iter().copied())
            .all(|(field_name, line)| line.starts_with(field_name))
            && human.contains("Result: findings (decision_required)")
            && human.contains("Next: Multiple allow entries compete for this finding.")
            && human.contains("Writes: nothing in this operation")
            && !human.contains('\u{1b}'),
        format!("ambiguous why human summary lost parity: {human}"),
    )?;

    let detailed = root.join("why-ambiguous.json");
    let detailed_text = detailed.to_string_lossy().to_string();
    let mut detailed_argv = WHY_ARGV.to_vec();
    detailed_argv.extend(["--format", "json", "--output", &detailed_text]);
    let detailed_output = run(&root, &detailed_argv)?;
    require(
        detailed_output.status.success(),
        format!(
            "ambiguous why JSON should be inspectable: {}",
            String::from_utf8_lossy(&detailed_output.stderr)
        ),
    )?;
    let detailed: Value = serde_json::from_str(
        &fs::read_to_string(&detailed)
            .map_err(|error| format!("read ambiguous why detail: {error}"))?,
    )
    .map_err(|error| format!("parse ambiguous why detail: {error}"))?;
    require(
        field(&detailed, &["outcome", "status"]) == Some(&Value::from("ambiguous"))
            && field(&detailed, &["outcome", "allow_id"]) == Some(&Value::Null)
            && field(&detailed, &["outcome", "candidate_ids"])
                == Some(&Value::from(vec!["allow-tied-a", "allow-tied-b"]))
            && field(&detailed, &["next", "proof_plans"])
                .and_then(Value::as_array)
                .is_some_and(|plans| {
                    plans.iter().any(|plan| {
                        field(plan, &["args"])
                            == Some(&Value::from(vec!["explain", "allow-tied-a"]))
                    }) && plans.iter().any(|plan| {
                        field(plan, &["args"])
                            == Some(&Value::from(vec!["explain", "allow-tied-b"]))
                    })
                }),
        format!("ambiguous why detail lost candidates or alternatives: {detailed}"),
    )?;

    remove_temp_root(root)
}

#[test]
fn why_plan_reports_the_candidate_write_it_performed() -> Result<(), String> {
    let root = temp_root("summary-why-plan")?;
    write_source(&root, "pub fn value(v: Option<u8>) -> u8 { v.unwrap() }\n")?;
    run(&root, &["init"])?;
    // An add-finding plan requires an exact evaluation, which requires a
    // committed Git inventory rather than the filesystem fallback.
    git_commit_fixture(&root)?;

    let sidecar = root.join("why-summary.json");
    let sidecar_text = sidecar.to_string_lossy().to_string();
    let plan = root.join("add-finding.plan.json");
    let plan_text = plan.to_string_lossy().to_string();
    let mut argv = vec!["--command-summary-output", &sidecar_text];
    argv.extend(WHY_ARGV.iter().copied());
    argv.push("--plan");
    argv.push(&plan_text);
    run(&root, &argv)?;

    require(
        plan.exists(),
        "why --plan must write its candidate artifact",
    )?;
    let summary: Value = serde_json::from_str(
        &fs::read_to_string(&sidecar).map_err(|error| format!("read why summary: {error}"))?,
    )
    .map_err(|error| format!("parse why summary: {error}"))?;
    require(
        field(&summary, &["operation_effects", "writes_repository"]) == Some(&Value::Bool(true)),
        "why --plan is not read-only",
    )?;
    require(
        field(&summary, &["operation_effects", "write_paths"])
            == Some(&Value::from(vec!["add-finding.plan.json"])),
        format!(
            "the summary must name the exact plan path, got {:?}",
            field(&summary, &["operation_effects", "write_paths"])
        ),
    )?;

    remove_temp_root(root)
}

#[test]
fn explain_summary_preserves_stale_policy_health_and_read_only_posture() -> Result<(), String> {
    let root = temp_root("summary-explain-stale")?;
    write_source(&root, "pub fn value(v: Option<u8>) -> u8 { v.unwrap() }\n")?;
    run(&root, &["init"])?;
    fs::write(
        root.join("policy/allow.toml"),
        r#"schema_version = "0.1"
policy = "cargo-allow"

[requirements]
owner_required = true
reason_required = true
classification_required = true
evidence_required = false
expires_or_review_after_required = true
stale_entries_fail = false
allow_bare_allow_attributes = false
lint_policy_id_required = false

[requirements.unsafe]
evidence_required = true
safety_comment_required = false

[[allow]]
id = "allow-stale"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed_exception"
reason = "Entry intentionally has no current finding."
evidence = ["test:stale"]
created = "2026-01-01"
# The test asserts this unused entry reports status "stale"; an unused
# entry becomes review-due once review_after arrives (advisory but a
# different status), so pin it far in the future to keep the asserted
# status stable.
review_after = "2099-01-01"

[allow.selector]
ast_kind = "method_call"
container = "removed_function"
callee = "unwrap"
"#,
    )
    .map_err(|error| format!("write stale policy: {error}"))?;
    git_commit_fixture(&root)?;

    let sidecar = root.join("explain-stale-summary.json");
    let sidecar_text = sidecar.to_string_lossy().to_string();
    let summary_output = run(
        &root,
        &[
            "--command-summary-output",
            &sidecar_text,
            "explain",
            "allow-stale",
        ],
    )?;
    require(
        summary_output.status.success(),
        format!(
            "stale explain should be inspectable: {}",
            String::from_utf8_lossy(&summary_output.stderr)
        ),
    )?;
    let human = stdout(&summary_output)?;
    let summary: Value = serde_json::from_str(
        &fs::read_to_string(&sidecar)
            .map_err(|error| format!("read stale explain summary: {error}"))?,
    )
    .map_err(|error| format!("parse stale explain summary: {error}"))?;
    let human_lines: Vec<&str> = human.lines().collect();
    require(
        human_lines.len() >= GRAMMAR_FIELDS.len()
            && GRAMMAR_FIELDS
                .iter()
                .zip(human_lines.iter().copied())
                .all(|(field_name, line)| line.starts_with(field_name))
            && human.contains("stale")
            && !human.contains('\u{1b}'),
        format!("stale explain human summary lost policy-health context: {human}"),
    )?;
    require(
        field(&summary, &["result_class"]) == Some(&Value::from("findings"))
            && field(&summary, &["posture"]) == Some(&Value::from("advisory"))
            && field(&summary, &["reason", "code"]) == Some(&Value::from("explain.stale"))
            && field(&summary, &["operation_effects", "writes_repository"])
                == Some(&Value::Bool(false))
            && field(&summary, &["primary_action", "kind"]) == Some(&Value::from("decision")),
        format!("stale explain summary lost advisory boundaries: {summary}"),
    )?;

    let detailed = root.join("explain-stale.json");
    let detailed_text = detailed.to_string_lossy().to_string();
    let detailed_output = run(
        &root,
        &[
            "explain",
            "allow-stale",
            "--format",
            "json",
            "--output",
            &detailed_text,
        ],
    )?;
    require(
        detailed_output.status.success(),
        format!(
            "stale explain JSON should be inspectable: {}",
            String::from_utf8_lossy(&detailed_output.stderr)
        ),
    )?;
    let detailed: Value = serde_json::from_str(
        &fs::read_to_string(&detailed)
            .map_err(|error| format!("read stale explain detail: {error}"))?,
    )
    .map_err(|error| format!("parse stale explain detail: {error}"))?;
    require(
        field(&detailed, &["allow_entry", "id"]) == Some(&Value::from("allow-stale"))
            && field(&detailed, &["summary", "current_status"]) == Some(&Value::from("stale"))
            && detailed.pointer("/match_outcomes/0/status") == Some(&Value::from("stale")),
        format!("stale explain detail lost policy-health status: {detailed}"),
    )?;

    remove_temp_root(root)
}

/// `why` writes `--plan` relative to the working directory, so resolving the
/// summary conflict list only under `--root` let a relative `--root` hide a
/// real collision: the sidecar then overwrote the plan it was meant to spare.
#[test]
fn why_plan_conflicts_with_the_summary_across_resolution_bases() -> Result<(), String> {
    let root = temp_root("summary-why-plan-base")?;
    write_source(&root, "pub fn value(v: Option<u8>) -> u8 { v.unwrap() }\n")?;
    run(&root, &["init"])?;
    git_commit_fixture(&root)?;

    let parent = root
        .parent()
        .ok_or_else(|| "temp root needs a parent".to_string())?
        .to_path_buf();
    let name = root
        .file_name()
        .ok_or_else(|| "temp root needs a name".to_string())?
        .to_string_lossy()
        .to_string();
    // From `parent`, both artifacts name the same file: the plan resolves
    // against the working directory, the sidecar against `--root`.
    let plan_arg = format!("{name}/collision.json");
    let mut argv = vec!["--command-summary-output", "collision.json"];
    argv.extend(WHY_ARGV.iter().copied());
    argv.push("--plan");
    argv.push(&plan_arg);
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .current_dir(&parent)
        .args(&argv)
        .arg("--root")
        .arg(&name)
        .output()
        .map_err(|error| format!("run {argv:?}: {error}"))?;

    require(
        !output.status.success(),
        format!(
            "the collision must be refused, got {:?} / {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ),
    )?;
    require(
        String::from_utf8_lossy(&output.stderr).contains("--command-summary-output must differ"),
        format!(
            "the refusal must name the conflicting flags, got {}",
            String::from_utf8_lossy(&output.stderr)
        ),
    )?;
    // The plan is written before the summary stage, so the guard's job is to
    // refuse rather than to prevent the write: what must not happen is the
    // sidecar replacing the plan on its way out.
    let artifact = fs::read_to_string(root.join("collision.json"))
        .map_err(|error| format!("read the contested artifact: {error}"))?;
    let artifact: Value = serde_json::from_str(&artifact)
        .map_err(|error| format!("parse the contested artifact: {error}"))?;
    require(
        field(&artifact, &["operation_effects"]).is_none(),
        "the add-finding plan must survive; the summary must not overwrite it",
    )?;

    remove_temp_root(root)
}

/// The mirror of the case above: `adopt` resolves `--output` under the root, so
/// a root-relative output that merely *looks* like the sidecar path under the
/// working-directory base targets a different file and must still be allowed.
#[test]
fn adopt_output_is_not_a_conflict_when_only_the_wrong_base_would_collide() -> Result<(), String> {
    let root = temp_root("summary-adopt-base")?;
    write_source(&root, "pub fn value(v: Option<u8>) -> u8 { v.unwrap() }\n")?;
    run(&root, &["init"])?;
    git_commit_fixture(&root)?;

    let parent = root
        .parent()
        .ok_or_else(|| "temp root needs a parent".to_string())?
        .to_path_buf();
    let name = root
        .file_name()
        .ok_or_else(|| "temp root needs a name".to_string())?
        .to_string_lossy()
        .to_string();
    // `adopt --output <name>/plan.json` resolves under the root, so it lands on
    // `<root>/<name>/plan.json` — not the sidecar's `<root>/plan.json`. Only the
    // working-directory base would make these two collide.
    let output_arg = format!("{name}/plan.json");
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .current_dir(&parent)
        .args(["--command-summary-output", "plan.json", "adopt"])
        .args(["--output", &output_arg])
        .arg("--root")
        .arg(&name)
        .output()
        .map_err(|error| format!("run adopt: {error}"))?;

    require(
        !String::from_utf8_lossy(&output.stderr).contains("--command-summary-output must differ"),
        format!(
            "distinct files must not be refused as a conflict, got {}",
            String::from_utf8_lossy(&output.stderr)
        ),
    )?;
    // Absence of the conflict message is not acceptance: without these the test
    // would also pass if adopt had failed for some unrelated reason.
    require(
        output.status.success(),
        format!(
            "adopt must succeed on distinct paths, got {}",
            String::from_utf8_lossy(&output.stderr)
        ),
    )?;
    require(
        root.join("plan.json").is_file(),
        "the sidecar must actually be written to the accepted path",
    )?;

    remove_temp_root(root)
}

#[test]
fn adopt_and_doctor_agree_between_human_and_summary_artifacts() -> Result<(), String> {
    let root = temp_root("summary-parity")?;
    write_source(&root, "pub fn value() -> u8 { 1 }\n")?;

    for command in ["adopt", "doctor"] {
        let sidecar = root.join(format!("{command}-summary.json"));
        let output = run(
            &root,
            &[
                "--command-summary-output",
                &sidecar.to_string_lossy(),
                command,
            ],
        )?;
        let text = stdout(&output)?;
        let summary: Value = serde_json::from_str(
            &fs::read_to_string(&sidecar)
                .map_err(|error| format!("read {command} summary: {error}"))?,
        )
        .map_err(|error| format!("parse {command} summary: {error}"))?;

        require(
            field(&summary, &["schema_id"])
                == Some(&Value::from("cargo-allow.core-command-summary.v1")),
            format!("{command} summary must carry the versioned schema ID"),
        )?;
        require(
            field(&summary, &["operation"]) == Some(&Value::from(command)),
            format!("{command} summary must name its own operation"),
        )?;
        // The load-bearing discriminator is the machine field, and the human
        // reason text must not disagree with it.
        let reason = field(&summary, &["reason", "message"])
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{command} summary needs a human reason"))?;
        require(
            text.contains(reason),
            format!("{command} human `Why:` must match the summary reason"),
        )?;
        require(
            field(&summary, &["operation_effects", "writes_repository"])
                == Some(&Value::Bool(false)),
            format!("{command} is read-only"),
        )?;
        require(
            text.contains("Writes: nothing in this operation"),
            format!("{command} must state its read-only posture in the summary"),
        )?;
    }

    remove_temp_root(root)
}

#[test]
fn adopt_and_doctor_preserve_their_detailed_artifacts() -> Result<(), String> {
    let root = temp_root("summary-detail-preserved")?;
    write_source(&root, "pub fn value() -> u8 { 1 }\n")?;

    // The summary is additive: the pre-existing detailed human sections and the
    // command-specific JSON artifacts must survive the migration unchanged.
    let adopt = stdout(&run(&root, &["adopt"])?)?;
    require(
        adopt.contains("Repository state:") && adopt.contains("Schema:"),
        "adopt must keep its detailed plan section",
    )?;

    let doctor = stdout(&run(&root, &["doctor"])?)?;
    require(
        doctor.contains("source tree root:") && doctor.contains("inventory:"),
        "doctor must keep its detailed diagnosis section",
    )?;

    // JSON stays the command's own artifact, not the summary projection.
    let adopt_json: Value =
        serde_json::from_str(&stdout(&run(&root, &["adopt", "--format", "json"])?)?)
            .map_err(|error| format!("adopt JSON: {error}"))?;
    require(
        field(&adopt_json, &["schema_id"])
            == Some(&Value::from("cargo-allow.core-adoption-plan.v1")),
        "adopt --format json must remain the adoption plan artifact",
    )?;

    remove_temp_root(root)
}

#[test]
fn summary_output_is_rejected_for_unmigrated_commands() -> Result<(), String> {
    let root = temp_root("summary-unmigrated")?;
    write_source(&root, "pub fn value() -> u8 { 1 }\n")?;
    let sidecar = root.join("summary.json");

    let output = run(
        &root,
        &[
            "--command-summary-output",
            &sidecar.to_string_lossy(),
            "vocabulary",
        ],
    )?;
    require(
        !output.status.success(),
        "an unmigrated command must reject --command-summary-output rather than silently ignore it",
    )?;
    require(
        !sidecar.exists(),
        "a rejected --command-summary-output must not leave a partial artifact behind",
    )?;

    remove_temp_root(root)
}

#[test]
fn doctor_summary_identity_is_stable_across_repository_relocation() -> Result<(), String> {
    // Same content, two locations: the portable identity an automation
    // consumer keys on must not depend on where the checkout lives.
    let mut identities = Vec::new();
    let mut roots = Vec::new();
    for label in ["relocation-left", "relocation-right"] {
        let root = temp_root(label)?;
        write_source(&root, "pub fn value() -> u8 { 1 }\n")?;
        let sidecar = root.join("summary.json");
        run(
            &root,
            &[
                "--command-summary-output",
                &sidecar.to_string_lossy(),
                "doctor",
            ],
        )?;
        let summary: Value = serde_json::from_str(
            &fs::read_to_string(&sidecar).map_err(|error| format!("read summary: {error}"))?,
        )
        .map_err(|error| format!("parse summary: {error}"))?;
        let identity = field(&summary, &["subject", "repository_identity"])
            .and_then(Value::as_str)
            .ok_or("summary needs a repository identity")?
            .to_string();
        require(
            !identity.contains(&root.to_string_lossy().to_string()),
            "the repository identity must not embed a private absolute path",
        )?;
        identities.push(identity);
        roots.push(root);
    }

    let mut distinct = identities.clone();
    distinct.dedup();
    require(
        distinct.len() == 1,
        format!("relocated identity drifted: {identities:?}"),
    )?;

    for root in roots {
        remove_temp_root(root)?;
    }
    Ok(())
}

/// Read a nested JSON field without panicking-index syntax.
fn field<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(key)?;
    }
    Some(current)
}

/// Run the real binary. `args` carries any global flags plus the subcommand, in
/// that order; `--root` is appended because it belongs to the subcommand.
fn run(root: &Path, args: &[&str]) -> Result<Output, String> {
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .args(args)
        .arg("--root")
        .arg(root)
        .output()
        .map_err(|error| format!("run {args:?}: {error}"))
}

fn stdout(output: &Output) -> Result<String, String> {
    String::from_utf8(output.stdout.clone()).map_err(|error| error.to_string())
}

/// Commit the fixture so the Git inventory, not the filesystem fallback,
/// backs the scan.
fn git_commit_fixture(root: &Path) -> Result<(), String> {
    for args in [
        vec!["init", "-q"],
        vec!["add", "-A"],
        vec![
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "user.name=fixture",
            "commit",
            "-q",
            "-m",
            "fixture",
        ],
    ] {
        let output = Command::new("git")
            .current_dir(root)
            .args(&args)
            .output()
            .map_err(|error| format!("git {args:?}: {error}"))?;
        require(
            output.status.success(),
            format!(
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        )?;
    }
    Ok(())
}

fn write_source(root: &Path, source: &str) -> Result<(), String> {
    fs::create_dir_all(root.join("src")).map_err(|error| error.to_string())?;
    fs::write(root.join("src/lib.rs"), source).map_err(|error| error.to_string())
}

fn temp_root(label: &str) -> Result<PathBuf, String> {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("system clock: {error}"))?
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root).map_err(|error| format!("create temp root: {error}"))?;
    Ok(root)
}

fn remove_temp_root(root: PathBuf) -> Result<(), String> {
    fs::remove_dir_all(&root)
        .map_err(|error| format!("remove temp root {}: {error}", root.display()))
}

fn require(condition: bool, message: impl Into<String>) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}
