//! Runtime parity adapters for the RepoSnapshot extraction stage (#3373).
//!
//! This module executes the old and new snapshot authorities against the same
//! repository and reduces their typed results to the policy comparison kernel.
//! It does not promote a cutover or manufacture reachability/package evidence.

use allow_core::{CargoAllowError, CargoAllowResult};
use allow_policy::extraction_parity::{ParityComparison, ParityObservation, compare_observations};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepoSnapshotParityRun {
    pub committed: RepoSnapshotParityCase,
    pub staged: RepoSnapshotParityCase,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepoSnapshotParityCase {
    pub comparison: ParityComparison,
    pub old_output: String,
    pub new_output: String,
}

/// Execute committed-head and staged-index parity against one exact root.
pub(crate) fn run_repo_snapshot_parity(root: &Path) -> CargoAllowResult<RepoSnapshotParityRun> {
    let committed = committed_head_parity(root)?;
    let staged = staged_index_parity(root)?;
    Ok(RepoSnapshotParityRun { committed, staged })
}

fn committed_head_parity(root: &Path) -> CargoAllowResult<RepoSnapshotParityCase> {
    let old_request =
        allow_diff::RepositorySnapshotRequest::committed_head("HEAD").with_dirty_state_probe(true);
    let old = allow_diff::repository_snapshot(root, &old_request).map_err(|error| {
        CargoAllowError::new(format!("old RepoSnapshot authority failed: {error}"))
    })?;

    let new_request = effortless_repo_snapshot::RepositorySnapshotRequest::committed_head("HEAD")
        .with_dirty_state_probe(true);
    let new =
        effortless_repo_snapshot::repository_snapshot(root, &new_request).map_err(|error| {
            CargoAllowError::new(format!("new RepoSnapshot authority failed: {error}"))
        })?;

    let old_output = committed_canonical_output(&CommittedCanonicalParts {
        schema: old.schema,
        kind: old.kind.as_str(),
        root_identity: &old.root_identity,
        object_format: old.object_format.as_str(),
        head_requested: &old.head.requested,
        head_commit: &old.head.commit,
        head_tree: &old.head.tree,
        base: old.base.as_ref().map(|identity| {
            (
                identity.requested.as_str(),
                identity.commit.as_str(),
                identity.tree.as_str(),
            )
        }),
        merge_base: old.merge_base.as_deref(),
        dirty_state: old.dirty_state.as_str(),
        selected_paths: format!("{:?}", old.selected_paths),
        selected_source_closure: &old.selected_source_closure,
        limitations: &old.limitations,
    });
    let new_output = committed_canonical_output(&CommittedCanonicalParts {
        schema: new.schema,
        kind: new.kind.as_str(),
        root_identity: &new.root_identity,
        object_format: new.object_format.as_str(),
        head_requested: &new.head.requested,
        head_commit: &new.head.commit,
        head_tree: &new.head.tree,
        base: new.base.as_ref().map(|identity| {
            (
                identity.requested.as_str(),
                identity.commit.as_str(),
                identity.tree.as_str(),
            )
        }),
        merge_base: new.merge_base.as_deref(),
        dirty_state: new.dirty_state.as_str(),
        selected_paths: format!("{:?}", new.selected_paths),
        selected_source_closure: &new.selected_source_closure,
        limitations: &new.limitations,
    });
    let source_identity = format!("commit:{}/tree:{}", old.head.commit, old.head.tree);
    Ok(parity_case(source_identity, old_output, new_output))
}

fn staged_index_parity(root: &Path) -> CargoAllowResult<RepoSnapshotParityCase> {
    let old = allow_diff::staged_repository_snapshot(root)
        .map_err(|error| CargoAllowError::new(format!("old staged authority failed: {error}")))?;
    let new = effortless_repo_snapshot::staged_repository_snapshot(root)
        .map_err(|error| CargoAllowError::new(format!("new staged authority failed: {error}")))?;

    let old_output = staged_canonical_output(
        &old.parent_commit,
        &old.capabilities,
        &old.entries,
        &old.changes,
        &old.identity,
        old.completeness,
        &old.limitations,
    );
    let new_output = staged_canonical_output(
        &new.parent_commit,
        &new.capabilities,
        &new.entries,
        &new.changes,
        &new.identity,
        new.completeness,
        &new.limitations,
    );
    let source_identity = format!(
        "staged:{}:{}",
        old.parent_commit.as_deref().unwrap_or("none"),
        old.identity.semantic_hash
    );
    Ok(parity_case(source_identity, old_output, new_output))
}

fn parity_case(
    source_identity: String,
    old_output: String,
    new_output: String,
) -> RepoSnapshotParityCase {
    let comparison = compare_observations(
        &ParityObservation {
            source_identity: source_identity.clone(),
            canonical_output: old_output.clone(),
        },
        &ParityObservation {
            source_identity,
            canonical_output: new_output.clone(),
        },
    );
    RepoSnapshotParityCase {
        comparison,
        old_output,
        new_output,
    }
}

struct CommittedCanonicalParts<'a> {
    schema: &'a str,
    kind: &'a str,
    root_identity: &'a str,
    object_format: &'a str,
    head_requested: &'a str,
    head_commit: &'a str,
    head_tree: &'a str,
    base: Option<(&'a str, &'a str, &'a str)>,
    merge_base: Option<&'a str>,
    dirty_state: &'a str,
    selected_paths: String,
    selected_source_closure: &'a str,
    limitations: &'a [String],
}

fn committed_canonical_output(parts: &CommittedCanonicalParts<'_>) -> String {
    let base = parts
        .base
        .map(|(requested, commit, tree)| format!("{requested}:{commit}:{tree}"))
        .unwrap_or_else(|| "none".to_string());
    format!(
        "schema={}|kind={}|root={}|object={}|head={}:{}:{}|base={base}|merge_base={:?}|dirty={}|paths={}|closure={}|limitations={:?}",
        parts.schema,
        parts.kind,
        parts.root_identity,
        parts.object_format,
        parts.head_requested,
        parts.head_commit,
        parts.head_tree,
        parts.merge_base,
        parts.dirty_state,
        parts.selected_paths,
        parts.selected_source_closure,
        parts.limitations,
    )
}

fn staged_canonical_output<T: std::fmt::Debug>(
    parent_commit: &Option<String>,
    capabilities: &T,
    entries: &[impl std::fmt::Debug],
    changes: &[impl std::fmt::Debug],
    identity: &impl std::fmt::Debug,
    completeness: impl std::fmt::Debug,
    limitations: &[String],
) -> String {
    format!(
        "parent={parent_commit:?}|capabilities={capabilities:?}|entries={entries:?}|changes={changes:?}|identity={identity:?}|completeness={completeness:?}|limitations={limitations:?}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_policy::extraction_parity::ParityComparisonResult;
    use std::path::PathBuf;

    #[test]
    fn repository_snapshot_authorities_are_parity_equivalent() -> Result<(), String> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let run = run_repo_snapshot_parity(&root).map_err(|error| error.to_string())?;
        for (label, case) in [("committed", run.committed), ("staged", run.staged)] {
            if case.comparison.result != ParityComparisonResult::SemanticallyEquivalent {
                return Err(format!(
                    "{label} RepoSnapshot parity was not equivalent: {:?}",
                    case.comparison
                ));
            }
            if case.old_output != case.new_output {
                return Err(format!("{label} canonical outputs differed"));
            }
        }
        Ok(())
    }
}
