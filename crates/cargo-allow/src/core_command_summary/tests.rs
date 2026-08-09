use allow_core::{CargoAllowError, CargoAllowErrorKind, MatchStatus};
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
fn core_command_summary_init_distinguishes_preview_and_live_write() -> Result<(), String> {
    let preview = core_command_summary_from_init(InitSummaryFactsV1 {
        repository_identity: "local-repository:test".to_string(),
        portable_identity: "worktree:init:policy/allow.toml".to_string(),
        config_path: "policy/allow.toml".to_string(),
        dry_run: true,
        force: true,
        path_existed: true,
    })?;
    ensure(
        preview.posture == CoreCommandPostureV1::Advisory
            && !preview.operation_effects.writes_repository
            && preview.primary_action.as_ref().is_some_and(|action| {
                action.write_posture == CoreCommandWritePostureV1::LiveMutation
                    && action.may_write_paths == vec!["policy/allow.toml"]
                    && action.args == vec!["init", "--config", "policy/allow.toml", "--force"]
            }),
        "init preview must be advisory and keep the possible write explicit",
    )?;

    let applied = core_command_summary_from_init(InitSummaryFactsV1 {
        repository_identity: "local-repository:test".to_string(),
        portable_identity: "worktree:init:policy/allow.toml".to_string(),
        config_path: "policy/allow.toml".to_string(),
        dry_run: false,
        force: false,
        path_existed: false,
    })?;
    ensure(
        applied.operation_effects.writes_repository
            && applied.operation_effects.write_paths == vec!["policy/allow.toml"]
            && applied
                .primary_action
                .as_ref()
                .is_some_and(|action| action.args == vec!["check", "--mode", "no-new"]),
        "init apply must name the written path and enforcing follow-up",
    )
}

#[test]
fn core_command_summary_propose_distinguishes_candidate_output_and_write() -> Result<(), String> {
    let stdout_candidate = core_command_summary_from_propose(ProposeSummaryFactsV1 {
        repository_identity: "local-repository:test".to_string(),
        portable_identity: "worktree:propose:stdout".to_string(),
        write_path: None,
        force: false,
        completeness: CompletenessV1::Complete,
        proposed_entries: 2,
        unsafe_proposed_entries: 1,
        truncated_new_findings: 0,
        unreceiptable_new_findings: 0,
    })?;
    ensure(
        stdout_candidate.posture == CoreCommandPostureV1::Advisory
            && !stdout_candidate.operation_effects.writes_repository
            && stdout_candidate
                .primary_action
                .as_ref()
                .is_some_and(|action| {
                    action.write_posture == CoreCommandWritePostureV1::ReadOnly
                        && action.title.contains("Review")
                })
            && stdout_candidate.next_proof.is_none(),
        "stdout proposal must remain candidate-only and require review",
    )?;

    let written_candidate = core_command_summary_from_propose(ProposeSummaryFactsV1 {
        repository_identity: "local-repository:test".to_string(),
        portable_identity: "worktree:propose:policy/allow.proposed.toml".to_string(),
        write_path: Some("policy/allow.proposed.toml".to_string()),
        force: true,
        completeness: CompletenessV1::Complete,
        proposed_entries: 1,
        unsafe_proposed_entries: 0,
        truncated_new_findings: 1,
        unreceiptable_new_findings: 0,
    })?;
    ensure(
        written_candidate.operation_effects.writes_repository
            && written_candidate.operation_effects.write_paths
                == vec!["policy/allow.proposed.toml"]
            && written_candidate.next_proof.as_ref().is_some_and(|action| {
                action.args
                    == vec![
                        "check",
                        "--mode",
                        "no-new",
                        "--config",
                        "policy/allow.proposed.toml",
                    ]
            }),
        "written proposal must name its target and targeted proof",
    )
}

#[test]
fn core_command_summary_add_distinguishes_preview_candidate_and_live_entry() -> Result<(), String> {
    let candidate = core_command_summary_from_add(AddSummaryFactsV1 {
        repository_identity: "local-repository:test".to_string(),
        portable_identity: "worktree:add:policy/allow.proposed.toml".to_string(),
        write_path: Some("policy/allow.proposed.toml".to_string()),
        live_update: false,
        candidate_write: true,
        dry_run: false,
        completeness: CompletenessV1::Complete,
        entry_id: "allow-0001".to_string(),
        kind: "panic".to_string(),
        scope: "src/lib.rs:1".to_string(),
    })?;
    ensure(
        candidate.posture == CoreCommandPostureV1::Advisory
            && candidate.operation_effects.writes_repository
            && candidate.operation_effects.write_paths == vec!["policy/allow.proposed.toml"]
            && candidate.next_proof.is_some(),
        "candidate add must remain advisory and name its written policy target",
    )?;

    let live = core_command_summary_from_add(AddSummaryFactsV1 {
        repository_identity: "local-repository:test".to_string(),
        portable_identity: "worktree:add:policy/allow.toml".to_string(),
        write_path: Some("policy/allow.toml".to_string()),
        live_update: true,
        candidate_write: false,
        dry_run: false,
        completeness: CompletenessV1::Complete,
        entry_id: "allow-0001".to_string(),
        kind: "panic".to_string(),
        scope: "src/lib.rs:1".to_string(),
    })?;
    ensure(
        live.posture == CoreCommandPostureV1::Satisfied
            && live.operation_effects.writes_repository
            && live
                .primary_action
                .as_ref()
                .is_some_and(|action| action.args == vec!["explain", "allow-0001"])
            && live
                .next_proof
                .as_ref()
                .is_some_and(|action| action.args == vec!["check", "--mode", "no-new"]),
        "live add must separate targeted entry inspection from full proof",
    )
}

#[test]
fn core_command_summary_add_from_plan_separates_targeted_and_full_proof() -> Result<(), String> {
    let summary = core_command_summary_from_add_plan(AddPlanSummaryFactsV1 {
        repository_identity: "sha256:v1:repo".to_string(),
        portable_identity: "worktree:add-from-plan:policy/allow.toml".to_string(),
        policy_path: "policy/allow.toml".to_string(),
        added_allow_id: "allow-0001".to_string(),
        targeted_recheck: "matched".to_string(),
        full_check_argv: strings(&[
            "check",
            "--mode",
            "no-new",
            "--root",
            "<root>",
            "--config",
            "policy/allow.toml",
        ]),
        completeness: CompletenessV1::Complete,
    })?;
    ensure(
        summary.operation == "add_from_plan"
            && summary.posture == CoreCommandPostureV1::Satisfied
            && summary.operation_effects.writes_repository
            && summary.operation_effects.write_paths == vec!["policy/allow.toml"]
            && summary.next_proof.as_ref().is_some_and(|action| {
                action.args
                    == strings(&[
                        "check",
                        "--mode",
                        "no-new",
                        "--root",
                        "<root>",
                        "--config",
                        "policy/allow.toml",
                    ])
            }),
        "add-from-plan must retain the exact full-check argv after targeted confirmation",
    )
}

#[test]
fn core_command_summary_refresh_separates_preview_and_live_write() -> Result<(), String> {
    let preview = core_command_summary_from_refresh(RefreshSummaryFactsV1 {
        repository_identity: "local-repository:test".to_string(),
        portable_identity: "worktree:refresh:policy/allow.toml:allow-0001".to_string(),
        policy_path: "policy/allow.toml".to_string(),
        allow_id: "allow-0001".to_string(),
        write_requested: false,
        dry_run: true,
        completeness: CompletenessV1::Complete,
    })?;
    ensure(
        preview.posture == CoreCommandPostureV1::Advisory
            && !preview.operation_effects.writes_repository
            && preview.primary_action.as_ref().is_some_and(|action| {
                action.args
                    == strings(&[
                        "refresh",
                        "--allow-id",
                        "allow-0001",
                        "--config",
                        "policy/allow.toml",
                        "--write",
                    ])
            }),
        "refresh preview must remain advisory and name the live write target",
    )?;
    let live = core_command_summary_from_refresh(RefreshSummaryFactsV1 {
        repository_identity: "local-repository:test".to_string(),
        portable_identity: "worktree:refresh:policy/allow.toml:allow-0001".to_string(),
        policy_path: "policy/allow.toml".to_string(),
        allow_id: "allow-0001".to_string(),
        write_requested: true,
        dry_run: false,
        completeness: CompletenessV1::Complete,
    })?;
    ensure(
        live.posture == CoreCommandPostureV1::Satisfied
            && live.operation_effects.writes_repository
            && live.operation_effects.write_paths == vec!["policy/allow.toml"]
            && live
                .next_proof
                .as_ref()
                .is_some_and(|action| action.args == strings(&["check", "--mode", "no-new"])),
        "live refresh must name its policy write and full proof",
    )
}

#[test]
fn core_command_summary_diff_preserves_revision_and_completeness_posture() -> Result<(), String> {
    let complete = core_command_summary_from_diff(DiffSummaryFactsV1 {
        repository_identity: "local-repository:test".to_string(),
        portable_identity: "diff:base:head".to_string(),
        base: "base".to_string(),
        head: Some("head".to_string()),
        result_class: ResultClassV1::Completed,
        completeness: CompletenessV1::Complete,
        currentness: CurrentnessV1::Current,
        current_failures: 0,
        failed: false,
    })?;
    ensure(
        complete.subject.base.as_deref() == Some("base")
            && complete.subject.head.as_deref() == Some("head"),
        "diff summary must retain the exact revision pair",
    )?;
    ensure(
        complete.next_proof.is_some(),
        "a complete diff must name the enforcing follow-up proof",
    )?;

    let partial = core_command_summary_from_diff(DiffSummaryFactsV1 {
        repository_identity: "local-repository:test".to_string(),
        portable_identity: "diff:base:current-worktree".to_string(),
        base: "base".to_string(),
        head: None,
        result_class: ResultClassV1::PartialData,
        completeness: CompletenessV1::Partial,
        currentness: CurrentnessV1::PartialOrUnavailable,
        current_failures: 1,
        failed: true,
    })?;
    ensure(
        partial.posture == CoreCommandPostureV1::Blocking
            && partial.primary_action.is_some()
            && partial.next_proof.is_none(),
        "partial diff must block and avoid a clean-proof claim",
    )
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

fn doctor_facts() -> DoctorSummaryFactsV1 {
    DoctorSummaryFactsV1 {
        tool_version: "0.2.0".to_string(),
        subject: CoreSourceSubjectV1::worktree(
            "local-repository:sha256:v1:test",
            "worktree:git_tracked:current-unpinned",
        ),
        completeness: CompletenessV1::Complete,
        coverage_limitation: None,
        config_present: true,
        config_valid: Some(true),
        config_diagnostic: None,
        broken_evidence_links: Some(0),
        weak_evidence_references: Some(0),
        claim_boundary: ClaimBoundaryV1::new("setup health only"),
    }
}

#[test]
fn adoption_adapter_describes_its_subject_in_the_shared_grammar() -> Result<(), String> {
    let summary = core_command_summary_from_adoption_plan(&adoption_plan())?;
    // `audit`, `check`, and `doctor` all name the evaluated subject as
    // `worktree:<inventory mode>:current-unpinned`. Adoption must not invent a
    // second spelling for the same concept (#3149).
    ensure(
        summary.subject.portable_identity == "worktree:git_tracked:current-unpinned",
        format!(
            "adoption subject must use the shared grammar, got {}",
            summary.subject.portable_identity
        ),
    )?;
    ensure(
        summary.subject.repository_identity == "repo:test",
        "content-addressed repository identity must be preserved",
    )?;
    ensure(
        summary
            .subject
            .limitations
            .iter()
            .any(|limitation| limitation.contains("not bound to a commit")),
        "an unpinned worktree subject must say so",
    )
}

#[test]
fn doctor_adapter_reports_healthy_setup_without_claiming_the_gate() -> Result<(), String> {
    let summary = core_command_summary_from_doctor(doctor_facts())?;
    ensure(
        summary.result_class == ResultClassV1::Completed
            && summary.posture == CoreCommandPostureV1::Satisfied,
        "a healthy, complete, fully probed setup is a satisfied result",
    )?;
    ensure(
        summary.primary_action.is_none(),
        "a healthy setup must not invent a repair action",
    )?;
    ensure(
        summary.next_proof.as_ref().is_some_and(|action| {
            action
                .args
                .iter()
                .map(String::as_str)
                .eq(["check", "--mode", "no-new"])
        }),
        "doctor must route to the enforcing gate rather than imply it already passed",
    )?;
    ensure(
        !summary.operation_effects.writes_repository,
        "doctor is read-only",
    )
}

#[test]
fn doctor_adapter_never_reads_unprobed_evidence_as_zero_defects() -> Result<(), String> {
    let mut facts = doctor_facts();
    facts.broken_evidence_links = None;
    facts.weak_evidence_references = None;
    let summary = core_command_summary_from_doctor(facts)?;
    ensure(
        summary.result_class == ResultClassV1::NotProven,
        "unprobed evidence health is not proof of health",
    )?;
    ensure(
        summary.posture != CoreCommandPostureV1::Satisfied,
        "an unprobed diagnosis must not render as satisfied",
    )?;
    ensure(
        summary.next_proof.is_none(),
        "an inconclusive diagnosis must not promise the gate proves anything",
    )
}

#[test]
fn doctor_adapter_defers_invalid_policy_to_repository_judgment() -> Result<(), String> {
    let mut facts = doctor_facts();
    facts.config_valid = Some(false);
    facts.config_diagnostic = Some("policy/allow.toml: expected table".to_string());
    let summary = core_command_summary_from_doctor(facts)?;
    ensure(
        summary.result_class == ResultClassV1::MalformedInput
            && summary.posture == CoreCommandPostureV1::DecisionRequired,
        "an invalid policy is malformed input requiring repository judgment",
    )?;
    ensure(
        summary
            .primary_action
            .as_ref()
            .is_some_and(|action| action.kind == CoreCommandActionKindV1::Decision),
        "cargo-allow must not guess a command that repairs a malformed policy",
    )?;
    ensure(
        summary.reason.message.contains("expected table"),
        "the typed diagnostic must survive into the summary reason",
    )?;
    ensure(
        summary.next_proof.is_none(),
        "a malformed policy cannot promise a meaningful gate run",
    )
}

#[test]
fn doctor_adapter_keeps_partial_coverage_non_green() -> Result<(), String> {
    let mut facts = doctor_facts();
    facts.completeness = CompletenessV1::Partial;
    facts.coverage_limitation = Some("3 path(s) were skipped".to_string());
    let summary = core_command_summary_from_doctor(facts)?;
    ensure(
        summary.result_class == ResultClassV1::PartialData
            && summary.posture == CoreCommandPostureV1::Blocking,
        "partial coverage must not be green",
    )?;
    ensure(
        summary.next_proof.is_none(),
        "partial coverage must not imply the gate would prove anything",
    )?;
    ensure(
        summary
            .subject
            .limitations
            .iter()
            .any(|limitation| limitation.contains("3 path(s) were skipped")),
        "the exact coverage limitation must reach the operator",
    )
}

#[test]
fn doctor_adapter_routes_an_absent_policy_to_adoption() -> Result<(), String> {
    let mut facts = doctor_facts();
    facts.config_present = false;
    facts.config_valid = None;
    facts.broken_evidence_links = None;
    facts.weak_evidence_references = None;
    let summary = core_command_summary_from_doctor(facts)?;
    ensure(
        summary.result_class == ResultClassV1::Findings
            && summary.posture == CoreCommandPostureV1::Advisory,
        "an unadopted repository is an advisory finding, not a failure",
    )?;
    ensure(
        summary.primary_action.as_ref().is_some_and(|action| {
            action.args.iter().map(String::as_str).eq(["adopt"])
                && action.may_write_paths.is_empty()
        }),
        "the first-hour route from doctor is read-only adopt",
    )
}

#[test]
fn doctor_adapter_separates_broken_from_weak_evidence_posture() -> Result<(), String> {
    let mut blocking = doctor_facts();
    blocking.broken_evidence_links = Some(2);
    let blocking = core_command_summary_from_doctor(blocking)?;
    ensure(
        blocking.posture == CoreCommandPostureV1::Blocking,
        "unresolved evidence references block",
    )?;
    ensure(
        blocking
            .primary_action
            .as_ref()
            .is_some_and(|action| action.args.contains(&"--broken-evidence".to_string())),
        "broken evidence must route to the broken-evidence worklist",
    )?;

    let mut advisory = doctor_facts();
    advisory.weak_evidence_references = Some(5);
    let advisory = core_command_summary_from_doctor(advisory)?;
    ensure(
        advisory.posture == CoreCommandPostureV1::Advisory,
        "weak evidence is advisory rather than blocking",
    )?;
    ensure(
        advisory
            .primary_action
            .as_ref()
            .is_some_and(|action| action.args.contains(&"--weak-evidence".to_string())),
        "weak evidence must route to the weak-evidence worklist",
    )
}

fn explain_facts() -> ExplainSummaryFactsV1 {
    ExplainSummaryFactsV1 {
        tool_version: "0.2.0".to_string(),
        subject: CoreSourceSubjectV1 {
            kind: CoreSourceSubjectKindV1::ScopedPath,
            repository_identity: "local-repository:sha256:v1:test".to_string(),
            portable_identity: "scoped:allow-entry:allow-0001:git_tracked:current-unpinned"
                .to_string(),
            base: None,
            head: None,
            paths: strings(&["src/lib.rs"]),
            limitations: Vec::new(),
        },
        completeness: CompletenessV1::Complete,
        coverage_limitation: None,
        allow_id: "allow-0001".to_string(),
        attention_status: None,
        matching_finding_count: 2,
        suggested_actions: Vec::new(),
        claim_boundary: ClaimBoundaryV1::new("one ledger entry only"),
    }
}

fn why_facts() -> WhySummaryFactsV1 {
    WhySummaryFactsV1 {
        tool_version: "0.2.0".to_string(),
        subject: CoreSourceSubjectV1 {
            kind: CoreSourceSubjectKindV1::ScopedPath,
            repository_identity: "local-repository:sha256:v1:test".to_string(),
            portable_identity: "scoped:finding:panic:src/lib.rs:42:git_tracked:current-unpinned"
                .to_string(),
            base: None,
            head: None,
            paths: strings(&["src/lib.rs:42"]),
            limitations: Vec::new(),
        },
        completeness: CompletenessV1::Complete,
        coverage_limitation: None,
        location: "src/lib.rs:42".to_string(),
        outcome_status: MatchStatus::Matched,
        matched_allow_id: Some("allow-0001".to_string()),
        near_miss_candidate_count: 0,
        suggested_actions: Vec::new(),
        plan_path: None,
        claim_boundary: ClaimBoundaryV1::new("one finding only"),
    }
}

fn worklist_facts() -> WorklistSummaryFactsV1 {
    WorklistSummaryFactsV1 {
        tool_version: "0.2.0".to_string(),
        subject: CoreSourceSubjectV1::worktree(
            "local-repository:sha256:v1:test",
            "worktree:git_tracked:current-unpinned",
        ),
        completeness: CompletenessV1::Complete,
        coverage_limitation: None,
        items: Vec::new(),
        filtered: false,
        claim_boundary: ClaimBoundaryV1::new("queued maintenance work only"),
    }
}

fn work_item(kind: &str, status: MatchStatus, actions: &[&str]) -> WorklistSummaryItemV1 {
    WorklistSummaryItemV1 {
        kind: kind.to_string(),
        status,
        allow_id: Some("allow-0001".to_string()),
        path: Some("src/lib.rs".to_string()),
        suggested_actions: strings(actions),
    }
}

#[test]
fn explain_adapter_reports_a_receipted_entry_as_satisfied() -> Result<(), String> {
    let summary = core_command_summary_from_explain(explain_facts())?;
    ensure(
        summary.result_class == ResultClassV1::Completed
            && summary.posture == CoreCommandPostureV1::Satisfied,
        "an entry whose findings are all matched, with no maintenance left, is satisfied",
    )?;
    ensure(
        summary.primary_action.is_none() && summary.additional_action_count == 0,
        "a healthy entry must not carry an invented action",
    )?;
    ensure(
        !summary.operation_effects.writes_repository
            && summary.operation_effects.write_paths.is_empty(),
        "explain is read-only",
    )?;
    ensure(
        summary.next_proof.as_ref().is_some_and(|action| {
            action
                .args
                .iter()
                .map(String::as_str)
                .eq(["check", "--mode", "no-new"])
        }),
        "one healthy entry must route to the gate rather than imply it already passed",
    )
}

#[test]
fn explain_adapter_keeps_a_gate_failing_entry_blocking() -> Result<(), String> {
    let mut facts = explain_facts();
    facts.attention_status = Some(MatchStatus::EvidenceMissing);
    facts.suggested_actions = strings(&[
        "add evidence that supports the exception reason",
        "keep the selector scoped to the reviewed boundary",
    ]);
    let summary = core_command_summary_from_explain(facts)?;
    ensure(
        summary.result_class == ResultClassV1::Findings
            && summary.posture == CoreCommandPostureV1::Blocking,
        "an outcome the no-new gate fails on must block",
    )?;
    // The ranked step is reused verbatim; the summary neither rewrites it nor
    // promotes it into a command cargo-allow would run.
    ensure(
        summary.primary_action.as_ref().is_some_and(|action| {
            action.kind == CoreCommandActionKindV1::Decision
                && action.title == "add evidence that supports the exception reason"
                && action.program.is_none()
        }),
        "explain must reuse the typed ranked step as a repository decision",
    )?;
    ensure(
        summary.additional_action_count == 1
            && summary
                .additional_actions_ref
                .as_deref()
                .is_some_and(|reference| reference.contains("explain.v1")),
        "the unpromoted ranked steps must stay retrievable from the detailed artifact",
    )
}

#[test]
fn explain_adapter_defers_a_competing_entry_to_repository_judgment() -> Result<(), String> {
    let mut facts = explain_facts();
    facts.attention_status = Some(MatchStatus::Ambiguous);
    facts.suggested_actions =
        strings(&["narrow selectors so each finding matches exactly one allow entry"]);
    let summary = core_command_summary_from_explain(facts)?;
    ensure(
        summary.posture == CoreCommandPostureV1::DecisionRequired,
        "competing entries are a repository decision, not a cargo-allow preference",
    )?;
    ensure(
        summary
            .primary_action
            .as_ref()
            .is_some_and(|action| action.kind == CoreCommandActionKindV1::Decision),
        "an ambiguous entry must not resolve to a guessed command",
    )
}

#[test]
fn explain_adapter_never_reports_an_unmatched_entry_as_satisfied() -> Result<(), String> {
    // No current finding matches the entry any more. That is the `stale`
    // outcome, which the no-new gate tolerates but which is still not a clean
    // entry, so it must render as advisory rather than satisfied.
    let mut facts = explain_facts();
    facts.matching_finding_count = 0;
    facts.attention_status = Some(MatchStatus::Stale);
    facts.suggested_actions = strings(&["remove the stale allow entry if the exception is gone"]);
    let summary = core_command_summary_from_explain(facts)?;
    ensure(
        summary.result_class == ResultClassV1::Findings
            && summary.posture == CoreCommandPostureV1::Advisory,
        "a stale entry is an advisory finding, not a satisfied result",
    )?;
    ensure(
        summary.reason.code == "explain.stale",
        format!(
            "the typed status must survive into the reason: {}",
            summary.reason.code
        ),
    )
}

#[test]
fn explain_adapter_keeps_partial_coverage_non_green() -> Result<(), String> {
    let mut facts = explain_facts();
    facts.completeness = CompletenessV1::Partial;
    facts.coverage_limitation = Some("2 Rust file(s) were skipped".to_string());
    let summary = core_command_summary_from_explain(facts)?;
    ensure(
        summary.result_class == ResultClassV1::PartialData
            && summary.posture == CoreCommandPostureV1::Blocking,
        "partial coverage must not be green",
    )?;
    ensure(
        summary.next_proof.is_none(),
        "partial coverage must not imply the gate would prove anything",
    )?;
    ensure(
        summary
            .subject
            .limitations
            .iter()
            .any(|limitation| limitation.contains("2 Rust file(s) were skipped")),
        "the exact coverage limitation must reach the operator",
    )
}

#[test]
fn why_adapter_reports_a_receipted_finding_as_satisfied() -> Result<(), String> {
    let summary = core_command_summary_from_why(why_facts())?;
    ensure(
        summary.result_class == ResultClassV1::Completed
            && summary.posture == CoreCommandPostureV1::Satisfied,
        "a finding bound to an allow entry is satisfied",
    )?;
    ensure(
        summary.primary_action.is_none(),
        "a receipted finding must not carry an invented action",
    )?;
    ensure(
        summary.reason.message.contains("allow-0001"),
        format!(
            "the receipting entry must be named: {}",
            summary.reason.message
        ),
    )?;
    ensure(
        !summary.operation_effects.writes_repository,
        "why without --plan is read-only",
    )
}

#[test]
fn why_adapter_never_promotes_a_near_miss_candidate_to_a_winner() -> Result<(), String> {
    let mut facts = why_facts();
    facts.outcome_status = MatchStatus::New;
    facts.matched_allow_id = None;
    facts.near_miss_candidate_count = 3;
    facts.suggested_actions = strings(&[
        "Receipt this occurrence with cargo-allow add.",
        "Or repair the source so the finding disappears.",
    ]);
    let summary = core_command_summary_from_why(facts)?;
    ensure(
        summary.result_class == ResultClassV1::Findings
            && summary.posture == CoreCommandPostureV1::Blocking,
        "an unreceipted finding blocks the no-new gate",
    )?;
    ensure(
        summary.reason.message.contains("no allow entry matches"),
        format!(
            "the absence of a match must be explicit: {}",
            summary.reason.message
        ),
    )?;
    ensure(
        summary
            .primary_action
            .as_ref()
            .is_some_and(|action| action.kind == CoreCommandActionKindV1::Decision),
        "receipting versus repairing is a repository judgment",
    )?;
    ensure(
        !summary.reason.message.contains("allow-0001"),
        "a near-miss candidate must never be reported as the covering entry",
    )
}

#[test]
fn why_adapter_reports_an_unattributable_match_as_not_proven() -> Result<(), String> {
    let mut facts = why_facts();
    facts.matched_allow_id = None;
    let summary = core_command_summary_from_why(facts)?;
    ensure(
        summary.result_class == ResultClassV1::NotProven
            && summary.posture == CoreCommandPostureV1::Blocking,
        "a matched outcome with no recorded entry ID is not proof of coverage",
    )?;
    ensure(
        summary.next_proof.is_none(),
        "an unattributable result must not promise the gate proves anything",
    )
}

#[test]
fn why_plan_reports_its_write_even_when_the_finding_is_receipted() -> Result<(), String> {
    // A receipted finding leaves nothing to decide, so there is no primary
    // action — but `--plan` still wrote a file, and the summary must keep
    // disclosing that. Pins the write disclosure on the satisfied path so a
    // later refactor cannot drop it.
    let mut facts = why_facts();
    facts.plan_path = Some("target/cargo-allow/add-finding.plan.json".to_string());
    let summary = core_command_summary_from_why(facts)?;
    ensure(
        summary.result_class == ResultClassV1::Completed && summary.primary_action.is_none(),
        "a receipted finding leaves nothing for the operator to decide",
    )?;
    ensure(
        summary.operation_effects.writes_repository
            && summary
                .operation_effects
                .write_paths
                .iter()
                .map(String::as_str)
                .eq(["target/cargo-allow/add-finding.plan.json"]),
        "the plan write must be disclosed even on a satisfied result",
    )
}

#[test]
fn why_plan_names_its_candidate_write_target() -> Result<(), String> {
    let mut facts = why_facts();
    facts.outcome_status = MatchStatus::New;
    facts.matched_allow_id = None;
    facts.suggested_actions = strings(&[
        "Receipt this occurrence with cargo-allow add.",
        "Or repair the source so the finding disappears.",
    ]);
    facts.plan_path = Some("target/cargo-allow/add-finding.plan.json".to_string());
    let summary = core_command_summary_from_why(facts)?;
    ensure(
        summary.operation_effects.writes_repository,
        "--plan writes a candidate artifact, so the operation is not read-only",
    )?;
    ensure(
        summary
            .operation_effects
            .write_paths
            .iter()
            .map(String::as_str)
            .eq(["target/cargo-allow/add-finding.plan.json"]),
        format!(
            "the exact plan path must be named: {:?}",
            summary.operation_effects.write_paths
        ),
    )?;
    ensure(
        summary
            .operation_effects
            .explicit_non_effects
            .iter()
            .any(|effect| effect.contains("does not retain, approve, or authorize the exception")),
        "a candidate plan must not read as an approved exception",
    )?;
    ensure(
        summary.primary_action.as_ref().is_some_and(|action| {
            action.kind == CoreCommandActionKindV1::Decision
                && action
                    .title
                    .contains("target/cargo-allow/add-finding.plan.json")
        }),
        "the written plan must be routed to human review rather than auto-applied",
    )?;
    let human = render_core_command_summary_human(&summary);
    ensure(
        human.contains("Writes: target/cargo-allow/add-finding.plan.json"),
        format!("the human summary must name the write target: {human}"),
    )
}

#[test]
fn worklist_adapter_reports_an_empty_queue_as_satisfied() -> Result<(), String> {
    let summary = core_command_summary_from_worklist(worklist_facts())?;
    ensure(
        summary.result_class == ResultClassV1::Completed
            && summary.posture == CoreCommandPostureV1::Satisfied,
        "an empty unfiltered queue is a satisfied result",
    )?;
    ensure(
        summary.primary_action.is_none()
            && summary.additional_action_count == 0
            && summary.additional_actions_ref.is_none(),
        "an empty queue must not carry an invented action",
    )?;
    ensure(
        !summary.operation_effects.writes_repository,
        "worklist is read-only",
    )
}

#[test]
fn worklist_adapter_blocks_on_the_highest_severity_queued_item() -> Result<(), String> {
    let mut facts = worklist_facts();
    facts.items = vec![
        work_item(
            "review_due",
            MatchStatus::ReviewDue,
            &["review the retained exception and update evidence or remove it"],
        ),
        work_item(
            "new_unreceipted_finding",
            MatchStatus::New,
            &["remove the new source exception if it is accidental"],
        ),
    ];
    let summary = core_command_summary_from_worklist(facts)?;
    ensure(
        summary.result_class == ResultClassV1::Findings
            && summary.posture == CoreCommandPostureV1::Blocking,
        "a queue containing a gate-failing item blocks even when it is not ranked first",
    )?;
    ensure(
        summary.primary_action.as_ref().is_some_and(|action| {
            action.kind == CoreCommandActionKindV1::Decision
                && action.title == "review the retained exception and update evidence or remove it"
        }),
        "the primary action must reuse the ranked step of the highest-ranked item",
    )?;
    ensure(
        summary.additional_action_count == 1
            && summary.additional_actions_ref.as_deref()
                == Some("cargo-allow.worklist.v1.work_items"),
        "remaining queue items must stay retrievable from the worklist artifact",
    )
}

#[test]
fn worklist_adapter_never_reads_an_empty_filtered_queue_as_a_clean_repository() -> Result<(), String>
{
    let mut facts = worklist_facts();
    facts.filtered = true;
    let summary = core_command_summary_from_worklist(facts)?;
    ensure(
        summary.result_class == ResultClassV1::NotProven
            && summary.posture != CoreCommandPostureV1::Satisfied,
        "an empty filtered queue proves nothing about the repository",
    )?;
    ensure(
        summary.primary_action.as_ref().is_some_and(|action| {
            action
                .args
                .iter()
                .map(String::as_str)
                .eq(["worklist", "--format", "json"])
        }),
        "the deterministic safe route is to list the unfiltered queue",
    )?;
    ensure(
        summary
            .subject
            .limitations
            .iter()
            .any(|limitation| limitation.contains("the queue was filtered")),
        "the filter must be stated as a subject limitation",
    )
}

#[test]
fn worklist_adapter_keeps_partial_coverage_non_green() -> Result<(), String> {
    let mut facts = worklist_facts();
    facts.completeness = CompletenessV1::Partial;
    facts.coverage_limitation = Some("Git reported no tracked files".to_string());
    let summary = core_command_summary_from_worklist(facts)?;
    ensure(
        summary.result_class == ResultClassV1::PartialData
            && summary.posture == CoreCommandPostureV1::Blocking,
        "an incomplete scan must not present its queue as the whole story",
    )?;
    ensure(
        summary.next_proof.is_none(),
        "partial coverage must not imply the gate would prove anything",
    )
}
