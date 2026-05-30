use super::worklist_item_kind::{BROKEN_EVIDENCE_LINK, WEAK_EVIDENCE_REFERENCE};
use super::worklist_priority::{DIFFICULTY_SMALL, RISK_HIGH, RISK_MEDIUM};
use super::{WorkItem, proof_commands};
use allow_core::{AllowConfig, AllowEntry, FindingKind, MatchStatus, normalize_path};
use allow_policy::{EvidenceReferenceDiagnostic, evidence_reference_diagnostics};
use std::path::Path;

pub(super) fn work_items_from_evidence_diagnostics(
    root: &Path,
    cfg: &AllowConfig,
    start_index: usize,
) -> Vec<WorkItem> {
    let mut items = Vec::new();
    for entry in &cfg.allow {
        for diagnostic in evidence_reference_diagnostics(root, entry)
            .into_iter()
            .filter(|diagnostic| {
                diagnostic.status.is_broken_local_link() || diagnostic.status.is_weak_reference()
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
    let path = if kind == BROKEN_EVIDENCE_LINK {
        diagnostic.target.as_ref().map(normalize_path)
    } else {
        None
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
        source_package: None,
        message: format!(
            "{} evidence `{}`: {}",
            entry.id, diagnostic.raw, diagnostic.message
        ),
        suggested_actions: super::suggested_actions(kind),
        proof_commands,
    }
}
