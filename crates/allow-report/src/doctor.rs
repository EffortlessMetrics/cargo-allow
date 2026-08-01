use crate::contracts::DOCTOR_ARTIFACT;
use crate::evidence_repair::{
    BROKEN_EVIDENCE_LINK_COMMAND, EvidenceRepairQueue, WEAK_EVIDENCE_REFERENCE_COMMAND,
    evidence_repair_queues_from_counts, push_evidence_repair_queue_json_fields,
};
use crate::json::{bool_json, option_json, push_json_fixed_artifact_preamble};
use crate::{CLAIM_BOUNDARY_TEXT, DoctorReport, InventoryContext, Style};
use allow_core::json_escape;

pub fn render_doctor_human(facts: DoctorReport<'_>) -> String {
    render_doctor_human_styled(facts, Style::PLAIN)
}

pub fn render_doctor_human_styled(facts: DoctorReport<'_>, style: Style) -> String {
    let mut out = String::new();
    out.push_str(&format!("source tree root: {}\n", facts.source_tree_root));
    out.push_str(&format!("root discovery: {}\n", facts.root_discovery));
    match facts.config_path {
        Some(path) => {
            out.push_str(&format!("config: {path}\n"));
            if let Some(schema_version) = facts.config_schema_version {
                out.push_str(&format!("policy schema version: {schema_version}\n"));
            }
            if let Some(policy) = facts.config_policy {
                out.push_str(&format!("policy: {policy}\n"));
            }
            if let Some(owner) = facts.config_owner {
                out.push_str(&format!("policy owner: {owner}\n"));
            }
            if let Some(status) = facts.config_status {
                out.push_str(&format!("policy status: {status}\n"));
            }
            out.push_str(&format!(
                "config status: {}{}\n",
                style.status(
                    config_status_label(facts.config_valid),
                    config_status_label(facts.config_valid)
                ),
                config_status_diagnostic_suffix(facts.config_valid, facts.config_diagnostic)
            ));
            if facts.config_valid == Some(false) {
                out.push_str(
                    "config repair: inspect the diagnostic above; run \
                     `cargo-allow check --mode no-new` for full validation details, \
                     or `cargo-allow migrate --from <file> --update` to convert a \
                     legacy policy\n",
                );
            }
            if let Some(count) = facts.broken_evidence_links {
                out.push_str(&format!("broken evidence links: {count}\n"));
                if count > 0 {
                    out.push_str(&format!(
                        "broken evidence worklist: {BROKEN_EVIDENCE_LINK_COMMAND}\n"
                    ));
                }
            }
            if let Some(count) = facts.weak_evidence_references {
                out.push_str(&format!("weak evidence/link references: {count}\n"));
                if count > 0 {
                    out.push_str(&format!(
                        "weak evidence worklist: {WEAK_EVIDENCE_REFERENCE_COMMAND}\n"
                    ));
                }
            }
        }
        None => {
            out.push_str(&format!(
                "config: not found; run `{}`\n",
                suggested_init_command(facts.source_tree_root)
            ));
            out.push_str(
                "  tip: `cargo-allow audit` works without a policy to surface source findings before bootstrapping\n",
            );
        }
    }
    out.push_str(&format!(
        "inventory: source_tree/source_syntax via {}{}\n",
        facts.inventory_source,
        doctor_inventory_suffix(facts)
    ));
    if facts.empty_git_tracked {
        out.push_str(
            "inventory warning: git reported no tracked files; newly initialized repos need `git add` or `--include-untracked` before cargo-allow scans source files\n",
        );
    }
    if facts.deleted_tracked_files > 0 {
        out.push_str(&format!(
            "inventory warning: {} tracked file(s) absent from the worktree; \
             these paths are excluded from the scan (check out the worktree or \
             restore the files to restore coverage)\n",
            facts.deleted_tracked_files
        ));
    }
    if let Some(git_error) = facts.git_inventory_error {
        out.push_str(&format!(
            "inventory warning: git ls-files failed; fell back to filesystem \
             scan — git error: {git_error}\n"
        ));
    }
    if facts.skipped_paths > 0 {
        out.push_str(&format!(
            "inventory warning: {} path(s) skipped due to I/O errors (permission \
             denied, etc.); these paths are excluded from the scan\n",
            facts.skipped_paths
        ));
    }
    if facts.submodule_paths > 0 {
        out.push_str(&format!(
            "inventory note: {} submodule(s) detected; submodule contents are \
             not scanned (run cargo-allow inside each submodule for coverage)\n",
            facts.submodule_paths
        ));
    }
    append_federation_doctor_human(facts, &mut out, style);
    out.push_str(CLAIM_BOUNDARY_TEXT);
    out
}

fn append_federation_doctor_human(facts: DoctorReport<'_>, out: &mut String, style: Style) {
    if !facts.federation_config_found {
        out.push_str("federation config: not found\n");
        return;
    }
    if let Some(path) = facts.federation_config_path {
        out.push_str(&format!("federation config: {path}\n"));
    }
    out.push_str(&format!(
        "federation config status: {}\n",
        style.status(
            federation_status_label(facts.federation_config_valid),
            federation_status_label(facts.federation_config_valid),
        )
    ));
    if let Some(ledgers) = facts.configured_ledgers {
        out.push_str(&format!("configured ledgers: {}\n", ledgers.len()));
        for ledger in ledgers {
            out.push_str(&format!(
                "  - {} ({}) role={} dialect={} mode={} priority={}",
                ledger.id, ledger.path, ledger.role, ledger.dialect, ledger.mode, ledger.priority
            ));
            if !ledger.lanes.is_empty() {
                out.push_str(&format!(" lanes={}", ledger.lanes.join(",")));
            }
            if let Some(mirrors) = ledger.mirrors {
                out.push_str(&format!(" mirrors={mirrors}"));
            }
            out.push('\n');
        }
    }
    if let Some(diagnostics) = facts.federation_diagnostics {
        for diagnostic in diagnostics {
            out.push_str(&format!(
                "federation {}: {}\n",
                diagnostic.kind, diagnostic.message
            ));
        }
    }
    if let Some(divergences) = facts.federation_divergences {
        for divergence in divergences {
            out.push_str(&format!(
                "federation {}: {}\n",
                divergence.kind, divergence.message
            ));
        }
    }
}

pub fn render_doctor_json(facts: DoctorReport<'_>) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    push_json_fixed_artifact_preamble(
        &mut out,
        DOCTOR_ARTIFACT,
        InventoryContext::source_syntax(
            facts.inventory_source,
            Some(facts.source_tree_root),
            Some(facts.files_scanned),
        )
        .with_empty_git_tracked(facts.empty_git_tracked)
        .with_completeness(facts.inventory_completeness),
    );
    out.push_str("  \"root\": {\n");
    out.push_str(&format!(
        "    \"path\": \"{}\",\n",
        json_escape(facts.source_tree_root)
    ));
    out.push_str(&format!(
        "    \"discovery\": \"{}\"\n",
        json_escape(facts.root_discovery)
    ));
    out.push_str("  },\n");
    out.push_str("  \"config\": {\n");
    let mut config_fields = vec![format!(
        "    \"found\": {}",
        bool_json(facts.config_path.is_some())
    )];
    push_optional_string_field(&mut config_fields, "path", facts.config_path);
    push_optional_string_field(
        &mut config_fields,
        "schema_version",
        facts.config_schema_version,
    );
    push_optional_string_field(&mut config_fields, "policy", facts.config_policy);
    push_optional_string_field(&mut config_fields, "owner", facts.config_owner);
    push_optional_string_field(&mut config_fields, "status", facts.config_status);
    if let Some(valid) = facts.config_valid {
        config_fields.push(format!("    \"valid\": {}", bool_json(valid)));
    }
    push_optional_string_field(&mut config_fields, "diagnostic", facts.config_diagnostic);
    if facts.config_path.is_none() {
        config_fields.push(format!(
            "    \"suggested_init_command\": \"{}\"",
            json_escape(&suggested_init_command(facts.source_tree_root))
        ));
    }
    if let Some(count) = facts.broken_evidence_links {
        config_fields.push(format!("    \"broken_evidence_links\": {count}"));
    }
    if let Some(count) = facts.weak_evidence_references {
        config_fields.push(format!("    \"weak_evidence_references\": {count}"));
    }
    if facts.deleted_tracked_files > 0 {
        config_fields.push(format!(
            "    \"deleted_tracked_files\": {}",
            facts.deleted_tracked_files
        ));
    }
    if let Some(git_error) = facts.git_inventory_error {
        config_fields.push(format!(
            "    \"git_inventory_error\": \"{}\"",
            json_escape(git_error)
        ));
    }
    if facts.skipped_paths > 0 {
        config_fields.push(format!("    \"skipped_paths\": {}", facts.skipped_paths));
    }
    if facts.submodule_paths > 0 {
        config_fields.push(format!(
            "    \"submodule_paths\": {}",
            facts.submodule_paths
        ));
    }
    for (index, field) in config_fields.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str(field);
    }
    out.push_str("\n  },\n");
    append_federation_doctor_json(facts, &mut out);
    append_doctor_evidence_repair_queues_json(facts, &mut out);
    out.push_str("\n}\n");
    out
}

fn append_federation_doctor_json(facts: DoctorReport<'_>, out: &mut String) {
    out.push_str("  \"federation\": {\n");
    out.push_str(&format!(
        "    \"found\": {},\n",
        bool_json(facts.federation_config_found)
    ));
    out.push_str(&format!(
        "    \"path\": {},\n",
        option_json(facts.federation_config_path)
    ));
    out.push_str(&format!(
        "    \"valid\": {}",
        option_bool_json(facts.federation_config_valid)
    ));
    if let Some(ledgers) = facts.configured_ledgers {
        out.push_str(",\n    \"configured_ledgers\": [\n");
        for (index, ledger) in ledgers.iter().enumerate() {
            if index > 0 {
                out.push_str(",\n");
            }
            out.push_str("      {\n");
            out.push_str(&format!(
                "        \"id\": \"{}\",\n",
                json_escape(ledger.id)
            ));
            out.push_str(&format!(
                "        \"path\": \"{}\",\n",
                json_escape(ledger.path)
            ));
            out.push_str(&format!(
                "        \"dialect\": \"{}\",\n",
                json_escape(ledger.dialect)
            ));
            out.push_str(&format!(
                "        \"role\": \"{}\",\n",
                json_escape(ledger.role)
            ));
            out.push_str(&format!(
                "        \"mode\": \"{}\",\n",
                json_escape(ledger.mode)
            ));
            out.push_str(&format!("        \"priority\": {}", ledger.priority));
            if !ledger.lanes.is_empty() {
                out.push_str(",\n        \"lanes\": [");
                for (lane_index, lane) in ledger.lanes.iter().enumerate() {
                    if lane_index > 0 {
                        out.push_str(", ");
                    }
                    out.push_str(&format!("\"{}\"", json_escape(lane)));
                }
                out.push(']');
            }
            if let Some(mirrors) = ledger.mirrors {
                out.push_str(&format!(
                    ",\n        \"mirrors\": \"{}\"",
                    json_escape(mirrors)
                ));
            }
            out.push_str("\n      }");
        }
        out.push_str("\n    ]");
    }
    if let Some(diagnostics) = facts.federation_diagnostics {
        out.push_str(",\n    \"diagnostics\": [\n");
        for (index, diagnostic) in diagnostics.iter().enumerate() {
            if index > 0 {
                out.push_str(",\n");
            }
            out.push_str("      {\n");
            out.push_str(&format!(
                "        \"kind\": \"{}\",\n",
                json_escape(diagnostic.kind)
            ));
            out.push_str(&format!(
                "        \"message\": \"{}\",\n",
                json_escape(diagnostic.message)
            ));
            out.push_str("        \"ledger_ids\": [");
            for (id_index, id) in diagnostic.ledger_ids.iter().enumerate() {
                if id_index > 0 {
                    out.push_str(", ");
                }
                out.push_str(&format!("\"{}\"", json_escape(id)));
            }
            out.push_str("]\n      }");
        }
        out.push_str("\n    ]");
    }
    if let Some(divergences) = facts.federation_divergences {
        out.push_str(",\n    \"divergences\": [\n");
        for (index, divergence) in divergences.iter().enumerate() {
            if index > 0 {
                out.push_str(",\n");
            }
            out.push_str("      {\n");
            out.push_str(&format!(
                "        \"kind\": \"{}\",\n",
                json_escape(divergence.kind)
            ));
            out.push_str(&format!(
                "        \"message\": \"{}\",\n",
                json_escape(divergence.message)
            ));
            out.push_str("        \"ledger_ids\": [");
            for (id_index, id) in divergence.ledger_ids.iter().enumerate() {
                if id_index > 0 {
                    out.push_str(", ");
                }
                out.push_str(&format!("\"{}\"", json_escape(id)));
            }
            out.push_str("]\n      }");
        }
        out.push_str("\n    ]");
    }
    out.push_str("\n  }");
}

fn federation_status_label(valid: Option<bool>) -> &'static str {
    match valid {
        Some(true) => "valid",
        Some(false) => "invalid",
        None => "not checked",
    }
}

fn append_doctor_evidence_repair_queues_json(facts: DoctorReport<'_>, out: &mut String) {
    let queues = doctor_evidence_repair_queues(facts);
    // #1858: always emit the array (even when empty) so downstream consumers
    // can distinguish "feature off" from "zero count" without per-artifact
    // special-casing. Receipt and report already do this; doctor now matches.
    out.push_str(",\n  \"evidence_repair_queues\": [\n");
    for (index, queue) in queues.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str("    {\n");
        push_evidence_repair_queue_json_fields(out, queue, "      ");
        out.push_str("    }");
    }
    out.push_str("\n  ]");
}

fn doctor_evidence_repair_queues(facts: DoctorReport<'_>) -> Vec<EvidenceRepairQueue> {
    evidence_repair_queues_from_counts(
        facts.broken_evidence_links.unwrap_or(0),
        0,
        facts.weak_evidence_references.unwrap_or(0),
        0,
    )
}

fn doctor_inventory_suffix(facts: DoctorReport<'_>) -> String {
    let mut suffix = format!("; files scanned: {}", facts.files_scanned);
    suffix.push_str(&format!("; completeness: {}", facts.inventory_completeness));
    suffix
}

fn config_status_label(valid: Option<bool>) -> &'static str {
    match valid {
        Some(true) => "valid",
        Some(false) => "invalid",
        None => "not checked",
    }
}

fn config_status_diagnostic_suffix(valid: Option<bool>, diagnostic: Option<&str>) -> String {
    match (valid, diagnostic) {
        (Some(false), Some(diagnostic)) => format!(": {diagnostic}"),
        _ => String::new(),
    }
}

fn option_bool_json(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "true",
        Some(false) => "false",
        None => "null",
    }
}

fn push_optional_string_field(fields: &mut Vec<String>, name: &str, value: Option<&str>) {
    if let Some(value) = value {
        fields.push(format!("    \"{name}\": \"{}\"", json_escape(value)));
    }
}

fn suggested_init_command(source_tree_root: &str) -> String {
    format!("cargo-allow init --root \"{source_tree_root}\"")
}
