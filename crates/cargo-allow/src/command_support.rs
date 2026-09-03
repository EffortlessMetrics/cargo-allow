#[cfg(test)]
pub(crate) use crate::cli::{CargoAllowCli, CargoAllowCommand, normalized_args};
pub(crate) use crate::cli_types::{
    HumanJsonFormat, InventoryFacts, OutputFormat, ProfileArg, RootArgs, parse_match_status_arg,
};
pub(crate) use crate::companion::{canonical_companion_findings, extend_unique_findings};
pub(crate) use crate::compat::load_compat_world;
pub(crate) use crate::core_command_router::print_report;
pub(crate) use crate::kind_filter::{
    FamilyFilter, KindFilter, is_clippy_compat_kind, is_dependency_surface_compat_kind,
    is_executable_compat_kind, is_network_compat_kind, is_no_panic_allowlist_compat_kind,
    is_panic_compat_kind, is_process_compat_kind, is_unsafe_compat_kind, is_workflow_compat_kind,
    parse_kind_filter, parse_kind_filter_arg,
};
pub(crate) use crate::mutation_lock::MutationLock;
pub(crate) use crate::policy_config::{
    EvidenceValidationMode, assert_path_within_root, config_path, git_relative_config_path,
    portable_relative_under_root, root_relative_path,
};
pub(crate) use allow_core::CargoAllowResult;
pub(crate) use effortless_repo_edit::{write_file, write_file_no_overwrite};
use effortless_repo_snapshot::{SnapshotError, SnapshotErrorKind};
pub(crate) use std::path::Path;

pub(crate) use crate::reporting::{
    EvidenceReportSummary, ReportRenderArgs, SourceTreeReportContext, report_config,
};
pub(crate) use crate::selector::selector_from_finding;
#[cfg(test)]
pub(crate) use crate::world::load_world;
pub(crate) use crate::world::{
    load_read_only_world, load_read_only_world_and_cache,
    load_read_only_world_with_selected_policy, load_staged_world, load_world_for_path,
    load_world_from_resolved_policy_with_options, load_world_with_evidence_mode,
    load_world_without_policy_after_selection,
};
pub(crate) use allow_inventory::resolve_source_tree_root;
pub(crate) use allow_report::policy_baseline_debt_entries;

pub(crate) fn snapshot_error(error: SnapshotError) -> allow_core::CargoAllowError {
    let kind = match error.kind() {
        SnapshotErrorKind::Internal => allow_core::CargoAllowErrorKind::Internal,
        SnapshotErrorKind::InvalidConfig => allow_core::CargoAllowErrorKind::InvalidConfig,
        SnapshotErrorKind::Inventory => allow_core::CargoAllowErrorKind::Inventory,
        SnapshotErrorKind::Artifact => allow_core::CargoAllowErrorKind::Artifact,
        SnapshotErrorKind::Unknown => allow_core::CargoAllowErrorKind::Unknown,
        SnapshotErrorKind::Scan => allow_core::CargoAllowErrorKind::Scan,
    };
    allow_core::CargoAllowError::with_kind(kind, error.to_string())
}

pub(crate) fn snapshot_result<T>(
    result: effortless_repo_snapshot::SnapshotResult<T>,
) -> allow_core::CargoAllowResult<T> {
    result.map_err(snapshot_error)
}

/// Centralized current-dir reader (#2824). Replaces 20+ copy-pasted
/// `env::current_dir().map_err(|e| CargoAllowError::new(...))` sites.
pub(crate) fn current_dir() -> CargoAllowResult<std::path::PathBuf> {
    current_dir_with_prefix("failed to read cwd: ")
}

pub(crate) fn current_dir_with_prefix(prefix: &str) -> CargoAllowResult<std::path::PathBuf> {
    std::env::current_dir().map_err(|error| current_dir_error(error, prefix))
}

fn current_dir_error(error: std::io::Error, prefix: &str) -> allow_core::CargoAllowError {
    allow_core::CargoAllowError::from(error).with_message_prefix(prefix)
}

pub(crate) fn emit_scan_status(
    command: &str,
    format: OutputFormat,
    output: Option<&Path>,
    receipt: Option<&Path>,
) {
    let quiet = std::env::var_os("CARGO_ALLOW_QUIET").is_some();
    if should_emit_scan_status(format, output, receipt, quiet) {
        eprintln!("cargo-allow {command}: scanning...");
    }
}

fn should_emit_scan_status(
    format: OutputFormat,
    output: Option<&Path>,
    receipt: Option<&Path>,
    quiet: bool,
) -> bool {
    format == OutputFormat::Human && output.is_none() && receipt.is_none() && !quiet
}

pub(crate) fn emit_text(output: Option<&Path>, contents: &str) -> CargoAllowResult<()> {
    if let Some(path) = output {
        write_file(path, contents)
            .map_err(crate::extraction_repo_edit_runtime::map_repo_edit_error)?;
    } else {
        println!("{contents}");
    }
    Ok(())
}

/// Reject a legacy per-command summary path that aliases a file the command
/// is about to mutate. These summaries are emitted after the mutation, so the
/// check must use the same canonical target identity as the mutation lock.
pub(crate) fn reject_legacy_summary_output_collision(
    repository_root: &Path,
    summary_output: Option<&Path>,
    mutation_targets: &[&Path],
) -> CargoAllowResult<()> {
    reject_output_collision(
        repository_root,
        summary_output,
        mutation_targets,
        "--summary-output must differ from the candidate or live policy output",
    )
}

pub(crate) fn reject_output_collision(
    repository_root: &Path,
    output: Option<&Path>,
    mutation_targets: &[&Path],
    message: &'static str,
) -> CargoAllowResult<()> {
    let Some(output) = output else {
        return Ok(());
    };
    let current = current_dir()?;
    let output_absolute = if output.is_absolute() {
        output.to_path_buf()
    } else {
        current.join(output)
    };
    let output_target =
        effortless_repo_edit::resolve_mutation_target(&output_absolute, repository_root)
            .map_err(crate::extraction_repo_edit_runtime::map_repo_edit_error)?;
    for mutation_target in mutation_targets {
        let target =
            effortless_repo_edit::resolve_mutation_target(mutation_target, repository_root)
                .map_err(crate::extraction_repo_edit_runtime::map_repo_edit_error)?;
        if target.target_fingerprint() == output_target.target_fingerprint() {
            return Err(allow_core::CargoAllowError::with_kind(
                allow_core::CargoAllowErrorKind::Usage,
                message,
            ));
        }
    }
    Ok(())
}

pub(crate) fn emit_stderr_text(output: Option<&Path>, contents: &str) -> CargoAllowResult<()> {
    if let Some(path) = output {
        write_file(path, contents)
            .map_err(crate::extraction_repo_edit_runtime::map_repo_edit_error)?;
    } else {
        eprintln!("{contents}");
    }
    Ok(())
}

/// Keep machine-readable summaries out of a mixed stderr stream. Commands
/// such as `add` and `migrate` may also emit human policy or warning text, so
/// a JSON summary without an explicit file target cannot be consumed safely.
pub(crate) fn require_json_summary_output(
    format: HumanJsonFormat,
    output: Option<&Path>,
) -> CargoAllowResult<()> {
    if format == HumanJsonFormat::Json && output.is_none() {
        return Err(allow_core::CargoAllowError::with_kind(
            allow_core::CargoAllowErrorKind::Usage,
            "--summary-format json requires --summary-output <path> to keep machine-readable output separate",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod io_tests {
    use super::*;
    use effortless_repo_snapshot::SnapshotErrorKind;
    use std::fs;
    use std::path::{Path, PathBuf};

    #[test]
    fn emit_text_writes_to_output_path() -> Result<(), Box<dyn std::error::Error>> {
        let root = TempRoot::new("emit-text")?;
        let output = root.path().join("nested/report.txt");

        let result = emit_text(Some(&output), "hello report\n");

        assert!(result.is_ok());
        assert_eq!(fs::read_to_string(&output)?, "hello report\n");
        Ok(())
    }

    #[test]
    fn current_dir_error_preserves_io_kind_and_source() -> Result<(), Box<dyn std::error::Error>> {
        let error = current_dir_error(
            std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "directory access denied",
            ),
            "failed to read cwd: ",
        );

        assert_eq!(error.kind(), allow_core::CargoAllowErrorKind::Inventory);
        assert!(error.to_string().contains("failed to read cwd"));
        use std::error::Error as _;
        assert!(error.source().is_some());
        Ok(())
    }

    #[test]
    fn snapshot_result_projects_neutral_error() {
        let result = snapshot_result::<()>(Err(SnapshotError::with_kind(
            SnapshotErrorKind::Inventory,
            "staged snapshot unavailable",
        )));
        assert_eq!(
            result.expect_err("snapshot error should project").kind(),
            allow_core::CargoAllowErrorKind::Inventory
        );
    }

    #[test]
    fn current_dir_error_preserves_custom_prefix() -> Result<(), Box<dyn std::error::Error>> {
        let error = current_dir_error(
            std::io::Error::new(std::io::ErrorKind::NotFound, "directory disappeared"),
            "failed to read current directory: ",
        );

        assert_eq!(error.kind(), allow_core::CargoAllowErrorKind::InvalidConfig);
        assert!(
            error
                .to_string()
                .starts_with("failed to read current directory:")
        );
        use std::error::Error as _;
        assert!(error.source().is_some());
        Ok(())
    }

    #[test]
    fn emit_stderr_text_writes_to_output_path() -> Result<(), Box<dyn std::error::Error>> {
        let root = TempRoot::new("emit-stderr-text")?;
        let output = root.path().join("nested/summary.txt");

        let result = emit_stderr_text(Some(&output), "summary\n");

        assert!(result.is_ok());
        assert_eq!(fs::read_to_string(&output)?, "summary\n");
        Ok(())
    }

    #[test]
    fn legacy_summary_collision_rejects_canonical_alias_before_write() -> Result<(), String> {
        let root = TempRoot::new("legacy-summary-collision").map_err(|error| error.to_string())?;
        let policy_dir = root.path().join("policy");
        fs::create_dir_all(&policy_dir).map_err(|error| error.to_string())?;
        let policy = policy_dir.join("allow.toml");
        fs::write(&policy, "original policy\n").map_err(|error| error.to_string())?;
        let summary_alias = policy_dir.join(".").join("allow.toml");

        let result = reject_legacy_summary_output_collision(
            root.path(),
            Some(&summary_alias),
            &[policy.as_path()],
        );
        let error = match result {
            Ok(()) => return Err("canonical summary collision was accepted".to_string()),
            Err(error) => error,
        };
        if error.kind() != allow_core::CargoAllowErrorKind::Usage {
            return Err(format!("unexpected collision error kind: {}", error.code()));
        }
        let contents = fs::read_to_string(&policy).map_err(|error| error.to_string())?;
        if contents != "original policy\n" {
            return Err("collision preflight changed the policy sentinel".to_string());
        }
        Ok(())
    }

    #[test]
    fn generic_output_collision_rejects_direct_and_alias_paths() -> Result<(), String> {
        let root = TempRoot::new("generic-output-collision").map_err(|error| error.to_string())?;
        let policy_dir = root.path().join("policy");
        fs::create_dir_all(&policy_dir).map_err(|error| error.to_string())?;
        let policy = policy_dir.join("allow.toml");
        fs::write(&policy, "original policy\n").map_err(|error| error.to_string())?;
        let message = "--output must differ from the selected policy output";

        for output in [policy.clone(), policy_dir.join(".").join("allow.toml")] {
            let result =
                reject_output_collision(root.path(), Some(&output), &[policy.as_path()], message);
            let error = match result {
                Ok(()) => return Err(format!("accepted output collision: {}", output.display())),
                Err(error) => error,
            };
            if error.kind() != allow_core::CargoAllowErrorKind::Usage
                || error.to_string() != message
            {
                return Err(format!("unexpected collision error: {}", error.code()));
            }
        }
        if fs::read_to_string(&policy).map_err(|error| error.to_string())? != "original policy\n" {
            return Err("collision preflight changed the policy sentinel".to_string());
        }
        Ok(())
    }

    #[test]
    fn json_summary_requires_an_explicit_output_path() -> Result<(), Box<dyn std::error::Error>> {
        let error = require_json_summary_output(HumanJsonFormat::Json, None)
            .expect_err("JSON summaries must not share a human stderr stream");
        assert_eq!(
            error.to_string(),
            "--summary-format json requires --summary-output <path> to keep machine-readable output separate"
        );
        require_json_summary_output(HumanJsonFormat::Json, Some(Path::new("summary.json")))?;
        require_json_summary_output(HumanJsonFormat::Human, None)?;
        Ok(())
    }

    #[test]
    fn scan_status_is_limited_to_human_terminal_output() -> Result<(), String> {
        let output = Path::new("target/report.txt");
        emit_scan_status("test", OutputFormat::Human, None, None);
        if !should_emit_scan_status(OutputFormat::Human, None, None, false) {
            return Err("human terminal output should emit scan status".to_string());
        }
        if should_emit_scan_status(OutputFormat::Human, Some(output), None, false) {
            return Err("file-backed output should stay free of scan status".to_string());
        }
        if should_emit_scan_status(OutputFormat::Human, None, Some(output), false) {
            return Err("receipt-backed output should stay free of scan status".to_string());
        }
        if should_emit_scan_status(OutputFormat::Json, None, None, false) {
            return Err("JSON output should stay free of scan status".to_string());
        }
        if should_emit_scan_status(OutputFormat::Human, None, None, true) {
            return Err("quiet output should stay free of scan status".to_string());
        }
        Ok(())
    }

    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn new(label: &str) -> std::io::Result<Self> {
            let unique = format!(
                "cargo-allow-io-{label}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_else(|err| {
                        std::panic::panic_any(format!("system time before epoch: {err}"))
                    })
                    .as_nanos()
            );
            let path = std::env::temp_dir().join(unique);
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path)?;
            Ok(Self { path })
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
