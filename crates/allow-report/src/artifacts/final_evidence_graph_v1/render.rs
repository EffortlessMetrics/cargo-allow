//! Deterministic JSON, digest, and Markdown projections for final evidence.

use super::model::{FinalEvidenceGraphEvaluationV1, FinalEvidenceGraphV1};

/// Render a graph in canonical, human-readable JSON form.
pub fn render_final_evidence_graph_canonical_json(
    graph: &FinalEvidenceGraphV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&graph.canonicalized())
}

/// Render a graph in canonical compact JSON bytes.
pub fn render_final_evidence_graph_canonical_bytes(
    graph: &FinalEvidenceGraphV1,
) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(&graph.canonicalized())
}

/// Compute the SHA-256 digest of the canonical graph bytes.
pub fn final_evidence_graph_digest(
    graph: &FinalEvidenceGraphV1,
) -> Result<String, serde_json::Error> {
    let bytes = render_final_evidence_graph_canonical_bytes(graph)?;
    Ok(allow_core::sha256_v1_bytes(&bytes))
}

/// Render an evaluation as JSON.
pub fn render_final_evidence_evaluation_json(
    evaluation: &FinalEvidenceGraphEvaluationV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(evaluation)
}

#[must_use]
/// Render an evaluation as a deterministic Markdown report.
pub fn render_final_evidence_evaluation_markdown(
    evaluation: &FinalEvidenceGraphEvaluationV1,
) -> String {
    let mut output = String::new();
    output.push_str("# Final release evidence graph\n\n");
    output.push_str(&format!(
        "- Result: `{}`\n",
        markdown_escape(evaluation.result.label())
    ));
    output.push_str(&format!(
        "- Graph digest: `{}`\n",
        markdown_escape(&evaluation.graph_digest)
    ));
    output.push_str(&format!(
        "- Rerun roots: {}\n",
        if evaluation.rerun_roots.is_empty() {
            "none".to_string()
        } else {
            evaluation
                .rerun_roots
                .iter()
                .map(|value| markdown_escape(value))
                .collect::<Vec<_>>()
                .join(", ")
        }
    ));
    output.push_str(&format!(
        "- Rerun owners: {}\n\n",
        if evaluation.rerun_owners.is_empty() {
            "none".to_string()
        } else {
            evaluation
                .rerun_owners
                .iter()
                .map(|value| markdown_escape(value))
                .collect::<Vec<_>>()
                .join(", ")
        }
    ));

    output.push_str("## Evidence nodes\n\n");
    output.push_str(
        "| Evidence | Class | Result | Currentness | Direct | Transitive roots | Owner |\n",
    );
    output.push_str("| --- | --- | --- | --- | --- | --- | --- |\n");
    for node in &evaluation.node_dispositions {
        output.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | {} | {} | {} |\n",
            markdown_escape(&node.evidence_id),
            markdown_escape(node.class.label()),
            markdown_escape(node.result.label()),
            markdown_escape(node.currentness.label()),
            if node.direct_non_current { "yes" } else { "no" },
            if node.root_causes.is_empty() {
                "—".to_string()
            } else {
                node.root_causes
                    .iter()
                    .map(|value| markdown_escape(value))
                    .collect::<Vec<_>>()
                    .join(", ")
            },
            markdown_escape(node.rerun_owner.as_deref().unwrap_or("—"))
        ));
    }

    output.push_str("\n## Findings\n\n");
    if evaluation.findings.is_empty() {
        output.push_str("No findings.\n");
    } else {
        for finding in &evaluation.findings {
            let subject = finding
                .evidence_id
                .as_deref()
                .or(finding.edge.as_deref())
                .unwrap_or("graph");
            output.push_str(&format!(
                "- `{:?}` on `{}`: {}\n",
                finding.kind,
                markdown_escape(subject),
                markdown_escape(&finding.message)
            ));
        }
    }

    output.push_str("\n## Claim boundary\n\n");
    output.push_str(&markdown_escape(&evaluation.claim_boundary));
    output.push('\n');
    output
}

fn markdown_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('`', "\\`")
        .replace('\n', "<br>")
}
