use effortless_repo_snapshot::parity_contract_paths;
use std::path::PathBuf;

#[test]
fn repo_snapshot_parity_fixtures_registered() -> Result<(), String> {
    let root = repo_root();
    let paths = parity_contract_paths(&root);
    if paths.len() < 4 {
        return Err(
            "expected revision, staged, deletion, and source-view parity fixtures".to_string(),
        );
    }
    for path in paths {
        if !path.is_file() {
            return Err(format!("missing parity fixture {}", path.display()));
        }
    }

    let doc = root.join("docs/architecture/repo-snapshot.md");
    let doc_text =
        std::fs::read_to_string(&doc).map_err(|err| format!("repo-snapshot doc: {err}"))?;
    if !doc_text.contains("2583-A") {
        return Err("human projection missing PR1 packet marker".to_string());
    }

    let ledger = std::fs::read_to_string(root.join("policy/product-move-ledger.toml"))
        .map_err(|err| format!("move ledger: {err}"))?;
    if !ledger.contains("move-allow-diff-staged-index") {
        return Err("move ledger missing staged-index entry".to_string());
    }

    Ok(())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
