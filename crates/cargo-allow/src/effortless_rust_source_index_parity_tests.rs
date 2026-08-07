use effortless_rust_source_index::test_subjects_parity_contract_paths;
use std::path::PathBuf;

#[test]
fn rust_source_index_parity_fixtures_registered() -> Result<(), String> {
    let root = repo_root();
    let paths = test_subjects_parity_contract_paths(&root);
    if paths.is_empty() {
        return Err("expected rust-source-index parity fixtures".to_string());
    }
    for path in paths {
        if !path.is_file() {
            return Err(format!("missing parity fixture {}", path.display()));
        }
    }

    let doc = root.join("docs/architecture/rust-source-index.md");
    let doc_text =
        std::fs::read_to_string(&doc).map_err(|err| format!("rust-source-index doc: {err}"))?;
    if !doc_text.contains("2587-A") {
        return Err("human projection missing PR1 packet marker".to_string());
    }

    let ledger = std::fs::read_to_string(root.join("policy/product-move-ledger.toml"))
        .map_err(|err| format!("move ledger: {err}"))?;
    if !ledger.contains("move-allow-rust-test-subjects") {
        return Err("move ledger missing test-subjects entry".to_string());
    }

    Ok(())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
