use allow_core::json_escape;

use crate::{CLAIM_BOUNDARY, InventoryContext, SCANNER_LIMITATIONS};

pub(crate) fn push_json_artifact_header(
    out: &mut String,
    schema_version: u32,
    schema_id: &str,
    command: &str,
) {
    out.push_str(&format!("  \"schema_version\": {schema_version},\n"));
    out.push_str(&format!(
        "  \"schema_id\": \"{}\",\n",
        json_escape(schema_id)
    ));
    out.push_str("  \"tool\": \"cargo-allow\",\n");
    out.push_str(&format!("  \"command\": \"{}\",\n", json_escape(command)));
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
    out.push('\n');
    out.push_str(&format!("{indent}}}"));
    out
}
