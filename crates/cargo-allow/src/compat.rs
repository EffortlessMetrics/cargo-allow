use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult, Finding, FindingKind};
use allow_inventory::{InventorySource, resolve_source_tree_root};
use std::env;
use std::path::{Path, PathBuf};

#[path = "compat_paths.rs"]
mod compat_paths;
#[path = "compat_scan.rs"]
mod compat_scan;

use crate::{
    FamilyFilter, InventoryFacts, KindFilter, is_clippy_compat_kind,
    is_dependency_surface_compat_kind, is_executable_compat_kind, is_network_compat_kind,
    is_no_panic_allowlist_compat_kind, is_panic_compat_kind, is_process_compat_kind,
    is_unsafe_compat_kind, is_workflow_compat_kind, parse_kind_filter,
};
use compat_paths::compat_policy_path;
use compat_scan::{scan_legacy_rust_compat, scan_non_rust_compat};

pub(crate) fn load_compat_world(
    explicit_root: Option<&Path>,
    config: Option<&Path>,
    kind_filter: Option<&str>,
    include_untracked: bool,
) -> CargoAllowResult<(PathBuf, AllowConfig, Vec<Finding>, InventoryFacts)> {
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
        let policy_path = compat_policy_path(config, &root, "policy/no-panic-allowlist.toml");
        let cfg = allow_policy_legacy::load_no_panic_allowlist_compat_config(policy_path)?;
        let (findings, inventory_facts) =
            scan_legacy_rust_compat(&root, &cfg, include_untracked, FindingKind::Panic)?;
        return Ok((root, cfg, findings, inventory_facts));
    }
    if is_panic_compat_kind(compat_kind) {
        let policy_path = compat_policy_path(config, &root, "policy/no-panic-baseline.toml");
        let cfg = allow_policy_legacy::load_no_panic_baseline_compat_config(policy_path)?;
        let (findings, inventory_facts) =
            scan_legacy_rust_compat(&root, &cfg, include_untracked, FindingKind::Panic)?;
        return Ok((root, cfg, findings, inventory_facts));
    }
    if is_clippy_compat_kind(compat_kind) {
        let policy_path = compat_policy_path(config, &root, "policy/clippy-exceptions.toml");
        let cfg = allow_policy_legacy::load_clippy_exceptions_compat_config(policy_path)?;
        let (findings, inventory_facts) =
            scan_legacy_rust_compat(&root, &cfg, include_untracked, FindingKind::LintException)?;
        return Ok((root, cfg, findings, inventory_facts));
    }
    if is_unsafe_compat_kind(compat_kind) {
        let policy_path = compat_policy_path(config, &root, "policy/unsafe-allowlist.toml");
        let cfg = allow_policy_legacy::load_unsafe_allowlist_compat_config(policy_path)?;
        let (findings, inventory_facts) =
            scan_legacy_rust_compat(&root, &cfg, include_untracked, FindingKind::Unsafe)?;
        return Ok((root, cfg, findings, inventory_facts));
    }
    if is_executable_compat_kind(compat_kind) {
        let policy_path = compat_policy_path(config, &root, "policy/executable-allowlist.toml");
        let cfg = allow_policy_legacy::load_executable_compat_config(policy_path)?;
        let findings = allow_policy_legacy::executable_findings_from_git(&root)?;
        return Ok((
            root,
            cfg,
            findings,
            InventoryFacts::source_only(InventorySource::FilesystemFallback),
        ));
    }
    if is_workflow_compat_kind(compat_kind) {
        let policy_path = compat_policy_path(config, &root, "policy/workflow-allowlist.toml");
        let cfg = allow_policy_legacy::load_workflow_compat_config(policy_path)?;
        let findings = allow_policy_legacy::workflow_findings_from_files(&root)?;
        return Ok((
            root,
            cfg,
            findings,
            InventoryFacts::source_only(InventorySource::GitTracked),
        ));
    }
    if is_dependency_surface_compat_kind(compat_kind) {
        let policy_path =
            compat_policy_path(config, &root, "policy/dependency-surface-allowlist.toml");
        let cfg = allow_policy_legacy::load_dependency_surface_compat_config(policy_path)?;
        let findings = allow_policy_legacy::dependency_surface_findings_from_git(&root, &cfg)?;
        return Ok((
            root,
            cfg,
            findings,
            InventoryFacts::source_only(InventorySource::GitTracked),
        ));
    }
    if is_process_compat_kind(compat_kind) {
        let policy_path = compat_policy_path(config, &root, "policy/process-allowlist.toml");
        let cfg = allow_policy_legacy::load_process_compat_config(policy_path)?;
        let findings = allow_policy_legacy::process_findings_from_config(&cfg);
        return Ok((
            root,
            cfg,
            findings,
            InventoryFacts::source_only(InventorySource::FilesystemFallback),
        ));
    }
    if is_network_compat_kind(compat_kind) {
        let policy_path = compat_policy_path(config, &root, "policy/network-allowlist.toml");
        let cfg = allow_policy_legacy::load_network_compat_config(policy_path)?;
        let findings = allow_policy_legacy::network_findings_from_config(&cfg);
        return Ok((
            root,
            cfg,
            findings,
            InventoryFacts::source_only(InventorySource::FilesystemFallback),
        ));
    }
    if parsed_filter.kind == FindingKind::GeneratedCode {
        let policy_path = compat_policy_path(config, &root, "policy/generated-allowlist.toml");
        let cfg = allow_policy_legacy::load_generated_compat_config(policy_path)?;
        let findings = allow_policy_legacy::generated_findings_from_gitattributes(&root)?;
        return Ok((
            root,
            cfg,
            findings,
            InventoryFacts::source_only(InventorySource::FilesystemFallback),
        ));
    }
    if parsed_filter.kind != FindingKind::NonRustFile {
        return Err(CargoAllowError::new(
            "--compat currently supports only --kind non-rust, --kind generated, --kind panic, --kind no-panic-allowlist, --kind lint-exception, --kind unsafe, --kind executable, --kind workflow, --kind dependency-surface, --kind process, or --kind network",
        ));
    }
    let (findings, inventory_facts) = scan_non_rust_compat(&root, include_untracked)?;
    let policy_path = compat_policy_path(config, &root, "policy/non-rust-allowlist.toml");
    let cfg = allow_policy_legacy::load_non_rust_compat_config(policy_path, &findings)?;
    Ok((root, cfg, findings, inventory_facts))
}
