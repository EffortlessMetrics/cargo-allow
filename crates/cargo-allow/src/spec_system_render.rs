use super::*;

pub(super) fn render_spec_system_report(report: &SpecSystemReport, format: OutputFormat) -> String {
    match format {
        OutputFormat::Json => render_spec_system_json(report),
        OutputFormat::Html => format!(
            "<!doctype html><meta charset=\"utf-8\"><title>cargo-allow spec-system</title><pre>{}</pre>\n",
            html_escape(&render_spec_system_markdown(report))
        ),
        OutputFormat::Sarif => render_spec_system_sarif(report),
        OutputFormat::Human | OutputFormat::Markdown => render_spec_system_markdown(report),
    }
}

pub(super) fn filter_spec_system_report_for_artifact(
    report: &SpecSystemReport,
    artifact_id: &str,
) -> CargoAllowResult<SpecSystemReport> {
    let artifact = report
        .artifacts
        .iter()
        .find(|artifact| artifact.id == artifact_id)
        .ok_or_else(|| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::Artifact,
                format!("no spec-system artifact `{artifact_id}`"),
            )
        })?;
    let links = report
        .links
        .iter()
        .filter(|link| spec_system_link_touches_artifact(link, artifact))
        .cloned()
        .collect::<Vec<_>>();
    let findings = report
        .findings
        .iter()
        .filter(|finding| spec_system_message_mentions_artifact(&finding.message, artifact))
        .cloned()
        .collect::<Vec<_>>();
    let work_items = report
        .work_items
        .iter()
        .filter(|item| spec_system_work_item_touches_artifact(item, artifact))
        .cloned()
        .collect::<Vec<_>>();

    Ok(SpecSystemReport {
        command: report.command.clone(),
        root: report.root.clone(),
        config_source: report.config_source.clone(),
        config_provenance: report.config_provenance.clone(),
        mode: report.mode.clone(),
        artifacts: vec![artifact.clone()],
        links,
        support_tier_rows: report.support_tier_rows,
        findings,
        work_items,
        readiness: None,
        federation: report.federation.clone(),
        import_graph: report.import_graph.clone(),
    })
}

fn spec_system_link_touches_artifact(link: &SpecSystemLink, artifact: &SpecSystemArtifact) -> bool {
    link.source_id == artifact.id || link.target == artifact.id || link.target == artifact.path
}

fn spec_system_work_item_touches_artifact(
    item: &SpecSystemWorkItem,
    artifact: &SpecSystemArtifact,
) -> bool {
    item.artifact_id.as_deref() == Some(artifact.id.as_str())
        || item.path.as_deref() == Some(artifact.path.as_str())
        || spec_system_message_mentions_artifact(&item.message, artifact)
}

fn spec_system_message_mentions_artifact(message: &str, artifact: &SpecSystemArtifact) -> bool {
    message.contains(&artifact.id) || message.contains(&artifact.path)
}

pub(super) fn render_spec_system_explain_markdown(report: &SpecSystemReport) -> String {
    let Some(artifact) = report.artifacts.first() else {
        return render_spec_system_markdown(report);
    };
    let mut text = String::new();
    text.push_str(&format!(
        "# cargo-allow explain {} --profile spec-system\n\n",
        artifact.id
    ));
    push_spec_system_report_preamble(&mut text, report);

    text.push_str("## Artifact\n\n");
    text.push_str("| Field | Value |\n|---|---|\n");
    text.push_str(&format!("| ID | `{}` |\n", artifact.id));
    text.push_str(&format!("| Kind | `{}` |\n", artifact.kind));
    text.push_str(&format!("| Path | `{}` |\n", artifact.path));
    text.push_str(&format!("| Status | `{}` |\n", artifact.status));
    text.push_str(&format!("| Owner | `{}` |\n", artifact.owner));
    text.push_str(&format!("| Created | `{}` |\n\n", artifact.created));

    render_spec_system_link_section(&mut text, "Outgoing Links", report, artifact, true);
    render_spec_system_link_section(&mut text, "Incoming Links", report, artifact, false);

    text.push_str("## Current Findings\n\n");
    if report.findings.is_empty() {
        text.push_str("No findings for this artifact.\n\n");
    } else {
        for finding in &report.findings {
            let posture = finding.blocking_reason.unwrap_or("advisory");
            text.push_str(&format!(
                "- `{}` (`{}`): {}\n",
                finding.kind, posture, finding.message
            ));
        }
        text.push('\n');
    }

    text.push_str("## Repair Work Items\n\n");
    if report.work_items.is_empty() {
        text.push_str("No work items for this artifact.\n\n");
    } else {
        for item in &report.work_items {
            let posture = spec_system_work_item_blocking_reason(item).unwrap_or("advisory");
            text.push_str(&format!(
                "- `{}` (`{}`): {}\n",
                item.kind, posture, item.message
            ));
            if !item.suggested_actions.is_empty() {
                text.push_str("  - Suggested actions:\n");
                for action in &item.suggested_actions {
                    text.push_str(&format!("    - {action}\n"));
                }
            }
            if !item.proof_commands.is_empty() {
                text.push_str("  - Proof commands:\n");
                for command in &item.proof_commands {
                    text.push_str(&format!("    - `{command}`\n"));
                }
            }
        }
        text.push('\n');
    }

    text.push_str("## Proof Commands\n\n");
    for command in spec_system_explain_proof_commands(&artifact.id) {
        text.push_str(&format!("- `{command}`\n"));
    }
    text.push('\n');
    text.push_str("> Claim boundary: structural source-tree graph validation only; cargo-allow did not execute proof commands, run tests, invoke Cargo, rustc, Clippy, build scripts, proc macros, external proof tools, network calls, or GitHub APIs.\n");
    text
}

fn render_spec_system_link_section(
    text: &mut String,
    title: &str,
    report: &SpecSystemReport,
    artifact: &SpecSystemArtifact,
    outgoing: bool,
) {
    let links = report
        .links
        .iter()
        .filter(|link| {
            if outgoing {
                link.source_id == artifact.id
            } else {
                link.target == artifact.id || link.target == artifact.path
            }
        })
        .collect::<Vec<_>>();
    text.push_str(&format!("## {title}\n\n"));
    if links.is_empty() {
        text.push_str("None.\n\n");
        return;
    }
    text.push_str("| Field | Source | Target | Target kind |\n|---|---|---|---|\n");
    for link in links {
        let target_kind = link.target_kind.unwrap_or("");
        text.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` |\n",
            link.field, link.source_id, link.target, target_kind
        ));
    }
    text.push('\n');
}

pub(super) fn render_spec_system_markdown(report: &SpecSystemReport) -> String {
    let mut text = String::new();
    text.push_str(&format!(
        "# cargo-allow {} --profile spec-system\n\n",
        report.command
    ));
    push_spec_system_report_preamble(&mut text, report);
    if let Some(readiness) = &report.readiness {
        text.push_str("## Setup Readiness\n\n");
        text.push_str(&format!("Mode: `{}`\n\n", readiness.mode));
        text.push_str(&format!("Ready: `{}`\n\n", readiness.ready));
        text.push_str("| Check | Status | Path | Message |\n|---|---|---|---|\n");
        for check in &readiness.checks {
            let path = check.path.as_deref().unwrap_or("");
            text.push_str(&format!(
                "| `{}` | `{}` | `{}` | {} |\n",
                check.kind, check.status, path, check.message
            ));
        }
        text.push('\n');
    }
    text.push_str("| Metric | Count |\n|---|---:|\n");
    text.push_str(&format!("| Artifacts | {} |\n", report.artifacts.len()));
    text.push_str(&format!("| Links | {} |\n", report.links.len()));
    text.push_str(&format!(
        "| Support-tier rows | {} |\n",
        report.support_tier_rows
    ));
    text.push_str(&format!("| Findings | {} |\n", report.findings.len()));
    text.push_str(&format!(
        "| Blocking-eligible findings | {} |\n",
        spec_system_blocking_finding_count(report)
    ));
    text.push_str(&format!(
        "| Advisory findings | {} |\n",
        spec_system_advisory_finding_count(report)
    ));
    text.push_str(&format!("| Work items | {} |\n", report.work_items.len()));
    text.push_str(&format!(
        "| Blocking-eligible work items | {} |\n",
        spec_system_blocking_work_item_count(report)
    ));
    text.push_str(&format!(
        "| Advisory work items | {} |\n",
        spec_system_advisory_work_item_count(report)
    ));
    text.push('\n');
    if report.findings.is_empty() {
        text.push_str(&format!(
            "No spec-system findings in `{}` mode.\n\n",
            spec_system_mode_name(&report.mode)
        ));
    } else {
        text.push_str("## Findings\n\n");
        if spec_system_blocking_finding_count(report) > 0 {
            text.push_str("### Blocking-Eligible Findings\n\n");
            for finding in report
                .findings
                .iter()
                .filter(|finding| finding.blocking_eligible)
            {
                let posture = finding.blocking_reason.unwrap_or("blocking_eligible");
                text.push_str(&format!(
                    "- `{}` (`{}`): {}\n",
                    finding.kind, posture, finding.message
                ));
            }
            text.push('\n');
        }
        if spec_system_advisory_finding_count(report) > 0 {
            text.push_str("### Advisory Findings\n\n");
            for finding in report
                .findings
                .iter()
                .filter(|finding| !finding.blocking_eligible)
            {
                text.push_str(&format!(
                    "- `{}` (`advisory`): {}\n",
                    finding.kind, finding.message
                ));
            }
            text.push('\n');
        }
    }
    if !report.work_items.is_empty() {
        text.push_str("## Work Items\n\n");
        for item in &report.work_items {
            let posture = spec_system_work_item_blocking_reason(item).unwrap_or("advisory");
            text.push_str(&format!(
                "- `{}` (`{}`): {}\n",
                item.kind, posture, item.message
            ));
            if let Some(artifact_id) = &item.artifact_id {
                text.push_str(&format!("  - Artifact: `{artifact_id}`\n"));
            }
            if let Some(path) = &item.path {
                text.push_str(&format!("  - Path: `{path}`\n"));
            }
            if !item.suggested_actions.is_empty() {
                text.push_str("  - Suggested actions:\n");
                for action in &item.suggested_actions {
                    text.push_str(&format!("    - {action}\n"));
                }
            }
            if !item.proof_commands.is_empty() {
                text.push_str("  - Proof commands:\n");
                for command in &item.proof_commands {
                    text.push_str(&format!("    - `{command}`\n"));
                }
            }
        }
        text.push('\n');
    }
    text.push_str("> Claim boundary: structural source-tree graph validation only; cargo-allow did not execute proof commands, run tests, invoke Cargo, rustc, Clippy, build scripts, proc macros, external proof tools, network calls, or GitHub APIs.\n");
    text
}

fn push_spec_system_report_preamble(text: &mut String, report: &SpecSystemReport) {
    text.push_str(&format!(
        "**Result:** {}\n\n",
        spec_system_mode_name(&report.mode)
    ));
    text.push_str(&format!(
        "Mode: `{}`\n\n",
        spec_system_mode_name(&report.mode)
    ));
    text.push_str(&format!(
        "Status: `{}`\n\n",
        spec_system_report_status(report)
    ));
    text.push_str("Profile: `spec-system`\n\n");
    text.push_str(&format!(
        "Source tree root: `{}`\n\n",
        allow_report::source_tree_path_text(&report.root)
    ));
    text.push_str(&format!("Config: `{}`\n\n", report.config_source));
    text.push_str(&format!(
        "Config provenance: `{}`\n\n",
        report.config_provenance
    ));
}

pub(super) fn render_spec_system_json(report: &SpecSystemReport) -> String {
    let mut text = String::new();
    text.push_str("{\n");
    text.push_str("  \"schema_version\": 1,\n");
    text.push_str(&format!(
        "  \"schema_id\": \"{}\",\n",
        allow_report::SPEC_SYSTEM_SCHEMA_ID
    ));
    text.push_str("  \"tool\": \"cargo-allow\",\n");
    text.push_str(&format!(
        "  \"command\": \"{}\",\n",
        json_escape(&report.command)
    ));
    text.push_str("  \"profile\": \"spec-system\",\n");
    text.push_str(&format!(
        "  \"mode\": \"{}\",\n",
        spec_system_mode_name(&report.mode)
    ));
    text.push_str(&format!(
        "  \"status\": \"{}\",\n",
        spec_system_report_status(report)
    ));
    text.push_str(&format!(
        "  \"failed\": {},\n",
        if spec_system_report_failed(report) {
            "true"
        } else {
            "false"
        }
    ));
    text.push_str("  \"claim_boundary\": ");
    render_string_array(&mut text, allow_report::SPEC_SYSTEM_CLAIM_BOUNDARY, "  ");
    text.push_str(",\n");
    text.push_str("  \"scanner_limitations\": ");
    render_string_array(
        &mut text,
        allow_report::SPEC_SYSTEM_SCANNER_LIMITATIONS,
        "  ",
    );
    text.push_str(",\n");
    text.push_str("  \"inventory\": {\n");
    text.push_str(&format!(
        "    \"scope\": \"{}\",\n",
        allow_report::INVENTORY_SCOPE_SOURCE_TREE
    ));
    text.push_str(&format!(
        "    \"scanner\": \"{}\",\n",
        allow_report::INVENTORY_SCANNER_SOURCE_TREE_GRAPH
    ));
    text.push_str(&format!(
        "    \"source\": \"{}\",\n",
        allow_report::INVENTORY_SOURCE_UNKNOWN
    ));
    text.push_str(&format!(
        "    \"root\": \"{}\"\n",
        json_escape(&allow_report::source_tree_path_text(&report.root))
    ));
    text.push_str("  },\n");
    text.push_str(&format!(
        "  \"source_tree_root\": \"{}\",\n",
        json_escape(&allow_report::source_tree_path_text(&report.root))
    ));
    text.push_str(&format!(
        "  \"config_source\": \"{}\",\n",
        json_escape(&report.config_source)
    ));
    text.push_str(&format!(
        "  \"config_provenance\": \"{}\",\n",
        json_escape(&report.config_provenance)
    ));
    if let Some(federation) = &report.federation {
        text.push_str("  \"federation\": {\n");
        text.push_str(&format!(
            "    \"federation_version\": \"{}\",\n",
            json_escape(&federation.federation_version)
        ));
        text.push_str(&format!(
            "    \"precedence_applied\": \"{}\",\n",
            json_escape(&federation.precedence_applied)
        ));
        text.push_str("    \"ledger_contributors\": [\n");
        for (index, contributor) in federation.ledger_contributors.iter().enumerate() {
            if index > 0 {
                text.push_str(",\n");
            }
            text.push_str("      {\n");
            text.push_str(&format!(
                "        \"id\": \"{}\",\n",
                json_escape(&contributor.id)
            ));
            text.push_str(&format!(
                "        \"path\": \"{}\",\n",
                json_escape(&contributor.path)
            ));
            text.push_str(&format!(
                "        \"role\": \"{}\",\n",
                json_escape(&contributor.role)
            ));
            text.push_str(&format!(
                "        \"dialect\": \"{}\",\n",
                json_escape(&contributor.dialect)
            ));
            text.push_str(&format!(
                "        \"mode\": \"{}\",\n",
                json_escape(&contributor.mode)
            ));
            text.push_str(&format!(
                "        \"priority\": {},\n",
                contributor.priority
            ));
            text.push_str("        \"lanes\": ");
            render_string_array(&mut text, &contributor.lanes, "        ");
            text.push_str("\n      }");
        }
        text.push_str("\n    ]\n  },\n");
    }
    if let Some(import_graph) = &report.import_graph {
        text.push_str("  \"import_graph\": {\n");
        text.push_str(&format!(
            "    \"node_count\": {},\n",
            import_graph.node_count
        ));
        text.push_str(&format!(
            "    \"edge_count\": {},\n",
            import_graph.edge_count
        ));
        text.push_str(&format!(
            "    \"diagnostic_count\": {},\n",
            import_graph.diagnostic_count
        ));
        text.push_str("    \"nodes\": [\n");
        for (index, node) in import_graph.nodes.iter().enumerate() {
            if index > 0 {
                text.push_str(",\n");
            }
            text.push_str("      {\n");
            text.push_str(&format!("        \"id\": \"{}\",\n", json_escape(&node.id)));
            text.push_str(&format!(
                "        \"path\": \"{}\",\n",
                json_escape(&node.path)
            ));
            text.push_str(&format!(
                "        \"role\": \"{}\",\n",
                json_escape(&node.role)
            ));
            text.push_str(&format!(
                "        \"ecosystem\": \"{}\",\n",
                json_escape(&node.ecosystem)
            ));
            text.push_str(&format!(
                "        \"provenance\": \"{}\",\n",
                json_escape(&node.provenance)
            ));
            text.push_str(&format!(
                "        \"confidence\": \"{}\"\n",
                json_escape(&node.confidence)
            ));
            text.push_str("      }");
        }
        text.push_str("\n    ],\n");
        text.push_str("    \"edges\": [\n");
        for (index, edge) in import_graph.edges.iter().enumerate() {
            if index > 0 {
                text.push_str(",\n");
            }
            text.push_str("      {\n");
            text.push_str(&format!(
                "        \"source_id\": \"{}\",\n",
                json_escape(&edge.source_id)
            ));
            text.push_str(&format!(
                "        \"target_id\": \"{}\",\n",
                json_escape(&edge.target_id)
            ));
            text.push_str(&format!(
                "        \"kind\": \"{}\",\n",
                json_escape(&edge.kind)
            ));
            text.push_str(&format!(
                "        \"provenance\": \"{}\"\n",
                json_escape(&edge.provenance)
            ));
            text.push_str("      }");
        }
        text.push_str("\n    ],\n");
        text.push_str("    \"diagnostics\": [\n");
        for (index, diagnostic) in import_graph.diagnostics.iter().enumerate() {
            if index > 0 {
                text.push_str(",\n");
            }
            text.push_str("      {\n");
            text.push_str(&format!(
                "        \"kind\": \"{}\",\n",
                json_escape(&diagnostic.kind)
            ));
            text.push_str(&format!(
                "        \"message\": \"{}\",\n",
                json_escape(&diagnostic.message)
            ));
            text.push_str("        \"root_ids\": ");
            render_string_array(&mut text, &diagnostic.root_ids, "        ");
            text.push_str("\n      }");
        }
        text.push_str("\n    ]\n  },\n");
    }
    if report.command == "explain"
        && let Some(artifact) = report.artifacts.first()
    {
        text.push_str(&format!(
            "  \"explained_artifact_id\": \"{}\",\n",
            json_escape(&artifact.id)
        ));
        text.push_str("  \"proof_commands\": ");
        render_string_array(
            &mut text,
            &spec_system_explain_proof_commands(&artifact.id),
            "  ",
        );
        text.push_str(",\n");
    }
    if let Some(readiness) = &report.readiness {
        text.push_str("  \"readiness\": {\n");
        text.push_str(&format!(
            "    \"ready\": {},\n",
            if readiness.ready { "true" } else { "false" }
        ));
        text.push_str(&format!(
            "    \"mode\": \"{}\",\n",
            json_escape(readiness.mode)
        ));
        text.push_str("    \"checks\": [\n");
        for (index, check) in readiness.checks.iter().enumerate() {
            text.push_str("      {\n");
            text.push_str(&format!(
                "        \"kind\": \"{}\",\n",
                json_escape(check.kind)
            ));
            if let Some(path) = &check.path {
                text.push_str(&format!("        \"path\": \"{}\",\n", json_escape(path)));
            }
            text.push_str(&format!(
                "        \"found\": {},\n",
                if check.found { "true" } else { "false" }
            ));
            text.push_str(&format!(
                "        \"valid\": {},\n",
                optional_bool_json(check.valid)
            ));
            text.push_str(&format!(
                "        \"status\": \"{}\",\n",
                json_escape(check.status)
            ));
            text.push_str(&format!(
                "        \"message\": \"{}\"\n",
                json_escape(&check.message)
            ));
            text.push_str("      }");
            if index + 1 != readiness.checks.len() {
                text.push(',');
            }
            text.push('\n');
        }
        text.push_str("    ]\n");
        text.push_str("  },\n");
    }
    text.push_str("  \"summary\": {\n");
    text.push_str(&format!("    \"artifacts\": {},\n", report.artifacts.len()));
    text.push_str(&format!("    \"links\": {},\n", report.links.len()));
    text.push_str(&format!(
        "    \"support_tier_rows\": {},\n",
        report.support_tier_rows
    ));
    text.push_str(&format!("    \"findings\": {},\n", report.findings.len()));
    text.push_str(&format!(
        "    \"blocking_eligible_findings\": {},\n",
        spec_system_blocking_finding_count(report)
    ));
    text.push_str(&format!(
        "    \"advisory_findings\": {},\n",
        spec_system_advisory_finding_count(report)
    ));
    text.push_str(&format!(
        "    \"work_items\": {},\n",
        report.work_items.len()
    ));
    text.push_str(&format!(
        "    \"blocking_eligible_work_items\": {},\n",
        spec_system_blocking_work_item_count(report)
    ));
    text.push_str(&format!(
        "    \"advisory_work_items\": {}\n",
        spec_system_advisory_work_item_count(report)
    ));
    text.push_str("  },\n");
    text.push_str("  \"artifacts\": [\n");
    for (index, artifact) in report.artifacts.iter().enumerate() {
        text.push_str("    {\n");
        text.push_str(&format!(
            "      \"id\": \"{}\",\n",
            json_escape(&artifact.id)
        ));
        text.push_str(&format!(
            "      \"kind\": \"{}\",\n",
            json_escape(artifact.kind)
        ));
        text.push_str(&format!(
            "      \"path\": \"{}\",\n",
            json_escape(&artifact.path)
        ));
        text.push_str(&format!(
            "      \"status\": \"{}\",\n",
            json_escape(artifact.status)
        ));
        text.push_str(&format!(
            "      \"owner\": \"{}\",\n",
            json_escape(&artifact.owner)
        ));
        text.push_str(&format!(
            "      \"created\": \"{}\"\n",
            json_escape(&artifact.created)
        ));
        text.push_str("    }");
        if index + 1 != report.artifacts.len() {
            text.push(',');
        }
        text.push('\n');
    }
    text.push_str("  ],\n");
    text.push_str("  \"links\": [\n");
    for (index, link) in report.links.iter().enumerate() {
        text.push_str("    {");
        text.push_str(&format!(
            "\"source_id\": \"{}\", ",
            json_escape(&link.source_id)
        ));
        text.push_str(&format!("\"field\": \"{}\", ", json_escape(link.field)));
        text.push_str(&format!("\"target\": \"{}\"", json_escape(&link.target)));
        if let Some(target_kind) = link.target_kind {
            text.push_str(&format!(
                ", \"target_kind\": \"{}\"",
                json_escape(target_kind)
            ));
        }
        text.push('}');
        if index + 1 != report.links.len() {
            text.push(',');
        }
        text.push('\n');
    }
    text.push_str("  ],\n");
    text.push_str("  \"findings\": [\n");
    for (index, finding) in report.findings.iter().enumerate() {
        text.push_str("    {");
        text.push_str(&format!("\"kind\": \"{}\", ", json_escape(finding.kind)));
        text.push_str(&format!(
            "\"message\": \"{}\", ",
            json_escape(&finding.message)
        ));
        text.push_str(&format!(
            "\"blocking_eligible\": {}",
            if finding.blocking_eligible {
                "true"
            } else {
                "false"
            }
        ));
        if let Some(reason) = finding.blocking_reason {
            text.push_str(&format!(
                ", \"blocking_reason\": \"{}\"",
                json_escape(reason)
            ));
        }
        text.push('}');
        if index + 1 != report.findings.len() {
            text.push(',');
        }
        text.push('\n');
    }
    text.push_str("  ],\n");
    text.push_str("  \"work_items\": [\n");
    for (index, item) in report.work_items.iter().enumerate() {
        text.push_str("    {\n");
        text.push_str(&format!("      \"kind\": \"{}\"", json_escape(item.kind)));
        if let Some(artifact_id) = &item.artifact_id {
            text.push_str(&format!(
                ",\n      \"artifact_id\": \"{}\"",
                json_escape(artifact_id)
            ));
        }
        if let Some(path) = &item.path {
            text.push_str(&format!(",\n      \"path\": \"{}\"", json_escape(path)));
        }
        if let Some(owner) = &item.owner {
            text.push_str(&format!(",\n      \"owner\": \"{}\"", json_escape(owner)));
        }
        if let Some(status) = &item.status {
            text.push_str(&format!(",\n      \"status\": \"{}\"", json_escape(status)));
        }
        let blocking_reason = spec_system_work_item_blocking_reason(item);
        text.push_str(&format!(
            ",\n      \"blocking_eligible\": {}",
            if blocking_reason.is_some() {
                "true"
            } else {
                "false"
            }
        ));
        if let Some(reason) = blocking_reason {
            text.push_str(&format!(
                ",\n      \"blocking_reason\": \"{}\"",
                json_escape(reason)
            ));
        }
        text.push_str(&format!(
            ",\n      \"message\": \"{}\",\n",
            json_escape(&item.message)
        ));
        text.push_str("      \"suggested_actions\": ");
        render_string_array(&mut text, &item.suggested_actions, "      ");
        text.push_str(",\n      \"proof_commands\": ");
        render_string_array(&mut text, &item.proof_commands, "      ");
        if let Some(ledger_id) = &item.ledger_id {
            text.push_str(&format!(
                ",\n      \"ledger_id\": \"{}\"",
                json_escape(ledger_id)
            ));
        }
        if let Some(ledger_path) = &item.ledger_path {
            text.push_str(&format!(
                ",\n      \"ledger_path\": \"{}\"",
                json_escape(ledger_path)
            ));
        }
        if let Some(lane) = &item.lane {
            text.push_str(&format!(",\n      \"lane\": \"{}\"", json_escape(lane)));
        }
        if let Some(mode) = &item.mode {
            text.push_str(&format!(",\n      \"mode\": \"{}\"", json_escape(mode)));
        }
        if let Some(role) = &item.role {
            text.push_str(&format!(",\n      \"role\": \"{}\"", json_escape(role)));
        }
        text.push('\n');
        text.push_str("    }");
        if index + 1 != report.work_items.len() {
            text.push(',');
        }
        text.push('\n');
    }
    text.push_str("  ]\n");
    text.push_str("}\n");
    text
}

fn spec_system_explain_proof_commands(artifact_id: &str) -> Vec<String> {
    let mut commands = spec_system_proof_commands();
    commands.push(format!(
        "cargo-allow explain {artifact_id} --profile spec-system"
    ));
    commands
}

fn render_spec_system_sarif(report: &SpecSystemReport) -> String {
    let mut text = String::new();
    text.push_str("{\n");
    text.push_str("  \"version\": \"2.1.0\",\n");
    text.push_str("  \"runs\": [\n");
    text.push_str("    {\n");
    text.push_str("      \"tool\": {\"driver\": {\"name\": \"cargo-allow spec-system\"}},\n");
    text.push_str("      \"results\": [\n");
    for (index, finding) in report.findings.iter().enumerate() {
        text.push_str("        {");
        text.push_str(&format!("\"ruleId\": \"{}\", ", json_escape(finding.kind)));
        text.push_str(&format!(
            "\"message\": {{\"text\": \"{}\"}}",
            json_escape(&finding.message)
        ));
        text.push('}');
        if index + 1 != report.findings.len() {
            text.push(',');
        }
        text.push('\n');
    }
    text.push_str("      ]\n");
    text.push_str("    }\n");
    text.push_str("  ]\n");
    text.push_str("}\n");
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_report() -> SpecSystemReport {
        SpecSystemReport {
            command: "check".to_string(),
            root: PathBuf::from("."),
            config_source: "built-in".to_string(),
            config_provenance: "default".to_string(),
            mode: SpecSystemMode::Advisory,
            artifacts: vec![SpecSystemArtifact {
                id: "artifact-1".to_string(),
                kind: "spec",
                path: "docs/spec.md".to_string(),
                status: "accepted",
                owner: "team".to_string(),
                created: "2026-01-01".to_string(),
            }],
            links: vec![],
            support_tier_rows: 0,
            findings: vec![SpecSystemFinding::new(
                "example",
                "finding message".to_string(),
            )],
            work_items: vec![],
            readiness: None,
            federation: None,
            import_graph: None,
        }
    }

    #[test]
    fn render_report_dispatches_sarif_findings() {
        let rendered = render_spec_system_report(&test_report(), OutputFormat::Sarif);

        assert!(rendered.contains("\"version\": \"2.1.0\""));
        assert!(rendered.contains("\"ruleId\": \"example\""));
    }

    #[test]
    fn render_json_escapes_dynamic_artifact_and_link_values() -> Result<(), String> {
        let mut report = test_report();
        let artifact = report
            .artifacts
            .first_mut()
            .ok_or_else(|| "test report has no artifact".to_string())?;
        artifact.id = "artifact-\"quoted\"".to_string();
        artifact.path = "docs/quoted\\path.md".to_string();
        let artifact_id = artifact.id.clone();
        report.links.push(SpecSystemLink {
            source_id: artifact_id,
            field: "depends_on",
            target: "target-\"quoted\"".to_string(),
            target_kind: Some("spec"),
        });

        let value: serde_json::Value = serde_json::from_str(&render_spec_system_json(&report))
            .map_err(|error| error.to_string())?;

        assert_eq!(value["artifacts"][0]["id"], "artifact-\"quoted\"");
        assert_eq!(value["artifacts"][0]["path"], "docs/quoted\\path.md");
        assert_eq!(value["links"][0]["target"], "target-\"quoted\"");
        Ok(())
    }
}
