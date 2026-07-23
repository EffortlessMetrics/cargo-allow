use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BoundedDomainQueriesParityContract {
    pub scenario_id: String,
    pub parity_case: String,
    pub move_ledger_entry: String,
    pub intent_engine_module: String,
    pub required_query_kinds: Vec<String>,
    pub protocol_response_schema: String,
}

pub fn bounded_domain_queries_parity_contract_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/intent-engine/parity-bounded-domain-queries-v1.toml")
}

pub fn bounded_domain_queries_parity_contract_paths(root: &Path) -> Vec<PathBuf> {
    vec![bounded_domain_queries_parity_contract_path(root)]
}

pub fn bounded_domain_query_catalog_fixture_path(root: &Path) -> PathBuf {
    root.join("tests/fixtures/intent-engine/bounded-domain-query-catalog-v1.toml")
}

pub fn load_bounded_domain_queries_parity_contract(
    path: &Path,
) -> Result<BoundedDomainQueriesParityContract, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}

pub fn load_bounded_domain_query_catalog_fixture(root: &Path) -> Result<Vec<String>, String> {
    let path = bounded_domain_query_catalog_fixture_path(root);
    let text =
        std::fs::read_to_string(&path).map_err(|err| format!("read {}: {err}", path.display()))?;
    crate::domain_queries::load_bounded_domain_query_catalog_toml(&text)
}
