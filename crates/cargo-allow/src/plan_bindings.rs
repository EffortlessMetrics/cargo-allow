//! Shared identity-binding computation for the add-finding plan lifecycle.
//!
//! `why --plan` (generation) and `add --from-plan` (verification) must derive
//! the exact same repository / inventory / policy / finding / selector bindings
//! from a live source-tree scan. Centralizing that computation here keeps the
//! producer and the consumer bit-for-bit consistent: a plan verifies iff every
//! recomputed binding equals the one the plan recorded. Any drift — edited
//! policy, changed source file, moved or vanished finding, selector change —
//! yields a different digest here and is rejected before policy is touched.

use std::collections::BTreeMap;
use std::path::Path;

use allow_core::{
    AllowConfig, CappedReadError, CargoAllowError, CargoAllowErrorKind, CargoAllowResult, Finding,
    Selector, finding_identity_key, normalize_path, read_file_capped, sha256_v1_bytes,
};
use serde_json::{Value, json};

use crate::config_path;
use crate::policy_config::{git_relative_selected_config_path, missing_plan_policy_config_error};
use crate::selector::selector_from_finding;

/// Recomputed identity bindings for one finding against the live scan. Every
/// field is a deterministic function of committed source, the inventory basis,
/// and the resolved policy — never of operator judgment, branch, or session
/// state.
pub(crate) struct PlanFindingBindings {
    pub repository_identity: String,
    pub inventory_basis_identity: String,
    pub policy_path: String,
    pub policy_digest: String,
    pub finding_kind: String,
    pub finding_family: Option<String>,
    pub finding_path: String,
    pub finding_line: Option<usize>,
    pub finding_column: Option<usize>,
    pub finding_identity: BTreeMap<String, Value>,
    pub finding_digest: String,
    pub source_file_digest: String,
    pub selector: BTreeMap<String, Value>,
}

/// Recompute the repository / inventory / policy / finding / selector bindings
/// for `finding` against the live source tree at `root`.
pub(crate) fn compute_plan_finding_bindings(
    root: &Path,
    config: Option<&Path>,
    cfg: &AllowConfig,
    include_untracked: bool,
    finding: &Finding,
) -> CargoAllowResult<PlanFindingBindings> {
    let policy_path = config_path(root, config).ok_or_else(missing_plan_policy_config_error)?;
    let policy_bytes = read_bound_file(&policy_path, "policy")?;
    let policy_digest = sha256_v1_bytes(&policy_bytes);
    compute_plan_finding_bindings_with_policy(
        root,
        &policy_path,
        &policy_digest,
        cfg,
        include_untracked,
        finding,
    )
}

/// Compute bindings from a policy path and byte observation already held by the
/// caller. Mutation callers use this form so binding computation cannot
/// silently select or read another policy.
pub(crate) fn compute_plan_finding_bindings_with_policy(
    root: &Path,
    policy_path: &Path,
    policy_digest: &str,
    cfg: &AllowConfig,
    include_untracked: bool,
    finding: &Finding,
) -> CargoAllowResult<PlanFindingBindings> {
    let relative_policy = git_relative_selected_config_path(root, policy_path)?;
    let source_path = root.join(&finding.path);
    let source_bytes = read_bound_file(&source_path, "source file")?;
    let finding_key = finding_identity_key(finding);
    let selector = selector_from_finding(finding);
    let inventory_basis_identity = inventory_identity(root, cfg, include_untracked)?;
    let repository_identity = sha256_v1_bytes(
        format!("cargo-allow.repository.v1\n{inventory_basis_identity}\n{policy_digest}")
            .as_bytes(),
    );

    Ok(PlanFindingBindings {
        repository_identity,
        inventory_basis_identity,
        policy_path: normalize_path(&relative_policy),
        policy_digest: policy_digest.to_string(),
        finding_kind: finding.kind.as_str().to_string(),
        finding_family: finding.family.clone(),
        finding_path: normalize_path(&finding.path),
        finding_line: finding.span.as_ref().map(|span| span.line as usize),
        finding_column: finding.span.as_ref().map(|span| span.column as usize),
        finding_identity: identity_values(finding),
        finding_digest: sha256_v1_bytes(finding_key.as_bytes()),
        source_file_digest: sha256_v1_bytes(&source_bytes),
        selector: selector_values(&selector),
    })
}

pub(crate) fn read_bound_file(path: &Path, label: &str) -> CargoAllowResult<Vec<u8>> {
    read_file_capped(path).map_err(|error| bound_file_error(path, label, error))
}

fn bound_file_error(path: &Path, label: &str, error: CappedReadError) -> CargoAllowError {
    let message = format!(
        "failed to read {label} {} for add-finding plan: {error}",
        path.display()
    );
    match (label, error) {
        ("policy", CappedReadError::Io(source)) => CargoAllowError::from(source)
            .with_message_prefix(format!(
                "failed to read {label} {} for add-finding plan: ",
                path.display()
            )),
        ("policy", CappedReadError::Oversized { .. } | CappedReadError::NotUtf8(_)) => {
            CargoAllowError::with_kind(CargoAllowErrorKind::InvalidPolicy, message)
        }
        ("source file", _) => CargoAllowError::with_kind(CargoAllowErrorKind::Scan, message),
        ("inventory file", _) => {
            CargoAllowError::with_kind(CargoAllowErrorKind::Inventory, message)
        }
        (_, _) => CargoAllowError::with_kind(CargoAllowErrorKind::InvalidPolicy, message),
    }
}

pub(crate) fn inventory_identity(
    root: &Path,
    cfg: &AllowConfig,
    include_untracked: bool,
) -> CargoAllowResult<String> {
    let options = allow_inventory::InventoryOptions {
        ignored: cfg.workspace.ignored.clone(),
        generated: cfg.workspace.generated.clone(),
        include_untracked,
    };
    let inventory = allow_inventory::inventory(root, &options)?;
    inventory_identity_from_inventory(root, &inventory)
}

pub(crate) fn inventory_identity_from_inventory(
    root: &Path,
    inventory: &allow_inventory::Inventory,
) -> CargoAllowResult<String> {
    let mut files = inventory.files.clone();
    files.sort_by_key(|path| normalize_path(path));
    let mut canonical = Vec::new();
    push_bound_value(&mut canonical, "cargo-allow.inventory-basis.v1");
    push_bound_value(&mut canonical, inventory.source.as_str());
    push_bound_value(&mut canonical, inventory.completeness.as_str());
    for path in &files {
        let relative = path.strip_prefix(root).unwrap_or(path);
        push_bound_value(&mut canonical, &normalize_path(relative));
        let source_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            root.join(path)
        };
        let bytes = read_bound_file(&source_path, "inventory file")?;
        push_bound_value(&mut canonical, &sha256_v1_bytes(&bytes));
    }
    for path in &inventory.deleted_tracked {
        push_bound_value(&mut canonical, &format!("deleted:{}", normalize_path(path)));
    }
    for path in &inventory.inaccessible_paths {
        push_bound_value(
            &mut canonical,
            &format!("inaccessible:{}", normalize_path(path)),
        );
    }
    for path in &inventory.skipped_paths {
        push_bound_value(&mut canonical, &format!("skipped:{}", normalize_path(path)));
    }
    for path in &inventory.submodule_paths {
        push_bound_value(
            &mut canonical,
            &format!("submodule:{}", normalize_path(path)),
        );
    }
    Ok(sha256_v1_bytes(&canonical))
}

fn push_bound_value(output: &mut Vec<u8>, value: &str) {
    output.extend_from_slice(&(value.len() as u64).to_be_bytes());
    output.extend_from_slice(value.as_bytes());
}

pub(crate) fn identity_values(finding: &Finding) -> BTreeMap<String, Value> {
    let identity = &finding.identity;
    BTreeMap::from([
        ("language".to_string(), json!(identity.language)),
        ("crate_name".to_string(), json!(identity.crate_name)),
        ("module".to_string(), json!(identity.module)),
        ("container".to_string(), json!(identity.container)),
        ("ast_kind".to_string(), json!(identity.ast_kind)),
        ("symbol".to_string(), json!(identity.symbol)),
        ("callee".to_string(), json!(identity.callee)),
        ("macro_name".to_string(), json!(identity.macro_name)),
        ("lint".to_string(), json!(identity.lint)),
        (
            "receiver_fingerprint".to_string(),
            json!(identity.receiver_fingerprint),
        ),
        (
            "target_fingerprint".to_string(),
            json!(identity.target_fingerprint),
        ),
        (
            "normalized_snippet_hash".to_string(),
            json!(identity.normalized_snippet_hash),
        ),
        ("line_hint".to_string(), json!(identity.line_hint)),
        ("column_hint".to_string(), json!(identity.column_hint)),
    ])
}

pub(crate) fn selector_values(selector: &Selector) -> BTreeMap<String, Value> {
    BTreeMap::from([
        ("ast_kind".to_string(), json!(selector.ast_kind)),
        ("container".to_string(), json!(selector.container)),
        ("callee".to_string(), json!(selector.callee)),
        ("macro_name".to_string(), json!(selector.macro_name)),
        ("lint".to_string(), json!(selector.lint)),
        ("symbol".to_string(), json!(selector.symbol)),
        (
            "receiver_fingerprint".to_string(),
            json!(selector.receiver_fingerprint),
        ),
        (
            "target_fingerprint".to_string(),
            json!(selector.target_fingerprint),
        ),
        (
            "normalized_snippet_hash".to_string(),
            json!(selector.normalized_snippet_hash),
        ),
        ("line_hint".to_string(), json!(selector.line_hint)),
        ("glob".to_string(), json!(selector.glob)),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_inventory::{Inventory, InventoryCompleteness, InventorySource};
    use std::path::PathBuf;

    #[test]
    fn inaccessible_path_changes_inventory_identity() {
        let base = Inventory {
            files: Vec::new(),
            source: InventorySource::GitTracked,
            completeness: InventoryCompleteness::Partial,
            empty_git_tracked: false,
            deleted_tracked: Vec::new(),
            inaccessible_paths: Vec::new(),
            git_error: None,
            skipped_paths: Vec::new(),
            submodule_paths: Vec::new(),
        };
        let mut changed = base.clone();
        changed.inaccessible_paths = vec![PathBuf::from("blocked.rs")];
        let first = inventory_identity_from_inventory(PathBuf::from(".").as_path(), &base)
            .unwrap_or_else(|error| std::panic::panic_any(error.to_string()));
        let second = inventory_identity_from_inventory(PathBuf::from(".").as_path(), &changed)
            .unwrap_or_else(|error| std::panic::panic_any(error.to_string()));
        assert_ne!(first, second);
    }

    #[test]
    fn source_file_read_failures_are_scan() {
        let path = std::env::temp_dir().join(format!(
            "cargo-allow-missing-plan-source-{}.rs",
            std::process::id()
        ));
        let error = read_bound_file(&path, "source file")
            .expect_err("missing source binding file should fail to read");

        assert_eq!(error.kind(), CargoAllowErrorKind::Scan);
        assert_eq!(error.code(), "E0005_SCAN");
        assert!(error.to_string().contains("failed to read source file"));
        assert!(error.to_string().contains(&path.display().to_string()));
    }

    #[test]
    fn inventory_file_read_failures_are_inventory() {
        let path = std::env::temp_dir().join(format!(
            "cargo-allow-missing-plan-inventory-{}.toml",
            std::process::id()
        ));
        let error = read_bound_file(&path, "inventory file")
            .expect_err("missing inventory binding file should fail to read");

        assert_eq!(error.kind(), CargoAllowErrorKind::Inventory);
        assert_eq!(error.code(), "E0004_INVENTORY");
        assert!(error.to_string().contains("failed to read inventory file"));
        assert!(error.to_string().contains(&path.display().to_string()));
    }

    #[test]
    fn policy_file_io_failures_preserve_config_kind_and_source() {
        let path = std::env::temp_dir().join(format!(
            "cargo-allow-missing-plan-policy-{}.toml",
            std::process::id()
        ));
        let error = read_bound_file(&path, "policy")
            .expect_err("missing policy binding file should fail to read");

        assert_eq!(error.kind(), CargoAllowErrorKind::InvalidConfig);
        assert_eq!(error.code(), "E0002_INVALID_CONFIG");
        assert!(error.to_string().contains("failed to read policy"));
        assert!(error.to_string().contains(&path.display().to_string()));
        assert!(std::error::Error::source(&error).is_some());
    }

    #[test]
    fn oversized_policy_binding_is_invalid_policy() {
        let path = Path::new("policy/allow.toml");
        let error = bound_file_error(
            path,
            "policy",
            CappedReadError::Oversized {
                len: Some(9 * 1024 * 1024),
                limit: 8 * 1024 * 1024,
            },
        );

        assert_eq!(error.kind(), CargoAllowErrorKind::InvalidPolicy);
        assert_eq!(error.code(), "E0003_INVALID_POLICY");
        assert!(error.to_string().contains("source-read limit"));
    }
}
