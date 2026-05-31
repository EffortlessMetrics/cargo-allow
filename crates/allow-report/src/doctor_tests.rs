use super::*;

#[test]
fn doctor_json_renderer_records_root_config_and_inventory() {
    let json = render_doctor_json(DoctorReport {
        source_tree_root: "H:/Code/Rust/cargo-allow",
        root_discovery: "nearest_git_root",
        config_path: Some("H:/Code/Rust/cargo-allow/policy/allow.toml"),
        config_valid: Some(true),
        config_diagnostic: None,
        inventory_source: "git_tracked",
        files_scanned: 50,
    });

    assert!(json.contains("\"schema_id\": \"cargo-allow.doctor.v1\""));
    assert!(json.contains("\"command\": \"doctor\""));
    assert!(json.contains("\"claim_boundary\""));
    assert!(json.contains("\"scanner_limitations\""));
    assert!(json.contains("\"path\": \"H:/Code/Rust/cargo-allow\""));
    assert!(json.contains("\"discovery\": \"nearest_git_root\""));
    assert!(json.contains("\"found\": true"));
    assert!(json.contains("\"path\": \"H:/Code/Rust/cargo-allow/policy/allow.toml\""));
    assert!(json.contains("\"valid\": true"));
    assert!(json.contains("\"diagnostic\": null"));
    assert!(json.contains("\"scanner\": \"source_syntax\""));
    assert!(json.contains("\"source\": \"git_tracked\""));
    assert!(json.contains("\"root\": \"H:/Code/Rust/cargo-allow\""));
    assert!(json.contains("\"files_scanned\": 50"));
    let expected = format!(
        r#"{{
  "schema_version": 1,
  "schema_id": "cargo-allow.doctor.v1",
  "tool": "cargo-allow",
  "command": "doctor",
  "claim_boundary": {},
  "scanner_limitations": {},
  "inventory": {{
    "scope": "source_tree",
    "scanner": "source_syntax",
    "source": "git_tracked",
    "root": "H:/Code/Rust/cargo-allow",
    "files_scanned": 50
  }},
  "root": {{
    "path": "H:/Code/Rust/cargo-allow",
    "discovery": "nearest_git_root"
  }},
  "config": {{
    "found": true,
    "path": "H:/Code/Rust/cargo-allow/policy/allow.toml",
    "valid": true,
    "diagnostic": null
  }}
}}
"#,
        render_claim_boundary_json(),
        render_scanner_limitations_json()
    );
    assert_eq!(json, expected);
}

#[test]
fn doctor_human_renderer_records_root_config_and_inventory() {
    let text = render_doctor_human(DoctorReport {
        source_tree_root: "H:/Code/Rust/cargo-allow",
        root_discovery: "nearest_git_root",
        config_path: None,
        config_valid: None,
        config_diagnostic: None,
        inventory_source: "filesystem_fallback",
        files_scanned: 7,
    });

    assert!(text.contains("source tree root: H:/Code/Rust/cargo-allow"));
    assert!(text.contains("root discovery: nearest_git_root"));
    assert!(text.contains("config: not found; run `cargo-allow init`"));
    assert!(text.contains(
        "inventory: source_tree/source_syntax via filesystem_fallback; files scanned: 7"
    ));
    assert!(text.contains("did not invoke Cargo metadata"));
    assert!(text.contains("external evidence tools"));
}

#[test]
fn doctor_human_renderer_reports_invalid_config_status() {
    let text = render_doctor_human(DoctorReport {
        source_tree_root: "H:/Code/Rust/cargo-allow",
        root_discovery: "nearest_git_root",
        config_path: Some("policy/allow.toml"),
        config_valid: Some(false),
        config_diagnostic: Some("policy schema_version must not be empty"),
        inventory_source: "git_tracked",
        files_scanned: 7,
    });

    assert!(text.contains("config: policy/allow.toml"));
    assert!(text.contains("config status: invalid: policy schema_version must not be empty"));
}
