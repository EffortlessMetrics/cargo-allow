use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowErrorKind, CargoAllowResult, normalize_path,
};
use allow_policy::{
    EvidenceReferenceCategory, EvidenceReferenceDiagnostic, EvidenceReferenceSource,
    EvidenceReferenceStatus, evidence_reference_diagnostics, policy_reference_diagnostics,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

pub(crate) const DEFAULT_SOURCE_TREE_INVENTORY_EVIDENCE_MESSAGE: &str =
    "local evidence file exists but is not in the default source-tree inventory";

pub(crate) fn current_evidence_source_tree_files(
    root: &Path,
    include_untracked: bool,
) -> Option<BTreeSet<String>> {
    if include_untracked {
        return None;
    }
    let Ok(files) = allow_inventory::git_ls_files(root) else {
        return None;
    };
    Some(
        files
            .into_iter()
            .filter(|path| {
                fs::symlink_metadata(root.join(path))
                    .map(|metadata| metadata.file_type().is_file())
                    .unwrap_or(false)
            })
            .map(normalize_path)
            .collect(),
    )
}

pub(crate) fn evidence_reference_diagnostics_for_source_tree(
    root: &Path,
    entry: &AllowEntry,
    source_tree_files: Option<&BTreeSet<String>>,
) -> Vec<EvidenceReferenceDiagnostic> {
    let mut diagnostics = evidence_reference_diagnostics(root, entry);
    for diagnostic in &mut diagnostics {
        apply_source_tree_inventory_to_diagnostic(diagnostic, source_tree_files);
    }
    diagnostics
}

pub(crate) fn validate_evidence_references_for_source_tree(
    root: &Path,
    cfg: &AllowConfig,
    source_tree_files: Option<&BTreeSet<String>>,
) -> CargoAllowResult<()> {
    for entry in &cfg.allow {
        for reference in
            policy_reference_diagnostics_for_source_tree(root, entry, source_tree_files)
        {
            if reference.diagnostic.status.is_broken_local_link() {
                return Err(CargoAllowError::with_kind(
                    CargoAllowErrorKind::Artifact,
                    format!(
                        "{} {} `{}`: {}",
                        entry.id,
                        reference.source.label(),
                        reference.diagnostic.raw,
                        reference.source.message(&reference.diagnostic.message)
                    ),
                ));
            }
        }
    }
    Ok(())
}

pub(crate) fn policy_reference_diagnostics_for_source_tree(
    root: &Path,
    entry: &AllowEntry,
    source_tree_files: Option<&BTreeSet<String>>,
) -> Vec<PolicyReferenceDiagnostic> {
    policy_reference_diagnostics(root, entry)
        .into_iter()
        .map(|mut reference| {
            apply_source_tree_inventory_to_diagnostic(&mut reference.diagnostic, source_tree_files);
            reference
        })
        .collect()
}

pub(crate) type PolicyReferenceDiagnostic = allow_policy::PolicyReferenceDiagnostic;
pub(crate) type ReferenceSource = EvidenceReferenceSource;

fn apply_source_tree_inventory_to_diagnostic(
    diagnostic: &mut EvidenceReferenceDiagnostic,
    source_tree_files: Option<&BTreeSet<String>>,
) {
    let Some(source_tree_files) = source_tree_files else {
        return;
    };
    if diagnostic.status != EvidenceReferenceStatus::LocalFilePresent {
        return;
    }
    let Some(target) = diagnostic.target.as_ref() else {
        return;
    };
    if source_tree_files.contains(&normalize_path(target)) {
        return;
    }
    diagnostic.status = EvidenceReferenceStatus::LocalFileMissing;
    diagnostic.category = EvidenceReferenceCategory::Missing;
    diagnostic.message = DEFAULT_SOURCE_TREE_INVENTORY_EVIDENCE_MESSAGE.to_string();
}

#[cfg(test)]
#[path = "evidence_inventory_tests.rs"]
mod tests;
