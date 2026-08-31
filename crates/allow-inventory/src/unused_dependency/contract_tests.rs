//! Contract fixtures for the unused-dependency inventory (#3909 PR A).
//!
//! Each test pins one true-unused or false-positive shape from the issue's
//! negative-control law. Fixture manifests and sources are built from
//! single-line parts joined with `\n`, never physical multi-line strings,
//! so autocrlf checkout smudging can never change what the analyzer sees.
//! Every test returns `Result<(), String>` and the module carries no
//! panic-family macros: failures flow through `require` messages.

use super::{
    UNUSED_DEPENDENCY_RECEIPT_V1_SCHEMA_ID, UnusedDependencyDependencyClassV1,
    UnusedDependencyDispositionV1, UnusedDependencyExceptionV1, UnusedDependencyFindingV1,
    UnusedDependencyInstrumentPostureV1, UnusedDependencyLibIdentityV1, UnusedDependencyReceiptV1,
    UnusedDependencyRequestV1, UnusedDependencySourceInputV1, empty_receipt, inventory_packages,
    inventory_unused_dependencies, receipt_scan_is_complete, render_unused_dependency_receipt_v1,
    validate_exception, validate_receipt,
};

fn require(condition: bool, message: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.to_string())
    }
}

/// Join fixture lines with `\n`: never physical multi-line strings.
fn lines(parts: &[&str]) -> String {
    parts.join("\n")
}

fn source(relative_path: &str, parts: &[&str]) -> UnusedDependencySourceInputV1 {
    UnusedDependencySourceInputV1 {
        relative_path: relative_path.to_string(),
        text: lines(parts),
    }
}

fn request(
    package_name: &str,
    manifest: String,
    source_inputs: Vec<UnusedDependencySourceInputV1>,
    build_script_present: bool,
) -> UnusedDependencyRequestV1 {
    UnusedDependencyRequestV1 {
        package_name: package_name.to_string(),
        package_version: "0.1.0".to_string(),
        configuration_id: format!("{package_name}.default"),
        manifest_text: manifest,
        source_inputs,
        build_script_present,
        dependency_lib_identities: Vec::new(),
    }
}

fn first_finding(receipt: &UnusedDependencyReceiptV1) -> Result<UnusedDependencyFindingV1, String> {
    receipt
        .findings
        .first()
        .cloned()
        .ok_or_else(|| "receipt lost its finding".to_string())
}

fn package_manifest(dependency_line: &str) -> String {
    lines(&[
        "[package]",
        "name = \"sample\"",
        "version = \"0.1.0\"",
        "",
        "[dependencies]",
        dependency_line,
    ])
}

/// Fixture 1: a normal dependency nothing references is an advisory
/// candidate finding whose evidence names the scanned set and whose
/// limitations name the composition's blind spots.
#[test]
fn true_unused_normal_dependency_is_apparently_unused() -> Result<(), String> {
    let manifest = package_manifest("serde_json = \"1\"");
    let sources = vec![source("src/lib.rs", &["pub fn nothing() {}"])];
    let receipt = inventory_unused_dependencies(&request("sample", manifest, sources, false))?;
    require(
        receipt.instrument_posture == UnusedDependencyInstrumentPostureV1::Complete,
        "an advisory zero-reference inventory must stay Complete",
    )?;
    require(receipt.packages_inspected == 1, "one package was inspected")?;
    require(
        receipt.findings.len() == 1,
        "exactly one finding is expected",
    )?;
    let finding = first_finding(&receipt)?;
    require(
        finding.disposition == UnusedDependencyDispositionV1::ApparentlyUnused,
        "a never-referenced normal dependency is ApparentlyUnused",
    )?;
    require(
        finding.manifest_row.dependency_name == "serde_json",
        "the finding keeps the registry dependency name",
    )?;
    require(
        finding.evidence.iter().any(|entry| {
            entry.contains("src/lib.rs") && entry.contains("scanned_no_reference_found")
        }),
        "zero-reference evidence must name the scanned input set",
    )?;
    require(
        finding
            .limitations
            .iter()
            .any(|note| note.contains("absence is not proof of non-use")),
        "the absence limitation must travel with the advisory finding",
    )?;
    require(
        finding
            .limitations
            .iter()
            .any(|note| note.contains("proc-macro expansion")),
        "the unscanned kinds must travel with the advisory finding",
    )
}

/// Fixture 2 (negative control 1): an optional dependency activated through
/// `dep:` is ConditionallyUsed — default-feature analysis can never declare
/// it unused globally.
#[test]
fn optional_feature_activator_is_conditionally_used() -> Result<(), String> {
    let manifest = lines(&[
        "[package]",
        "name = \"with-optional\"",
        "version = \"0.1.0\"",
        "",
        "[dependencies]",
        "changie = { version = \"1\", optional = true }",
        "",
        "[features]",
        "changie = [\"dep:changie\"]",
    ]);
    let sources = vec![source("src/lib.rs", &["pub fn ready() {}"])];
    let receipt =
        inventory_unused_dependencies(&request("with-optional", manifest, sources, false))?;
    let finding = first_finding(&receipt)?;
    require(
        finding.disposition == UnusedDependencyDispositionV1::ConditionallyUsed,
        "a dep:-activated optional dependency is ConditionallyUsed",
    )?;
    require(
        finding.manifest_row.class == UnusedDependencyDependencyClassV1::OptionalNormal,
        "the optional row keeps the OptionalNormal class",
    )?;
    require(
        finding.manifest_row.optional,
        "the optional flag is carried on the row",
    )?;
    require(
        finding
            .evidence
            .iter()
            .any(|entry| entry.contains("Cargo.toml") && entry.contains("dep:changie")),
        "feature-activator evidence must be retained with its manifest line",
    )
}

/// Fixture 3 (negative control 3): a dev-dependency referenced only by an
/// integration test is DevFixtureUse, while the same reference under src/
/// is Used.
#[test]
fn dev_dependency_used_by_integration_tests_is_dev_fixture_use() -> Result<(), String> {
    let manifest = lines(&[
        "[package]",
        "name = \"with-dev\"",
        "version = \"0.1.0\"",
        "",
        "[dev-dependencies]",
        "tempfile = \"3\"",
    ]);
    let fixture_only = vec![
        source("src/lib.rs", &["pub fn core() {}"]),
        source("tests/it.rs", &["use tempfile::tempdir;", ""]),
    ];
    let receipt =
        inventory_unused_dependencies(&request("with-dev", manifest.clone(), fixture_only, false))?;
    let finding = first_finding(&receipt)?;
    require(
        finding.disposition == UnusedDependencyDispositionV1::DevFixtureUse,
        "a dev-dependency referenced only under tests/ is DevFixtureUse",
    )?;
    require(
        finding
            .evidence
            .iter()
            .all(|entry| entry.starts_with("tests/")),
        "DevFixtureUse evidence must come only from fixture inputs",
    )?;

    let with_src_use = vec![
        source(
            "src/lib.rs",
            &["pub fn core() {}", "use tempfile::tempdir;", ""],
        ),
        source("tests/it.rs", &["use tempfile::tempdir;", ""]),
    ];
    let receipt =
        inventory_unused_dependencies(&request("with-dev", manifest, with_src_use, false))?;
    let finding = first_finding(&receipt)?;
    require(
        finding.disposition == UnusedDependencyDispositionV1::Used,
        "a dev-dependency referenced anywhere in src/ is Used",
    )
}

/// Fixture 4 (negative control 2 class): a build-dependency consumed by the
/// build-script input is BuildOrGeneratedUse; a declared build script with
/// no scanned input keeps the row out of unused as well.
#[test]
fn build_dependency_is_build_or_generated_use() -> Result<(), String> {
    let manifest = lines(&[
        "[package]",
        "name = \"with-build\"",
        "version = \"0.1.0\"",
        "",
        "[build-dependencies]",
        "cc = \"1\"",
    ]);
    let with_build_input = vec![source(
        "build.rs",
        &["fn main() {", "    let build = cc::Build::new();", "}"],
    )];
    let receipt = inventory_unused_dependencies(&request(
        "with-build",
        manifest.clone(),
        with_build_input,
        false,
    ))?;
    let finding = first_finding(&receipt)?;
    require(
        finding.disposition == UnusedDependencyDispositionV1::BuildOrGeneratedUse,
        "a build-dependency referenced by build.rs is BuildOrGeneratedUse",
    )?;
    require(
        finding
            .evidence
            .iter()
            .any(|entry| entry.starts_with("build.rs:")),
        "the evidence must name the build-script input",
    )?;

    let no_inputs = Vec::new();
    let receipt = inventory_unused_dependencies(&request("with-build", manifest, no_inputs, true))?;
    let finding = first_finding(&receipt)?;
    require(
        finding.disposition == UnusedDependencyDispositionV1::BuildOrGeneratedUse,
        "a declared build script keeps an unreferenced build-dependency out of unused",
    )?;
    require(
        finding
            .limitations
            .iter()
            .any(|note| note.contains("build script")),
        "the receipt must state that the build script is outside the scan",
    )
}

/// Fixture 5 (negative control 4): alias and package identity stay distinct.
/// The code references the alias; the finding keeps the registry name.
#[test]
fn alias_is_not_package_identity() -> Result<(), String> {
    let manifest =
        package_manifest("serde_json_alias = { package = \"serde_json\", version = \"1\" }");
    let sources = vec![source("src/lib.rs", &["use serde_json_alias::Value;", ""])];
    let receipt = inventory_unused_dependencies(&request("sample", manifest, sources, false))?;
    let finding = first_finding(&receipt)?;
    require(
        finding.disposition == UnusedDependencyDispositionV1::Used,
        "an aliased dependency referenced through its alias is Used",
    )?;
    require(
        finding.manifest_row.dependency_name == "serde_json",
        "the finding's dependency name must stay the registry package name",
    )?;
    require(
        finding.manifest_row.alias.as_deref() == Some("serde_json_alias"),
        "the alias must be carried as the manifest table key",
    )?;
    require(
        finding
            .evidence
            .iter()
            .any(|entry| entry.contains("serde_json_alias")),
        "the evidence must carry the alias identifier",
    )
}

/// Fixture 6 (negative control 5): one product's use does not retain
/// another package's dependency — per-package receipts never share
/// evidence.
#[test]
fn one_package_use_does_not_retain_another() -> Result<(), String> {
    let keeper = request(
        "keeper",
        package_manifest("shared_tool = \"1\""),
        vec![source(
            "src/lib.rs",
            &["pub fn run() {", "    shared_tool::go();", "}"],
        )],
        false,
    );
    let other = request(
        "other",
        package_manifest("shared_tool = \"1\""),
        vec![source("src/lib.rs", &["pub fn quiet() {}"])],
        false,
    );
    let receipts = inventory_packages(&[keeper, other]);
    require(receipts.len() == 2, "one receipt per package request")?;
    let keeper_receipt = receipts
        .iter()
        .find(|receipt| receipt.package_name == "keeper")
        .ok_or_else(|| "the keeper receipt is missing".to_string())?;
    let keeper_finding = first_finding(keeper_receipt)?;
    require(
        keeper_finding.disposition == UnusedDependencyDispositionV1::Used,
        "the referencing package's row is Used",
    )?;
    let other_receipt = receipts
        .iter()
        .find(|receipt| receipt.package_name == "other")
        .ok_or_else(|| "the other receipt is missing".to_string())?;
    let other_finding = first_finding(other_receipt)?;
    require(
        other_finding.disposition == UnusedDependencyDispositionV1::ApparentlyUnused,
        "the other package's identical row stays ApparentlyUnused",
    )?;
    require(
        other_finding
            .evidence
            .iter()
            .all(|entry| !entry.contains("shared_tool::")),
        "no evidence from the first package may leak into the second receipt",
    )
}

/// Fixture 7: a reference only inside a macro_rules body is still a
/// reference. The composition treats textual path evidence conservatively
/// as use, so this row never downgrades to Unsupported.
#[test]
fn macro_only_reference_is_supported_conservatively() -> Result<(), String> {
    let manifest = package_manifest("serde_json = \"1\"");
    let sources = vec![source(
        "src/lib.rs",
        &[
            "macro_rules! emit {",
            "    ($payload:expr) => {",
            "        serde_json::to_string($payload)",
            "    };",
            "}",
        ],
    )];
    let receipt = inventory_unused_dependencies(&request("sample", manifest, sources, false))?;
    let finding = first_finding(&receipt)?;
    require(
        finding.disposition == UnusedDependencyDispositionV1::Used,
        "textual path evidence inside a macro body is conservatively Used",
    )?;
    require(
        finding
            .evidence
            .iter()
            .any(|entry| entry.starts_with("src/lib.rs:3: serde_json")),
        "the evidence must carry the exact file, line, and identifier",
    )
}

/// Fixture 8 (negative control 8): analyzer success with zero inspected
/// packages is never clean — the empty receipt is pinned InstrumentFailure.
#[test]
fn zero_packages_is_instrument_failure() -> Result<(), String> {
    let receipt = empty_receipt("orphan");
    require(
        receipt.instrument_posture == UnusedDependencyInstrumentPostureV1::InstrumentFailure,
        "zero inspected packages must be InstrumentFailure",
    )?;
    require(
        receipt.packages_inspected == 0,
        "zero packages were inspected",
    )?;
    require(receipt.findings.is_empty(), "no findings may be rendered")?;
    require(
        receipt.package_name == "orphan",
        "the zero-inspection receipt keeps the package identity",
    )
}

/// Fixture 9 (negative control 1, target cousin): an unreferenced
/// target-specific row is ApparentlyUnused with the target spec carried.
#[test]
fn unreferenced_target_specific_dependency_is_apparently_unused() -> Result<(), String> {
    let manifest = lines(&[
        "[package]",
        "name = \"with-target\"",
        "version = \"0.1.0\"",
        "",
        "[target.'cfg(windows)'.dependencies]",
        "winapi = \"0.3\"",
    ]);
    let sources = vec![source("src/lib.rs", &["pub fn portable() {}"])];
    let receipt = inventory_unused_dependencies(&request("with-target", manifest, sources, false))?;
    let finding = first_finding(&receipt)?;
    require(
        finding.disposition == UnusedDependencyDispositionV1::ApparentlyUnused,
        "an unreferenced target-specific row is advisory ApparentlyUnused",
    )?;
    require(
        finding.manifest_row.class == UnusedDependencyDependencyClassV1::TargetSpecific,
        "the row keeps the TargetSpecific class",
    )?;
    require(
        finding.manifest_row.target.as_deref() == Some("cfg(windows)"),
        "the target spec must be carried on the row",
    )?;
    require(
        finding
            .limitations
            .iter()
            .any(|note| note.contains("declared target")),
        "the target-config limitation must travel with the finding",
    )
}

/// Fixture 10 (negative control 7): a malformed manifest is an
/// InstrumentFailure and nothing renders clean.
#[test]
fn malformed_manifest_is_instrument_failure() -> Result<(), String> {
    let manifest = lines(&[
        "[package]",
        "name = \"broken\"",
        "[dependencies",
        "serde_json = \"1\"",
    ]);
    let receipt = inventory_unused_dependencies(&request("broken", manifest, Vec::new(), false))?;
    require(
        receipt.instrument_posture == UnusedDependencyInstrumentPostureV1::InstrumentFailure,
        "a malformed manifest must be InstrumentFailure",
    )?;
    require(
        receipt.packages_inspected == 1,
        "the package was still presented",
    )?;
    require(
        receipt
            .findings
            .iter()
            .all(|finding| finding.disposition == UnusedDependencyDispositionV1::InstrumentFailure),
        "every row of a failed manifest is a failure description",
    )?;
    require(
        receipt
            .findings
            .iter()
            .all(|finding| !finding.limitations.is_empty()),
        "failure rows must name the failure in their limitations",
    )?;
    require(
        receipt.findings.iter().any(|finding| {
            finding
                .evidence
                .iter()
                .any(|entry| entry.contains("manifest_parse_error"))
        }),
        "the parse failure must be recorded on the receipt rows",
    )
}

/// Fixture 11 (negative control 6): exception validation demands a
/// controlling issue, ordered dates, and a non-transferable claim boundary.
#[test]
fn transitional_disposition_requires_controlling_issue() -> Result<(), String> {
    let mut exception = UnusedDependencyExceptionV1 {
        package_name: "sample".to_string(),
        manifest_dependency_name: "serde_json".to_string(),
        class: UnusedDependencyDependencyClassV1::Normal,
        target: None,
        features_selected: Vec::new(),
        owner: "release-eng".to_string(),
        reason: "retained for the transitional extraction shim".to_string(),
        use_evidence_or_limitation: "src/shim.rs:12: serde_json".to_string(),
        controlling_issue: "#2607".to_string(),
        created: "2026-08-30".to_string(),
        review_after: "2026-09-30".to_string(),
        expiry: Some("2026-10-31".to_string()),
        selected_configuration_ids: vec!["sample.default".to_string()],
        claim_boundary: "one package's exception never retains another; this row-scoped \
             retention binds package sample and row serde_json only"
            .to_string(),
    };
    require(
        validate_exception(&exception).is_ok(),
        "a fully populated exception must validate",
    )?;

    exception.controlling_issue = String::new();
    require(
        validate_exception(&exception).is_err(),
        "an empty controlling issue must be rejected",
    )?;
    exception.controlling_issue = "2607".to_string();
    require(
        validate_exception(&exception).is_err(),
        "a controlling issue without the '#' reference must be rejected",
    )?;
    exception.controlling_issue = "#2607".to_string();

    exception.expiry = Some("2026-09-29".to_string());
    require(
        validate_exception(&exception).is_err(),
        "expiry before review_after must be rejected",
    )?;
    exception.expiry = Some("2026-10-31".to_string());

    exception.review_after = "2026-08-29".to_string();
    require(
        validate_exception(&exception).is_err(),
        "review_after before created must be rejected",
    )?;
    exception.review_after = "2026-09-30".to_string();

    exception.owner = String::new();
    require(
        validate_exception(&exception).is_err(),
        "an empty owner must be rejected",
    )?;
    exception.owner = "release-eng".to_string();

    exception.selected_configuration_ids = Vec::new();
    require(
        validate_exception(&exception).is_err(),
        "an empty configuration selection must be rejected",
    )?;
    exception.selected_configuration_ids = vec!["sample.default".to_string()];

    exception.claim_boundary = "retains the dependency wherever it appears".to_string();
    require(
        validate_exception(&exception).is_err(),
        "a claim boundary without the non-transferability phrase must be rejected",
    )
}

/// Fixture 12: the receipt renders with its schema identity and round-trips
/// through serde preserving finding order.
#[test]
fn receipt_schema_renders_and_round_trips() -> Result<(), String> {
    let manifest = lines(&[
        "[package]",
        "name = \"roundtrip\"",
        "version = \"0.1.0\"",
        "",
        "[dependencies]",
        "tempfile = \"3\"",
        "serde_json = \"1\"",
    ]);
    let sources = vec![source("src/lib.rs", &["pub fn nothing() {}"])];
    let receipt = inventory_unused_dependencies(&request("roundtrip", manifest, sources, false))?;
    require(receipt.findings.len() == 2, "both rows produce findings")?;
    require(
        receipt
            .findings
            .windows(2)
            .all(|pair| matches!(pair, [left, right] if left.manifest_row.dependency_name <= right.manifest_row.dependency_name)),
        "findings are deterministically ordered by dependency name",
    )?;
    let rendered =
        render_unused_dependency_receipt_v1(&receipt).map_err(|error| error.to_string())?;
    require(
        rendered.contains(UNUSED_DEPENDENCY_RECEIPT_V1_SCHEMA_ID),
        "the rendering must carry the schema id",
    )?;
    require(
        rendered.contains("\"roundtrip\""),
        "the rendering must carry the package name",
    )?;
    require(
        rendered.contains("bounded homegrown composition"),
        "the rendering must carry the analyzer identity",
    )?;
    let parsed: UnusedDependencyReceiptV1 =
        serde_json::from_str(&rendered).map_err(|error| error.to_string())?;
    require(
        parsed == receipt,
        "the receipt must round-trip through serde unchanged",
    )?;
    let first = parsed
        .findings
        .first()
        .ok_or_else(|| "round-trip lost the first finding".to_string())?;
    require(
        first.disposition == UnusedDependencyDispositionV1::ApparentlyUnused
            && first.manifest_row.dependency_name == "serde_json",
        "the round-trip must preserve finding order and content",
    )
}

/// Fixture 13 (negative control 11): broad workspace-wide ignores are
/// structurally inexpressible. The exception serializes to exactly the
/// row-scoped key set; no field can carry a package set.
#[test]
fn wide_workspace_ignore_is_inexpressible() -> Result<(), String> {
    let exception = UnusedDependencyExceptionV1 {
        package_name: "sample".to_string(),
        manifest_dependency_name: "serde_json".to_string(),
        class: UnusedDependencyDependencyClassV1::Normal,
        target: None,
        features_selected: vec!["std".to_string()],
        owner: "release-eng".to_string(),
        reason: "row-scoped retention".to_string(),
        use_evidence_or_limitation: "src/lib.rs:1: serde_json".to_string(),
        controlling_issue: "#3909".to_string(),
        created: "2026-08-30".to_string(),
        review_after: "2026-09-30".to_string(),
        expiry: None,
        selected_configuration_ids: vec!["sample.default".to_string()],
        claim_boundary: "one package's exception never retains another".to_string(),
    };
    let value = serde_json::to_value(&exception).map_err(|error| error.to_string())?;
    let object = value
        .as_object()
        .ok_or_else(|| "an exception must serialize to a JSON object".to_string())?;
    let expected_keys = [
        "package_name",
        "manifest_dependency_name",
        "class",
        "target",
        "features_selected",
        "owner",
        "reason",
        "use_evidence_or_limitation",
        "controlling_issue",
        "created",
        "review_after",
        "expiry",
        "selected_configuration_ids",
        "claim_boundary",
    ];
    require(
        object.len() == expected_keys.len(),
        "the exception carries exactly the row-scoped fields",
    )?;
    require(
        object
            .keys()
            .all(|key| expected_keys.iter().any(|candidate| candidate == key)),
        "every serialized key must be one of the row-scoped fields",
    )?;
    require(
        object
            .keys()
            .all(|key| !key.contains("package_set") && !key.contains("workspace")),
        "no serialized key may carry a package set or workspace scope",
    )?;
    require(
        object
            .get("package_name")
            .and_then(serde_json::Value::as_str)
            == Some("sample"),
        "exactly one package identity is bound",
    )
}

/// Closed vocabulary observer: disposition, class, and posture labels plus
/// their snake_case serde names are pinned so downstream consumers cannot
/// drift.
#[test]
fn closed_vocabularies_serialize_snake_case() -> Result<(), String> {
    let samples = [
        (
            UnusedDependencyDispositionV1::ApparentlyUnused,
            "apparently_unused",
        ),
        (
            UnusedDependencyDispositionV1::ConditionallyUsed,
            "conditionally_used",
        ),
        (
            UnusedDependencyDispositionV1::BuildOrGeneratedUse,
            "build_or_generated_use",
        ),
        (
            UnusedDependencyDispositionV1::DevFixtureUse,
            "dev_fixture_use",
        ),
        (
            UnusedDependencyDispositionV1::TransitionalUse,
            "transitional_use",
        ),
        (
            UnusedDependencyDispositionV1::ExplicitException,
            "explicit_exception",
        ),
        (UnusedDependencyDispositionV1::Unsupported, "unsupported"),
        (
            UnusedDependencyDispositionV1::InstrumentFailure,
            "instrument_failure",
        ),
    ];
    for (disposition, label) in samples {
        let serialized = serde_json::to_string(&disposition).map_err(|error| error.to_string())?;
        require(
            disposition.as_str() == label,
            "the disposition label must match the variant name",
        )?;
        require(
            serialized == format!("\"{label}\""),
            "the disposition must serialize as snake_case",
        )?;
    }
    let class = serde_json::to_string(&UnusedDependencyDependencyClassV1::TargetSpecific)
        .map_err(|error| error.to_string())?;
    require(
        class == "\"target_specific\"",
        "the dependency class must serialize as snake_case",
    )?;
    let posture = serde_json::to_string(&UnusedDependencyInstrumentPostureV1::InstrumentFailure)
        .map_err(|error| error.to_string())?;
    require(
        posture == "\"instrument_failure\"",
        "the posture must serialize as snake_case",
    )
}

/// Advisory law: an empty scanned set over declared dependencies stays a
/// Complete advisory receipt with absence evidence, never an
/// InstrumentFailure.
#[test]
fn zero_source_inputs_stay_advisory() -> Result<(), String> {
    let manifest = package_manifest("serde_json = \"1\"");
    let receipt = inventory_unused_dependencies(&request("sample", manifest, Vec::new(), false))?;
    require(
        receipt.instrument_posture == UnusedDependencyInstrumentPostureV1::Complete,
        "declared dependencies with zero scanned inputs stay advisory Complete",
    )?;
    let finding = first_finding(&receipt)?;
    require(
        finding.disposition == UnusedDependencyDispositionV1::ApparentlyUnused,
        "the row is advisory ApparentlyUnused over the empty scanned set",
    )?;
    require(
        finding
            .evidence
            .iter()
            .any(|entry| entry == "no_source_inputs_supplied"),
        "the evidence must name the empty scanned set",
    )
}

/// Negative control 7: a non-optional row whose only evidence is `dep:`
/// feature activators is a manifest shape Cargo rejects and the composition
/// cannot attribute — it must render Unsupported with non-empty
/// limitations, never clean or used (pins the disposition so the contract
/// update law protects it).
#[test]
fn non_optional_dep_activator_only_is_unsupported() -> Result<(), String> {
    let manifest = lines(&[
        "[package]",
        "name = \"sample\"",
        "version = \"0.1.0\"",
        "",
        "[dependencies]",
        "serde_json = \"1\"",
        "",
        "[features]",
        "payload = [\"dep:serde_json\"]",
    ]);
    let sources = vec![source("src/lib.rs", &["pub fn nothing() {}"])];
    let receipt = inventory_unused_dependencies(&request("sample", manifest, sources, false))?;
    let finding = first_finding(&receipt)?;
    require(
        finding.disposition == UnusedDependencyDispositionV1::Unsupported,
        "a non-optional dep:-activated row must render Unsupported",
    )?;
    require(
        !finding.limitations.is_empty(),
        "Unsupported findings must carry non-empty limitations",
    )?;
    require(
        finding
            .evidence
            .iter()
            .any(|entry| entry.contains("dep:serde_json")),
        "the evidence must name the dep: activator",
    )
}

/// The false-unused class this contract exists to prevent: a dependency
/// package whose crate root is renamed via `[lib] name` is referenced under
/// the lib spelling, so a caller-supplied lib identity must make the row
/// classify Used (the intent-compiler -> intent_engine and
/// proof-orchestrator -> proof_engine remaps hid live use before this was
/// modeled).
#[test]
fn lib_identity_supplied_dependency_is_used_via_lib_name() -> Result<(), String> {
    let manifest = package_manifest("intent-compiler = \"0.1.0\"");
    let sources = vec![source(
        "src/lib.rs",
        &["use intent_engine::compile;", "pub fn nothing() {}"],
    )];
    let mut probe = request("sample", manifest, sources, false);
    probe.dependency_lib_identities = vec![UnusedDependencyLibIdentityV1 {
        package_name: "intent-compiler".to_string(),
        lib_name: "intent_engine".to_string(),
    }];
    let receipt = inventory_unused_dependencies(&probe)?;
    let finding = first_finding(&receipt)?;
    require(
        finding.disposition == UnusedDependencyDispositionV1::Used,
        "a lib-renamed dependency referenced by its lib name must classify Used",
    )?;
    require(
        finding
            .evidence
            .iter()
            .any(|entry| entry.contains("intent_engine")),
        "the evidence must name the matched lib identity",
    )
}

/// The declared residual limitation: without a supplied identity, references
/// under a renamed lib spelling are invisible and the row renders advisory
/// ApparentlyUnused with the absence limitations attached — never Used, and
/// never more than an advisory candidate.
#[test]
fn lib_identity_absent_renaming_stays_advisory_absence() -> Result<(), String> {
    let manifest = package_manifest("intent-compiler = \"0.1.0\"");
    let sources = vec![source(
        "src/lib.rs",
        &["use intent_engine::compile;", "pub fn nothing() {}"],
    )];
    let receipt = inventory_unused_dependencies(&request("sample", manifest, sources, false))?;
    let finding = first_finding(&receipt)?;
    require(
        finding.disposition == UnusedDependencyDispositionV1::ApparentlyUnused,
        "without a supplied lib identity the renaming reference is invisible and          the row stays advisory ApparentlyUnused",
    )?;
    require(
        !finding.limitations.is_empty(),
        "absence findings must carry the composition limitations",
    )
}

/// Receipt validation (the PR D selective-guard prerequisite): a Complete
/// receipt passes, and an Unsupported row without limitations is rejected —
/// an unexplained unsupported row can never be review-visible.
#[test]
fn validate_receipt_rejects_unsupported_without_limitations() -> Result<(), String> {
    let manifest = package_manifest("serde_json = \"1\"");
    let sources = vec![source("src/lib.rs", &["pub fn nothing() {}"])];
    let receipt = inventory_unused_dependencies(&request("sample", manifest, sources, false))?;
    validate_receipt(&receipt)?;
    let mut unexplained = receipt;
    let finding = unexplained
        .findings
        .first_mut()
        .ok_or_else(|| "receipt lost its finding".to_string())?;
    finding.disposition = UnusedDependencyDispositionV1::Unsupported;
    finding.limitations = Vec::new();
    require(
        validate_receipt(&unexplained).is_err(),
        "an Unsupported row without limitations must fail receipt validation",
    )
}

/// Posture agreement: a receipt claiming InstrumentFailure posture while
/// carrying classified rows is rejected — failed inspections must not
/// render classified rows.
#[test]
fn validate_receipt_rejects_classified_rows_in_failed_receipts() -> Result<(), String> {
    let manifest = package_manifest("serde_json = \"1\"");
    let sources = vec![source("src/lib.rs", &["pub fn nothing() {}"])];
    let mut receipt = inventory_unused_dependencies(&request("sample", manifest, sources, false))?;
    require(
        validate_receipt(&receipt).is_ok(),
        "a live advisory receipt must pass receipt validation",
    )?;
    receipt.instrument_posture = UnusedDependencyInstrumentPostureV1::InstrumentFailure;
    require(
        validate_receipt(&receipt).is_err(),
        "a receipt claiming InstrumentFailure posture with classified rows must fail          receipt validation",
    )
}

/// Scan-completeness law: an empty input set is an incomplete scan, and a
/// Complete-posture receipt over supplied inputs is complete. The selective
/// guard grades only the former's absence findings as review-visible noise
/// and the latter's as enforcement-grade.
#[test]
fn receipt_scan_is_complete_tracks_the_supplied_inputs() -> Result<(), String> {
    let manifest = package_manifest("serde_json = \"1\"");
    let with_inputs = inventory_unused_dependencies(&request(
        "sample",
        manifest.clone(),
        vec![source("src/lib.rs", &["pub fn nothing() {}"])],
        false,
    ))?;
    require(
        receipt_scan_is_complete(&with_inputs),
        "a receipt over supplied inputs is a complete scan",
    )?;
    let without_inputs =
        inventory_unused_dependencies(&request("sample", manifest, Vec::new(), false))?;
    require(
        !receipt_scan_is_complete(&without_inputs),
        "a receipt over zero inputs is an incomplete scan",
    )
}

/// Date-impossibility tightening: validate_exception rejects a review_after
/// naming a day the month does not have, while real dates keep validating.
#[test]
fn validate_exception_rejects_impossible_calendar_dates() -> Result<(), String> {
    let mut exception = UnusedDependencyExceptionV1 {
        package_name: "sample".to_string(),
        manifest_dependency_name: "serde_json".to_string(),
        class: UnusedDependencyDependencyClassV1::Normal,
        target: None,
        features_selected: Vec::new(),
        owner: "release-eng".to_string(),
        reason: "retained for the transitional extraction shim".to_string(),
        use_evidence_or_limitation: "src/shim.rs:12: serde_json".to_string(),
        controlling_issue: "#2607".to_string(),
        created: "2026-01-31".to_string(),
        review_after: "2026-02-28".to_string(),
        expiry: None,
        selected_configuration_ids: vec!["sample.default".to_string()],
        claim_boundary: "one package's exception never retains another; this row-scoped              retention binds package sample and row serde_json only"
            .to_string(),
    };
    validate_exception(&exception)?;
    exception.review_after = "2026-02-30".to_string();
    require(
        validate_exception(&exception).is_err(),
        "review_after 2026-02-30 must be rejected: February 2026 has 28 days",
    )?;
    exception.review_after = "2024-02-29".to_string();
    require(
        validate_exception(&exception).is_err(),
        "review_after 2024-02-29 must be rejected as before created 2026-01-31 even in          a leap year",
    )
}
