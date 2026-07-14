//! Shared provenance envelope for mutation commands (`add`, `propose`,
//! `refresh`, `prune`, `migrate`), per CARGO-ALLOW-SPEC-0008 "Mutation Receipt
//! Envelope" (GOAL-0004 PR 5). Command-specific payloads remain outside this
//! envelope; provenance and changed-entry metadata must not be independently
//! reinvented per command. The envelope is wired into all five PR 5 mutation
//! commands.

use allow_core::json_escape;

use crate::json::{json_string_array, option_json};

pub const MUTATION_RECEIPT_SCHEMA_VERSION: u32 = 1;
pub const MUTATION_RECEIPT_SCHEMA_ID: &str = "cargo-allow.mutation-receipt.v1";

pub const MUTATION_RECEIPT_CLAIM_BOUNDARY: &str = "Provenance envelope only: records what changed and how to verify it. Does not itself \
     validate entry correctness, authorize merge, or change command semantics (GOAL-0004 PR 5, \
     CARGO-ALLOW-SPEC-0008).";

/// Shared mutation-receipt envelope embedded in a mutation command's JSON
/// output. Fields mirror CARGO-ALLOW-SPEC-0008 exactly:
/// `schema_id, operation, tool_version, repo_root, config_source, ledger_ids,
/// changed_allow_ids, before_fingerprints, after_fingerprints, result,
/// next_commands, claim_boundary`.
#[derive(Debug, Clone)]
pub struct MutationReceipt<'a> {
    pub operation: &'static str,
    pub tool_version: &'a str,
    pub repo_root: Option<&'a str>,
    pub config_source: Option<&'a str>,
    pub ledger_ids: Vec<&'a str>,
    pub changed_allow_ids: Vec<&'a str>,
    /// Parallel to `changed_allow_ids`. `None` for an entry with no prior
    /// state (e.g. a newly added entry).
    pub before_fingerprints: Vec<Option<String>>,
    /// Parallel to `changed_allow_ids`.
    pub after_fingerprints: Vec<Option<String>>,
    pub result: &'static str,
    pub next_commands: Vec<String>,
}

/// Renders a [`MutationReceipt`] as a JSON object, indented to nest under a
/// parent object whose own fields are indented by `indent`. Public so every
/// mutation-command JSON path — including compact, non-`AddReport` shapes
/// like `add --glob`'s summary — embeds the same envelope rather than
/// reinventing provenance rendering per call site.
pub fn render_mutation_receipt_json(receipt: &MutationReceipt<'_>, indent: &str) -> String {
    format!(
        "{{\n\
         {indent}    \"schema_id\": \"{}\",\n\
         {indent}    \"operation\": \"{}\",\n\
         {indent}    \"tool_version\": \"{}\",\n\
         {indent}    \"repo_root\": {},\n\
         {indent}    \"config_source\": {},\n\
         {indent}    \"ledger_ids\": {},\n\
         {indent}    \"changed_allow_ids\": {},\n\
         {indent}    \"before_fingerprints\": {},\n\
         {indent}    \"after_fingerprints\": {},\n\
         {indent}    \"result\": \"{}\",\n\
         {indent}    \"next_commands\": {},\n\
         {indent}    \"claim_boundary\": \"{}\"\n\
         {indent}  }}",
        json_escape(MUTATION_RECEIPT_SCHEMA_ID),
        json_escape(receipt.operation),
        json_escape(receipt.tool_version),
        option_json(receipt.repo_root),
        option_json(receipt.config_source),
        json_string_array(&receipt.ledger_ids),
        json_string_array(&receipt.changed_allow_ids),
        json_option_string_array(&receipt.before_fingerprints),
        json_option_string_array(&receipt.after_fingerprints),
        json_escape(receipt.result),
        json_string_array(&receipt.next_commands),
        json_escape(MUTATION_RECEIPT_CLAIM_BOUNDARY),
    )
}

fn json_option_string_array(values: &[Option<String>]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| match value {
                Some(value) => format!("\"{}\"", json_escape(value)),
                None => "null".to_string(),
            })
            .collect::<Vec<_>>()
            .join(", ")
    )
}
