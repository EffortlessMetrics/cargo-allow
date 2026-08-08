use super::*;
use allow_core::{Finding, MatchOutcome};
use allow_inventory::InventorySource;
use serde_json::{Map, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

fn require(condition: bool, message: impl Into<String>) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

fn report_args<'a>(
    root: &'a Path,
    output: Option<&'a Path>,
    format: OutputFormat,
    findings: &'a [Finding],
    outcomes: &'a [MatchOutcome],
    inventory_facts: crate::InventoryFacts,
) -> ReportRenderArgs<'a> {
    ReportRenderArgs {
        command: "audit",
        format,
        baseline_debt_entries: 0,
        evidence: crate::EvidenceReportSummary::default(),
        findings,
        outcomes,
        failed: false,
        output,
        root,
        inventory_facts,
        inventory_source_identity: None,
        enforcement: None,
    }
}

#[test]
fn human_report_starts_with_the_common_summary() -> Result<(), String> {
    let root = temp_root("human").map_err(|error| error.to_string())?;
    let output = root.join("report.txt");
    let args = report_args(
        &root,
        Some(&output),
        OutputFormat::Human,
        &[],
        &[],
        crate::InventoryFacts::scanned(InventorySource::GitTracked, 1),
    );
    print_report_with_summary_config(args, None).map_err(|error| error.to_string())?;
    let text = fs::read_to_string(&output).map_err(|error| error.to_string())?;
    require(
        text.starts_with("Result: satisfied\nWhy:"),
        format!("common summary was not first: {text}"),
    )?;
    require(
        text.contains("cargo-allow audit"),
        "detailed human report was not preserved",
    )?;
    fs::remove_dir_all(root).map_err(|error| error.to_string())
}

#[test]
fn non_core_report_commands_delegate_without_a_summary() -> Result<(), String> {
    let root = temp_root("delegation").map_err(|error| error.to_string())?;
    let output = root.join("report.txt");
    let mut args = report_args(
        &root,
        Some(&output),
        OutputFormat::Human,
        &[],
        &[],
        crate::InventoryFacts::scanned(InventorySource::GitTracked, 1),
    );
    args.command = "other";
    print_report(args).map_err(|error| error.to_string())?;
    let text = fs::read_to_string(&output).map_err(|error| error.to_string())?;
    require(
        !text.starts_with("Result:"),
        format!("non-core command received a common summary: {text}"),
    )?;
    require(
        text.contains("cargo-allow other"),
        "delegated detailed report was not preserved",
    )?;
    fs::remove_dir_all(root).map_err(|error| error.to_string())
}

#[test]
fn json_detail_is_byte_equal_to_the_existing_renderer() -> Result<(), String> {
    let root = temp_root("json").map_err(|error| error.to_string())?;
    let expected_path = root.join("expected.json");
    let actual_path = root.join("actual.json");
    let facts = crate::InventoryFacts::scanned(InventorySource::GitTracked, 1);
    crate::reporting::print_report(report_args(
        &root,
        Some(&expected_path),
        OutputFormat::Json,
        &[],
        &[],
        facts,
    ))
    .map_err(|error| error.to_string())?;
    print_report_with_summary_config(
        report_args(
            &root,
            Some(&actual_path),
            OutputFormat::Json,
            &[],
            &[],
            facts,
        ),
        None,
    )
    .map_err(|error| error.to_string())?;
    let expected = fs::read(&expected_path).map_err(|error| error.to_string())?;
    let actual = fs::read(&actual_path).map_err(|error| error.to_string())?;
    require(expected == actual, "detailed JSON bytes changed")?;
    fs::remove_dir_all(root).map_err(|error| error.to_string())
}

#[test]
fn canonical_json_sorting_is_recursive_and_deterministic() -> Result<(), String> {
    let mut nested = Map::new();
    nested.insert("z".to_string(), Value::from(1));
    nested.insert("a".to_string(), Value::from(2));
    let mut root = Map::new();
    root.insert("z".to_string(), Value::Object(nested));
    root.insert("a".to_string(), Value::from(3));
    let mut value = Value::Object(root);
    sort_json_keys(&mut value);
    let bytes = serde_json::to_vec(&value).map_err(|error| error.to_string())?;
    require(
        bytes == br#"{"a":3,"z":{"a":2,"z":1}}"#,
        format!(
            "canonical JSON order changed: {}",
            String::from_utf8_lossy(&bytes)
        ),
    )
}

#[test]
fn summary_sidecar_is_structured_and_rejects_output_collision() -> Result<(), String> {
    let root = temp_root("sidecar").map_err(|error| error.to_string())?;
    let detail = root.join("report.json");
    let summary = root.join("summary.json");
    let config = SummaryOutputConfig::new(summary.clone(), vec![detail.clone()]);
    print_report_with_summary_config(
        report_args(
            &root,
            Some(&detail),
            OutputFormat::Json,
            &[],
            &[],
            crate::InventoryFacts::scanned(InventorySource::GitTracked, 1),
        ),
        Some(&config),
    )
    .map_err(|error| error.to_string())?;
    let value: Value =
        serde_json::from_str(&fs::read_to_string(&summary).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    let schema_id = value.pointer("/schema_id").and_then(Value::as_str);
    require(
        schema_id == Some(crate::core_command_summary::CORE_COMMAND_SUMMARY_SCHEMA_ID),
        "summary sidecar schema ID is missing",
    )?;
    let collision = SummaryOutputConfig::new(detail.clone(), vec![detail.clone()]);
    let error = print_report_with_summary_config(
        report_args(
            &root,
            Some(&detail),
            OutputFormat::Json,
            &[],
            &[],
            crate::InventoryFacts::scanned(InventorySource::GitTracked, 1),
        ),
        Some(&collision),
    )
    .err()
    .ok_or_else(|| "summary/detail collision should fail".to_string())?;
    require(
        error.kind() == CargoAllowErrorKind::Usage,
        format!("collision used the wrong error kind: {}", error.code()),
    )?;
    fs::remove_dir_all(root).map_err(|error| error.to_string())
}

#[test]
fn partial_inventory_cannot_render_as_satisfied() -> Result<(), String> {
    let root = temp_root("partial").map_err(|error| error.to_string())?;
    let mut facts = crate::InventoryFacts::scanned(InventorySource::GitTracked, 1);
    facts.completeness = InventoryCompleteness::Partial;
    let args = report_args(&root, None, OutputFormat::Json, &[], &[], facts);
    let summary = build_report_summary(&args).map_err(|error| error.to_string())?;
    require(
        summary.result_class == ResultClassV1::PartialData,
        "partial inventory did not remain partial-data",
    )?;
    require(
        summary.posture == CoreCommandPostureV1::Blocking,
        "partial inventory did not remain blocking",
    )?;
    fs::remove_dir_all(root).map_err(|error| error.to_string())
}

#[test]
fn scoped_inventory_with_a_later_scanner_skip_remains_partial() -> Result<(), String> {
    let root = temp_root("scoped-skip").map_err(|error| error.to_string())?;
    let facts =
        crate::InventoryFacts::scanned(InventorySource::GitTracked, 1).with_rust_files_skipped(1);
    let args = report_args(&root, None, OutputFormat::Json, &[], &[], facts);
    let summary = build_report_summary(&args).map_err(|error| error.to_string())?;
    require(
        summary.completeness == CompletenessV1::Partial,
        "scoped inventory hid a later Rust scanner skip",
    )?;
    require(
        summary.result_class == ResultClassV1::PartialData,
        "scoped inventory with scanner omissions did not remain partial-data",
    )?;
    fs::remove_dir_all(root).map_err(|error| error.to_string())
}

static NEXT_TEMP_ROOT: AtomicUsize = AtomicUsize::new(0);

fn temp_root(label: &str) -> std::io::Result<PathBuf> {
    let id = NEXT_TEMP_ROOT.fetch_add(1, Ordering::Relaxed);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-core-router-{label}-{}-{stamp}-{id}",
        std::process::id()
    ));
    fs::create_dir_all(&root)?;
    Ok(root)
}

fn identity(artifact_json: &str, root: &Path) -> Result<String, String> {
    canonical_semantic_identity(artifact_json, Some(root)).map_err(|error| error.to_string())
}

#[test]
fn semantic_identity_survives_repository_relocation() -> Result<(), String> {
    // The same repository content checked out at two different absolute paths
    // must produce one identity. Key scrubbing alone cannot reach a root that
    // is embedded inside a suggested command string (#3149).
    let artifact = |root: &str| {
        format!(
            r#"{{"root":{{"path":"{root}"}},
                "inventory":{{"root":"{root}","files_scanned":7}},
                "config":{{"suggested_init_command":"cargo-allow init --root \"{root}\""}}}}"#
        )
    };
    let left = identity(
        &artifact("/home/alice/checkout"),
        Path::new("/home/alice/checkout"),
    )?;
    let right = identity(&artifact("/srv/ci/build-42"), Path::new("/srv/ci/build-42"))?;
    require(
        left == right,
        format!("relocated identity drifted: {left} != {right}"),
    )
}

#[test]
fn semantic_identity_still_separates_different_content() -> Result<(), String> {
    // Root redaction must not flatten genuinely different repositories into
    // one identity.
    let root = Path::new("/home/alice/checkout");
    let left = identity(r#"{"inventory":{"files_scanned":7},"findings":1}"#, root)?;
    let right = identity(r#"{"inventory":{"files_scanned":7},"findings":2}"#, root)?;
    require(
        left != right,
        "different findings must not share a semantic identity",
    )
}

#[test]
fn semantic_identity_redacts_both_path_spellings() -> Result<(), String> {
    // Windows artifacts mix native backslash roots with forward-slash portable
    // paths in the same document; both must redact to the same placeholder.
    let root = Path::new(r"C:\work\repo");
    let mixed = identity(
        r#"{"a":"C:\\work\\repo\\src","b":"C:/work/repo/src"}"#,
        root,
    )?;
    let redacted = identity(
        r#"{"a":"<repository-root>\\src","b":"<repository-root>/src"}"#,
        root,
    )?;
    require(
        mixed == redacted,
        "native and portable root spellings must redact identically",
    )
}
