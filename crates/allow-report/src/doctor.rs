use crate::json::{
    bool_json, option_json, push_json_artifact_header, render_claim_boundary_json,
    render_scanner_limitations_json,
};
use crate::{CLAIM_BOUNDARY_TEXT, DOCTOR_SCHEMA_ID, DOCTOR_SCHEMA_VERSION, DoctorReport};
use allow_core::json_escape;

pub fn render_doctor_human(facts: DoctorReport<'_>) -> String {
    let mut out = String::new();
    out.push_str(&format!("source tree root: {}\n", facts.source_tree_root));
    out.push_str(&format!("root discovery: {}\n", facts.root_discovery));
    match facts.config_path {
        Some(path) => out.push_str(&format!("config: {path}\n")),
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
    push_json_artifact_header(&mut out, DOCTOR_SCHEMA_VERSION, DOCTOR_SCHEMA_ID, "doctor");
    out.push_str(&format!(
        "  \"claim_boundary\": {},\n",
        render_claim_boundary_json()
    ));
    out.push_str(&format!(
        "  \"scanner_limitations\": {},\n",
        render_scanner_limitations_json()
    ));
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
        "    \"path\": {}\n",
        option_json(facts.config_path)
    ));
    out.push_str("  },\n");
    out.push_str("  \"inventory\": {\n");
    out.push_str("    \"scope\": \"source_tree\",\n");
    out.push_str("    \"scanner\": \"source_syntax\",\n");
    out.push_str(&format!(
        "    \"source\": \"{}\",\n",
        json_escape(facts.inventory_source)
    ));
    out.push_str(&format!("    \"files_scanned\": {}\n", facts.files_scanned));
    out.push_str("  }\n");
    out.push_str("}\n");
    out
}
