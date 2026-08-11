//! Runtime parity adapters for the RepoEdit extraction stage (#3373).
//!
//! The compatibility surface in `cargo-allow` is intentionally private, but
//! it remains an authority that must agree with the direct extracted crate
//! until the shim is removed. This harness executes both surfaces for the
//! core containment, atomic-write, and mutation-lock contracts. It does not
//! promote a cutover or manufacture command-receipt evidence.

use allow_core::{CargoAllowError, CargoAllowResult};
use allow_policy::extraction_parity::{ParityComparison, ParityObservation, compare_observations};
use allow_policy::starter_policy;
use effortless_repo_edit::{SingleTargetApplyMode, SingleTargetApplyRequest, apply_single_target};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ROOT_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepoEditParityRun {
    pub cases: Vec<RepoEditParityCase>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepoEditParityCase {
    pub id: String,
    pub comparison: ParityComparison,
    pub old_output: String,
    pub new_output: String,
}

/// Execute the live compatibility and direct authorities on equivalent roots.
pub(crate) fn run_repo_edit_parity(root: &Path) -> CargoAllowResult<RepoEditParityRun> {
    let workspace = parity_workspace(root)?;
    let result = run_cases(&workspace);
    let cleanup = fs::remove_dir_all(&workspace);
    result.and_then(|run| {
        cleanup.map_err(|error| {
            CargoAllowError::new(format!(
                "failed to clean RepoEdit parity workspace {}: {error}",
                workspace.display()
            ))
        })?;
        Ok(run)
    })
}

fn run_cases(workspace: &Path) -> CargoAllowResult<RepoEditParityRun> {
    let cases = vec![
        containment_case(workspace),
        atomic_write_case(workspace),
        no_overwrite_case(workspace),
        mutation_lock_case(workspace),
        init_command_case(workspace),
    ]
    .into_iter()
    .collect::<CargoAllowResult<Vec<_>>>()?;
    Ok(RepoEditParityRun { cases })
}

fn init_command_case(workspace: &Path) -> CargoAllowResult<RepoEditParityCase> {
    let old_root = workspace.join("init-old");
    let new_root = workspace.join("init-new");
    fs::create_dir_all(&old_root).map_err(io_error)?;
    fs::create_dir_all(&new_root).map_err(io_error)?;
    let config = PathBuf::from("policy/allow.toml");
    let contents = starter_policy(false, "policy/allow.toml");

    crate::init::cmd_init(&crate::init::parity_init_args(
        old_root.clone(),
        config.clone(),
    ))?;
    let new_path = new_root.join(&config);
    apply_single_target(SingleTargetApplyRequest {
        repository_root: &new_root,
        target: &new_path,
        contents: &contents,
        caller_reference: Some("cargo-allow:init"),
        lock_identity: Some("policy/allow.toml".to_string()),
        mode: SingleTargetApplyMode::CreateNewOnly,
    })
    .into_result()
    .map_err(|error| CargoAllowError::new(format!("new init apply failed: {error}")))?;

    let old_output = fs::read_to_string(old_root.join(&config)).map_err(io_error)?;
    let new_output = fs::read_to_string(new_path).map_err(io_error)?;
    Ok(parity_case(
        "parity-repo-edit-init-command-v1",
        "init:policy/allow.toml",
        old_output,
        new_output,
    ))
}

fn containment_case(workspace: &Path) -> CargoAllowResult<RepoEditParityCase> {
    let old_root = workspace.join("containment-old");
    let new_root = workspace.join("containment-new");
    fs::create_dir_all(&old_root).map_err(io_error)?;
    fs::create_dir_all(&new_root).map_err(io_error)?;
    let old_output = containment_output(
        crate::policy_config::assert_path_within_root(&old_root, &old_root.join("target.toml")),
        &old_root,
    );
    let new_output = containment_output(
        effortless_repo_edit::assert_path_within_root(&new_root, &new_root.join("target.toml")),
        &new_root,
    );
    let old_escape = containment_output(
        crate::policy_config::assert_path_within_root(&old_root, &old_root.join("..")),
        &old_root,
    );
    let new_escape = containment_output(
        effortless_repo_edit::assert_path_within_root(&new_root, &new_root.join("..")),
        &new_root,
    );
    Ok(parity_case(
        "parity-repo-edit-path-containment-v1",
        "containment",
        format!("inside={old_output}|escape={old_escape}"),
        format!("inside={new_output}|escape={new_escape}"),
    ))
}

fn atomic_write_case(workspace: &Path) -> CargoAllowResult<RepoEditParityCase> {
    let old_root = workspace.join("atomic-old");
    let new_root = workspace.join("atomic-new");
    let old_path = old_root.join("nested").join("policy.toml");
    let new_path = new_root.join("nested").join("policy.toml");
    let old_result = crate::command_support::write_file(&old_path, "[policy]\nvalue = 1\n");
    let new_result = effortless_repo_edit::write_file(&new_path, "[policy]\nvalue = 1\n");
    let old_output = write_output(old_result, &old_path);
    let new_output = write_output(new_result, &new_path);
    Ok(parity_case(
        "parity-repo-edit-atomic-write-v1",
        "atomic_write",
        old_output,
        new_output,
    ))
}

fn no_overwrite_case(workspace: &Path) -> CargoAllowResult<RepoEditParityCase> {
    let old_root = workspace.join("no-overwrite-old");
    let new_root = workspace.join("no-overwrite-new");
    let old_path = old_root.join("policy.toml");
    let new_path = new_root.join("policy.toml");
    fs::create_dir_all(&old_root).map_err(io_error)?;
    fs::create_dir_all(&new_root).map_err(io_error)?;
    fs::write(&old_path, "original\n").map_err(io_error)?;
    fs::write(&new_path, "original\n").map_err(io_error)?;
    let old_result =
        crate::command_support::write_file_no_overwrite(&old_path, "replacement\n", true);
    let new_result = apply_single_target(SingleTargetApplyRequest {
        repository_root: &new_root,
        target: &new_path,
        contents: "replacement\n",
        caller_reference: Some("extraction-parity"),
        lock_identity: None,
        mode: SingleTargetApplyMode::ReplaceWithBackup,
    });
    let old_output = backup_output(old_result, &old_path);
    let new_output = backup_receipt_output(new_result, &new_path);
    Ok(parity_case(
        "parity-repo-edit-apply-backup-mode-v1",
        "no_overwrite",
        old_output,
        new_output,
    ))
}

fn backup_output<E: std::fmt::Display>(result: Result<(), E>, path: &Path) -> String {
    match result {
        Ok(()) => file_and_backup_output(path),
        Err(error) => format!("error:{error}"),
    }
}

fn backup_receipt_output(
    response: effortless_repo_edit::SingleTargetApplyResponse,
    path: &Path,
) -> String {
    if response.receipt.applied() {
        file_and_backup_output(path)
    } else {
        format!(
            "error:{}",
            response
                .receipt
                .error_detail
                .as_deref()
                .unwrap_or("single-target apply failed")
        )
    }
}

fn file_and_backup_output(path: &Path) -> String {
    let backup = path.with_extension("toml.bak");
    match (fs::read_to_string(path), fs::read_to_string(backup)) {
        (Ok(contents), Ok(backup)) => format!("target={contents}|backup={backup}"),
        (target, backup) => format!("target={target:?}|backup={backup:?}"),
    }
}

fn mutation_lock_case(workspace: &Path) -> CargoAllowResult<RepoEditParityCase> {
    let old_target = workspace.join("lock-old").join("policy.toml");
    let new_target = workspace.join("lock-new").join("policy.toml");
    let old_result = crate::mutation_lock::MutationLock::acquire_with_timeout(
        &old_target,
        std::time::Duration::from_secs(1),
    );
    let new_result = effortless_repo_edit::MutationLock::acquire_with_timeout(
        &new_target,
        std::time::Duration::from_secs(1),
    );
    let old_output = lock_output(old_result);
    let new_output = lock_output(new_result);
    Ok(parity_case(
        "parity-repo-edit-mutation-lock-alias-v1",
        "mutation_lock",
        old_output,
        new_output,
    ))
}

fn parity_workspace(root: &Path) -> CargoAllowResult<PathBuf> {
    let id = NEXT_ROOT_ID.fetch_add(1, Ordering::Relaxed);
    let workspace = root.join("target").join(format!(
        "cargo-allow-repo-edit-parity-{}-{id}",
        std::process::id()
    ));
    fs::create_dir_all(&workspace).map_err(io_error)?;
    Ok(workspace)
}

fn parity_case(
    id: &str,
    source_identity: &str,
    old_output: String,
    new_output: String,
) -> RepoEditParityCase {
    let comparison = compare_observations(
        &ParityObservation {
            source_identity: source_identity.to_string(),
            canonical_output: old_output.clone(),
        },
        &ParityObservation {
            source_identity: source_identity.to_string(),
            canonical_output: new_output.clone(),
        },
    );
    RepoEditParityCase {
        id: id.to_string(),
        comparison,
        old_output,
        new_output,
    }
}

fn containment_output<T, E: std::fmt::Display>(result: Result<T, E>, root: &Path) -> String {
    match result {
        Ok(_) => "ok".to_string(),
        Err(error) => error
            .to_string()
            .replace(&*root.to_string_lossy(), "<root>"),
    }
}

fn write_output<E: std::fmt::Display>(result: Result<(), E>, path: &Path) -> String {
    match result {
        Ok(()) => match fs::read_to_string(path) {
            Ok(contents) => format!("ok:{contents}"),
            Err(error) => format!("read_error:{error}"),
        },
        Err(error) => error
            .to_string()
            .replace(&*path.to_string_lossy(), "<target>"),
    }
}

fn lock_output<T, E: std::fmt::Display>(result: Result<T, E>) -> String {
    match result {
        Ok(_) => "ok".to_string(),
        Err(error) => error.to_string(),
    }
}

fn io_error(error: std::io::Error) -> CargoAllowError {
    CargoAllowError::new(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_policy::extraction_parity::ParityComparisonResult;

    #[test]
    fn repo_edit_authorities_are_parity_equivalent() -> Result<(), String> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let run = run_repo_edit_parity(&root).map_err(|error| error.to_string())?;
        for case in run.cases {
            if case.comparison.result != ParityComparisonResult::SemanticallyEquivalent {
                return Err(format!(
                    "{} parity differed: {:?}",
                    case.id, case.comparison
                ));
            }
            if case.old_output != case.new_output {
                return Err(format!("{} canonical outputs differed", case.id));
            }
        }
        Ok(())
    }
}
