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
            "Ok(effortless_repo_edit::assert_path_within_root(root, path)?)",
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

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
