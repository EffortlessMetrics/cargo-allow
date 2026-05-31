use crate::contracts::DOCTOR_ARTIFACT;
use crate::json::{bool_json, option_json, push_json_fixed_artifact_preamble};
use crate::{CLAIM_BOUNDARY_TEXT, DoctorReport, InventoryContext};
use allow_core::json_escape;

pub fn render_doctor_human(facts: DoctorReport<'_>) -> String {
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
                "config status: {}\n",
                config_status_text(facts.config_valid, facts.config_diagnostic)
            ));
        }
        None => out.push_str("config: not found; run `cargo-allow init`\n"),
    }
    out.push_str(&format!(
        "inventory: source_tree/source_syntax via {}; files scanned: {}\n",
        facts.inventory_source, facts.files_scanned
    ));
    out.push_str(CLAIM_BOUNDARY_TEXT);
    out
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
        ),
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
    out.push_str(&format!(
        "    \"found\": {},\n",
        bool_json(facts.config_path.is_some())
    ));
    out.push_str(&format!(
        "    \"path\": {},\n",
        option_json(facts.config_path)
    ));
    out.push_str(&format!(
        "    \"schema_version\": {},\n",
        option_json(facts.config_schema_version)
    ));
    out.push_str(&format!(
        "    \"policy\": {},\n",
        option_json(facts.config_policy)
    ));
    out.push_str(&format!(
        "    \"owner\": {},\n",
        option_json(facts.config_owner)
    ));
    out.push_str(&format!(
        "    \"status\": {},\n",
        option_json(facts.config_status)
    ));
    out.push_str(&format!(
        "    \"valid\": {},\n",
        option_bool_json(facts.config_valid)
    ));
    out.push_str(&format!(
        "    \"diagnostic\": {}\n",
        option_json(facts.config_diagnostic)
    ));
    out.push_str("  }\n");
    out.push_str("}\n");
    out
}

fn config_status_text(valid: Option<bool>, diagnostic: Option<&str>) -> String {
    match (valid, diagnostic) {
        (Some(true), _) => "valid".to_string(),
        (Some(false), Some(diagnostic)) => format!("invalid: {diagnostic}"),
        (Some(false), None) => "invalid".to_string(),
        (None, _) => "not checked".to_string(),
    }
}

fn option_bool_json(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "true",
        Some(false) => "false",
        None => "null",
    }
}
