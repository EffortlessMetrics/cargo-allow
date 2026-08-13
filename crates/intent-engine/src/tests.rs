use crate::parity::{
    BoundedDomainQueriesParityContract, EvaluatorPacketParityContract,
    GraphComparisonParityContract, ParityCorpusParityContract, PhaseObligationsParityContract,
    WorkspaceCompositionParityContract, load_bounded_domain_queries_parity_contract,
    load_bounded_domain_query_catalog_fixture, load_evaluator_packet_parity_contract,
    load_graph_comparison_parity_contract, load_graph_movement_kinds_fixture,
    load_parity_corpus_contract, load_parity_corpus_fixture,
    load_phase_obligations_parity_contract, load_precommit_obligation_plan_fixture,
    load_self_hosted_workspace_composition_fixture, load_workspace_composition_parity_contract,
};
use std::path::PathBuf;

#[test]
fn parity_contracts_load_from_fixtures() -> Result<(), String> {
    let root = workspace_root();
    for path in crate::parity::evaluator_packet_parity_contract_paths(&root) {
        let contract = load_evaluator_packet_parity_contract(&path)?;
        validate_packet_contract(&contract)?;
    }
    for path in crate::parity::workspace_composition_parity_contract_paths(&root) {
        let contract = load_workspace_composition_parity_contract(&path)?;
        validate_workspace_contract(&contract)?;
    }
    for path in crate::parity::graph_comparison_parity_contract_paths(&root) {
        let contract = load_graph_comparison_parity_contract(&path)?;
        validate_graph_comparison_contract(&contract)?;
    }
    for path in crate::parity::phase_obligations_parity_contract_paths(&root) {
        let contract = load_phase_obligations_parity_contract(&path)?;
        validate_phase_obligations_contract(&contract)?;
    }
    for path in crate::parity::bounded_domain_queries_parity_contract_paths(&root) {
        let contract = load_bounded_domain_queries_parity_contract(&path)?;
        validate_bounded_domain_queries_contract(&contract)?;
    }
    for path in crate::parity::parity_corpus_contract_paths(&root) {
        let contract = load_parity_corpus_contract(&path)?;
        validate_parity_corpus_contract(&contract)?;
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
    if decoded.query_schema_id != intent_protocol::INTENT_QUERY_SCHEMA_ID {
        return Err("query schema id mismatch".to_string());
    }
    Ok(())
}

#[test]
fn self_hosted_composition_matches_fixture() -> Result<(), String> {
    let root = workspace_root();
    let fixture = load_self_hosted_workspace_composition_fixture(&root)?;
    let canonical = crate::WorkspaceCompositionV1::self_hosted_runtime_promotion();
    if fixture != canonical {
        return Err("self-hosted fixture must match canonical composition".to_string());
    }
    Ok(())
}

#[test]
fn authority_compile_plan_orders_sources() -> Result<(), String> {
    let composition = crate::WorkspaceCompositionV1::self_hosted_runtime_promotion();
    let plan = crate::plan_authority_compile(&composition);
    if plan.sources.len() != 4 {
        return Err(format!(
            "expected four authority sources, got {}",
            plan.sources.len()
        ));
    }
    let first = plan
        .sources
        .first()
        .ok_or_else(|| "authority compile plan missing sources".to_string())?;
    if first.role != crate::AuthoritySourceRoleV1::Requirement {
        return Err("first authority source must be requirement".to_string());
    }
    Ok(())
}

#[test]
fn graph_movement_kinds_match_fixture() -> Result<(), String> {
    let root = workspace_root();
    let fixture_kinds = load_graph_movement_kinds_fixture(&root)?;
    let canonical = crate::canonical_graph_movement_kinds()
        .iter()
        .map(|kind| kind.as_str().to_string())
        .collect::<Vec<_>>();
    if fixture_kinds != canonical {
        return Err("graph movement kinds fixture drifted from canonical ordering".to_string());
    }
    Ok(())
}

#[test]
fn compile_phase_obligation_plan_from_movements() -> Result<(), String> {
    let plan = crate::compile_phase_obligation_plan(&crate::PhaseObligationCompileInputV1 {
        phase: crate::PRECOMMIT_PHASE_ID.to_string(),
        movements: vec![
            crate::GraphMovementV1 {
                kind: crate::GraphMovementKindV1::RequirementChanged,
                id: "REQ-0001".to_string(),
            },
            crate::GraphMovementV1 {
                kind: crate::GraphMovementKindV1::SubjectBodyIdentityChanged,
                id: "subject-1".to_string(),
            },
        ],
        inventory: crate::InventoryPostureV1::Partial,
        legacy_baseline: false,
    });
    if plan.obligations.is_empty() {
        return Err("expected obligations from movement profile".to_string());
    }
    let has_inventory = plan
        .obligations
        .iter()
        .any(|item| item.kind == crate::PhaseObligationKindV1::InventoryCompleteness);
    if !has_inventory {
        return Err("partial inventory must surface inventory completeness".to_string());
    }
    Ok(())
}

#[test]
fn precommit_obligation_plan_fixture_loads() -> Result<(), String> {
    let root = workspace_root();
    let fixture = load_precommit_obligation_plan_fixture(&root)?;
    if fixture.phase != crate::PRECOMMIT_PHASE_ID {
        return Err("fixture phase must be precommit".to_string());
    }
    if fixture.obligations.len() < 3 {
        return Err("fixture must include representative obligations".to_string());
    }
    Ok(())
}

#[test]
fn bounded_domain_query_catalog_matches_fixture() -> Result<(), String> {
    let root = workspace_root();
    let fixture_kinds = load_bounded_domain_query_catalog_fixture(&root)?;
    let canonical = crate::canonical_bounded_domain_query_kinds()
        .iter()
        .map(|kind| kind.as_str().to_string())
        .collect::<Vec<_>>();
    if fixture_kinds != canonical {
        return Err("bounded domain query catalog drifted from canonical ordering".to_string());
    }
    Ok(())
}

#[test]
fn bounded_domain_query_returns_protocol_shaped_response() -> Result<(), String> {
    let request = crate::BoundedDomainQueryRequestV1::new(
        crate::BoundedDomainQueryKindV1::MovementKindsCatalog,
    );
    let response = crate::execute_bounded_domain_query(&request);
    if response.result_class != crate::RESULT_CLASS_COMPLETED {
        return Err("expected completed result class".to_string());
    }
    let protocol_json = crate::to_intent_query_response_json(&response);
    if protocol_json.get("schema_id").and_then(|v| v.as_str())
        != Some(intent_protocol::INTENT_QUERY_RESPONSE_SCHEMA_ID)
    {
        return Err("protocol projection missing query-response schema".to_string());
    }
    let projected: intent_protocol::IntentQueryResponseV1 =
        serde_json::from_value(protocol_json)
            .map_err(|err| format!("deserialize intent-protocol response: {err}"))?;
    if projected.payload_schema != response.payload_schema {
        return Err("payload schema mismatch in protocol projection".to_string());
    }
    Ok(())
}

fn validate_packet_contract(contract: &EvaluatorPacketParityContract) -> Result<(), String> {
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

fn validate_workspace_contract(
    contract: &WorkspaceCompositionParityContract,
) -> Result<(), String> {
    if contract.scenario_id.is_empty() {
        return Err("empty scenario_id".to_string());
    }
    if contract.move_ledger_entry != "move-cargo-allow-spec-system-workspace" {
        return Err(format!(
            "unexpected move ledger entry {}",
            contract.move_ledger_entry
        ));
    }
    if contract.required_composition_fields.len() < 4 {
        return Err("required_composition_fields too small".to_string());
    }
    Ok(())
}

fn validate_graph_comparison_contract(
    contract: &GraphComparisonParityContract,
) -> Result<(), String> {
    if contract.scenario_id.is_empty() {
        return Err("empty scenario_id".to_string());
    }
    if contract.required_movement_kinds.len() < 10 {
        return Err("required_movement_kinds too small".to_string());
    }
    Ok(())
}

fn validate_phase_obligations_contract(
    contract: &PhaseObligationsParityContract,
) -> Result<(), String> {
    if contract.scenario_id.is_empty() {
        return Err("empty scenario_id".to_string());
    }
    if contract.required_obligation_kinds.len() < 4 {
        return Err("required_obligation_kinds too small".to_string());
    }
    if contract.sample_phase != crate::PRECOMMIT_PHASE_ID {
        return Err("sample_phase must be precommit".to_string());
    }
    Ok(())
}

fn validate_bounded_domain_queries_contract(
    contract: &BoundedDomainQueriesParityContract,
) -> Result<(), String> {
    if contract.scenario_id.is_empty() {
        return Err("empty scenario_id".to_string());
    }
    if contract.protocol_response_schema != intent_protocol::INTENT_QUERY_RESPONSE_SCHEMA_ID {
        return Err("protocol_response_schema must be intent.query-response.v1".to_string());
    }
    if contract.required_query_kinds.len() < 3 {
        return Err("required_query_kinds too small".to_string());
    }
    Ok(())
}

#[test]
fn parity_corpus_fixture_records_all_dimensions() -> Result<(), String> {
    let root = workspace_root();
    let corpus = load_parity_corpus_fixture(&root)?;
    if corpus.corpus_digest != crate::PARITY_CORPUS_DIGEST_V1 {
        return Err("corpus digest drift".to_string());
    }
    for scenario in &corpus.scenarios {
        if !crate::canonical_parity_dispositions()
            .iter()
            .any(|disposition| *disposition == scenario.disposition)
        {
            return Err(format!(
                "scenario {} has unknown disposition {}",
                scenario.id, scenario.disposition
            ));
        }
    }
    Ok(())
}

fn validate_parity_corpus_contract(contract: &ParityCorpusParityContract) -> Result<(), String> {
    if contract.scenario_id.is_empty() {
        return Err("empty scenario_id".to_string());
    }
    if contract.required_dimensions.len() < 5 {
        return Err("required_dimensions too small".to_string());
    }
    if contract.corpus_digest != crate::PARITY_CORPUS_DIGEST_V1 {
        return Err("corpus_digest mismatch".to_string());
    }
    Ok(())
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
