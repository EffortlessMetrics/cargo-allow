//! Strict readback projections of the existing A-family artifact generations.
//! These projections reuse native pure summary adapters; they never scan or
//! resolve policy. Fields omitted by a live artifact are not manufactured.

use super::{BinaryContext, CaseContext, CaseSpec, FixtureSpec, decode};
use crate::core_command_summary::CoreCommandSummaryV1;
use allow_core::{MatchOutcome, MatchStatus};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Inventory {
    scope: String,
    scanner: String,
    source: String,
    root: String,
    files_scanned: Option<usize>,
    completeness: String,
    #[serde(default)]
    empty_git_tracked: bool,
    source_identity: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Skipped {
    read_failed_or_unsupported: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Scanner {
    completeness: String,
    files_considered: usize,
    files_scanned: usize,
    files_skipped: usize,
    files_with_parse_errors: usize,
    skipped_by_reason: Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AdoptionDetail {
    schema_id: String,
    schema_version: u32,
    tool: String,
    command: String,
    claim_boundary: Vec<String>,
    scanner_limitations: Vec<String>,
    inventory: Inventory,
    plan: allow_report::CoreAdoptionPlanV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Root {
    path: String,
    discovery: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Provenance {
    source: String,
    precedence: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DoctorConfig {
    found: bool,
    path: Option<String>,
    schema_version: Option<String>,
    policy: Option<String>,
    owner: Option<String>,
    status: Option<String>,
    provenance: Option<Provenance>,
    valid: Option<bool>,
    diagnostic: Option<String>,
    suggested_init_command: Option<String>,
    broken_evidence_links: Option<usize>,
    weak_evidence_references: Option<usize>,
    #[serde(default)]
    deleted_tracked_files: usize,
    git_inventory_error: Option<String>,
    #[serde(default)]
    skipped_paths: usize,
    #[serde(default)]
    submodule_paths: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DoctorRust {
    files_considered: usize,
    files_scanned: usize,
    files_skipped: usize,
    files_with_parse_errors: usize,
    skipped_by_reason: Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DoctorScanner {
    completeness: String,
    rust: DoctorRust,
}

// The pinned first-family fixtures have no custom file families, federation
// ledgers or provider payloads. Nonempty extensions require a later supported
// case/readback projection, not an arbitrary JSON value accepted as evidence.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
enum OutsideFirstFamilyFixture {}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FileFamilies {
    configured: Vec<OutsideFirstFamilyFixture>,
    conflicts: Vec<OutsideFirstFamilyFixture>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Federation {
    found: bool,
    path: Option<String>,
    valid: Option<bool>,
    provenance: Option<String>,
    configured_ledgers: Option<Vec<OutsideFirstFamilyFixture>>,
    diagnostics: Option<Vec<OutsideFirstFamilyFixture>>,
    divergences: Option<Vec<OutsideFirstFamilyFixture>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Queue {
    signal: String,
    label: Option<String>,
    route_kind: Option<String>,
    item_kind: Option<String>,
    worklist_status: Option<String>,
    worklist_filter: Option<String>,
    count: usize,
    command: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DoctorDetail {
    schema_id: String,
    schema_version: u32,
    tool: String,
    command: String,
    claim_boundary: Vec<String>,
    scanner_limitations: Vec<String>,
    inventory: Inventory,
    root: Root,
    config: DoctorConfig,
    scanner: DoctorScanner,
    file_families: FileFamilies,
    federation: Federation,
    evidence_repair_queues: Vec<Queue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Outcome {
    status: String,
    allow_id: Option<String>,
    #[serde(default)]
    candidate_ids: Vec<String>,
    finding_index: Option<usize>,
    score: u32,
    message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Finding {
    kind: String,
    family: Option<String>,
    path: String,
    line: Option<usize>,
    container: Option<String>,
    source_package: Option<String>,
    ast_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct InventoryRow {
    kind: String,
    family: Option<String>,
    label: Option<String>,
    total: usize,
    matched: usize,
    new: usize,
    review_items: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SourceInventory {
    findings: usize,
    by_kind: Vec<InventoryRow>,
    by_family: Vec<InventoryRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ReportDetail {
    schema_id: String,
    schema_version: u32,
    tool: String,
    command: String,
    status: String,
    failed: bool,
    claim_boundary: Vec<String>,
    scanner_limitations: Vec<String>,
    inventory: Inventory,
    rust_scanner: Scanner,
    summary: BTreeMap<String, usize>,
    trend: BTreeMap<String, usize>,
    audit_remediation_roadmap: Option<Vec<Queue>>,
    evidence_repair_queues: Vec<Queue>,
    source_inventory: Option<SourceInventory>,
    outcomes: Vec<Outcome>,
    findings: Vec<Finding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ReceiptFederation {
    federation_version: String,
    ledger_contributors: Vec<OutsideFirstFamilyFixture>,
    precedence_applied: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema_id: String,
    schema_version: u32,
    tool: String,
    tool_version: String,
    command: String,
    status: String,
    failed: bool,
    claim_boundary: Vec<String>,
    scanner_limitations: Vec<String>,
    inventory: Inventory,
    mode: String,
    enforcement: String,
    policy_config: Option<String>,
    policy_digest: Option<String>,
    git_sha: Option<String>,
    started_at: String,
    run_id: String,
    lifecycle_posture: Option<String>,
    lane_posture: Option<BTreeMap<String, String>>,
    federation: Option<ReceiptFederation>,
    counts: BTreeMap<String, usize>,
    advisory: BTreeMap<String, usize>,
    evidence_repair_queues: Vec<Queue>,
    source_inventory: Option<SourceInventory>,
    diagnostic: Option<String>,
}

pub(super) struct DetailProjection {
    pub(super) summary: CoreCommandSummaryV1,
    pub(super) exit_code: i32,
    pub(super) binding_gaps: Vec<String>,
}

pub(super) fn project(
    spec: &CaseSpec,
    fixture: &FixtureSpec,
    context: &CaseContext,
    binary: &BinaryContext,
    bytes: &[u8],
    receipt: Option<&[u8]>,
) -> Result<DetailProjection, String> {
    match spec.command.as_str() {
        "adopt" => adoption(spec, fixture, context, binary, bytes),
        "doctor" => doctor(fixture, context, bytes),
        "audit" | "check" => report(spec, fixture, context, binary, bytes, receipt),
        _ => Err("command is outside the native first-family reader".to_string()),
    }
}

fn adoption(
    spec: &CaseSpec,
    fixture: &FixtureSpec,
    context: &CaseContext,
    binary: &BinaryContext,
    bytes: &[u8],
) -> Result<DetailProjection, String> {
    let detail: AdoptionDetail = decode(bytes)?;
    header(
        &detail.schema_id,
        detail.schema_version,
        &detail.tool,
        &detail.command,
        "cargo-allow.core-adoption-plan.v1",
        "adopt",
        &detail.claim_boundary,
        &detail.scanner_limitations,
    )?;
    inventory(
        &detail.inventory,
        "<repository-root>",
        fixture,
        if fixture.parse_errors > 0 {
            "partial"
        } else {
            "scoped"
        },
    )?;
    let plan = &detail.plan;
    // The older nested plan type predates deny_unknown_fields. Exact typed
    // roundtrip comparison rejects discarded extension keys at that boundary.
    let raw: serde_json::Value = decode(bytes)?;
    let typed_plan = serde_json::to_value(plan).map_err(|error| error.to_string())?;
    need(
        raw.get("plan") == Some(&typed_plan),
        "unknown or missing nested adoption-plan fields",
    )?;
    need(
        plan.schema_id == detail.schema_id
            && plan.schema_version == 1
            && plan.tool_version == binary.tool_version
            && plan.executable_identity == binary.digest
            && plan.selected_root == "<repository-root>",
        "adoption schema/binary/generation/root identity mismatch",
    )?;
    let expected_state = if spec.scenario == "invalid_policy" {
        allow_report::PolicyState::Invalid
    } else if fixture.policy.is_some() {
        allow_report::PolicyState::Valid
    } else {
        allow_report::PolicyState::Absent
    };
    need(
        plan.policy.state == expected_state && plan.policy.path == context.config_path,
        "adoption policy state/path contradicts the selected fixture",
    )?;
    // Invalid policy stops before the scan; the plan retains that failed setup
    // observation rather than claiming zero findings from a completed scan.
    let expected_findings = if expected_state == allow_report::PolicyState::Invalid {
        0
    } else {
        fixture.source_findings + usize::from(fixture.policy.is_some())
    };
    need(
        plan.policy.total_findings == expected_findings
            && plan.policy.new_unreceipted_findings == fixture.source_findings,
        "adoption findings contradict the literal fixture",
    )?;
    if expected_state == allow_report::PolicyState::Valid {
        need(
            plan.policy.digest == context.policy_digest,
            "adoption policy digest mismatch",
        )?;
    } else {
        need(
            plan.policy.digest.is_none(),
            "absent/invalid policy falsely has a consumed-policy digest",
        )?;
    }
    let mut paths = vec!["src/lib.rs".to_string()];
    if fixture.policy.is_some() {
        paths.push("policy/allow.toml".to_string());
    }
    let identity = crate::adoption::inventory_identity_from_facts(
        plan.policy.digest.as_deref(),
        Some(("git_tracked", "scoped", paths)),
    );
    need(
        plan.repository_identity == identity
            && matches!(plan.channel.as_str(), "source-preview" | "published")
            && plan.policy.schema_version.as_deref()
                == (expected_state == allow_report::PolicyState::Valid).then_some("0.1")
            && plan.policy.stale_entries == 0
            && plan.policy.location_drift_entries == 0
            && plan.policy.broken_evidence_entries == 0
            && plan.policy.review_due_entries == 0
            && plan.policy.expired_entries == 0
            && plan.policy.occurrence_headroom_entries == 0
            && !plan.policy.mirror_divergence,
        "adoption inventory/configuration/lifecycle identity contradicts the bounded fixture",
    )?;
    let expected_complete = if spec.scenario == "partial_inventory" {
        allow_report::InventoryCompleteness::Partial
    } else {
        allow_report::InventoryCompleteness::Complete
    };
    need(
        plan.inventory.mode == allow_report::InventoryMode::GitTracked
            && plan.inventory.completeness == expected_complete,
        "adoption inventory contradicts the selected fixture",
    )?;
    let expected = allow_report::recommend_core_adoption_plan(&allow_report::AdoptionFacts {
        tool_version: binary.tool_version.clone(),
        repository_identity: plan.repository_identity.clone(),
        selected_root: "<repository-root>".to_string(),
        channel: plan.channel.clone(),
        executable_identity: binary.digest.clone(),
        inventory: plan.inventory.clone(),
        policy: plan.policy.clone(),
        policy_config_diagnostic: (expected_state == allow_report::PolicyState::Invalid)
            .then(|| "retained invalid-policy diagnostic presence".to_string()),
        unsupported_repository_state: false,
        instrument_failure: None,
        strict_gate_requested: false,
        ci_guidance_completed: false,
    });
    need(
        expected == *plan,
        "adoption actions/disposition/effects contradict the native plan projection",
    )?;
    let summary = crate::core_command_summary::core_command_summary_from_adoption_plan(plan)?;
    let exit_code = match plan.bootstrap_disposition {
        allow_report::BootstrapDisposition::PartialInventory
        | allow_report::BootstrapDisposition::InvalidPolicy
        | allow_report::BootstrapDisposition::UnsupportedRepositoryState
        | allow_report::BootstrapDisposition::InstrumentFailure => 1,
        _ => 0,
    };
    Ok(DetailProjection {
        summary,
        exit_code,
        binding_gaps: Vec::new(),
    })
}

fn doctor(
    fixture: &FixtureSpec,
    context: &CaseContext,
    bytes: &[u8],
) -> Result<DetailProjection, String> {
    let detail: DoctorDetail = decode(bytes)?;
    header(
        &detail.schema_id,
        detail.schema_version,
        &detail.tool,
        &detail.command,
        "cargo-allow.doctor.v1",
        "doctor",
        &detail.claim_boundary,
        &detail.scanner_limitations,
    )?;
    let displayed_root = allow_core::normalize_path(Path::new(&context.root));
    inventory(&detail.inventory, &displayed_root, fixture, "scoped")?;
    need(
        detail.root.path == displayed_root && detail.root.discovery == "explicit_root",
        "doctor root/discovery mismatch",
    )?;
    let config_path = context
        .config_path
        .as_ref()
        .map(|path| allow_core::normalize_path(&Path::new(&context.root).join(path)));
    need(
        detail.config.found == fixture.policy.is_some() && detail.config.path == config_path,
        "doctor selected configuration mismatch",
    )?;
    need(
        detail.config.valid == fixture.policy_valid,
        "doctor configuration validity contradicts the selected fixture",
    )?;
    if fixture.policy_valid == Some(true) {
        need(
            detail.config.schema_version.as_deref() == Some("0.1")
                && detail.config.policy.as_deref() == Some("cargo-allow")
                && detail.config.owner.as_deref() == Some("core/policy")
                && detail.config.status.as_deref() == Some("active")
                && detail.config.diagnostic.is_none()
                && detail.config.broken_evidence_links == Some(0)
                && detail.config.weak_evidence_references == Some(0),
            "doctor policy metadata/evidence health contradicts the literal fixture",
        )?;
    } else {
        need(
            detail.config.schema_version.is_none()
                && detail.config.policy.is_none()
                && detail.config.owner.is_none()
                && detail.config.status.is_none(),
            "absent/invalid policy acquired valid configuration metadata",
        )?;
    }
    need(
        detail.config.deleted_tracked_files == 0
            && detail.config.git_inventory_error.is_none()
            && detail.config.skipped_paths == 0
            && detail.config.submodule_paths == 0,
        "doctor reports unsupported source-inventory defects in the bounded fixture",
    )?;
    need(
        !detail.federation.found
            && detail.federation.path.is_none()
            && detail.federation.valid.is_none(),
        "unexpected federation in the bounded fixture",
    )?;
    let scanner = Scanner {
        completeness: detail.scanner.completeness.clone(),
        files_considered: detail.scanner.rust.files_considered,
        files_scanned: detail.scanner.rust.files_scanned,
        files_skipped: detail.scanner.rust.files_skipped,
        files_with_parse_errors: detail.scanner.rust.files_with_parse_errors,
        skipped_by_reason: detail.scanner.rust.skipped_by_reason.clone(),
    };
    scanner_facts(&scanner, fixture)?;
    need(
        scanner.completeness
            == if fixture.parse_errors > 0 {
                "partial"
            } else {
                "complete"
            },
        "doctor scanner completeness contradicts its typed counters",
    )?;
    let facts = inventory_facts(&detail.inventory, None, detail.config.deleted_tracked_files);
    let root = Path::new(&context.root);
    let source_context = crate::reporting::SourceTreeReportContext::new(root, facts);
    let report = allow_report::DoctorReport {
        source_tree_root: &detail.root.path,
        root_discovery: &detail.root.discovery,
        config_path: detail.config.path.as_deref(),
        config_schema_version: detail.config.schema_version.as_deref(),
        config_policy: detail.config.policy.as_deref(),
        config_owner: detail.config.owner.as_deref(),
        config_status: detail.config.status.as_deref(),
        config_provenance: detail.config.provenance.as_ref().map(|value| {
            allow_report::ConfigProvenanceSummary {
                source: &value.source,
                precedence: value.precedence.as_deref(),
            }
        }),
        config_valid: detail.config.valid,
        config_diagnostic: detail.config.diagnostic.as_deref(),
        broken_evidence_links: detail.config.broken_evidence_links,
        weak_evidence_references: detail.config.weak_evidence_references,
        inventory_source: &detail.inventory.source,
        inventory_completeness: &detail.inventory.completeness,
        files_scanned: detail.inventory.files_scanned.unwrap_or(0),
        empty_git_tracked: detail.inventory.empty_git_tracked,
        deleted_tracked_files: detail.config.deleted_tracked_files,
        git_inventory_error: detail.config.git_inventory_error.as_deref(),
        skipped_paths: detail.config.skipped_paths,
        submodule_paths: detail.config.submodule_paths,
        rust_scanner_completeness: &detail.scanner.completeness,
        rust_files_considered: scanner.files_considered,
        rust_files_scanned: scanner.files_scanned,
        rust_files_skipped: scanner.files_skipped,
        rust_files_with_parse_errors: scanner.files_with_parse_errors,
        rust_files_skipped_by_read_or_unsupported: scanner
            .skipped_by_reason
            .read_failed_or_unsupported,
        federation_config_path: None,
        federation_config_found: false,
        federation_config_valid: None,
        configured_ledgers: None,
        federation_diagnostics: None,
        federation_divergences: None,
        file_family_rules: &[],
        file_family_conflicts: &[],
    };
    let native_detail: serde_json::Value =
        decode(allow_report::render_doctor_json(report).as_bytes())?;
    let retained_detail: serde_json::Value = decode(bytes)?;
    need(
        native_detail == retained_detail,
        "doctor detail contradicts its native typed rendering",
    )?;
    let summary = crate::doctor::doctor_summary(
        report,
        root,
        &source_context,
        crate::doctor::DoctorSetupFacts {
            config_present: detail.config.found,
            config_valid: detail.config.valid,
            config_diagnostic: detail.config.diagnostic.as_deref(),
            broken_evidence_links: detail.config.broken_evidence_links,
            weak_evidence_references: detail.config.weak_evidence_references,
        },
    )
    .map_err(|error| error.to_string())?;
    let exit_code = i32::from(
        fixture.require_clean
            && (scanner.files_skipped > 0
                || scanner.files_with_parse_errors > 0
                || detail.config.valid != Some(true)),
    );
    Ok(DetailProjection {
        summary,
        exit_code,
        binding_gaps: Vec::new(),
    })
}

fn report(
    spec: &CaseSpec,
    fixture: &FixtureSpec,
    context: &CaseContext,
    binary: &BinaryContext,
    bytes: &[u8],
    receipt: Option<&[u8]>,
) -> Result<DetailProjection, String> {
    let detail: ReportDetail = decode(bytes)?;
    header(
        &detail.schema_id,
        detail.schema_version,
        &detail.tool,
        &detail.command,
        "cargo-allow.report.v1",
        &spec.command,
        &detail.claim_boundary,
        &detail.scanner_limitations,
    )?;
    inventory(
        &detail.inventory,
        &allow_core::normalize_path(Path::new(&context.root)),
        fixture,
        if fixture.parse_errors > 0 {
            "partial"
        } else {
            "scoped"
        },
    )?;
    scanner_facts(&detail.rust_scanner, fixture)?;
    need(
        detail.rust_scanner.completeness
            == if fixture.parse_errors > 0 {
                "partial"
            } else {
                "scoped"
            },
        "report scanner completeness contradicts its typed inventory/counters",
    )?;
    need(
        detail.status == if detail.failed { "failed" } else { "passed" },
        "report status contradicts failed",
    )?;
    need(
        detail.failed == (spec.command == "check" && fixture.check_fails),
        "report gate posture contradicts the selected fixture",
    )?;
    if spec.command == "audit" {
        need(
            !detail.failed && receipt.is_none(),
            "audit is informational and does not own a check receipt",
        )?;
    }
    let outcomes = detail
        .outcomes
        .iter()
        .map(|outcome| {
            let status = MatchStatus::ALL
                .iter()
                .copied()
                .find(|status| status.as_str() == outcome.status)
                .ok_or_else(|| format!("unknown outcome status: {}", outcome.status))?;
            if outcome
                .finding_index
                .is_some_and(|index| index >= detail.findings.len())
            {
                return Err("outcome refers to a nonexistent finding".to_string());
            }
            Ok(MatchOutcome {
                status,
                allow_id: outcome.allow_id.clone(),
                candidate_ids: outcome.candidate_ids.clone(),
                finding_index: outcome.finding_index,
                score: outcome.score,
                message: outcome.message.clone(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    need(
        outcomes.len() == detail.findings.len(),
        "bounded fixture needs exactly one outcome per finding",
    )?;
    let mut covered = std::collections::BTreeSet::new();
    for outcome in &outcomes {
        let index = outcome
            .finding_index
            .ok_or("fixture outcome lacks its finding")?;
        let finding = detail
            .findings
            .get(index)
            .ok_or("fixture outcome has a foreign finding")?;
        need(
            covered.insert(index),
            "duplicate outcome for the same fixture finding",
        )?;
        let expected = if finding.path == "policy/allow.toml" {
            outcome.status == MatchStatus::Matched
                && outcome.allow_id.as_deref() == Some("allow-0001")
                && outcome.candidate_ids == ["allow-0001"]
                && outcome.score == 200
        } else {
            outcome.status == MatchStatus::New
                && outcome.allow_id.is_none()
                && outcome.candidate_ids.is_empty()
                && outcome.score == 0
        };
        need(
            expected,
            "typed matching outcome contradicts the selected literal fixture",
        )?;
    }
    let counts = allow_report::Summary::from_outcomes(&outcomes);
    for status in MatchStatus::ALL {
        need(
            detail.summary.get(status.as_str()) == Some(&counts.count(*status)),
            "report status counts contradict typed outcomes",
        )?;
    }
    need(
        detail.summary.get("findings") == Some(&detail.findings.len())
            && detail.summary.get("outcomes") == Some(&outcomes.len()),
        "report finding/outcome totals mismatch",
    )?;
    need(
        detail.summary.keys().all(|key| {
            matches!(
                key.as_str(),
                "findings" | "outcomes" | "policy_baseline_debt"
            ) || key == "matched"
                || allow_report::AdvisoryClass::parse_field_name(key).is_some()
        }),
        "unknown report count field",
    )?;
    for finding in &detail.findings {
        let expected = match finding.path.as_str() {
            "src/lib.rs" => {
                finding.kind == "panic"
                    && finding.family.as_deref() == Some("unwrap")
                    && finding.ast_kind == "method_call"
                    && finding.container.as_deref() == Some("fixture")
            }
            "policy/allow.toml" => {
                finding.kind == "non_rust_file"
                    && finding.family.as_deref() == Some("configuration")
                    && finding.ast_kind == "tracked_file"
                    && finding.container.is_none()
            }
            _ => false,
        };
        need(
            expected && finding.line == Some(1) && finding.source_package.is_none(),
            "reported finding facts contradict the exact literal fixture",
        )?;
    }
    need(
        detail
            .findings
            .iter()
            .filter(|finding| finding.path == "src/lib.rs")
            .count()
            == fixture.source_findings
            && detail
                .findings
                .iter()
                .filter(|finding| finding.path == "policy/allow.toml")
                .count()
                == usize::from(fixture.policy.is_some()),
        "report findings contradict the exact retained source fixture",
    )?;
    // Current report JSON omits raw policy-missing-evidence counts when they do
    // not exceed EvidenceMissing outcomes. Do not invent those omitted facts.
    need(
        counts.count(MatchStatus::EvidenceMissing) == 0
            || detail.summary.contains_key("policy_missing_evidence"),
        "report lacks the raw policy-evidence count required for summary readback",
    )?;
    let facts = inventory_facts(&detail.inventory, Some(&detail.rust_scanner), 0);
    let root = Path::new(&context.root);
    let source_context = crate::reporting::SourceTreeReportContext::new(root, facts);
    let evidence = crate::EvidenceReportSummary {
        policy_missing_evidence_entries: count(&detail.summary, "policy_missing_evidence"),
        broken_evidence_links: count(&detail.summary, "broken_evidence_links"),
        weak_evidence_references: count(&detail.summary, "weak_evidence_references"),
        occurrence_headroom_entries: count(&detail.trend, "occurrence_headroom"),
    };
    let baseline_debt =
        count(&detail.summary, "policy_baseline_debt").max(counts.count(MatchStatus::BaselineDebt));
    let mut report_context = source_context.report(Some(baseline_debt));
    evidence.apply_to(&mut report_context);
    let expected_trend = allow_report::AdvisoryClass::receipt_fields(&counts, report_context)
        .into_iter()
        .map(|(class, count)| (class.field_name().to_string(), count))
        .collect::<BTreeMap<_, _>>();
    need(
        detail.trend == expected_trend,
        "report advisory projection contradicts typed outcomes/context",
    )?;
    validate_source_inventory(&detail, &outcomes)?;
    let advisory_count = crate::core_command_router::report_advisory_count(&outcomes, evidence);
    let text = std::str::from_utf8(bytes).map_err(|error| error.to_string())?;
    let identity = crate::core_command_router::canonical_semantic_identity(text, None)
        .map_err(|error| error.to_string())?;
    let subject = crate::core_command_router::report_subject_from_identity(
        &identity,
        facts,
        detail.inventory.source_identity.as_deref(),
    );
    let summary = crate::core_command_router::build_report_summary_from_facts(
        crate::core_command_router::ReportSummaryFacts {
            command: &spec.command,
            root,
            inventory_facts: facts,
            failed: detail.failed,
            advisory_count,
            subject,
        },
    )
    .map_err(|error| error.to_string())?;
    if spec.command == "check" {
        validate_receipt(
            receipt.ok_or("check detail lacks its requested receipt")?,
            &detail,
            context,
            binary,
            &outcomes,
            report_context,
        )?;
    }
    Ok(DetailProjection {
        summary, exit_code: i32::from(detail.failed),
        binding_gaps: vec!["detailed report finding rows omit full scanner identities; exact literal fixture snapshots are checked separately without reconstructing unreported finding bytes".to_string()],
    })
}

fn validate_receipt(
    bytes: &[u8],
    detail: &ReportDetail,
    context: &CaseContext,
    binary: &BinaryContext,
    outcomes: &[MatchOutcome],
    report_context: allow_report::ReportContext<'_>,
) -> Result<(), String> {
    let receipt: Receipt = decode(bytes)?;
    header(
        &receipt.schema_id,
        receipt.schema_version,
        &receipt.tool,
        &receipt.command,
        "cargo-allow.receipt.v1",
        "check",
        &receipt.claim_boundary,
        &receipt.scanner_limitations,
    )?;
    // Counts and repair routes have their own established receipt projection.
    // Read its pure output instead of assuming JSON report keys are identical.
    let native =
        allow_report::render_receipt_with_context("check", outcomes, detail.failed, report_context);
    let native: serde_json::Value = decode(native.as_bytes())?;
    let expected_counts: BTreeMap<String, usize> = serde_json::from_value(
        native
            .get("counts")
            .ok_or("native receipt lacks counts")?
            .clone(),
    )
    .map_err(|error| error.to_string())?;
    let expected_queues: Vec<Queue> = serde_json::from_value(
        native
            .get("evidence_repair_queues")
            .ok_or("native receipt lacks repair queues")?
            .clone(),
    )
    .map_err(|error| error.to_string())?;
    need(
        receipt.tool_version == binary.tool_version
            && receipt.mode == "no-new"
            && context.mode.as_deref() == Some("no-new")
            && receipt.policy_digest == context.policy_digest
            && receipt.git_sha.as_deref() == Some(context.fixture_commit.as_str()),
        "receipt binary/mode/policy/source identity mismatch",
    )?;
    let config = context
        .config_path
        .as_ref()
        .map(|path| allow_core::normalize_path(&Path::new(&context.root).join(path)));
    need(
        receipt.policy_config == config
            && receipt.inventory == detail.inventory
            && receipt.failed == detail.failed
            && receipt.status == detail.status
            && receipt.counts == expected_counts
            && receipt.advisory == detail.trend
            && receipt.source_inventory == detail.source_inventory
            && receipt.evidence_repair_queues == detail.evidence_repair_queues
            && receipt.evidence_repair_queues == expected_queues
            && receipt.diagnostic.is_none(),
        "same-invocation receipt/detail semantics mismatch",
    )?;
    need(
        matches!(receipt.enforcement.as_str(), "advisory" | "enforcing")
            && !receipt.started_at.is_empty()
            && !receipt.run_id.is_empty(),
        "receipt lacks observed enforcement/run identity",
    )?;
    if context.policy_digest.is_some() {
        need(
            receipt.enforcement == "enforcing",
            "selected no-new policy receipt is not enforcing",
        )?;
    } else {
        need(
            receipt.enforcement == "advisory",
            "no-policy inspection cannot be promoted to an enforcing pass",
        )?;
    }
    Ok(())
}

pub(super) fn validate_error_receipt(
    bytes: &[u8],
    context: &CaseContext,
    binary: &BinaryContext,
    diagnostic: &str,
) -> Result<(), String> {
    let receipt: Receipt = decode(bytes)?;
    header(
        &receipt.schema_id,
        receipt.schema_version,
        &receipt.tool,
        &receipt.command,
        "cargo-allow.receipt.v1",
        "check",
        &receipt.claim_boundary,
        &receipt.scanner_limitations,
    )?;
    need(
        !receipt.started_at.is_empty() && !receipt.run_id.is_empty(),
        "error receipt lacks its observed run metadata",
    )?;
    let root = Path::new(&context.root);
    let source = crate::reporting::SourceTreeReportContext::new(
        root,
        crate::InventoryFacts::source_only(allow_inventory::InventorySource::FilesystemFallback),
    );
    let config = context
        .config_path
        .as_ref()
        .map(|path| allow_report::source_tree_path_text(&root.join(path)));
    let mut native_context = source.report(None);
    native_context.mode = Some("no-new");
    native_context.enforcement = Some("enforcing");
    native_context.policy_config = config.as_deref();
    native_context.tool_version = Some(&binary.tool_version);
    native_context.git_sha = Some(&context.fixture_commit);
    native_context.started_at = Some(&receipt.started_at);
    native_context.run_id = Some(&receipt.run_id);
    let native = allow_report::render_error_receipt(diagnostic, native_context);
    let native: serde_json::Value = decode(native.as_bytes())?;
    let retained: serde_json::Value = decode(bytes)?;
    need(
        native == retained,
        "hard-error receipt contradicts native error/detail/source/mode projection",
    )
}

fn header(
    schema: &str,
    version: u32,
    tool: &str,
    command: &str,
    expected_schema: &str,
    expected_command: &str,
    claims: &[String],
    limitations: &[String],
) -> Result<(), String> {
    need(
        schema == expected_schema
            && version == 1
            && tool == "cargo-allow"
            && command == expected_command,
        "detail schema/generation/command mismatch",
    )?;
    let expected_claims = allow_report::claim_boundary_for_schema_id(schema);
    let expected_limits = allow_report::scanner_limitations_for_schema_id(schema);
    need(
        claims
            .iter()
            .map(String::as_str)
            .eq(expected_claims.iter().copied())
            && limitations
                .iter()
                .map(String::as_str)
                .eq(expected_limits.iter().copied()),
        "detail claim boundary or scanner limitations changed",
    )
}

fn inventory(
    value: &Inventory,
    root: &str,
    fixture: &FixtureSpec,
    rendered_completeness: &str,
) -> Result<(), String> {
    need(
        value.scope == "source_tree"
            && value.scanner == "source_syntax"
            && value.source == "git_tracked"
            && value.root == root
            && value.source_identity.is_none()
            && !value.empty_git_tracked
            && value.files_scanned == Some(1 + usize::from(fixture.policy.is_some()))
            && value.completeness == rendered_completeness,
        "detail inventory subject or scope mismatch",
    )
}

fn inventory_facts(
    value: &Inventory,
    scanner: Option<&Scanner>,
    deleted: usize,
) -> crate::InventoryFacts {
    // The pinned tracked fixture has all of its paths present. Audit/check's
    // displayed inventory merges scanner coverage, but the live summary keeps
    // the underlying scoped inventory distinct. Doctor instead passes scanner
    // counters through DoctorReport, not its source context. Reconstruct those
    // existing boundaries without counting a parse failure twice.
    crate::InventoryFacts {
        source: allow_inventory::InventorySource::GitTracked,
        completeness: allow_inventory::InventoryCompleteness::Scoped,
        files_scanned: value.files_scanned,
        empty_git_tracked: value.empty_git_tracked,
        deleted_tracked: Some(deleted),
        rust_files_skipped: scanner.map_or(0, |scanner| scanner.files_skipped),
        rust_files_considered: scanner.map_or(0, |scanner| scanner.files_considered),
        rust_files_with_parse_errors: scanner.map_or(0, |scanner| scanner.files_with_parse_errors),
        policy_digest: None,
    }
}

fn scanner_facts(scanner: &Scanner, fixture: &FixtureSpec) -> Result<(), String> {
    need(
        scanner.files_considered == 1
            && scanner.files_scanned == 1
            && scanner.files_skipped == 0
            && scanner.skipped_by_reason.read_failed_or_unsupported == 0,
        "scanner counts contradict the exact one-file fixture",
    )?;
    need(
        scanner.files_with_parse_errors == fixture.parse_errors,
        "scanner partial/full coverage contradicts the exact fixture",
    )?;
    need(
        matches!(
            scanner.completeness.as_str(),
            "complete" | "scoped" | "partial"
        ),
        "unsupported scanner generation",
    )
}

fn count(values: &BTreeMap<String, usize>, key: &str) -> usize {
    values.get(key).copied().unwrap_or(0)
}

fn validate_source_inventory(
    detail: &ReportDetail,
    outcomes: &[MatchOutcome],
) -> Result<(), String> {
    let Some(inventory) = &detail.source_inventory else {
        return need(
            detail.findings.is_empty(),
            "nonempty report omitted source inventory",
        );
    };
    need(
        inventory.findings == detail.findings.len(),
        "source-inventory finding total mismatch",
    )?;
    for family in [false, true] {
        let mut expected =
            BTreeMap::<(String, Option<String>), (usize, usize, usize, usize)>::new();
        for (index, finding) in detail.findings.iter().enumerate() {
            let key = (
                finding.kind.clone(),
                if family { finding.family.clone() } else { None },
            );
            let row = expected.entry(key).or_default();
            row.0 += 1;
            for outcome in outcomes
                .iter()
                .filter(|outcome| outcome.finding_index == Some(index))
            {
                row.1 += usize::from(outcome.status == MatchStatus::Matched);
                row.2 += usize::from(outcome.status == MatchStatus::New);
                row.3 += usize::from(outcome.status != MatchStatus::Matched);
            }
        }
        let rows = if family {
            &inventory.by_family
        } else {
            &inventory.by_kind
        };
        let mut actual = BTreeMap::new();
        for row in rows {
            let label = row
                .family
                .as_ref()
                .map(|family| format!("{}.{}", row.kind, family));
            need(
                row.label == label
                    && (family || row.family.is_none())
                    && actual
                        .insert(
                            (row.kind.clone(), row.family.clone()),
                            (row.total, row.matched, row.new, row.review_items),
                        )
                        .is_none(),
                "source-inventory row label/identity is malformed or duplicated",
            )?;
        }
        need(
            actual == expected,
            "source-inventory rows contradict typed finding/outcome counts",
        )?;
    }
    Ok(())
}

fn need(condition: bool, message: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.to_string())
    }
}
