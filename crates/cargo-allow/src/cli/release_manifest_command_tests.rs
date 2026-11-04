//! Negative-control and typed-completion tests for the release-manifest
//! command (#2497). Tests live in a dedicated, finding-free `_tests.rs`
//! module and report through `Result` instead of panicking macros, so no
//! policy receipts are required for test-only assertion paths.

use super::*;
use serde_json::json;

const COMMIT: &str = "fixture-commit";
const TREE: &str = "fixture-tree";
const VERSION: &str = "0.2.0";

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn fixture_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "cargo-allow-release-manifest-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn digest(character: char) -> String {
    format!("sha256:{}", character.to_string().repeat(64))
}

fn rows_fixture() -> Vec<Value> {
    vec![
        json!({
            "logical_id": "allow-core",
            "name": "allow-core",
            "version": VERSION,
            "release_order": 10,
            "local_checksum": digest('a'),
            "registry_checksum": digest('a'),
            "state": "published_verified",
        }),
        json!({
            "logical_id": "cargo-allow",
            "name": "cargo-allow",
            "version": VERSION,
            "release_order": 20,
            "local_checksum": digest('b'),
            "registry_checksum": digest('b'),
            "state": "published_verified",
        }),
    ]
}

fn prepare_fixture_dir(dir: &Path) -> TestResult {
    Ok(std::fs::create_dir_all(dir).map_err(|error| format!("fixture directory: {error}"))?)
}

fn write_receipt(path: &Path, document: &Value) -> TestResult {
    let rendered =
        serde_json::to_vec_pretty(document).map_err(|error| format!("render: {error}"))?;
    Ok(std::fs::write(path, rendered)
        .map_err(|error| format!("write {}: {error}", path.display()))?)
}

fn receipt_bytes(path: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    std::fs::read(path).map_err(|error| format!("read {}: {error}", path.display()).into())
}

/// Set or replace one top-level field of a receipt document without
/// panicking indexing.
fn set_field(document: &mut Value, key: &str, value: Value) -> TestResult {
    document
        .as_object_mut()
        .ok_or("receipt fixture is not an object")?
        .insert(key.to_string(), value);
    Ok(())
}

/// Mutate one row of the receipt fixture by row index without panicking
/// indexing.
fn mutate_row(document: &mut Value, index: usize, key: &str, value: Value) -> TestResult {
    let row = document
        .get_mut("rows")
        .and_then(Value::as_array_mut)
        .and_then(|rows| rows.get_mut(index))
        .ok_or("fixture row is missing")?;
    row.as_object_mut()
        .ok_or("fixture row is not an object")?
        .insert(key.to_string(), value);
    Ok(())
}

/// Convert every packaged row to the unpublished candidate posture.
fn candidate_posture(document: &mut Value) -> TestResult {
    set_field(document, "publish", json!(false))?;
    set_field(document, "first_irreversible_row", Value::Null)?;
    let rows = document
        .get_mut("rows")
        .and_then(Value::as_array_mut)
        .ok_or("fixture rows are missing")?;
    for row in rows.iter_mut() {
        let object = row.as_object_mut().ok_or("fixture row is not an object")?;
        object.insert("state".to_string(), json!("packaged"));
        object.insert("registry_checksum".to_string(), Value::Null);
    }
    Ok(())
}

fn str_field_of<'a>(
    document: &'a Value,
    path: &[&str],
) -> Result<&'a str, Box<dyn std::error::Error>> {
    let mut current = document;
    for key in path {
        current = current
            .get(key)
            .ok_or_else(|| format!("manifest field {key:?} is missing"))?;
    }
    current
        .as_str()
        .ok_or_else(|| format!("manifest field {:?} is not a string", path.join(".")).into())
}

fn array_len_of(document: &Value, path: &[&str]) -> Result<usize, Box<dyn std::error::Error>> {
    let mut current = document;
    for key in path {
        current = current
            .get(key)
            .ok_or_else(|| format!("manifest field {key:?} is missing"))?;
    }
    current
        .as_array()
        .map(|entries| entries.len())
        .ok_or_else(|| format!("manifest field {:?} is not an array", path.join(".")).into())
}

/// Run the command expecting refusal; returns the typed error.
fn expect_refusal(
    outcome: CargoAllowResult<()>,
    label: &str,
) -> Result<CargoAllowError, Box<dyn std::error::Error>> {
    match outcome {
        Ok(()) => Err(format!("{label}: unexpectedly succeeded").into()),
        Err(error) => Ok(error),
    }
}

fn published_receipt_document() -> Value {
    json!({
        "schema_id": TOPOLOGY_RECEIPT_SCHEMA,
        "schema_version": 1,
        "mode": "cargo-allow",
        "publish": true,
        "authorization": "issue:3760",
        "topology_id": "fixture-topology",
        "topology_sha256": "b".repeat(64),
        "cargo_lock_sha256": "c".repeat(64),
        "commit": COMMIT,
        "tree": TREE,
        "rows": rows_fixture(),
        "complete": true,
        "incident_state": "none",
        "first_irreversible_row": 20,
    })
}

fn generate_args(dir: &Path, receipt: &Path) -> ReleaseManifestArgs {
    ReleaseManifestArgs {
        version: Some(VERSION.to_string()),
        repository: Some("EffortlessMetrics/cargo-allow".to_string()),
        tag: Some("v0.2.0".to_string()),
        commit: Some(COMMIT.to_string()),
        tree: Some(TREE.to_string()),
        auth_source: Some("crates_io_api_token".to_string()),
        authorization_digest: None,
        workflow_run_id: Some(4427),
        platforms: "linux".to_string(),
        rust_toolchain: "stable".to_string(),
        runner: "ubuntu-latest".to_string(),
        binary_package_receipt: None,
        binary_install_receipt: None,
        topology_receipt: receipt.to_path_buf(),
        require_published_topology: false,
        expected_topology_receipt_sha256: None,
        manifest: None,
        output: dir.join("release-manifest-v2.json"),
        require_result: None,
        result_out: dir.join("release-manifest-v2.result.json"),
        github_output: None,
    }
}

fn binary_receipt_document(schema: &str, attestation_verified: bool) -> Value {
    json!({
        "schema_id": schema,
        "schema_version": 1,
        "version": VERSION,
        "tag": "v0.2.0",
        "commit": COMMIT,
        "tree": TREE,
        "target_triple": "x86_64-unknown-linux-gnu",
        "archive_name": format!("cargo-allow-v{VERSION}-x86_64-unknown-linux-gnu.tar.gz"),
        "archive_format": "tar.gz",
        "executable_name": "cargo-allow",
        "archive_sha256": digest('d'),
        "executable_sha256": digest('e'),
        "attestation_verified": attestation_verified,
    })
}

#[test]
fn published_manifest_generates_and_revalidates_complete() -> TestResult {
    let dir = fixture_dir("published-complete");
    prepare_fixture_dir(&dir)?;
    let receipt = dir.join("topology-publish.receipt.json");
    write_receipt(&receipt, &published_receipt_document())?;
    let mut args = generate_args(&dir, &receipt);
    args.require_result = Some("complete".to_string());
    let github_output = dir.join("github-output.txt");
    args.github_output = Some(github_output.clone());
    cmd_release_manifest(&args)?;

    let manifest_bytes = receipt_bytes(&args.output)?;
    let manifest: Value =
        serde_json::from_slice(&manifest_bytes).map_err(|error| format!("manifest: {error}"))?;
    if str_field_of(&manifest, &["payload", "schema_id"])? != "cargo-allow.release-manifest.v2" {
        return Err("manifest must carry the V2 schema id".into());
    }
    if str_field_of(&manifest, &["payload", "publication_posture"])? != "published" {
        return Err("published receipt must produce the published posture".into());
    }
    if str_field_of(&manifest, &["authorization_reference"])? != "workflow/4427/crates-io" {
        return Err("the envelope must record the workflow authorization reference".into());
    }
    if str_field_of(&manifest, &["github_ref"])? != "v0.2.0" {
        return Err("the envelope must record the observed tag".into());
    }
    let rows = manifest
        .get("payload")
        .and_then(|payload| payload.get("package_rows"))
        .and_then(Value::as_array)
        .ok_or("package rows are missing")?;
    let first = rows
        .first()
        .and_then(|row| row.get("registry_checksum"))
        .and_then(Value::as_str)
        .ok_or("the first row must carry its registry checksum")?;
    if first != digest('a') {
        return Err(format!("the first row digest drifted: {first}").into());
    }
    let result_bytes = receipt_bytes(&args.result_out)?;
    let result: Value =
        serde_json::from_slice(&result_bytes).map_err(|error| format!("receipt: {error}"))?;
    if result.get("schema").and_then(Value::as_str) != Some(RESULT_RECEIPT_SCHEMA) {
        return Err("the result receipt must carry its schema identity".into());
    }
    if result.get("result").and_then(Value::as_str) != Some("complete") {
        return Err(format!(
            "published evidence must reach complete, got {:?}",
            result.get("result")
        )
        .into());
    }
    if result.get("required_result").and_then(Value::as_str) != Some("complete") {
        return Err("the result receipt must record the required result".into());
    }
    let recorded_digest = result
        .get("manifest_sha256")
        .and_then(Value::as_str)
        .ok_or("the result receipt must bind the manifest digest")?;
    if recorded_digest != canonical_sha256(&manifest_bytes) {
        return Err("the recorded manifest digest differs from the written bytes".into());
    }
    let emitted = std::fs::read_to_string(&github_output)
        .map_err(|error| format!("github output: {error}"))?;
    if emitted != "result=complete\n" {
        return Err(format!("github output drifted: {emitted:?}").into());
    }
    let sidecar = std::fs::read_to_string(args.output.with_extension("sha256"))
        .map_err(|error| format!("sidecar: {error}"))?;
    let expected_hex = canonical_sha256(&manifest_bytes)
        .strip_prefix("sha256:")
        .unwrap_or_default()
        .to_string();
    let basename = args
        .output
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    if sidecar != format!("{expected_hex}  {basename}\n") {
        return Err("the checksum sidecar drifted from the manifest bytes".into());
    }

    // Validate mode re-reads the exact bytes and reaches the same result.
    let mut validate_args = generate_args(&dir, &receipt);
    validate_args.manifest = Some(args.output.clone());
    validate_args.require_result = Some("complete".to_string());
    cmd_release_manifest(&validate_args)?;
    Ok(())
}

#[test]
fn row_version_not_the_selected_identity_is_refused_without_manifest() -> TestResult {
    let dir = fixture_dir("version-mismatch");
    prepare_fixture_dir(&dir)?;
    let receipt = dir.join("topology-publish.receipt.json");
    let mut document = published_receipt_document();
    mutate_row(&mut document, 1, "version", json!("0.1.11"))?;
    write_receipt(&receipt, &document)?;
    let args = generate_args(&dir, &receipt);
    let error = expect_refusal(cmd_release_manifest(&args), "version mismatch")?;
    if !error
        .to_string()
        .contains("is not the selected release identity")
    {
        return Err(format!("refusal must name the selected identity: {error}").into());
    }
    if args.output.exists() {
        return Err("a refused run must not write the manifest".into());
    }
    Ok(())
}

#[test]
fn published_incident_refuses() -> TestResult {
    let dir = fixture_dir("incident");
    prepare_fixture_dir(&dir)?;
    let receipt = dir.join("topology-publish.receipt.json");
    let mut document = published_receipt_document();
    set_field(&mut document, "incident_state", json!("partial"))?;
    write_receipt(&receipt, &document)?;
    let args = generate_args(&dir, &receipt);
    let error = expect_refusal(cmd_release_manifest(&args), "incident")?;
    if !error.to_string().contains("records an incident") {
        return Err(format!("refusal must name the incident: {error}").into());
    }
    if args.output.exists() {
        return Err("a refused run must not write the manifest".into());
    }
    Ok(())
}

#[test]
fn published_checksum_disagreement_refuses() -> TestResult {
    let dir = fixture_dir("checksum-disagreement");
    prepare_fixture_dir(&dir)?;
    let receipt = dir.join("topology-publish.receipt.json");
    let mut document = published_receipt_document();
    mutate_row(&mut document, 0, "registry_checksum", json!(digest('0')))?;
    write_receipt(&receipt, &document)?;
    let args = generate_args(&dir, &receipt);
    let error = expect_refusal(cmd_release_manifest(&args), "disagreement")?;
    if !error
        .to_string()
        .contains("registry checksum disagrees with the candidate package")
    {
        return Err(format!("refusal must name the disagreement: {error}").into());
    }
    if args.output.exists() {
        return Err("a refused run must not write the manifest".into());
    }
    Ok(())
}

#[test]
fn require_published_demands_published_topology() -> TestResult {
    let dir = fixture_dir("require-published");
    prepare_fixture_dir(&dir)?;
    let receipt = dir.join("topology-publish.receipt.json");
    let mut document = published_receipt_document();
    candidate_posture(&mut document)?;
    write_receipt(&receipt, &document)?;
    let mut args = generate_args(&dir, &receipt);
    args.require_published_topology = true;
    args.expected_topology_receipt_sha256 = Some(canonical_sha256(&receipt_bytes(&receipt)?));
    let error = expect_refusal(cmd_release_manifest(&args), "unpublished entry")?;
    if !error
        .to_string()
        .contains("public release requires published topology evidence")
    {
        return Err(format!("refusal must name the posture law: {error}").into());
    }
    Ok(())
}

#[test]
fn unknown_published_row_state_refuses() -> TestResult {
    let dir = fixture_dir("unknown-state");
    prepare_fixture_dir(&dir)?;
    let receipt = dir.join("topology-publish.receipt.json");
    let mut document = published_receipt_document();
    mutate_row(
        &mut document,
        0,
        "state",
        json!("uploaded_waiting_for_registry"),
    )?;
    write_receipt(&receipt, &document)?;
    let args = generate_args(&dir, &receipt);
    let error = expect_refusal(cmd_release_manifest(&args), "unknown state")?;
    if !error.to_string().contains("unknown publication state") {
        return Err(format!("refusal must name the state law: {error}").into());
    }
    Ok(())
}

#[test]
fn duplicate_package_identity_refuses() -> TestResult {
    let dir = fixture_dir("duplicate");
    prepare_fixture_dir(&dir)?;
    let receipt = dir.join("topology-publish.receipt.json");
    let mut document = published_receipt_document();
    mutate_row(&mut document, 0, "release_order", json!(20))?;
    write_receipt(&receipt, &document)?;
    let args = generate_args(&dir, &receipt);
    let error = expect_refusal(cmd_release_manifest(&args), "duplicate")?;
    if !error.to_string().contains("duplicate package identity") {
        return Err(format!("refusal must name the dedupe law: {error}").into());
    }
    Ok(())
}

#[test]
fn phase_requirement_fails_closed_and_records_diagnostics() -> TestResult {
    let dir = fixture_dir("phase-fail-closed");
    prepare_fixture_dir(&dir)?;
    let receipt = dir.join("topology-publish.receipt.json");
    write_receipt(&receipt, &published_receipt_document())?;
    let package_receipt = dir.join("release-binary.receipt.json");
    let install_receipt = dir.join("release-binary-install.receipt.json");
    write_receipt(
        &package_receipt,
        &binary_receipt_document(BINARY_PACKAGE_RECEIPT_SCHEMA, false),
    )?;
    write_receipt(
        &install_receipt,
        &binary_receipt_document(BINARY_INSTALL_RECEIPT_SCHEMA, false),
    )?;
    let mut args = generate_args(&dir, &receipt);
    args.binary_package_receipt = Some(package_receipt);
    args.binary_install_receipt = Some(install_receipt);
    args.require_result = Some("complete".to_string());
    let error = expect_refusal(cmd_release_manifest(&args), "unverified attestation")?;
    if error.kind() != CargoAllowErrorKind::PolicyViolation {
        return Err(format!("the phase gate must fail as policy: {error}").into());
    }
    let result_bytes = receipt_bytes(&args.result_out)
        .map_err(|error| format!("failed reconciliation must be recorded: {error}"))?;
    let result: Value =
        serde_json::from_slice(&result_bytes).map_err(|error| format!("receipt: {error}"))?;
    if result.get("result").and_then(Value::as_str) != Some("instrument_failure") {
        return Err("the diagnostics must record the typed instrument failure".into());
    }
    if !args.output.exists() {
        return Err("diagnostics must retain the manifest".into());
    }
    Ok(())
}

#[test]
fn unpublished_candidate_records_instrument_failure_without_phase_failure() -> TestResult {
    let dir = fixture_dir("unpublished-candidate");
    prepare_fixture_dir(&dir)?;
    let receipt = dir.join("topology-publish.receipt.json");
    let mut document = published_receipt_document();
    candidate_posture(&mut document)?;
    write_receipt(&receipt, &document)?;
    let package_receipt = dir.join("release-binary.receipt.json");
    let install_receipt = dir.join("release-binary-install.receipt.json");
    write_receipt(
        &package_receipt,
        &binary_receipt_document(BINARY_PACKAGE_RECEIPT_SCHEMA, false),
    )?;
    write_receipt(
        &install_receipt,
        &binary_receipt_document(BINARY_INSTALL_RECEIPT_SCHEMA, false),
    )?;
    let mut args = generate_args(&dir, &receipt);
    args.binary_package_receipt = Some(package_receipt);
    args.binary_install_receipt = Some(install_receipt);
    cmd_release_manifest(&args)
        .map_err(|error| format!("candidate manifests must generate: {error}"))?;
    let manifest_bytes = receipt_bytes(&args.output)?;
    let manifest: Value =
        serde_json::from_slice(&manifest_bytes).map_err(|error| format!("manifest: {error}"))?;
    if str_field_of(&manifest, &["payload", "publication_posture"])? != "unpublished" {
        return Err("the candidate receipt must produce the unpublished posture".into());
    }
    let diagnostics = array_len_of(&manifest, &["instrument_diagnostics"])?;
    if diagnostics != 1 {
        return Err(format!("expected one instrument diagnostic, got {diagnostics}").into());
    }
    let result_bytes = receipt_bytes(&args.result_out)?;
    let result: Value =
        serde_json::from_slice(&result_bytes).map_err(|error| format!("receipt: {error}"))?;
    if result.get("result").and_then(Value::as_str) != Some("instrument_failure") {
        return Err("the candidate manifest must record its instrument failure".into());
    }
    Ok(())
}

#[test]
fn validate_mode_records_typed_diagnostics_for_incomplete_payloads() -> TestResult {
    let dir = fixture_dir("validate-incomplete");
    prepare_fixture_dir(&dir)?;
    let receipt = dir.join("topology-publish.receipt.json");
    write_receipt(&receipt, &published_receipt_document())?;
    let args = generate_args(&dir, &receipt);
    cmd_release_manifest(&args)?;
    let mut tampered: Value = serde_json::from_slice(&receipt_bytes(&args.output)?)
        .map_err(|error| format!("manifest: {error}"))?;
    set_field(
        tampered
            .get_mut("payload")
            .ok_or("manifest payload is missing")?,
        "publication_posture",
        json!("incident"),
    )?;
    let tampered_path = dir.join("tampered-manifest.json");
    write_receipt(&tampered_path, &tampered)?;
    let mut validate_args = generate_args(&dir, &receipt);
    validate_args.manifest = Some(tampered_path);
    validate_args.require_result = Some("complete".to_string());
    validate_args.result_out = dir.join("tampered-result.json");
    let error = expect_refusal(cmd_release_manifest(&validate_args), "incomplete payload")?;
    if error.kind() != CargoAllowErrorKind::PolicyViolation {
        return Err(format!("incomplete payloads must fail as policy: {error}").into());
    }
    let result_bytes = receipt_bytes(&validate_args.result_out)
        .map_err(|error| format!("diagnostics must be recorded: {error}"))?;
    let result: Value =
        serde_json::from_slice(&result_bytes).map_err(|error| format!("receipt: {error}"))?;
    if result.get("result").and_then(Value::as_str) != Some("release_incident") {
        return Err("the tampered manifest must classify as a release incident".into());
    }
    let gaps = array_len_of(&result, &["gaps"])?;
    if gaps == 0 {
        return Err("the release incident must name its gaps".into());
    }
    Ok(())
}

#[test]
fn topology_digest_binding_is_enforced() -> TestResult {
    let dir = fixture_dir("digest-binding");
    prepare_fixture_dir(&dir)?;
    let receipt = dir.join("topology-publish.receipt.json");
    write_receipt(&receipt, &published_receipt_document())?;
    let mut args = generate_args(&dir, &receipt);
    args.expected_topology_receipt_sha256 = Some("sha256:nope".to_string());
    let error = expect_refusal(cmd_release_manifest(&args), "digest mismatch")?;
    if !error
        .to_string()
        .contains("differs from the downloaded evidence")
    {
        return Err(format!("refusal must name the digest binding: {error}").into());
    }
    Ok(())
}

#[test]
fn generated_at_utc_formats_the_typed_timestamp() -> TestResult {
    for (seconds, expected) in [
        (0_u64, "1970-01-01T00:00:00Z"),
        (86_400, "1970-01-02T00:00:00Z"),
        (1_764_604_800, "2025-12-01T16:00:00Z"),
        (86_399, "1970-01-01T23:59:59Z"),
    ] {
        let instant = UNIX_EPOCH
            .checked_add(std::time::Duration::from_secs(seconds))
            .ok_or("fixture timestamp is unsupported")?;
        let rendered = generated_at_utc(instant)?;
        if rendered != expected {
            return Err(
                format!("at {seconds} seconds: expected {expected}, got {rendered}").into(),
            );
        }
    }
    Ok(())
}
