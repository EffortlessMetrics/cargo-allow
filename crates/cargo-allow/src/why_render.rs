use allow_core::{AllowEntry, Finding, MatchOutcome, MatchStatus, normalize_path};
use allow_match::finding_location;
use allow_report::EvaluationContext;

use super::why_shell::{ProofPlan, render_proof_command};

pub(super) struct WhyCandidate<'a> {
    pub entry: &'a AllowEntry,
    pub reasons: Vec<String>,
}

pub(super) struct WhyNextSteps {
    pub suggested_actions: Vec<String>,
    pub proof_plans: Vec<ProofPlan>,
}

impl WhyNextSteps {
    pub(super) fn proof_commands(&self) -> Vec<String> {
        self.proof_plans.iter().map(render_proof_command).collect()
    }
}

pub(super) fn why_next_steps(
    finding: &Finding,
    outcome: &MatchOutcome,
    candidates: &[WhyCandidate<'_>],
) -> WhyNextSteps {
    match outcome.status {
        MatchStatus::New => {
            let mut suggested_actions = vec![
                "Receipt this occurrence with cargo-allow add.".to_string(),
                "Or repair the source so the finding disappears, then re-run cargo-allow check --mode no-new."
                    .to_string(),
            ];
            let mut proof_plans = vec![add_receipt_plan(finding), check_no_new_plan()];
            if let Some(first) = candidates.first() {
                suggested_actions.push(format!(
                    "Inspect near-miss allow entry `{}` with cargo-allow explain.",
                    first.entry.id
                ));
                proof_plans.push(explain_plan(&first.entry.id));
            }
            WhyNextSteps {
                suggested_actions,
                proof_plans,
            }
        }
        MatchStatus::Ambiguous => {
            let mut suggested_actions = Vec::new();
            let mut proof_plans = Vec::new();
            if outcome.candidate_ids.is_empty() {
                suggested_actions.push(
                    "Multiple allow entries compete, but no structured candidate IDs were recorded. \
Inspect the ledger with cargo-allow list or cargo-allow check."
                        .to_string(),
                );
            } else {
                suggested_actions.push(
                    "Multiple allow entries compete for this finding. \
Inspect every candidate with cargo-allow explain; do not treat any one as authoritative."
                        .to_string(),
                );
                for id in &outcome.candidate_ids {
                    suggested_actions.push(format!("Inspect candidate allow entry `{id}`."));
                    proof_plans.push(explain_plan(id));
                }
            }
            proof_plans.push(check_no_new_plan());
            proof_plans.push(ProofPlan::cargo_allow(["list"]));
            WhyNextSteps {
                suggested_actions,
                proof_plans,
            }
        }
        _ => {
            let mut suggested_actions = Vec::new();
            let mut proof_plans = Vec::new();
            if let Some(id) = &outcome.allow_id {
                suggested_actions.push(format!(
                    "Inspect the linked allow entry `{id}` with cargo-allow explain."
                ));
                proof_plans.push(explain_plan(id));
            } else {
                suggested_actions.push(
                    "Inspect the broader ledger with cargo-allow list or cargo-allow check."
                        .to_string(),
                );
            }
            proof_plans.push(check_no_new_plan());
            proof_plans.push(ProofPlan::cargo_allow(["list"]));
            WhyNextSteps {
                suggested_actions,
                proof_plans,
            }
        }
    }
}

fn add_receipt_plan(finding: &Finding) -> ProofPlan {
    let path = normalize_path(&finding.path);
    let line = finding
        .span
        .as_ref()
        .map(|span| span.line)
        .unwrap_or(1)
        .to_string();
    ProofPlan::cargo_allow([
        "add".to_string(),
        "--kind".to_string(),
        finding.kind.as_str().to_string(),
        "--path".to_string(),
        path,
        "--line".to_string(),
        line,
        "--owner".to_string(),
        "<owner>".to_string(),
        "--reason".to_string(),
        "...".to_string(),
        "--evidence".to_string(),
        "<ref>".to_string(),
        "--update".to_string(),
    ])
}

fn explain_plan(id: &str) -> ProofPlan {
    ProofPlan::cargo_allow(["explain".to_string(), id.to_string()])
}

fn check_no_new_plan() -> ProofPlan {
    ProofPlan::cargo_allow([
        "check".to_string(),
        "--mode".to_string(),
        "no-new".to_string(),
    ])
}

#[cfg(test)]
pub(super) fn render_why_text(
    finding: &Finding,
    outcome: &MatchOutcome,
    candidates: &[WhyCandidate<'_>],
) -> String {
    render_why_text_styled(finding, outcome, candidates, allow_report::Style::PLAIN)
}

#[cfg(test)]
pub(super) fn render_why_text_styled(
    finding: &Finding,
    outcome: &MatchOutcome,
    candidates: &[WhyCandidate<'_>],
    style: allow_report::Style,
) -> String {
    render_why_text_styled_with_evaluation_and_scanner_completeness(
        allow_report::InventoryContext::source_syntax("git_tracked", None, None)
            .with_completeness("complete"),
        finding,
        outcome,
        candidates,
        style,
        EvaluationContext {
            scope: "scoped",
            locality: "proven",
            reasons: &[],
        },
        None,
    )
}

pub(super) fn render_why_text_styled_with_evaluation_and_scanner_completeness(
    inventory: allow_report::InventoryContext<'_>,
    finding: &Finding,
    outcome: &MatchOutcome,
    candidates: &[WhyCandidate<'_>],
    style: allow_report::Style,
    evaluation: EvaluationContext<'_>,
    scanner_completeness: Option<&str>,
) -> String {
    let next = why_next_steps(finding, outcome, candidates);
    let proof_commands = next.proof_commands();
    let mut out = String::new();
    out.push_str("# Why this finding is unreceipted\n\n");
    out.push_str("## Finding\n\n");
    out.push_str(&format!("- location: {}\n", finding_location(finding)));
    out.push_str(&format!("- kind: {}\n", finding.kind.as_str()));
    match &finding.family {
        Some(family) => out.push_str(&format!("- family: {family}\n")),
        None => out.push_str("- family: <none>\n"),
    }
    out.push_str(&format!("- message: {}\n", finding.message));
    out.push_str(&format!(
        "- identity.ast_kind: {}\n",
        finding.identity.ast_kind
    ));
    if let Some(container) = &finding.identity.container {
        out.push_str(&format!("- identity.container: {container}\n"));
    }
    if let Some(callee) = &finding.identity.callee {
        out.push_str(&format!("- identity.callee: {callee}\n"));
    }
    if let Some(hash) = &finding.identity.normalized_snippet_hash {
        out.push_str(&format!("- identity.normalized_snippet_hash: {hash}\n"));
    }
    out.push('\n');

    out.push_str("## Evaluation scope\n\n");
    if let Some(result_class) =
        evaluation.result_class_with_scanner_completeness(inventory, scanner_completeness)
    {
        out.push_str(&format!("- result_class: {result_class}\n"));
    }
    out.push_str(&format!("- scope: {}\n", evaluation.scope));
    out.push_str(&format!("- locality: {}\n", evaluation.locality));
    if evaluation.reasons.is_empty() {
        out.push_str("- locality reasons: none\n\n");
    } else {
        out.push_str("- locality reasons:\n");
        for reason in evaluation.reasons {
            out.push_str(&format!("  - {reason}\n"));
        }
        out.push('\n');
    }

    out.push_str("## Current posture\n\n");
    out.push_str(&format!(
        "- status: {}\n",
        style.status(outcome.status.as_str(), outcome.status.as_str())
    ));
    match &outcome.allow_id {
        Some(id) => out.push_str(&format!("- allow_id: {id}\n")),
        None => out.push_str("- allow_id: <none>\n"),
    }
    if !outcome.candidate_ids.is_empty() {
        out.push_str(&format!(
            "- candidate_ids: {}\n",
            outcome.candidate_ids.join(", ")
        ));
    }
    if !outcome.message.is_empty() {
        out.push_str(&format!("- message: {}\n", outcome.message));
    }
    out.push('\n');

    match outcome.status {
        MatchStatus::New => {
            out.push_str("## Candidate allow entries\n\n");
            if candidates.is_empty() {
                out.push_str(
                    "No same-kind allow entries look related to this finding. \
The finding is unreceipted because no policy entry covers it.\n\n",
                );
            } else {
                out.push_str(
                    "Nearby same-kind entries that do not match, with selector mismatch reasons:\n\n",
                );
                for candidate in candidates {
                    out.push_str(&format!("### `{}`\n\n", candidate.entry.id));
                    out.push_str(&format!(
                        "- kind/family: {} / {}\n",
                        candidate.entry.kind.as_str(),
                        candidate.entry.family.as_deref().unwrap_or("<none>")
                    ));
                    out.push_str(&format!(
                        "- path: {}\n",
                        candidate
                            .entry
                            .path
                            .as_ref()
                            .map(normalize_path)
                            .unwrap_or_else(|| "<none>".to_string())
                    ));
                    if let Some(glob) = &candidate.entry.glob {
                        out.push_str(&format!("- glob: {glob}\n"));
                    }
                    if let Some(glob) = &candidate.entry.selector.glob {
                        out.push_str(&format!("- selector.glob: {glob}\n"));
                    }
                    out.push_str("- mismatch reasons:\n");
                    for reason in &candidate.reasons {
                        out.push_str(&format!("  - {reason}\n"));
                    }
                    out.push('\n');
                }
            }
        }
        MatchStatus::Matched
        | MatchStatus::LocationDrift
        | MatchStatus::ReviewDue
        | MatchStatus::BaselineDebt
        | MatchStatus::EvidenceMissing => {
            out.push_str("## Already receipted\n\n");
            if let Some(id) = &outcome.allow_id {
                out.push_str(&format!(
                    "This finding is already linked to `{id}`. \
Use `cargo-allow explain {id}` for the entry-facing inverse view.\n\n"
                ));
            } else {
                out.push_str(
                    "This finding is not in a plain `new` posture. \
Use `cargo-allow list` or `cargo-allow check` for the broader ledger view.\n\n",
                );
            }
        }
        MatchStatus::Ambiguous => {
            out.push_str("## Ambiguous match\n\n");
            out.push_str(
                "Multiple allow entries compete for this finding. \
Inspect every candidate ID with `cargo-allow explain <id>` and narrow selectors; \
no candidate is selected as authoritative.\n\n",
            );
            if !outcome.candidate_ids.is_empty() {
                out.push_str("Candidate-specific explain plans:\n\n");
                for id in &outcome.candidate_ids {
                    out.push_str(&format!("- `{id}`\n"));
                }
                out.push('\n');
            }
        }
        MatchStatus::Stale
        | MatchStatus::Expired
        | MatchStatus::InvalidSelector
        | MatchStatus::MissingRequiredField => {
            out.push_str("## Policy health issue\n\n");
            out.push_str(
                "The selected finding is blocked by a policy-health status rather than a simple unreceipted gap. \
Use `cargo-allow explain <id>` when an allow ID is present, or `cargo-allow check --mode strict` for the full gate view.\n\n",
            );
        }
    }

    out.push_str("## Suggested next steps\n\n");
    for (index, action) in next.suggested_actions.iter().enumerate() {
        out.push_str(&format!("{}. {action}\n", index + 1));
    }
    out.push('\n');
    out.push_str("## Proof commands\n\n");
    out.push_str(
        "Commands below are rendered for the current platform shell when paste-safe. \
Structured argv is authoritative in `cargo-allow.why.v1` `next.proof_plans`.\n\n",
    );
    for command in &proof_commands {
        if command.contains('\n') {
            out.push_str("- \n```\n");
            out.push_str(command);
            out.push_str("\n```\n");
        } else {
            out.push_str(&format!("- `{command}`\n"));
        }
    }
    out.push('\n');

    out.push_str("## Claim boundary\n\n");
    out.push_str(
        "`why` reports source-tree / source-syntax matching posture only. \
It does not prove that an exception is safe, that tests are adequate, or that \
macro-expanded / type-aware behavior would match. Proof commands are guidance only \
and are not executed by cargo-allow.\n",
    );
    out
}

#[cfg(test)]
pub(super) fn render_why_json(
    inventory: allow_report::InventoryContext<'_>,
    finding: &Finding,
    outcome: &MatchOutcome,
    candidates: &[WhyCandidate<'_>],
) -> String {
    let inventory = if inventory.completeness.is_some() {
        inventory
    } else {
        inventory.with_completeness("complete")
    };
    render_why_json_with_evaluation_and_scanner_completeness(
        inventory,
        EvaluationContext {
            scope: "scoped",
            locality: "proven",
            reasons: &[],
        },
        finding,
        outcome,
        candidates,
        None,
    )
}

pub(super) fn render_why_json_with_evaluation_and_scanner_completeness(
    inventory: allow_report::InventoryContext<'_>,
    evaluation: EvaluationContext<'_>,
    finding: &Finding,
    outcome: &MatchOutcome,
    candidates: &[WhyCandidate<'_>],
    scanner_completeness: Option<&str>,
) -> String {
    let next = why_next_steps(finding, outcome, candidates);
    let proof_commands = next.proof_commands();
    let path_texts = candidates
        .iter()
        .map(|candidate| candidate.entry.path.as_ref().map(normalize_path))
        .collect::<Vec<_>>();
    let candidate_entries = candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| allow_report::WhyCandidateEntry {
            id: candidate.entry.id.as_str(),
            kind: candidate.entry.kind.as_str(),
            family: candidate.entry.family.as_deref(),
            path: path_texts.get(index).and_then(|path| path.as_deref()),
            glob: candidate.entry.glob.as_deref(),
            selector_glob: candidate.entry.selector.glob.as_deref(),
            mismatch_reasons: candidate.reasons.as_slice(),
        })
        .collect::<Vec<_>>();
    let proof_plans = next
        .proof_plans
        .iter()
        .map(|plan| allow_report::WhyProofPlan {
            program: plan.program.as_str(),
            args: plan.args.as_slice(),
        })
        .collect::<Vec<_>>();
    let result_class =
        evaluation.result_class_kind_with_scanner_completeness(inventory, scanner_completeness);
    allow_report::render_why_json_with_result_class(
        allow_report::WhyReport {
            inventory,
            evaluation,
            finding,
            outcome,
            candidate_entries: &candidate_entries,
            suggested_actions: &next.suggested_actions,
            proof_commands: &proof_commands,
            proof_plans: &proof_plans,
        },
        result_class,
        scanner_completeness,
    )
}

pub(super) fn render_why_target_scan_json(
    inventory: allow_report::InventoryContext<'_>,
    evaluation: EvaluationContext<'_>,
    path: &str,
    status: &str,
    reason: Option<&str>,
) -> String {
    let suggested_actions = vec![
        format!("Repair or reduce the target so the Rust scanner can inspect `{path}`."),
        "Re-run cargo-allow why after the target scan is complete.".to_string(),
        "No add-finding plan was written because the selected target was not fully scanned."
            .to_string(),
    ];
    allow_report::render_why_target_scan_json(allow_report::WhyTargetScanReport {
        inventory,
        evaluation,
        target: allow_report::WhyTargetScan {
            path,
            status,
            reason,
        },
        suggested_actions: &suggested_actions,
        proof_commands: &[],
        proof_plans: &[],
    })
}

pub(super) fn render_why_target_scan_text(
    evaluation: EvaluationContext<'_>,
    inventory: allow_report::InventoryContext<'_>,
    path: &str,
    status: &str,
    reason: Option<&str>,
) -> String {
    let mut out = String::new();
    out.push_str("# Why the selected target could not be fully scanned\n\n");
    out.push_str("## Target scan\n\n");
    out.push_str(&format!("- path: {path}\n- status: {status}\n"));
    if let Some(reason) = reason {
        out.push_str(&format!("- reason: {reason}\n"));
    }
    out.push_str("\n## Evaluation scope\n\n");
    if let Some(result_class) =
        evaluation.result_class_with_scanner_completeness(inventory, Some("partial"))
    {
        out.push_str(&format!("- result_class: {result_class}\n"));
    }
    out.push_str(&format!("- scope: {}\n", evaluation.scope));
    out.push_str(&format!("- locality: {}\n", evaluation.locality));
    out.push_str("- scanner_completeness: partial\n\n");
    out.push_str("No finding was selected and no add-finding plan was written. Repair the target or reduce its size, then re-run `cargo-allow why`.\n");
    out
}
