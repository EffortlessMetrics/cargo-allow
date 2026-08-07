//! RIPR grip receipt validation, currentness, and requirement-grip comparison (#2556).
//!
//! Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow);
//! `proof-adapter-ripr` validates captured RIPR grip receipts, evaluates currentness,
//! and compares provider facts with intent-owned evidence purposes without importing
//! RIPR crates or intent application code. It does not scan source files, does not
//! invoke Cargo, compile code, execute repository code, spawn processes, or
//! depend on intent crates.

#[cfg(test)]
mod boundary;
mod grip_comparison;
mod grip_receipt;
mod parity;
mod receipt_currentness;
mod ripr_adapter;

#[cfg(test)]
mod tests;

pub use grip_comparison::{
    GripComparisonDispositionV1, GripComparisonError, REQUIREMENT_GRIP_COMPARISON_SCHEMA_ID,
    RequirementEvidencePurposeV1, RequirementGripComparisonV1, compare_requirement_grip,
};
pub use grip_receipt::{
    RIPR_GRIP_RECEIPT_SCHEMA_ID, RiprCompletenessV1, RiprExecutionModeV1, RiprGripDispositionV1,
    RiprGripReceiptError, RiprGripReceiptV1, validate_ripr_grip_receipt,
};
pub use parity::{
    GripComparisonParityContract, GripReceiptParityContract, load_grip_comparison_parity_contract,
    load_grip_receipt_parity_contract, parity_contract_path, parity_contract_paths,
};
pub use receipt_currentness::{
    RIPR_RECEIPT_CURRENTNESS_SCHEMA_ID, RiprReceiptCurrentnessReportV1,
    RiprReceiptCurrentnessStatusV1, evaluate_receipt_currentness,
};
pub use ripr_adapter::{RIPR_PROOF_PROVIDER_ID, RiprProofProviderV1};
