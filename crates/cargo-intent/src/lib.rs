mod config;
mod exit;
mod identity;
mod render;

pub use config::{ConfigProfileV1, IntentConfigV1, load_config};
pub use exit::{
    ProcessExitFamilyV1, exit_code_for_family, exit_code_for_result_class,
    exit_family_for_result_class,
};
pub use identity::{
    PRODUCT_CLAIM_BOUNDARY, PRODUCT_ID, PRODUCT_IDENTITY_SCHEMA_ID, ProductIdentityV1,
    load_product_identity_fixture_toml,
};
pub use render::{IdentityFrameV1, OutputFormat, RenderFrame, emit_frame};

#[cfg(test)]
mod tests;
