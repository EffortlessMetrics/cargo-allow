//! Source-view supply for workspace compositions (#3306).
//!
//! Staged, committed, and worktree views of a repository's composition
//! authority files are supplied through `effortless-repo-snapshot`'s
//! `RepositorySourceView`: every read goes through one view handle, so a
//! staged or committed analysis can never mix bytes from the dirty
//! worktree. This is the production seam the orchestrator uses at
//! cutover; structural subject resolution stays on
//! `effortless-rust-source-index` via `crate::subject_resolution`.

use std::path::Path;

use effortless_repo_snapshot::{RepositorySourceView, SnapshotError};

use super::composition::WorkspaceCompositionV1;

/// The four composition authority file texts, read through a single
/// source view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceCompositionSources {
    pub requirement: String,
    pub slice: String,
    pub seams: String,
    pub evidence: String,
}

/// Read every composition authority file through the given view. A view
/// that cannot read any of the four files fails with that error rather
/// than silently substituting worktree bytes.
pub fn read_workspace_composition_sources(
    view: &RepositorySourceView,
    composition: &WorkspaceCompositionV1,
) -> Result<WorkspaceCompositionSources, SnapshotError> {
    Ok(WorkspaceCompositionSources {
        requirement: view.read_text(Path::new(&composition.requirement_path))?,
        slice: view.read_text(Path::new(&composition.slice_path))?,
        seams: view.read_text(Path::new(&composition.seams_path))?,
        evidence: view.read_text(Path::new(&composition.evidence_path))?,
    })
}

/// Whether every composition authority file is present in the given
/// view. Unlike a worktree filesystem check this answers for the exact
/// staged or committed candidate under analysis.
pub fn composition_sources_present_in_view(
    view: &RepositorySourceView,
    composition: &WorkspaceCompositionV1,
) -> bool {
    read_workspace_composition_sources(view, composition).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::WorkspaceCompositionV1;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> Result<std::path::PathBuf, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "intent-engine-source-views-{label}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        Ok(root)
    }

    #[test]
    fn worktree_view_reads_every_composition_source() -> Result<(), String> {
        let root = temp_root("worktree")?;
        let composition = WorkspaceCompositionV1::self_hosted_runtime_promotion();
        for path in composition.authority_source_paths() {
            let full = root.join(path);
            if let Some(parent) = full.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            fs::write(&full, format!("authority bytes for {path}"))
                .map_err(|error| error.to_string())?;
        }
        let view = RepositorySourceView::filesystem(&root)
            .map_err(|error| format!("open worktree view: {error}"))?;
        let sources = read_workspace_composition_sources(&view, &composition)
            .map_err(|error| format!("read sources: {error}"))?;
        if !sources.requirement.contains("authority bytes") {
            return Err("requirement source not read through the view".to_string());
        }
        if !composition_sources_present_in_view(&view, &composition) {
            return Err("present check disagrees with a successful read".to_string());
        }
        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn missing_authority_file_fails_the_read() -> Result<(), String> {
        let root = temp_root("missing")?;
        let composition = WorkspaceCompositionV1::self_hosted_runtime_promotion();
        let view = RepositorySourceView::filesystem(&root)
            .map_err(|error| format!("open worktree view: {error}"))?;
        if composition_sources_present_in_view(&view, &composition) {
            return Err("absent authority files must not read as present".to_string());
        }
        let _ = fs::remove_dir_all(&root);
        Ok(())
    }
}
