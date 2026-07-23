//! cargo-allow parity corpus anchors aligned with intent-engine (#2586-E).

pub const SPEC_SYSTEM_PROFILE_ID: &str = "spec-system";

pub const DIAGNOSTIC_SELECTOR_NOT_FOUND: &str = "spec_graph_selector_not_found";
pub const DIAGNOSTIC_RUST_INVENTORY_PARTIAL: &str = "spec_graph_rust_inventory_partial";
pub const DIAGNOSTIC_SOURCE_VIEW_PARTIAL: &str = "spec_graph_source_view_partial";

pub const EXIT_PASSED: &str = "Passed:success";
pub const EXIT_BLOCKING_FINDINGS: &str = "FindingsBlocking:blocking";
pub const EXIT_STALE_INPUT: &str = "StaleInput:instrument_failure";
pub const EXIT_MALFORMED_INPUT: &str = "MalformedInput:usage";

pub const MOVEMENT_SEAM_MAPPING_CHANGED: &str = "seam_mapping_changed";
pub const MOVEMENT_SUBJECT_BODY_IDENTITY_CHANGED: &str = "subject_body_identity_changed";

pub const WORKSPACE_COMPOSITION_SELECTOR: &str = "self-hosted-runtime-promotion-v1";

pub fn parity_corpus_anchors() -> [(&'static str, &'static str); 10] {
    [
        ("profile-spec-system", SPEC_SYSTEM_PROFILE_ID),
        (
            "selector-workspace-composition",
            WORKSPACE_COMPOSITION_SELECTOR,
        ),
        (
            "movement-seam-mapping-changed",
            MOVEMENT_SEAM_MAPPING_CHANGED,
        ),
        (
            "movement-subject-body-identity-changed",
            MOVEMENT_SUBJECT_BODY_IDENTITY_CHANGED,
        ),
        (
            "diagnostic-selector-not-found",
            DIAGNOSTIC_SELECTOR_NOT_FOUND,
        ),
        (
            "diagnostic-rust-inventory-partial",
            DIAGNOSTIC_RUST_INVENTORY_PARTIAL,
        ),
        (
            "diagnostic-source-view-partial",
            DIAGNOSTIC_SOURCE_VIEW_PARTIAL,
        ),
        ("exit-passed", EXIT_PASSED),
        ("exit-blocking-findings", EXIT_BLOCKING_FINDINGS),
        ("exit-stale-input", EXIT_STALE_INPUT),
    ]
}
