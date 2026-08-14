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
    // Old twin (allow-diff) deleted at cutover (#3556): the committed case
    // now proves the new authority resolves the exact git ground truth.
    let new_request = effortless_repo_snapshot::RepositorySnapshotRequest::committed_head("HEAD")
        .with_dirty_state_probe(true);
    let new =
        effortless_repo_snapshot::repository_snapshot(root, &new_request).map_err(|error| {
            CargoAllowError::new(format!("new RepoSnapshot authority failed: {error}"))
        })?;

    let oracle_commit = git_oracle_value(root, &["rev-parse", "HEAD"])?;
    let oracle_tree = git_oracle_value(root, &["rev-parse", "HEAD^{tree}"])?;
    let oracle_format = git_oracle_value(root, &["rev-parse", "--show-object-format"])?;

    let oracle_output = format!("head=HEAD:{oracle_commit}:{oracle_tree}|object={oracle_format}");
    let new_output = format!(
        "head=HEAD:{}:{}|object={}",
        new.head.commit,
        new.head.tree,
        new.object_format.as_str()
    );
    let source_identity = format!("commit:{}/tree:{}", new.head.commit, new.head.tree);
    Ok(parity_case(source_identity, oracle_output, new_output))
}

fn staged_index_parity(root: &Path) -> CargoAllowResult<RepoSnapshotParityCase> {
    // Old twin deleted at cutover (#3556): the staged case proves the new
    // authority's parent commit and staged change set match git ground truth.
    let new = effortless_repo_snapshot::staged_repository_snapshot(root)
        .map_err(|error| CargoAllowError::new(format!("new staged authority failed: {error}")))?;

    let oracle_parent = git_oracle_value(root, &["rev-parse", "HEAD"])?;
    let oracle_changes = git_oracle_staged_changes(root)?;

    let new_parent = new
        .parent_commit
        .clone()
        .unwrap_or_else(|| "none".to_string());
    let mut new_changes: Vec<String> = new
        .changes
        .iter()
        .map(|change| {
            format!(
                "{}	{}",
                staged_status_letter(&change.status),
                change
                    .path
                    .as_ref()
                    .map(|path| path.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_default()
            )
        })
        .collect();
    new_changes.sort();

    let oracle_output = format!("parent={oracle_parent}|changes={oracle_changes:?}");
    let new_output = format!("parent={new_parent}|changes={new_changes:?}");
    let source_identity = format!("staged:{}:{}", new_parent, new.identity.semantic_hash);
    Ok(parity_case(source_identity, oracle_output, new_output))
}

fn git_oracle_value(root: &Path, args: &[&str]) -> CargoAllowResult<String> {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|error| CargoAllowError::new(format!("git oracle {:?}: {error}", args)))?;
    if !output.status.success() {
        return Err(CargoAllowError::new(format!(
            "git oracle {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn git_oracle_staged_changes(root: &Path) -> CargoAllowResult<Vec<String>> {
    let text = git_oracle_value(root, &["diff", "--cached", "--name-status"])?;
    let mut changes = Vec::new();
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        let mut fields = line.split('\t');
        let (status, path) = match (fields.next(), fields.next_back()) {
            (Some(status), Some(path)) => (status, path),
            _ => continue,
        };
        let letter = status.chars().next().unwrap_or('T');
        changes.push(format!("{letter}\t{}", path.replace('\\', "/")));
    }
    changes.sort();
    Ok(changes)
}

fn staged_status_letter(status: &effortless_repo_snapshot::StagedPathStatus) -> char {
    match status {
        effortless_repo_snapshot::StagedPathStatus::Added => 'A',
        effortless_repo_snapshot::StagedPathStatus::Modified => 'M',
        effortless_repo_snapshot::StagedPathStatus::Deleted => 'D',
        effortless_repo_snapshot::StagedPathStatus::Renamed => 'R',
        effortless_repo_snapshot::StagedPathStatus::Copied => 'C',
        effortless_repo_snapshot::StagedPathStatus::TypeChanged => 'T',
        effortless_repo_snapshot::StagedPathStatus::Unmerged => 'U',
        effortless_repo_snapshot::StagedPathStatus::Unknown => '?',
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use allow_policy::extraction_parity::ParityComparisonResult;
    use std::path::PathBuf;

    #[test]
    fn repository_snapshot_matches_git_ground_truth() -> Result<(), String> {
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
