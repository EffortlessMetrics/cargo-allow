use intent_model::spec_system_parity_contract_paths;
use std::path::PathBuf;

#[test]
fn intent_model_parity_fixtures_registered() -> Result<(), String> {
    let root = repo_root();
    let paths = spec_system_parity_contract_paths(&root);
    if paths.is_empty() {
        return Err("expected intent-model parity fixtures".to_string());
    }
    for path in paths {
        if !path.is_file() {
            return Err(format!("missing parity fixture {}", path.display()));
        }
    }

    let doc = root.join("docs/architecture/intent-model.md");
    let doc_text =
        std::fs::read_to_string(&doc).map_err(|err| format!("intent-model doc: {err}"))?;
    if !doc_text.contains("2584-A") {
        return Err("human projection missing PR1 packet marker".to_string());
    }

    let ledger = std::fs::read_to_string(root.join("policy/product-move-ledger.toml"))
        .map_err(|err| format!("move ledger: {err}"))?;
    if !ledger.contains("move-allow-policy-spec-system") {
        return Err("move ledger missing spec-system entry".to_string());
    }

    Ok(())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
