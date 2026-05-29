use super::worklist_item_kind::BROKEN_EVIDENCE_LINK;
use super::worklist_priority::{DIFFICULTY_SMALL, RISK_HIGH, RISK_MEDIUM};
use super::{WorkItem, proof_commands};
use allow_core::{AllowConfig, FindingKind, MatchStatus, normalize_path};
use allow_policy::evidence_reference_diagnostics;
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
            .filter(|diagnostic| diagnostic.status.is_broken_local_link())
        {
            let item_index = start_index + items.len();
            let kind = BROKEN_EVIDENCE_LINK.to_string();
            let proof_commands = proof_commands(&kind, None, Some(entry));
            items.push(WorkItem {
                id: format!("work-broken-evidence-link-{item_index:04}"),
                kind,
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
                path: diagnostic.target.as_ref().map(normalize_path),
                source_package: None,
                message: format!(
                    "{} evidence `{}`: {}",
                    entry.id, diagnostic.raw, diagnostic.message
                ),
                suggested_actions: vec![
                    "restore or commit the referenced local evidence artifact".to_string(),
                    "or update the evidence reference to a valid source-tree-relative path"
                        .to_string(),
                ],
                proof_commands,
            });
        }
    }
    items
}
