//! V2 governance DTOs for the repository architecture authority
//! (#2942 step 1 / #3327).
//!
//! Pure authored DTOs and local validation for product/crate identity,
//! package/version/publication/support posture, move/shim/parity/cutover
//! references, target dispositions, and claim boundaries. These DTOs will
//! become the canonical generation-2 governance model consumed by
//! intent-engine reconciliation; allow-policy keeps its V1 authority
//! unchanged during cutover.
//!
//! Boundary: no Cargo process handles, filesystem/Git access, ambient
//! workspace state, release execution state, provider execution state, or
//! cargo-allow Finding/AllowEntry/Selector semantics. Compatibility parsing
//! consumes supplied authority text only.

mod claim;
mod compat;
mod dependency_law;
mod identity;
mod package_posture;
mod transitions;

pub use claim::ClaimBoundaryV2;
pub use compat::{
    parse_crate_identities_v1, parse_dependency_law_v1, parse_move_references_v1,
    parse_package_postures_v1, parse_parity_references_v1, parse_shim_references_v1,
};
pub use dependency_law::{GovernanceForbiddenEdgeV2, GovernanceRequiredEdgeV2};
pub use identity::{
    GovernanceComponentKindV2, GovernanceCrateIdentityV2, GovernanceCrateRoleV2, GovernanceOwnerV2,
    TargetDispositionV2,
};
pub use package_posture::{
    CandidateMembershipV2, GovernancePackagePostureV2, ProductPostureV2, PublicationStateV2,
    VersionSourceV2,
};
pub use transitions::{
    CutoverReferenceV2, MoveReferenceV2, ParityDispositionV2, ParityReferenceV2, ShimReferenceV2,
    ShimStatusV2, TransitionExpiryV2,
};

#[cfg(test)]
mod tests {
    use super::*;

    const CRATE_IDENTITY_FIXTURE: &str = r#"
[[crate_identity]]
logical_id = "proof-engine"
workspace_path = "crates/proof-engine"
workspace_dependency_aliases = ["proof-orchestrator"]
cargo_package_name = "proof-orchestrator"
rust_library_name = "proof_engine"
product_or_shared_owner = "cargo-proof"
crate_role = "CargoProof"
"#;

    const PACKAGE_POSTURE_FIXTURE: &str = r#"
[[package]]
logical_id = "cargo-allow"
cargo_package_name = "cargo-allow"
version_line = "cargo-allow-0.2"
product_family = "cargo-allow"
posture = "CargoAllowSupported"
package_version = "0.2.0"
version_source = "WorkspaceProduct"
publication_state = "UnpublishedInternal"
publish = true
candidate_inclusion = true
release_order = 400
"#;

    const SHIM_FIXTURE: &str = r#"
[[shim]]
id = "shim-proof-engine-crate-scaffold"
old_identity = "cargo-proof::proof_engine_unowned"
new_identity = "proof-engine::boundary"
status = "active"
move_ledger_entry = "introduce-proof-engine-crate"
controlling_issue = 2589
latest_allowed_stage = 1
removal_condition = "issue:#2606 proof-engine stage-1 cutover receipt"
claim_boundary = "Engine orchestration scaffold only."
"#;

    const PARITY_FIXTURE: &str = r#"
[[case]]
id = "parity-proof-engine-boundary-v1"
stage = "ProofEngineAndCli"
move_ledger_entry = "introduce-proof-engine-crate"
shim_id = "shim-proof-engine-crate-scaffold"
old_producer = "cargo-proof::proof_engine_unowned"
new_producer = "proof-engine::boundary"
expected_result = "EquivalentWithCanonicalRenaming"
disposition = "contract_only"
claim_boundary = "Contract only."
"#;

    #[test]
    fn crate_identity_fixture_parses_and_validates() -> Result<(), String> {
        let identities = parse_crate_identities_v1(CRATE_IDENTITY_FIXTURE)?;
        let identity = identities
            .first()
            .ok_or("fixture must yield one crate identity")?;
        if identity.logical_id != "proof-engine" || identity.owner != GovernanceOwnerV2::CargoProof
        {
            return Err(format!("unexpected identity: {identity:?}"));
        }
        identity.validate()
    }

    #[test]
    fn crate_identity_rejects_unknown_owner_and_role() -> Result<(), String> {
        let bad_owner = CRATE_IDENTITY_FIXTURE.replace("cargo-proof\"", "third-party\"");
        if parse_crate_identities_v1(&bad_owner).is_ok() {
            return Err("unknown owner must fail compat parsing".into());
        }
        let bad_role = CRATE_IDENTITY_FIXTURE.replace("CargoProof\"", "NotARole\"");
        if parse_crate_identities_v1(&bad_role).is_ok() {
            return Err("unknown role must fail compat parsing".into());
        }
        Ok(())
    }

    #[test]
    fn package_posture_fixture_parses_and_validates() -> Result<(), String> {
        let postures = parse_package_postures_v1(PACKAGE_POSTURE_FIXTURE)?;
        let posture = postures
            .first()
            .ok_or("fixture must yield one package posture")?;
        if posture.posture != ProductPostureV2::CargoAllowSupported {
            return Err(format!("unexpected posture: {:?}", posture.posture));
        }
        if !posture.membership.candidate_inclusion || !posture.membership.publish {
            return Err("candidate membership must round-trip booleans".into());
        }
        posture.validate()
    }

    #[test]
    fn package_posture_rejects_unknown_enums() -> Result<(), String> {
        let bad = PACKAGE_POSTURE_FIXTURE.replace("CargoAllowSupported\"", "CargoAllowMandatory\"");
        if parse_package_postures_v1(&bad).is_ok() {
            return Err("unknown posture must fail compat parsing".into());
        }
        Ok(())
    }

    #[test]
    fn shim_fixture_parses_with_expiry() -> Result<(), String> {
        let (references, expiries) = parse_shim_references_v1(SHIM_FIXTURE)?;
        let reference = references
            .first()
            .ok_or("fixture must yield one shim reference")?;
        if reference.status != ShimStatusV2::Active {
            return Err(format!("unexpected shim status: {:?}", reference.status));
        }
        let expiry = expiries
            .first()
            .ok_or("fixture must yield one transition expiry")?;
        if !expiry.removal_condition.contains("#2606") {
            return Err(format!("removal condition drift: {expiry:?}"));
        }
        reference.validate()?;
        expiry.validate()
    }

    #[test]
    fn parity_fixture_parses_and_ignores_unmodeled_fields() -> Result<(), String> {
        let references = parse_parity_references_v1(PARITY_FIXTURE)?;
        let reference = references
            .first()
            .ok_or("fixture must yield one parity reference")?;
        if reference.disposition != ParityDispositionV2::ContractOnly {
            return Err(format!(
                "unexpected disposition: {:?}",
                reference.disposition
            ));
        }
        if reference.shim_id.as_deref() != Some("shim-proof-engine-crate-scaffold") {
            return Err("optional shim reference must round-trip".into());
        }
        reference.validate()
    }

    #[test]
    fn parity_reference_without_shim_is_valid() -> Result<(), String> {
        let no_shim =
            PARITY_FIXTURE.replace("shim_id = \"shim-proof-engine-crate-scaffold\"\n", "");
        let references = parse_parity_references_v1(&no_shim)?;
        let reference = references
            .first()
            .ok_or("fixture must yield one parity reference")?;
        if reference.shim_id.is_some() {
            return Err("absent shim_id must stay absent".into());
        }
        reference.validate()
    }

    #[test]
    fn target_disposition_round_trips_strict_vocabulary() -> Result<(), String> {
        for (value, expected) in [
            ("retain_package", TargetDispositionV2::RetainPackage),
            (
                "collapse_into_package",
                TargetDispositionV2::CollapseIntoPackage,
            ),
            ("compatibility_only", TargetDispositionV2::CompatibilityOnly),
            (
                "defer_until_evidence",
                TargetDispositionV2::DeferUntilEvidence,
            ),
            (
                "remove_after_cutover",
                TargetDispositionV2::RemoveAfterCutover,
            ),
        ] {
            let parsed = TargetDispositionV2::parse(value)?;
            if parsed != expected || parsed.as_str() != value {
                return Err(format!("disposition {value} must round-trip"));
            }
        }
        if TargetDispositionV2::parse("keep_forever").is_ok() {
            return Err("unknown disposition must fail".into());
        }
        Ok(())
    }

    #[test]
    fn claim_boundary_requires_claim_and_allows_limitations() -> Result<(), String> {
        let claim = ClaimBoundaryV2 {
            claim: "Data seam only".to_string(),
            limitations: vec!["no semantic evaluation".to_string()],
        };
        claim.validate()?;
        let empty = ClaimBoundaryV2 {
            claim: "  ".to_string(),
            limitations: Vec::new(),
        };
        if empty.validate().is_ok() {
            return Err("blank claim must fail validation".into());
        }
        Ok(())
    }

    #[test]
    fn cutover_reference_requires_receipt() -> Result<(), String> {
        let reference = CutoverReferenceV2 {
            stage: 1,
            product: "cargo-proof".to_string(),
            receipt_id: "cutover-proof-stage-1".to_string(),
        };
        reference.validate()?;
        let blank = CutoverReferenceV2 {
            stage: 1,
            product: "cargo-proof".to_string(),
            receipt_id: String::new(),
        };
        if blank.validate().is_ok() {
            return Err("blank receipt id must fail validation".into());
        }
        Ok(())
    }

    #[test]
    fn dependency_law_round_trips_forbidden_and_required() -> Result<(), String> {
        let (forbidden, required) = parse_dependency_law_v1(
            r#"
[[forbidden_crate_dependency]]
from = "proof-engine"
to = "intent-engine"
repair_hint = "intent-protocol"

[[required_crate_dependency]]
from = "proof-engine"
from_package = "proof-orchestrator"
to = "intent-protocol"
rationale_issue = 2936
"#,
        )?;
        let forbidden_edge = forbidden
            .first()
            .ok_or("fixture must yield one forbidden edge")?;
        if forbidden_edge.from_logical_id != "proof-engine"
            || forbidden_edge.to_logical_id != "intent-engine"
            || forbidden_edge.repair_hint.as_deref() != Some("intent-protocol")
        {
            return Err(format!("forbidden edge drift: {forbidden_edge:?}"));
        }
        let required_edge = required
            .first()
            .ok_or("fixture must yield one required edge")?;
        if required_edge.to_logical_id != "intent-protocol"
            || required_edge.rationale_issue != Some(2936)
        {
            return Err(format!("required edge drift: {required_edge:?}"));
        }
        if parse_dependency_law_v1("[[forbidden_crate_dependency\nfrom = ").is_ok() {
            return Err("malformed law must fail".into());
        }
        Ok(())
    }

    #[test]
    fn live_authorities_parse_through_compat() -> Result<(), String> {
        // The current V1 allow-policy authority must read into the V2 DTOs
        // without behavior change on the allow-policy side.
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let read = |rel: &str| -> Result<String, String> {
            std::fs::read_to_string(root.join(rel)).map_err(|err| format!("read {rel}: {err}"))
        };

        let identities = parse_crate_identities_v1(&read("policy/product-crates-v2.toml")?)?;
        if identities.len() < 20 {
            return Err(format!(
                "expected the full crate identity roster, got {}",
                identities.len()
            ));
        }
        let engine = identities
            .iter()
            .find(|identity| identity.logical_id == "proof-engine")
            .ok_or("proof-engine identity missing from live authority")?;
        if engine.cargo_package_name != "proof-orchestrator"
            || engine.owner != GovernanceOwnerV2::CargoProof
        {
            return Err(format!("proof-engine identity drift: {engine:?}"));
        }

        let postures =
            parse_package_postures_v1(&read("policy/product-package-topology-v2.toml")?)?;
        if postures.is_empty() {
            return Err("live topology must yield package postures".into());
        }

        let (shims, expiries) = parse_shim_references_v1(&read("policy/extraction-shims.toml")?)?;
        if shims.is_empty() || shims.len() != expiries.len() {
            return Err("live shims must parse one-to-one with expiries".into());
        }

        let parity = parse_parity_references_v1(&read("policy/extraction-parity.toml")?)?;
        if parity.len() < 50 {
            return Err(format!(
                "expected the full parity case roster, got {}",
                parity.len()
            ));
        }

        let (forbidden, required) = parse_dependency_law_v1(&read("policy/product-crates.toml")?)?;
        if !forbidden.iter().any(|edge| {
            edge.from_logical_id == "proof-engine" && edge.to_logical_id == "intent-engine"
        }) {
            return Err(
                "live dependency law must retain the proof-engine -> intent-engine edge".into(),
            );
        }
        if !required.iter().any(|edge| {
            edge.from_logical_id == "proof-engine" && edge.to_logical_id == "intent-protocol"
        }) {
            return Err("live dependency law must retain the converged required edge".into());
        }
        Ok(())
    }
}
