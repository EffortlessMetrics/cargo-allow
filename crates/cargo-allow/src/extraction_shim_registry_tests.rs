use allow_policy::extraction_shims::{ShimDiagnosticKind, validate_extraction_shim_registry_at};
use std::path::PathBuf;

#[test]
fn extraction_shim_registry_report_only() -> Result<(), String> {
    let root = repo_root();
    let (_, diagnostics, report) = validate_extraction_shim_registry_at(
        &root,
        &root.join("policy/extraction-shims.toml"),
        &root.join("policy/product-move-ledger.toml"),
    )
    .map_err(|err| format!("validate shim registry: {err}"))?;

    if diagnostics
        .iter()
        .any(|diag| diag.kind == ShimDiagnosticKind::MissingMoveLedgerEntry)
    {
        return Err(format!("missing move ledger links: {diagnostics:?}"));
    }
    if report.shim_count < 7 {
        return Err("seeded shim inventory too small".to_string());
    }
    if report.active_count < 4 {
        return Err(
            "expected repo-snapshot, rust-source-index, and spec-system shims active".to_string(),
        );
    }
    if report.planned_count < 3 {
        return Err("expected remaining shims planned".to_string());
    }

    let doc = root.join("docs/architecture/extraction-shims.md");
    let doc_text =
        std::fs::read_to_string(&doc).map_err(|err| format!("shim doc readable: {err}"))?;
    if !doc_text.contains("CARGO-ALLOW-SHIM-REGISTRY-0001") {
        return Err("human projection missing registry id".to_string());
    }

    Ok(())
}

#[test]
fn repo_edit_core_shims_match_live_private_forwards() -> Result<(), String> {
    let root = repo_root();
    let registry_text = std::fs::read_to_string(root.join("policy/extraction-shims.toml"))
        .map_err(|err| format!("read shim registry: {err}"))?;
    let registry = allow_policy::extraction_shims::parse_extraction_shim_registry(&registry_text)
        .map_err(|err| format!("parse shim registry: {err}"))?;

    let expected = [
        (
            "shim-cargo-allow-mutation-lock",
            "cargo-allow::mutation_lock",
            "repo-edit::mutation_lock",
            "crates/cargo-allow/src/mutation_lock.rs",
            "pub(crate) use effortless_repo_edit::MutationLock;",
        ),
        (
            "shim-cargo-allow-path-containment",
            "cargo-allow::policy_config::assert_path_within_root",
            "repo-edit::containment",
            "crates/cargo-allow/src/policy_config.rs",
            "map_err(crate::extraction_repo_edit_runtime::map_repo_edit_error)",
        ),
        (
            "shim-cargo-allow-atomic-write",
            "cargo-allow::io::write_file",
            "repo-edit::atomic_write",
            "crates/cargo-allow/src/command_support.rs",
            "pub(crate) use effortless_repo_edit::{write_file, write_file_no_overwrite};",
        ),
    ];

    for (id, old_identity, new_identity, source_path, forwarding_marker) in expected {
        let shim = registry
            .shim
            .iter()
            .find(|shim| shim.id == id)
            .ok_or_else(|| format!("missing repo-edit core shim {id}"))?;
        if shim.posture != allow_policy::extraction_shims::ShimPosture::Private
            || shim.status != allow_policy::extraction_shims::ShimStatus::Active
            || shim.old_identity != old_identity
            || shim.new_identity != new_identity
            || !shim.removal_condition.contains("#2606")
        {
            return Err(format!(
                "repo-edit core shim {id} has an unexpected compatibility boundary"
            ));
        }

        let source = std::fs::read_to_string(root.join(source_path))
            .map_err(|err| format!("read {source_path}: {err}"))?;
        if !source.contains(forwarding_marker) {
            return Err(format!(
                "repo-edit core shim {id} is missing forwarding marker in {source_path}"
            ));
        }
    }

    Ok(())
}

#[test]
fn repo_edit_command_shims_match_live_apply_forwards() -> Result<(), String> {
    let root = repo_root();
    let registry_text = std::fs::read_to_string(root.join("policy/extraction-shims.toml"))
        .map_err(|err| format!("read shim registry: {err}"))?;
    let registry = allow_policy::extraction_shims::parse_extraction_shim_registry(&registry_text)
        .map_err(|err| format!("parse shim registry: {err}"))?;

    let expected = [
        (
            "shim-cargo-allow-init-apply",
            "cargo-allow::init",
            "crates/cargo-allow/src/init.rs",
        ),
        (
            "shim-cargo-allow-refresh-apply",
            "cargo-allow::refresh",
            "crates/cargo-allow/src/refresh.rs",
        ),
        (
            "shim-cargo-allow-prune-apply",
            "cargo-allow::prune",
            "crates/cargo-allow/src/prune.rs",
        ),
        (
            "shim-cargo-allow-add-apply",
            "cargo-allow::add",
            "crates/cargo-allow/src/add.rs",
        ),
        (
            "shim-cargo-allow-migrate-apply",
            "cargo-allow::migrate",
            "crates/cargo-allow/src/migrate.rs",
        ),
        (
            "shim-cargo-allow-propose-apply",
            "cargo-allow::propose",
            "crates/cargo-allow/src/propose.rs",
        ),
    ];

    for (id, old_identity, source_path) in expected {
        let shim = registry
            .shim
            .iter()
            .find(|shim| shim.id == id)
            .ok_or_else(|| format!("missing repo-edit command shim {id}"))?;
        if shim.posture != allow_policy::extraction_shims::ShimPosture::Private
            || shim.status != allow_policy::extraction_shims::ShimStatus::Active
            || shim.old_identity != old_identity
            || shim.new_identity != "repo-edit::single_target_apply"
            || !shim.removal_condition.contains("#2606")
        {
            return Err(format!(
                "repo-edit command shim {id} has an unexpected compatibility boundary"
            ));
        }

        let source = std::fs::read_to_string(root.join(source_path))
            .map_err(|err| format!("read {source_path}: {err}"))?;
        if !source.contains("effortless_repo_edit::{SingleTargetApplyMode")
            || !source.contains("apply_single_target(SingleTargetApplyRequest")
        {
            return Err(format!(
                "repo-edit command shim {id} is missing live apply forwarding in {source_path}"
            ));
        }
    }

    Ok(())
}

#[test]
fn repo_snapshot_shims_retired_after_cutover() -> Result<(), String> {
    // #3556: the allow-diff forwarding shims are removed at cutover; the
    // old-path files are deleted and the registry records the retired state.
    let root = repo_root();
    let registry_text = std::fs::read_to_string(root.join("policy/extraction-shims.toml"))
        .map_err(|err| format!("read shim registry: {err}"))?;
    let registry = allow_policy::extraction_shims::parse_extraction_shim_registry(&registry_text)
        .map_err(|err| format!("parse shim registry: {err}"))?;

    let expected = [
        (
            "shim-allow-diff-staged-index",
            "crates/allow-diff/src/staged_index.rs",
        ),
        (
            "shim-allow-diff-revision-identity",
            "crates/allow-diff/src/revision_identity.rs",
        ),
    ];

    for (id, deleted_path) in expected {
        let shim = registry
            .shim
            .iter()
            .find(|shim| shim.id == id)
            .ok_or_else(|| format!("missing repo-snapshot shim {id}"))?;
        if shim.status != allow_policy::extraction_shims::ShimStatus::Removed {
            return Err(format!(
                "repo-snapshot shim {id} must be removed after the cutover"
            ));
        }
        if root.join(deleted_path).exists() {
            return Err(format!(
                "old path {deleted_path} still exists after shim removal"
            ));
        }
    }

    Ok(())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
