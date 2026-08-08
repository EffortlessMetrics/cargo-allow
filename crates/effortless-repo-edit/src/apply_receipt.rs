//! Generic single-target apply receipt (#2602-C).

use crate::error::json_escape;

pub const APPLY_RECEIPT_SCHEMA_VERSION: u32 = 1;
pub const APPLY_RECEIPT_SCHEMA_ID: &str = "repo-edit.apply-receipt.v1";

pub const APPLY_RECEIPT_CLAIM_BOUNDARY: &str = "Bounded filesystem apply evidence only: records \
     containment-checked target identity, digests, and per-target outcome. Does not validate \
     cargo-allow ledger semantics, cargo-intent graph settlement, or authorize merge.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomicityClass {
    AtomicSingleTarget,
}

impl AtomicityClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AtomicSingleTarget => "atomic_single_target",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyOperation {
    Create,
    Replace,
}

impl ApplyOperation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Replace => "replace",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetOutcome {
    Applied,
    Failed,
}

impl TargetOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::Failed => "failed",
        }
    }
}

/// Portable apply receipt for one repository-contained target (#2602 issue).
#[derive(Debug, Clone)]
pub struct ApplyReceiptV1 {
    pub tool_version: String,
    pub repository_root: String,
    pub target_requested: String,
    pub target_canonical: String,
    pub operation: ApplyOperation,
    pub atomicity_class: AtomicityClass,
    pub preconditions_checked: Vec<&'static str>,
    pub bytes_before_digest: Option<String>,
    pub bytes_after_digest: Option<String>,
    pub lock_identity: Option<String>,
    pub outcome: TargetOutcome,
    pub caller_reference: Option<String>,
    pub limitations: Vec<String>,
    pub error_detail: Option<String>,
}

impl ApplyReceiptV1 {
    pub fn applied(&self) -> bool {
        self.outcome == TargetOutcome::Applied
    }
}

pub fn render_apply_receipt_json(receipt: &ApplyReceiptV1, indent: &str) -> String {
    format!(
        "{{\n\
         {indent}    \"schema_id\": \"{}\",\n\
         {indent}    \"schema_version\": {},\n\
         {indent}    \"tool_version\": \"{}\",\n\
         {indent}    \"repository_root\": \"{}\",\n\
         {indent}    \"target_requested\": \"{}\",\n\
         {indent}    \"target_canonical\": \"{}\",\n\
         {indent}    \"operation\": \"{}\",\n\
         {indent}    \"atomicity_class\": \"{}\",\n\
         {indent}    \"preconditions_checked\": {},\n\
         {indent}    \"bytes_before_digest\": {},\n\
         {indent}    \"bytes_after_digest\": {},\n\
         {indent}    \"lock_identity\": {},\n\
         {indent}    \"outcome\": \"{}\",\n\
         {indent}    \"caller_reference\": {},\n\
         {indent}    \"limitations\": {},\n\
         {indent}    \"error_detail\": {},\n\
         {indent}    \"claim_boundary\": \"{}\"\n\
         {indent}  }}",
        json_escape(APPLY_RECEIPT_SCHEMA_ID),
        APPLY_RECEIPT_SCHEMA_VERSION,
        json_escape(&receipt.tool_version),
        json_escape(&receipt.repository_root),
        json_escape(&receipt.target_requested),
        json_escape(&receipt.target_canonical),
        json_escape(receipt.operation.as_str()),
        json_escape(receipt.atomicity_class.as_str()),
        json_string_array_static(&receipt.preconditions_checked),
        option_json_string(&receipt.bytes_before_digest),
        option_json_string(&receipt.bytes_after_digest),
        option_json_string(&receipt.lock_identity),
        json_escape(receipt.outcome.as_str()),
        option_json_string(&receipt.caller_reference),
        json_string_array(&receipt.limitations),
        option_json_string(&receipt.error_detail),
        json_escape(APPLY_RECEIPT_CLAIM_BOUNDARY),
    )
}

fn option_json_string(value: &Option<String>) -> String {
    match value {
        Some(value) => format!("\"{}\"", json_escape(value)),
        None => "null".to_string(),
    }
}

fn json_string_array(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| format!("\"{}\"", json_escape(value)))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn json_string_array_static(values: &[&'static str]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| format!("\"{}\"", json_escape(value)))
            .collect::<Vec<_>>()
            .join(", ")
    )
}
