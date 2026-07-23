use intent_protocol::{
    identity_query_parity_contract_paths, obligation_plan_parity_contract_paths,
    view_diff_closure_parity_contract_paths,
};
use std::path::PathBuf;

#[test]
fn intent_protocol_parity_fixtures_registered() -> Result<(), String> {
    let root = repo_root();
    let paths = identity_query_parity_contract_paths(&root);
    if paths.is_empty() {
        return Err("expected intent-protocol parity fixtures".to_string());
    }
    for path in paths {
        if !path.is_file() {
            return Err(format!("missing parity fixture {}", path.display()));
        }
    }
    for path in view_diff_closure_parity_contract_paths(&root) {
        if !path.is_file() {
            return Err(format!("missing parity fixture {}", path.display()));
        }
    }
    for path in obligation_plan_parity_contract_paths(&root) {
        if !path.is_file() {
            return Err(format!("missing parity fixture {}", path.display()));
        }
    }

    let doc = root.join("docs/architecture/intent-protocol.md");
    let doc_text =
        std::fs::read_to_string(&doc).map_err(|err| format!("intent-protocol doc: {err}"))?;
    if !doc_text.contains("2585-A") {
        return Err("human projection missing PR1 packet marker".to_string());
    }
    if !doc_text.contains("2585-B") {
        return Err("human projection missing PR2 packet marker".to_string());
    }
    if !doc_text.contains("2585-C") {
        return Err("human projection missing PR3 packet marker".to_string());
    }

    let ledger = std::fs::read_to_string(root.join("policy/product-move-ledger.toml"))
        .map_err(|err| format!("move ledger: {err}"))?;
    if !ledger.contains("move-allow-report-spec-system-schema") {
        return Err("move ledger missing spec-system schema entry".to_string());
    }

    Ok(())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
