use crate::spec_system_graph_movement::SpecGraphMovementKind;
use crate::spec_system_parity_corpus::{SPEC_SYSTEM_PROFILE_ID, parity_corpus_anchors};
use crate::spec_system_workspace_composition::SELF_HOSTED_RUNTIME_PROMOTION;
use allow_policy::spec_system::PrecommitMovementKind as LegacyPrecommitMovementKind;
use intent_engine::{
    GraphDiagnosticV1, GraphMovementKindV1, bounded_domain_queries_parity_contract_paths,
    bounded_domain_query_catalog_fixture_path, canonical_bounded_domain_query_kinds,
    canonical_graph_movement_kinds, evaluator_packet_parity_contract_paths,
    graph_comparison_parity_contract_paths, graph_movement_kinds_fixture_path,
    parity_corpus_contract_paths, parity_corpus_fixture_path,
    phase_obligations_parity_contract_paths, precommit_obligation_plan_fixture_path,
    self_hosted_workspace_composition_fixture_path, workspace_composition_parity_contract_paths,
};
use intent_engine::{graph_movement_kind_to_precommit, subject_resolution_from_diagnostic};
use intent_model::PrecommitMovementKind as CanonicalPrecommitMovementKind;
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
    for path in parity_corpus_contract_paths(&root) {
        if !path.is_file() {
            return Err(format!("missing parity fixture {}", path.display()));
        }
    }
    let corpus_fixture = parity_corpus_fixture_path(&root);
    if !corpus_fixture.is_file() {
        return Err(format!(
            "missing parity corpus fixture {}",
            corpus_fixture.display()
        ));
    }
    let corpus = intent_engine::load_parity_corpus_fixture(&root)?;
    for (scenario_id, anchor) in parity_corpus_anchors() {
        let Some(scenario) = corpus
            .scenarios
            .iter()
            .find(|entry| entry.id == scenario_id)
        else {
            return Err(format!("corpus missing scenario {scenario_id}"));
        };
        if scenario.old_value != anchor {
            return Err(format!(
                "cargo-allow anchor drift for {scenario_id}: {} != {anchor}",
                scenario.old_value
            ));
        }
        if scenario.old_value != scenario.new_value
            && scenario.disposition == "SemanticallyEquivalent"
        {
            return Err(format!(
                "silent drift risk for {scenario_id}: old/new differ under SemanticallyEquivalent"
            ));
        }
    }
    if corpus
        .scenarios
        .iter()
        .find(|scenario| scenario.id == "profile-spec-system")
        .map(|scenario| scenario.old_value.as_str())
        != Some(SPEC_SYSTEM_PROFILE_ID)
    {
        return Err("profile-spec-system scenario drifted from cargo-allow profile id".to_string());
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
    if !doc_text.contains("2586-E") {
        return Err("human projection missing PR5 packet marker".to_string());
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

fn as_legacy_precommit_kind(
    kind: CanonicalPrecommitMovementKind,
) -> allow_policy::spec_system::PrecommitMovementKind {
    use CanonicalPrecommitMovementKind as Canonical;
    use allow_policy::spec_system::PrecommitMovementKind as Legacy;
    match kind {
        Canonical::RequirementAdded => Legacy::RequirementAdded,
        Canonical::RequirementRemoved => Legacy::RequirementRemoved,
        Canonical::RequirementChanged => Legacy::RequirementChanged,
        Canonical::ImplementationSliceAdded => Legacy::ImplementationSliceAdded,
        Canonical::ImplementationSliceRemoved => Legacy::ImplementationSliceRemoved,
        Canonical::ImplementationSliceChanged => Legacy::ImplementationSliceChanged,
        Canonical::SeamMappingAdded => Legacy::SeamMappingAdded,
        Canonical::SeamMappingRemoved => Legacy::SeamMappingRemoved,
        Canonical::SeamMappingChanged => Legacy::SeamMappingChanged,
        Canonical::EvidencePurposeAdded => Legacy::EvidencePurposeAdded,
        Canonical::EvidencePurposeRemoved => Legacy::EvidencePurposeRemoved,
        Canonical::EvidencePurposeChanged => Legacy::EvidencePurposeChanged,
        Canonical::EvidenceClaimChanged => Legacy::EvidenceClaimChanged,
        Canonical::SubjectSelectorAdded => Legacy::SubjectSelectorAdded,
        Canonical::SubjectSelectorRemoved => Legacy::SubjectSelectorRemoved,
        Canonical::SubjectSelectorChanged => Legacy::SubjectSelectorChanged,
        Canonical::SubjectBodyIdentityChanged => Legacy::SubjectBodyIdentityChanged,
        Canonical::GeneratedSourceRelationAdded => Legacy::GeneratedSourceRelationAdded,
        Canonical::GeneratedSourceRelationRemoved => Legacy::GeneratedSourceRelationRemoved,
        Canonical::GeneratedSourceRelationChanged => Legacy::GeneratedSourceRelationChanged,
        Canonical::ProfileOrDialectChanged => Legacy::ProfileOrDialectChanged,
        Canonical::UnknownOrUncomparable => Legacy::UnknownOrUncomparable,
    }
}

fn legacy_precommit_kind_for(kind: GraphMovementKindV1) -> LegacyPrecommitMovementKind {
    use allow_policy::spec_system::PrecommitMovementKind as Legacy;
    match kind {
        GraphMovementKindV1::RequirementAdded => Legacy::RequirementAdded,
        GraphMovementKindV1::RequirementRemoved => Legacy::RequirementRemoved,
        GraphMovementKindV1::RequirementChanged => Legacy::RequirementChanged,
        GraphMovementKindV1::ImplementationSliceAdded => Legacy::ImplementationSliceAdded,
        GraphMovementKindV1::ImplementationSliceRemoved => Legacy::ImplementationSliceRemoved,
        GraphMovementKindV1::ImplementationSliceChanged => Legacy::ImplementationSliceChanged,
        GraphMovementKindV1::SeamMappingAdded => Legacy::SeamMappingAdded,
        GraphMovementKindV1::SeamMappingRemoved => Legacy::SeamMappingRemoved,
        GraphMovementKindV1::SeamMappingChanged => Legacy::SeamMappingChanged,
        GraphMovementKindV1::EvidencePurposeAdded => Legacy::EvidencePurposeAdded,
        GraphMovementKindV1::EvidencePurposeRemoved => Legacy::EvidencePurposeRemoved,
        GraphMovementKindV1::EvidencePurposeChanged => Legacy::EvidencePurposeChanged,
        GraphMovementKindV1::EvidenceClaimChanged => Legacy::EvidenceClaimChanged,
        GraphMovementKindV1::SubjectSelectorAdded => Legacy::SubjectSelectorAdded,
        GraphMovementKindV1::SubjectSelectorRemoved => Legacy::SubjectSelectorRemoved,
        GraphMovementKindV1::SubjectSelectorChanged => Legacy::SubjectSelectorChanged,
        GraphMovementKindV1::SubjectBodyIdentityChanged => Legacy::SubjectBodyIdentityChanged,
        GraphMovementKindV1::ProfileOrDialectChanged => Legacy::ProfileOrDialectChanged,
        GraphMovementKindV1::UnknownOrUncomparable => Legacy::UnknownOrUncomparable,
    }
}

#[test]
fn paired_precommit_movement_mapping_parity() {
    for kind in canonical_graph_movement_kinds() {
        let canonical = graph_movement_kind_to_precommit(*kind);
        // The canonical mapping is string-stable with the movement kind it
        // maps from, and the allow-policy mirror vocabulary receives the
        // same kind through the engine mapping as through a direct mapping,
        // binding engine mapping, movement parity fixture, and the legacy
        // copy together.
        assert_eq!(canonical.as_str(), kind.as_str());
        assert_eq!(
            as_legacy_precommit_kind(canonical),
            legacy_precommit_kind_for(*kind),
            "legacy precommit kind drift for {}",
            kind.as_str()
        );
    }
}

#[test]
fn paired_precommit_diagnostic_status_parity() {
    use intent_model::PrecommitSubjectResolutionStatus as Status;
    // Expected statuses mirror the legacy adapter's inline table in
    // spec_system_workspace.rs (evaluate_paired_precommit_objectives).
    let cases = [
        ("spec_graph_selector_ambiguous", Some(Status::Ambiguous)),
        ("spec_graph_selector_not_found", Some(Status::Missing)),
        ("spec_graph_rust_inventory_partial", Some(Status::Partial)),
        (
            "spec_graph_subject_non_executable",
            Some(Status::Unsupported),
        ),
        (
            "spec_graph_subject_generated_or_parameterized",
            Some(Status::Unsupported),
        ),
        ("spec_graph_selector_malformed", Some(Status::Unsupported)),
        (
            "spec_graph_selector_cfg_or_feature_unknown",
            Some(Status::Unsupported),
        ),
        ("spec_graph_something_else", None),
    ];
    for (code, expected) in cases {
        let diagnostic = GraphDiagnosticV1 {
            code: code.to_string(),
            subject: "subject-1".to_string(),
            message: "diagnostic".to_string(),
        };
        let resolution = subject_resolution_from_diagnostic(&diagnostic);
        assert_eq!(
            resolution.as_ref().map(|r| r.status),
            expected,
            "code {code}"
        );
        if let Some(resolution) = resolution {
            assert_eq!(resolution.id.as_str(), "subject-1", "code {code}");
        }
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
