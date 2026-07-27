use super::*;

#[test]
fn saved_doctor_output_reports_broken_evidence_config_diagnostic() {
    let fixture = SourceTreeFixture::new("saved-doctor-broken-evidence");
    fixture.write_policy_with_broken_evidence();

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let doctor = artifact_dir.join("doctor.json");

    run_cargo_allow(&[
        "doctor",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--format",
        "json",
        "--output",
        path_arg(&doctor),
    ]);
    let value = assert_source_syntax_artifact(&doctor, allow_report::DOCTOR_SCHEMA_ID, "doctor");
    assert_eq!(
        value
            .pointer("/config/valid")
            .and_then(serde_json::Value::as_bool),
        Some(false),
        "doctor should mark config invalid when local evidence is broken"
    );
    let diagnostic = value
        .pointer("/config/diagnostic")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| std::panic::panic_any("doctor diagnostic should be a string"));
    assert!(
        diagnostic.contains("allow-broken-evidence evidence"),
        "doctor diagnostic should identify the allow entry: {diagnostic}"
    );
    assert!(
        diagnostic.contains("docs/missing-evidence.md"),
        "doctor diagnostic should include the missing evidence path: {diagnostic}"
    );
}

#[test]
fn saved_doctor_output_routes_redundant_segment_evidence_scope_repair_queue() {
    let fixture = SourceTreeFixture::new("saved-doctor-redundant-evidence-scope");
    fixture.write_policy_with_redundant_segment_evidence_scope();

    let value = run_doctor_json(&fixture, "doctor-redundant-evidence-scope.json");
    assert_eq!(
        value
            .pointer("/config/valid")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        value
            .pointer("/config/broken_evidence_links")
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
    assert_eq!(
        value
            .pointer("/config/weak_evidence_references")
            .and_then(serde_json::Value::as_u64),
        Some(0)
    );
    let diagnostic = value
        .pointer("/config/diagnostic")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| std::panic::panic_any("doctor diagnostic should be a string"));
    assert!(
        diagnostic
            .contains("allow-redundant-segment-evidence-scope evidence `doc:docs/./safety.md`")
    );
    assert!(diagnostic.contains("evidence path must not contain current directory segments"));
    let queues = value
        .pointer("/evidence_repair_queues")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| {
            std::panic::panic_any("doctor should route redundant evidence repair queue")
        });
    assert_doctor_evidence_repair_queue(
        queues,
        "broken_evidence_links",
        "broken_evidence_link",
        "broken_evidence",
        "cargo-allow worklist --broken-evidence --format json",
    );
}

#[test]
fn saved_doctor_output_routes_redundant_segment_link_scope_repair_queue() {
    let fixture = SourceTreeFixture::new("saved-doctor-redundant-link-scope");
    fixture.write_policy_with_redundant_segment_link_scope();

    let value = run_doctor_json(&fixture, "doctor-redundant-link-scope.json");
    assert_eq!(
        value
            .pointer("/config/valid")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        value
            .pointer("/config/broken_evidence_links")
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
    assert_eq!(
        value
            .pointer("/config/weak_evidence_references")
            .and_then(serde_json::Value::as_u64),
        Some(0)
    );
    let diagnostic = value
        .pointer("/config/diagnostic")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| std::panic::panic_any("doctor diagnostic should be a string"));
    assert!(diagnostic.contains("allow-redundant-segment-link-scope link `doc:docs/./safety.md`"));
    assert!(diagnostic.contains("link path must not contain current directory segments"));
    let queues = value
        .pointer("/evidence_repair_queues")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| {
            std::panic::panic_any("doctor should route redundant link repair queue")
        });
    assert_doctor_evidence_repair_queue(
        queues,
        "broken_evidence_links",
        "broken_evidence_link",
        "broken_evidence",
        "cargo-allow worklist --broken-evidence --format json",
    );
}

fn run_doctor_json(fixture: &SourceTreeFixture, file_name: &str) -> serde_json::Value {
    let artifact_dir = fixture.root.join("target/cargo-allow");
    let doctor = artifact_dir.join(file_name);

    run_cargo_allow(&[
        "doctor",
        "--root",
        fixture.root_str(),
        "--config",
        "policy/allow.toml",
        "--format",
        "json",
        "--output",
        path_arg(&doctor),
    ]);
    assert_source_syntax_artifact(&doctor, allow_report::DOCTOR_SCHEMA_ID, "doctor")
}

fn assert_doctor_evidence_repair_queue(
    queues: &[serde_json::Value],
    signal: &str,
    item_kind: &str,
    worklist_filter: &str,
    command: &str,
) {
    let queue = queues
        .iter()
        .find(|queue| queue.get("signal").and_then(serde_json::Value::as_str) == Some(signal))
        .unwrap_or_else(|| std::panic::panic_any(format!("missing doctor queue for {signal}")));
    assert_eq!(
        queue.get("route_kind").and_then(serde_json::Value::as_str),
        Some("worklist_filter")
    );
    assert_eq!(
        queue.get("item_kind").and_then(serde_json::Value::as_str),
        Some(item_kind)
    );
    assert_eq!(
        queue
            .get("worklist_filter")
            .and_then(serde_json::Value::as_str),
        Some(worklist_filter)
    );
    assert_eq!(
        queue.get("count").and_then(serde_json::Value::as_u64),
        Some(1)
    );
    assert_eq!(
        queue.get("command").and_then(serde_json::Value::as_str),
        Some(command)
    );
}
#[test]
fn saved_doctor_output_suggests_init_when_config_is_missing() {
    let fixture = SourceTreeFixture::new("saved-doctor-missing-config");

    let artifact_dir = fixture.root.join("target/cargo-allow");
    let doctor = artifact_dir.join("doctor.json");

    run_cargo_allow(&[
        "doctor",
        "--root",
        fixture.root_str(),
        "--format",
        "json",
        "--output",
        path_arg(&doctor),
    ]);
    let value = assert_source_syntax_artifact(&doctor, allow_report::DOCTOR_SCHEMA_ID, "doctor");
    assert_eq!(
        value
            .pointer("/config/found")
            .and_then(serde_json::Value::as_bool),
        Some(false),
        "doctor should report missing config in source-tree setup diagnostics"
    );
    // #1825: On Windows the source-tree root is canonicalized by
    // resolve_source_tree_root (yielding long names with a verbatim \\?\
    // prefix), while the test's temp dir may use 8.3 short names. The doctor
    // output uses the canonicalized root with the verbatim prefix stripped.
    // Canonicalize and strip the prefix before building the expected command.
    let canonical_root = fixture
        .root
        .canonicalize()
        .unwrap_or_else(|err| std::panic::panic_any(format!("canonicalize fixture root: {err}")));
    let canonical_root_str = canonical_root
        .to_str()
        .unwrap_or_else(|| std::panic::panic_any("non-UTF-8 canonical root"))
        .replace('\\', "/");
    // Strip the verbatim prefix if present (//?/ on forward-slashed paths).
    let canonical_root_str = canonical_root_str
        .strip_prefix("//?/")
        .map(|s| s.to_string())
        .unwrap_or(canonical_root_str);
    let expected = format!("cargo-allow init --root \"{canonical_root_str}\"");
    assert_eq!(
        value
            .pointer("/config/suggested_init_command")
            .and_then(serde_json::Value::as_str),
        Some(expected.as_str()),
        "doctor should emit a root-aware standalone init command"
    );
}
