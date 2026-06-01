use super::worklist_item_kind::{BROKEN_EVIDENCE_LINK, WEAK_EVIDENCE_REFERENCE};
use super::worklist_priority::{DIFFICULTY_SMALL, RISK_HIGH, RISK_MEDIUM};
use super::{WorkItem, WorkItemEvidenceReference, proof_commands};
use crate::evidence_inventory::{
    DEFAULT_SOURCE_TREE_INVENTORY_EVIDENCE_MESSAGE, evidence_reference_diagnostics_for_source_tree,
};
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
                entry,
                diagnostic,
                item_index,
                ReferenceSource::Evidence,
            ));
        }
        let mut link_entry = entry.clone();
        link_entry.evidence = entry.links.clone();
        for mut diagnostic in evidence_reference_diagnostics_for_source_tree(
            root,
            &link_entry,
            evidence_source_tree_files,
        )
        .into_iter()
        .filter(|diagnostic| {
            diagnostic.status.is_broken_local_link() || diagnostic.status.is_weak_reference()
        }) {
            diagnostic.message = link_reference_message(&diagnostic.message);
            let item_index = start_index + items.len();
            items.push(work_item_from_evidence_diagnostic(
                entry,
                diagnostic,
                item_index,
                ReferenceSource::Link,
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
            "{} {} `{}`: {}",
            entry.id,
            source.label(),
            diagnostic.raw,
            diagnostic.message
        ),
        suggested_actions: evidence_suggested_actions(kind, &diagnostic, source),
        proof_commands,
    }
}

fn evidence_suggested_actions(
    kind: &str,
    diagnostic: &EvidenceReferenceDiagnostic,
    source: ReferenceSource,
) -> Vec<String> {
    if evidence_exists_outside_default_inventory(diagnostic) {
        return source.outside_inventory_actions();
    }
    if source == ReferenceSource::Link {
        return source.link_actions(kind);
    }
    super::suggested_actions(kind)
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
            == link_reference_message(DEFAULT_SOURCE_TREE_INVENTORY_EVIDENCE_MESSAGE)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReferenceSource {
    Evidence,
    Link,
}

impl ReferenceSource {
    fn label(self) -> &'static str {
        match self {
            Self::Evidence => "evidence",
            Self::Link => "link",
        }
    }

    fn outside_inventory_actions(self) -> Vec<String> {
        match self {
            Self::Evidence => vec![
                "commit the referenced evidence file if it should support repository policy"
                    .to_string(),
                "or rerun with --include-untracked when intentionally reviewing local receipt artifacts"
                    .to_string(),
            ],
            Self::Link => vec![
                "commit the referenced traceability file if it should support repository policy"
                    .to_string(),
                "or rerun with --include-untracked when intentionally reviewing local traceability files"
                    .to_string(),
            ],
        }
    }

    fn link_actions(self, kind: &str) -> Vec<String> {
        debug_assert_eq!(self, Self::Link);
        match kind {
            BROKEN_EVIDENCE_LINK => vec![
                "restore or commit the referenced local traceability file".to_string(),
                "or update the link reference to a valid source-tree-relative path".to_string(),
            ],
            WEAK_EVIDENCE_REFERENCE => vec![
                "replace the weak link string with a typed traceability reference".to_string(),
                format!(
                    "use a recognized prefix such as {}",
                    super::worklist_actions::evidence_prefix_examples()
                ),
            ],
            _ => super::suggested_actions(kind),
        }
    }
}

fn link_reference_message(message: &str) -> String {
    message.replace("evidence", "link")
}
