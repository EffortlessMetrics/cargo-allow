use allow_core::{CargoAllowError, CargoAllowResult, FindingKind, normalize_path};
use allow_inventory::{InventoryOptions, inventory, resolve_source_tree_root};
use std::env;
use std::path::{Path, PathBuf};

use super::migrate_types::{MigrateContext, MigrationLoad};
use crate::root_relative_path;

pub(super) fn load_single_file_migration_config(
    explicit_root: Option<&Path>,
    from: &Path,
) -> CargoAllowResult<MigrationLoad> {
    let root = explicit_root
        .map(|root| resolve_source_tree_root(Some(root), root))
        .transpose()?;
    Ok(MigrationLoad {
        cfg: allow_policy_legacy::load_legacy_or_canonical(from)?,
        context: MigrateContext {
            inventory_source: "unknown".to_string(),
            source_tree_root: root.as_deref().map(allow_report::source_tree_path_text),
            inventory_files: None,
            input_kind: "from".to_string(),
            input_path: normalize_path(from),
        },
    })
}

pub(super) fn load_repo_policy_migration_config(
    explicit_root: Option<&Path>,
    repo_policy: &Path,
) -> CargoAllowResult<MigrationLoad> {
    let root = repo_policy_source_tree_root(explicit_root, repo_policy)?;
    let repo_policy = root_relative_path(&root, repo_policy);
    let inventory = inventory(&root, &InventoryOptions::default())?;
    let inventory_source = inventory.source;
    let files_scanned = inventory.files.len();
    let findings = allow_files::scan_files(&inventory.files)
        .into_iter()
        .filter(|finding| finding.kind == FindingKind::NonRustFile)
        .collect::<Vec<_>>();
    let cfg = allow_policy_legacy::load_legacy_policy_dir_with_non_rust_findings(
        &repo_policy,
        &findings,
    )?;
    Ok(MigrationLoad {
        cfg,
        context: MigrateContext {
            inventory_source: inventory_source.as_str().to_string(),
            source_tree_root: Some(allow_report::source_tree_path_text(&root)),
            inventory_files: Some(files_scanned),
            input_kind: "repo_policy".to_string(),
            input_path: normalize_path(&repo_policy),
        },
    })
}

fn repo_policy_source_tree_root(
    explicit_root: Option<&Path>,
    repo_policy: &Path,
) -> CargoAllowResult<PathBuf> {
    if let Some(root) = explicit_root {
        return resolve_source_tree_root(Some(root), root);
    }
    let cwd =
        env::current_dir().map_err(|e| CargoAllowError::new(format!("failed to read cwd: {e}")))?;
    let full_policy_path = if repo_policy.is_absolute() {
        repo_policy.to_path_buf()
    } else {
        cwd.join(repo_policy)
    };
    if full_policy_path.file_name().and_then(|name| name.to_str()) == Some("policy") {
        return full_policy_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| {
                CargoAllowError::new(format!(
                    "failed to infer repository root from {}",
                    repo_policy.display()
                ))
            });
    }
    resolve_source_tree_root(None, cwd)
}
