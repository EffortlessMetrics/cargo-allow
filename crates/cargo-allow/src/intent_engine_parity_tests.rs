use crate::spec_system_workspace_composition::SELF_HOSTED_RUNTIME_PROMOTION;
use intent_engine::{
    evaluator_packet_parity_contract_paths, self_hosted_workspace_composition_fixture_path,
    workspace_composition_parity_contract_paths,
};
use std::path::PathBuf;

#[test]
fn intent_engine_parity_fixtures_registered() -> Result<(), String> {
    let root = repo_root();
    for path in evaluator_packet_parity_contract_paths(&root) {
        if !path.is_file() {
            return Err(format!("missing parity fixture {}", path.display()));
        }
    }
    for path in workspace_composition_parity_contract_paths(&root) {
        if !path.is_file() {
            return Err(format!("missing parity fixture {}", path.display()));
        }
    }
    let composition_fixture = self_hosted_workspace_composition_fixture_path(&root);
    if !composition_fixture.is_file() {
        return Err(format!(
            "missing composition fixture {}",
            composition_fixture.display()
        ));
    }

    let fixture_text = std::fs::read_to_string(&composition_fixture)
        .map_err(|err| format!("composition fixture: {err}"))?;
    let fixture: toml::Table =
        toml::from_str(&fixture_text).map_err(|err| format!("parse composition fixture: {err}"))?;
    for (field, value) in [
        (
            "composition_id",
            SELF_HOSTED_RUNTIME_PROMOTION.composition_id,
        ),
        (
            "requirement_path",
            SELF_HOSTED_RUNTIME_PROMOTION.requirement_path,
        ),
        ("slice_path", SELF_HOSTED_RUNTIME_PROMOTION.slice_path),
        ("seams_path", SELF_HOSTED_RUNTIME_PROMOTION.seams_path),
        ("evidence_path", SELF_HOSTED_RUNTIME_PROMOTION.evidence_path),
        (
            "subject_inventory",
            SELF_HOSTED_RUNTIME_PROMOTION.subject_inventory,
        ),
    ] {
        let Some(fixture_value) = fixture.get(field).and_then(|value| value.as_str()) else {
            return Err(format!("composition fixture missing {field}"));
        };
        if fixture_value != value {
            return Err(format!(
                "cargo-allow composition {field} drifted from fixture: {fixture_value} != {value}"
            ));
        }
    }

    let doc = root.join("docs/architecture/intent-engine.md");
    let doc_text =
        std::fs::read_to_string(&doc).map_err(|err| format!("intent-engine doc: {err}"))?;
    if !doc_text.contains("2586-A") {
        return Err("human projection missing PR1 packet marker".to_string());
    }
    if !doc_text.contains("2586-B") {
        return Err("human projection missing PR2 packet marker".to_string());
    }

    let ledger = std::fs::read_to_string(root.join("policy/product-move-ledger.toml"))
        .map_err(|err| format!("move ledger: {err}"))?;
    if !ledger.contains("move-cargo-allow-spec-system-workspace") {
        return Err("move ledger missing spec-system workspace entry".to_string());
    }

    Ok(())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
