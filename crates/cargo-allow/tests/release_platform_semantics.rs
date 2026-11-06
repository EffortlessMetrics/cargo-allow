//! #2276 — one normalized semantic determinism fixture for Linux, Windows,
//! and macOS.
//!
//! The oracle runs the real cargo-allow pipeline over a checked-in fixture
//! tree (`tests/fixtures/cross_platform_semantics`) — inventory, Rust and
//! non-Rust scanning, policy load, matching, and receipt/report rendering —
//! and compares a canonical projection against the single golden below, so
//! every `test-core-platforms` runner must produce byte-identical semantic
//! output. Inputs that cannot be materialized identically on every host
//! (slash/backslash spellings, Windows drive and UNC roots, Unicode NFC/NFD
//! aliases, case-pair filenames) are exercised at the owning `allow-core`
//! seams instead of behind `cfg`-gated passing aggregates.
//!
//! ## Projection contract
//!
//! The projection retains the schema/tool generation, repository-relative
//! normalized paths, finding kinds/families/structural identity inputs,
//! allow IDs and match outcomes, inventory completeness and selected counts,
//! posture counts, policy and inventory semantic digests, canonical ordering,
//! and the semantic fields of the receipt/JSON/Markdown renders the pipeline
//! produces for the fixture scenario.
//!
//! ## Documented exclusions (non-semantic only)
//!
//! - absolute scan root, runner OS/architecture, wall-clock timestamps (the
//!   evaluation `as_of` date is pinned below), git SHA, run id, and the
//!   native executable path are environment, not semantics;
//! - line/column spans are documented hints, not identity — the product's own
//!   [`finding_identity_key`] already excludes them;
//! - raw path bytes: the projection pins `normalize_path` output only, which
//!   is the product's identity spelling;
//! - deleted-tracked inventory paths are covered by the retained
//!   `inventory_deleted_tracked` integration test rather than duplicated
//!   here (the fixture tree cannot contain a deleted tracked path without
//!   mutating the worktree).
//!
//! Nothing here normalizes case, separators, or Unicode beyond what the
//! product itself does; the case-sensitivity seam and the negative controls
//! pin exactly that boundary.

use allow_core::{
    Finding, FindingKind, MatchOutcome, MatchStatus, STRUCTURAL_IDENTITY_SCHEMA_ID, SimpleDate,
    StructuralIdentity, finding_identity_key, glob_matches, json_escape, normalize_path,
    normalize_snippet, sha256_v1_bytes, stable_hash_hex,
};
use allow_files::scan_files;
use allow_inventory::{InventoryCompleteness, InventoryOptions, InventorySource, inventory};
use allow_match::{CheckMode, evaluate};
use allow_policy::load_policy;
use allow_report::{render_json, render_markdown, render_receipt};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

const FIXTURE_DIR: &str = "tests/fixtures/cross_platform_semantics";
const PROJECTION_SCHEMA: &str = "cargo-allow.semantic-result-projection.v1";
const AS_OF: &str = "2026-11-05";
const CHECK_MODE: CheckMode = CheckMode::NoNew;

/// The single canonical semantic golden shared across platforms. Every
/// supported runner must reproduce these bytes from the checked-in fixture
/// tree; the hosted `test-core-platforms` matrix turns that into the
/// cross-platform equality proof (#2265).
const GOLDEN: &str = r##"schema: cargo-allow.semantic-result-projection.v1
structural_identity_schema: cargo-allow.structural-identity.v1
tool_generation: cargo-allow 0.2.0
as_of: 2026-11-05
check_mode: no-new
[inventory]
source: git_tracked
completeness: complete
empty_git_tracked: false
deleted_tracked_count: 0
skipped_paths: 0
files_scanned: 5
file: README.md
file: policy.toml
file: tree/alpha.rs
file: tree/alpha_crlf.rs
file: tree/nested/gamma.rs
policy_digest: sha256:v1:b6f982b1a03511f5b02b221446807f1e6e0129c812762bcbecaf97d4204e97b9
inventory_digest: sha256:v1:17a8030bc3b8cfaf59e17ed1b466fa1efc352cb2b64f6fdc372bebda3eec4e00
[findings]
count: 5
  kind: non_rust_file
  family: configuration
  path: policy.toml
  ast_kind: tracked_file
  container: -
  module: -
  symbol: policy.toml
  callee: -
  macro_name: -
  lint: -
  receiver_fingerprint: -
  target_fingerprint: toml
  normalized_snippet_hash: -
  identity_key: kind:13:non_rust_file|family:13:configuration|path:11:policy.toml|language:4:file|crate_name:0:|module:0:|container:0:|ast_kind:12:tracked_file|symbol:11:policy.toml|callee:0:|macro_name:0:|lint:0:|receiver_fingerprint:0:|target_fingerprint:4:toml|normalized_snippet_hash:0:
  outcome_status: matched
  outcome_allow_id: allow-xp-policy
  outcome_score: 200
  kind: panic
  family: unreachable
  path: tree/nested/gamma.rs
  ast_kind: macro_call
  container: guard
  module: -
  symbol: -
  callee: -
  macro_name: unreachable
  lint: -
  receiver_fingerprint: -
  target_fingerprint: unreachable
  normalized_snippet_hash: fnv1a64:9059f89f8c5ac8ee
  identity_key: kind:5:panic|family:11:unreachable|path:20:tree/nested/gamma.rs|language:4:rust|crate_name:0:|module:0:|container:5:guard|ast_kind:10:macro_call|symbol:0:|callee:0:|macro_name:11:unreachable|lint:0:|receiver_fingerprint:0:|target_fingerprint:11:unreachable|normalized_snippet_hash:24:fnv1a64:9059f89f8c5ac8ee
  outcome_status: new
  outcome_allow_id: -
  outcome_score: 0
  kind: panic
  family: expect
  path: tree/nested/gamma.rs
  ast_kind: method_call
  container: chosen
  module: -
  symbol: -
  callee: expect
  macro_name: -
  lint: -
  receiver_fingerprint: param:0
  target_fingerprint: -
  normalized_snippet_hash: fnv1a64:216308e073e2c641
  identity_key: kind:5:panic|family:6:expect|path:20:tree/nested/gamma.rs|language:4:rust|crate_name:0:|module:0:|container:6:chosen|ast_kind:11:method_call|symbol:0:|callee:6:expect|macro_name:0:|lint:0:|receiver_fingerprint:7:param:0|target_fingerprint:0:|normalized_snippet_hash:24:fnv1a64:216308e073e2c641
  outcome_status: expired
  outcome_allow_id: allow-xp-gamma-expect
  outcome_score: 300
  kind: panic
  family: unwrap
  path: tree/alpha.rs
  ast_kind: method_call
  container: load
  module: -
  symbol: -
  callee: unwrap
  macro_name: -
  lint: -
  receiver_fingerprint: param:0
  target_fingerprint: -
  normalized_snippet_hash: fnv1a64:3e906e44fd8d34ac
  identity_key: kind:5:panic|family:6:unwrap|path:13:tree/alpha.rs|language:4:rust|crate_name:0:|module:0:|container:4:load|ast_kind:11:method_call|symbol:0:|callee:6:unwrap|macro_name:0:|lint:0:|receiver_fingerprint:7:param:0|target_fingerprint:0:|normalized_snippet_hash:24:fnv1a64:3e906e44fd8d34ac
  outcome_status: matched
  outcome_allow_id: allow-xp-alpha
  outcome_score: 300
  kind: panic
  family: unwrap
  path: tree/alpha_crlf.rs
  ast_kind: method_call
  container: load
  module: -
  symbol: -
  callee: unwrap
  macro_name: -
  lint: -
  receiver_fingerprint: param:0
  target_fingerprint: -
  normalized_snippet_hash: fnv1a64:3e906e44fd8d34ac
  identity_key: kind:5:panic|family:6:unwrap|path:18:tree/alpha_crlf.rs|language:4:rust|crate_name:0:|module:0:|container:4:load|ast_kind:11:method_call|symbol:0:|callee:6:unwrap|macro_name:0:|lint:0:|receiver_fingerprint:7:param:0|target_fingerprint:0:|normalized_snippet_hash:24:fnv1a64:3e906e44fd8d34ac
  outcome_status: review_due
  outcome_allow_id: allow-xp-alpha-crlf
  outcome_score: 300
[posture]
matched: 2
new: 1
stale: 0
expired: 1
review_due: 1
location_drift: 0
ambiguous: 0
invalid_selector: 0
missing_required_field: 0
evidence_missing: 0
baseline_debt: 0
[receipt]
schema_version: 1
schema_id: "cargo-allow.receipt.v1"
tool: "cargo-allow"
command: "check"
status: "failed"
failed: true
counts: {"ambiguous":0,"baseline_debt":0,"evidence_missing":0,"expired":1,"invalid_selector":0,"location_drift":0,"matched":2,"missing_required_field":0,"new":1,"review_due":1,"stale":0}
[report_json]
summary: {"ambiguous":0,"baseline_debt":0,"evidence_missing":0,"expired":1,"findings":5,"invalid_selector":0,"location_drift":0,"matched":2,"missing_required_field":0,"new":1,"outcomes":5,"review_due":1,"stale":0}
rendered_finding: {"ast_kind":"method_call","container":"load","family":"unwrap","kind":"panic","line":2,"path":"tree/alpha.rs"}
rendered_finding: {"ast_kind":"method_call","container":"load","family":"unwrap","kind":"panic","line":2,"path":"tree/alpha_crlf.rs"}
rendered_finding: {"ast_kind":"method_call","container":"chosen","family":"expect","kind":"panic","line":2,"path":"tree/nested/gamma.rs"}
rendered_finding: {"ast_kind":"macro_call","container":"guard","family":"unreachable","kind":"panic","line":7,"path":"tree/nested/gamma.rs"}
rendered_finding: {"ast_kind":"tracked_file","container":null,"family":"configuration","kind":"non_rust_file","line":1,"path":"policy.toml"}
[report_markdown]
markdown_line: **Result:** failed (1 new, 1 expired)
markdown_line: Findings scanned: `5`
markdown_line: | `matched` | 2 |
markdown_line: | `new` | 1 |
markdown_line: | `expired` | 1 |
markdown_line: | `review_due` | 1 |
markdown_line: | `location_drift` | 0 |
markdown_line: | `stale` | 0 |
markdown_line: | `ambiguous` | 0 |
markdown_line: | `invalid_selector` | 0 |
markdown_line: | `evidence_missing` | 0 |
markdown_line: | `missing_required_field` | 0 |
markdown_line: | `baseline_debt` | 0 |
markdown_line: | `configuration` | 1 |
markdown_line: | `matched` | `configuration` | `policy.toml` |
"##;

struct FixtureWorld {
    inventory_files: Vec<String>,
    inventory_source: InventorySource,
    inventory_completeness: InventoryCompleteness,
    empty_git_tracked: bool,
    deleted_tracked_count: usize,
    skipped_paths: usize,
    findings: Vec<Finding>,
    outcomes: Vec<MatchOutcome>,
    policy_digest: String,
}

fn require_result<T, E: std::fmt::Debug>(result: Result<T, E>, what: &str) -> T {
    result.unwrap_or_else(|err| std::panic::panic_any(format!("{what}: {err:?}")))
}

fn require_present<T>(value: Option<T>, what: &str) -> T {
    value.unwrap_or_else(|| std::panic::panic_any(format!("{what}: absent")))
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_DIR)
}

fn inventory_source_label(source: InventorySource) -> &'static str {
    match source {
        InventorySource::GitTracked => "git_tracked",
        InventorySource::GitIndexStagedCandidate => "git_index_staged_candidate",
        InventorySource::FilesystemFallback => "filesystem_fallback",
        InventorySource::FilesystemIncludeUntracked => "filesystem_include_untracked",
    }
}

fn optional_field(value: Option<&str>) -> String {
    match value {
        Some(text) => text.to_string(),
        None => "-".to_string(),
    }
}

fn load_world() -> FixtureWorld {
    let root = fixture_root();
    // The fixture tree contains no ignorable paths, so the scan runs with
    // empty ignore scopes and the inventory classifies as Complete — the
    // projection pins that completeness fact (negative control 3 mutates it).
    let options = InventoryOptions {
        ignored: Vec::new(),
        generated: Vec::new(),
        include_untracked: false,
    };
    let listed = require_result(inventory(&root, &options), "fixture inventory");
    let inventory_files = listed.files.iter().map(normalize_path).collect::<Vec<_>>();
    let policy_path = root.join("policy.toml");
    let cfg = require_result(load_policy(&policy_path), "fixture policy load");
    // Portable policy digest: hash the EOL-normalized text, never the raw
    // checkout bytes, so a platform line-ending conversion cannot forge a
    // semantic difference.
    let policy_text = require_result(fs::read_to_string(&policy_path), "fixture policy read");
    let policy_normalized = policy_text.replace("\r\n", "\n");
    let policy_digest = sha256_v1_bytes(policy_normalized.as_bytes());
    let rust_scan = require_result(
        allow_rust::scan_rust_files(&root, &listed.files),
        "fixture rust scan",
    );
    let mut findings = rust_scan.findings;
    findings.extend(scan_files(&listed.files));
    let as_of = require_present(SimpleDate::parse(AS_OF), "pinned as_of date");
    let outcomes = evaluate(&cfg, &findings, CHECK_MODE, as_of);
    FixtureWorld {
        skipped_paths: listed.skipped_paths.len(),
        inventory_source: listed.source,
        inventory_completeness: listed.completeness,
        empty_git_tracked: listed.empty_git_tracked,
        deleted_tracked_count: listed.deleted_tracked.len(),
        inventory_files,
        findings,
        outcomes,
        policy_digest,
    }
}

fn finding_projection(finding: &Finding, outcome: &MatchOutcome) -> String {
    let identity = &finding.identity;
    let mut out = String::new();
    out.push_str(&format!("  kind: {}\n", finding.kind.as_str()));
    out.push_str(&format!(
        "  family: {}\n",
        optional_field(finding.family.as_deref())
    ));
    out.push_str(&format!("  path: {}\n", normalize_path(&finding.path)));
    out.push_str(&format!("  ast_kind: {}\n", identity.ast_kind));
    out.push_str(&format!(
        "  container: {}\n",
        optional_field(identity.container.as_deref())
    ));
    out.push_str(&format!(
        "  module: {}\n",
        optional_field(identity.module.as_deref())
    ));
    out.push_str(&format!(
        "  symbol: {}\n",
        optional_field(identity.symbol.as_deref())
    ));
    out.push_str(&format!(
        "  callee: {}\n",
        optional_field(identity.callee.as_deref())
    ));
    out.push_str(&format!(
        "  macro_name: {}\n",
        optional_field(identity.macro_name.as_deref())
    ));
    out.push_str(&format!(
        "  lint: {}\n",
        optional_field(identity.lint.as_deref())
    ));
    out.push_str(&format!(
        "  receiver_fingerprint: {}\n",
        optional_field(identity.receiver_fingerprint.as_deref())
    ));
    out.push_str(&format!(
        "  target_fingerprint: {}\n",
        optional_field(identity.target_fingerprint.as_deref())
    ));
    out.push_str(&format!(
        "  normalized_snippet_hash: {}\n",
        optional_field(identity.normalized_snippet_hash.as_deref())
    ));
    out.push_str(&format!(
        "  identity_key: {}\n",
        finding_identity_key(finding)
    ));
    out.push_str(&format!("  outcome_status: {}\n", outcome.status.as_str()));
    out.push_str(&format!(
        "  outcome_allow_id: {}\n",
        optional_field(outcome.allow_id.as_deref())
    ));
    out.push_str(&format!("  outcome_score: {}\n", outcome.score));
    out
}

/// Canonical projection of one fixture evaluation. The findings section is
/// pinned in canonical (identity-key sorted) order; the JSON render section
/// additionally pins the pipeline's own emission order through the parsed
/// `findings` array, so a nondeterministic pipeline order fails the oracle
/// even though the canonical section would still match.
fn projection_with(
    world: &FixtureWorld,
    findings: &[Finding],
    outcomes: &[MatchOutcome],
) -> String {
    let mut out = String::new();
    out.push_str(&format!("schema: {PROJECTION_SCHEMA}\n"));
    out.push_str(&format!(
        "structural_identity_schema: {STRUCTURAL_IDENTITY_SCHEMA_ID}\n"
    ));
    out.push_str("tool_generation: cargo-allow 0.2.0\n");
    out.push_str(&format!("as_of: {AS_OF}\n"));
    out.push_str(&format!("check_mode: {}\n", CHECK_MODE.as_str()));
    out.push_str("[inventory]\n");
    out.push_str(&format!(
        "source: {}\n",
        inventory_source_label(world.inventory_source)
    ));
    out.push_str(&format!(
        "completeness: {}\n",
        world.inventory_completeness.as_str()
    ));
    out.push_str(&format!("empty_git_tracked: {}\n", world.empty_git_tracked));
    out.push_str(&format!(
        "deleted_tracked_count: {}\n",
        world.deleted_tracked_count
    ));
    out.push_str(&format!("skipped_paths: {}\n", world.skipped_paths));
    let mut files = world.inventory_files.clone();
    files.sort();
    out.push_str(&format!("files_scanned: {}\n", files.len()));
    for file in &files {
        out.push_str(&format!("file: {file}\n"));
    }
    let mut inventory_digest_input = files.join("\n");
    inventory_digest_input.push('\n');
    out.push_str(&format!("policy_digest: {}\n", world.policy_digest));
    out.push_str(&format!(
        "inventory_digest: {}\n",
        sha256_v1_bytes(inventory_digest_input.as_bytes())
    ));
    out.push_str("[findings]\n");
    let mut indexed = findings
        .iter()
        .enumerate()
        .map(|(index, finding)| (finding_identity_key(finding), index))
        .collect::<Vec<_>>();
    indexed.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
    out.push_str(&format!("count: {}\n", findings.len()));
    for (_, index) in &indexed {
        let finding = require_present(
            findings.get(*index),
            "canonical finding index (sorted from input slice)",
        );
        let outcome = require_present(
            outcomes.get(*index),
            "canonical outcome index (sorted from input slice)",
        );
        out.push_str(&finding_projection(finding, outcome));
    }
    out.push_str("[posture]\n");
    for status in MatchStatus::ALL {
        let count = outcomes
            .iter()
            .filter(|outcome| outcome.status == *status)
            .count();
        out.push_str(&format!("{}: {count}\n", status.as_str()));
    }
    out.push_str("[receipt]\n");
    let receipt_text = render_receipt("check", outcomes, true);
    let receipt: Value = require_result(serde_json::from_str(&receipt_text), "receipt parse");
    for field in [
        "schema_version",
        "schema_id",
        "tool",
        "command",
        "status",
        "failed",
    ] {
        let value = require_present(receipt.get(field), "receipt field");
        out.push_str(&format!("{field}: {value}\n"));
    }
    let counts = require_present(receipt.get("counts"), "receipt counts");
    out.push_str(&format!("counts: {}\n", counts));
    out.push_str("[report_json]\n");
    let report_text = render_json("check", findings, outcomes, true);
    let report: Value = require_result(serde_json::from_str(&report_text), "report json parse");
    let summary = require_present(report.get("summary"), "report summary");
    out.push_str(&format!("summary: {summary}\n"));
    let rendered_findings = require_present(report.get("findings"), "report findings array");
    let rendered_array =
        require_present(rendered_findings.as_array(), "report findings array items");
    for rendered in rendered_array {
        out.push_str(&format!("rendered_finding: {rendered}\n"));
    }
    out.push_str("[report_markdown]\n");
    let markdown = render_markdown("check", findings, outcomes, true);
    for line in markdown.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("**Result:**")
            || trimmed.starts_with("Findings scanned:")
            || trimmed.starts_with("| `")
        {
            out.push_str(&format!("markdown_line: {trimmed}\n"));
        }
    }
    out
}

fn canonical_projection(world: &FixtureWorld) -> String {
    projection_with(world, &world.findings, &world.outcomes)
}

fn synthetic_finding(path: &str, container: &str, callee: &str, line: &str) -> Finding {
    let mut identity = StructuralIdentity::new("rust", "method_call");
    identity.container = Some(container.to_string());
    identity.callee = Some(callee.to_string());
    identity.normalized_snippet_hash = Some(stable_hash_hex(&normalize_snippet(line)));
    Finding {
        kind: FindingKind::Panic,
        family: Some(callee.to_string()),
        path: PathBuf::from(path),
        span: None,
        identity,
        message: "fixture synthetic finding".to_string(),
        ledger: None,
    }
}

#[test]
fn cross_platform_semantics_pipeline_projection_matches_golden() {
    let world = load_world();
    let projection = canonical_projection(&world);
    if projection != GOLDEN {
        std::panic::panic_any(format!(
            "semantic projection drifted from the canonical golden (#2276).\n\
             If the drift is an intended product change, refresh GOLDEN with the\n\
             projection below after review.\n--- actual projection ---\n{projection}"
        ));
    }
}

#[test]
fn cross_platform_semantics_inventory_is_real_git_inventory() {
    let world = load_world();
    assert_eq!(world.inventory_source, InventorySource::GitTracked);
    assert_eq!(
        world.inventory_completeness,
        InventoryCompleteness::Complete
    );
    assert!(!world.empty_git_tracked);
    assert_eq!(world.deleted_tracked_count, 0);
    assert_eq!(world.skipped_paths, 0);
    let mut files = world.inventory_files.clone();
    files.sort();
    assert_eq!(
        files,
        vec![
            "README.md".to_string(),
            "policy.toml".to_string(),
            "tree/alpha.rs".to_string(),
            "tree/alpha_crlf.rs".to_string(),
            "tree/nested/gamma.rs".to_string(),
        ]
    );
}

#[test]
fn cross_platform_semantics_newline_handling_is_identity_invariant() {
    let world = load_world();
    let lf = world
        .findings
        .iter()
        .find(|finding| normalize_path(&finding.path) == "tree/alpha.rs");
    let crlf = world
        .findings
        .iter()
        .find(|finding| normalize_path(&finding.path) == "tree/alpha_crlf.rs");
    let lf_finding = require_present(lf, "LF fixture finding");
    let crlf_finding = require_present(crlf, "CRLF fixture finding");
    let lf_hash = require_present(
        lf_finding.identity.normalized_snippet_hash.as_deref(),
        "LF snippet hash",
    );
    let crlf_hash = require_present(
        crlf_finding.identity.normalized_snippet_hash.as_deref(),
        "CRLF snippet hash",
    );
    assert_eq!(lf_hash, crlf_hash);
    // Owning seam: the snippet normalizer itself folds line endings.
    assert_eq!(
        normalize_snippet("let value = input.unwrap();\r"),
        normalize_snippet("let value = input.unwrap();")
    );
}

#[test]
fn cross_platform_semantics_path_spelling_and_unicode_seams() {
    // Slash and backslash spelling fold to one normalized identity path.
    assert_eq!(normalize_path("tree\\alpha.rs"), "tree/alpha.rs");
    assert_eq!(normalize_path("tree/nested/..\\alpha.rs"), "tree/alpha.rs");
    // Pure Windows drive and verbatim/UNC normalization inputs keep their
    // documented identity forms.
    assert_eq!(
        normalize_path("C:\\repo\\tree\\alpha.rs"),
        "C:/repo/tree/alpha.rs"
    );
    assert_eq!(
        normalize_path("\\\\?\\C:\\repo\\tree\\alpha.rs"),
        "C:/repo/tree/alpha.rs"
    );
    assert_eq!(
        normalize_path("\\\\server\\share\\tree\\alpha.rs"),
        "/server/share/tree/alpha.rs"
    );
    // Unicode NFC and NFD spellings of one name produce one identity (#1823).
    assert_eq!(
        normalize_path("tree/caf\u{e9}.rs"),
        normalize_path("tree/cafe\u{301}.rs")
    );
    // Policy glob matching runs through the same normalization, so a
    // backslash-spelled path matches a forward-slash glob.
    assert!(glob_matches("tree/*.rs", Path::new("tree\\alpha.rs")));
    assert!(!glob_matches("tree/*.rs", Path::new("other/alpha.rs")));
}

#[test]
fn cross_platform_semantics_case_sensitivity_seam() {
    // Case-distinct paths are distinct identities: the oracle forbids
    // over-normalization that would erase a case-sensitive distinction.
    let upper = synthetic_finding("tree/Alpha.rs", "load", "unwrap", "value.unwrap()");
    let lower = synthetic_finding("tree/alpha.rs", "load", "unwrap", "value.unwrap()");
    assert_ne!(finding_identity_key(&upper), finding_identity_key(&lower));
    assert_ne!(
        stable_hash_hex(&normalize_snippet("let value = A.unwrap();")),
        stable_hash_hex(&normalize_snippet("let value = a.unwrap();"))
    );
}

#[test]
fn cross_platform_semantics_negative_control_separator_drift() {
    // The product folds separators, so both spellings share one identity.
    let forward = synthetic_finding("tree/alpha.rs", "load", "unwrap", "value.unwrap()");
    let backslashed = synthetic_finding("tree\\alpha.rs", "load", "unwrap", "value.unwrap()");
    assert_eq!(
        finding_identity_key(&forward),
        finding_identity_key(&backslashed)
    );
    // Oracle sensitivity: an identity that carries the raw spelling (what a
    // separator-handling regression would produce) must differ from the
    // product key, so the golden would fail.
    let product_key = finding_identity_key(&backslashed);
    let normalized = normalize_path("tree\\alpha.rs");
    let product_segment = format!("path:{}:{normalized}", normalized.len());
    let regressed_segment = format!("path:{}:tree\\alpha.rs", "tree\\alpha.rs".len());
    assert!(product_key.contains(&product_segment));
    let regressed_key = product_key.replacen(&product_segment, &regressed_segment, 1);
    assert_ne!(product_key, regressed_key);
}

#[test]
fn cross_platform_semantics_negative_control_nondeterministic_order() {
    let world = load_world();
    let canonical = canonical_projection(&world);
    let mut reversed_findings = world.findings.clone();
    reversed_findings.reverse();
    let mut reversed_outcomes = world.outcomes.clone();
    reversed_outcomes.reverse();
    let drifted = projection_with(&world, &reversed_findings, &reversed_outcomes);
    assert_ne!(canonical, drifted);
}

#[test]
fn cross_platform_semantics_negative_control_dropped_inventory_completeness() {
    let world = load_world();
    let canonical = canonical_projection(&world);
    // Simulate the projection losing the completeness fact: the oracle must
    // fail because the golden pins it.
    let drifted = canonical.replacen(
        &format!("completeness: {}", world.inventory_completeness.as_str()),
        "completeness: unknown",
        1,
    );
    assert_ne!(canonical, drifted);
}

#[test]
fn cross_platform_semantics_negative_control_over_normalization() {
    let world = load_world();
    let canonical = canonical_projection(&world);
    // Simulate an oracle that case-folds paths: the golden pins the
    // case-preserving spelling, so the folded projection must differ.
    let folded = canonical
        .lines()
        .map(|line| {
            if line.starts_with("  path: ") {
                line.to_ascii_lowercase()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_ne!(canonical, folded);
    // And the case-distinct identities must not collide under the product.
    let upper = synthetic_finding("tree/Alpha.rs", "load", "unwrap", "value.unwrap()");
    let lower = synthetic_finding("tree/alpha.rs", "load", "unwrap", "value.unwrap()");
    assert_ne!(finding_identity_key(&upper), finding_identity_key(&lower));
}

#[test]
fn cross_platform_semantics_negative_control_windows_path_escaping() {
    // The product escapes a Windows-shaped path so the JSON render parses
    // back to the exact string.
    let windows_shaped = "C:\\repo\\tree\\alpha.rs";
    let rendered = format!("{{\"path\": \"{}\"}}", json_escape(windows_shaped));
    let parsed: Value = require_result(serde_json::from_str(&rendered), "escaped json parse");
    assert_eq!(
        require_present(parsed.get("path"), "escaped json path").as_str(),
        Some(windows_shaped)
    );
    // Oracle sensitivity: corrupted escaping (raw backslashes) must fail.
    let corrupted = format!("{{\"path\": \"{windows_shaped}\"}}");
    let corrupted_parse: Result<Value, serde_json::Error> = serde_json::from_str(&corrupted);
    assert!(corrupted_parse.is_err());
}
