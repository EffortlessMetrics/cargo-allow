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
///
/// The first line is `Outcome:`, not `Result:` (#4393): commands whose detailed
/// report also prints a gate verdict (`Result: passed (enforcing)`) must not
/// leave one screen with two different words under one `Result:` label.
const GRAMMAR_FIELDS: [&str; 8] = [
    "Outcome:",
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
        stdout(&output)?.starts_with("Outcome: satisfied"),
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
        stdout(&preview)?.starts_with("Outcome: completed (advisory)"),
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
        String::from_utf8_lossy(&written.stderr).starts_with("Outcome: completed (advisory)"),
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
        String::from_utf8_lossy(&candidate.stderr).starts_with("Outcome: completed (advisory)"),
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
    // This unrelated finding precedes the explained one in the full inventory.
    // Entry projection must exclude it and remap the retained finding's index.
    fs::write(
        root.join("src/a_unrelated.rs"),
        "pub fn other(v: Option<u8>) -> u8 { v.expect(\"fixture\") }\n",
    )
    .map_err(|error| format!("write unrelated source: {error}"))?;
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
review_after = "2099-01-01"

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
review_after = "2099-01-01"

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
            && human.contains("Outcome: findings (decision_required)")
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

    let check_output = run(
        &root,
        &[
            "check", "--kind", "panic", "--mode", "no-new", "--format", "json",
        ],
    )?;
    require(
        check_output.status.code() == Some(1),
        format!("ambiguous check must fail the gate: {check_output:?}"),
    )?;
    let check: Value = serde_json::from_str(&stdout(&check_output)?)
        .map_err(|error| format!("parse ambiguous check: {error}"))?;
    let ambiguous = check
        .get("outcomes")
        .and_then(Value::as_array)
        .and_then(|outcomes| {
            outcomes
                .iter()
                .find(|outcome| outcome.get("status").and_then(Value::as_str) == Some("ambiguous"))
        })
        .ok_or("check must retain the ambiguous finding")?;
    require(
        ambiguous.get("candidate_ids") == field(&detailed, &["outcome", "candidate_ids"])
            && ambiguous.get("allow_id") == Some(&Value::Null)
            && ambiguous
                .get("finding_index")
                .and_then(Value::as_u64)
                .is_some_and(|index| index > 0),
        format!("check/why must agree before entry-local index remapping: {ambiguous}"),
    )?;
    let worklist_output = run(
        &root,
        &["worklist", "--status", "ambiguous", "--format", "json"],
    )?;
    require(
        worklist_output.status.success(),
        format!("ambiguous worklist must remain inspectable: {worklist_output:?}"),
    )?;
    let worklist: Value = serde_json::from_str(&stdout(&worklist_output)?)
        .map_err(|error| format!("parse ambiguous worklist: {error}"))?;
    require(
        worklist.pointer("/work_items/0/candidate_ids") == ambiguous.get("candidate_ids")
            && worklist.pointer("/work_items/0/status") == ambiguous.get("status"),
        format!("worklist/check must preserve the same ambiguity: {worklist}"),
    )?;
    for id in ["allow-tied-a", "allow-tied-b"] {
        require_explain_projection(&root, id, ambiguous)?;
    }

    // Strengthen one existing candidate with the scanner's actual identity.
    // The unique winner must stay matched; the weaker entry must not inherit
    // that finding merely because it remains in the winner's candidate_ids.
    let snippet_hash = detailed
        .pointer("/finding/identity/normalized_snippet_hash")
        .and_then(Value::as_str)
        .ok_or("why must expose the scanned finding identity")?;
    let policy_path = root.join("policy/allow.toml");
    let policy =
        fs::read_to_string(&policy_path).map_err(|error| format!("read tied policy: {error}"))?;
    fs::write(
        &policy_path,
        policy.replacen(
            "callee = \"unwrap\"",
            &format!("callee = \"unwrap\"\nnormalized_snippet_hash = \"{snippet_hash}\""),
            1,
        ),
    )
    .map_err(|error| format!("strengthen first candidate: {error}"))?;
    let stronger_output = run(
        &root,
        &[
            "check", "--kind", "panic", "--mode", "no-new", "--format", "json",
        ],
    )?;
    let stronger: Value = serde_json::from_str(&stdout(&stronger_output)?)
        .map_err(|error| format!("parse unique-winner check: {error}"))?;
    let stronger_outcomes = stronger
        .get("outcomes")
        .and_then(Value::as_array)
        .ok_or("unique-winner check must retain evaluated outcomes")?;
    for (id, status) in [("allow-tied-a", "matched"), ("allow-tied-b", "stale")] {
        let outcome = stronger_outcomes
            .iter()
            .find(|outcome| outcome.get("allow_id").and_then(Value::as_str) == Some(id))
            .ok_or_else(|| format!("unique-winner check omitted {id}"))?;
        require(
            outcome.get("status").and_then(Value::as_str) == Some(status),
            format!("{id} must have canonical {status} state: {outcome}"),
        )?;
        if status == "matched" {
            require(
                outcome.get("candidate_ids") == ambiguous.get("candidate_ids"),
                format!("unique winner must still compete with the weaker entry: {outcome}"),
            )?;
        }
        require_explain_projection(&root, id, outcome)?;
    }

    remove_temp_root(root)
}

#[test]
fn explain_lifecycle_status_preserves_tied_finding_decisions() -> Result<(), String> {
    for (lifecycle_field, entry_status) in [("expires", "expired"), ("review_after", "review_due")]
    {
        let root = temp_root(&format!("explain-{entry_status}-tie"))?;
        write_source(&root, "pub fn tied(v: Option<u8>) -> u8 { v.unwrap() }\n")?;
        fs::write(
            root.join("src/a_unique.rs"),
            "pub fn unique(v: Option<u8>) -> u8 { v.unwrap() }\n",
        )
        .map_err(|error| format!("write earlier unique finding: {error}"))?;
        fs::create_dir_all(root.join("policy"))
            .map_err(|error| format!("create lifecycle policy directory: {error}"))?;
        let mut policy = String::from(
            "schema_version = \"0.1\"\npolicy = \"cargo-allow\"\n\n\
             [requirements]\nevidence_required = false\n\
             calendar_expiry_blocks_no_new = false\n",
        );
        for (id, scope) in [
            ("allow-tied-a", "glob = \"src/*.rs\""),
            ("allow-tied-b", "path = \"src/lib.rs\""),
        ] {
            policy.push_str(&format!(
                "\n[[allow]]\nid = \"{id}\"\nkind = \"panic\"\nfamily = \"unwrap\"\n\
                 {scope}\nowner = \"core\"\nclassification = \"reviewed_exception\"\n\
                 reason = \"Retain lifecycle state and exact competing candidates.\"\n\
                 evidence = [\"test:lifecycle_tie\"]\ncreated = \"1999-01-01\"\n\
                 {lifecycle_field} = \"2000-01-01\"\n\n\
                 [allow.selector]\nast_kind = \"method_call\"\ncallee = \"unwrap\"\n"
            ));
        }
        fs::write(root.join("policy/allow.toml"), policy)
            .map_err(|error| format!("write lifecycle policy: {error}"))?;
        git_commit_fixture(&root)?;

        let check_output = run(
            &root,
            &[
                "check", "--kind", "panic", "--mode", "no-new", "--format", "json",
            ],
        )?;
        require(
            check_output.status.code() == Some(1),
            format!("the tied finding must still block no-new: {check_output:?}"),
        )?;
        let check: Value = serde_json::from_str(&stdout(&check_output)?)
            .map_err(|error| format!("parse lifecycle check: {error}"))?;
        let outcomes = check
            .get("outcomes")
            .and_then(Value::as_array)
            .ok_or("lifecycle check must contain outcomes")?;
        let statuses = outcomes
            .iter()
            .filter_map(|outcome| outcome.get("status").and_then(Value::as_str))
            .collect::<Vec<_>>();
        require(
            statuses == [entry_status, "ambiguous"],
            format!("fixture must retain an earlier lifecycle row before the tie: {outcomes:?}"),
        )?;
        for id in ["allow-tied-a", "allow-tied-b"] {
            require_lifecycle_explain_projection(&root, id, entry_status, outcomes)?;
        }
        remove_temp_root(root)?;
    }
    Ok(())
}

fn require_lifecycle_explain_projection(
    root: &Path,
    id: &str,
    entry_status: &str,
    check_outcomes: &[Value],
) -> Result<(), String> {
    let expected = check_outcomes
        .iter()
        .filter(|outcome| {
            outcome.get("allow_id").and_then(Value::as_str) == Some(id)
                || outcome
                    .get("candidate_ids")
                    .and_then(Value::as_array)
                    .is_some_and(|ids| ids.iter().any(|candidate| candidate.as_str() == Some(id)))
        })
        .cloned()
        .enumerate()
        .map(|(index, mut outcome)| {
            if let Some(finding_index) = outcome.get_mut("finding_index") {
                *finding_index = Value::from(index);
            }
            outcome
        })
        .collect::<Vec<_>>();
    let sidecar = root.join(format!("explain-{id}-summary.json"));
    let sidecar_text = sidecar.to_string_lossy().to_string();
    let output = run(
        root,
        &[
            "--command-summary-output",
            &sidecar_text,
            "explain",
            id,
            "--format",
            "json",
        ],
    )?;
    require(
        output.status.success(),
        format!("explain JSON failed: {output:?}"),
    )?;
    let detail: Value = serde_json::from_str(&stdout(&output)?)
        .map_err(|error| format!("parse lifecycle explain: {error}"))?;
    let summary: Value = serde_json::from_str(
        &fs::read_to_string(&sidecar)
            .map_err(|error| format!("read lifecycle summary: {error}"))?,
    )
    .map_err(|error| format!("parse lifecycle summary: {error}"))?;
    require(
        detail.pointer("/summary/current_status") == Some(&Value::from(entry_status))
            && detail.get("match_outcomes") == Some(&Value::from(expected))
            && summary.pointer("/reason/code")
                == Some(&Value::from(format!("explain.{entry_status}")))
            && summary.get("result_class") == Some(&Value::from("findings"))
            && summary.get("posture") == Some(&Value::from("decision_required"))
            && summary.pointer("/operation_effects/writes_repository") == Some(&Value::Bool(false)),
        format!(
            "entry lifecycle and blocking tie must remain distinct and consistent: {detail}\n{summary}"
        ),
    )?;
    let action = detail
        .pointer("/next/suggested_actions/0")
        .and_then(Value::as_str)
        .ok_or("tied lifecycle entry must retain its ambiguity decision")?;
    require(
        action.contains("allow-tied-a")
            && action.contains("allow-tied-b")
            && summary.pointer("/primary_action/title") == Some(&Value::from(action))
            && summary.pointer("/primary_action/kind") == Some(&Value::from("decision"))
            && summary
                .pointer("/reason/message")
                .and_then(Value::as_str)
                .is_some_and(|message| message.contains("ambiguous")),
        format!("lifecycle annotation must not hide the unresolved competitor: {summary}"),
    )?;
    let output = run(
        root,
        &["--command-summary-output", &sidecar_text, "explain", id],
    )?;
    require(
        output.status.success(),
        format!("explain human failed: {output:?}"),
    )?;
    let human = stdout(&output)?;
    let human_summary: Value = serde_json::from_str(
        &fs::read_to_string(&sidecar)
            .map_err(|error| format!("read human lifecycle summary: {error}"))?,
    )
    .map_err(|error| format!("parse human lifecycle summary: {error}"))?;
    require(
        summary == human_summary
            && human.contains("Outcome: findings (decision_required)")
            && human.contains(&format!("current_status: {entry_status}"))
            && human.contains("ambiguous")
            && human.contains(&format!("Next: {action}")),
        format!("human and JSON lifecycle/tie projections diverged: {human}"),
    )
}

/// Compare the actual explain consumer with the canonical check outcome,
/// including its human screen and common summary artifact (#4350).
fn require_explain_projection(
    root: &Path,
    id: &str,
    expected_outcome: &Value,
) -> Result<(), String> {
    let status = expected_outcome
        .get("status")
        .and_then(Value::as_str)
        .ok_or("expected outcome must have a status")?;
    let (result_class, posture, reason_code) = match status {
        "ambiguous" => ("findings", "decision_required", "explain.ambiguous"),
        "matched" => ("completed", "satisfied", "explain.entry_healthy"),
        "stale" => ("findings", "advisory", "explain.stale"),
        _ => return Err(format!("unsupported fixture outcome: {expected_outcome}")),
    };
    let has_finding = expected_outcome
        .get("finding_index")
        .and_then(Value::as_u64)
        .is_some();
    let finding_count = if has_finding { 1u64 } else { 0 };
    let mut expected = expected_outcome.clone();
    if has_finding {
        let index = expected
            .get_mut("finding_index")
            .ok_or("finding-level outcome must carry an index")?;
        *index = Value::from(0);
    }
    let sidecar = root.join(format!("explain-{id}-summary.json"));
    let sidecar_text = sidecar.to_string_lossy().to_string();
    let json_output = run(
        root,
        &[
            "--command-summary-output",
            &sidecar_text,
            "explain",
            id,
            "--format",
            "json",
        ],
    )?;
    require(
        json_output.status.success(),
        format!("explain {id} JSON must remain inspectable: {json_output:?}"),
    )?;
    let detail: Value = serde_json::from_str(&stdout(&json_output)?)
        .map_err(|error| format!("parse explain {id}: {error}"))?;
    let schema: Value =
        serde_json::from_str(include_str!("../../../docs/schemas/explain.schema.json"))
            .map_err(|error| format!("parse explain schema: {error}"))?;
    jsonschema::validator_for(&schema)
        .map_err(|error| format!("compile explain schema: {error}"))?
        .validate(&detail)
        .map_err(|error| format!("explain {id} must satisfy its schema: {error}"))?;
    require(
        field(&detail, &["allow_entry", "id"]) == Some(&Value::from(id))
            && field(&detail, &["summary", "current_status"]) == Some(&Value::from(status))
            && field(&detail, &["summary", "current_matches"]) == Some(&Value::from(finding_count))
            && detail.get("match_outcomes") == Some(&Value::from(vec![expected]))
            && detail
                .get("current_findings")
                .and_then(Value::as_array)
                .is_some_and(|findings| findings.len() as u64 == finding_count),
        format!("explain {id} must project only its canonical outcome: {detail}"),
    )?;
    if has_finding {
        require(
            detail.pointer("/current_findings/0/status") == Some(&Value::from(status))
                && detail.pointer("/current_findings/0/path") == Some(&Value::from("src/lib.rs"))
                && detail.pointer("/current_findings/0/line") == Some(&Value::from(1)),
            format!("explain {id} must remap its finding without unrelated state: {detail}"),
        )?;
    }
    let summary: Value = serde_json::from_str(
        &fs::read_to_string(&sidecar)
            .map_err(|error| format!("read explain {id} summary: {error}"))?,
    )
    .map_err(|error| format!("parse explain {id} summary: {error}"))?;
    require(
        summary.get("result_class") == Some(&Value::from(result_class))
            && summary.get("posture") == Some(&Value::from(posture))
            && field(&summary, &["reason", "code"]) == Some(&Value::from(reason_code))
            && field(&summary, &["operation_effects", "writes_repository"])
                == Some(&Value::Bool(false)),
        format!("explain {id} summary must retain the evaluated posture: {summary}"),
    )?;
    if status == "ambiguous" {
        let action = detail
            .pointer("/next/suggested_actions/0")
            .and_then(Value::as_str)
            .ok_or("ambiguous explain must suggest a decision")?;
        require(
            action.contains("allow-tied-a")
                && action.contains("allow-tied-b")
                && field(&summary, &["primary_action", "title"]) == Some(&Value::from(action)),
            format!("explain {id} detail and summary must name competing entries: {summary}"),
        )?;
    }
    let human_output = run(
        root,
        &["--command-summary-output", &sidecar_text, "explain", id],
    )?;
    require(
        human_output.status.success(),
        format!("explain {id} human output must remain inspectable: {human_output:?}"),
    )?;
    let human = stdout(&human_output)?;
    let human_summary: Value = serde_json::from_str(
        &fs::read_to_string(&sidecar)
            .map_err(|error| format!("read human explain {id} summary: {error}"))?,
    )
    .map_err(|error| format!("parse human explain {id} summary: {error}"))?;
    let outcome_label = if status == "matched" {
        "Outcome: satisfied".to_string()
    } else {
        format!("Outcome: {result_class} ({posture})")
    };
    require(
        summary == human_summary
            && human.contains(&outcome_label)
            && human.contains(&format!("current_status: {status}"))
            && human.contains(&format!("current_matches: {finding_count}"))
            && !human.contains("src/a_unrelated.rs"),
        format!("explain {id} text/JSON/common-summary parity failed: {human}"),
    )?;
    if status == "ambiguous" {
        let message = expected_outcome
            .get("message")
            .and_then(Value::as_str)
            .ok_or("ambiguous outcome must explain the tie")?;
        let action = detail
            .pointer("/next/suggested_actions/0")
            .and_then(Value::as_str)
            .ok_or("ambiguous explain must suggest a decision")?;
        require(
            human.contains(message) && human.contains(&format!("Next: {action}")),
            format!("explain {id} must show competing IDs in attention and Next: {human}"),
        )?;
    }
    Ok(())
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

/// `why` writes `--plan` relative to the working directory, so a collision is
/// a property of the resolved files, not of the requested spellings: an
/// absolute sidecar path and a working-directory-relative plan path that land
/// on one file must still be refused, and the plan must survive the refusal.
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
    // From `parent`, both artifacts resolve to `<root>/collision.json`: the
    // plan spells it relative to the working directory, the sidecar absolute.
    let sidecar_arg = root.join("collision.json").to_string_lossy().to_string();
    let plan_arg = format!("{name}/collision.json");
    let mut argv = vec!["--command-summary-output", &sidecar_arg];
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

/// The mirror of the case above: `adopt` resolves `--output` under the root
/// while the sidecar resolves against the working directory, so one relative
/// spelling can land on two different files. Identical argv for both flags
/// must therefore be allowed when the resolved files differ.
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
    // The same `<name>/plan.json` spelling resolves differently per flag: the
    // sidecar lands on `<root>/plan.json` (working directory), adopt's output
    // on `<root>/<name>/plan.json` (source-tree root). Distinct files, so the
    // run must succeed and the sidecar must be written to the accepted path.
    let path_arg = format!("{name}/plan.json");
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .current_dir(&parent)
        .args(["--command-summary-output", &path_arg, "adopt"])
        .args(["--output", &path_arg])
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

/// #4363: a relative `--command-summary-output` resolves against the working
/// directory — the same base as `--output` — and a resolved path outside the
/// source-tree root fails closed with the same `E0002_INVALID_CONFIG` surface
/// `doctor` already emits. A sidecar requested from a scratch directory must
/// therefore never be silently relocated into the scanned tree, on any
/// command that accepts the flag: they all share one validation funnel.
#[test]
fn a_relative_summary_path_from_outside_the_root_never_writes_into_the_repo() -> Result<(), String>
{
    let root = temp_root("summary-cwd-base")?;
    write_source(&root, "pub fn value(v: Option<u8>) -> u8 { v.unwrap() }\n")?;
    let parent = root
        .parent()
        .ok_or_else(|| "temp root needs a parent".to_string())?
        .to_path_buf();
    let name = root
        .file_name()
        .ok_or_else(|| "temp root needs a name".to_string())?
        .to_string_lossy()
        .to_string();

    // `init` goes first: its own policy write precedes the summary stage, so
    // the refusal must leave the policy in place while the sidecar lands
    // nowhere. The commit it enables is what lets `diff --base HEAD` below
    // reach its own summary stage.
    let init_output = run_summary_from_outside(&parent, &name, "init", &["init"])?;
    require(
        root.join("policy/allow.toml").is_file(),
        format!(
            "init's own policy write must survive the sidecar refusal, got {:?} / {}",
            init_output.status,
            String::from_utf8_lossy(&init_output.stderr)
        ),
    )?;
    require_summary_refused("init", &init_output, &root, &parent)?;
    git_commit_fixture(&root)?;

    let commands: [(&str, &[&str]); 8] = [
        ("adopt", &["adopt"]),
        ("doctor", &["doctor"]),
        ("audit", &["audit"]),
        ("check", CHECK_ARGV),
        ("explain", EXPLAIN_ARGV),
        ("why", WHY_ARGV),
        ("diff", &["diff", "--base", "HEAD"]),
        ("worklist", WORKLIST_ARGV),
    ];
    for (label, argv) in commands {
        let output = run_summary_from_outside(&parent, &name, label, argv)?;
        require_summary_refused(label, &output, &root, &parent)?;
    }

    remove_temp_root(root)
}

/// Run one command from `parent` — outside the scanned `<parent>/<name>` —
/// with a relative `--command-summary-output`.
fn run_summary_from_outside(
    parent: &Path,
    name: &str,
    label: &str,
    argv: &[&str],
) -> Result<Output, String> {
    let sidecar = format!("{label}-sidecar.json");
    let mut args: Vec<&str> = vec!["--command-summary-output", &sidecar];
    args.extend(argv.iter().copied());
    Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .current_dir(parent)
        .args(&args)
        .arg("--root")
        .arg(name)
        .output()
        .map_err(|error| format!("run {args:?}: {error}"))
}

/// The fail-closed contract: non-zero exit naming the containment error, and
/// the sidecar present nowhere — not beside the scanned tree, and never
/// relocated into it.
fn require_summary_refused(
    label: &str,
    output: &Output,
    root: &Path,
    parent: &Path,
) -> Result<(), String> {
    let stderr = String::from_utf8_lossy(&output.stderr);
    require(
        !output.status.success(),
        format!(
            "{label} must refuse a sidecar that resolves outside the root, got {:?} / {stderr}",
            output.status
        ),
    )?;
    require(
        stderr.contains("E0002_INVALID_CONFIG") && stderr.contains("outside the source-tree root"),
        format!("{label} refusal lost the containment surface: {stderr}"),
    )?;
    let sidecar = format!("{label}-sidecar.json");
    require(
        !parent.join(&sidecar).exists(),
        format!("{label} wrote the sidecar beside the scanned tree after all"),
    )?;
    require(
        !root.join(&sidecar).exists(),
        format!("{label} silently relocated the sidecar into the scanned tree"),
    )?;
    Ok(())
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
            // An unmigrated command that still parses `--root`, so the refusal
            // comes from the router gate rather than clap argv parsing.
            "list",
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
    // #4393: the rejection names the one authoritative set, including the
    // mutation commands that `--help` and the schema docs used to omit.
    let stderr = String::from_utf8_lossy(&output.stderr);
    require(
        stderr.contains("currently supports")
            && stderr.contains("refresh, prune, migrate")
            && stderr.contains("and worklist commands only"),
        format!("rejection must name the authoritative supported set: {stderr}"),
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

/// #4393 gap 1: every command the authoritative supported set names must
/// actually emit the sidecar — a probe of the whole set, each with
/// success-path argv, so `--help`, the schema docs, the rejection message,
/// and the router cannot disagree about what emits.
#[test]
fn every_supported_command_emits_a_summary_sidecar() -> Result<(), String> {
    let root = temp_root("summary-supported-set")?;
    write_source(&root, "pub fn value(v: Option<u8>) -> u8 { v.unwrap() }\n")?;
    run(&root, &["init"])?;
    git_commit_fixture(&root)?;

    // `add --update` below receipts the finding as `allow-0002` (`init` seeds
    // `allow-0001` for the ledger itself), which gives `explain` a real entry.
    for (label, command) in [
        ("adopt", vec!["adopt"]),
        ("doctor", vec!["doctor"]),
        ("audit", vec!["audit"]),
        ("check", vec!["check", "--mode", "no-new"]),
        ("diff", vec!["diff", "--base", "HEAD"]),
        ("worklist", vec!["worklist"]),
        (
            "why",
            vec![
                "why",
                "--kind",
                "panic",
                "--path",
                "src/lib.rs",
                "--line",
                "1",
            ],
        ),
        ("propose", vec!["propose"]),
        ("init", vec!["init", "--config", "policy/allow.probe.toml"]),
        (
            "add",
            vec![
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
                "supported-set probe",
                "--update",
            ],
        ),
        ("explain", vec!["explain", "allow-0002"]),
        ("prune", vec!["prune", "--stale", "--dry-run"]),
        (
            "migrate",
            vec![
                "migrate",
                "--from",
                "legacy-ledger.toml",
                "--output",
                "policy/allow.migrated.toml",
                "--force",
            ],
        ),
        // `refresh` needs real location drift: move the finding one line down
        // after `add` pinned it, so the dry-run preview has something to show.
        (
            "refresh",
            vec!["refresh", "--allow-id", "allow-0002", "--dry-run"],
        ),
    ] {
        if label == "refresh" {
            // Drift fires only past the #1808 line tolerance (3), so move the
            // finding well clear of where `add` pinned its last_seen.
            let mut drifted = "pub fn padding() {}\n".repeat(8);
            drifted.push_str("pub fn value(v: Option<u8>) -> u8 { v.unwrap() }\n");
            fs::write(root.join("src/lib.rs"), drifted)
                .map_err(|error| format!("rewrite source for drift: {error}"))?;
        }
        if label == "migrate" {
            fs::write(
                root.join("legacy-ledger.toml"),
                LEGACY_BESPOKE_LEDGER_FIXTURE,
            )
            .map_err(|error| format!("write legacy ledger: {error}"))?;
        }
        let sidecar = root.join(format!("{label}-probe-summary.json"));
        let sidecar_text = sidecar.to_string_lossy().to_string();
        let mut argv: Vec<&str> = vec!["--command-summary-output", &sidecar_text];
        argv.extend(command.iter().copied());
        // `migrate` resolves `--from` and `--output` against the process
        // working directory, not `--root`, so both must be spelled absolutely:
        // a relative `--output` would otherwise write into the test runner's
        // own working directory instead of the fixture.
        let legacy = root
            .join("legacy-ledger.toml")
            .to_string_lossy()
            .to_string();
        let migrated = root
            .join("policy/allow.migrated.toml")
            .to_string_lossy()
            .to_string();
        let argv: Vec<&str> = argv
            .into_iter()
            .map(|arg| match arg {
                "legacy-ledger.toml" => legacy.as_str(),
                "policy/allow.migrated.toml" => migrated.as_str(),
                other => other,
            })
            .collect();
        let output = run(&root, &argv)?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        require(
            sidecar.exists(),
            format!("{label} must emit the summary sidecar, got {stderr}"),
        )?;
        let summary: Value = serde_json::from_str(
            &fs::read_to_string(&sidecar)
                .map_err(|error| format!("read {label} summary: {error}"))?,
        )
        .map_err(|error| format!("parse {label} summary: {error}"))?;
        require(
            field(&summary, &["operation"]) == Some(&Value::from(label))
                && field(&summary, &["schema_id"])
                    == Some(&Value::from("cargo-allow.core-command-summary.v1")),
            format!("{label} sidecar lost its operation identity: {summary}"),
        )?;
    }

    remove_temp_root(root)
}

/// #4393 gap 2: an adoption Next whose argv carries an unsubstituted
/// `<placeholder>` is not executable as printed, and the plan's facts carry
/// counts rather than the reference the placeholder stands for — so the
/// summary must project the step as a decision, never as a template command.
#[test]
fn adopt_projects_a_placeholder_next_step_as_a_decision() -> Result<(), String> {
    let root = temp_root("summary-adopt-placeholder")?;
    write_source(&root, "pub fn value(v: Option<u8>) -> u8 { v.unwrap() }\n")?;
    run(&root, &["init"])?;
    git_commit_fixture(&root)?;

    let sidecar = root.join("adopt-summary.json");
    let output = run(
        &root,
        &[
            "--command-summary-output",
            &sidecar.to_string_lossy(),
            "adopt",
        ],
    )?;
    require(
        output.status.success(),
        format!("adopt failed: {}", String::from_utf8_lossy(&output.stderr)),
    )?;
    let summary: Value = serde_json::from_str(
        &fs::read_to_string(&sidecar).map_err(|error| format!("read adopt summary: {error}"))?,
    )
    .map_err(|error| format!("parse adopt summary: {error}"))?;
    // Pin the disposition so the fixture cannot silently drift away from the
    // placeholder-carrying `inspect_new_finding` primary route.
    require(
        field(&summary, &["reason", "code"])
            == Some(&Value::from("adoption.existing_policy_has_new_findings")),
        format!("fixture must hit the new-finding adoption route: {summary}"),
    )?;
    let primary = summary
        .get("primary_action")
        .ok_or_else(|| "adopt summary needs a primary action".to_string())?;
    require(
        field(primary, &["kind"]) == Some(&Value::from("decision"))
            && field(primary, &["program"]).is_none()
            && field(primary, &["args"])
                .and_then(Value::as_array)
                .is_none_or(|args| args.is_empty()),
        format!("placeholder next step must be a decision, not a template command: {primary}"),
    )?;
    require(
        field(primary, &["expected_effect"])
            .and_then(Value::as_str)
            .is_some_and(|effect| effect.contains("why") && effect.contains("<finding>")),
        format!("decision must carry the template and its named input: {primary}"),
    )?;
    for action in ["primary_action", "next_proof"] {
        if let Some(args) = summary
            .get(action)
            .and_then(|action| action.get("args"))
            .and_then(Value::as_array)
        {
            require(
                args.iter().all(|arg| {
                    arg.as_str()
                        .is_none_or(|arg| !(arg.starts_with('<') && arg.ends_with('>')))
                }),
                format!("{action} args carry placeholder syntax: {args:?}"),
            )?;
        }
    }
    let human = stdout(&output)?;
    require(
        human.contains("Next: Choose the concrete input for the inspect_new_finding step"),
        format!("adopt human Next must present the decision route: {human}"),
    )?;
    require(
        !human.contains("\"<finding>\""),
        format!("adopt human Next must not print the template as a command: {human}"),
    )?;

    remove_temp_root(root)
}

/// #4393 gap 3: one human screen must never print two different words under
/// the `Result:` label. The grammar block states the #3148 class word as
/// `Outcome:`; the report tail keeps the single gate-verdict `Result:` line.
#[test]
fn one_human_screen_prints_the_result_word_once() -> Result<(), String> {
    let root = temp_root("summary-dual-result")?;
    write_source(&root, "pub fn value(v: Option<u8>) -> u8 { v.unwrap() }\n")?;
    run(&root, &["init"])?;
    git_commit_fixture(&root)?;

    let audit = stdout(&run(&root, &["audit"])?)?;
    require(
        audit
            .lines()
            .next()
            .is_some_and(|line| line.starts_with("Outcome: findings (advisory)")),
        format!("audit summary must open with the Outcome class line: {audit}"),
    )?;
    let audit_result_lines = result_lines(&audit);
    require(
        audit_result_lines.len() == 1
            && audit_result_lines
                .first()
                .is_some_and(|line| line.starts_with("Result: passed")),
        format!(
            "audit screen must keep exactly one gate-verdict Result line: {audit_result_lines:?}"
        ),
    )?;

    let check = stdout(&run(&root, &["check", "--mode", "no-new"])?)?;
    require(
        check
            .lines()
            .next()
            .is_some_and(|line| line.starts_with("Outcome: findings (blocking)")),
        format!("check summary must open with the Outcome class line: {check}"),
    )?;
    let check_result_lines = result_lines(&check);
    require(
        check_result_lines.len() == 1
            && check_result_lines
                .first()
                .is_some_and(|line| line.starts_with("Result: failed")),
        format!(
            "check screen must keep exactly one gate-verdict Result line: {check_result_lines:?}"
        ),
    )?;
    // The class word lives on the Outcome line only: no `Result:` line may
    // carry it, so no screen states the result twice with two words.
    for text in [&audit, &check] {
        require(
            result_lines(text)
                .iter()
                .all(|line| !line.starts_with("Result: findings")),
            format!("a Result line claims the class word: {text}"),
        )?;
    }

    remove_temp_root(root)
}

/// #4393 secondary observation: the scan-surface Next commands a summary
/// suggests resolve their configuration under the source-tree root, so as-
/// printed execution from a scratch consumer cwd used to fail with `E0002`.
/// The suggested argv carries `--root`, so a wrapper can chain it from any cwd.
#[test]
fn summary_next_commands_carry_the_root_and_run_from_a_foreign_cwd() -> Result<(), String> {
    let root = temp_root("summary-rooted-next")?;
    let scratch = temp_root("summary-scratch-cwd")?;
    write_source(&root, "pub fn value(v: Option<u8>) -> u8 { v.unwrap() }\n")?;
    run(&root, &["init"])?;
    git_commit_fixture(&root)?;

    // The consumer runs from a scratch cwd: the sidecar is requested with an
    // absolute in-root path (#4363 containment) and the scan target with
    // `--root`, exactly as a wrapper chaining one command to the next would.
    let sidecar = root.join("check-summary.json");
    let sidecar_text = sidecar.to_string_lossy().to_string();
    // The suggested argv spells the root in the portable forward-slash form
    // `normalize_path` produces, so compare against that spelling.
    let root_native = root.to_string_lossy().to_string();
    let output = run_from(
        &scratch,
        &[
            "--command-summary-output",
            &sidecar_text,
            "check",
            "--mode",
            "no-new",
            "--root",
            &root_native,
        ],
    )?;
    require(
        output.status.code() == Some(1),
        format!(
            "a blocking check must exit with the gate code 1, got {:?}",
            output.status.code()
        ),
    )?;
    let summary: Value = serde_json::from_str(
        &fs::read_to_string(&sidecar).map_err(|error| format!("read check summary: {error}"))?,
    )
    .map_err(|error| format!("parse check summary: {error}"))?;
    let argv = field(&summary, &["primary_action", "args"])
        .and_then(Value::as_array)
        .ok_or_else(|| "a blocking check must route to the worklist".to_string())?;
    // Compare the root by canonicalized path, not exact spelling: on some
    // Windows runners `std::env::temp_dir()` spells the directory in 8.3
    // short form while the tool's `normalize_path` output spells it out.
    let root_canonical = std::fs::canonicalize(&root)
        .map_err(|error| format!("canonicalize fixture root: {error}"))?;
    let argv_root = argv
        .last()
        .and_then(Value::as_str)
        .ok_or("the suggested next command must end with the root path")?;
    let argv_root_canonical = std::fs::canonicalize(argv_root)
        .map_err(|error| format!("canonicalize suggested root {argv_root}: {error}"))?;
    require(
        argv.first() == Some(&Value::from("worklist"))
            && argv.iter().any(|arg| arg.as_str() == Some("--root"))
            && argv_root_canonical == root_canonical,
        format!("the suggested next command must carry --root <root>: {argv:?}"),
    )?;
    // Executed as printed from the scratch cwd, the suggestion must reach the
    // subject repository instead of failing config discovery (`E0002`, exit 2).
    let next_argv: Vec<String> = argv
        .iter()
        .map(|arg| arg.as_str().unwrap_or_default().to_string())
        .collect();
    let next = Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .args(&next_argv)
        .current_dir(&scratch)
        .output()
        .map_err(|error| format!("run suggested next from scratch cwd: {error}"))?;
    require(
        next.status.success(),
        format!(
            "the suggested next command must run from a scratch cwd: {}",
            String::from_utf8_lossy(&next.stderr)
        ),
    )?;
    // The human block carries the same executable suggestion.
    let human = stdout(&output)?;
    require(
        human
            .lines()
            .next()
            .is_some_and(|line| line.starts_with("Outcome: findings (blocking)")),
        format!("check must open with the blocking Outcome line: {human}"),
    )?;
    require(
        human.contains("--root"),
        format!("the human Next line must carry --root: {human}"),
    )?;

    remove_temp_root(root)?;
    remove_temp_root(scratch)
}

/// #4393: a summary-supported command that exits on a hard error still writes
/// its configured sidecar, classified by the typed `E000x` code, so the
/// failure direction is reachable without parsing stderr.
#[test]
fn a_hard_error_writes_the_e000x_classified_summary_sidecar() -> Result<(), String> {
    let root = temp_root("summary-hard-error")?;
    write_source(&root, "pub fn value(v: Option<u8>) -> u8 { v.unwrap() }\n")?;
    // A tracked but clean file: `why` must scan it, find no matching finding,
    // and exit on the typed usage error ("no current panic finding found").
    fs::create_dir_all(root.join("src")).map_err(|error| error.to_string())?;
    fs::write(root.join("src/other.rs"), "pub fn clean() {}\n")
        .map_err(|error| error.to_string())?;
    run(&root, &["init"])?;
    git_commit_fixture(&root)?;

    // `src/other.rs` has no finding, so `why` exits on a typed usage error.
    let sidecar = root.join("why-error-summary.json");
    fs::write(
        &sidecar,
        r#"{"result_class":"completed","sentinel":"stale"}"#,
    )
    .map_err(|error| format!("seed stale hard-error summary: {error}"))?;
    let sidecar_text = sidecar.to_string_lossy().to_string();
    let output = run(
        &root,
        &[
            "--command-summary-output",
            &sidecar_text,
            "why",
            "--kind",
            "panic",
            "--path",
            "src/other.rs",
            "--line",
            "1",
        ],
    )?;
    require(
        output.status.code() == Some(2),
        format!(
            "a usage hard error must exit 2, got {:?}",
            output.status.code()
        ),
    )?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    require(
        stderr.contains("E0001"),
        format!("the typed usage error must reach stderr: {stderr}"),
    )?;
    require(
        sidecar.exists(),
        "the hard-error path must still write the configured sidecar",
    )?;
    let summary: Value = serde_json::from_str(
        &fs::read_to_string(&sidecar).map_err(|error| format!("read error summary: {error}"))?,
    )
    .map_err(|error| format!("parse error summary: {error}"))?;
    require(
        field(&summary, &["operation"]) == Some(&Value::from("why"))
            && field(&summary, &["result_class"]) == Some(&Value::from("malformed_input"))
            && field(&summary, &["posture"]) == Some(&Value::from("blocking"))
            && field(&summary, &["reason", "code"]) == Some(&Value::from("E0001_USAGE")),
        format!("the sidecar must classify the typed hard error: {summary}"),
    )?;
    require(
        field(&summary, &["reason", "message"])
            .and_then(Value::as_str)
            .is_some_and(|message| message.contains("src/other.rs:1")),
        format!("the failure reason must reach the sidecar: {summary}"),
    )?;

    remove_temp_root(root)
}

#[test]
fn adoption_evaluated_errors_preserve_their_summary_and_detailed_output() -> Result<(), String> {
    for (label, invalid_policy, partial, disposition, completeness, class, reason, exit) in [
        (
            "summary-adopt-invalid",
            true,
            false,
            "InvalidPolicy",
            "complete",
            "malformed_input",
            "adoption.invalid_policy",
            1,
        ),
        (
            "summary-adopt-partial",
            false,
            true,
            "PartialInventory",
            "partial",
            "partial_data",
            "adoption.partial_inventory",
            1,
        ),
        (
            "summary-adopt-healthy",
            false,
            false,
            "ExistingPolicyHealthy",
            "complete",
            "completed",
            "adoption.existing_policy_healthy",
            0,
        ),
    ] {
        let root = summary_outcome_fixture(label, partial)?;
        if invalid_policy {
            fs::write(root.join("policy/allow.toml"), "not = [valid\n")
                .map_err(|error| format!("write invalid policy: {error}"))?;
        }
        let (detail, summary) = summary_with_unchanged_output(
            &root,
            &["adopt", "--config", "policy/allow.toml", "--format", "json"],
            exit,
        )?;
        require(
            field(&detail, &["plan", "bootstrap_disposition"]) == Some(&Value::from(disposition))
                && field(&detail, &["plan", "inventory", "completeness"])
                    == Some(&Value::from(if partial { "Partial" } else { "Complete" })),
            format!("{label} must retain the evaluated adoption plan: {detail}"),
        )?;
        require(
            field(&summary, &["operation"]) == Some(&Value::from("adopt"))
                && field(&summary, &["result_class"]) == Some(&Value::from(class))
                && field(&summary, &["completeness"]) == Some(&Value::from(completeness))
                && field(&summary, &["reason", "code"]) == Some(&Value::from(reason)),
            format!("{label} must retain the evaluated summary: {summary}"),
        )?;
        remove_temp_root(root)?;
    }
    Ok(())
}

#[test]
fn doctor_require_clean_keeps_partial_coverage_in_the_summary() -> Result<(), String> {
    for (label, partial, require_clean, exit) in [
        ("summary-doctor-partial-strict", true, true, 1),
        ("summary-doctor-partial-relaxed", true, false, 0),
        ("summary-doctor-clean-strict", false, true, 0),
    ] {
        let root = summary_outcome_fixture(label, partial)?;
        let mut args = vec![
            "doctor",
            "--config",
            "policy/allow.toml",
            "--format",
            "json",
        ];
        if require_clean {
            args.push("--require-clean");
        }
        let (detail, summary) = summary_with_unchanged_output(&root, &args, exit)?;
        let completeness = if partial { "partial" } else { "complete" };
        require(
            field(&detail, &["scanner", "completeness"]) == Some(&Value::from(completeness))
                && field(&detail, &["scanner", "rust", "files_with_parse_errors"])
                    == Some(&Value::from(u64::from(partial))),
            format!("{label} must retain its scanner diagnosis: {detail}"),
        )?;
        require(
            field(&summary, &["operation"]) == Some(&Value::from("doctor"))
                && field(&summary, &["result_class"])
                    == Some(&Value::from(if partial {
                        "partial_data"
                    } else {
                        "completed"
                    }))
                && field(&summary, &["completeness"]) == Some(&Value::from(completeness))
                && field(&summary, &["reason", "code"])
                    == Some(&Value::from(if partial {
                        "doctor.partial_coverage"
                    } else {
                        "doctor.healthy_setup"
                    })),
            format!("{label} must not become complete because require-clean failed: {summary}"),
        )?;
        remove_temp_root(root)?;
    }
    Ok(())
}

#[test]
fn receipt_only_human_check_writes_the_same_summary_as_report_routes() -> Result<(), String> {
    let root = summary_outcome_fixture("summary-check-receipt-routes", false)?;
    let mut first_summary = None;
    for route in ["receipt-only", "json", "output"] {
        let sidecar = root.join(format!("target/probe/{route}-summary.json"));
        let receipt = root.join(format!("target/probe/{route}-receipt.json"));
        let report = root.join(format!("target/probe/{route}-report.txt"));
        let sidecar_text = sidecar.to_string_lossy().to_string();
        let receipt_text = receipt.to_string_lossy().to_string();
        let report_text = report.to_string_lossy().to_string();
        require(
            !sidecar.exists(),
            "each check route must start without a summary",
        )?;
        let mut args = vec![
            "--command-summary-output",
            &sidecar_text,
            "check",
            "--config",
            "policy/allow.toml",
            "--mode",
            "no-new",
            "--persistent-cache",
            "off",
            "--receipt",
            &receipt_text,
        ];
        match route {
            "json" => args.extend(["--format", "json"]),
            "output" => args.extend(["--output", &report_text]),
            _ => {}
        }
        let output = run(&root, &args)?;
        require(
            output.status.success(),
            format!(
                "{route} check failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        )?;
        if route == "json" {
            let _: Value = serde_json::from_slice(&output.stdout)
                .map_err(|error| format!("parse JSON check detail: {error}"))?;
        } else {
            require(
                output.stdout.is_empty(),
                format!("{route} must keep stdout quiet"),
            )?;
        }
        if route == "receipt-only" {
            require(
                String::from_utf8_lossy(&output.stderr)
                    .contains("cargo-allow check: passed (mode: no-new, receipt written to "),
                "receipt-only Human check must retain its brief stderr status",
            )?;
        } else if route == "output" {
            require(
                fs::read_to_string(&report)
                    .map_err(|error| error.to_string())?
                    .contains("Result: passed (enforcing)"),
                "the Human output file must retain its detailed gate verdict",
            )?;
        }
        let receipt = read_summary_json(&receipt)?;
        require(
            field(&receipt, &["status"]) == Some(&Value::from("passed"))
                && field(&receipt, &["failed"]) == Some(&Value::Bool(false))
                && field(&receipt, &["enforcement"]) == Some(&Value::from("enforcing"))
                && field(&receipt, &["counts", "new"]) == Some(&Value::from(0)),
            format!("{route} must retain the passing no-new receipt: {receipt}"),
        )?;
        let summary = read_summary_json(&sidecar)?;
        // Native init leaves one ordinary policy-missing-evidence advisory.
        // A passing no-new receipt must not erase that finding from the summary.
        require(
            field(&receipt, &["advisory", "policy_missing_evidence"]) == Some(&Value::from(1))
                && field(&summary, &["operation"]) == Some(&Value::from("check"))
                && field(&summary, &["result_class"]) == Some(&Value::from("findings"))
                && field(&summary, &["completeness"]) == Some(&Value::from("complete"))
                && field(&summary, &["posture"]) == Some(&Value::from("advisory")),
            format!("{route} must preserve the advisory summary: {summary}"),
        )?;
        if let Some(expected) = first_summary.as_ref() {
            require(
                &summary == expected,
                format!("{route} changed the report-derived summary"),
            )?;
        } else {
            first_summary = Some(summary);
        }
    }
    remove_temp_root(root)
}

#[test]
fn later_output_failures_replace_evaluated_summaries_with_typed_errors() -> Result<(), String> {
    for command in ["adopt", "doctor", "check"] {
        let root = summary_outcome_fixture(
            &format!("summary-late-output-{command}"),
            command != "check",
        )?;
        let sidecar = root.join("target/probe/summary.json");
        let blocked_output = root.join("target/probe/owned-directory");
        fs::create_dir(&blocked_output).map_err(|error| error.to_string())?;
        let canary = blocked_output.join("owner.txt");
        fs::write(&canary, b"prior owner's bytes\n").map_err(|error| error.to_string())?;
        let sidecar_text = sidecar.to_string_lossy().to_string();
        let blocked_text = blocked_output.to_string_lossy().to_string();
        let mut args = vec![
            "--command-summary-output",
            &sidecar_text,
            command,
            "--config",
            "policy/allow.toml",
        ];
        if command == "check" {
            // The quiet Human route emits its summary before writing the receipt.
            args.extend([
                "--mode",
                "no-new",
                "--persistent-cache",
                "off",
                "--receipt",
                &blocked_text,
            ]);
        } else {
            // Directory resolution succeeds; the later detail write cannot replace
            // a nonempty directory. This failure occurs after summary emission.
            args.extend(["--format", "json", "--output", &blocked_text]);
            if command == "doctor" {
                args.push("--require-clean");
            }
        }
        let output = run(&root, &args)?;
        require(
            output.status.code() == Some(1)
                && output.stdout.is_empty()
                && String::from_utf8_lossy(&output.stderr).contains("E0007"),
            format!("{command} must retain the artifact I/O failure: {output:?}"),
        )?;
        let summary = read_summary_json(&sidecar)?;
        require(
            field(&summary, &["operation"]) == Some(&Value::from(command))
                && field(&summary, &["result_class"]) == Some(&Value::from("instrument_failure"))
                && field(&summary, &["completeness"]) == Some(&Value::from("unknown"))
                && field(&summary, &["reason", "code"]) == Some(&Value::from("E0007_ARTIFACT"))
                && field(&summary, &["reason", "message"])
                    .and_then(Value::as_str)
                    .is_some_and(|message| message.contains("owned-directory")),
            format!(
                "{command} must not hide later I/O failure behind its evaluated outcome: {summary}"
            ),
        )?;
        require(
            fs::read(&canary).map_err(|error| error.to_string())? == b"prior owner's bytes\n",
            "failed output replacement must preserve the prior owner",
        )?;
        remove_temp_root(root)?;
    }
    Ok(())
}

#[test]
fn check_emit_usage_failure_replaces_evaluated_outputs() -> Result<(), String> {
    check_emit_route_controls(&["usage"])
}

#[test]
fn check_emit_directory_failure_replaces_evaluated_outputs() -> Result<(), String> {
    check_emit_route_controls(&["directory"])
}

#[test]
fn check_emit_write_failure_replaces_evaluated_outputs() -> Result<(), String> {
    check_emit_route_controls(&["write"])
}

#[test]
fn check_emit_preserves_success_and_evaluated_gate_failure() -> Result<(), String> {
    check_emit_route_controls(&["pass", "gate"])
}

/// Exercise every report route even when an earlier route exposes a regression.
fn check_emit_route_controls(scenarios: &[&str]) -> Result<(), String> {
    let mut failures = Vec::new();
    for scenario in scenarios {
        for route in ["receipt-only", "json", "output"] {
            if let Err(error) = check_emit_route_control(route, scenario) {
                failures.push(format!("{scenario}/{route}: {error}"));
            }
        }
    }
    require(failures.is_empty(), failures.join("\n"))
}

fn check_emit_route_control(route: &str, scenario: &str) -> Result<(), String> {
    let root = summary_outcome_fixture(&format!("summary-emit-{scenario}-{route}"), false)?;
    let result = (|| -> Result<(), String> {
        if scenario == "gate" {
            // This tracked edit introduces a genuine unreceipted finding after init.
            write_source(
                &root,
                "pub fn fixture(v: Option<u8>) -> u8 { v.unwrap() }\n",
            )?;
        }
        let sidecar = root.join("target/probe/summary.json");
        let receipt = root.join("target/probe/receipt.json");
        let report = root.join("target/probe/report.md");
        let artifacts = root.join("target/probe/artifacts");
        let canary = root.join("target/probe/prior-owner.txt");
        fs::write(&canary, b"prior owner's unrelated output\n")
            .map_err(|error| error.to_string())?;
        fs::write(
            &sidecar,
            r#"{"result_class":"completed","sentinel":"stale"}"#,
        )
        .map_err(|error| error.to_string())?;
        let mut preserved_paths = vec![
            root.join("src/lib.rs"),
            root.join("policy/allow.toml"),
            root.join(".git/HEAD"),
            root.join(".git/index"),
            canary,
        ];
        let emit = match scenario {
            "usage" => "not-a-renderer",
            "directory" => {
                fs::write(&artifacts, b"prior owner of the artifact path\n")
                    .map_err(|error| error.to_string())?;
                preserved_paths.push(artifacts.clone());
                "json"
            }
            "write" => {
                // Markdown succeeds first; a nonempty directory blocks JSON.
                // The invocation must fail even after an earlier member was written.
                let occupied = artifacts.join("check-json.json");
                fs::create_dir_all(&occupied).map_err(|error| error.to_string())?;
                let owner = occupied.join("owner.txt");
                fs::write(&owner, b"prior owner of the JSON member\n")
                    .map_err(|error| error.to_string())?;
                preserved_paths.push(owner);
                "markdown,json"
            }
            _ => "json",
        };
        let preserved = preserved_paths
            .into_iter()
            .map(|path| {
                fs::read(&path)
                    .map(|bytes| (path, bytes))
                    .map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let head_before = fixture_command("git")
            .current_dir(&root)
            .args(["rev-parse", "HEAD"])
            .output()
            .map_err(|error| error.to_string())?;
        require(head_before.status.success(), "fixture HEAD must resolve")?;
        let sidecar_text = sidecar.to_string_lossy().to_string();
        let receipt_text = receipt.to_string_lossy().to_string();
        let report_text = report.to_string_lossy().to_string();
        let artifacts_text = artifacts.to_string_lossy().to_string();
        let mut args = vec![
            "--command-summary-output",
            &sidecar_text,
            "check",
            "--config",
            "policy/allow.toml",
            "--mode",
            "no-new",
            "--persistent-cache",
            "off",
            "--receipt",
            &receipt_text,
            "--artifact-dir",
            &artifacts_text,
            "--emit",
            emit,
        ];
        match route {
            "json" => args.extend(["--format", "json"]),
            "output" => args.extend(["--output", &report_text]),
            _ => {}
        }
        let output = run(&root, &args)?;
        for (path, bytes) in preserved {
            require(
                fs::read(&path).map_err(|error| error.to_string())? == bytes,
                format!("check must preserve {}", path.display()),
            )?;
        }
        let head_after = fixture_command("git")
            .current_dir(&root)
            .args(["rev-parse", "HEAD"])
            .output()
            .map_err(|error| error.to_string())?;
        require(
            head_after.status.success() && head_before.stdout == head_after.stdout,
            "check must preserve the resolved fixture HEAD",
        )?;
        if route == "json" {
            require(
                serde_json::from_slice::<Value>(&output.stdout).is_ok(),
                format!("the already-emitted JSON detail must remain valid: {output:?}"),
            )?;
        } else {
            require(
                output.stdout.is_empty(),
                "file/receipt routes must keep stdout quiet",
            )?;
        }
        let summary = read_summary_json(&sidecar)?;
        let receipt_value = read_summary_json(&receipt)?;
        require(
            field(&summary, &["operation"]) == Some(&Value::from("check"))
                && field(&summary, &["sentinel"]).is_none(),
            format!("the final sidecar must describe this check invocation: {summary}"),
        )?;
        if matches!(scenario, "pass" | "gate") {
            let failed = scenario == "gate";
            require(
                output.status.code() == Some(i32::from(failed))
                    && field(&receipt_value, &["status"])
                        == Some(&Value::from(if failed { "failed" } else { "passed" }))
                    && field(&receipt_value, &["failed"]) == Some(&Value::Bool(failed))
                    && field(&receipt_value, &["enforcement"]) == Some(&Value::from("enforcing")),
                format!("emit must preserve the evaluated gate: {output:?}; {receipt_value}"),
            )?;
            // Native init retains one ordinary evidence advisory on the pass route.
            require(
                field(&summary, &["result_class"]) == Some(&Value::from("findings"))
                    && field(&summary, &["completeness"]) == Some(&Value::from("complete"))
                    && field(&summary, &["posture"])
                        == Some(&Value::from(if failed { "blocking" } else { "advisory" })),
                format!("emit must retain the evaluated summary: {summary}"),
            )?;
            require(
                field(&receipt_value, &["counts", "new"]) == Some(&Value::from(u64::from(failed))),
                format!(
                    "the gate control must contain exactly its intended finding: {receipt_value}"
                ),
            )?;
            read_summary_json(&artifacts.join("check-json.json"))?;
            let manifest = read_summary_json(&artifacts.join("check-artifact_set_manifest.json"))?;
            require(
                field(&manifest, &["blocking"]) == Some(&Value::Bool(failed)),
                format!("the artifact manifest must retain the gate posture: {manifest}"),
            )?;
            require(
                report.is_file() == (route == "output"),
                "successful output routing changed",
            )?;
        } else {
            let (exit, class, code) = if scenario == "usage" {
                (2, "malformed_input", "E0001_USAGE")
            } else {
                (1, "instrument_failure", "E0007_ARTIFACT")
            };
            require(
                output.status.code() == Some(exit)
                    && String::from_utf8_lossy(&output.stderr).contains(code),
                format!(
                    "late emit failure must return {code}: exit {:?}; stderr {}",
                    output.status.code(),
                    String::from_utf8_lossy(&output.stderr),
                ),
            )?;
            require(
                field(&summary, &["result_class"]) == Some(&Value::from(class))
                    && field(&summary, &["completeness"]) == Some(&Value::from("unknown"))
                    && field(&summary, &["posture"]) == Some(&Value::from("blocking"))
                    && field(&summary, &["reason", "code"]) == Some(&Value::from(code)),
                format!("late emit failure must replace the evaluated summary: {summary}"),
            )?;
            require(
                field(&receipt_value, &["status"]) == Some(&Value::from("error"))
                    && field(&receipt_value, &["failed"]) == Some(&Value::Bool(true))
                    // Error receipts retain the plain diagnostic, while the
                    // command summary carries the stable typed error code.
                    && field(&receipt_value, &["diagnostic"])
                        == field(&summary, &["reason", "message"]),
                format!("late emit failure must replace the passing receipt: {receipt_value}"),
            )?;
            require(
                !report.exists(),
                "late emit failure must remove the stale detail file",
            )?;
            require(
                !artifacts.join("check-artifact_set_manifest.json").exists(),
                "these failed emissions must not claim a completed artifact manifest",
            )?;
            if scenario == "write" {
                require(
                    artifacts.join("check-markdown.md").is_file(),
                    "write-failure control must execute after the first artifact succeeds",
                )?;
            }
        }
        Ok(())
    })();
    let cleanup = remove_temp_root(root);
    result.and(cleanup)
}

/// Use native init and a tracked inventory, then select complete or partial Rust input.
fn summary_outcome_fixture(label: &str, partial: bool) -> Result<PathBuf, String> {
    let root = temp_root(label)?;
    write_source(&root, "pub fn fixture() {}\n")?;
    let init = run(&root, &["init"])?;
    require(init.status.success(), format!("init {label}: {init:?}"))?;
    git_commit_fixture(&root)?;
    if partial {
        write_source(&root, "pub fn broken( {\n")?;
    }
    fs::create_dir_all(root.join("target/probe")).map_err(|error| error.to_string())?;
    Ok(root)
}

/// A sidecar must describe this invocation without changing its detailed output,
/// exit, or repository bytes; preexisting bytes are never proof of emission.
fn summary_with_unchanged_output(
    root: &Path,
    args: &[&str],
    exit: i32,
) -> Result<(Value, Value), String> {
    let source_before = fs::read(root.join("src/lib.rs")).map_err(|error| error.to_string())?;
    let policy_before =
        fs::read(root.join("policy/allow.toml")).map_err(|error| error.to_string())?;
    let baseline = run(root, args)?;
    let sidecar = root.join("target/probe/summary.json");
    fs::write(
        &sidecar,
        r#"{"result_class":"completed","sentinel":"stale"}"#,
    )
    .map_err(|error| format!("seed stale summary: {error}"))?;
    let sidecar_text = sidecar.to_string_lossy().to_string();
    let mut summary_args = vec!["--command-summary-output", &sidecar_text];
    summary_args.extend_from_slice(args);
    let output = run(root, &summary_args)?;
    require(
        baseline.status.code() == Some(exit)
            && output.status.code() == Some(exit)
            && baseline.stdout == output.stdout
            && baseline.stderr == output.stderr,
        format!(
            "the sidecar changed {args:?} detail or exit: baseline={baseline:?}, sidecar={output:?}"
        ),
    )?;
    require(
        fs::read(root.join("src/lib.rs")).map_err(|error| error.to_string())? == source_before
            && fs::read(root.join("policy/allow.toml")).map_err(|error| error.to_string())?
                == policy_before,
        "summary probes must preserve source and policy bytes",
    )?;
    let detail = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("parse {args:?} detail: {error}"))?;
    let summary = read_summary_json(&sidecar)?;
    require(
        field(&summary, &["sentinel"]).is_none(),
        "stale summary bytes survived this invocation",
    )?;
    Ok((detail, summary))
}

fn read_summary_json(path: &Path) -> Result<Value, String> {
    let bytes = fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("parse {}: {error}", path.display()))
}

/// Run the real binary from an explicit working directory, without the
/// implicit `--root` the [`run`] helper appends.
fn run_from(cwd: &Path, args: &[&str]) -> Result<Output, String> {
    fixture_command(env!("CARGO_BIN_EXE_cargo-allow"))
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|error| format!("run {args:?} from {}: {error}", cwd.display()))
}

/// Collect the lines of a human screen that claim the `Result:` label.
fn result_lines(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| line.starts_with("Result:"))
        .collect()
}

/// Minimal bespoke xtask/ripr ledger for the `migrate` probe (#4393).
const LEGACY_BESPOKE_LEDGER_FIXTURE: &str = r#"
schema_version = 1
dialect = "xtask-ripr"

[[entries]]
id = "fixture-semantic-unwrap"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "parser"
reason = "Semantic selector pins unwrap on optional after validation."
selector = "method_call"
container = "value"
callee = "unwrap"
receiver = "v"
"#;

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
    fixture_command(env!("CARGO_BIN_EXE_cargo-allow"))
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
        let output = fixture_command("git")
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

/// Fixture setup and CLI children must not inherit another repository's index
/// or policy selection. Keep these overrides local to each child process.
fn fixture_command(program: &str) -> Command {
    let mut command = Command::new(program);
    for key in [
        "CARGO_ALLOW_ROOT",
        "CARGO_ALLOW_CONFIG",
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_COMMON_DIR",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    ] {
        command.env_remove(key);
    }
    command
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
