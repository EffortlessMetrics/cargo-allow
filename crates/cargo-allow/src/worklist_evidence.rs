use super::worklist_actions::suggested_actions_for_context;
use super::worklist_item_kind::{BROKEN_EVIDENCE_LINK, WEAK_EVIDENCE_REFERENCE};
use super::worklist_priority::DIFFICULTY_SMALL;
use super::worklist_scoring::work_item_risk;
use super::worklist_types::WorkItemLedger;
use super::{WorkItem, WorkItemEvidenceReference, proof_commands};
use crate::evidence_inventory::{
    DEFAULT_SOURCE_TREE_INVENTORY_EVIDENCE_MESSAGE, ReferenceSource,
    policy_reference_diagnostics_for_source_tree,
};
use crate::evidence_render::evidence_reference_target_text;
use allow_core::{AllowConfig, AllowEntry, MatchStatus};
use allow_diff::selector_precision_score;
use allow_policy::EvidenceReferenceDiagnostic;
use std::collections::BTreeSet;
use std::path::Path;

#[cfg(test)]
pub(super) fn work_items_from_evidence_diagnostics(
    root: &Path,
    cfg: &AllowConfig,
    start_index: usize,
) -> Vec<WorkItem> {
    work_items_from_evidence_diagnostics_with_source_tree_files(root, cfg, start_index, None)
}

pub(super) fn work_items_from_evidence_diagnostics_with_source_tree_files(
    root: &Path,
    cfg: &AllowConfig,
    start_index: usize,
    evidence_source_tree_files: Option<&BTreeSet<String>>,
) -> Vec<WorkItem> {
    let mut items = Vec::new();
    for entry in &cfg.allow {
        for reference in
            policy_reference_diagnostics_for_source_tree(root, entry, evidence_source_tree_files)
                .into_iter()
                .filter(|reference| {
                    reference.diagnostic.status.is_broken_local_link()
                        || reference.diagnostic.status.is_weak_reference()
                })
        {
            let mut diagnostic = reference.diagnostic;
            diagnostic.message = reference.source.message(&diagnostic.message);
            let item_index = start_index + items.len();
            items.push(work_item_from_evidence_diagnostic(
                entry,
                diagnostic,
                item_index,
                reference.source,
            ));
        }
    }
    items
}

fn work_item_from_evidence_diagnostic(
    entry: &AllowEntry,
    diagnostic: EvidenceReferenceDiagnostic,
    item_index: usize,
    source: ReferenceSource,
) -> WorkItem {
    let kind = if diagnostic.status.is_weak_reference() {
        WEAK_EVIDENCE_REFERENCE
    } else {
        debug_assert!(diagnostic.status.is_broken_local_link());
        BROKEN_EVIDENCE_LINK
    };
    let proof_commands = evidence_proof_commands(kind, entry, &diagnostic);
    let target = evidence_reference_target_text(&diagnostic);
    let path = if kind == BROKEN_EVIDENCE_LINK {
        target.clone()
    } else {
        None
    };
    let evidence_reference = WorkItemEvidenceReference {
        raw: diagnostic.raw.clone(),
        prefix: diagnostic.prefix.clone(),
        target,
        status: diagnostic.status.as_str().to_string(),
        category: diagnostic.category.as_str().to_string(),
        message: diagnostic.message.clone(),
    };
    WorkItem {
        id: format!("work-{}-{item_index:04}", kind.replace('_', "-")),
        kind: kind.to_string(),
        exception_kind: Some(entry.kind.as_str().to_string()),
        family: entry.family.clone(),
        owner: Some(entry.owner.clone()),
        classification: Some(entry.classification.clone()),
        reason: Some(entry.reason.clone()),
        created: entry.lifecycle.created.clone(),
        review_after: entry.lifecycle.review_after.clone(),
        expires: entry.lifecycle.expires.clone(),
        evidence_count: Some(entry.evidence.len()),
        selector_precision: Some(selector_precision_score(entry)),
        risk: work_item_risk(kind, MatchStatus::EvidenceMissing, None, Some(entry)),
        difficulty: DIFFICULTY_SMALL,
        status: MatchStatus::EvidenceMissing,
        allow_id: Some(entry.id.clone()),
        finding_index: None,
        path,
        evidence_reference: Some(evidence_reference),
        source_package: None,
        message: format!(
            "{} {} `{}`: {}",
            entry.id,
            source.label(),
            diagnostic.raw,
            diagnostic.message
        ),
        suggested_actions: evidence_suggested_actions(kind, entry, &diagnostic, source),
        proof_commands,
        ledger: WorkItemLedger::default(),
    }
}

fn evidence_suggested_actions(
    kind: &str,
    entry: &AllowEntry,
    diagnostic: &EvidenceReferenceDiagnostic,
    source: ReferenceSource,
) -> Vec<String> {
    if evidence_exists_outside_default_inventory(diagnostic) {
        return outside_inventory_actions(source);
    }
    if source == ReferenceSource::Link {
        return super::worklist_actions::suggested_link_actions_for_context(
            kind,
            None,
            Some(entry),
        );
    }
    suggested_actions_for_context(kind, None, Some(entry))
}

fn evidence_proof_commands(
    kind: &str,
    entry: &AllowEntry,
    diagnostic: &EvidenceReferenceDiagnostic,
) -> Vec<String> {
    if !evidence_exists_outside_default_inventory(diagnostic) {
        return proof_commands(kind, None, Some(entry));
    }

    let mut commands = vec![
        format!("cargo-allow explain {}", entry.id),
        format!("cargo-allow explain {} --include-untracked", entry.id),
        format!("cargo-allow worklist --allow-id {} --format json", entry.id),
        "cargo-allow check --include-untracked --mode no-new".to_string(),
    ];
    for command in proof_commands(kind, None, Some(entry)) {
        if !commands.iter().any(|existing| existing == &command) {
            commands.push(command);
        }
    }
    commands
}

fn evidence_exists_outside_default_inventory(diagnostic: &EvidenceReferenceDiagnostic) -> bool {
    diagnostic.message == DEFAULT_SOURCE_TREE_INVENTORY_EVIDENCE_MESSAGE
        || diagnostic.message
            == ReferenceSource::Link.message(DEFAULT_SOURCE_TREE_INVENTORY_EVIDENCE_MESSAGE)
}

fn outside_inventory_actions(source: ReferenceSource) -> Vec<String> {
    match source {
        ReferenceSource::Evidence => vec![
            "commit the referenced evidence file if it should support repository policy"
                .to_string(),
            "or rerun with --include-untracked when intentionally reviewing local receipt artifacts"
                .to_string(),
        ],
        ReferenceSource::Link => vec![
            "commit the referenced traceability file if it should support repository policy"
                .to_string(),
            "or rerun with --include-untracked when intentionally reviewing local traceability files"
                .to_string(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{FindingKind, Lifecycle, Selector};
    use allow_policy::{EvidenceReferenceCategory, EvidenceReferenceStatus};
    use std::path::PathBuf;

    fn entry(id: &str, kind: FindingKind) -> AllowEntry {
        AllowEntry {
            id: id.to_string(),
            kind,
            family: Some("network_destination".to_string()),
            path: Some(PathBuf::from("src/lib.rs")),
            glob: None,
            owner: "security".to_string(),
            classification: "reviewed".to_string(),
            reason: "Network exception is reviewed.".to_string(),
            evidence: vec!["doc:docs/evidence.md".to_string()],
            links: vec!["doc:docs/trace.md".to_string()],
            occurrence_limit: Some(1),
            lifecycle: Lifecycle {
                created: Some("2026-06-13".to_string()),
                review_after: Some("2026-07-13".to_string()),
                expires: Some("2026-08-13".to_string()),
            },
            selector: Selector {
                ast_kind: Some("function".to_string()),
                ..Selector::default()
            },
            last_seen: None,
        }
    }

    fn diagnostic(
        raw: &str,
        prefix: Option<&str>,
        target: Option<&str>,
        status: EvidenceReferenceStatus,
        category: EvidenceReferenceCategory,
        message: &str,
    ) -> EvidenceReferenceDiagnostic {
        EvidenceReferenceDiagnostic {
            raw: raw.to_string(),
            prefix: prefix.map(str::to_string),
            target: target.map(PathBuf::from),
            status,
            category,
            message: message.to_string(),
        }
    }

    #[test]
    fn work_item_from_evidence_diagnostic_observes_broken_evidence_fields() {
        let entry = entry("allow-network", FindingKind::PolicyException);
        let diagnostic = diagnostic(
            "doc:docs/missing.md",
            Some("doc"),
            Some("docs/missing.md"),
            EvidenceReferenceStatus::LocalFileMissing,
            EvidenceReferenceCategory::Missing,
            "local evidence file is missing",
        );

        let item =
            work_item_from_evidence_diagnostic(&entry, diagnostic, 7, ReferenceSource::Evidence);

        assert_eq!(item.id, "work-broken-evidence-link-0007");
        assert_eq!(item.kind, "broken_evidence_link");
        assert_eq!(item.exception_kind.as_deref(), Some("policy_exception"));
        assert_eq!(item.family.as_deref(), Some("network_destination"));
        assert_eq!(item.owner.as_deref(), Some("security"));
        assert_eq!(item.classification.as_deref(), Some("reviewed"));
        assert_eq!(
            item.reason.as_deref(),
            Some("Network exception is reviewed.")
        );
        assert_eq!(item.created.as_deref(), Some("2026-06-13"));
        assert_eq!(item.review_after.as_deref(), Some("2026-07-13"));
        assert_eq!(item.expires.as_deref(), Some("2026-08-13"));
        assert_eq!(item.evidence_count, Some(1));
        assert_eq!(item.risk, "high");
        assert_eq!(item.difficulty, "small");
        assert_eq!(item.status, MatchStatus::EvidenceMissing);
        assert_eq!(item.allow_id.as_deref(), Some("allow-network"));
        assert_eq!(item.finding_index, None);
        assert_eq!(item.path.as_deref(), Some("docs/missing.md"));
        assert_eq!(item.source_package, None);
        assert!(item.selector_precision.is_some());
        assert!(
            item.message
                .contains("allow-network evidence `doc:docs/missing.md`")
        );
        assert!(item.message.contains("local evidence file is missing"));

        let Some(reference) = item.evidence_reference.as_ref() else {
            return;
        };
        assert_eq!(reference.raw, "doc:docs/missing.md");
        assert_eq!(reference.prefix.as_deref(), Some("doc"));
        assert_eq!(reference.target.as_deref(), Some("docs/missing.md"));
        assert_eq!(reference.status, "local_file_missing");
        assert_eq!(reference.category, "missing");
        assert!(reference.message.contains("local evidence file is missing"));
        assert!(
            item.suggested_actions
                .iter()
                .any(|action| action.contains("referenced local evidence artifact"))
        );
        assert!(
            item.proof_commands
                .iter()
                .any(|command| command == "cargo-allow explain allow-network")
        );
        assert!(
            item.proof_commands
                .iter()
                .any(|command| command == "cargo-allow list --broken-evidence --format json")
        );
    }

    #[test]
    fn work_item_from_evidence_diagnostic_observes_link_and_weak_routes() {
        let entry = entry("allow-network", FindingKind::PolicyException);
        let broken_link = diagnostic(
            "doc:docs/missing-link.md",
            Some("doc"),
            Some("docs/missing-link.md"),
            EvidenceReferenceStatus::LocalFileMissing,
            EvidenceReferenceCategory::Missing,
            "local link file is missing",
        );

        let link_item =
            work_item_from_evidence_diagnostic(&entry, broken_link, 3, ReferenceSource::Link);

        assert_eq!(link_item.id, "work-broken-evidence-link-0003");
        assert_eq!(link_item.kind, "broken_evidence_link");
        assert_eq!(link_item.path.as_deref(), Some("docs/missing-link.md"));
        assert!(
            link_item
                .message
                .contains("allow-network link `doc:docs/missing-link.md`")
        );
        assert!(link_item.message.contains("local link file is missing"));
        let Some(reference) = link_item.evidence_reference.as_ref() else {
            return;
        };
        assert!(reference.message.contains("local link file is missing"));
        assert!(
            link_item
                .suggested_actions
                .iter()
                .any(|action| action.contains("traceability"))
        );

        let weak_evidence = diagnostic(
            "spreadsheet:manual-review",
            Some("spreadsheet"),
            Some("manual-review"),
            EvidenceReferenceStatus::Unstructured,
            EvidenceReferenceCategory::UnknownPrefix,
            "unrecognized evidence prefix",
        );

        let weak_item =
            work_item_from_evidence_diagnostic(&entry, weak_evidence, 4, ReferenceSource::Evidence);

        assert_eq!(weak_item.id, "work-weak-evidence-reference-0004");
        assert_eq!(weak_item.kind, "weak_evidence_reference");
        assert_eq!(weak_item.path, None);
        assert!(weak_item.message.contains("unrecognized evidence prefix"));
        let Some(reference) = weak_item.evidence_reference.as_ref() else {
            return;
        };
        assert_eq!(reference.raw, "spreadsheet:manual-review");
        assert_eq!(reference.prefix.as_deref(), Some("spreadsheet"));
        assert_eq!(reference.target.as_deref(), Some("manual-review"));
        assert_eq!(reference.status, "unstructured");
        assert_eq!(reference.category, "unknown_prefix");
        assert!(
            weak_item
                .suggested_actions
                .iter()
                .any(|action| action.contains("policy_exception.network_destination"))
        );
        assert!(
            weak_item
                .proof_commands
                .iter()
                .any(|command| command == "cargo-allow worklist --weak-evidence --format json")
        );
    }

    #[test]
    fn outside_inventory_diagnostics_add_include_untracked_routes() {
        let entry = entry("allow-network", FindingKind::PolicyException);
        let diagnostic = diagnostic(
            "doc:docs/untracked.md",
            Some("doc"),
            Some("docs/untracked.md"),
            EvidenceReferenceStatus::LocalFileMissing,
            EvidenceReferenceCategory::Missing,
            DEFAULT_SOURCE_TREE_INVENTORY_EVIDENCE_MESSAGE,
        );

        let item =
            work_item_from_evidence_diagnostic(&entry, diagnostic, 5, ReferenceSource::Evidence);

        assert_eq!(item.kind, "broken_evidence_link");
        assert_eq!(item.path.as_deref(), Some("docs/untracked.md"));
        assert!(
            item.suggested_actions
                .iter()
                .any(|action| action.contains("commit the referenced evidence file"))
        );
        assert!(
            item.suggested_actions
                .iter()
                .any(|action| action.contains("--include-untracked"))
        );
        assert!(
            item.proof_commands
                .iter()
                .any(|command| command == "cargo-allow explain allow-network --include-untracked")
        );
        assert!(
            item.proof_commands
                .iter()
                .any(|command| command == "cargo-allow check --include-untracked --mode no-new")
        );
    }
}
