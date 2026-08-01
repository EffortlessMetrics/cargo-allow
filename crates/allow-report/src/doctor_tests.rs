use super::*;

#[test]
fn doctor_json_renderer_records_root_config_and_inventory() {
    let json = render_doctor_json(DoctorReport {
        source_tree_root: "H:/Code/Rust/cargo-allow",
        root_discovery: "nearest_git_root",
        config_path: Some("H:/Code/Rust/cargo-allow/policy/allow.toml"),
        config_schema_version: Some("0.1"),
        config_policy: Some("cargo-allow"),
        config_owner: Some("core/policy"),
        config_status: Some("active"),
        config_valid: Some(true),
        config_diagnostic: None,
        broken_evidence_links: Some(0),
        weak_evidence_references: Some(0),
        inventory_source: "git_tracked",
        inventory_completeness: "scoped",
        files_scanned: 50,
        empty_git_tracked: false,
        deleted_tracked_files: 0,
        git_inventory_error: None,
        skipped_paths: 0,
        submodule_paths: 0,
        federation_config_path: None,
        federation_config_found: false,
        federation_config_valid: None,
        configured_ledgers: None,
        federation_diagnostics: None,
        federation_divergences: None,
    });

    assert!(json.contains("\"schema_id\": \"cargo-allow.doctor.v1\""));
    assert!(json.contains("\"command\": \"doctor\""));
    assert!(json.contains("\"claim_boundary\""));
    assert!(json.contains("\"scanner_limitations\""));
    assert!(json.contains("\"path\": \"H:/Code/Rust/cargo-allow\""));
    assert!(json.contains("\"discovery\": \"nearest_git_root\""));
    assert!(json.contains("\"found\": true"));
    assert!(json.contains("\"path\": \"H:/Code/Rust/cargo-allow/policy/allow.toml\""));
    assert!(json.contains("\"schema_version\": \"0.1\""));
    assert!(json.contains("\"policy\": \"cargo-allow\""));
    assert!(json.contains("\"owner\": \"core/policy\""));
    assert!(json.contains("\"status\": \"active\""));
    assert!(json.contains("\"valid\": true"));
    assert!(json.contains("\"broken_evidence_links\": 0"));
    assert!(json.contains("\"weak_evidence_references\": 0"));
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
    "files_scanned": 50,
    "completeness": "scoped"
  }},
  "root": {{
    "path": "H:/Code/Rust/cargo-allow",
    "discovery": "nearest_git_root"
  }},
  "config": {{
    "found": true,
    "path": "H:/Code/Rust/cargo-allow/policy/allow.toml",
    "schema_version": "0.1",
    "policy": "cargo-allow",
    "owner": "core/policy",
    "status": "active",
    "valid": true,
    "broken_evidence_links": 0,
    "weak_evidence_references": 0
  }},
  "federation": {{
    "found": false,
    "path": null,
    "valid": null
  }},
  "evidence_repair_queues": [

  ]
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
        config_schema_version: None,
        config_policy: None,
        config_owner: None,
        config_status: None,
        config_valid: None,
        config_diagnostic: None,
        broken_evidence_links: None,
        weak_evidence_references: None,
        inventory_source: "filesystem_fallback",
        inventory_completeness: "fallback",
        files_scanned: 7,
        empty_git_tracked: false,
        deleted_tracked_files: 0,
        git_inventory_error: None,
        skipped_paths: 0,
        submodule_paths: 0,
        federation_config_path: None,
        federation_config_found: false,
        federation_config_valid: None,
        configured_ledgers: None,
        federation_diagnostics: None,
        federation_divergences: None,
    });

    assert!(text.contains("source tree root: H:/Code/Rust/cargo-allow"));
    assert!(text.contains("root discovery: nearest_git_root"));
    assert!(
        text.contains(
            "config: not found; run `cargo-allow init --root \"H:/Code/Rust/cargo-allow\"`"
        )
    );
    assert!(text.contains(
        "inventory: source_tree/source_syntax via filesystem_fallback; files scanned: 7; completeness: fallback"
    ));
    assert!(text.contains("did not invoke Cargo metadata"));
    assert!(text.contains("external evidence tools"));
}

#[test]
fn doctor_json_renderer_suggests_init_when_config_is_missing() {
    let json = render_doctor_json(DoctorReport {
        source_tree_root: "H:/Code/Rust/cargo-allow",
        root_discovery: "nearest_git_root",
        config_path: None,
        config_schema_version: None,
        config_policy: None,
        config_owner: None,
        config_status: None,
        config_valid: None,
        config_diagnostic: None,
        broken_evidence_links: None,
        weak_evidence_references: None,
        inventory_source: "filesystem_fallback",
        inventory_completeness: "fallback",
        files_scanned: 7,
        empty_git_tracked: false,
        deleted_tracked_files: 0,
        git_inventory_error: None,
        skipped_paths: 0,
        submodule_paths: 0,
        federation_config_path: None,
        federation_config_found: false,
        federation_config_valid: None,
        configured_ledgers: None,
        federation_diagnostics: None,
        federation_divergences: None,
    });

    assert!(json.contains("\"found\": false"));
    let value = serde_json::from_str::<serde_json::Value>(&json)
        .unwrap_or_else(|err| std::panic::panic_any(format!("doctor JSON should parse: {err}")));
    assert!(value.pointer("/config/path").is_none());
    assert!(value.pointer("/config/valid").is_none());
    assert!(value.pointer("/config/diagnostic").is_none());
    assert!(json.contains(
        "\"suggested_init_command\": \"cargo-allow init --root \\\"H:/Code/Rust/cargo-allow\\\"\""
    ));
}

#[test]
fn doctor_human_renderer_reports_invalid_config_status() {
    let text = render_doctor_human(DoctorReport {
        source_tree_root: "H:/Code/Rust/cargo-allow",
        root_discovery: "nearest_git_root",
        config_path: Some("policy/allow.toml"),
        config_schema_version: Some("0.1"),
        config_policy: Some("cargo-allow"),
        config_owner: Some("core/policy"),
        config_status: Some("active"),
        config_valid: Some(false),
        config_diagnostic: Some("policy schema_version must not be empty"),
        broken_evidence_links: Some(2),
        weak_evidence_references: Some(1),
        inventory_source: "git_tracked",
        inventory_completeness: "scoped",
        files_scanned: 7,
        empty_git_tracked: false,
        deleted_tracked_files: 0,
        git_inventory_error: None,
        skipped_paths: 0,
        submodule_paths: 0,
        federation_config_path: None,
        federation_config_found: false,
        federation_config_valid: None,
        configured_ledgers: None,
        federation_diagnostics: None,
        federation_divergences: None,
    });

    assert!(text.contains("config: policy/allow.toml"));
    assert!(text.contains("policy schema version: 0.1"));
    assert!(text.contains("policy: cargo-allow"));
    assert!(text.contains("policy owner: core/policy"));
    assert!(text.contains("policy status: active"));
    assert!(text.contains("config status: invalid: policy schema_version must not be empty"));
    assert!(text.contains("broken evidence links: 2"));
    assert!(text.contains(
        "broken evidence worklist: cargo-allow worklist --broken-evidence --format json"
    ));
    assert!(text.contains("weak evidence/link references: 1"));
    assert!(
        text.contains("weak evidence worklist: cargo-allow worklist --weak-evidence --format json")
    );
}

#[test]
fn doctor_human_renderer_styles_fixed_status_labels_only() {
    let text = render_doctor_human_styled(
        DoctorReport {
            source_tree_root: "H:/repo",
            root_discovery: "nearest_git_root",
            config_path: Some("policy/allow.toml"),
            config_schema_version: Some("0.1"),
            config_policy: Some("cargo-allow"),
            config_owner: Some("core/policy"),
            config_status: Some("active"),
            config_valid: Some(false),
            config_diagnostic: Some("policy schema_version must not be empty"),
            broken_evidence_links: Some(0),
            weak_evidence_references: Some(0),
            inventory_source: "git_tracked",
            inventory_completeness: "scoped",
            files_scanned: 7,
            empty_git_tracked: false,
            deleted_tracked_files: 0,
            git_inventory_error: None,
            skipped_paths: 0,
            submodule_paths: 0,
            federation_config_path: Some(".allow/federation.toml"),
            federation_config_found: true,
            federation_config_valid: Some(false),
            configured_ledgers: None,
            federation_diagnostics: None,
            federation_divergences: None,
        },
        Style::ANSI,
    );

    assert!(text.contains("config status: \u{1b}[31minvalid\u{1b}[0m: policy schema_version"));
    assert!(text.contains("federation config status: \u{1b}[31minvalid\u{1b}[0m"));
    assert!(text.contains("federation config provenance: unknown"));
    assert!(
        !text.contains("policy schema_version must not be empty\u{1b}"),
        "repository-controlled diagnostics must stay unstyled"
    );
}

#[test]
fn doctor_json_renderer_includes_optional_evidence_health_counts() {
    let json = render_doctor_json(DoctorReport {
        source_tree_root: "H:/Code/Rust/cargo-allow",
        root_discovery: "nearest_git_root",
        config_path: Some("policy/allow.toml"),
        config_schema_version: Some("0.1"),
        config_policy: Some("cargo-allow"),
        config_owner: Some("core/policy"),
        config_status: Some("active"),
        config_valid: Some(false),
        config_diagnostic: Some("missing evidence file"),
        broken_evidence_links: Some(2),
        weak_evidence_references: Some(1),
        inventory_source: "git_tracked",
        inventory_completeness: "scoped",
        files_scanned: 7,
        empty_git_tracked: false,
        deleted_tracked_files: 0,
        git_inventory_error: None,
        skipped_paths: 0,
        submodule_paths: 0,
        federation_config_path: None,
        federation_config_found: false,
        federation_config_valid: None,
        configured_ledgers: None,
        federation_diagnostics: None,
        federation_divergences: None,
    });

    assert!(json.contains("\"broken_evidence_links\": 2"));
    assert!(json.contains("\"weak_evidence_references\": 1"));
}

#[test]
fn doctor_json_renderer_routes_evidence_repair_queues() {
    let json = render_doctor_json(DoctorReport {
        source_tree_root: "H:/Code/Rust/cargo-allow",
        root_discovery: "nearest_git_root",
        config_path: Some("policy/allow.toml"),
        config_schema_version: Some("0.1"),
        config_policy: Some("cargo-allow"),
        config_owner: Some("core/policy"),
        config_status: Some("active"),
        config_valid: Some(false),
        config_diagnostic: Some("missing evidence file"),
        broken_evidence_links: Some(2),
        weak_evidence_references: Some(1),
        inventory_source: "git_tracked",
        inventory_completeness: "scoped",
        files_scanned: 7,
        empty_git_tracked: false,
        deleted_tracked_files: 0,
        git_inventory_error: None,
        skipped_paths: 0,
        submodule_paths: 0,
        federation_config_path: None,
        federation_config_found: false,
        federation_config_valid: None,
        configured_ledgers: None,
        federation_diagnostics: None,
        federation_divergences: None,
    });

    assert!(json.contains("\"evidence_repair_queues\""));
    assert!(json.contains("\"signal\": \"broken_evidence_links\""));
    assert!(json.contains("\"label\": \"broken evidence links\""));
    assert!(json.contains("\"route_kind\": \"worklist_filter\""));
    assert!(json.contains("\"item_kind\": \"broken_evidence_link\""));
    assert!(json.contains("\"worklist_filter\": \"broken_evidence\""));
    assert!(json.contains("\"count\": 2"));
    assert!(json.contains("\"command\": \"cargo-allow worklist --broken-evidence --format json\""));
    assert!(json.contains("\"signal\": \"weak_evidence_references\""));
    assert!(json.contains("\"label\": \"weak evidence references\""));
    assert!(json.contains("\"item_kind\": \"weak_evidence_reference\""));
    assert!(json.contains("\"worklist_filter\": \"weak_evidence\""));
    assert!(json.contains("\"count\": 1"));
    assert!(json.contains("\"command\": \"cargo-allow worklist --weak-evidence --format json\""));
}

#[test]
fn doctor_json_renderer_always_includes_evidence_repair_queues_even_when_clean() {
    // #1858: doctor should always emit evidence_repair_queues (even when empty)
    // for consistent empty-handling across artifacts, matching receipt and report.
    let json = render_doctor_json(DoctorReport {
        source_tree_root: "H:/Code/Rust/cargo-allow",
        root_discovery: "nearest_git_root",
        config_path: Some("policy/allow.toml"),
        config_schema_version: Some("0.1"),
        config_policy: Some("cargo-allow"),
        config_owner: Some("core/policy"),
        config_status: Some("active"),
        config_valid: Some(true),
        config_diagnostic: None,
        broken_evidence_links: Some(0),
        weak_evidence_references: Some(0),
        inventory_source: "git_tracked",
        inventory_completeness: "scoped",
        files_scanned: 7,
        empty_git_tracked: false,
        deleted_tracked_files: 0,
        git_inventory_error: None,
        skipped_paths: 0,
        submodule_paths: 0,
        federation_config_path: None,
        federation_config_found: false,
        federation_config_valid: None,
        configured_ledgers: None,
        federation_diagnostics: None,
        federation_divergences: None,
    });

    assert!(
        json.contains("\"evidence_repair_queues\":"),
        "doctor should always emit evidence_repair_queues even when clean: {json}"
    );
}

#[test]
fn doctor_json_renderer_records_configured_federation_ledgers() {
    let lanes = vec!["source-exception".to_string()];
    let json = render_doctor_json(DoctorReport {
        source_tree_root: "H:/Code/Rust/cargo-allow",
        root_discovery: "nearest_git_root",
        config_path: Some("policy/allow.toml"),
        config_schema_version: Some("0.1"),
        config_policy: Some("cargo-allow"),
        config_owner: None,
        config_status: None,
        config_valid: Some(true),
        config_diagnostic: None,
        broken_evidence_links: Some(0),
        weak_evidence_references: Some(0),
        inventory_source: "git_tracked",
        inventory_completeness: "scoped",
        files_scanned: 7,
        empty_git_tracked: false,
        deleted_tracked_files: 0,
        git_inventory_error: None,
        skipped_paths: 0,
        submodule_paths: 0,
        federation_config_path: Some(".allow/config.toml"),
        federation_config_found: true,
        federation_config_valid: Some(true),
        configured_ledgers: Some(&[ConfiguredLedgerSummary {
            id: "source-policy",
            path: "policy/allow.toml",
            dialect: "cargo-allow",
            role: "canonical",
            mode: "blocking",
            priority: 10,
            lanes: lanes.as_slice(),
            mirrors: None,
        }]),
        federation_diagnostics: None,
        federation_divergences: None,
    });

    assert!(json.contains("\"federation\""));
    assert!(json.contains("\"configured_ledgers\""));
    assert!(json.contains("\"id\": \"source-policy\""));
    assert!(json.contains("\"dialect\": \"cargo-allow\""));
    assert!(json.contains("\"provenance\": \"fixed_allow_config\""));
}
