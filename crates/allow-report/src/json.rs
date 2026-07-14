use allow_core::{MatchOutcome, json_escape};

use crate::artifacts::federation::FederationReportContext;
use crate::{
    ARTIFACT_STATUS_FAILED, ARTIFACT_STATUS_PASSED, ArtifactContract, CLAIM_BOUNDARY,
    InventoryContext, ReportContext, SCANNER_LIMITATIONS,
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

pub(crate) fn push_json_receipt_run_metadata(out: &mut String, context: ReportContext<'_>) {
    if let Some(mode) = context.mode {
        out.push_str(&format!("  \"mode\": \"{}\",\n", json_escape(mode)));
    }
    if let Some(enforcement) = context.enforcement {
        out.push_str(&format!(
            "  \"enforcement\": \"{}\",\n",
            json_escape(enforcement)
        ));
    }
    if let Some(policy_config) = context.policy_config {
        out.push_str(&format!(
            "  \"policy_config\": \"{}\",\n",
            json_escape(policy_config)
        ));
    }
    if let Some(tool_version) = context.tool_version {
        out.push_str(&format!(
            "  \"tool_version\": \"{}\",\n",
            json_escape(tool_version)
        ));
    }
    if let Some(lane_posture) = context.lane_posture {
        out.push_str("  \"lane_posture\": {\n");
        for (index, (lane, mode)) in lane_posture.iter().enumerate() {
            let comma = if index + 1 == lane_posture.len() {
                ""
            } else {
                ","
            };
            out.push_str(&format!(
                "    \"{}\": \"{}\"{comma}\n",
                json_escape(lane),
                json_escape(mode.as_str())
            ));
        }
        out.push_str("  },\n");
    }
    if let Some(federation) = context.federation {
        push_json_federation_context(out, federation);
    }
    // Provenance binding (#1850): emit git_sha and policy_digest when
    // available so consumers can verify a receipt matches a specific commit
    // and policy state.
    if let Some(git_sha) = context.git_sha {
        out.push_str(&format!("  \"git_sha\": \"{}\",\n", json_escape(git_sha)));
    }
    if let Some(policy_digest) = context.policy_digest {
        out.push_str(&format!(
            "  \"policy_digest\": \"{}\",\n",
            json_escape(policy_digest)
        ));
    }
    // Run provenance (#1854): started_at + run_id so a consumer can correlate a
    // receipt to a specific CI run / wall-clock time. Receipts with timestamps
    // are NOT byte-stable across runs (documented).
    if let Some(started_at) = context.started_at {
        out.push_str(&format!(
            "  \"started_at\": \"{}\",\n",
            json_escape(started_at)
        ));
    }
    if let Some(run_id) = context.run_id {
        out.push_str(&format!("  \"run_id\": \"{}\",\n", json_escape(run_id)));
    }
}

pub(crate) fn push_json_federation_context(
    out: &mut String,
    federation: FederationReportContext<'_>,
) {
    out.push_str("  \"federation\": {\n");
    out.push_str(&format!(
        "    \"federation_version\": {},\n",
        option_json(federation.federation_version)
    ));
    out.push_str(&format!(
        "    \"precedence_applied\": {},\n",
        option_json(federation.precedence_applied)
    ));
    out.push_str("    \"ledger_contributors\": [\n");
    if let Some(contributors) = federation.ledger_contributors {
        for (index, contributor) in contributors.iter().enumerate() {
            if index > 0 {
                out.push_str(",\n");
            }
            out.push_str("      {\n");
            out.push_str(&format!(
                "        \"id\": \"{}\",\n",
                json_escape(contributor.id)
            ));
            out.push_str(&format!(
                "        \"path\": \"{}\",\n",
                json_escape(contributor.path)
            ));
            out.push_str(&format!(
                "        \"role\": \"{}\",\n",
                json_escape(contributor.role)
            ));
            out.push_str(&format!(
                "        \"dialect\": \"{}\",\n",
                json_escape(contributor.dialect)
            ));
            out.push_str(&format!(
                "        \"mode\": \"{}\",\n",
                json_escape(contributor.mode)
            ));
            out.push_str(&format!(
                "        \"priority\": {},\n",
                contributor.priority
            ));
            out.push_str(&format!(
                "        \"lanes\": {}",
                json_string_array(contributor.lanes)
            ));
            out.push_str("\n      }");
        }
    }
    out.push_str("\n    ]");
    if let Some(summary) = federation.divergence_summary {
        out.push_str(",\n    \"divergence_summary\": {\n");
        out.push_str("      \"counts_by_kind\": [\n");
        if let Some(counts) = summary.counts_by_kind {
            for (index, count) in counts.iter().enumerate() {
                if index > 0 {
                    out.push_str(",\n");
                }
                out.push_str("        {\n");
                out.push_str(&format!(
                    "          \"kind\": \"{}\",\n",
                    json_escape(count.kind)
                ));
                out.push_str(&format!("          \"count\": {}\n", count.count));
                out.push_str("        }");
            }
        }
        out.push_str("\n      ],\n");
        out.push_str("      \"records\": [\n");
        if let Some(records) = summary.records {
            for (index, record) in records.iter().enumerate() {
                if index > 0 {
                    out.push_str(",\n");
                }
                out.push_str("        {\n");
                out.push_str(&format!(
                    "          \"kind\": \"{}\",\n",
                    json_escape(record.kind)
                ));
                out.push_str(&format!(
                    "          \"message\": \"{}\",\n",
                    json_escape(record.message)
                ));
                out.push_str(&format!(
                    "          \"canonical_ledger_id\": \"{}\",\n",
                    json_escape(record.canonical_ledger_id)
                ));
                out.push_str(&format!(
                    "          \"mirror_ledger_id\": \"{}\",\n",
                    json_escape(record.mirror_ledger_id)
                ));
                out.push_str(&format!(
                    "          \"canonical_path\": \"{}\",\n",
                    json_escape(record.canonical_path)
                ));
                out.push_str(&format!(
                    "          \"mirror_path\": \"{}\",\n",
                    json_escape(record.mirror_path)
                ));
                out.push_str(&format!(
                    "          \"sample_entry_ids\": {},\n",
                    json_string_array(record.sample_entry_ids)
                ));
                out.push_str(&format!(
                    "          \"canonical_fingerprint\": {},\n",
                    option_json(record.canonical_fingerprint)
                ));
                out.push_str(&format!(
                    "          \"mirror_fingerprint\": {},\n",
                    option_json(record.mirror_fingerprint)
                ));
                out.push_str(&format!(
                    "          \"recommended_action\": \"{}\"\n",
                    json_escape(record.recommended_action)
                ));
                out.push_str("        }");
            }
        }
        out.push_str("\n      ]\n");
        out.push_str("    }");
    }
    out.push_str("\n  },\n");
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
    out.push('\n');
    out.push_str(&format!("{indent}}}"));
    out
}
