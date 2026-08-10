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

    let old_output = committed_canonical_output(
        old.schema,
        old.kind.as_str(),
        &old.root_identity,
        old.object_format.as_str(),
        &old.head.requested,
        &old.head.commit,
        &old.head.tree,
        old.base.as_ref().map(|identity| {
            (
                identity.requested.as_str(),
                identity.commit.as_str(),
                identity.tree.as_str(),
            )
        }),
        old.merge_base.as_deref(),
        old.dirty_state.as_str(),
        format!("{:?}", old.selected_paths),
        &old.selected_source_closure,
        &old.limitations,
    );
    let new_output = committed_canonical_output(
        new.schema,
        new.kind.as_str(),
        &new.root_identity,
        new.object_format.as_str(),
        &new.head.requested,
        &new.head.commit,
        &new.head.tree,
        new.base.as_ref().map(|identity| {
            (
                identity.requested.as_str(),
                identity.commit.as_str(),
                identity.tree.as_str(),
            )
        }),
        new.merge_base.as_deref(),
        new.dirty_state.as_str(),
        format!("{:?}", new.selected_paths),
        &new.selected_source_closure,
        &new.limitations,
    );
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

fn committed_canonical_output(
    schema: &str,
    kind: &str,
    root_identity: &str,
    object_format: &str,
    head_requested: &str,
    head_commit: &str,
    head_tree: &str,
    base: Option<(&str, &str, &str)>,
    merge_base: Option<&str>,
    dirty_state: &str,
    selected_paths: String,
    selected_source_closure: &str,
    limitations: &[String],
) -> String {
    let base = base
        .map(|(requested, commit, tree)| format!("{requested}:{commit}:{tree}"))
        .unwrap_or_else(|| "none".to_string());
    format!(
        "schema={schema}|kind={kind}|root={root_identity}|object={object_format}|head={head_requested}:{head_commit}:{head_tree}|base={base}|merge_base={merge_base:?}|dirty={dirty_state}|paths={selected_paths}|closure={selected_source_closure}|limitations={limitations:?}"
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
