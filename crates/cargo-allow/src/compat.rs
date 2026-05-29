use allow_core::{CargoAllowError, CargoAllowResult, FindingKind};
use allow_inventory::{InventorySource, resolve_source_tree_root};
use std::env;
use std::path::Path;

#[path = "compat_paths.rs"]
pub(crate) mod compat_paths;
#[path = "compat_scan.rs"]
pub(crate) mod compat_scan;

use crate::compat_profiles::{finding_source_world, legacy_rust_world};
use crate::compat_world::{CompatWorld, compat_world};
use crate::{
    FamilyFilter, KindFilter, is_clippy_compat_kind, is_dependency_surface_compat_kind,
    is_executable_compat_kind, is_network_compat_kind, is_no_panic_allowlist_compat_kind,
    is_panic_compat_kind, is_process_compat_kind, is_unsafe_compat_kind, is_workflow_compat_kind,
    parse_kind_filter,
};
use compat_scan::scan_non_rust_compat;

pub(crate) fn load_compat_world(
    explicit_root: Option<&Path>,
    config: Option<&Path>,
    kind_filter: Option<&str>,
    include_untracked: bool,
) -> CargoAllowResult<CompatWorld> {
    let compat_kind = kind_filter.unwrap_or("non-rust");
    let parsed_filter = kind_filter
        .map(parse_kind_filter)
        .transpose()?
        .unwrap_or(KindFilter {
            kind: FindingKind::NonRustFile,
            family: FamilyFilter::Any,
        });
    let cwd =
        env::current_dir().map_err(|e| CargoAllowError::new(format!("failed to read cwd: {e}")))?;
    let root = resolve_source_tree_root(explicit_root, cwd)?;
    if is_no_panic_allowlist_compat_kind(compat_kind) {
        return legacy_rust_world(
            config,
            &root,
            "policy/no-panic-allowlist.toml",
            allow_policy_legacy::load_no_panic_allowlist_compat_config,
            include_untracked,
            FindingKind::Panic,
        );
    }
    if is_panic_compat_kind(compat_kind) {
        return legacy_rust_world(
            config,
            &root,
            "policy/no-panic-baseline.toml",
            allow_policy_legacy::load_no_panic_baseline_compat_config,
            include_untracked,
            FindingKind::Panic,
        );
    }
    if is_clippy_compat_kind(compat_kind) {
        return legacy_rust_world(
            config,
            &root,
            "policy/clippy-exceptions.toml",
            allow_policy_legacy::load_clippy_exceptions_compat_config,
            include_untracked,
            FindingKind::LintException,
        );
    }
    if is_unsafe_compat_kind(compat_kind) {
        return legacy_rust_world(
            config,
            &root,
            "policy/unsafe-allowlist.toml",
            allow_policy_legacy::load_unsafe_allowlist_compat_config,
            include_untracked,
            FindingKind::Unsafe,
        );
    }
    if is_executable_compat_kind(compat_kind) {
        return finding_source_world(
            config,
            &root,
            "policy/executable-allowlist.toml",
            allow_policy_legacy::load_executable_compat_config,
            |root, _cfg| allow_policy_legacy::executable_findings_from_git(root),
            InventorySource::FilesystemFallback,
        );
    }
    if is_workflow_compat_kind(compat_kind) {
        return finding_source_world(
            config,
            &root,
            "policy/workflow-allowlist.toml",
            allow_policy_legacy::load_workflow_compat_config,
            |root, _cfg| allow_policy_legacy::workflow_findings_from_files(root),
            InventorySource::GitTracked,
        );
    }
    if is_dependency_surface_compat_kind(compat_kind) {
        return finding_source_world(
            config,
            &root,
            "policy/dependency-surface-allowlist.toml",
            allow_policy_legacy::load_dependency_surface_compat_config,
            |root, cfg| allow_policy_legacy::dependency_surface_findings_from_git(root, cfg),
            InventorySource::GitTracked,
        );
    }
    if is_process_compat_kind(compat_kind) {
        return finding_source_world(
            config,
            &root,
            "policy/process-allowlist.toml",
            allow_policy_legacy::load_process_compat_config,
            |_root, cfg| Ok(allow_policy_legacy::process_findings_from_config(cfg)),
            InventorySource::FilesystemFallback,
        );
    }
    if is_network_compat_kind(compat_kind) {
        return finding_source_world(
            config,
            &root,
            "policy/network-allowlist.toml",
            allow_policy_legacy::load_network_compat_config,
            |_root, cfg| Ok(allow_policy_legacy::network_findings_from_config(cfg)),
            InventorySource::FilesystemFallback,
        );
    }
    if parsed_filter.kind == FindingKind::GeneratedCode {
        return finding_source_world(
            config,
            &root,
            "policy/generated-allowlist.toml",
            allow_policy_legacy::load_generated_compat_config,
            |root, _cfg| allow_policy_legacy::generated_findings_from_gitattributes(root),
            InventorySource::FilesystemFallback,
        );
    }
    if parsed_filter.kind != FindingKind::NonRustFile {
        return Err(CargoAllowError::new(
            "--compat currently supports only --kind non-rust, --kind generated, --kind panic, --kind no-panic-allowlist, --kind lint-exception, --kind unsafe, --kind executable, --kind workflow, --kind dependency-surface, --kind process, or --kind network",
        ));
    }
    let (findings, inventory_facts) = scan_non_rust_compat(&root, include_untracked)?;
    let policy_path = crate::compat::compat_paths::compat_policy_path(
        config,
        &root,
        "policy/non-rust-allowlist.toml",
    );
    let cfg = allow_policy_legacy::load_non_rust_compat_config(policy_path, &findings)?;
    Ok(compat_world(root, cfg, findings, inventory_facts))
}
