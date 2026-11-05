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

/// Assert one refusal message names its law without panicking macros.
fn expect_message(error: &CargoAllowError, needle: &str, label: &str) -> TestResult {
    if !error.to_string().contains(needle) {
        return Err(format!("{label}: refusal must name {needle:?}, got {error}").into());
    }
    Ok(())
}

/// Remove one top-level field of a receipt fixture without panicking indexing.
fn remove_field(document: &mut Value, key: &str) -> TestResult {
    if document
        .as_object_mut()
        .ok_or("receipt fixture is not an object")?
        .remove(key)
        .is_none()
    {
        return Err(format!("fixture field {key:?} is absent").into());
    }
    Ok(())
}

/// Run the command against one mutated copy of the published receipt fixture
/// and return the typed refusal.
fn topology_refusal_with<F>(
    name: &str,
    mutate: F,
) -> Result<CargoAllowError, Box<dyn std::error::Error>>
where
    F: FnOnce(&mut Value) -> TestResult,
{
    let dir = fixture_dir(name);
    prepare_fixture_dir(&dir)?;
    let receipt = dir.join("topology-publish.receipt.json");
    let mut document = published_receipt_document();
    mutate(&mut document)?;
    write_receipt(&receipt, &document)?;
    let args = generate_args(&dir, &receipt);
    expect_refusal(cmd_release_manifest(&args), name)
}

/// Run the command with a mutated binary receipt pair and return the typed
/// refusal.
fn binary_case_with<F, G>(
    name: &str,
    mutate_documents: F,
    mutate_args: G,
) -> Result<CargoAllowError, Box<dyn std::error::Error>>
where
    F: FnOnce(&mut Value, &mut Value) -> TestResult,
    G: FnOnce(&mut ReleaseManifestArgs),
{
    let dir = fixture_dir(name);
    prepare_fixture_dir(&dir)?;
    let receipt = dir.join("topology-publish.receipt.json");
    write_receipt(&receipt, &published_receipt_document())?;
    let mut package = binary_receipt_document(BINARY_PACKAGE_RECEIPT_SCHEMA, false);
    let mut install = binary_receipt_document(BINARY_INSTALL_RECEIPT_SCHEMA, false);
    mutate_documents(&mut package, &mut install)?;
    let package_path = dir.join("release-binary.receipt.json");
    let install_path = dir.join("release-binary-install.receipt.json");
    write_receipt(&package_path, &package)?;
    write_receipt(&install_path, &install)?;
    let mut args = generate_args(&dir, &receipt);
    args.binary_package_receipt = Some(package_path);
    args.binary_install_receipt = Some(install_path);
    mutate_args(&mut args);
    expect_refusal(cmd_release_manifest(&args), name)
}

// ---------------------------------------------------------------------------
// Receipt loading laws (load_receipt).
// ---------------------------------------------------------------------------

#[test]
fn missing_receipt_file_refuses() -> TestResult {
    let dir = fixture_dir("receipt-missing");
    prepare_fixture_dir(&dir)?;
    let args = generate_args(&dir, &dir.join("absent-topology-publish.receipt.json"));
    let error = expect_refusal(cmd_release_manifest(&args), "missing receipt")?;
    expect_message(&error, "invalid receipt", "missing receipt")
}

#[test]
fn malformed_json_receipt_refuses() -> TestResult {
    let dir = fixture_dir("receipt-malformed-json");
    prepare_fixture_dir(&dir)?;
    let receipt = dir.join("topology-publish.receipt.json");
    std::fs::write(&receipt, b"not json at all")
        .map_err(|error| format!("write {}: {error}", receipt.display()))?;
    let args = generate_args(&dir, &receipt);
    let error = expect_refusal(cmd_release_manifest(&args), "malformed receipt")?;
    expect_message(&error, "invalid receipt", "malformed receipt")
}

#[test]
fn non_object_receipt_refuses() -> TestResult {
    let dir = fixture_dir("receipt-array");
    prepare_fixture_dir(&dir)?;
    let receipt = dir.join("topology-publish.receipt.json");
    write_receipt(&receipt, &json!(["not", "an", "object"]))?;
    let args = generate_args(&dir, &receipt);
    let error = expect_refusal(cmd_release_manifest(&args), "array receipt")?;
    expect_message(&error, "JSON payload is not an object", "array receipt")
}

#[test]
fn unsupported_receipt_schema_refuses() -> TestResult {
    let error = topology_refusal_with("receipt-schema-id", |document| {
        set_field(
            document,
            "schema_id",
            json!("cargo-allow.some-other-receipt.v1"),
        )
    })?;
    expect_message(&error, "unsupported receipt schema", "schema drift")
}

#[test]
fn unsupported_receipt_schema_version_refuses() -> TestResult {
    let error = topology_refusal_with("receipt-schema-version", |document| {
        set_field(document, "schema_version", json!(2))
    })?;
    expect_message(&error, "unsupported receipt schema", "schema version drift")
}

// ---------------------------------------------------------------------------
// Topology evidence laws (load_topology_evidence).
// ---------------------------------------------------------------------------

#[test]
fn require_published_demands_expected_digest() -> TestResult {
    let dir = fixture_dir("require-published-digest");
    prepare_fixture_dir(&dir)?;
    let receipt = dir.join("topology-publish.receipt.json");
    write_receipt(&receipt, &published_receipt_document())?;
    let mut args = generate_args(&dir, &receipt);
    args.require_published_topology = true;
    let error = expect_refusal(cmd_release_manifest(&args), "missing digest")?;
    expect_message(
        &error,
        "requires the downloaded topology receipt digest",
        "missing digest",
    )
}

#[test]
fn topology_identity_disagreement_refuses() -> TestResult {
    let dir = fixture_dir("identity-drift");
    prepare_fixture_dir(&dir)?;
    let receipt = dir.join("topology-publish.receipt.json");
    write_receipt(&receipt, &published_receipt_document())?;
    let mut args = generate_args(&dir, &receipt);
    args.commit = Some("drifted-commit".to_string());
    let error = expect_refusal(cmd_release_manifest(&args), "identity drift")?;
    expect_message(&error, "identity disagrees with release", "identity drift")
}

#[test]
fn incomplete_candidate_receipt_refuses() -> TestResult {
    let error = topology_refusal_with("receipt-incomplete", |document| {
        set_field(document, "complete", json!(false))
    })?;
    expect_message(
        &error,
        "not a complete cargo-allow candidate",
        "incomplete receipt",
    )
}

#[test]
fn non_cargo_allow_receipt_mode_refuses() -> TestResult {
    let error = topology_refusal_with("receipt-mode-drift", |document| {
        set_field(document, "mode", json!("other-tool"))
    })?;
    expect_message(&error, "not a complete cargo-allow candidate", "mode drift")
}

#[test]
fn receipt_without_rows_refuses() -> TestResult {
    let error =
        topology_refusal_with("receipt-no-rows", |document| remove_field(document, "rows"))?;
    expect_message(&error, "has no package rows", "absent rows")
}

#[test]
fn empty_rows_receipt_refuses() -> TestResult {
    let error = topology_refusal_with("receipt-empty-rows", |document| {
        set_field(document, "rows", json!([]))
    })?;
    expect_message(&error, "has no package rows", "empty rows")
}

#[test]
fn non_boolean_publish_refuses() -> TestResult {
    let error = topology_refusal_with("receipt-publish-string", |document| {
        set_field(document, "publish", json!("yes"))
    })?;
    expect_message(&error, "publish field must be a Boolean", "string publish")
}

#[test]
fn published_receipt_without_marker_refuses() -> TestResult {
    let error = topology_refusal_with("receipt-marker-absent", |document| {
        remove_field(document, "first_irreversible_row")
    })?;
    expect_message(&error, "lacks an irreversible-row marker", "absent marker")
}

#[test]
fn null_marker_with_published_rows_refuses() -> TestResult {
    let error = topology_refusal_with("receipt-null-marker", |document| {
        set_field(document, "first_irreversible_row", Value::Null)
    })?;
    expect_message(
        &error,
        "newly published rows require an irreversible-row marker",
        "null marker",
    )
}

#[test]
fn invalid_irreversible_marker_refuses() -> TestResult {
    let boolean = topology_refusal_with("receipt-marker-boolean", |document| {
        set_field(document, "first_irreversible_row", json!(true))
    })?;
    expect_message(
        &boolean,
        "invalid irreversible-row marker",
        "boolean marker",
    )?;
    let zero = topology_refusal_with("receipt-marker-zero", |document| {
        set_field(document, "first_irreversible_row", json!(0))
    })?;
    expect_message(&zero, "invalid irreversible-row marker", "zero marker")
}

#[test]
fn malformed_topology_digest_refuses() -> TestResult {
    let error = topology_refusal_with("receipt-topology-digest", |document| {
        set_field(document, "topology_sha256", json!("nope"))
    })?;
    expect_message(&error, "malformed topology digest", "topology digest")
}

#[test]
fn malformed_lock_digest_refuses() -> TestResult {
    let error = topology_refusal_with("receipt-lock-digest", |document| {
        set_field(document, "cargo_lock_sha256", json!("nope"))
    })?;
    expect_message(&error, "malformed Cargo.lock digest", "lock digest")
}

#[test]
fn publication_authorization_gate_refuses() -> TestResult {
    let dir = fixture_dir("authorization-gate");
    prepare_fixture_dir(&dir)?;
    let receipt = dir.join("topology-publish.receipt.json");
    write_receipt(&receipt, &published_receipt_document())?;
    let mut args = generate_args(&dir, &receipt);
    args.require_published_topology = true;
    args.expected_topology_receipt_sha256 = Some(canonical_sha256(&receipt_bytes(&receipt)?));
    // The release gate compiles a one-use sha256 digest; the issue-form
    // authorization on the fixture receipt must refuse through the same arm.
    args.authorization_digest = Some("issue:3760".to_string());
    let error = expect_refusal(cmd_release_manifest(&args), "authorization gate")?;
    expect_message(
        &error,
        "publication authorization differs from the release gate",
        "authorization gate",
    )
}

#[test]
fn require_published_demands_package_archive_evidence() -> TestResult {
    let dir = fixture_dir("require-published-archive");
    prepare_fixture_dir(&dir)?;
    let receipt = dir.join("topology-publish.receipt.json");
    let mut document = published_receipt_document();
    set_field(&mut document, "authorization", json!(digest('f')))?;
    write_receipt(&receipt, &document)?;
    let mut args = generate_args(&dir, &receipt);
    args.require_published_topology = true;
    args.expected_topology_receipt_sha256 = Some(canonical_sha256(&receipt_bytes(&receipt)?));
    args.authorization_digest = Some(digest('f'));
    let error = expect_refusal(cmd_release_manifest(&args), "archive evidence")?;
    expect_message(&error, "package archive unavailable", "archive evidence")
}

// ---------------------------------------------------------------------------
// Candidate row laws (candidate_row).
// ---------------------------------------------------------------------------

#[test]
fn incomplete_row_refuses() -> TestResult {
    let error = topology_refusal_with("row-incomplete", |document| {
        mutate_row(document, 0, "local_checksum", Value::Null)
    })?;
    expect_message(
        &error,
        "topology receipt row is incomplete",
        "incomplete row",
    )
}

#[test]
fn unparseable_row_version_refuses() -> TestResult {
    let error = topology_refusal_with("row-version-unparseable", |document| {
        mutate_row(document, 0, "version", json!("not-a-version"))
    })?;
    expect_message(
        &error,
        "is not a validated typed release identity",
        "unparseable version",
    )
}

#[test]
fn malformed_local_checksum_refuses() -> TestResult {
    let error = topology_refusal_with("row-local-digest", |document| {
        mutate_row(document, 0, "local_checksum", json!("sha256:zz"))
    })?;
    expect_message(
        &error,
        "malformed crate digest for allow-core",
        "local digest",
    )
}

#[test]
fn oversized_release_order_refuses() -> TestResult {
    let error = topology_refusal_with("row-order-oversized", |document| {
        mutate_row(document, 0, "release_order", json!(5_000_000_000_u64))
    })?;
    expect_message(
        &error,
        "exceeds the manifest order range",
        "oversized order",
    )
}

#[test]
fn zero_release_order_refuses() -> TestResult {
    let error = topology_refusal_with("row-order-zero", |document| {
        mutate_row(document, 0, "release_order", json!(0))
    })?;
    expect_message(&error, "duplicate package identity", "zero order")
}

// ---------------------------------------------------------------------------
// Published and candidate row laws (manifest_row_from_candidate and
// candidate_manifest_row).
// ---------------------------------------------------------------------------

#[test]
fn missing_state_row_refuses() -> TestResult {
    let error = topology_refusal_with("row-state-missing", |document| {
        mutate_row(document, 0, "state", json!("missing"))
    })?;
    expect_message(&error, "is not registry-verified", "missing state")
}

#[test]
fn provider_unavailable_state_row_refuses() -> TestResult {
    let error = topology_refusal_with("row-state-provider", |document| {
        mutate_row(document, 0, "state", json!("provider_unavailable"))
    })?;
    expect_message(&error, "is not registry-verified", "provider state")
}

#[test]
fn published_row_without_registry_checksum_refuses() -> TestResult {
    let error = topology_refusal_with("row-registry-null", |document| {
        mutate_row(document, 0, "registry_checksum", Value::Null)
    })?;
    expect_message(
        &error,
        "registry checksum missing or malformed for published row",
        "null registry digest",
    )
}

#[test]
fn malformed_candidate_registry_checksum_refuses() -> TestResult {
    let error = topology_refusal_with("row-registry-candidate", |document| {
        set_field(document, "publish", json!(false))?;
        set_field(document, "first_irreversible_row", Value::Null)?;
        mutate_row(document, 0, "state", json!("packaged"))?;
        mutate_row(document, 1, "state", json!("packaged"))?;
        mutate_row(document, 1, "registry_checksum", Value::Null)?;
        mutate_row(document, 0, "registry_checksum", json!("sha256:zz"))
    })?;
    expect_message(
        &error,
        "malformed registry checksum for allow-core",
        "candidate digest",
    )
}

// ---------------------------------------------------------------------------
// Binary evidence laws (load_binary_evidence).
// ---------------------------------------------------------------------------

#[test]
fn binary_receipt_missing_field_refuses() -> TestResult {
    let error = binary_case_with(
        "binary-field-missing",
        |package, _| remove_field(package, "archive_name"),
        |_| {},
    )?;
    expect_message(
        &error,
        "binary receipt is missing a required field",
        "missing field",
    )
}

#[test]
fn binary_receipt_field_disagreement_refuses() -> TestResult {
    let error = binary_case_with(
        "binary-field-drift",
        |_, install| set_field(install, "commit", json!("other-commit")),
        |_| {},
    )?;
    expect_message(
        &error,
        "binary receipt disagreement in commit",
        "field drift",
    )
}

#[test]
fn binary_receipt_version_disagreement_refuses() -> TestResult {
    let error = binary_case_with(
        "binary-version-drift",
        |package, install| {
            set_field(package, "version", json!("0.1.11"))?;
            set_field(install, "version", json!("0.1.11"))
        },
        |_| {},
    )?;
    expect_message(
        &error,
        "binary receipt version disagrees with manifest",
        "version drift",
    )
}

#[test]
fn binary_receipt_identity_disagreement_refuses() -> TestResult {
    let error = binary_case_with(
        "binary-tag-drift",
        |_, _| Ok(()),
        |args| {
            args.tag = Some("v0.1.11".to_string());
        },
    )?;
    expect_message(
        &error,
        "binary receipt tag disagrees with manifest identity",
        "tag drift",
    )
}

#[test]
fn unsupported_binary_target_refuses() -> TestResult {
    let error = binary_case_with(
        "binary-target",
        |package, install| {
            set_field(package, "target_triple", json!("aarch64-unknown-linux-gnu"))?;
            set_field(install, "target_triple", json!("aarch64-unknown-linux-gnu"))
        },
        |_| {},
    )?;
    expect_message(&error, "unsupported binary target", "target drift")
}

#[test]
fn binary_requires_linux_platform_refuses() -> TestResult {
    let error = binary_case_with(
        "binary-platform",
        |_, _| Ok(()),
        |args| {
            args.platforms = "macos".to_string();
        },
    )?;
    expect_message(
        &error,
        "Linux binary asset requires linux in PLATFORMS",
        "platform drift",
    )
}

#[test]
fn binary_archive_name_law_refuses() -> TestResult {
    let error = binary_case_with(
        "binary-archive-name",
        |package, install| {
            set_field(package, "archive_name", json!("wrong.tar.gz"))?;
            set_field(install, "archive_name", json!("wrong.tar.gz"))
        },
        |_| {},
    )?;
    expect_message(
        &error,
        "binary archive name is not the stable Linux name",
        "archive name",
    )
}

#[test]
fn binary_archive_format_law_refuses() -> TestResult {
    let error = binary_case_with(
        "binary-archive-format",
        |package, install| {
            set_field(package, "archive_format", json!("zip"))?;
            set_field(install, "archive_format", json!("zip"))
        },
        |_| {},
    )?;
    expect_message(
        &error,
        "Linux binary archive format must be tar.gz",
        "archive format",
    )
}

#[test]
fn binary_executable_name_law_refuses() -> TestResult {
    let error = binary_case_with(
        "binary-executable-name",
        |package, install| {
            set_field(package, "executable_name", json!("cargo-allowx"))?;
            set_field(install, "executable_name", json!("cargo-allowx"))
        },
        |_| {},
    )?;
    expect_message(
        &error,
        "binary executable name must be cargo-allow",
        "executable name",
    )
}

#[test]
fn binary_requires_toolchain_and_runner_refuses() -> TestResult {
    let error = binary_case_with(
        "binary-toolchain",
        |_, _| Ok(()),
        |args| {
            args.rust_toolchain = String::new();
        },
    )?;
    expect_message(
        &error,
        "binary toolchain and runner are required",
        "toolchain drift",
    )
}

#[test]
fn malformed_binary_digest_refuses() -> TestResult {
    let error = binary_case_with(
        "binary-digest",
        |package, install| {
            set_field(package, "executable_sha256", json!("sha256:zz"))?;
            set_field(install, "executable_sha256", json!("sha256:zz"))
        },
        |_| {},
    )?;
    expect_message(
        &error,
        "malformed binary digest executable_sha256",
        "binary digest",
    )
}

#[test]
fn verified_attestation_binary_completes_without_diagnostics() -> TestResult {
    let dir = fixture_dir("binary-attested");
    prepare_fixture_dir(&dir)?;
    let receipt = dir.join("topology-publish.receipt.json");
    write_receipt(&receipt, &published_receipt_document())?;
    let package_path = dir.join("release-binary.receipt.json");
    let install_path = dir.join("release-binary-install.receipt.json");
    write_receipt(
        &package_path,
        &binary_receipt_document(BINARY_PACKAGE_RECEIPT_SCHEMA, true),
    )?;
    write_receipt(
        &install_path,
        &binary_receipt_document(BINARY_INSTALL_RECEIPT_SCHEMA, true),
    )?;
    let mut args = generate_args(&dir, &receipt);
    args.binary_package_receipt = Some(package_path);
    args.binary_install_receipt = Some(install_path);
    args.require_result = Some("complete".to_string());
    cmd_release_manifest(&args)
        .map_err(|error| format!("attested binary evidence must complete: {error}"))?;
    let manifest: Value = serde_json::from_slice(&receipt_bytes(&args.output)?)
        .map_err(|error| format!("manifest: {error}"))?;
    let diagnostics = array_len_of(&manifest, &["instrument_diagnostics"])?;
    if diagnostics != 0 {
        return Err(format!(
            "verified attestation must stay silent, got {diagnostics} diagnostics"
        )
        .into());
    }
    let evidence = manifest
        .get("payload")
        .and_then(|payload| payload.get("consumed_evidence"))
        .and_then(Value::as_array)
        .ok_or("consumed evidence is missing")?;
    let install_verified = evidence.iter().any(|row| {
        row.get("schema_id").and_then(Value::as_str) == Some(BINARY_INSTALL_RECEIPT_SCHEMA)
            && row.get("result_class").and_then(Value::as_str) == Some("verified")
    });
    if !install_verified {
        return Err("the verified install receipt must be retained as verified evidence".into());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Generate input and required-result laws (require_generate_inputs,
// parse_required_result, build_envelope).
// ---------------------------------------------------------------------------

#[test]
fn missing_generate_input_refuses() -> TestResult {
    let dir = fixture_dir("input-version");
    prepare_fixture_dir(&dir)?;
    let mut args = generate_args(&dir, &dir.join("unused-topology-publish.receipt.json"));
    args.version = None;
    let error = expect_refusal(cmd_release_manifest(&args), "missing version")?;
    expect_message(
        &error,
        "--version is required in generate mode",
        "missing version",
    )
}

#[test]
fn binary_receipts_require_each_other_refuses() -> TestResult {
    let dir = fixture_dir("input-binary-pair");
    prepare_fixture_dir(&dir)?;
    let mut args = generate_args(&dir, &dir.join("unused-topology-publish.receipt.json"));
    args.binary_package_receipt = Some(dir.join("only-package.receipt.json"));
    let error = expect_refusal(cmd_release_manifest(&args), "lone package receipt")?;
    expect_message(
        &error,
        "binary package and install receipts must be supplied together",
        "binary pair",
    )
}

#[test]
fn unsupported_auth_source_refuses() -> TestResult {
    let dir = fixture_dir("input-auth-source");
    prepare_fixture_dir(&dir)?;
    let mut args = generate_args(&dir, &dir.join("unused-topology-publish.receipt.json"));
    args.auth_source = Some("github_token".to_string());
    let error = expect_refusal(cmd_release_manifest(&args), "auth source")?;
    expect_message(
        &error,
        "only crates_io_api_token is supported",
        "auth source",
    )
}

#[test]
fn unknown_required_result_refuses() -> TestResult {
    let dir = fixture_dir("input-required-result");
    prepare_fixture_dir(&dir)?;
    let receipt = dir.join("topology-publish.receipt.json");
    write_receipt(&receipt, &published_receipt_document())?;
    let mut args = generate_args(&dir, &receipt);
    args.manifest = Some(receipt);
    args.require_result = Some("not-a-typed-result".to_string());
    args.result_out = dir.join("unused-result.json");
    let error = expect_refusal(cmd_release_manifest(&args), "required result")?;
    expect_message(&error, "unknown required result", "required result")?;
    if args.result_out.exists() {
        return Err("an unknown required result must fail before any receipt is written".into());
    }
    Ok(())
}

#[test]
fn workflow_run_id_is_optional_in_the_authorization_reference() -> TestResult {
    let dir = fixture_dir("workflow-reference-optional");
    prepare_fixture_dir(&dir)?;
    let receipt = dir.join("topology-publish.receipt.json");
    write_receipt(&receipt, &published_receipt_document())?;
    let mut args = generate_args(&dir, &receipt);
    args.workflow_run_id = None;
    cmd_release_manifest(&args)
        .map_err(|error| format!("a run without a workflow id must generate: {error}"))?;
    let manifest: Value = serde_json::from_slice(&receipt_bytes(&args.output)?)
        .map_err(|error| format!("manifest: {error}"))?;
    if str_field_of(&manifest, &["authorization_reference"])? != "workflow/crates-io" {
        return Err("a run without a workflow id must use the generic reference".into());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Validate-mode refusal laws (cmd_validate).
// ---------------------------------------------------------------------------

#[test]
fn validate_mode_missing_manifest_refuses() -> TestResult {
    let dir = fixture_dir("validate-missing");
    prepare_fixture_dir(&dir)?;
    let mut args = generate_args(&dir, &dir.join("unused-topology-publish.receipt.json"));
    args.manifest = Some(dir.join("absent-manifest.json"));
    let error = expect_refusal(cmd_release_manifest(&args), "missing manifest")?;
    expect_message(&error, "cannot read manifest", "missing manifest")
}

#[test]
fn validate_mode_unparseable_manifest_refuses_without_diagnostics() -> TestResult {
    let dir = fixture_dir("validate-garbage");
    prepare_fixture_dir(&dir)?;
    let manifest_path = dir.join("garbage-manifest.json");
    std::fs::write(&manifest_path, b"not a manifest at all")
        .map_err(|error| format!("write {}: {error}", manifest_path.display()))?;
    let mut args = generate_args(&dir, &dir.join("unused-topology-publish.receipt.json"));
    args.manifest = Some(manifest_path);
    let error = expect_refusal(cmd_release_manifest(&args), "garbage manifest")?;
    expect_message(
        &error,
        "does not parse against the typed contract",
        "garbage manifest",
    )?;
    if args.result_out.exists() {
        return Err("an unparseable manifest must not produce a diagnostics receipt".into());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Artifact emission refusal laws (write_manifest_and_sidecar,
// emit_result_receipt).
// ---------------------------------------------------------------------------

#[test]
fn unwritable_manifest_directory_refuses() -> TestResult {
    let dir = fixture_dir("output-parent-blocked");
    prepare_fixture_dir(&dir)?;
    let receipt = dir.join("topology-publish.receipt.json");
    write_receipt(&receipt, &published_receipt_document())?;
    let blocker = dir.join("blocker");
    std::fs::write(&blocker, b"not a directory")
        .map_err(|error| format!("write blocker: {error}"))?;
    let mut args = generate_args(&dir, &receipt);
    args.output = blocker.join("release-manifest-v2.json");
    let error = expect_refusal(cmd_release_manifest(&args), "blocked output")?;
    expect_message(&error, "cannot create manifest directory", "blocked output")
}

#[test]
fn unwritable_result_directory_refuses() -> TestResult {
    let dir = fixture_dir("result-parent-blocked");
    prepare_fixture_dir(&dir)?;
    let receipt = dir.join("topology-publish.receipt.json");
    write_receipt(&receipt, &published_receipt_document())?;
    let blocker = dir.join("blocker");
    std::fs::write(&blocker, b"not a directory")
        .map_err(|error| format!("write blocker: {error}"))?;
    let mut args = generate_args(&dir, &receipt);
    args.result_out = blocker.join("release-manifest-v2.result.json");
    let error = expect_refusal(cmd_release_manifest(&args), "blocked result")?;
    expect_message(
        &error,
        "cannot create result receipt directory",
        "blocked result",
    )
}

#[test]
fn unwritable_github_output_refuses() -> TestResult {
    let dir = fixture_dir("github-output-blocked");
    prepare_fixture_dir(&dir)?;
    let receipt = dir.join("topology-publish.receipt.json");
    write_receipt(&receipt, &published_receipt_document())?;
    let blocked = dir.join("blocked-github-output");
    std::fs::create_dir_all(&blocked).map_err(|error| format!("create blocker: {error}"))?;
    let mut args = generate_args(&dir, &receipt);
    args.github_output = Some(blocked);
    let error = expect_refusal(cmd_release_manifest(&args), "blocked github output")?;
    expect_message(&error, "cannot open GitHub output", "blocked github output")
}

// ---------------------------------------------------------------------------
// Timestamp and artifact-reference projection laws.
// ---------------------------------------------------------------------------

#[test]
fn generated_at_utc_rejects_a_pre_epoch_clock() -> TestResult {
    let instant = UNIX_EPOCH
        .checked_sub(std::time::Duration::from_secs(1))
        .ok_or("pre-epoch instants must be representable")?;
    let error = match generated_at_utc(instant) {
        Ok(rendered) => return Err(format!("a pre-epoch clock rendered {rendered}").into()),
        Err(error) => error,
    };
    if error.kind() != CargoAllowErrorKind::InstrumentFailure {
        return Err(format!("a pre-epoch clock must fail as an instrument: {error}").into());
    }
    expect_message(&error, "clock precedes the Unix epoch", "pre-epoch clock")
}

#[test]
fn generated_at_utc_rejects_a_clock_beyond_the_calendar() -> TestResult {
    let seconds = 3_900_000_u64 * 86_400;
    let instant = UNIX_EPOCH
        .checked_add(std::time::Duration::from_secs(seconds))
        .ok_or("far-future instants must be representable")?;
    let error = match generated_at_utc(instant) {
        Ok(rendered) => return Err(format!("a far-future clock rendered {rendered}").into()),
        Err(error) => error,
    };
    if error.kind() != CargoAllowErrorKind::InstrumentFailure {
        return Err(format!("a far-future clock must fail as an instrument: {error}").into());
    }
    expect_message(&error, "exceeds the supported calendar", "far-future clock")
}

#[test]
fn artifact_reference_forms_the_receipt_paths() -> TestResult {
    let relative = artifact_reference(Path::new("target/cargo-allow/evidence.json"));
    if relative != "target/cargo-allow/evidence.json" {
        return Err(format!("relative evidence must stay relative: {relative}").into());
    }
    let cwd = std::env::current_dir().map_err(|error| format!("cwd: {error}"))?;
    let inside = artifact_reference(&cwd.join("sub").join("evidence.json"));
    if inside != "sub/evidence.json" {
        return Err(format!(
            "absolute evidence under the checkout must project relative: {inside}"
        )
        .into());
    }
    let outside =
        artifact_reference(&std::env::temp_dir().join("cargo-allow-artifact-reference.json"));
    if outside != "artifact/cargo-allow-artifact-reference.json" {
        return Err(format!(
            "absolute evidence outside the checkout must use the artifact/ form: {outside}"
        )
        .into());
    }
    let root = std::env::temp_dir()
        .ancestors()
        .last()
        .ok_or("path ancestors are empty")?
        .to_path_buf();
    let rendered = artifact_reference(&root);
    if rendered != "artifact/" {
        return Err(format!(
            "a path without a file name must fall back to the artifact/ form: {rendered}"
        )
        .into());
    }
    Ok(())
}

#[test]
fn null_marker_with_verified_existing_rows_completes() -> TestResult {
    let dir = fixture_dir("verified-existing-null-marker");
    prepare_fixture_dir(&dir)?;
    let receipt = dir.join("topology-publish.receipt.json");
    let mut document = published_receipt_document();
    set_field(&mut document, "first_irreversible_row", Value::Null)?;
    let rows = document
        .get_mut("rows")
        .and_then(Value::as_array_mut)
        .ok_or("fixture rows are missing")?;
    for row in rows.iter_mut() {
        let object = row.as_object_mut().ok_or("fixture row is not an object")?;
        object.insert("state".to_string(), json!("verified_existing"));
    }
    write_receipt(&receipt, &document)?;
    let mut args = generate_args(&dir, &receipt);
    args.require_result = Some("complete".to_string());
    cmd_release_manifest(&args)
        .map_err(|error| format!("verified-existing rows must generate: {error}"))?;
    let result_bytes = receipt_bytes(&args.result_out)?;
    let result: Value =
        serde_json::from_slice(&result_bytes).map_err(|error| format!("receipt: {error}"))?;
    if result.get("result").and_then(Value::as_str) != Some("complete") {
        return Err(format!(
            "verified-existing evidence must reach complete, got {:?}",
            result.get("result")
        )
        .into());
    }
    Ok(())
}
