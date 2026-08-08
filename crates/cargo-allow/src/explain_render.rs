use super::{
    ExplainContext,
    explain_steps::{ExplainReferenceAttention, explain_next_steps},
};
use crate::evidence_inventory::{
    DEFAULT_SOURCE_TREE_INVENTORY_EVIDENCE_MESSAGE, evidence_reference_diagnostics_for_source_tree,
    policy_reference_diagnostics_for_source_tree,
};
use crate::evidence_render::evidence_reference_target_text;
use allow_core::{AllowEntry, Finding, MatchOutcome, allow_entry_broad_scope};
use allow_diff::selector_precision_score;
use std::collections::BTreeSet;
use std::path::Path;

pub(super) fn render_explain_entry_styled(
    root: &Path,
    entry: &AllowEntry,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    evidence_source_tree_files: Option<&BTreeSet<String>>,
    style: allow_report::Style,
) -> String {
    render_explain_report(
        root,
        entry,
        findings,
        outcomes,
        evidence_source_tree_files,
        ExplainContext::default(),
        None,
        |report| allow_report::render_explain_human_styled(report, style),
    )
}

pub(super) fn explain_reference_attention_for_source_tree(
    root: &Path,
    entry: &AllowEntry,
    evidence_source_tree_files: Option<&BTreeSet<String>>,
) -> ExplainReferenceAttention {
    let evidence_diagnostics =
        evidence_reference_diagnostics_for_source_tree(root, entry, evidence_source_tree_files);
    let link_diagnostics =
        link_reference_diagnostics_for_source_tree(root, entry, evidence_source_tree_files);
    explain_reference_attention(&evidence_diagnostics, &link_diagnostics)
}

pub(super) fn render_explain_entry_styled_with_steps(
    root: &Path,
    entry: &AllowEntry,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    evidence_source_tree_files: Option<&BTreeSet<String>>,
    style: allow_report::Style,
    suggested_actions: &[String],
    proof_commands: &[String],
) -> String {
    render_explain_report(
        root,
        entry,
        findings,
        outcomes,
        evidence_source_tree_files,
        ExplainContext::default(),
        Some((suggested_actions, proof_commands)),
        |report| allow_report::render_explain_human_styled(report, style),
    )
}

pub(super) fn render_explain_entry_json(
    root: &Path,
    entry: &AllowEntry,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    evidence_source_tree_files: Option<&BTreeSet<String>>,
    context: ExplainContext<'_>,
) -> String {
    render_explain_report(
        root,
        entry,
        findings,
        outcomes,
        evidence_source_tree_files,
        context,
        None,
        allow_report::render_explain_json,
    )
}

pub(super) fn render_explain_entry_json_with_steps(
    root: &Path,
    entry: &AllowEntry,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    evidence_source_tree_files: Option<&BTreeSet<String>>,
    context: ExplainContext<'_>,
    suggested_actions: &[String],
    proof_commands: &[String],
) -> String {
    render_explain_report(
        root,
        entry,
        findings,
        outcomes,
        evidence_source_tree_files,
        context,
        Some((suggested_actions, proof_commands)),
        allow_report::render_explain_json,
    )
}

fn render_explain_report<R>(
    root: &Path,
    entry: &AllowEntry,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    evidence_source_tree_files: Option<&BTreeSet<String>>,
    context: ExplainContext<'_>,
    precomputed_steps: Option<(&[String], &[String])>,
    render: impl FnOnce(allow_report::ExplainReport<'_>) -> R,
) -> R {
    let evidence_diagnostics =
        evidence_reference_diagnostics_for_source_tree(root, entry, evidence_source_tree_files);
    let link_diagnostics =
        link_reference_diagnostics_for_source_tree(root, entry, evidence_source_tree_files);
    let (suggested_actions, proof_commands) = match precomputed_steps {
        Some((suggested_actions, proof_commands)) => {
            (suggested_actions.to_vec(), proof_commands.to_vec())
        }
        None => {
            let references = explain_reference_attention(&evidence_diagnostics, &link_diagnostics);
            explain_next_steps(entry, findings, outcomes, references)
        }
    };
    let normalized_targets = evidence_diagnostics
        .iter()
        .map(evidence_reference_target_text)
        .collect::<Vec<_>>();
    let evidence_references = evidence_diagnostics
        .iter()
        .zip(normalized_targets.iter())
        .map(|(diagnostic, target)| allow_report::EvidenceReference {
            raw: &diagnostic.raw,
            prefix: diagnostic.prefix.as_deref(),
            target: target.as_deref(),
            status: diagnostic.status.as_str(),
            category: diagnostic.category.as_str(),
            message: &diagnostic.message,
        })
        .collect::<Vec<_>>();
    let link_normalized_targets = link_diagnostics
        .iter()
        .map(evidence_reference_target_text)
        .collect::<Vec<_>>();
    let link_messages = link_diagnostics
        .iter()
        .map(|diagnostic| link_reference_message(&diagnostic.message))
        .collect::<Vec<_>>();
    let link_references = link_diagnostics
        .iter()
        .zip(link_normalized_targets.iter())
        .zip(link_messages.iter())
        .map(
            |((diagnostic, target), message)| allow_report::EvidenceReference {
                raw: &diagnostic.raw,
                prefix: diagnostic.prefix.as_deref(),
                target: target.as_deref(),
                status: diagnostic.status.as_str(),
                category: diagnostic.category.as_str(),
                message,
            },
        )
        .collect::<Vec<_>>();

    render(allow_report::ExplainReport {
        inventory: context.inventory,
        entry,
        selector_precision: selector_precision_score(entry),
        broad_scope: allow_entry_broad_scope(entry).is_some(),
        current_findings: findings,
        match_outcomes: outcomes,
        evidence_references: &evidence_references,
        link_references: &link_references,
        suggested_actions: &suggested_actions,
        proof_commands: &proof_commands,
    })
}

fn link_reference_diagnostics_for_source_tree(
    root: &Path,
    entry: &AllowEntry,
    evidence_source_tree_files: Option<&BTreeSet<String>>,
) -> Vec<allow_policy::EvidenceReferenceDiagnostic> {
    policy_reference_diagnostics_for_source_tree(root, entry, evidence_source_tree_files)
        .into_iter()
        .filter(|reference| reference.source == crate::evidence_inventory::ReferenceSource::Link)
        .map(|reference| reference.diagnostic)
        .collect()
}

fn explain_reference_attention(
    evidence_diagnostics: &[allow_policy::EvidenceReferenceDiagnostic],
    link_diagnostics: &[allow_policy::EvidenceReferenceDiagnostic],
) -> ExplainReferenceAttention {
    ExplainReferenceAttention {
        has_broken_evidence: evidence_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.status.is_broken_local_link()),
        has_weak_evidence: evidence_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.status.is_weak_reference()),
        has_evidence_outside_default_inventory: evidence_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == DEFAULT_SOURCE_TREE_INVENTORY_EVIDENCE_MESSAGE),
        has_broken_link: link_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.status.is_broken_local_link()),
        has_weak_link: link_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.status.is_weak_reference()),
        has_link_outside_default_inventory: link_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == DEFAULT_SOURCE_TREE_INVENTORY_EVIDENCE_MESSAGE),
    }
}

fn link_reference_message(message: &str) -> String {
    message.replace("evidence", "link")
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{test_entry, test_finding};
    use super::*;
    use allow_core::{FindingKind, MatchOutcome, MatchStatus};

    #[test]
    fn render_explain_entry_json_uses_json_renderer_with_context() {
        let mut entry = test_entry("allow-json-render", FindingKind::NonRustFile);
        entry.evidence = vec!["test:manual-review".to_string()];
        let json = render_explain_entry_json(
            Path::new("."),
            &entry,
            &[],
            &[],
            None,
            ExplainContext {
                inventory: allow_report::InventoryContext::source_syntax(
                    "git_tracked",
                    Some("H:/Code/Rust/cargo-allow"),
                    Some(47),
                ),
            },
        );

        assert!(json.contains("\"schema_id\": \"cargo-allow.explain.v1\""));
        assert!(json.contains("\"command\": \"explain\""));
        assert!(json.contains("\"id\": \"allow-json-render\""));
        assert!(json.contains("\"source\": \"git_tracked\""));
        assert!(json.contains("\"files_scanned\": 47"));
    }

    #[test]
    fn render_explain_report_collects_reference_diagnostics_and_next_steps() {
        let mut entry = test_entry("allow-reference-diagnostics", FindingKind::NonRustFile);
        entry.evidence = vec![
            "doc:Cargo.toml".to_string(),
            "doc:docs/missing-evidence.md".to_string(),
            "test:manual-review".to_string(),
        ];
        entry.links = vec![
            "doc:Cargo.toml".to_string(),
            "doc:docs/missing-link.md".to_string(),
            "issue:123".to_string(),
        ];
        let source_tree_files = BTreeSet::new();
        let captured = render_explain_report(
            Path::new("."),
            &entry,
            &[],
            &[],
            Some(&source_tree_files),
            ExplainContext::default(),
            None,
            capture_explain_report,
        );

        assert_eq!(captured.entry_id, "allow-reference-diagnostics");
        assert_eq!(captured.evidence_references.len(), 3);
        assert_eq!(captured.link_references.len(), 3);
        let cargo_toml_evidence =
            captured.reference_by_raw(&captured.evidence_references, "doc:Cargo.toml");
        assert_eq!(cargo_toml_evidence.target.as_deref(), Some("Cargo.toml"));
        assert_eq!(cargo_toml_evidence.status, "local_file_missing");
        assert_eq!(
            cargo_toml_evidence.message,
            DEFAULT_SOURCE_TREE_INVENTORY_EVIDENCE_MESSAGE
        );
        let missing_evidence = captured.reference_by_raw(
            &captured.evidence_references,
            "doc:docs/missing-evidence.md",
        );
        assert_eq!(missing_evidence.status, "local_file_missing");
        assert_eq!(missing_evidence.category, "missing");
        let traceability_evidence =
            captured.reference_by_raw(&captured.evidence_references, "test:manual-review");
        assert_eq!(traceability_evidence.status, "traceability_only");

        let cargo_toml_link =
            captured.reference_by_raw(&captured.link_references, "doc:Cargo.toml");
        assert_eq!(cargo_toml_link.target.as_deref(), Some("Cargo.toml"));
        assert_eq!(cargo_toml_link.status, "local_file_missing");
        assert_eq!(
            cargo_toml_link.message,
            "local link file exists but is not in the default source-tree inventory"
        );
        let missing_link =
            captured.reference_by_raw(&captured.link_references, "doc:docs/missing-link.md");
        assert_eq!(missing_link.status, "local_file_missing");
        assert_eq!(missing_link.message, "local link file is missing");
        let issue_link = captured.reference_by_raw(&captured.link_references, "issue:123");
        assert_eq!(issue_link.status, "traceability_only");

        assert!(captured.suggested_actions.iter().any(|action| {
            action == "commit the referenced evidence file if it should support repository policy"
        }));
        assert!(captured.proof_commands.iter().any(|command| {
            command == "cargo-allow explain allow-reference-diagnostics --include-untracked"
        }));
    }

    #[test]
    fn render_explain_report_maps_live_state_and_scope_metrics() {
        let mut entry = test_entry("allow-broad", FindingKind::Panic);
        entry.path = None;
        entry.glob = Some("src/**".to_string());
        entry.family = Some("unwrap".to_string());
        entry.selector.callee = Some("unwrap".to_string());
        let finding = test_finding(
            FindingKind::Panic,
            Some("unwrap"),
            "src/lib.rs",
            "method_call",
        );
        let outcomes = vec![MatchOutcome {
            status: MatchStatus::New,
            allow_id: Some(entry.id.clone()),
            candidate_ids: Vec::new(),
            finding_index: Some(0),
            message: "new panic finding".to_string(),
            score: 72,
        }];

        let captured = render_explain_report(
            Path::new("."),
            &entry,
            std::slice::from_ref(&finding),
            &outcomes,
            None,
            ExplainContext::default(),
            None,
            capture_explain_report,
        );

        assert!(captured.broad_scope);
        assert_eq!(captured.current_findings, 1);
        assert_eq!(captured.match_outcomes, 1);
        assert!(captured.selector_precision > 0);
        assert!(!captured.suggested_actions.is_empty());
        assert!(
            captured
                .proof_commands
                .iter()
                .any(|command| command == "cargo-allow explain allow-broad")
        );
    }

    #[test]
    fn link_reference_diagnostics_filters_link_sources_only() {
        let mut entry = test_entry("allow-links-only", FindingKind::NonRustFile);
        entry.evidence = vec!["doc:docs/missing-evidence.md".to_string()];
        entry.links = vec![
            "doc:docs/missing-link.md".to_string(),
            "issue:123".to_string(),
        ];

        let diagnostics = link_reference_diagnostics_for_source_tree(Path::new("."), &entry, None);

        assert_eq!(diagnostics.len(), 2);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.raw == "doc:docs/missing-link.md")
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.raw == "issue:123")
        );
        assert!(
            !diagnostics
                .iter()
                .any(|diagnostic| diagnostic.raw == "doc:docs/missing-evidence.md")
        );
    }

    #[derive(Debug)]
    struct CapturedExplainReport {
        entry_id: String,
        selector_precision: u32,
        broad_scope: bool,
        current_findings: usize,
        match_outcomes: usize,
        evidence_references: Vec<CapturedReference>,
        link_references: Vec<CapturedReference>,
        suggested_actions: Vec<String>,
        proof_commands: Vec<String>,
    }

    impl CapturedExplainReport {
        fn reference_by_raw<'a>(
            &'a self,
            references: &'a [CapturedReference],
            raw: &str,
        ) -> &'a CapturedReference {
            let Some(reference) = references.iter().find(|reference| reference.raw == raw) else {
                std::panic::panic_any(format!("expected reference `{raw}` in {references:?}"));
            };
            reference
        }
    }

    #[derive(Debug)]
    struct CapturedReference {
        raw: String,
        target: Option<String>,
        status: String,
        category: String,
        message: String,
    }

    fn capture_explain_report(report: allow_report::ExplainReport<'_>) -> CapturedExplainReport {
        CapturedExplainReport {
            entry_id: report.entry.id.clone(),
            selector_precision: report.selector_precision,
            broad_scope: report.broad_scope,
            current_findings: report.current_findings.len(),
            match_outcomes: report.match_outcomes.len(),
            evidence_references: capture_references(report.evidence_references),
            link_references: capture_references(report.link_references),
            suggested_actions: report.suggested_actions.to_vec(),
            proof_commands: report.proof_commands.to_vec(),
        }
    }

    fn capture_references(
        references: &[allow_report::EvidenceReference<'_>],
    ) -> Vec<CapturedReference> {
        references
            .iter()
            .map(|reference| CapturedReference {
                raw: reference.raw.to_string(),
                target: reference.target.map(str::to_string),
                status: reference.status.to_string(),
                category: reference.category.to_string(),
                message: reference.message.to_string(),
            })
            .collect()
    }
}
