//! Durable authored intent and obligation compiler (#2599).
//!
//! Most users should use `cargo intent` through the cargo subcommand alias;
//! [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow) remains the
//! direct source-tree exception ledger during extraction. `cargo-intent` is the
//! product shell for config entrypoint, renderer framework, and process exit
//! mapping. It parses source-tree inputs without executing repository code and
//! does not invoke Cargo, rustc, Clippy, build scripts, proc macros, or proof
//! commands.

mod change;
mod config;
mod exit;
mod governance;
mod identity;
mod render;
mod transport;

pub use change::{CHANGE_STATUS_SCHEMA_ID, ChangeStatusReportV1, change_status_staged_precommit};
pub use config::{ConfigProfileV1, IntentConfigV1, load_config};
pub use exit::{
    ProcessExitFamilyV1, exit_code_for_family, exit_code_for_result_class,
    exit_family_for_result_class,
};
pub use governance::{
    CandidatePackageRowV1, GOVERNANCE_CLAIM_BOUNDARY, GOVERNANCE_RECEIPT_SCHEMA_ID,
    GovernanceAuthorityStateV1, GovernanceReceiptV1, compile_governance_receipt,
    compile_governance_receipt_at,
};
pub use identity::{
    PRODUCT_CLAIM_BOUNDARY, PRODUCT_ID, PRODUCT_IDENTITY_SCHEMA_ID, ProductIdentityV1,
    load_product_identity_fixture_toml,
};
pub use render::{IdentityFrameV1, OutputFormat, RenderFrame, emit_frame};

#[cfg(test)]
mod tests;
