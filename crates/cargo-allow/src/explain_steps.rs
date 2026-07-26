use allow_core::{AllowEntry, Finding, FindingKind, MatchOutcome, MatchStatus, SimpleDate};

use crate::worklist;

const EXPLAIN_PROOF_COMMAND_LIMIT: usize = 8;

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct ExplainReferenceAttention {
    pub(super) has_broken_evidence: bool,
    pub(super) has_weak_evidence: bool,
    pub(super) has_evidence_outside_default_inventory: bool,
    pub(super) has_broken_link: bool,
    pub(super) has_weak_link: bool,
    pub(super) has_link_outside_default_inventory: bool,
}

pub(super) fn explain_next_steps(
    entry: &AllowEntry,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    references: ExplainReferenceAttention,
) -> (Vec<String>, Vec<String>) {
    let attention = outcomes
        .iter()
        .filter(|outcome| outcome.status != MatchStatus::Matched)
        .collect::<Vec<_>>();
    if let Some(outcome) = attention.first() {
        let finding = outcome.finding_index.and_then(|index| findings.get(index));
        let kind = worklist::work_item_kind(outcome, finding, Some(entry));
        return (
            worklist::suggested_actions_for_context(&kind, finding, Some(entry))
                .into_iter()
                .take(2)
                .collect(),
            worklist::proof_commands(&kind, finding, Some(entry))
                .into_iter()
                .take(EXPLAIN_PROOF_COMMAND_LIMIT)
                .collect(),
        );
    }
    if references.has_evidence_outside_default_inventory {
        let finding = findings.first();
        let kind = "broken_evidence_link";
        return (
            vec![
                "commit the referenced evidence file if it should support repository policy"
                    .to_string(),
                "or rerun with --include-untracked when intentionally reviewing local receipt artifacts"
                    .to_string(),
            ],
            untracked_proof_commands(entry, finding, kind),
        );
    }
    if references.has_link_outside_default_inventory {
        let finding = findings.first();
        let kind = "broken_evidence_link";
        return (
            vec![
                "commit the referenced traceability file if it should support repository policy"
                    .to_string(),
                "or rerun with --include-untracked when intentionally reviewing local traceability files"
                    .to_string(),
            ],
            untracked_proof_commands(entry, finding, kind),
        );
    }
    if references.has_broken_evidence {
        let finding = findings.first();
        let kind = "broken_evidence_link";
        return (
            worklist::suggested_actions(kind)
                .into_iter()
                .take(2)
                .collect(),
            worklist::proof_commands(kind, finding, Some(entry))
                .into_iter()
                .take(EXPLAIN_PROOF_COMMAND_LIMIT)
                .collect(),
        );
    }
    if references.has_broken_link {
        let finding = findings.first();
        let kind = "broken_evidence_link";
        return (
            worklist::suggested_link_actions_for_context(kind, finding, Some(entry))
                .into_iter()
                .take(2)
                .collect(),
            worklist::proof_commands(kind, finding, Some(entry))
                .into_iter()
                .take(EXPLAIN_PROOF_COMMAND_LIMIT)
                .collect(),
        );
    }
    if references.has_weak_evidence {
        let finding = findings.first();
        let kind = "weak_evidence_reference";
        return (
            worklist::suggested_actions_for_context(kind, finding, Some(entry))
                .into_iter()
                .take(2)
                .collect(),
            worklist::proof_commands(kind, finding, Some(entry))
                .into_iter()
                .take(EXPLAIN_PROOF_COMMAND_LIMIT)
                .collect(),
        );
    }
    if references.has_weak_link {
        let finding = findings.first();
        let kind = "weak_evidence_reference";
        return (
            worklist::suggested_link_actions_for_context(kind, finding, Some(entry))
                .into_iter()
                .take(2)
                .collect(),
            worklist::proof_commands(kind, finding, Some(entry))
                .into_iter()
                .take(EXPLAIN_PROOF_COMMAND_LIMIT)
                .collect(),
        );
    }
    // Lifecycle-driven next steps: an entry with expired or review-due dates
    // needs operator action even when findings still match and evidence is
    // present. Without this branch, explain would return empty next-steps for
    // exactly the entries that most need remediation guidance (#2817).
    let today = SimpleDate::today_utc_approx();
    let is_expired = entry
        .lifecycle
        .expires
        .as_deref()
        .filter(|&expires| expires != "never")
        .and_then(SimpleDate::parse)
        .is_some_and(|date| date < today);
    let is_review_due = entry
        .lifecycle
        .review_after
        .as_deref()
        .and_then(SimpleDate::parse)
        .is_some_and(|date| date <= today);
    if is_expired || is_review_due {
        let finding = findings.first();
        let kind = if is_expired {
            "expired_allow"
        } else {
            "review_due"
        };
        return (
            worklist::suggested_actions_for_context(kind, finding, Some(entry))
                .into_iter()
                .take(2)
                .collect(),
            worklist::proof_commands(kind, finding, Some(entry))
                .into_iter()
                .take(EXPLAIN_PROOF_COMMAND_LIMIT)
                .collect(),
        );
    }
    if entry.classification == "baseline_debt" {
        let finding = findings.first();
        let kind = "baseline_debt";
        return (
            worklist::suggested_actions_for_context(kind, finding, Some(entry))
                .into_iter()
                .take(2)
                .collect(),
            worklist::proof_commands(kind, finding, Some(entry))
                .into_iter()
                .take(EXPLAIN_PROOF_COMMAND_LIMIT)
                .collect(),
        );
    }
    if entry.evidence.is_empty() {
        let finding = findings.first();
        let kind = if entry.kind == FindingKind::Unsafe {
            "unsafe_missing_evidence"
        } else {
            "missing_evidence"
        };
        return (
            worklist::suggested_actions_for_context(kind, finding, Some(entry))
                .into_iter()
                .take(2)
                .collect(),
            worklist::proof_commands(kind, finding, Some(entry))
                .into_iter()
                .take(EXPLAIN_PROOF_COMMAND_LIMIT)
                .collect(),
        );
    }
    (Vec::new(), Vec::new())
}

fn untracked_proof_commands(
    entry: &AllowEntry,
    finding: Option<&Finding>,
    kind: &str,
) -> Vec<String> {
    let mut commands = vec![
        format!("cargo-allow explain {}", entry.id),
        format!("cargo-allow explain {} --include-untracked", entry.id),
        format!("cargo-allow worklist --allow-id {} --format json", entry.id),
        "cargo-allow check --include-untracked --mode no-new".to_string(),
    ];
    for command in worklist::proof_commands(kind, finding, Some(entry)) {
        if !commands.iter().any(|existing| existing == &command) {
            commands.push(command);
        }
    }
    commands
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{test_entry, test_finding};
    use super::*;
    use allow_core::{FindingKind, MatchOutcome, MatchStatus};

    #[test]
    fn explain_next_steps_call_presence_observer() {
        let mut entry = test_entry("allow-explain-steps", FindingKind::Panic);
        entry.evidence = vec!["test:existing-proof".to_string()];
        let finding = test_finding(
            FindingKind::Panic,
            Some("unwrap"),
            "src/lib.rs",
            "method_call",
        );
        let outcome = MatchOutcome {
            status: MatchStatus::New,
            allow_id: None,
            candidate_ids: Vec::new(),
            finding_index: Some(0),
            message: "new finding".to_string(),
            score: 90,
        };

        let (actions, proofs) = explain_next_steps(
            &entry,
            std::slice::from_ref(&finding),
            &[outcome],
            ExplainReferenceAttention::default(),
        );
        assert_eq!(
            actions,
            vec![
                "remove the new source exception if it is accidental".to_string(),
                "or add a reviewed allow entry with owner, reason, scope, evidence, and lifecycle"
                    .to_string(),
            ]
        );
        assert!(proofs.contains(&"cargo-allow explain allow-explain-steps".to_string()));

        assert_reference_route(
            ExplainReferenceAttention {
                has_evidence_outside_default_inventory: true,
                ..ExplainReferenceAttention::default()
            },
            "commit the referenced evidence file if it should support repository policy",
            "cargo-allow explain allow-explain-steps --include-untracked",
        );
        assert_reference_route(
            ExplainReferenceAttention {
                has_link_outside_default_inventory: true,
                ..ExplainReferenceAttention::default()
            },
            "commit the referenced traceability file if it should support repository policy",
            "cargo-allow explain allow-explain-steps --include-untracked",
        );
        assert_reference_route(
            ExplainReferenceAttention {
                has_broken_evidence: true,
                ..ExplainReferenceAttention::default()
            },
            "restore or commit the referenced local evidence artifact",
            "cargo-allow explain allow-explain-steps",
        );
        assert_reference_route(
            ExplainReferenceAttention {
                has_broken_link: true,
                ..ExplainReferenceAttention::default()
            },
            "restore or commit the referenced local traceability file",
            "cargo-allow explain allow-explain-steps",
        );
        assert_reference_route(
            ExplainReferenceAttention {
                has_weak_evidence: true,
                ..ExplainReferenceAttention::default()
            },
            "replace the weak evidence string with a typed evidence reference",
            "cargo-allow explain allow-explain-steps",
        );
        assert_reference_route(
            ExplainReferenceAttention {
                has_weak_link: true,
                ..ExplainReferenceAttention::default()
            },
            "replace the weak link string with a typed traceability reference",
            "cargo-allow explain allow-explain-steps",
        );

        let mut baseline_entry = test_entry("allow-baseline", FindingKind::Panic);
        baseline_entry.classification = "baseline_debt".to_string();
        let (actions, proofs) = explain_next_steps(
            &baseline_entry,
            &[],
            &[],
            ExplainReferenceAttention::default(),
        );
        assert_eq!(
            actions.first().map(String::as_str),
            Some("replace generated baseline debt with a reviewed allow entry")
        );
        assert!(proofs.contains(&"cargo-allow explain allow-baseline".to_string()));

        let missing_evidence_entry = test_entry("allow-missing-evidence", FindingKind::Unsafe);
        let (actions, proofs) = explain_next_steps(
            &missing_evidence_entry,
            &[],
            &[],
            ExplainReferenceAttention::default(),
        );
        assert_eq!(
            actions.first().map(String::as_str),
            Some("add unsafe-review, test, spec, or boundary evidence for the unsafe exception")
        );
        assert!(proofs.contains(&"cargo-allow explain allow-missing-evidence".to_string()));
    }

    #[test]
    fn explain_next_steps_for_expired_entry_with_evidence() {
        let mut entry = test_entry("allow-expired", FindingKind::Panic);
        entry.evidence = vec!["test:existing-proof".to_string()];
        entry.lifecycle.expires = Some("2020-01-01".to_string());

        let (actions, proofs) =
            explain_next_steps(&entry, &[], &[], ExplainReferenceAttention::default());

        assert!(
            !actions.is_empty(),
            "expired entry with evidence should still get next steps"
        );
        assert!(
            actions
                .iter()
                .any(|action| action.contains("expired") || action.contains("remove")),
            "expired entry should suggest removal or renewal: {actions:?}"
        );
        assert!(proofs.contains(&"cargo-allow explain allow-expired".to_string()));
    }

    #[test]
    fn explain_next_steps_for_review_due_entry_with_evidence() {
        let mut entry = test_entry("allow-review-due", FindingKind::Panic);
        entry.evidence = vec!["test:existing-proof".to_string()];
        entry.lifecycle.review_after = Some("2020-01-01".to_string());

        let (actions, proofs) =
            explain_next_steps(&entry, &[], &[], ExplainReferenceAttention::default());

        assert!(
            !actions.is_empty(),
            "review-due entry with evidence should still get next steps"
        );
        assert!(
            actions
                .iter()
                .any(|action| action.contains("review") || action.contains("update")),
            "review-due entry should suggest review: {actions:?}"
        );
        assert!(proofs.contains(&"cargo-allow explain allow-review-due".to_string()));
    }

    #[test]
    fn explain_next_steps_skips_lifecycle_when_not_due() {
        let mut entry = test_entry("allow-future", FindingKind::Panic);
        entry.evidence = vec!["test:existing-proof".to_string()];
        entry.lifecycle.expires = Some("2099-12-31".to_string());
        entry.lifecycle.review_after = Some("2099-12-31".to_string());

        let (actions, _proofs) =
            explain_next_steps(&entry, &[], &[], ExplainReferenceAttention::default());

        assert!(
            actions.is_empty(),
            "entry with future dates and evidence should get no next steps"
        );
    }

    #[test]
    fn untracked_proof_commands_call_presence_observer() {
        let entry = test_entry("allow-untracked", FindingKind::NonRustFile);
        let finding = test_finding(
            FindingKind::NonRustFile,
            None,
            "tracked.file",
            "tracked_file",
        );

        let commands = untracked_proof_commands(&entry, Some(&finding), "broken_evidence_link");

        assert_eq!(
            commands[..4],
            [
                "cargo-allow explain allow-untracked".to_string(),
                "cargo-allow explain allow-untracked --include-untracked".to_string(),
                "cargo-allow worklist --allow-id allow-untracked --format json".to_string(),
                "cargo-allow check --include-untracked --mode no-new".to_string(),
            ]
        );
        assert!(
            commands
                .contains(&"cargo-allow list --allow-id allow-untracked --format json".to_string())
        );
        assert_eq!(
            commands
                .iter()
                .filter(|command| command.as_str() == "cargo-allow explain allow-untracked")
                .count(),
            1
        );
    }

    fn assert_reference_route(
        references: ExplainReferenceAttention,
        expected_action: &str,
        expected_proof_command: &str,
    ) {
        let mut entry = test_entry("allow-explain-steps", FindingKind::Panic);
        entry.evidence = vec!["test:existing-proof".to_string()];
        let finding = test_finding(
            FindingKind::Panic,
            Some("unwrap"),
            "src/lib.rs",
            "method_call",
        );

        let (actions, proofs) = explain_next_steps(&entry, &[finding], &[], references);

        assert_eq!(actions.first().map(String::as_str), Some(expected_action));
        assert!(proofs.contains(&expected_proof_command.to_string()));
    }
}
