use crate::EvaluatorPacketSurface;
use crate::parity::{EvaluatorPacketParityContract, load_evaluator_packet_parity_contract};
use std::path::PathBuf;

#[test]
fn parity_contracts_load_from_fixtures() -> Result<(), String> {
    let root = workspace_root();
    for path in crate::parity::evaluator_packet_parity_contract_paths(&root) {
        let contract = load_evaluator_packet_parity_contract(&path)?;
        validate_contract(&contract)?;
    }
    Ok(())
}

#[test]
fn evaluator_packet_surface_matches_parity_contract() -> Result<(), String> {
    let root = workspace_root();
    let contract_path = crate::parity::evaluator_packet_parity_contract_path(&root);
    let contract = load_evaluator_packet_parity_contract(&contract_path)?;
    if contract.intent_engine_module != EvaluatorPacketSurface::MODULE_ID {
        return Err(format!(
            "surface marker {} does not match contract {}",
            EvaluatorPacketSurface::MODULE_ID,
            contract.intent_engine_module
        ));
    }
    Ok(())
}

#[test]
fn evaluator_packet_envelope_roundtrip() -> Result<(), String> {
    let packet = crate::IntentEnginePacketEnvelopeV1::new(
        serde_json::json!({
            "schema_id": "intent.query.v1",
            "kind": "validate_artifact",
            "selector": "policy/spec-system.toml",
        }),
        crate::IntentEnginePacketKindV1::LoadAndValidate,
    );
    let json = serde_json::to_string(&packet)
        .map_err(|err| format!("serialize evaluator packet: {err}"))?;
    let decoded: crate::IntentEnginePacketEnvelopeV1 = serde_json::from_str(&json)
        .map_err(|err| format!("deserialize evaluator packet: {err}"))?;
    if decoded.kind != crate::IntentEnginePacketKindV1::LoadAndValidate {
        return Err("packet kind did not round-trip".to_string());
    }
    if decoded.query_schema_id != crate::INTENT_QUERY_TRANSPORT_SCHEMA_ID {
        return Err("query schema id mismatch".to_string());
    }
    Ok(())
}

fn validate_contract(contract: &EvaluatorPacketParityContract) -> Result<(), String> {
    if contract.scenario_id.is_empty() {
        return Err("empty scenario_id".to_string());
    }
    if contract.move_ledger_entry != "move-cargo-allow-spec-system-workspace" {
        return Err(format!(
            "unexpected move ledger entry {}",
            contract.move_ledger_entry
        ));
    }
    if contract.required_packet_fields.len() < 2 {
        return Err("required_packet_fields too small".to_string());
    }
    Ok(())
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
