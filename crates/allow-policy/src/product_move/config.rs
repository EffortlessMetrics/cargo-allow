use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use serde::Deserialize;
use std::path::Path;

pub const PRODUCT_MOVE_LEDGER_SCHEMA_ID: &str = "cargo-allow.three-product-move-ledger.v1";
pub const PRODUCT_MOVE_LEDGER_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductMoveLedger {
    pub schema_id: String,
    pub schema_version: u32,
    pub ledger_id: String,
    pub controlling_issue: u32,
    pub owner_issue: u32,
    pub topology_issue: u32,
    pub architecture_issue: u32,
    pub package_issue: u32,
    pub parity_issue: u32,
    pub shim_issue: u32,
    pub linked_plan: String,
    pub linked_adr: String,
    pub projection: String,
    pub plan: String,
    pub claim_boundary: String,
    pub discovery: MoveDiscovery,
    #[serde(default)]
    pub entry: Vec<MoveEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MoveDiscovery {
    #[serde(default)]
    pub recursive_roots: Vec<String>,
    #[serde(default)]
    pub token_scan_roots: Vec<String>,
    #[serde(default)]
    pub selected_files: Vec<String>,
    #[serde(default)]
    pub filename_tokens: Vec<String>,
    #[serde(default)]
    pub no_new_enforcement: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MoveEntry {
    pub id: String,
    pub source_kind: String,
    #[serde(default)]
    pub current_paths: Vec<String>,
    #[serde(default)]
    pub current_refs: Vec<String>,
    pub current_identity: String,
    pub current_product: String,
    pub current_crate: String,
    #[serde(default)]
    pub current_consumers: Vec<String>,
    pub posture: String,
    pub target_product: String,
    pub target_crate: String,
    pub target_module: String,
    pub disposition: String,
    pub compatibility_strategy: String,
    pub schema_producer_impact: String,
    #[serde(default)]
    pub parity_case_ids: Vec<String>,
    pub cutover_stage: String,
    pub expected_cutover_receipt: String,
    pub old_path_reachability_disposition: String,
    #[serde(default)]
    pub active_shim_ids: Vec<String>,
    pub latest_allowed_shim_stage: String,
    pub duplicate_authority_class: String,
    pub selected_public_producer_after_cutover: String,
    #[serde(default)]
    pub package_ci_docs_impact: Vec<String>,
    pub removal_issue_or_condition: String,
    pub migration_owner_issue: String,
    pub risk: String,
    pub rollback: String,
    pub status: String,
    pub claim_boundary: String,
    pub next_move: String,
    pub deletion_output: String,
}

pub fn parse_product_move_ledger(input: &str) -> CargoAllowResult<ProductMoveLedger> {
    parse_product_move_ledger_at(None, input)
}

pub fn parse_product_move_ledger_at(
    path: Option<&Path>,
    input: &str,
) -> CargoAllowResult<ProductMoveLedger> {
    let ledger = toml::from_str::<ProductMoveLedger>(input).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("failed to parse product move ledger TOML: {error}"),
        )
        .with_toml_span(path, input, error.span())
    })?;

    if ledger.schema_id != PRODUCT_MOVE_LEDGER_SCHEMA_ID {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!(
                "unsupported product move ledger schema_id `{}`; expected `{}`",
                ledger.schema_id, PRODUCT_MOVE_LEDGER_SCHEMA_ID
            ),
        ));
    }
    if ledger.schema_version != PRODUCT_MOVE_LEDGER_SCHEMA_VERSION {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!(
                "unsupported product move ledger schema_version `{}`; expected `{}`",
                ledger.schema_version, PRODUCT_MOVE_LEDGER_SCHEMA_VERSION
            ),
        ));
    }

    Ok(ledger)
}
