//! Exact-snapshot evidence orchestration shell (#2589-B).
//!
//! Most users should use `cargo proof` through the cargo subcommand alias;
//! [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow) remains the
//! direct source-tree exception ledger during extraction. `cargo-proof` is the
//! product shell for config entrypoint, renderer framework, process exit mapping,
//! and proof-engine plan/dry-run wiring. It does not scan source files, does not
//! invoke Cargo, compile code, execute repository code, or spawn processes.
//!
//! Dependency boundary: the `intent-protocol` dependency is the accepted public
//! obligation-transport seam consumed through proof-engine (#3310) — a public
//! protocol edge, not cargo-intent application coupling. No cargo-intent
//! application crate (`intent-model`, `intent-engine`/`intent-compiler`,
//! `intent-edit`, `cargo-intent`) and no cargo-allow product crate is a
//! dependency. This records the current dependency graph only; it does not
//! stabilize sibling APIs or promote experimental provider support. Canonical
//! dependency direction lives in
//! `docs/adr/CARGO-ALLOW-ADR-0002-three-product-ownership.md`.

mod config;
mod dry_run_cmd;
mod exit;
mod identity;
mod plan;
mod providers;
mod receipt_projection;
mod receipt_status;
mod render;

pub use config::{ConfigProfileV1, ProofConfigV1, load_config};
pub use dry_run_cmd::{
    DRY_RUN_CLAIM_BOUNDARY, DRY_RUN_FRAME_SCHEMA_ID, dry_run_from_plan_path, render_dry_run_frame,
};
pub use exit::{
    ProcessExitFamilyV1, exit_code_for_family, exit_code_for_result_class,
    exit_family_for_result_class,
};
pub use identity::{
    PRODUCT_CLAIM_BOUNDARY, PRODUCT_ID, PRODUCT_IDENTITY_SCHEMA_ID, ProductIdentityV1,
    load_product_identity_fixture_toml,
};
pub use plan::{
    PLAN_CLAIM_BOUNDARY, PLAN_FRAME_SCHEMA_ID, PlanOutcomeV1, PlanV2OutcomeV1,
    plan_from_obligation_path, plan_v2_from_paths, plan_v2_from_selected_registry,
    render_plan_frame, render_plan_v2_frame,
};
pub use providers::{
    PROVIDER_REGISTRY_SCHEMA_ID, ProviderAvailabilityV1, ProviderDispositionV1,
    ProviderProjectionV1, ProviderRegistryError, StaticProviderRegistryV1,
};
pub use receipt_projection::{
    RECEIPT_EXPLAIN_SCHEMA_ID, RECEIPT_RECONCILE_SCHEMA_ID, ReceiptExplainItemV1,
    ReceiptExplainProjectionV1, ReceiptProjectionError, ReceiptReconcileItemV1,
    ReceiptReconcileProjectionV1, explain_receipt_item, reconcile_receipts, render_receipt_explain,
    render_receipt_reconcile,
};
pub use receipt_status::{
    CapturedReceiptInputsV1, ReceiptCommandError, captured_receipt_inputs_from_paths,
    captured_receipt_status_from_paths, receipt_validation_satisfies_plan,
    render_captured_receipt_status, render_captured_receipt_validation,
};
pub use render::{
    DryRunFrameV1, IdentityFrameV1, OutputFormat, PlanFrameV1, RenderFrame, emit_frame,
};

#[cfg(test)]
mod semantic_routing_guard_tests;
#[cfg(test)]
mod tests;
