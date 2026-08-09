use allow_core::{CargoAllowError, CargoAllowErrorKind};
use effortless_repo_protocol::{ClaimBoundaryV1, CompletenessV1, CurrentnessV1, ResultClassV1};

use super::{
    CoreCommandActionV1, CoreCommandEffectsV1, CoreCommandPostureV1, CoreCommandReasonV1,
    CoreCommandSummaryInputV1, CoreCommandSummaryV1, CoreCommandWritePostureV1,
    CoreSourceSubjectKindV1, CoreSourceSubjectV1, build_core_command_summary,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitSummaryFactsV1 {
    pub repository_identity: String,
    pub portable_identity: String,
    pub config_path: String,
    pub dry_run: bool,
    pub force: bool,
    pub path_existed: bool,
}

pub fn core_command_summary_from_init(
    facts: InitSummaryFactsV1,
) -> Result<CoreCommandSummaryV1, String> {
    let InitSummaryFactsV1 {
        repository_identity,
        portable_identity,
        config_path,
        dry_run,
        force,
        path_existed,
    } = facts;
    let action = if dry_run {
        if path_existed && !force {
            "keep"
        } else if path_existed {
            "overwrite"
        } else {
            "create"
        }
    } else if path_existed {
        "overwrote"
    } else {
        "created"
    };
    let mut apply_args = vec![
        "init".to_string(),
        "--config".to_string(),
        config_path.clone(),
    ];
    if force {
        apply_args.push("--force".to_string());
    }
    let (result_class, posture, effects, primary_action) = if dry_run {
        (
            ResultClassV1::Completed,
            CoreCommandPostureV1::Advisory,
            CoreCommandEffectsV1::read_only(vec![
                "does not create, overwrite, or modify the policy file".to_string(),
                "does not execute repository code or external evidence tools".to_string(),
            ]),
            Some(
                CoreCommandActionV1::command(
                    "init.apply",
                    "Apply the starter policy plan",
                    "cargo-allow",
                    apply_args,
                )
                .with_write_posture(
                    CoreCommandWritePostureV1::LiveMutation,
                    vec![config_path.clone()],
                )
                .with_contract(
                    "the dry run is only a preview of the selected policy target",
                    "the starter source-exception policy is written to the named target",
                    "writing a starter policy does not approve any exception or prove the repository clean",
                ),
            ),
        )
    } else {
        (
            ResultClassV1::Completed,
            CoreCommandPostureV1::Satisfied,
            CoreCommandEffectsV1 {
                reads_repository: true,
                writes_repository: true,
                executes_repository_code: false,
                invokes_network: false,
                write_paths: vec![config_path.clone()],
                explicit_non_effects: vec![
                    "does not approve or receipt individual findings".to_string(),
                    "does not execute repository code or external evidence tools".to_string(),
                ],
            },
            Some(
                CoreCommandActionV1::command(
                    "init.full_no_new_check",
                    "Run the enforcing no-new check",
                    "cargo-allow",
                    vec![
                        "check".to_string(),
                        "--mode".to_string(),
                        "no-new".to_string(),
                    ],
                )
                .with_contract(
                    "initialization changes policy bytes but does not establish repository posture",
                    "the current source tree is evaluated under the no-new gate",
                    "source-syntax evaluation does not prove compiled or runtime correctness",
                ),
            ),
        )
    };
    build_core_command_summary(CoreCommandSummaryInputV1 {
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        operation: "init".to_string(),
        mode: None,
        profile: None,
        subject: CoreSourceSubjectV1 {
            kind: CoreSourceSubjectKindV1::Worktree,
            repository_identity,
            portable_identity,
            base: None,
            head: None,
            paths: vec![config_path.clone()],
            limitations: vec![
                "initialization describes the selected policy target, not the full repository posture"
                    .to_string(),
            ],
        },
        result_class,
        posture,
        completeness: CompletenessV1::Complete,
        currentness: CurrentnessV1::Current,
        reason: CoreCommandReasonV1 {
            code: if dry_run {
                format!("init.preview_{action}")
            } else {
                format!("init.{action}")
            },
            message: if dry_run {
                format!("would {action} {config_path}")
            } else {
                format!("{action} {config_path}")
            },
        },
        primary_action,
        additional_action_count: 0,
        additional_actions_ref: None,
        operation_effects: effects,
        next_proof: None,
        artifacts: Vec::new(),
        claim_boundary: ClaimBoundaryV1::new(
            "starter source-exception policy target and write posture only",
        )
        .with_limitations(vec![
            "the summary does not approve exceptions or establish compiled/runtime correctness"
                .to_string(),
        ]),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposeSummaryFactsV1 {
    pub repository_identity: String,
    pub portable_identity: String,
    pub write_path: Option<String>,
    pub force: bool,
    pub completeness: CompletenessV1,
    pub proposed_entries: usize,
    pub unsafe_proposed_entries: usize,
    pub truncated_new_findings: usize,
    pub unreceiptable_new_findings: usize,
}

pub fn core_command_summary_from_propose(
    facts: ProposeSummaryFactsV1,
) -> Result<CoreCommandSummaryV1, String> {
    let ProposeSummaryFactsV1 {
        repository_identity,
        portable_identity,
        write_path,
        force,
        completeness,
        proposed_entries,
        unsafe_proposed_entries,
        truncated_new_findings,
        unreceiptable_new_findings,
    } = facts;
    let complete = completeness == CompletenessV1::Complete;
    let write_path_for_action = write_path.clone();
    let primary_action = Some(
        CoreCommandActionV1::decision(
            "propose.review_candidate",
            "Review the generated candidate policy before relying on it",
        )
        .with_contract(
            "generated baseline entries are candidate policy, not human approval",
            "a maintainer reviews ownership, expiry, evidence, and scope before adoption",
            "the summary does not establish that any generated exception is justified",
        ),
    );
    let next_proof = write_path_for_action.map(|path| {
        CoreCommandActionV1::command(
            "propose.targeted_no_new_check",
            "Run the no-new check against the candidate policy",
            "cargo-allow",
            vec![
                "check".to_string(),
                "--mode".to_string(),
                "no-new".to_string(),
                "--config".to_string(),
                path,
            ],
        )
        .with_contract(
            "proposal generation and candidate review do not prove policy posture",
            "the source tree is evaluated against the written candidate policy",
            "this remains source-syntax proof and does not establish compiled or runtime correctness",
        )
    });
    let (result_class, posture) = if complete {
        (ResultClassV1::Completed, CoreCommandPostureV1::Advisory)
    } else {
        (ResultClassV1::PartialData, CoreCommandPostureV1::Blocking)
    };
    let write_effect = write_path.clone();
    let operation_effects = match write_effect {
        Some(path) => CoreCommandEffectsV1 {
            reads_repository: true,
            writes_repository: true,
            executes_repository_code: false,
            invokes_network: false,
            write_paths: vec![path],
            explicit_non_effects: vec![
                "does not approve or justify generated baseline entries".to_string(),
                "does not execute repository code or external evidence tools".to_string(),
            ],
        },
        None => CoreCommandEffectsV1::read_only(vec![
            "does not write a policy file when --write is absent".to_string(),
            "does not approve or justify generated baseline entries".to_string(),
            "does not execute repository code or external evidence tools".to_string(),
        ]),
    };
    let write_mode = if force {
        "overwrite allowed"
    } else {
        "create-only"
    };
    let message = if let Some(path) = write_path.as_deref() {
        format!(
            "wrote a candidate policy with {proposed_entries} baseline_debt entr{} at {path} ({write_mode})",
            if proposed_entries == 1 { "y" } else { "ies" }
        )
    } else {
        format!(
            "generated {proposed_entries} candidate baseline_debt entr{} for review",
            if proposed_entries == 1 { "y" } else { "ies" }
        )
    };
    let mut limitations = vec![
        "generated entries remain subject to human ownership, evidence, expiry, and scope review"
            .to_string(),
        "the proposal does not approve exceptions or prove compiled/runtime correctness"
            .to_string(),
    ];
    if truncated_new_findings > 0 {
        limitations.push(format!(
            "{truncated_new_findings} new finding(s) were outside the proposal limit"
        ));
    }
    if unreceiptable_new_findings > 0 {
        limitations.push(format!(
            "{unreceiptable_new_findings} new finding(s) were not eligible for generated entries"
        ));
    }
    if unsafe_proposed_entries > 0 {
        limitations.push(format!(
            "{unsafe_proposed_entries} generated entr{} concern unsafe findings and require explicit review",
            if unsafe_proposed_entries == 1 { "y" } else { "ies" }
        ));
    }
    build_core_command_summary(CoreCommandSummaryInputV1 {
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        operation: "propose".to_string(),
        mode: None,
        profile: None,
        subject: CoreSourceSubjectV1 {
            kind: CoreSourceSubjectKindV1::Worktree,
            repository_identity,
            portable_identity,
            base: None,
            head: None,
            paths: write_path.into_iter().collect(),
            limitations: vec![
                "the current worktree result is not bound to a commit, tree, or Git-index identity"
                    .to_string(),
            ],
        },
        result_class,
        posture,
        completeness,
        currentness: CurrentnessV1::Current,
        reason: CoreCommandReasonV1 {
            code: if complete {
                "propose.candidate_generated".to_string()
            } else {
                "propose.partial_coverage".to_string()
            },
            message,
        },
        primary_action,
        additional_action_count: 0,
        additional_actions_ref: None,
        operation_effects,
        next_proof,
        artifacts: Vec::new(),
        claim_boundary: ClaimBoundaryV1::new(
            "generated baseline candidate and policy write posture only",
        )
        .with_limitations(limitations),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffSummaryFactsV1 {
    pub repository_identity: String,
    pub portable_identity: String,
    pub base: String,
    pub head: Option<String>,
    pub result_class: ResultClassV1,
    pub completeness: CompletenessV1,
    pub currentness: CurrentnessV1,
    pub current_failures: usize,
    pub failed: bool,
}

pub fn core_command_summary_from_diff(
    facts: DiffSummaryFactsV1,
) -> Result<CoreCommandSummaryV1, String> {
    let DiffSummaryFactsV1 {
        repository_identity,
        portable_identity,
        base,
        head,
        result_class,
        completeness,
        currentness,
        current_failures,
        failed,
    } = facts;
    let comparison_complete = result_class == ResultClassV1::Completed;
    let result_class = if failed && comparison_complete {
        ResultClassV1::Findings
    } else {
        result_class
    };
    let posture = if failed {
        CoreCommandPostureV1::Blocking
    } else {
        CoreCommandPostureV1::Satisfied
    };
    let reason = if comparison_complete && failed {
        CoreCommandReasonV1 {
            code: "diff.blocking_findings".to_string(),
            message: format!(
                "the revision comparison is complete but {} finding(s) require attention",
                current_failures
            ),
        }
    } else if result_class == ResultClassV1::Completed {
        CoreCommandReasonV1 {
            code: "diff.satisfied".to_string(),
            message: "the selected revisions have a complete diff posture".to_string(),
        }
    } else {
        CoreCommandReasonV1 {
            code: format!("diff.{}", result_class.as_str()),
            message: format!(
                "the selected revision comparison is not complete ({} failure(s) remain)",
                current_failures
            ),
        }
    };
    let primary_action = failed.then(|| {
        CoreCommandActionV1::command(
            "diff.inspect_report",
            "Inspect the diff report",
            "cargo-allow",
            vec![
                "diff".to_string(),
                "--format".to_string(),
                "json".to_string(),
            ],
        )
        .with_contract(
            "the diff summary is a compact posture projection",
            "the detailed diff report exposes the exact finding and policy changes",
            "the report does not prove compiled or runtime behavior",
        )
    });
    let next_proof = (!failed && result_class == ResultClassV1::Completed).then(|| {
        CoreCommandActionV1::command(
            "diff.rerun_no_new",
            "Run the enforcing no-new check",
            "cargo-allow",
            vec![
                "check".to_string(),
                "--mode".to_string(),
                "no-new".to_string(),
            ],
        )
        .with_contract(
            "diff is comparative evidence, not the repository enforcement gate",
            "the current repository posture is evaluated under no-new",
            "source-syntax evaluation does not prove compiled or runtime correctness",
        )
    });
    let mut subject = CoreSourceSubjectV1 {
        kind: CoreSourceSubjectKindV1::CommittedRange,
        repository_identity,
        portable_identity,
        base: Some(base),
        head,
        paths: Vec::new(),
        limitations: Vec::new(),
    };
    if subject.head.is_none() {
        subject.limitations.push(
            "the comparison head is the current worktree rather than an explicitly pinned revision"
                .to_string(),
        );
    }
    build_core_command_summary(CoreCommandSummaryInputV1 {
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        operation: "diff".to_string(),
        mode: None,
        profile: None,
        subject,
        result_class,
        posture,
        completeness,
        currentness,
        reason,
        primary_action,
        additional_action_count: 0,
        additional_actions_ref: None,
        operation_effects: CoreCommandEffectsV1::read_only(vec![
            "does not modify source, policy, Git, hooks, workflows, or GitHub settings".to_string(),
            "does not execute repository code or external evidence tools".to_string(),
        ]),
        next_proof,
        artifacts: Vec::new(),
        claim_boundary: ClaimBoundaryV1::new(
            "revision comparison and source-exception posture only",
        )
        .with_limitations(vec![
            "macro expansion, type information, MIR, control flow, and data flow were not analyzed"
                .to_string(),
            "a complete diff does not establish release readiness or runtime correctness"
                .to_string(),
        ]),
    })
}

pub fn core_command_summary_from_adoption_plan(
    plan: &allow_report::CoreAdoptionPlanV1,
) -> Result<CoreCommandSummaryV1, String> {
    let (result_class, posture) = adoption_result(plan.bootstrap_disposition);
    let completeness = match plan.inventory.completeness {
        allow_report::InventoryCompleteness::Complete => CompletenessV1::Complete,
        allow_report::InventoryCompleteness::Partial => CompletenessV1::Partial,
        allow_report::InventoryCompleteness::Unknown => CompletenessV1::Unknown,
    };
    let primary_action = action_from_adoption(&plan.primary_action, &plan.may_write_paths)?;
    let (next_proof_index, next_proof) = match plan
        .follow_up_actions
        .iter()
        .enumerate()
        .find(|(_, action)| action.kind == allow_report::AdoptionActionKind::RunNoNewCheck)
    {
        Some((index, action)) => (Some(index), Some(action_from_adoption(action, &[])?)),
        None => (None, None),
    };
    let additional_action_count = plan
        .follow_up_actions
        .len()
        .saturating_sub(usize::from(next_proof_index.is_some()));

    build_core_command_summary(CoreCommandSummaryInputV1 {
        tool_version: plan.tool_version.clone(),
        operation: "adopt".to_string(),
        mode: None,
        profile: None,
        subject: adoption_subject(plan),
        result_class,
        posture,
        completeness,
        currentness: CurrentnessV1::Current,
        reason: CoreCommandReasonV1 {
            code: format!(
                "adoption.{}",
                bootstrap_disposition_code(plan.bootstrap_disposition)
            ),
            message: plan.primary_action.reason.clone(),
        },
        primary_action: Some(primary_action),
        additional_action_count,
        additional_actions_ref: (additional_action_count > 0).then_some(format!(
            "core_adoption_plan.follow_up_actions?exclude_index={}",
            next_proof_index
                .map(|index| index.to_string())
                .unwrap_or_else(|| "none".to_string())
        )),
        operation_effects: CoreCommandEffectsV1::read_only(plan.explicit_non_effects.clone()),
        next_proof,
        artifacts: Vec::new(),
        claim_boundary: ClaimBoundaryV1::new(plan.claim_boundary.clone())
            .with_limitations(plan.limitations.clone()),
    })
}

/// Describe the adoption subject in the same grammar `audit`, `check`, and
/// `doctor` use: the content-addressed repository identity stays in
/// `repository_identity`, while `portable_identity` names the evaluated subject
/// kind and inventory mode.
fn adoption_subject(plan: &allow_report::CoreAdoptionPlanV1) -> CoreSourceSubjectV1 {
    let mode = match plan.inventory.mode {
        allow_report::InventoryMode::GitTracked => "git_tracked",
        allow_report::InventoryMode::Filesystem => "filesystem",
        allow_report::InventoryMode::Unknown => "unknown",
    };
    let mut subject = CoreSourceSubjectV1::worktree(
        plan.repository_identity.clone(),
        format!("worktree:{mode}:current-unpinned"),
    );
    subject.limitations.push(
        "the current worktree result is not bound to a commit, tree, or Git-index identity"
            .to_string(),
    );
    subject
        .limitations
        .extend(plan.inventory.limitations.iter().cloned());
    subject
}

fn adoption_result(
    disposition: allow_report::BootstrapDisposition,
) -> (ResultClassV1, CoreCommandPostureV1) {
    match disposition {
        allow_report::BootstrapDisposition::CleanNoPolicy
        | allow_report::BootstrapDisposition::ExistingPolicyHealthy => {
            (ResultClassV1::Completed, CoreCommandPostureV1::Satisfied)
        }
        allow_report::BootstrapDisposition::FindingsNoPolicy
        | allow_report::BootstrapDisposition::ExistingPolicyHasNewFindings
        | allow_report::BootstrapDisposition::ExistingPolicyNeedsRepair => {
            (ResultClassV1::Findings, CoreCommandPostureV1::Advisory)
        }
        allow_report::BootstrapDisposition::PartialInventory => {
            (ResultClassV1::PartialData, CoreCommandPostureV1::Blocking)
        }
        allow_report::BootstrapDisposition::InvalidPolicy => (
            ResultClassV1::MalformedInput,
            CoreCommandPostureV1::Blocking,
        ),
        allow_report::BootstrapDisposition::UnsupportedRepositoryState => {
            (ResultClassV1::Unsupported, CoreCommandPostureV1::Blocking)
        }
        allow_report::BootstrapDisposition::InstrumentFailure => (
            ResultClassV1::InstrumentFailure,
            CoreCommandPostureV1::Blocking,
        ),
    }
}

fn bootstrap_disposition_code(disposition: allow_report::BootstrapDisposition) -> &'static str {
    match disposition {
        allow_report::BootstrapDisposition::CleanNoPolicy => "clean_no_policy",
        allow_report::BootstrapDisposition::FindingsNoPolicy => "findings_no_policy",
        allow_report::BootstrapDisposition::ExistingPolicyHealthy => "existing_policy_healthy",
        allow_report::BootstrapDisposition::ExistingPolicyHasNewFindings => {
            "existing_policy_has_new_findings"
        }
        allow_report::BootstrapDisposition::ExistingPolicyNeedsRepair => {
            "existing_policy_needs_repair"
        }
        allow_report::BootstrapDisposition::PartialInventory => "partial_inventory",
        allow_report::BootstrapDisposition::InvalidPolicy => "invalid_policy",
        allow_report::BootstrapDisposition::UnsupportedRepositoryState => {
            "unsupported_repository_state"
        }
        allow_report::BootstrapDisposition::InstrumentFailure => "instrument_failure",
    }
}

fn action_from_adoption(
    action: &allow_report::AdoptionAction,
    may_write_paths: &[String],
) -> Result<CoreCommandActionV1, String> {
    let mut argv = action.argv.iter();
    let Some(program) = argv.next() else {
        return Err("adoption action argv must include a program".to_string());
    };
    let (write_posture, write_paths) = match action.write_posture {
        allow_report::WritePosture::ReadOnly => (CoreCommandWritePostureV1::ReadOnly, Vec::new()),
        allow_report::WritePosture::PreviewOnly => {
            (CoreCommandWritePostureV1::PreviewOnly, Vec::new())
        }
        allow_report::WritePosture::MayWrite if may_write_paths.is_empty() => {
            return Err(
                "adoption MayWrite action requires policy.path-derived may_write_paths".to_string(),
            );
        }
        allow_report::WritePosture::MayWrite => (
            CoreCommandWritePostureV1::LiveMutation,
            may_write_paths.to_vec(),
        ),
    };

    Ok(CoreCommandActionV1::command(
        format!("adoption.{}", action.kind.as_str()),
        format!("Run {}", action.kind.as_str()),
        program.clone(),
        argv.cloned().collect(),
    )
    .with_write_posture(write_posture, write_paths)
    .with_contract(
        action.reason.clone(),
        action.expected_result.clone(),
        "The action must be re-evaluated against current command-specific inputs; this summary does not execute it.",
    ))
}

/// Doctor facts required to project the common operator summary.
///
/// These are read from the already-built [`allow_report::DoctorReport`] inputs.
/// The adapter does not rescan source, reload policy, or re-evaluate evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorSummaryFactsV1 {
    pub tool_version: String,
    pub subject: CoreSourceSubjectV1,
    pub completeness: CompletenessV1,
    pub coverage_limitation: Option<String>,
    pub config_present: bool,
    pub config_valid: Option<bool>,
    pub config_diagnostic: Option<String>,
    /// `None` means evidence health was not probed, which is never the same as
    /// zero defects and must not render as a satisfied result.
    pub broken_evidence_links: Option<usize>,
    pub weak_evidence_references: Option<usize>,
    pub claim_boundary: ClaimBoundaryV1,
}

/// Project `doctor` onto the common operator summary.
///
/// Doctor is a read-only diagnosis. It never repairs policy, so a non-green
/// posture selects the smallest safe *diagnostic or repair-planning* route
/// rather than a mutation. Where the repair is a repository judgment (invalid
/// or absent policy), the summary emits a decision action instead of guessing
/// a preferred command.
pub fn core_command_summary_from_doctor(
    facts: DoctorSummaryFactsV1,
) -> Result<CoreCommandSummaryV1, String> {
    let (result_class, posture, reason, primary_action) = doctor_disposition(&facts);
    // The enforcing gate is only a meaningful next step when the diagnosis
    // actually covered the repository. A partial, malformed, unprobed, or
    // otherwise inconclusive world must not imply that running `check` would
    // prove anything.
    let next_proof = (facts.completeness == CompletenessV1::Complete
        && matches!(
            result_class,
            ResultClassV1::Completed | ResultClassV1::Findings
        ))
    .then(|| {
        CoreCommandActionV1::command(
            "doctor.full_no_new_check",
            "Run the enforcing no-new check",
            "cargo-allow",
            vec![
                "check".to_string(),
                "--mode".to_string(),
                "no-new".to_string(),
            ],
        )
        .with_contract(
            "doctor diagnoses setup health and never enforces the source-exception gate",
            "the current repository is evaluated under the no-new gate",
            "the source-syntax gate does not prove compiled or runtime correctness",
        )
    });

    let mut subject = facts.subject;
    if let Some(limitation) = facts.coverage_limitation {
        subject.limitations.push(limitation);
    }

    build_core_command_summary(CoreCommandSummaryInputV1 {
        tool_version: facts.tool_version,
        operation: "doctor".to_string(),
        mode: None,
        profile: None,
        subject,
        result_class,
        posture,
        completeness: facts.completeness,
        currentness: CurrentnessV1::Current,
        reason,
        primary_action,
        additional_action_count: 0,
        additional_actions_ref: None,
        operation_effects: CoreCommandEffectsV1::read_only(vec![
            "does not modify source, policy, Git, hooks, workflows, or GitHub settings".to_string(),
            "does not repair, extend, or authorize any source exception".to_string(),
            "does not execute repository code or external evidence tools".to_string(),
        ]),
        next_proof,
        artifacts: Vec::new(),
        claim_boundary: facts.claim_boundary,
    })
}

fn doctor_disposition(
    facts: &DoctorSummaryFactsV1,
) -> (
    ResultClassV1,
    CoreCommandPostureV1,
    CoreCommandReasonV1,
    Option<CoreCommandActionV1>,
) {
    // Ordered most- to least-fundamental. An invalid policy is checked before
    // coverage because every later evidence fact is read through that policy.
    if facts.config_present && facts.config_valid != Some(true) {
        return (
            ResultClassV1::MalformedInput,
            CoreCommandPostureV1::DecisionRequired,
            CoreCommandReasonV1 {
                code: "doctor.invalid_policy_config".to_string(),
                message: facts.config_diagnostic.clone().unwrap_or_else(|| {
                    "the discovered policy configuration is not valid".to_string()
                }),
            },
            Some(
                CoreCommandActionV1::decision(
                    "doctor.repair_policy_config",
                    "Repair the invalid policy configuration",
                )
                .with_contract(
                    "a malformed policy cannot be repaired automatically without inventing repository judgment",
                    "a maintainer corrects the reported configuration defect at its source",
                    "cargo-allow does not validate the repair until doctor is rerun",
                ),
            ),
        );
    }

    if facts.completeness != CompletenessV1::Complete {
        return (
            ResultClassV1::PartialData,
            CoreCommandPostureV1::Blocking,
            CoreCommandReasonV1 {
                code: "doctor.partial_coverage".to_string(),
                message: "source inventory or scanner coverage is incomplete, so the remaining diagnosis is not conclusive"
                    .to_string(),
            },
            None,
        );
    }

    if !facts.config_present {
        return (
            ResultClassV1::Findings,
            CoreCommandPostureV1::Advisory,
            CoreCommandReasonV1 {
                code: "doctor.no_policy_config".to_string(),
                message: "no source-exception policy configuration was discovered".to_string(),
            },
            Some(
                CoreCommandActionV1::command(
                    "doctor.plan_adoption",
                    "Plan repository adoption",
                    "cargo-allow",
                    vec!["adopt".to_string()],
                )
                .with_contract(
                    "the repository has no exception ledger to diagnose yet",
                    "the bootstrap disposition and one bounded next adoption step are reported",
                    "adopt is read-only and does not create policy or approve any exception",
                ),
            ),
        );
    }

    // A valid policy is expected to yield probed evidence counts. If it did
    // not, say so rather than reading absent counts as zero defects.
    let (Some(broken_evidence_links), Some(weak_evidence_references)) =
        (facts.broken_evidence_links, facts.weak_evidence_references)
    else {
        return (
            ResultClassV1::NotProven,
            CoreCommandPostureV1::Blocking,
            CoreCommandReasonV1 {
                code: "doctor.evidence_health_not_probed".to_string(),
                message:
                    "policy evidence health was not probed, so setup health cannot be confirmed"
                        .to_string(),
            },
            None,
        );
    };

    if broken_evidence_links > 0 {
        return (
            ResultClassV1::Findings,
            CoreCommandPostureV1::Blocking,
            CoreCommandReasonV1 {
                code: "doctor.broken_evidence_links".to_string(),
                message: format!(
                    "{broken_evidence_links} policy evidence reference(s) do not resolve"
                ),
            },
            Some(evidence_worklist_action(
                "doctor.inspect_broken_evidence",
                "Inspect unresolved evidence references",
                "--broken-evidence",
                "policy entries reference evidence that cannot be resolved in this source tree",
            )),
        );
    }

    if weak_evidence_references > 0 {
        return (
            ResultClassV1::Findings,
            CoreCommandPostureV1::Advisory,
            CoreCommandReasonV1 {
                code: "doctor.weak_evidence_references".to_string(),
                message: format!(
                    "{weak_evidence_references} policy evidence reference(s) are weak"
                ),
            },
            Some(evidence_worklist_action(
                "doctor.inspect_weak_evidence",
                "Inspect weak evidence references",
                "--weak-evidence",
                "policy entries carry evidence references that do not durably identify their subject",
            )),
        );
    }

    (
        ResultClassV1::Completed,
        CoreCommandPostureV1::Satisfied,
        CoreCommandReasonV1 {
            code: "doctor.healthy_setup".to_string(),
            message:
                "policy configuration, inventory coverage, and evidence references are healthy"
                    .to_string(),
        },
        None,
    )
}

fn evidence_worklist_action(
    id: &str,
    title: &str,
    filter: &str,
    reason: &str,
) -> CoreCommandActionV1 {
    CoreCommandActionV1::command(
        id,
        title,
        "cargo-allow",
        vec![
            "worklist".to_string(),
            filter.to_string(),
            "--format".to_string(),
            "json".to_string(),
        ],
    )
    .with_contract(
        reason,
        "the affected typed work items and their detailed actions are emitted",
        "the worklist is guidance and does not mutate source or policy",
    )
}

pub fn core_command_summary_from_error(
    tool_version: impl Into<String>,
    operation: impl Into<String>,
    subject: CoreSourceSubjectV1,
    error: &CargoAllowError,
    operation_effects: CoreCommandEffectsV1,
    primary_action: Option<CoreCommandActionV1>,
    claim_boundary: ClaimBoundaryV1,
) -> Result<CoreCommandSummaryV1, String> {
    let (result_class, completeness, currentness) = error_result(error.kind());
    build_core_command_summary(CoreCommandSummaryInputV1 {
        tool_version: tool_version.into(),
        operation: operation.into(),
        mode: None,
        profile: None,
        subject,
        result_class,
        posture: CoreCommandPostureV1::Blocking,
        completeness,
        currentness,
        reason: CoreCommandReasonV1 {
            code: error.code().to_string(),
            message: error.message().to_string(),
        },
        primary_action,
        additional_action_count: 0,
        additional_actions_ref: None,
        operation_effects,
        next_proof: None,
        artifacts: Vec::new(),
        claim_boundary,
    })
}

fn error_result(kind: CargoAllowErrorKind) -> (ResultClassV1, CompletenessV1, CurrentnessV1) {
    match kind {
        CargoAllowErrorKind::Usage
        | CargoAllowErrorKind::InvalidConfig
        | CargoAllowErrorKind::InvalidPolicy => (
            ResultClassV1::MalformedInput,
            CompletenessV1::Unknown,
            CurrentnessV1::NotProbed,
        ),
        CargoAllowErrorKind::PolicyViolation => (
            ResultClassV1::Findings,
            CompletenessV1::Complete,
            CurrentnessV1::Current,
        ),
        CargoAllowErrorKind::Unsupported => (
            ResultClassV1::Unsupported,
            CompletenessV1::Unknown,
            CurrentnessV1::NotProbed,
        ),
        _ => (
            ResultClassV1::InstrumentFailure,
            CompletenessV1::Unknown,
            CurrentnessV1::PartialOrUnavailable,
        ),
    }
}
