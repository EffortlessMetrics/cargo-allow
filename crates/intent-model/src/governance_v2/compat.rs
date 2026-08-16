//! V1 authority compatibility parsing (#2942 step 1 / #3327).
//!
//! Reads the current allow-policy-owned governance authority TOML into the
//! V2 DTOs so callers can migrate without churn. Parsing is tolerant of
//! extra authority fields (the authorities carry producer/result fields the
//! V2 references do not model) but strict about the fields it maps: unknown
//! enum values and missing required fields fail loudly.
//!
//! This module parses supplied text only; it never touches the filesystem.

use serde::Deserialize;

use super::dependency_law::{GovernanceForbiddenEdgeV2, GovernanceRequiredEdgeV2};
use super::identity::{GovernanceCrateIdentityV2, GovernanceCrateRoleV2, GovernanceOwnerV2};
use super::package_posture::{
    CandidateMembershipV2, GovernancePackagePostureV2, ProductPostureV2, PublicationStateV2,
    VersionSourceV2,
};
use super::transitions::{
    MoveReferenceV2, ParityDispositionV2, ParityReferenceV2, ShimReferenceV2, ShimStatusV2,
    TransitionExpiryV2,
};

/// Parse `policy/product-crates-v2.toml` `[[crate_identity]]` rows.
pub fn parse_crate_identities_v1(text: &str) -> Result<Vec<GovernanceCrateIdentityV2>, String> {
    #[derive(Deserialize)]
    struct ManifestToml {
        #[serde(default)]
        crate_identity: Vec<CrateIdentityToml>,
    }
    #[derive(Deserialize)]
    struct CrateIdentityToml {
        logical_id: String,
        workspace_path: String,
        #[serde(default)]
        workspace_dependency_aliases: Vec<String>,
        cargo_package_name: String,
        rust_library_name: String,
        product_or_shared_owner: String,
        crate_role: String,
    }
    let manifest: ManifestToml =
        toml::from_str(text).map_err(|err| format!("parse manifest: {err}"))?;
    manifest
        .crate_identity
        .into_iter()
        .map(|row| {
            let identity = GovernanceCrateIdentityV2 {
                logical_id: row.logical_id,
                workspace_path: row.workspace_path,
                workspace_dependency_aliases: row.workspace_dependency_aliases,
                cargo_package_name: row.cargo_package_name,
                rust_library_name: row.rust_library_name,
                owner: GovernanceOwnerV2::parse(&row.product_or_shared_owner)
                    .map_err(|err| format!("crate_identity owner: {err}"))?,
                role: GovernanceCrateRoleV2::parse(&row.crate_role)
                    .map_err(|err| format!("crate_identity role: {err}"))?,
            };
            identity.validate()?;
            Ok(identity)
        })
        .collect()
}

/// Parse `policy/product-package-topology-v2.toml` `[[package]]` rows.
pub fn parse_package_postures_v1(text: &str) -> Result<Vec<GovernancePackagePostureV2>, String> {
    #[derive(Deserialize)]
    struct TopologyToml {
        #[serde(default)]
        package: Vec<PackageToml>,
    }
    #[derive(Deserialize)]
    struct PackageToml {
        logical_id: String,
        cargo_package_name: String,
        version_line: String,
        product_family: String,
        posture: String,
        package_version: String,
        version_source: String,
        publication_state: String,
        publish: bool,
        candidate_inclusion: bool,
        release_order: u32,
        ci_lane: String,
        support_tier: String,
        asset_roots: Vec<String>,
        extraction_destination: String,
    }
    let topology: TopologyToml =
        toml::from_str(text).map_err(|err| format!("parse topology: {err}"))?;
    topology
        .package
        .into_iter()
        .map(|row| {
            let posture = GovernancePackagePostureV2 {
                logical_id: row.logical_id,
                cargo_package_name: row.cargo_package_name,
                version_line: row.version_line,
                product_family: row.product_family,
                posture: ProductPostureV2::parse(&row.posture)
                    .map_err(|err| format!("package posture: {err}"))?,
                package_version: row.package_version,
                version_source: VersionSourceV2::parse(&row.version_source)
                    .map_err(|err| format!("package version source: {err}"))?,
                publication_state: PublicationStateV2::parse(&row.publication_state)
                    .map_err(|err| format!("package publication state: {err}"))?,
                membership: CandidateMembershipV2 {
                    candidate_inclusion: row.candidate_inclusion,
                    publish: row.publish,
                },
                release_order: row.release_order,
                ci_lane: row.ci_lane,
                support_tier: row.support_tier,
                asset_roots: row.asset_roots,
                extraction_destination: row.extraction_destination,
            };
            posture.validate()?;
            Ok(posture)
        })
        .collect()
}

/// Parse `policy/extraction-shims.toml` `[[shim]]` rows into shim
/// references plus their transition expiry records.
pub fn parse_shim_references_v1(
    text: &str,
) -> Result<(Vec<ShimReferenceV2>, Vec<TransitionExpiryV2>), String> {
    #[derive(Deserialize)]
    struct ShimsToml {
        #[serde(default)]
        shim: Vec<ShimToml>,
    }
    #[derive(Deserialize)]
    struct ShimToml {
        id: String,
        old_identity: String,
        new_identity: String,
        status: String,
        move_ledger_entry: String,
        controlling_issue: u32,
        latest_allowed_stage: u32,
        removal_condition: String,
        #[serde(default)]
        rollback_note: Option<String>,
    }
    let shims: ShimsToml = toml::from_str(text).map_err(|err| format!("parse shims: {err}"))?;
    let mut references = Vec::with_capacity(shims.shim.len());
    let mut expiries = Vec::with_capacity(shims.shim.len());
    for row in shims.shim {
        let reference = ShimReferenceV2 {
            shim_id: row.id.clone(),
            old_identity: row.old_identity,
            new_identity: row.new_identity,
            status: ShimStatusV2::parse(&row.status)
                .map_err(|err| format!("shim `{}` status: {err}", row.id))?,
            move_ledger_entry: row.move_ledger_entry,
            controlling_issue: row.controlling_issue,
            latest_allowed_stage: row.latest_allowed_stage,
        };
        reference.validate()?;
        let expiry = TransitionExpiryV2 {
            component_id: row.id,
            removal_condition: row.removal_condition,
            rollback_note: row.rollback_note.unwrap_or_default(),
        };
        expiry.validate()?;
        references.push(reference);
        expiries.push(expiry);
    }
    Ok((references, expiries))
}

/// Parse `policy/extraction-parity.toml` `[[case]]` rows.
pub fn parse_parity_references_v1(text: &str) -> Result<Vec<ParityReferenceV2>, String> {
    #[derive(Deserialize)]
    struct ParityToml {
        #[serde(default)]
        case: Vec<CaseToml>,
    }
    #[derive(Deserialize)]
    struct CaseToml {
        id: String,
        stage: String,
        move_ledger_entry: String,
        #[serde(default)]
        shim_id: Option<String>,
        disposition: String,
    }
    let parity: ParityToml = toml::from_str(text).map_err(|err| format!("parse parity: {err}"))?;
    parity
        .case
        .into_iter()
        .map(|row| {
            let reference = ParityReferenceV2 {
                case_id: row.id,
                stage: row.stage,
                move_ledger_entry: row.move_ledger_entry,
                shim_id: row.shim_id,
                disposition: ParityDispositionV2::parse(&row.disposition)
                    .map_err(|err| format!("parity case disposition: {err}"))?,
            };
            reference.validate()?;
            Ok(reference)
        })
        .collect()
}

/// Parse `policy/product-crates.toml` `[[forbidden_crate_dependency]]` and
/// `[[required_crate_dependency]]` rows into V2 dependency-law DTOs.
///
/// The V1 rows use logical crate names directly, so no identity resolution
/// happens here; closure validation maps them against V2 identities.
pub fn parse_dependency_law_v1(
    text: &str,
) -> Result<
    (
        Vec<GovernanceForbiddenEdgeV2>,
        Vec<GovernanceRequiredEdgeV2>,
    ),
    String,
> {
    #[derive(Deserialize)]
    struct LawToml {
        #[serde(default)]
        forbidden_crate_dependency: Vec<ForbiddenToml>,
        #[serde(default)]
        required_crate_dependency: Vec<RequiredToml>,
    }
    #[derive(Deserialize)]
    struct ForbiddenToml {
        from: String,
        to: String,
        #[serde(default)]
        repair_hint: Option<String>,
    }
    #[derive(Deserialize)]
    struct RequiredToml {
        from: String,
        #[serde(default)]
        from_package: Option<String>,
        to: String,
        #[serde(default)]
        rationale_issue: Option<u32>,
    }
    let law: LawToml =
        toml::from_str(text).map_err(|err| format!("parse dependency law: {err}"))?;
    let mut forbidden = Vec::with_capacity(law.forbidden_crate_dependency.len());
    for row in law.forbidden_crate_dependency {
        let edge = GovernanceForbiddenEdgeV2 {
            from_logical_id: row.from,
            to_logical_id: row.to,
            repair_hint: row.repair_hint,
        };
        edge.validate()?;
        forbidden.push(edge);
    }
    let mut required = Vec::with_capacity(law.required_crate_dependency.len());
    for row in law.required_crate_dependency {
        // The V1 required row carries an optional cargo package alias; the
        // V2 DTO is logical-only, so the alias is validated to match the
        // from identity during closure validation instead of here.
        let _ = row.from_package;
        let edge = GovernanceRequiredEdgeV2 {
            from_logical_id: row.from,
            to_logical_id: row.to,
            rationale_issue: row.rationale_issue,
        };
        edge.validate()?;
        required.push(edge);
    }
    Ok((forbidden, required))
}

/// Parse `policy/product-move-ledger.toml` `[[entry]]` rows into move
/// references. Tolerant of the ledger's current/consumer fields; strict on
/// the reference fields.
pub fn parse_move_references_v1(text: &str) -> Result<Vec<MoveReferenceV2>, String> {
    #[derive(Deserialize)]
    struct LedgerToml {
        #[serde(default)]
        entry: Vec<EntryToml>,
    }
    #[derive(Deserialize)]
    struct EntryToml {
        id: String,
        #[serde(default)]
        source_kind: String,
        #[serde(default)]
        current_product: String,
        #[serde(default)]
        current_crate: String,
        #[serde(default)]
        target_product: String,
        #[serde(default)]
        target_crate: String,
    }
    let ledger: LedgerToml =
        toml::from_str(text).map_err(|err| format!("parse move ledger: {err}"))?;
    ledger
        .entry
        .into_iter()
        .map(|row| {
            let reference = MoveReferenceV2 {
                entry_id: row.id,
                source_kind: row.source_kind,
                current_product: row.current_product,
                current_crate: row.current_crate,
                target_product: row.target_product,
                target_crate: row.target_crate,
            };
            reference.validate()?;
            Ok(reference)
        })
        .collect()
}
