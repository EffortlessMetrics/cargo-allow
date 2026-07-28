use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult, Finding, FindingKind};
use allow_inventory::{InventoryOptions, InventorySource, inventory, resolve_source_tree_root};
use std::path::{Path, PathBuf};

#[path = "compat_paths.rs"]
mod compat_paths;
#[path = "compat_scan.rs"]
mod compat_scan;

use crate::{
    FamilyFilter, InventoryFacts, KindFilter, current_dir, is_clippy_compat_kind,
    is_dependency_surface_compat_kind, is_executable_compat_kind, is_network_compat_kind,
    is_no_panic_allowlist_compat_kind, is_panic_compat_kind, is_process_compat_kind,
    is_unsafe_compat_kind, is_workflow_compat_kind, parse_kind_filter,
};
use compat_paths::compat_policy_path;
use compat_scan::{scan_legacy_rust_compat, scan_non_rust_compat};

/// Whether a compat surface ignores `--include-untracked`.
///
/// The rust compat kinds, the dependency-surface kind, and the default
/// non-rust surface all scan the file inventory, so the flag is meaningful
/// there. The executable/workflow/generated surfaces read a fixed git or
/// `.gitattributes` source, and the process/network surfaces derive findings
/// from policy config, so the flag has no effect for them (#1948).
fn compat_kind_ignores_include_untracked(compat_kind: &str, parsed_filter: &KindFilter) -> bool {
    is_executable_compat_kind(compat_kind)
        || is_workflow_compat_kind(compat_kind)
        || is_process_compat_kind(compat_kind)
        || is_network_compat_kind(compat_kind)
        || parsed_filter.kind == FindingKind::GeneratedCode
}

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
    // The executable/workflow/generated compat surfaces read a fixed git or
    // .gitattributes source, and the process/network surfaces derive findings
    // from policy config, so `--include-untracked` would be silently ignored.
    // Fail closed rather than accept a flag that does nothing (#1948).
    if include_untracked && compat_kind_ignores_include_untracked(compat_kind, &parsed_filter) {
        return Err(CargoAllowError::new(format!(
            "--include-untracked has no effect for --compat --kind {compat_kind}: this compat surface scans a fixed source (git-tracked files, .gitattributes, or policy config), so untracked files are never inventoried; re-run without --include-untracked"
        )));
    }
    let cwd = current_dir()?;
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
        let inventory = inventory(
            &root,
            &InventoryOptions {
                include_untracked,
                ..InventoryOptions::default()
            },
        )?;
        let findings =
            allow_policy_legacy::dependency_surface_findings_from_paths(&inventory.files, &cfg);
        return Ok((
            root,
            cfg,
            findings,
            InventoryFacts::scanned_inventory(&inventory),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_compat_world_rejects_no_effect_include_untracked() {
        // #1948: compat surfaces that never inventory untracked files must
        // reject --include-untracked instead of silently ignoring it. The
        // rejection happens before any filesystem access, so no fixture repo is
        // needed.
        for kind in ["executable", "workflow", "process", "network", "generated"] {
            let err = load_compat_world(None, None, Some(kind), true)
                .expect_err("compat kind should reject a no-op --include-untracked");
            assert!(
                err.to_string()
                    .contains("--include-untracked has no effect"),
                "{kind}: unexpected error: {err}"
            );
        }
    }

    #[test]
    fn compat_kind_ignores_include_untracked_discriminates_surfaces() {
        let non_rust = KindFilter {
            kind: FindingKind::NonRustFile,
            family: FamilyFilter::Any,
        };
        let generated = KindFilter {
            kind: FindingKind::GeneratedCode,
            family: FamilyFilter::Any,
        };
        // Fixed-source / config-only surfaces ignore the flag.
        assert!(compat_kind_ignores_include_untracked(
            "executable",
            &non_rust
        ));
        assert!(compat_kind_ignores_include_untracked("workflow", &non_rust));
        assert!(compat_kind_ignores_include_untracked("process", &non_rust));
        assert!(compat_kind_ignores_include_untracked("network", &non_rust));
        assert!(compat_kind_ignores_include_untracked(
            "generated",
            &generated
        ));
        // Inventory-scanning surfaces honor it.
        assert!(!compat_kind_ignores_include_untracked(
            "non-rust", &non_rust
        ));
        assert!(!compat_kind_ignores_include_untracked("unsafe", &non_rust));
        assert!(!compat_kind_ignores_include_untracked(
            "dependency-surface",
            &non_rust
        ));
    }
}
