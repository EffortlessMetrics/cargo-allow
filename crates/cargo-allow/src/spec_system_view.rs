use allow_core::{CargoAllowError, CargoAllowResult};
use allow_policy::spec_system::GraphDiagnostic;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::Path;

use crate::spec_system_workspace::{
    SelfHostedGraphCompilation, SelfHostedGraphDiagnostic, compile_self_hosted_graph,
};

const SELF_HOSTED_VIEW_SCHEMA_ID: &str = "cargo_allow.spec_system.self_hosted_graph_view.v1";

pub(crate) fn render_self_hosted_explain(
    root: &Path,
    id: &str,
    json_format: bool,
) -> CargoAllowResult<Option<String>> {
    if !looks_like_self_hosted_id(id) {
        return Ok(None);
    }
    let compilation = compile_self_hosted_graph(root)?;
    if !contains_graph_id(&compilation, id) {
        return Err(CargoAllowError::new(format!(
            "no self-hosted graph object `{id}`"
        )));
    }
    let value = graph_view(&compilation, id)?;
    if json_format {
        serde_json::to_string_pretty(&value)
            .map(|text| Some(format!("{text}\n")))
            .map_err(|error| CargoAllowError::new(format!("failed to render graph JSON: {error}")))
    } else {
        Ok(Some(render_human(id, &value)))
    }
}

fn looks_like_self_hosted_id(id: &str) -> bool {
    id.starts_with("CARGO-ALLOW-SPEC-")
        || id.starts_with("cargo-allow.slice.")
        || id.starts_with("seam:")
        || id.starts_with("evidence:")
        || id.starts_with("subject:")
}

fn contains_graph_id(compilation: &SelfHostedGraphCompilation, id: &str) -> bool {
    compilation
        .graph
        .requirements
        .keys()
        .any(|value| value.as_str() == id)
        || compilation
            .graph
            .slices
            .keys()
            .any(|value| value.as_str() == id)
        || compilation
            .graph
            .seams
            .keys()
            .any(|value| value.as_str() == id)
        || compilation
            .graph
            .evidence_claims
            .keys()
            .any(|value| value.as_str() == id)
        || compilation
            .graph
            .subjects
            .keys()
            .any(|value| value.as_str() == id)
}

fn graph_view(compilation: &SelfHostedGraphCompilation, id: &str) -> CargoAllowResult<Value> {
    let kind = graph_id_kind(compilation, id)
        .ok_or_else(|| CargoAllowError::new(format!("no self-hosted graph object `{id}`")))?;
    let related = related_graph_nodes(compilation, id, kind);
    let result_class = result_class(compilation);
    let next_actions = next_actions(&compilation.diagnostics, &compilation.graph.diagnostics);
    Ok(json!({
        "schema_version": 1,
        "schema_id": SELF_HOSTED_VIEW_SCHEMA_ID,
        "tool": "cargo-allow",
        "query": { "id": id, "kind": kind },
        "result_class": result_class,
        "source_basis": {
            "file_inventory_source": compilation.file_inventory.source.as_str(),
            "file_inventory_completeness": compilation.file_inventory.completeness.as_str(),
            "file_count": compilation.file_inventory.files.len(),
            "rust_inventory_status": rust_inventory_status(compilation),
            "rust_inventory_diagnostic_count": compilation.inventory.diagnostics.len(),
            "graph_snapshot_id": compilation.graph.snapshot_id.as_str(),
        },
        "relationships": related,
        "findings": findings(&compilation.diagnostics, &compilation.graph.diagnostics),
        "next_actions": next_actions,
        "claim_boundary": [
            "This view proves current structural linkage from retained source files.",
            "It does not prove test relevance, execution, semantic acceptance, or support promotion."
        ]
    }))
}

fn graph_id_kind(compilation: &SelfHostedGraphCompilation, id: &str) -> Option<&'static str> {
    if compilation
        .graph
        .requirements
        .keys()
        .any(|value| value.as_str() == id)
    {
        Some("requirement")
    } else if compilation
        .graph
        .slices
        .keys()
        .any(|value| value.as_str() == id)
    {
        Some("slice")
    } else if compilation
        .graph
        .seams
        .keys()
        .any(|value| value.as_str() == id)
    {
        Some("seam")
    } else if compilation
        .graph
        .evidence_claims
        .keys()
        .any(|value| value.as_str() == id)
    {
        Some("evidence")
    } else if compilation
        .graph
        .subjects
        .keys()
        .any(|value| value.as_str() == id)
    {
        Some("subject")
    } else {
        None
    }
}

fn related_graph_nodes(compilation: &SelfHostedGraphCompilation, id: &str, kind: &str) -> Value {
    let graph = &compilation.graph;
    let mut requirement_ids = BTreeSet::new();
    let mut slice_ids = BTreeSet::new();
    let mut seam_ids = BTreeSet::new();
    let mut evidence_ids = BTreeSet::new();
    let mut subject_ids = BTreeSet::new();

    match kind {
        "requirement" => {
            requirement_ids.insert(id.to_string());
            for slice in graph.slices.values() {
                if slice
                    .requirement_ids
                    .iter()
                    .any(|value| value.as_str() == id)
                {
                    slice_ids.insert(slice.id.as_str().to_string());
                }
            }
        }
        "slice" => {
            slice_ids.insert(id.to_string());
            if let Some(slice) = graph.slices.get(&find_slice_id(graph, id)) {
                requirement_ids.extend(
                    slice
                        .requirement_ids
                        .iter()
                        .map(|value| value.as_str().to_string()),
                );
                seam_ids.extend(slice.owned_seams.iter().cloned());
                seam_ids.extend(slice.shared_seams.iter().cloned());
            }
        }
        "seam" => {
            seam_ids.insert(id.to_string());
            for slice in graph.slices.values() {
                if slice.owned_seams.contains(id) || slice.shared_seams.contains(id) {
                    slice_ids.insert(slice.id.as_str().to_string());
                }
            }
        }
        "evidence" => {
            evidence_ids.insert(id.to_string());
        }
        "subject" => {
            subject_ids.insert(id.to_string());
        }
        _ => {}
    }

    for claim in graph.evidence_claims.values() {
        let relevant = (kind == "requirement" && claim.requirement_id.as_str() == id)
            || (kind == "slice" && claim.slice_id.as_str() == id)
            || (kind == "seam" && claim.seam_id.as_str() == id)
            || (kind == "evidence" && claim.id.as_str() == id)
            || (kind == "subject"
                && (claim.subject_ids.iter().any(|value| value.as_str() == id)
                    || claim
                        .related_subject_ids
                        .iter()
                        .any(|value| value.as_str() == id)));
        if relevant {
            evidence_ids.insert(claim.id.as_str().to_string());
            requirement_ids.insert(claim.requirement_id.as_str().to_string());
            slice_ids.insert(claim.slice_id.as_str().to_string());
            seam_ids.insert(claim.seam_id.as_str().to_string());
            subject_ids.extend(
                claim
                    .subject_ids
                    .iter()
                    .map(|value| value.as_str().to_string()),
            );
            subject_ids.extend(
                claim
                    .related_subject_ids
                    .iter()
                    .map(|value| value.as_str().to_string()),
            );
        }
    }

    json!({
        "requirements": graph.requirements.iter()
            .filter(|(key, _)| requirement_ids.contains(key.as_str()))
            .map(|(_, node)| json!({
                "id": node.id.as_str(),
                "generation": node.generation,
                "status": node.status,
                "claim_class": node.claim_class,
                "source": node.source,
            })).collect::<Vec<_>>(),
        "slices": graph.slices.iter()
            .filter(|(key, _)| slice_ids.contains(key.as_str()))
            .map(|(_, node)| json!({
                "id": node.id.as_str(),
                "generation": node.generation,
                "change_class": node.change_class,
                "implementation_claim_status": node.implementation_claim_status,
                "evidence_state": node.evidence_state,
                "support_claim_state": node.support_claim_state,
                "requirement_ids": node.requirement_ids,
                "owned_seams": node.owned_seams,
                "shared_seams": node.shared_seams,
                "forbidden_seams": node.forbidden_seams,
            })).collect::<Vec<_>>(),
        "seams": graph.seams.iter()
            .filter(|(key, _)| seam_ids.contains(key.as_str()))
            .map(|(_, node)| json!({
                "id": node.id.as_str(),
                "owner": node.owner,
                "operation": node.operation,
                "source": node.source,
            })).collect::<Vec<_>>(),
        "evidence": graph.evidence_claims.iter()
            .filter(|(key, _)| evidence_ids.contains(key.as_str()))
            .map(|(_, node)| json!({
                "id": node.id.as_str(),
                "purpose": node.purpose,
                "requirement_id": node.requirement_id,
                "slice_id": node.slice_id,
                "seam_id": node.seam_id,
                "subject_ids": node.subject_ids,
                "related_subject_ids": node.related_subject_ids,
                "source": node.source,
            })).collect::<Vec<_>>(),
        "subjects": graph.subjects.iter()
            .filter(|(key, _)| subject_ids.contains(key.as_str()))
            .map(|(_, node)| json!({
                "id": node.id.as_str(),
                "role": node.role,
                "package": node.package,
                "target": node.target,
                "module_path": node.module_path,
                "test_name": node.test_name,
                "source": node.source,
                "source_identity": node.source_identity,
            })).collect::<Vec<_>>(),
    })
}

fn find_slice_id(
    graph: &allow_policy::spec_system::CompiledSpecGraph,
    id: &str,
) -> allow_policy::spec_system::ImplementationSliceId {
    graph
        .slices
        .keys()
        .find(|value| value.as_str() == id)
        .cloned()
        .unwrap_or_else(|| allow_policy::spec_system::ImplementationSliceId(id.to_string()))
}

fn result_class(compilation: &SelfHostedGraphCompilation) -> &'static str {
    if !compilation.graph.diagnostics.is_empty() {
        "FindingsBlocking"
    } else if compilation.diagnostics.is_empty() {
        "Current"
    } else if compilation
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == "spec_graph_rust_inventory_partial")
    {
        "PartialData"
    } else {
        "FindingsBlocking"
    }
}

fn rust_inventory_status(compilation: &SelfHostedGraphCompilation) -> &'static str {
    match compilation.inventory.status {
        allow_rust::RustTestInventoryStatus::Complete => "complete",
        allow_rust::RustTestInventoryStatus::Partial => "partial",
    }
}

fn findings(
    diagnostics: &[SelfHostedGraphDiagnostic],
    graph_diagnostics: &[GraphDiagnostic],
) -> Vec<Value> {
    let mut values = diagnostics
        .iter()
        .map(|diagnostic| {
            json!({
                "code": diagnostic.code,
                "subject": diagnostic.subject,
                "message": diagnostic.message,
                "source": "workspace_composition",
            })
        })
        .collect::<Vec<_>>();
    values.extend(graph_diagnostics.iter().map(|diagnostic| {
        json!({
            "code": format!("{:?}", diagnostic.code),
            "subject": diagnostic.subject,
            "message": diagnostic.message,
            "source": "compiled_graph",
        })
    }));
    values
}

fn next_actions(
    diagnostics: &[SelfHostedGraphDiagnostic],
    graph_diagnostics: &[GraphDiagnostic],
) -> Vec<String> {
    let mut actions = BTreeSet::new();
    for diagnostic in diagnostics {
        let action = match diagnostic.code {
            "spec_graph_rust_inventory_partial" => {
                "narrow or repair source-only Rust target inventory before claiming exact evidence"
            }
            "spec_graph_selector_not_found" => "restore or remove the missing authored selector",
            "spec_graph_selector_ambiguous" => {
                "qualify the selector with its exact target and module path"
            }
            "spec_graph_subject_non_executable" => {
                "replace the ignored subject with an executable evidence subject"
            }
            "spec_graph_subject_generated_or_parameterized" => {
                "map generated or parameterized proof through a supported exact adapter"
            }
            "spec_graph_subject_cfg_or_feature_unknown" => {
                "resolve the cfg or feature condition before claiming exact evidence"
            }
            _ => "inspect the source-located structural finding and update the retained mapping",
        };
        actions.insert(action.to_string());
    }
    if !graph_diagnostics.is_empty() {
        actions.insert(
            "repair the compiled graph structural finding before promoting the slice".to_string(),
        );
    }
    actions.into_iter().collect()
}

fn render_human(id: &str, value: &Value) -> String {
    let result_class = value
        .get("result_class")
        .and_then(Value::as_str)
        .unwrap_or("InstrumentFailure");
    let source_basis = value.get("source_basis");
    let mut text = String::new();
    text.push_str(&format!(
        "# cargo-allow explain {id} --profile spec-system\n\n"
    ));
    text.push_str(&format!("**Result:** `{result_class}`\n\n"));
    if let Some(source_basis) = source_basis {
        text.push_str("## Source basis\n\n");
        text.push_str(&format!(
            "- Graph snapshot: `{}`\n- File inventory: `{}` files, `{}`\n- Rust inventory: `{}` with `{}` diagnostic(s)\n\n",
            source_basis["graph_snapshot_id"].as_str().unwrap_or(""),
            source_basis["file_count"].as_u64().unwrap_or(0),
            source_basis["file_inventory_completeness"].as_str().unwrap_or(""),
            source_basis["rust_inventory_status"].as_str().unwrap_or(""),
            source_basis["rust_inventory_diagnostic_count"].as_u64().unwrap_or(0),
        ));
    }
    render_relationships(&mut text, value.get("relationships"));
    text.push_str("## Structural findings\n\n");
    let findings = value
        .get("findings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if findings.is_empty() {
        text.push_str("None.\n\n");
    } else {
        for finding in findings {
            text.push_str(&format!(
                "- `{}` (`{}`): {}\n",
                finding["code"].as_str().unwrap_or(""),
                finding["subject"].as_str().unwrap_or(""),
                finding["message"].as_str().unwrap_or(""),
            ));
        }
        text.push('\n');
    }
    text.push_str("## Next actions\n\n");
    for action in value["next_actions"]
        .as_array()
        .cloned()
        .unwrap_or_default()
    {
        text.push_str(&format!("- {}\n", action.as_str().unwrap_or("")));
    }
    text.push('\n');
    text.push_str("## Not proven here\n\n");
    text.push_str("- Test relevance or semantic acceptance\n- Test execution or receipts\n- Support promotion\n\n");
    text.push_str("> Claim boundary: current structural linkage only; this view does not prove test relevance, execution, semantic acceptance, or support promotion.\n");
    text
}

fn render_relationships(text: &mut String, relationships: Option<&Value>) {
    let Some(relationships) = relationships else {
        return;
    };
    for (label, key) in [
        ("Requirements", "requirements"),
        ("Implementation slices", "slices"),
        ("Implementation seams", "seams"),
        ("Evidence", "evidence"),
        ("Subjects", "subjects"),
    ] {
        let rows = relationships[key].as_array().cloned().unwrap_or_default();
        text.push_str(&format!("## {label}\n\n"));
        if rows.is_empty() {
            text.push_str("None.\n\n");
            continue;
        }
        for row in rows {
            let id = row["id"].as_str().unwrap_or("");
            let source = row["source"]["path"].as_str().unwrap_or("");
            let line = row["source"]["line"].as_u64().unwrap_or(0);
            let detail = row
                .get("status")
                .or_else(|| row.get("implementation_claim_status"))
                .or_else(|| row.get("purpose"))
                .or_else(|| row.get("role"))
                .map(|value| value.to_string())
                .unwrap_or_else(|| "".to_string());
            text.push_str(&format!("- `{id}` `{detail}` ({source}:{line})\n"));
        }
        text.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    }

    #[test]
    fn self_hosted_slice_view_has_one_machine_and_human_projection() -> Result<(), String> {
        let id = "cargo-allow.slice.self-hosted-runtime-promotion.v1";
        let human = render_self_hosted_explain(&workspace_root(), id, false)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "expected human graph view".to_string())?;
        for expected in [id, "PartialData", "Subjects", "Not proven"] {
            if !human.contains(expected) {
                return Err(format!("human graph view omitted {expected}: {human}"));
            }
        }
        let machine = render_self_hosted_explain(&workspace_root(), id, true)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "expected JSON graph view".to_string())?;
        let value: Value = serde_json::from_str(&machine).map_err(|error| error.to_string())?;
        if value["query"]["id"] != id || value["query"]["kind"] != "slice" {
            return Err(format!("unexpected graph query: {value}"));
        }
        if value["relationships"]["subjects"]
            .as_array()
            .is_none_or(|values| values.is_empty())
        {
            return Err(format!("expected subject relationships: {value}"));
        }
        Ok(())
    }

    #[test]
    fn non_graph_ids_use_the_existing_spec_system_explain_path() -> Result<(), String> {
        if render_self_hosted_explain(Path::new("."), "doc-artifact", false)
            .map_err(|error| error.to_string())?
            .is_some()
        {
            return Err("legacy artifact id was claimed by graph projection".to_string());
        }
        Ok(())
    }
}
