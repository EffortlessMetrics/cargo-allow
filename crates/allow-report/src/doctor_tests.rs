use super::*;

#[test]
fn doctor_json_renderer_records_root_config_and_inventory() {
    let json = render_doctor_json(DoctorReport {
        source_tree_root: "H:/Code/Rust/cargo-allow",
        root_discovery: "nearest_git_root",
        config_path: Some("H:/Code/Rust/cargo-allow/policy/allow.toml"),
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
    assert!(json.contains("\"scanner\": \"source_syntax\""));
    assert!(json.contains("\"source\": \"git_tracked\""));
    assert!(json.contains("\"files_scanned\": 50"));
}

#[test]
fn doctor_human_renderer_records_root_config_and_inventory() {
    let text = render_doctor_human(DoctorReport {
        source_tree_root: "H:/Code/Rust/cargo-allow",
        root_discovery: "nearest_git_root",
        config_path: None,
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
}
