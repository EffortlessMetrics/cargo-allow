use std::path::PathBuf;

#[test]
fn proof_engine_parity_fixtures_registered() -> Result<(), String> {
    let root = repo_root();
    for path in proof_engine_parity_fixture_paths(&root) {
        if !path.is_file() {
            return Err(format!("missing parity fixture {}", path.display()));
        }
    }

    let ledger = std::fs::read_to_string(root.join("policy/product-move-ledger.toml"))
        .map_err(|err| format!("move ledger: {err}"))?;
    if !ledger.contains("introduce-proof-engine-crate") {
        return Err("move ledger missing proof-engine scaffold entry".to_string());
    }

    let doc = root.join("docs/architecture/proof-engine.md");
    let doc_text =
        std::fs::read_to_string(&doc).map_err(|err| format!("proof-engine doc: {err}"))?;
    if !doc_text.contains("2589-A") {
        return Err("human projection missing packet marker".to_string());
    }

    let manifest = root.join("crates/cargo-allow/Cargo.toml");
    let manifest_text = std::fs::read_to_string(&manifest)
        .map_err(|err| format!("read cargo-allow manifest: {err}"))?;
    if manifest_lists_dependency(&manifest_text, "proof-engine") {
        return Err("cargo-allow must not depend on proof-engine".to_string());
    }

    Ok(())
}

fn proof_engine_parity_fixture_paths(root: &std::path::Path) -> Vec<PathBuf> {
    vec![root.join("tests/fixtures/proof-engine/parity-boundary-v1.toml")]
}

fn manifest_lists_dependency(manifest_text: &str, crate_name: &str) -> bool {
    for section in ["dependencies", "dev-dependencies"] {
        let Ok(table) = toml::from_str::<toml::Table>(manifest_text) else {
            continue;
        };
        let Some(deps) = table.get(section).and_then(|value| value.as_table()) else {
            continue;
        };
        if deps.contains_key(crate_name) {
            return true;
        }
    }
    false
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
