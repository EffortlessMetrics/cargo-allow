use super::worklist_item_kind::{BROKEN_EVIDENCE_LINK, WEAK_EVIDENCE_REFERENCE};
use super::worklist_priority::{DIFFICULTY_SMALL, RISK_HIGH, RISK_MEDIUM};
use super::{WorkItem, WorkItemEvidenceReference, proof_commands};
use crate::evidence_inventory::evidence_reference_diagnostics_for_source_tree;
use crate::evidence_render::evidence_reference_target_text;
use allow_core::{AllowConfig, AllowEntry, FindingKind, MatchStatus};
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
        for diagnostic in
            evidence_reference_diagnostics_for_source_tree(root, entry, evidence_source_tree_files)
                .into_iter()
                .filter(|diagnostic| {
                    diagnostic.status.is_broken_local_link()
                        || diagnostic.status.is_weak_reference()
                })
        {
            let item_index = start_index + items.len();
            items.push(work_item_from_evidence_diagnostic(
                entry, diagnostic, item_index,
            ));
        }
    }
    items
}

fn work_item_from_evidence_diagnostic(
    entry: &AllowEntry,
    diagnostic: EvidenceReferenceDiagnostic,
    item_index: usize,
) -> WorkItem {
    let kind = if diagnostic.status.is_weak_reference() {
        WEAK_EVIDENCE_REFERENCE
    } else {
        debug_assert!(diagnostic.status.is_broken_local_link());
        BROKEN_EVIDENCE_LINK
    };
    let proof_commands = proof_commands(kind, None, Some(entry));
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
        risk: if entry.kind == FindingKind::Unsafe {
            RISK_HIGH
        } else {
            RISK_MEDIUM
        },
        difficulty: DIFFICULTY_SMALL,
        status: MatchStatus::EvidenceMissing,
        allow_id: Some(entry.id.clone()),
        finding_index: None,
        path,
        evidence_reference: Some(evidence_reference),
        source_package: None,
        message: format!(
            "{} evidence `{}`: {}",
            entry.id, diagnostic.raw, diagnostic.message
        ),
        suggested_actions: super::suggested_actions(kind),
        proof_commands,
    }
}
