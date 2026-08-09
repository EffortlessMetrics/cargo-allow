//! Projections for the read-only inspection commands: `explain`, `why`, and
//! `worklist`.
//!
//! Every adapter here is a pure function of facts the command already computed.
//! None of them scans source, reloads policy, re-evaluates matching, or selects
//! a repository judgment. The detailed command artifacts remain authoritative.
//!
//! ## One shared route rule
//!
//! These commands rank their next steps with the typed worklist ontology
//! (`worklist::work_item_kind`, `worklist::suggested_actions_for_context`,
//! `worklist::proof_commands`) or, for `why`, with `why_next_steps`. Those
//! ranked steps are operator prose describing a repository judgment — "remove
//! the exception" versus "receipt it with a reviewed entry" — not a command
//! cargo-allow may run on the operator's behalf. So the projection promotes the
//! first ranked step as a [`CoreCommandActionV1::decision`] and never invents a
//! second action ontology or a preferred mutation. Where a command *is*
//! deterministic and safe (re-running an unfiltered `worklist`, running the
//! enforcing gate), it is emitted as a command action instead.

use allow_core::MatchStatus;
use effortless_repo_protocol::{ClaimBoundaryV1, CompletenessV1, CurrentnessV1, ResultClassV1};

use super::{
    CoreCommandActionV1, CoreCommandEffectsV1, CoreCommandPostureV1, CoreCommandReasonV1,
    CoreCommandSummaryInputV1, CoreCommandSummaryV1, CoreSourceSubjectV1,
    build_core_command_summary,
};

/// Facts `explain` already holds about the single entry it explained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplainSummaryFactsV1 {
    pub tool_version: String,
    pub subject: CoreSourceSubjectV1,
    pub completeness: CompletenessV1,
    pub coverage_limitation: Option<String>,
    pub allow_id: String,
    /// Status of the first outcome for this entry that is not `Matched`.
    /// `None` means every finding this entry matches is receipted.
    pub attention_status: Option<MatchStatus>,
    pub matching_finding_count: usize,
    /// Ordered next steps from `explain_steps::explain_next_steps`, unmodified.
    pub suggested_actions: Vec<String>,
    pub claim_boundary: ClaimBoundaryV1,
}

/// Facts `why` already holds about the single finding it explained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhySummaryFactsV1 {
    pub tool_version: String,
    pub subject: CoreSourceSubjectV1,
    pub completeness: CompletenessV1,
    pub coverage_limitation: Option<String>,
    /// The exact queried location, `path:line`.
    pub location: String,
    pub outcome_status: MatchStatus,
    /// The allow entry the evaluation bound to, when it bound to one. Near-miss
    /// candidates are never promoted into this field.
    pub matched_allow_id: Option<String>,
    pub near_miss_candidate_count: usize,
    /// Ordered next steps from `why_render::why_next_steps`, unmodified.
    pub suggested_actions: Vec<String>,
    /// Source-tree-relative path of the add-finding plan this run wrote, when
    /// `--plan` was requested.
    pub plan_path: Option<String>,
    pub claim_boundary: ClaimBoundaryV1,
}

/// One already-built work item, projected down to the fields the summary reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorklistSummaryItemV1 {
    /// Typed `worklist::work_item_kind` value.
    pub kind: String,
    pub status: MatchStatus,
    pub allow_id: Option<String>,
    pub path: Option<String>,
    /// The item's own ranked suggested actions, unmodified.
    pub suggested_actions: Vec<String>,
}

/// Facts `worklist` already holds about the queue it rendered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorklistSummaryFactsV1 {
    pub tool_version: String,
    pub subject: CoreSourceSubjectV1,
    pub completeness: CompletenessV1,
    pub coverage_limitation: Option<String>,
    /// The rendered queue, in the order `worklist` already ranked it.
    pub items: Vec<WorklistSummaryItemV1>,
    /// True when any queue filter narrowed the listing, so an empty queue does
    /// not prove the repository has no work.
    pub filtered: bool,
    pub claim_boundary: ClaimBoundaryV1,
}

/// Project `explain` onto the common operator summary.
///
/// The subject is the explained ledger entry. Posture comes from the entry's
/// own outcome status through the gate's severity ranking, so `explain` cannot
/// call an entry healthy that `check` would fail on.
pub fn core_command_summary_from_explain(
    facts: ExplainSummaryFactsV1,
) -> Result<CoreCommandSummaryV1, String> {
    let ExplainSummaryFactsV1 {
        tool_version,
        mut subject,
        completeness,
        coverage_limitation,
        allow_id,
        attention_status,
        matching_finding_count,
        suggested_actions,
        claim_boundary,
    } = facts;
    if let Some(limitation) = coverage_limitation {
        subject.limitations.push(limitation);
    }

    let (result_class, posture, reason) = if completeness != CompletenessV1::Complete {
        (
            ResultClassV1::PartialData,
            CoreCommandPostureV1::Blocking,
            CoreCommandReasonV1 {
                code: "explain.partial_coverage".to_string(),
                message: format!(
                    "source inventory or scanner coverage is incomplete, so the current state of `{allow_id}` is not conclusive"
                ),
            },
        )
    } else if let Some(status) = attention_status {
        (
            ResultClassV1::Findings,
            status_posture(status),
            CoreCommandReasonV1 {
                code: format!("explain.{}", status.as_str()),
                message: format!(
                    "allow entry `{allow_id}` carries a `{}` match outcome",
                    status.as_str()
                ),
            },
        )
    } else if suggested_actions.is_empty() {
        (
            ResultClassV1::Completed,
            CoreCommandPostureV1::Satisfied,
            CoreCommandReasonV1 {
                code: "explain.entry_healthy".to_string(),
                message: format!(
                    "allow entry `{allow_id}` matches {matching_finding_count} current finding(s) with no outstanding maintenance"
                ),
            },
        )
    } else {
        (
            ResultClassV1::Findings,
            CoreCommandPostureV1::Advisory,
            CoreCommandReasonV1 {
                code: "explain.entry_maintenance".to_string(),
                message: format!(
                    "allow entry `{allow_id}` matches its current findings but its evidence or lifecycle still needs maintenance"
                ),
            },
        )
    };

    let primary_action = ranked_judgment_action(
        &suggested_actions,
        format!("explain.{allow_id}.next_step"),
        format!(
            "the ranked next step for `{allow_id}` is a repository judgment, so cargo-allow does not choose it"
        ),
        "the detailed explain artifact lists every ranked step and its proof commands",
    );
    let (additional_action_count, additional_actions_ref) = additional_actions(
        &suggested_actions,
        primary_action.is_some(),
        "cargo-allow.explain.v1.next.suggested_actions",
    );

    build_core_command_summary(CoreCommandSummaryInputV1 {
        tool_version,
        operation: "explain".to_string(),
        mode: None,
        profile: None,
        subject,
        result_class,
        posture,
        completeness,
        currentness: CurrentnessV1::Current,
        reason,
        primary_action,
        additional_action_count,
        additional_actions_ref,
        operation_effects: CoreCommandEffectsV1::read_only(read_only_non_effects()),
        next_proof: enforcing_gate_proof(
            completeness,
            result_class,
            "explain",
            "explain describes one ledger entry and never evaluates the repository gate",
        ),
        artifacts: Vec::new(),
        claim_boundary,
    })
}

/// Project `why` onto the common operator summary.
///
/// The subject is the exact queried location. A finding with no matching entry
/// stays an unresolved finding: near-miss candidates are counted, never named
/// as a winner, because choosing among them is repository judgment.
///
/// `--plan` writes an add-finding plan. That candidate artifact is not policy
/// and not source, but it *is* a write, so the summary reports the operation as
/// writing exactly that path rather than claiming read-only.
pub fn core_command_summary_from_why(
    facts: WhySummaryFactsV1,
) -> Result<CoreCommandSummaryV1, String> {
    let WhySummaryFactsV1 {
        tool_version,
        mut subject,
        completeness,
        coverage_limitation,
        location,
        outcome_status,
        matched_allow_id,
        near_miss_candidate_count,
        suggested_actions,
        plan_path,
        claim_boundary,
    } = facts;
    if let Some(limitation) = coverage_limitation {
        subject.limitations.push(limitation);
    }

    let (result_class, posture, reason) = why_disposition(
        completeness,
        outcome_status,
        matched_allow_id.as_deref(),
        near_miss_candidate_count,
        &location,
    );

    // A satisfied query has nothing for the operator to decide. Everything else
    // routes through the ranked step the detailed artifact already published.
    let primary_action = if result_class == ResultClassV1::Completed {
        None
    } else if let Some(plan_path) = plan_path.as_deref() {
        Some(
            CoreCommandActionV1::decision(
                "why.review_add_finding_plan",
                format!("Review the add-finding plan written to {plan_path}"),
            )
            .with_contract(
                "the plan is a reviewable candidate artifact; retaining the exception is repository judgment",
                format!(
                    "a maintainer reviews {plan_path} and either applies it with reviewed owner and reason fields or repairs the source instead"
                ),
                "writing the plan neither approves the exception nor changes policy, source, or the gate result",
            ),
        )
    } else {
        ranked_judgment_action(
            &suggested_actions,
            "why.next_step",
            "the ranked next step for this finding is a repository judgment, so cargo-allow does not choose it",
            "the detailed why artifact lists every ranked step and its structured proof plans",
        )
    };
    let (additional_action_count, additional_actions_ref) = additional_actions(
        &suggested_actions,
        // A plan review replaces the ranked step rather than consuming it, so
        // every ranked step remains available behind the retrieval reference.
        primary_action.is_some() && plan_path.is_none(),
        "cargo-allow.why.v1.next.suggested_actions",
    );

    let operation_effects = match plan_path.as_deref() {
        None => CoreCommandEffectsV1::read_only(read_only_non_effects()),
        Some(plan_path) => CoreCommandEffectsV1 {
            reads_repository: true,
            writes_repository: true,
            executes_repository_code: false,
            invokes_network: false,
            write_paths: vec![plan_path.to_string()],
            explicit_non_effects: vec![
                "does not modify source, policy, Git, hooks, workflows, or GitHub settings"
                    .to_string(),
                "does not retain, approve, or authorize the exception the plan describes"
                    .to_string(),
                "does not execute repository code or external evidence tools".to_string(),
            ],
        },
    };

    build_core_command_summary(CoreCommandSummaryInputV1 {
        tool_version,
        operation: "why".to_string(),
        mode: None,
        profile: None,
        subject,
        result_class,
        posture,
        completeness,
        currentness: CurrentnessV1::Current,
        reason,
        primary_action,
        additional_action_count,
        additional_actions_ref,
        operation_effects,
        next_proof: enforcing_gate_proof(
            completeness,
            result_class,
            "why",
            "why explains one finding and never evaluates the repository gate",
        ),
        artifacts: Vec::new(),
        claim_boundary,
    })
}

fn why_disposition(
    completeness: CompletenessV1,
    status: MatchStatus,
    matched_allow_id: Option<&str>,
    near_miss_candidate_count: usize,
    location: &str,
) -> (ResultClassV1, CoreCommandPostureV1, CoreCommandReasonV1) {
    if completeness != CompletenessV1::Complete {
        return (
            ResultClassV1::PartialData,
            CoreCommandPostureV1::Blocking,
            CoreCommandReasonV1 {
                code: "why.partial_coverage".to_string(),
                message: format!(
                    "source inventory or scanner coverage is incomplete, so the evaluation of {location} is not conclusive"
                ),
            },
        );
    }
    if status == MatchStatus::Matched {
        // A matched outcome without a recorded entry ID cannot be attributed,
        // and an unattributable receipt is not proof of coverage.
        let Some(allow_id) = matched_allow_id else {
            return (
                ResultClassV1::NotProven,
                CoreCommandPostureV1::Blocking,
                CoreCommandReasonV1 {
                    code: "why.matched_without_allow_id".to_string(),
                    message: format!(
                        "the finding at {location} evaluated as matched without a recorded allow entry ID, so its coverage cannot be attributed"
                    ),
                },
            );
        };
        return (
            ResultClassV1::Completed,
            CoreCommandPostureV1::Satisfied,
            CoreCommandReasonV1 {
                code: "why.receipted".to_string(),
                message: format!(
                    "the finding at {location} is receipted by allow entry `{allow_id}`"
                ),
            },
        );
    }
    let message = match (status, matched_allow_id) {
        (MatchStatus::New, _) if near_miss_candidate_count > 0 => {
            let listed = if near_miss_candidate_count == 1 {
                "1 near-miss entry was listed"
            } else {
                &format!("{near_miss_candidate_count} near-miss entries were listed")
            };
            format!(
                "no allow entry matches the finding at {location}; {listed} and none of them receipts it"
            )
        }
        (MatchStatus::New, _) => {
            format!("no allow entry matches the finding at {location}")
        }
        (status, Some(allow_id)) => format!(
            "the finding at {location} evaluated as `{}` against allow entry `{allow_id}`",
            status.as_str()
        ),
        (status, None) => format!(
            "the finding at {location} evaluated as `{}` with no bound allow entry",
            status.as_str()
        ),
    };
    (
        ResultClassV1::Findings,
        status_posture(status),
        CoreCommandReasonV1 {
            code: format!("why.{}", status.as_str()),
            message,
        },
    )
}

/// Project `worklist` onto the common operator summary.
///
/// The subject is the repository worktree. An empty unfiltered queue is a
/// satisfied result; a filtered queue only ever describes the slice it listed,
/// so an empty filtered queue routes to the unfiltered listing instead of
/// claiming the repository is clean.
pub fn core_command_summary_from_worklist(
    facts: WorklistSummaryFactsV1,
) -> Result<CoreCommandSummaryV1, String> {
    let WorklistSummaryFactsV1 {
        tool_version,
        mut subject,
        completeness,
        coverage_limitation,
        items,
        filtered,
        claim_boundary,
    } = facts;
    if let Some(limitation) = coverage_limitation {
        subject.limitations.push(limitation);
    }
    if filtered {
        subject.limitations.push(
            "the queue was filtered; work items outside the selected filters were not listed"
                .to_string(),
        );
    }

    let (result_class, posture, reason, primary_action) =
        worklist_disposition(completeness, filtered, &items);
    // Every queue item beyond the promoted one stays retrievable in the
    // command's own artifact rather than being re-ranked here.
    let remaining = items
        .len()
        .saturating_sub(usize::from(primary_action.is_some()));
    let (additional_action_count, additional_actions_ref) = if remaining == 0 {
        (0, None)
    } else {
        (
            remaining,
            Some("cargo-allow.worklist.v1.work_items".to_string()),
        )
    };

    build_core_command_summary(CoreCommandSummaryInputV1 {
        tool_version,
        operation: "worklist".to_string(),
        mode: None,
        profile: None,
        subject,
        result_class,
        posture,
        completeness,
        currentness: CurrentnessV1::Current,
        reason,
        primary_action,
        additional_action_count,
        additional_actions_ref,
        operation_effects: CoreCommandEffectsV1::read_only(read_only_non_effects()),
        next_proof: enforcing_gate_proof(
            completeness,
            result_class,
            "worklist",
            "the worklist is guidance and never evaluates the repository gate",
        ),
        artifacts: Vec::new(),
        claim_boundary,
    })
}

fn worklist_disposition(
    completeness: CompletenessV1,
    filtered: bool,
    items: &[WorklistSummaryItemV1],
) -> (
    ResultClassV1,
    CoreCommandPostureV1,
    CoreCommandReasonV1,
    Option<CoreCommandActionV1>,
) {
    if completeness != CompletenessV1::Complete {
        return (
            ResultClassV1::PartialData,
            CoreCommandPostureV1::Blocking,
            CoreCommandReasonV1 {
                code: "worklist.partial_coverage".to_string(),
                message:
                    "source inventory or scanner coverage is incomplete, so the queue may omit work"
                        .to_string(),
            },
            None,
        );
    }
    let Some(first) = items.first() else {
        if filtered {
            return (
                ResultClassV1::NotProven,
                CoreCommandPostureV1::Advisory,
                CoreCommandReasonV1 {
                    code: "worklist.filtered_queue_empty".to_string(),
                    message:
                        "the filtered work queue is empty; the unfiltered repository queue was not listed"
                            .to_string(),
                },
                Some(
                    CoreCommandActionV1::command(
                        "worklist.list_unfiltered_queue",
                        "List the unfiltered work queue",
                        "cargo-allow",
                        vec![
                            "worklist".to_string(),
                            "--format".to_string(),
                            "json".to_string(),
                        ],
                    )
                    .with_contract(
                        "an empty filtered queue says nothing about work outside the selected filters",
                        "the complete current work queue is emitted",
                        "the worklist is guidance and does not mutate source or policy",
                    ),
                ),
            );
        }
        return (
            ResultClassV1::Completed,
            CoreCommandPostureV1::Satisfied,
            CoreCommandReasonV1 {
                code: "worklist.empty_queue".to_string(),
                message: "no source-exception work items are queued".to_string(),
            },
            None,
        );
    };

    // Severity comes from the gate's own ranking over every queued item, not
    // from the position of the first one.
    let blocking = items.iter().any(|item| item.status.is_failure_in_no_new());
    let posture = if blocking {
        CoreCommandPostureV1::Blocking
    } else {
        CoreCommandPostureV1::Advisory
    };
    // Keep both coordinates when the item has them: the allow ID says which
    // ledger entry to open, the path says where to look.
    let scope = match (first.allow_id.as_deref(), first.path.as_deref()) {
        (Some(allow_id), Some(path)) => format!(" for allow entry `{allow_id}` at {path}"),
        (Some(allow_id), None) => format!(" for allow entry `{allow_id}`"),
        (None, Some(path)) => format!(" at {path}"),
        (None, None) => String::new(),
    };
    let reason = CoreCommandReasonV1 {
        code: format!("worklist.{}", first.kind),
        message: format!(
            "{} work item(s) are queued; the highest-ranked item is `{}`{scope}",
            items.len(),
            first.kind
        ),
    };
    let primary_action = ranked_judgment_action(
        &first.suggested_actions,
        format!("worklist.{}.next_step", first.kind),
        format!(
            "the ranked next step for the `{}` work item is a repository judgment, so cargo-allow does not choose it",
            first.kind
        ),
        "the detailed worklist artifact lists every queued item with its own actions and proof commands",
    );
    (ResultClassV1::Findings, posture, reason, primary_action)
}

/// Posture for one typed match status.
///
/// Blocking versus advisory reuses `MatchStatus::is_failure_in_no_new`, the
/// same ranking the enforcing gate applies, so no second severity ontology is
/// introduced. An ambiguous outcome is escalated to a repository decision:
/// several allow entries compete and choosing between them is judgment.
fn status_posture(status: MatchStatus) -> CoreCommandPostureV1 {
    match status {
        MatchStatus::Ambiguous => CoreCommandPostureV1::DecisionRequired,
        status if status.is_failure_in_no_new() => CoreCommandPostureV1::Blocking,
        _ => CoreCommandPostureV1::Advisory,
    }
}

/// Promote the first already-ranked next step as the single primary action.
///
/// The step stays verbatim so the summary and the detailed artifact cannot
/// disagree, and it is emitted as a decision because the ranked steps describe
/// repository judgment rather than a command cargo-allow may choose.
fn ranked_judgment_action(
    suggested_actions: &[String],
    id: impl Into<String>,
    reason: impl Into<String>,
    proof_boundary: &str,
) -> Option<CoreCommandActionV1> {
    let title = suggested_actions
        .iter()
        .find(|action| !action.trim().is_empty())?;
    Some(
        CoreCommandActionV1::decision(id, title.clone()).with_contract(
            reason,
            "a maintainer carries out the ranked step, or decides against it, in the repository",
            proof_boundary,
        ),
    )
}

fn additional_actions(
    suggested_actions: &[String],
    primary_promoted: bool,
    retrieval_reference: &str,
) -> (usize, Option<String>) {
    let count = suggested_actions
        .len()
        .saturating_sub(usize::from(primary_promoted));
    if count == 0 {
        return (0, None);
    }
    (count, Some(retrieval_reference.to_string()))
}

/// The enforcing gate is only a meaningful follow-up when the inspection
/// actually covered its subject and reached a conclusive result.
fn enforcing_gate_proof(
    completeness: CompletenessV1,
    result_class: ResultClassV1,
    operation: &str,
    reason: &str,
) -> Option<CoreCommandActionV1> {
    (completeness == CompletenessV1::Complete
        && matches!(
            result_class,
            ResultClassV1::Completed | ResultClassV1::Findings
        ))
    .then(|| {
        CoreCommandActionV1::command(
            format!("{operation}.full_no_new_check"),
            "Run the enforcing no-new check",
            "cargo-allow",
            vec![
                "check".to_string(),
                "--mode".to_string(),
                "no-new".to_string(),
            ],
        )
        .with_contract(
            reason,
            "the current repository is evaluated under the no-new gate",
            "the source-syntax gate does not prove compiled or runtime correctness",
        )
    })
}

fn read_only_non_effects() -> Vec<String> {
    vec![
        "does not modify source, policy, Git, hooks, workflows, or GitHub settings".to_string(),
        "does not retain, repair, extend, or authorize any source exception".to_string(),
        "does not execute repository code or external evidence tools".to_string(),
    ]
}
