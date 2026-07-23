use crate::spec_system_graph_movement::SpecGraphMovementKind;
use crate::spec_system_workspace_composition::SELF_HOSTED_RUNTIME_PROMOTION;
use intent_engine::{
    GraphMovementKindV1, bounded_domain_queries_parity_contract_paths,
    bounded_domain_query_catalog_fixture_path, canonical_bounded_domain_query_kinds,
    canonical_graph_movement_kinds, evaluator_packet_parity_contract_paths,
    graph_comparison_parity_contract_paths, graph_movement_kinds_fixture_path,
    phase_obligations_parity_contract_paths, precommit_obligation_plan_fixture_path,
    self_hosted_workspace_composition_fixture_path, workspace_composition_parity_contract_paths,
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
    for path in graph_comparison_parity_contract_paths(&root) {
        if !path.is_file() {
            return Err(format!("missing parity fixture {}", path.display()));
        }
    }
    for path in phase_obligations_parity_contract_paths(&root) {
        if !path.is_file() {
            return Err(format!("missing parity fixture {}", path.display()));
        }
    }
    for path in bounded_domain_queries_parity_contract_paths(&root) {
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

    let movement_kinds_fixture = graph_movement_kinds_fixture_path(&root);
    if !movement_kinds_fixture.is_file() {
        return Err(format!(
            "missing graph movement kinds fixture {}",
            movement_kinds_fixture.display()
        ));
    }
    let fixture_kinds = intent_engine::load_graph_movement_kinds_fixture(&root)?;
    for kind in canonical_graph_movement_kinds() {
        let kind_str = kind.as_str();
        if !fixture_kinds.iter().any(|fixture| fixture == kind_str) {
            return Err(format!("fixture missing movement kind {kind_str}"));
        }
        let cargo_kind = spec_graph_movement_kind_as_str(*kind);
        if cargo_kind != kind_str {
            return Err(format!(
                "cargo-allow movement kind drift for {kind_str}: {cargo_kind}"
            ));
        }
    }

    let obligation_fixture = precommit_obligation_plan_fixture_path(&root);
    if !obligation_fixture.is_file() {
        return Err(format!(
            "missing obligation plan fixture {}",
            obligation_fixture.display()
        ));
    }

    let query_catalog_fixture = bounded_domain_query_catalog_fixture_path(&root);
    if !query_catalog_fixture.is_file() {
        return Err(format!(
            "missing bounded query catalog fixture {}",
            query_catalog_fixture.display()
        ));
    }
    let catalog_kinds = intent_engine::load_bounded_domain_query_catalog_fixture(&root)?;
    for kind in canonical_bounded_domain_query_kinds() {
        let kind_str = kind.as_str();
        if !catalog_kinds.iter().any(|fixture| fixture == kind_str) {
            return Err(format!("catalog fixture missing query kind {kind_str}"));
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
    if !doc_text.contains("2586-C") {
        return Err("human projection missing PR3 packet marker".to_string());
    }
    if !doc_text.contains("2586-D") {
        return Err("human projection missing PR4 packet marker".to_string());
    }

    let ledger = std::fs::read_to_string(root.join("policy/product-move-ledger.toml"))
        .map_err(|err| format!("move ledger: {err}"))?;
    if !ledger.contains("move-cargo-allow-spec-system-workspace") {
        return Err("move ledger missing spec-system workspace entry".to_string());
    }

    Ok(())
}

fn spec_graph_movement_kind_as_str(kind: GraphMovementKindV1) -> &'static str {
    match kind {
        GraphMovementKindV1::RequirementAdded => SpecGraphMovementKind::RequirementAdded.as_str(),
        GraphMovementKindV1::RequirementRemoved => {
            SpecGraphMovementKind::RequirementRemoved.as_str()
        }
        GraphMovementKindV1::RequirementChanged => {
            SpecGraphMovementKind::RequirementChanged.as_str()
        }
        GraphMovementKindV1::ImplementationSliceAdded => {
            SpecGraphMovementKind::ImplementationSliceAdded.as_str()
        }
        GraphMovementKindV1::ImplementationSliceRemoved => {
            SpecGraphMovementKind::ImplementationSliceRemoved.as_str()
        }
        GraphMovementKindV1::ImplementationSliceChanged => {
            SpecGraphMovementKind::ImplementationSliceChanged.as_str()
        }
        GraphMovementKindV1::SeamMappingAdded => SpecGraphMovementKind::SeamMappingAdded.as_str(),
        GraphMovementKindV1::SeamMappingRemoved => {
            SpecGraphMovementKind::SeamMappingRemoved.as_str()
        }
        GraphMovementKindV1::SeamMappingChanged => {
            SpecGraphMovementKind::SeamMappingChanged.as_str()
        }
        GraphMovementKindV1::EvidencePurposeAdded => {
            SpecGraphMovementKind::EvidencePurposeAdded.as_str()
        }
        GraphMovementKindV1::EvidencePurposeRemoved => {
            SpecGraphMovementKind::EvidencePurposeRemoved.as_str()
        }
        GraphMovementKindV1::EvidencePurposeChanged => {
            SpecGraphMovementKind::EvidencePurposeChanged.as_str()
        }
        GraphMovementKindV1::EvidenceClaimChanged => {
            SpecGraphMovementKind::EvidenceClaimChanged.as_str()
        }
        GraphMovementKindV1::SubjectSelectorAdded => {
            SpecGraphMovementKind::SubjectSelectorAdded.as_str()
        }
        GraphMovementKindV1::SubjectSelectorRemoved => {
            SpecGraphMovementKind::SubjectSelectorRemoved.as_str()
        }
        GraphMovementKindV1::SubjectSelectorChanged => {
            SpecGraphMovementKind::SubjectSelectorChanged.as_str()
        }
        GraphMovementKindV1::SubjectBodyIdentityChanged => {
            SpecGraphMovementKind::SubjectBodyIdentityChanged.as_str()
        }
        GraphMovementKindV1::ProfileOrDialectChanged => {
            SpecGraphMovementKind::ProfileOrDialectChanged.as_str()
        }
        GraphMovementKindV1::UnknownOrUncomparable => {
            SpecGraphMovementKind::UnknownOrUncomparable.as_str()
        }
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
