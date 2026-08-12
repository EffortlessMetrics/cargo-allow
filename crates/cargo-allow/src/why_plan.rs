use std::path::Path;

use allow_core::{
    AllowConfig, CargoAllowError, CargoAllowErrorKind, CargoAllowResult, Finding, MatchOutcome,
    MatchStatus,
};
use allow_report::{
    AddFindingPlanCandidate, AddFindingPlanFinding, AddFindingPlanOutcome, AddFindingPlanPolicy,
    AddFindingPlanProofPlan, AddFindingPlanRepository, AddFindingPlanV1, EvaluationContext,
};

use super::why_render::{WhyCandidate, why_next_steps};
use crate::SourceTreeReportContext;
use crate::plan_bindings::compute_plan_finding_bindings;

pub(super) struct AddFindingPlanInput<'a> {
    pub root: &'a Path,
    pub config: Option<&'a Path>,
    pub cfg: &'a AllowConfig,
    pub include_untracked: bool,
    pub source_context: &'a SourceTreeReportContext,
    pub evaluation: EvaluationContext<'a>,
    pub scanner_completeness: Option<&'a str>,
    pub finding: &'a Finding,
    pub outcome: &'a MatchOutcome,
    pub candidates: &'a [WhyCandidate<'a>],
}

pub(super) fn render_add_finding_plan(input: AddFindingPlanInput<'_>) -> CargoAllowResult<String> {
    let AddFindingPlanInput {
        root,
        config,
        cfg,
        include_untracked,
        source_context,
        evaluation,
        scanner_completeness,
        finding,
        outcome,
        candidates,
    } = input;
    ensure_plan_outcome_is_new(outcome.status)?;
    let inventory = source_context.inventory();
    ensure_exact_plan_evaluation(evaluation, inventory, scanner_completeness)?;

    let bindings = compute_plan_finding_bindings(root, config, cfg, include_untracked, finding)?;
    let root_text = source_context.source_tree_root().to_string();

    let plan = AddFindingPlanV1 {
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        repository: AddFindingPlanRepository {
            identity: bindings.repository_identity,
            root: root_text.clone(),
        },
        inventory,
        evaluation,
        inventory_basis_identity: bindings.inventory_basis_identity,
        policy: AddFindingPlanPolicy {
            path: bindings.policy_path.clone(),
            digest: bindings.policy_digest,
        },
        finding: AddFindingPlanFinding {
            kind: bindings.finding_kind,
            family: bindings.finding_family,
            path: bindings.finding_path,
            line: bindings.finding_line,
            column: bindings.finding_column,
            identity: bindings.finding_identity,
            digest: bindings.finding_digest,
            source_file_digest: bindings.source_file_digest,
            selector: bindings.selector,
        },
        outcome: AddFindingPlanOutcome {
            status: outcome.status.as_str().to_string(),
            allow_id: outcome.allow_id.clone(),
            message: outcome.message.clone(),
        },
        candidates: candidates
            .iter()
            .map(|candidate| AddFindingPlanCandidate {
                allow_id: candidate.entry.id.clone(),
                mismatch_reasons: candidate.reasons.clone(),
            })
            .collect(),
        required_fields: required_human_fields(cfg, finding),
        proof_plans: scoped_proof_plans(
            finding,
            outcome,
            candidates,
            &root_text,
            &bindings.policy_path,
            include_untracked,
        ),
    };
    Ok(
        allow_report::render_add_finding_plan_json_with_result_class(
            &plan,
            evaluation.result_class_kind_with_scanner_completeness(inventory, scanner_completeness),
            scanner_completeness,
        ),
    )
}

fn ensure_plan_outcome_is_new(status: MatchStatus) -> CargoAllowResult<()> {
    if status == MatchStatus::New {
        return Ok(());
    }
    Err(CargoAllowError::with_kind(
        CargoAllowErrorKind::Usage,
        format!(
            "cannot produce an add-finding plan for status `{}`; use ordinary `why --format json` or `explain` for diagnosis",
            status.as_str()
        ),
    ))
}

fn ensure_exact_plan_evaluation(
    evaluation: EvaluationContext<'_>,
    inventory: allow_report::InventoryContext<'_>,
    scanner_completeness: Option<&str>,
) -> CargoAllowResult<()> {
    if matches!(
        evaluation.result_class_with_scanner_completeness(inventory, scanner_completeness),
        Some("exact_scoped" | "exact_after_full_fallback")
    ) {
        Ok(())
    } else {
        Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            "cannot produce an add-finding plan from a non-exact evaluation; re-run why after the scanner or full-fallback inventory is complete",
        ))
    }
}

fn scoped_proof_plans(
    finding: &Finding,
    outcome: &MatchOutcome,
    candidates: &[WhyCandidate<'_>],
    root: &str,
    policy_path: &str,
    include_untracked: bool,
) -> Vec<AddFindingPlanProofPlan> {
    why_next_steps(finding, outcome, candidates)
        .proof_plans
        .into_iter()
        .map(|mut proof| {
            proof.args.extend([
                "--root".to_string(),
                root.to_string(),
                "--config".to_string(),
                policy_path.to_string(),
            ]);
            if include_untracked {
                proof.args.push("--include-untracked".to_string());
            }
            AddFindingPlanProofPlan {
                program: proof.program,
                args: proof.args,
            }
        })
        .collect()
}

fn required_human_fields(cfg: &AllowConfig, finding: &Finding) -> Vec<String> {
    let requirements = &cfg.requirements;
    let mut fields = Vec::new();
    if requirements.owner_required {
        fields.push("owner".to_string());
    }
    if requirements.classification_required {
        fields.push("classification".to_string());
    }
    if requirements.reason_required {
        fields.push("reason".to_string());
    }
    if requirements.evidence_required
        || (finding.kind == allow_core::FindingKind::Unsafe
            && (requirements.unsafe_evidence_required
                || requirements.unsafe_verified_evidence_required))
    {
        fields.push("evidence".to_string());
    }
    if requirements.expires_or_review_after_required {
        fields.push("review_after_or_expires".to_string());
    }
    fields
}

#[cfg(test)]
pub(crate) fn sample_add_finding_plan_json_for_contract_test() -> String {
    use allow_core::{FindingKind, Span, StructuralIdentity, normalize_path};

    use crate::plan_bindings::{identity_values, selector_values};
    use crate::selector::selector_from_finding;

    let mut structural = StructuralIdentity::new("rust", "method_call");
    structural.container = Some("load".to_string());
    structural.callee = Some("unwrap".to_string());
    structural.line_hint = Some(10);
    structural.column_hint = Some(5);
    let finding = Finding {
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: "src/lib.rs".into(),
        span: Some(Span {
            line: 10,
            column: 5,
        }),
        identity: structural,
        message: "unwrap call".to_string(),
        ledger: None,
    };
    let digest = "sha256:v1:0000000000000000000000000000000000000000000000000000000000000000";
    let reasons = vec!["fixture fallback reason".to_string()];
    let plan = AddFindingPlanV1 {
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        repository: AddFindingPlanRepository {
            identity: digest.to_string(),
            root: "H:/repo".to_string(),
        },
        inventory: allow_report::InventoryContext::source_syntax(
            "git_tracked",
            Some("H:/repo"),
            Some(3),
        )
        .with_completeness("complete"),
        evaluation: EvaluationContext {
            scope: "full_fallback",
            locality: "global_dependency",
            reasons: &reasons,
        },
        inventory_basis_identity: digest.to_string(),
        policy: AddFindingPlanPolicy {
            path: "policy/allow.toml".to_string(),
            digest: digest.to_string(),
        },
        finding: AddFindingPlanFinding {
            kind: finding.kind.as_str().to_string(),
            family: finding.family.clone(),
            path: normalize_path(&finding.path),
            line: Some(10),
            column: Some(5),
            identity: identity_values(&finding),
            digest: digest.to_string(),
            source_file_digest: digest.to_string(),
            selector: selector_values(&selector_from_finding(&finding)),
        },
        outcome: AddFindingPlanOutcome {
            status: "new".to_string(),
            allow_id: None,
            message: "unreceipted panic.unwrap".to_string(),
        },
        candidates: vec![AddFindingPlanCandidate {
            allow_id: "allow-near".to_string(),
            mismatch_reasons: vec!["callee mismatch".to_string()],
        }],
        required_fields: vec!["owner".to_string(), "reason".to_string()],
        proof_plans: vec![AddFindingPlanProofPlan {
            program: "cargo-allow".to_string(),
            args: vec![
                "check".to_string(),
                "--mode".to_string(),
                "no-new".to_string(),
            ],
        }],
    };
    allow_report::render_add_finding_plan_json(&plan)
}

#[cfg(test)]
mod tests {
    use super::{ensure_exact_plan_evaluation, ensure_plan_outcome_is_new, required_human_fields};
    use allow_core::{
        AllowConfig, CargoAllowErrorKind, Finding, FindingKind, MatchStatus, StructuralIdentity,
    };
    use allow_report::{EvaluationContext, InventoryContext};

    #[test]
    fn add_finding_plan_rejects_non_new_outcomes_as_usage() {
        let error = ensure_plan_outcome_is_new(MatchStatus::Matched)
            .expect_err("a matched finding cannot produce an add-finding plan");

        assert_eq!(error.kind(), CargoAllowErrorKind::Usage);
        assert!(error.to_string().contains("status `matched`"));
    }

    #[test]
    fn add_finding_plan_requires_an_exact_result_class() {
        let reasons = vec!["repository-wide policy scope".to_string()];
        let cases = [
            (
                EvaluationContext {
                    scope: "scoped",
                    locality: "proven",
                    reasons: &[],
                },
                "partial",
                false,
            ),
            (
                EvaluationContext {
                    scope: "full_fallback",
                    locality: "global_dependency",
                    reasons: &reasons,
                },
                "fallback",
                false,
            ),
            (
                EvaluationContext {
                    scope: "scoped",
                    locality: "proven",
                    reasons: &[],
                },
                "complete",
                true,
            ),
        ];

        for (evaluation, completeness, expected_ok) in cases {
            let result = ensure_exact_plan_evaluation(
                evaluation,
                InventoryContext::source_syntax("git_tracked", Some("H:/repo"), Some(3))
                    .with_completeness(completeness),
                None,
            );
            if expected_ok {
                assert!(result.is_ok(), "completeness={completeness}");
            } else {
                let error = result.expect_err("non-exact evaluation should be rejected");
                assert_eq!(error.kind(), CargoAllowErrorKind::Usage);
            }
        }

        let result = ensure_exact_plan_evaluation(
            EvaluationContext {
                scope: "scoped",
                locality: "proven",
                reasons: &[],
            },
            InventoryContext::source_syntax("git_tracked", Some("H:/repo"), Some(3))
                .with_completeness("partial"),
            Some("complete"),
        );
        assert!(
            result.is_ok(),
            "a complete target scan may produce a scoped plan despite unrelated inventory partiality"
        );
    }

    #[test]
    fn verified_unsafe_evidence_is_a_required_human_field() {
        let mut cfg = AllowConfig::empty();
        cfg.requirements.evidence_required = false;
        cfg.requirements.unsafe_evidence_required = false;
        cfg.requirements.unsafe_verified_evidence_required = true;
        let finding = Finding {
            kind: FindingKind::Unsafe,
            family: Some("unsafe_block".to_string()),
            path: "src/lib.rs".into(),
            span: None,
            identity: StructuralIdentity::new("rust", "unsafe_block"),
            message: "unsafe block".to_string(),
            ledger: None,
        };

        assert!(
            required_human_fields(&cfg, &finding).contains(&"evidence".to_string()),
            "an add-finding plan must request evidence that can satisfy the verified mandate"
        );
    }
}
