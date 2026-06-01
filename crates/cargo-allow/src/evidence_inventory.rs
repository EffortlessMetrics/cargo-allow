use allow_core::{AllowEntry, normalize_path};
use allow_policy::{
    EvidenceReferenceCategory, EvidenceReferenceDiagnostic, EvidenceReferenceStatus,
    evidence_reference_diagnostics,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

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
    let Some(source_tree_files) = source_tree_files else {
        return diagnostics;
    };
    for diagnostic in &mut diagnostics {
        if diagnostic.status != EvidenceReferenceStatus::LocalFilePresent {
            continue;
        }
        let Some(target) = diagnostic.target.as_ref() else {
            continue;
        };
        if source_tree_files.contains(&normalize_path(target)) {
            continue;
        }
        diagnostic.status = EvidenceReferenceStatus::LocalFileMissing;
        diagnostic.category = EvidenceReferenceCategory::Missing;
        diagnostic.message =
            "local evidence file exists but is not in the default source-tree inventory"
                .to_string();
    }
    diagnostics
}
