#[cfg(test)]
pub(crate) use crate::cli::{CargoAllowCli, CargoAllowCommand, normalized_args};
pub(crate) use crate::cli_types::{
    HumanJsonFormat, InventoryFacts, OutputFormat, ProfileArg, RootArgs, parse_match_status_arg,
};
pub(crate) use crate::companion::{canonical_companion_findings, extend_unique_findings};
pub(crate) use crate::compat::load_compat_world;
pub(crate) use crate::kind_filter::{
    FamilyFilter, KindFilter, is_clippy_compat_kind, is_dependency_surface_compat_kind,
    is_executable_compat_kind, is_network_compat_kind, is_no_panic_allowlist_compat_kind,
    is_panic_compat_kind, is_process_compat_kind, is_unsafe_compat_kind, is_workflow_compat_kind,
    parse_kind_filter, parse_kind_filter_arg,
};
pub(crate) use crate::mutation_lock::MutationLock;
pub(crate) use crate::policy_config::{
    EvidenceValidationMode, assert_path_within_root, config_path, git_relative_config_path,
    load_policy_at_path, portable_relative_under_root, root_relative_path,
};
pub(crate) use allow_core::CargoAllowResult;
pub(crate) use repo_edit::{write_file, write_file_no_overwrite};
pub(crate) use std::path::Path;

pub(crate) use crate::reporting::{
    EvidenceReportSummary, ReportRenderArgs, SourceTreeReportContext, print_report, report_config,
};
pub(crate) use crate::selector::selector_from_finding;
pub(crate) use crate::world::{load_world, load_world_for_path, load_world_with_evidence_mode};
pub(crate) use allow_inventory::resolve_source_tree_root;
pub(crate) use allow_report::policy_baseline_debt_entries;

/// Centralized current-dir reader (#2824). Replaces 20+ copy-pasted
/// `env::current_dir().map_err(|e| CargoAllowError::new(...))` sites.
pub(crate) fn current_dir() -> CargoAllowResult<std::path::PathBuf> {
    std::env::current_dir().map_err(|e| {
        allow_core::CargoAllowError::new(format!("failed to read cwd: {e}"))
    })
}

pub(crate) fn emit_text(output: Option<&Path>, contents: &str) -> CargoAllowResult<()> {
    if let Some(path) = output {
        write_file(path, contents)?;
    } else {
        println!("{contents}");
    }
    Ok(())
}

pub(crate) fn emit_stderr_text(output: Option<&Path>, contents: &str) -> CargoAllowResult<()> {
    if let Some(path) = output {
        write_file(path, contents)?;
    } else {
        eprintln!("{contents}");
    }
    Ok(())
}

#[cfg(test)]
mod io_tests {
    use super::*;
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
    fn emit_stderr_text_writes_to_output_path() -> Result<(), Box<dyn std::error::Error>> {
        let root = TempRoot::new("emit-stderr-text")?;
        let output = root.path().join("nested/summary.txt");

        let result = emit_stderr_text(Some(&output), "summary\n");

        assert!(result.is_ok());
        assert_eq!(fs::read_to_string(&output)?, "summary\n");
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
