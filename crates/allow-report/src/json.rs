use allow_core::{MatchOutcome, json_escape};

use crate::{
    ARTIFACT_STATUS_FAILED, ARTIFACT_STATUS_PASSED, ArtifactContract, CLAIM_BOUNDARY,
    InventoryContext, SCANNER_LIMITATIONS,
};

pub(crate) fn push_json_artifact_header(
    out: &mut String,
    contract: ArtifactContract,
    command: &str,
) {
    if let Some(fixed_command) = contract.fixed_command {
        debug_assert_eq!(fixed_command, command);
    }
    let schema_version = contract.schema_version;
    out.push_str(&format!("  \"schema_version\": {schema_version},\n"));
    out.push_str(&format!(
        "  \"schema_id\": \"{}\",\n",
        json_escape(contract.schema_id)
    ));
    out.push_str("  \"tool\": \"cargo-allow\",\n");
    out.push_str(&format!("  \"command\": \"{}\",\n", json_escape(command)));
}

pub(crate) fn push_json_artifact_preamble(
    out: &mut String,
    contract: ArtifactContract,
    command: &str,
    inventory: InventoryContext<'_>,
) {
    push_json_artifact_header(out, contract, command);
    push_json_artifact_source_context(out, inventory);
}

pub(crate) fn push_json_fixed_artifact_preamble(
    out: &mut String,
    contract: ArtifactContract,
    inventory: InventoryContext<'_>,
) {
    let Some(command) = contract.fixed_command else {
        std::panic::panic_any("fixed artifact preamble requires a fixed-command artifact contract");
    };
    push_json_artifact_preamble(out, contract, command, inventory);
}

pub(crate) fn push_json_artifact_source_context(out: &mut String, inventory: InventoryContext<'_>) {
    out.push_str(&format!(
        "  \"claim_boundary\": {},\n",
        render_claim_boundary_json()
    ));
    out.push_str(&format!(
        "  \"scanner_limitations\": {},\n",
        render_scanner_limitations_json()
    ));
    out.push_str("  \"inventory\": ");
    out.push_str(&render_inventory_json(inventory, "  "));
    out.push_str(",\n");
}

pub(crate) fn push_json_status_fields(out: &mut String, failed: bool) {
    push_json_status_fields_with_status(
        out,
        if failed {
            ARTIFACT_STATUS_FAILED
        } else {
            ARTIFACT_STATUS_PASSED
        },
        failed,
    );
}

pub(crate) fn push_json_status_fields_with_status(out: &mut String, status: &str, failed: bool) {
    out.push_str(&format!("  \"status\": \"{}\",\n", json_escape(status)));
    out.push_str(&format!("  \"failed\": {},\n", bool_json(failed)));
}

pub(crate) fn push_json_source_context_properties(
    out: &mut String,
    inventory: InventoryContext<'_>,
    indent: &str,
) {
    out.push_str(&format!("{indent}\"inventory\": "));
    out.push_str(&render_inventory_json(inventory, indent));
    out.push_str(",\n");
    out.push_str(&format!(
        "{indent}\"claim_boundary\": {},\n",
        render_claim_boundary_json()
    ));
    out.push_str(&format!(
        "{indent}\"scanner_limitations\": {}\n",
        render_scanner_limitations_json()
    ));
}

pub(crate) fn option_json(value: Option<&str>) -> String {
    value
        .map(|v| format!("\"{}\"", json_escape(v)))
        .unwrap_or_else(|| "null".to_string())
}

pub(crate) fn bool_json(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

pub(crate) fn option_u32_json(value: Option<u32>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_string())
}

pub(crate) fn option_usize_json(value: Option<usize>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_string())
}

pub(crate) fn render_match_outcome_json(outcome: &MatchOutcome, indent: &str) -> String {
    let fields = MatchOutcomeJsonFields::new(outcome);
    format!(
        "{indent}  {{\n{indent}    \"status\": \"{}\",\n{indent}    \"allow_id\": {},\n{indent}    \"candidate_ids\": {},\n{indent}    \"finding_index\": {},\n{indent}    \"score\": {},\n{indent}    \"message\": \"{}\"\n{indent}  }}",
        fields.status,
        fields.allow_id,
        fields.candidate_ids,
        fields.finding_index,
        fields.score,
        fields.message
    )
}

pub(crate) fn render_match_outcome_json_compact(outcome: &MatchOutcome) -> String {
    let fields = MatchOutcomeJsonFields::new(outcome);
    format!(
        "{{\"status\": \"{}\", \"allow_id\": {}, \"candidate_ids\": {}, \"finding_index\": {}, \"score\": {}, \"message\": \"{}\"}}",
        fields.status,
        fields.allow_id,
        fields.candidate_ids,
        fields.finding_index,
        fields.score,
        fields.message
    )
}

struct MatchOutcomeJsonFields {
    status: &'static str,
    allow_id: String,
    candidate_ids: String,
    finding_index: String,
    score: u32,
    message: String,
}

impl MatchOutcomeJsonFields {
    fn new(outcome: &MatchOutcome) -> Self {
        Self {
            status: outcome.status.as_str(),
            allow_id: option_json(outcome.allow_id.as_deref()),
            candidate_ids: json_string_array(&outcome.candidate_ids),
            finding_index: option_usize_json(outcome.finding_index),
            score: outcome.score,
            message: json_escape(&outcome.message),
        }
    }
}

pub(crate) fn json_string_array<T: AsRef<str>>(values: &[T]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| format!("\"{}\"", json_escape(value.as_ref())))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

pub fn render_claim_boundary_json() -> String {
    json_string_array(CLAIM_BOUNDARY)
}

pub fn render_scanner_limitations_json() -> String {
    json_string_array(SCANNER_LIMITATIONS)
}

pub fn render_inventory_json(context: InventoryContext<'_>, indent: &str) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "{indent}  \"scope\": \"{}\",\n",
        json_escape(context.scope)
    ));
    out.push_str(&format!(
        "{indent}  \"scanner\": \"{}\",\n",
        json_escape(context.scanner)
    ));
    out.push_str(&format!(
        "{indent}  \"source\": \"{}\"",
        json_escape(context.source)
    ));
    if let Some(root) = context.root {
        out.push_str(",\n");
        out.push_str(&format!("{indent}  \"root\": \"{}\"", json_escape(root)));
    }
    if let Some(files) = context.files_scanned {
        out.push_str(",\n");
        out.push_str(&format!("{indent}  \"files_scanned\": {files}"));
    }
    if context.empty_git_tracked {
        out.push_str(",\n");
        out.push_str(&format!("{indent}  \"empty_git_tracked\": true"));
    }
    if let Some(completeness) = context.completeness {
        out.push_str(",\n");
        out.push_str(&format!(
            "{indent}  \"completeness\": \"{}\"",
            json_escape(completeness)
        ));
    }
    if let Some(source_identity) = context.source_identity {
        out.push_str(",\n");
        out.push_str(&format!(
            "{indent}  \"source_identity\": \"{}\"",
            json_escape(source_identity)
        ));
    }
    out.push('\n');
    out.push_str(&format!("{indent}}}"));
    out
}
