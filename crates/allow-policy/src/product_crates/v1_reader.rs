//! V1 historical reader: projects generation-1 manifests into V2 shape (#2921).
//!
//! This reader parses a V1 `ArchitectureManifest` and derives V2 identity
//! fields from the V1 conflated names. It emits migration diagnostics for
//! every entry where logical_id, package name, and library name happened to
//! use the same text. A V1 manifest can never produce a current clean V2
//! result — it is always historical.

use allow_core::CargoAllowResult;

use crate::product_crates::config::ArchitectureManifest;
use crate::product_crates::v2::{
    ARCHITECTURE_MANIFEST_V2_AUTHORITY_GENERATION, ARCHITECTURE_MANIFEST_V2_SCHEMA_VERSION,
    ArchitectureManifestV2,
};

/// A diagnostic emitted when projecting a V1 manifest entry into V2 shape.
///
/// Records that the V1 entry conflated logical/package/library identity and
/// was derived heuristically rather than from an explicit V2 record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalReaderDiagnostic {
    pub logical_id: String,
    pub derived_package_name: String,
    pub derived_library_name: String,
    pub note: String,
}

/// Result of reading a V1 manifest as historical input.
///
/// Contains the projected V2 manifest and any migration diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalV1Projection {
    pub manifest: ArchitectureManifestV2,
    pub diagnostics: Vec<HistoricalReaderDiagnostic>,
}

/// Read a V1 architecture manifest as historical input, projecting it into
/// the V2 shape (#2921).
///
/// Derives `logical_id` from the V1 crate name, `cargo_package_name` from the
/// same, and `rust_library_name` by replacing `-` with `_`. Every derived
/// entry produces a [`HistoricalReaderDiagnostic`] noting the heuristic
/// derivation.
pub fn read_v1_as_historical(
    v1: &ArchitectureManifest,
) -> CargoAllowResult<HistoricalV1Projection> {
    let mut crate_identity = Vec::new();
    let mut diagnostics = Vec::new();

    // Collect all crate names from products and shared crates.
    for product in &v1.product {
        let role = infer_role_from_product(&product.id);
        for crate_name in &product.owned_crates {
            push_derived_identity(
                crate_name,
                &product.id,
                role,
                &mut crate_identity,
                &mut diagnostics,
            );
        }
    }
    for shared in &v1.shared_crate {
        push_derived_identity(
            &shared.name,
            "shared",
            shared.role,
            &mut crate_identity,
            &mut diagnostics,
        );
    }

    Ok(HistoricalV1Projection {
        manifest: ArchitectureManifestV2 {
            schema_version: ARCHITECTURE_MANIFEST_V2_SCHEMA_VERSION.to_string(),
            authority_generation: ARCHITECTURE_MANIFEST_V2_AUTHORITY_GENERATION,
            manifest_id: v1.manifest_id.clone(),
            controlling_issue: v1.controlling_issue,
            linked_move_ledger: v1.linked_move_ledger.clone(),
            crate_identity,
        },
        diagnostics,
    })
}

fn push_derived_identity(
    name: &str,
    owner: &str,
    role: crate::product_crates::config::CrateRole,
    crate_identity: &mut Vec<crate::product_crates::v2::CrateIdentityV2>,
    diagnostics: &mut Vec<HistoricalReaderDiagnostic>,
) {
    let derived_library = name.replace('-', "_");
    diagnostics.push(HistoricalReaderDiagnostic {
        logical_id: name.to_string(),
        derived_package_name: name.to_string(),
        derived_library_name: derived_library.clone(),
        note: "V1 conflated identity: logical_id == package_name; library_name derived by dash-to-underscore".to_string(),
    });
    crate_identity.push(crate::product_crates::v2::CrateIdentityV2 {
        logical_id: name.to_string(),
        workspace_path: format!("crates/{name}"),
        workspace_dependency_aliases: vec![name.to_string()],
        cargo_package_name: name.to_string(),
        rust_library_name: derived_library,
        product_or_shared_owner: owner.to_string(),
        crate_role: role,
    });
}

fn infer_role_from_product(product_id: &str) -> crate::product_crates::config::CrateRole {
    match product_id {
        "cargo-allow" => crate::product_crates::config::CrateRole::CargoAllowCore,
        "cargo-intent" => crate::product_crates::config::CrateRole::CargoIntent,
        "cargo-proof" => crate::product_crates::config::CrateRole::CargoProof,
        _ => crate::product_crates::config::CrateRole::CargoAllowCore,
    }
}
