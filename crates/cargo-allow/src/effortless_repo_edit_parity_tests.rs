use effortless_repo_edit::parity_contract_paths;
use std::path::PathBuf;

#[test]
fn repo_edit_parity_fixtures_registered() -> Result<(), String> {
    let root = repo_root();
    let paths = parity_contract_paths(&root);
    if paths.len() < 11 {
        return Err(
            "expected lock, containment, atomic-write, apply-receipt, backup-mode, init, refresh, prune, add, migrate, and propose parity fixtures"
                .to_string(),
        );
    }
    for path in paths {
        if !path.is_file() {
            return Err(format!("missing parity fixture {}", path.display()));
        }
    }

    let doc = root.join("docs/architecture/repo-edit.md");
    let doc_text = std::fs::read_to_string(&doc).map_err(|err| format!("repo-edit doc: {err}"))?;
    if !doc_text.contains("2602-A") {
        return Err("human projection missing packet marker".to_string());
    }

    let ledger = std::fs::read_to_string(root.join("policy/product-move-ledger.toml"))
        .map_err(|err| format!("move ledger: {err}"))?;
    if !ledger.contains("move-cargo-allow-mutation-lock") {
        return Err("move ledger missing mutation-lock entry".to_string());
    }
    if !ledger.contains("move-cargo-allow-atomic-write") {
        return Err("move ledger missing atomic-write entry".to_string());
    }
    if !ledger.contains("introduce-repo-edit-apply-receipt") {
        return Err("move ledger missing apply-receipt entry".to_string());
    }
    if !ledger.contains("extend-repo-edit-apply-backup-mode") {
        return Err("move ledger missing apply backup-mode entry".to_string());
    }
    if !ledger.contains("migrate-cargo-allow-init-command") {
        return Err("move ledger missing init migration entry".to_string());
    }
    if !ledger.contains("migrate-cargo-allow-refresh-command") {
        return Err("move ledger missing refresh migration entry".to_string());
    }
    if !ledger.contains("migrate-cargo-allow-prune-command") {
        return Err("move ledger missing prune migration entry".to_string());
    }
    if !ledger.contains("migrate-cargo-allow-add-command") {
        return Err("move ledger missing add migration entry".to_string());
    }
    if !ledger.contains("migrate-cargo-allow-migrate-command") {
        return Err("move ledger missing migrate migration entry".to_string());
    }
    if !ledger.contains("migrate-cargo-allow-propose-command") {
        return Err("move ledger missing propose migration entry".to_string());
    }

    Ok(())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
