use allow_core::{CargoAllowError, CargoAllowErrorKind};
use effortless_repo_protocol::{ClaimBoundaryV1, CompletenessV1, CurrentnessV1, ResultClassV1};

use super::*;

fn ensure(condition: bool, message: impl Into<String>) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn base_input(operation: &str) -> CoreCommandSummaryInputV1 {
    CoreCommandSummaryInputV1 {
        tool_version: "0.2.0".to_string(),
        operation: operation.to_string(),
        mode: None,
        profile: None,
        subject: CoreSourceSubjectV1::worktree("repo:test", "repo:test:worktree"),
        result_class: ResultClassV1::Completed,
        posture: CoreCommandPostureV1::Satisfied,
        completeness: CompletenessV1::Complete,
        currentness: CurrentnessV1::Current,
        reason: CoreCommandReasonV1 {
            code: "test.complete".to_string(),
            message: "operation completed".to_string(),
        },
        primary_action: None,
        additional_action_count: 0,
        additional_actions_ref: None,
        operation_effects: CoreCommandEffectsV1::read_only(vec![
            "does not execute repository code".to_string(),
        ]),
        next_proof: None,
        artifacts: Vec::new(),
        claim_boundary: ClaimBoundaryV1::new("source syntax only"),
    }
}

fn adoption_plan() -> allow_report::CoreAdoptionPlanV1 {
    allow_report::CoreAdoptionPlanV1 {
        schema_id: allow_report::CORE_ADOPTION_PLAN_SCHEMA_ID.to_string(),
        schema_version: allow_report::CORE_ADOPTION_PLAN_SCHEMA_VERSION,
        tool_version: "0.2.0".to_string(),
        repository_identity: "repo:test".to_string(),
        selected_root: "<repository-root>".to_string(),
        channel: "candidate".to_string(),
        executable_identity: "sha256:test".to_string(),
        inventory: allow_report::AdoptionInventoryFacts {
            mode: allow_report::InventoryMode::GitTracked,
            completeness: allow_report::InventoryCompleteness::Complete,
            limitations: Vec::new(),
        },
        policy: allow_report::AdoptionPolicyFacts {
            state: allow_report::PolicyState::Absent,
            path: None,
            schema_version: None,
            digest: None,
            total_findings: 2,
            new_unreceipted_findings: 0,
            stale_entries: 0,
            location_drift_entries: 0,
            broken_evidence_entries: 0,
            review_due_entries: 0,
            expired_entries: 0,
            occurrence_headroom_entries: 0,
            mirror_divergence: false,
        },
        bootstrap_disposition: allow_report::BootstrapDisposition::FindingsNoPolicy,
        primary_action: allow_report::AdoptionAction {
            kind: allow_report::AdoptionActionKind::PreviewPropose,
            argv: strings(&["cargo-allow", "propose"]),
            reason: "preview before retaining debt".to_string(),
            write_posture: allow_report::WritePosture::PreviewOnly,
            expected_result: "candidate entries are reviewable".to_string(),
        },
        follow_up_actions: vec![allow_report::AdoptionAction {
            kind: allow_report::AdoptionActionKind::RunNoNewCheck,
            argv: strings(&["cargo-allow", "check", "--mode", "no-new"]),
            reason: "verify the selected policy".to_string(),
            write_posture: allow_report::WritePosture::ReadOnly,
            expected_result: "the full source-tree posture is evaluated".to_string(),
        }],
        may_write_paths: Vec::new(),
        explicit_non_effects: vec!["does not write policy".to_string()],
        expected_result_markers: vec!["preview".to_string()],
        ci_example_path: "docs/how-to/adopt-cargo-allow.md".to_string(),
        rollback_guide_path: "docs/how-to/rollback-cargo-allow-adoption.md".to_string(),
        limitations: vec!["source syntax only".to_string()],
        claim_boundary: "adoption recommendation only".to_string(),
    }
}

#[test]
fn completed_summary_rejects_partial_coverage() -> Result<(), String> {
    let mut input = base_input("check");
    input.completeness = CompletenessV1::Partial;
    match build_core_command_summary(input) {
        Ok(_) => Err("partial coverage must not build as completed".to_string()),
        Err(error) => ensure(
            error.contains("complete coverage"),
            format!("unexpected validation error: {error}"),
        ),
    }
}

#[test]
fn summary_survives_a_json_round_trip() -> Result<(), String> {
    let summary = build_core_command_summary(base_input("check"))?;
    let json = render_core_command_summary_json(&summary)?;
    let parsed: CoreCommandSummaryV1 =
        serde_json::from_str(&json).map_err(|error| error.to_string())?;
    ensure(parsed == summary, "round trip changed the summary")?;
    validate_core_command_summary(&parsed)
}

#[test]
fn check_summary_keeps_human_and_json_result_semantics_equal() -> Result<(), String> {
    let mut input = base_input("check");
    input.mode = Some("no-new".to_string());
    input.result_class = ResultClassV1::Findings;
    input.posture = CoreCommandPostureV1::Blocking;
    input.reason = CoreCommandReasonV1 {
        code: "check.new_unreceipted_findings".to_string(),
        message: "one new unreceipted finding".to_string(),
    };
    input.primary_action = Some(
        CoreCommandActionV1::command(
            "check.inspect_finding",
            "Inspect the new finding",
            "cargo-allow",
            strings(&[
                "why",
                "--kind",
                "panic",
                "--path",
                "src/lib.rs",
                "--line",
                "42",
            ]),
        )
        .with_contract(
            "the blocking finding needs an exact explanation",
            "a bounded finding explanation is produced",
            "the explanation is not a policy mutation",
        ),
    );
    let summary = build_core_command_summary(input)?;
    let human = render_core_command_summary_human(&summary);
    let json = render_core_command_summary_json(&summary)?;
    let value: serde_json::Value =
        serde_json::from_str(&json).map_err(|error| error.to_string())?;
    ensure(
        human.contains("Result: findings (blocking)"),
        format!("human result missing: {human}"),
    )?;
    ensure(
        value
            .pointer("/result_class")
            .and_then(serde_json::Value::as_str)
            == Some("findings"),
        format!("JSON result mismatch: {json}"),
    )?;
    ensure(
        value
            .pointer("/primary_action/args/0")
            .and_then(serde_json::Value::as_str)
            == Some("why"),
        format!("structured argv missing: {json}"),
    )?;
    ensure(
        value.pointer("/primary_action/display").is_none(),
        format!("host-dependent display leaked into JSON: {json}"),
    )
}

#[test]
fn why_summary_routes_repository_judgment_without_false_preference() -> Result<(), String> {
    let mut input = base_input("why");
    input.result_class = ResultClassV1::Findings;
    input.posture = CoreCommandPostureV1::DecisionRequired;
    input.reason = CoreCommandReasonV1 {
        code: "why.ambiguous".to_string(),
        message: "multiple allow entries compete".to_string(),
    };
    input.additional_action_count = 2;
    input.additional_actions_ref = Some("cargo-allow.why.v1.proof_plans".to_string());
    let summary = build_core_command_summary(input)?;
    let human = render_core_command_summary_human(&summary);
    ensure(
        summary.primary_action.is_none(),
        "ambiguous why must not select one candidate",
    )?;
    ensure(
        human.contains("Next: repository decision required"),
        format!("decision route missing: {human}"),
    )
}

#[test]
fn mutation_summary_names_live_target_and_full_check() -> Result<(), String> {
    let mut input = base_input("add");
    input.operation_effects = CoreCommandEffectsV1 {
        reads_repository: true,
        writes_repository: true,
        executes_repository_code: false,
        invokes_network: false,
        write_paths: vec!["policy/allow.toml".to_string()],
        explicit_non_effects: vec!["does not approve the exception".to_string()],
    };
    input.reason = CoreCommandReasonV1 {
        code: "add.applied".to_string(),
        message: "one reviewed allow entry was written".to_string(),
    };
    input.next_proof = Some(
        CoreCommandActionV1::command(
            "add.full_check",
            "Run the full no-new check",
            "cargo-allow",
            strings(&["check", "--mode", "no-new"]),
        )
        .with_contract(
            "targeted confirmation is not full repository proof",
            "the complete current repository posture is evaluated",
            "source-syntax evaluation does not prove compiled or runtime correctness",
        ),
    );
    let summary = build_core_command_summary(input)?;
    let human = render_core_command_summary_human(&summary);
    ensure(
        human.contains("Writes: policy/allow.toml"),
        format!("write target missing: {human}"),
    )?;
    ensure(
        human.contains("Then: cargo-allow check --mode no-new"),
        format!("full-check route missing: {human}"),
    )
}

#[test]
fn adoption_adapter_reuses_typed_primary_action() -> Result<(), String> {
    let summary = core_command_summary_from_adoption_plan(&adoption_plan())?;
    ensure(
        summary.result_class == ResultClassV1::Findings,
        "findings/no-policy must remain a findings result",
    )?;
    ensure(
        summary
            .primary_action
            .as_ref()
            .and_then(|action| action.program.as_deref())
            == Some("cargo-allow"),
        "adoption primary action must keep structured program identity",
    )?;
    ensure(
        summary.next_proof.as_ref().is_some_and(|action| {
            action
                .args
                .iter()
                .map(String::as_str)
                .eq(["check", "--mode", "no-new"])
        }),
        "adoption follow-up should expose the full check",
    )?;
    ensure(
        summary.additional_action_count == 0 && summary.additional_actions_ref.is_none(),
        "promoted next proof must not be counted again as an additional action",
    )
}

#[test]
fn adoption_adapter_counts_only_unpromoted_follow_ups() -> Result<(), String> {
    let mut plan = adoption_plan();
    plan.follow_up_actions.push(allow_report::AdoptionAction {
        kind: allow_report::AdoptionActionKind::ConfigureCi,
        argv: strings(&["cargo-allow", "reference", "--format", "markdown"]),
        reason: "inspect the supported CI reference".to_string(),
        write_posture: allow_report::WritePosture::ReadOnly,
        expected_result: "the checked CI contract is available".to_string(),
    });
    let summary = core_command_summary_from_adoption_plan(&plan)?;
    ensure(
        summary.additional_action_count == 1,
        format!(
            "expected one unpromoted follow-up, got {}",
            summary.additional_action_count
        ),
    )?;
    ensure(
        summary
            .additional_actions_ref
            .as_deref()
            .is_some_and(|reference| reference.contains("exclude_index=0")),
        "additional action reference must exclude the promoted next proof",
    )
}

#[test]
fn adoption_adapter_rejects_unknown_live_mutation_target() -> Result<(), String> {
    let mut plan = adoption_plan();
    plan.primary_action = allow_report::AdoptionAction {
        kind: allow_report::AdoptionActionKind::ApplyStaleSafeFindingPlan,
        argv: strings(&["cargo-allow", "add", "--from-plan", "plan.json", "--update"]),
        reason: "apply a reviewed finding plan".to_string(),
        write_posture: allow_report::WritePosture::MayWrite,
        expected_result: "the selected ledger entry is updated".to_string(),
    };
    plan.may_write_paths.clear();
    match core_command_summary_from_adoption_plan(&plan) {
        Ok(_) => Err("MayWrite without a target must fail".to_string()),
        Err(error) => ensure(
            error.contains("policy.path-derived may_write_paths"),
            format!("unexpected missing-target error: {error}"),
        ),
    }
}

#[test]
fn error_adapter_maps_typed_kind_without_parsing_message_text() -> Result<(), String> {
    let error = CargoAllowError::with_kind(
        CargoAllowErrorKind::InvalidPolicy,
        "policy contains an unknown field",
    );
    let summary = core_command_summary_from_error(
        "0.2.0",
        "check",
        CoreSourceSubjectV1::worktree("repo:test", "repo:test:worktree"),
        &error,
        CoreCommandEffectsV1::read_only(Vec::new()),
        None,
        ClaimBoundaryV1::new("source syntax only"),
    )?;
    ensure(
        summary.result_class == ResultClassV1::MalformedInput,
        "invalid policy should map to malformed input",
    )?;
    ensure(
        summary.reason.code == "E0003_INVALID_POLICY",
        format!("typed code was not preserved: {}", summary.reason.code),
    )
}

#[test]
fn human_renderer_sanitizes_repository_control_text() -> Result<(), String> {
    let mut input = base_input("check");
    input.subject.portable_identity = "repo:test\nforged".to_string();
    input.reason.message = "bad\u{1b}[31m reason".to_string();
    let summary = build_core_command_summary(input)?;
    let human = render_core_command_summary_human(&summary);
    ensure(
        !human.contains("repo:test\nforged"),
        format!("subject injected a new line: {human}"),
    )?;
    ensure(
        !human.contains('\u{1b}'),
        format!("terminal escape survived rendering: {human}"),
    )
}

#[test]
fn command_display_falls_back_for_control_text() -> Result<(), String> {
    let mut input = base_input("why");
    input.result_class = ResultClassV1::Findings;
    input.posture = CoreCommandPostureV1::Advisory;
    input.primary_action = Some(
        CoreCommandActionV1::command(
            "why.inspect",
            "Inspect finding",
            "cargo-allow",
            vec!["why".to_string(), "src/\u{1b}[31m.rs".to_string()],
        )
        .with_contract(
            "inspect the finding",
            "a bounded explanation is produced",
            "the command remains read-only",
        ),
    );
    let summary = build_core_command_summary(input)?;
    let human = render_core_command_summary_human(&summary);
    ensure(
        human.contains("use structured argv; command contains non-pasteable control text"),
        format!("structured-argv fallback missing: {human}"),
    )?;
    ensure(
        !human.contains('\u{1b}'),
        format!("control character survived command rendering: {human}"),
    )
}
