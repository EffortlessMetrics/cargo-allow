//! RIPR proof provider (#2556, absorbed into cargo-proof #2938).
mod adapter;
mod grip_comparison;
mod grip_receipt;
mod receipt_currentness;

pub use adapter::{RIPR_PROOF_PROVIDER_ID, RiprProofProviderV1};
pub use grip_comparison::{
    GripComparisonDispositionV1, GripComparisonError, REQUIREMENT_GRIP_COMPARISON_SCHEMA_ID,
    RequirementEvidencePurposeV1, RequirementGripComparisonV1, compare_requirement_grip,
};
pub use grip_receipt::{
    RIPR_GRIP_RECEIPT_SCHEMA_ID, RiprCompletenessV1, RiprExecutionModeV1, RiprGripDispositionV1,
    RiprGripReceiptError, RiprGripReceiptV1, validate_ripr_grip_receipt,
};
pub use receipt_currentness::{
    RIPR_RECEIPT_CURRENTNESS_SCHEMA_ID, RiprReceiptCurrentnessReportV1,
    RiprReceiptCurrentnessStatusV1, evaluate_receipt_currentness,
};
