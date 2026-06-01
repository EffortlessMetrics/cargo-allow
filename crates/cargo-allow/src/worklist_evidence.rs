use super::worklist_actions::suggested_actions_for_context;
use super::worklist_item_kind::{BROKEN_EVIDENCE_LINK, WEAK_EVIDENCE_REFERENCE};
use super::worklist_priority::DIFFICULTY_SMALL;
use super::worklist_scoring::work_item_risk;
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
