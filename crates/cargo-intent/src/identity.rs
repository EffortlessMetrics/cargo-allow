//! cargo-intent product identity (#2599-A).

use serde::{Deserialize, Serialize};

pub const PRODUCT_ID: &str = "cargo-intent";
pub const PRODUCT_IDENTITY_SCHEMA_ID: &str = "cargo-intent.product-identity.v1";
pub const PRODUCT_CLAIM_BOUNDARY: &str = "Authored intent and obligation compiler shell; no source-exception ledger ownership and no proof execution.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductIdentityV1 {
    pub schema_id: String,
    pub product_id: String,
    pub binary_name: String,
    pub crate_version: String,
    pub claim_boundary: String,
}

impl ProductIdentityV1 {
    pub fn current(crate_version: &str) -> Self {
        Self {
            schema_id: PRODUCT_IDENTITY_SCHEMA_ID.to_string(),
            product_id: PRODUCT_ID.to_string(),
            binary_name: PRODUCT_ID.to_string(),
            crate_version: crate_version.to_string(),
            claim_boundary: PRODUCT_CLAIM_BOUNDARY.to_string(),
        }
    }
}

pub fn load_product_identity_fixture_toml(text: &str) -> Result<ProductIdentityV1, String> {
    toml::from_str(text).map_err(|err| format!("parse product identity fixture: {err}"))
}
