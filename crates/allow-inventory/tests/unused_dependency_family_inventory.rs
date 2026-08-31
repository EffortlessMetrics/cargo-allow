//! #3909 PR B: disposition the live unused-dependency inventory over the
//! cargo-allow family.
//!
//! This is the advisory dogfood lane for the #3909 composition: it reads the
//! real family manifests and sources from the repository, inventories every
//! package/configuration row through
//! [`inventory_packages`](allow_inventory::inventory_packages), requires a
//! `Complete` instrument posture, and pins exactly one reviewed disposition
//! per non-`Used` finding. It renders deterministic line-free JSON and
//! markdown artifacts under `docs/dogfood/receipts/` and fails on drift from
//! the checked-in copies. Nothing here removes a dependency, edits policy, or
//! wires any CI enforcement: the dispositions table is advisory evidence for
//! a separately authorized follow-up (PR C owns any removal).
//!
//! ## Routing decision (#3909 PR D)
//!
//! The issue sketched PR D as routed CI: derive affected package rows from
//! changed files and run only those on ordinary PRs, with a scheduled full
//! lane. This test supersedes that sketch, deliberately. The whole 14-package
//! inventory runs in about eight seconds inside the always-on `test` job
//! (no analyzer binary, no Cargo invocation — the composition is a pure
//! function of the repository's own files), so routing would save nothing
//! measurable while adding a job, a drift surface, and a completeness gate
//! of its own. The always-on lane is also strictly stronger than routing:
//! every pull request proves the entire family, not just the rows its
//! files touch, and the coverage law fails on any unlisted non-`Used`
//! finding, which is the no-new guard the issue asked PR D to build — now
//! with the PR D severity split below.
//!
//! ## Guard severity split (#3909 PR D)
//!
//! Enforcement grades only complete-scan absence: a non-`Used` finding over
//! supplied inputs without a disposition row fails the suite. Findings from
//! incomplete scans (evidence carries
//! [`INCOMPLETE_SCAN_EVIDENCE_MARKER`](allow_inventory::INCOMPLETE_SCAN_EVIDENCE_MARKER))
//! are review-visible in the artifacts but exempt from coverage, because
//! their absence is noise; `receipt_scan_is_complete` and `validate_receipt`
//! are the module-level primitives, and the contract fixtures pin them.
//!
//! ## Family denominator
//!
//! The family is the exact release-set package list from the
//! `.github/workflows/ci.yml` release-set clippy lane (the `-p` list on the
//! `cargo-allow release-set clippy (#3358)` step) plus the `cargo-proof`
//! product package from the #3905 shared product line: 14 packages total.
//! Those are the packages the release lane actually compiles and proof-reads
//! as one unit, so they are the denominator an unused-dependency inventory
//! must cover before any removal PR is legitimate.
//!
//! ## Configuration scope
//!
//! Each package is inspected under its default configuration. `allow-rust`,
//! `allow-files`, and `cargo-proof` carry a #3905 feature-configuration
//! matrix, and their `.default` ids are that matrix's default rows. Every
//! other family package is featureless, and the matrix law gives featureless
//! packages no rows, so their `<package>.default` ids are the featureless
//! default configurations outside the #3905 matrix. Deeper matrix rows
//! (`allow-rust.minimal-model`, `allow-rust.syntax-explicit`,
//! `allow-files.changie`, `cargo-proof.provider-*`, `cargo-proof.all-providers`)
//! are separate scopes reserved for later PRs.
//!
//! ## Probe record (tool-defect law)
//!
//! Before the dispositions table was authored, a scratch probe ran the
//! composition over all 14 real packages and printed every finding. The
//! probe found no mis-parse: dotted workspace-inherited rows
//! (`serde.workspace = true`), inline workspace-inherited rows
//! (`allow-files = { workspace = true, features = ["changie"] }`), optional
//! workspace-inherited rows, and path dependencies all resolved to the right
//! registry names, classes, and optional flags, so no analyzer repair was
//! needed. The probe also showed the optional feature dependencies
//! (`yaml-rust2` on `allow-files`, `tree-sitter` and `tree-sitter-rust` on
//! `allow-rust`) classify `Used`, not `ConditionallyUsed`: their references
//! sit inside cfg-gated module files whose `use` lines carry no `cfg(`
//! token, which the analyzer counts as direct textual evidence — a
//! conservative never-false-unused outcome, covered by the analyzer's
//! declared "cfg-gated module bodies beyond their cfg( attribute line"
//! limitation.
//!
//! ## Test-file law
//!
//! This file is itself cargo-allow scan surface: it must stay finding-free
//! (no `unwrap`/`expect`/`panic!`/`assert!`/indexing), every fs error maps
//! to a `String`, and the source walk uses an explicit stack over
//! `std::fs::read_dir` instead of recursion with indexing.

use std::fs;
use std::path::{Path, PathBuf};

use allow_inventory::{
    INCOMPLETE_SCAN_EVIDENCE_MARKER, UNUSED_DEPENDENCY_ANALYZER_IDENTITY,
    UNUSED_DEPENDENCY_CLAIM_BOUNDARY, UnusedDependencyDependencyClassV1,
    UnusedDependencyDispositionV1, UnusedDependencyFindingV1, UnusedDependencyInstrumentPostureV1,
    UnusedDependencyLibIdentityV1, UnusedDependencyReceiptV1, UnusedDependencyRequestV1,
    UnusedDependencySourceInputV1, inventory_packages, receipt_scan_is_complete, validate_receipt,
};
use serde::Serialize;

/// Schema identity of the rendered family-inventory artifact (distinct from
/// the per-package receipt schema owned by the analyzer module).
const FAMILY_INVENTORY_SCHEMA_ID: &str = "cargo-allow.unused-dependency-family-inventory.v1";

/// Current schema version of the rendered family-inventory artifact.
const FAMILY_INVENTORY_SCHEMA_VERSION: u32 = 1;

/// Repository-relative directory holding the checked-in artifacts.
const ARTIFACT_RECEIPTS_DIR: &str = "docs/dogfood/receipts";

/// Checked-in JSON artifact file name.
const ARTIFACT_JSON_FILE_NAME: &str = "unused-dependency-inventory-v1.json";

/// Checked-in markdown artifact file name.
const ARTIFACT_MD_FILE_NAME: &str = "unused-dependency-inventory-v1.md";

/// The family denominator: (package name, manifest-relative directory under
/// `crates/`), in the `.github/workflows/ci.yml` release-set clippy lane
/// order plus `cargo-proof`. See the module documentation for why these 14
/// packages are the family.
const FAMILY_PACKAGES: [(&str, &str); 14] = [
    ("cargo-allow", "cargo-allow"),
    ("allow-core", "allow-core"),
    ("allow-policy", "allow-policy"),
    ("allow-policy-legacy", "allow-policy-legacy"),
    ("allow-inventory", "allow-inventory"),
    ("allow-files", "allow-files"),
    ("allow-rust", "allow-rust"),
    ("allow-match", "allow-match"),
    ("allow-report", "allow-report"),
    ("allow-diff", "allow-diff"),
    ("effortless-repo-protocol", "effortless-repo-protocol"),
    ("effortless-repo-snapshot", "effortless-repo-snapshot"),
    ("effortless-repo-edit", "effortless-repo-edit"),
    ("cargo-proof", "cargo-proof"),
];

/// The family packages that carry a #3905 feature-configuration matrix and
/// whose `.default` ids are that matrix's default rows. Every other family
/// package is featureless and sits outside the matrix.
const MATRIX_DEFAULT_ROW_PACKAGES: [&str; 3] = ["allow-rust", "allow-files", "cargo-proof"];

/// Family definition note rendered into both artifacts.
const FAMILY_DEFINITION_NOTE: &str = "the cargo-allow release-set lane package list from \
     .github/workflows/ci.yml (release-set clippy step) plus the cargo-proof product package \
     from the #3905 shared product line (14 packages); each package is inspected under its \
     default configuration: allow-rust.default, allow-files.default, and cargo-proof.default \
     are the #3905 matrix default rows, the other <package>.default ids are the featureless \
     default configurations outside the #3905 matrix, and the deeper matrix rows are separate \
     scopes reserved for later PRs";

/// Note carried by `Used` findings, which need no reviewed row.
const IMPLICIT_RETAIN_NOTE: &str = "implicit retain: exact reference evidence exists in the scanned inputs, so no reviewed \
     row is required";

/// Reviewed disposition verdicts for one live non-`Used` finding. The five
/// variants are the issue's disposition classes: remove, retain with
/// evidence, transition, unsupported, and tool defect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DispositionVerdict {
    /// Remove the dependency (in a separately authorized PR).
    Remove,
    /// Keep the dependency; the note carries the retained evidence.
    RetainWithEvidence,
    /// Keep through a named transitional authority.
    Transition,
    /// The edge stays outside what this inventory can attribute.
    Unsupported,
    /// The finding is an analyzer defect to repair in the analyzer.
    ToolDefect,
}

impl DispositionVerdict {
    fn as_str(self) -> &'static str {
        match self {
            Self::Remove => "remove",
            Self::RetainWithEvidence => "retain_with_evidence",
            Self::Transition => "transition",
            Self::Unsupported => "unsupported",
            Self::ToolDefect => "tool_defect",
        }
    }
}

/// One reviewed disposition row. Rows are keyed by (package, dependency,
/// class) because one dependency name can occupy several manifest tables
/// with different verdicts; the live-inventory tests keep every row honest.
struct DispositionRow {
    package: &'static str,
    dependency: &'static str,
    class: UnusedDependencyDependencyClassV1,
    verdict: DispositionVerdict,
    note: &'static str,
}

/// The reviewed dispositions for the live inventory. Requirements: every
/// non-`Used` live finding has exactly one row here, and every row matches a
/// live non-`Used` finding — the coverage test enforces both directions, so
/// an unlisted finding fails with instructions and a stale row fails too.
/// `Used` findings need no rows (Used == implicit retain with evidence).
///
/// History: the PR B inventory listed three Remove candidates. Two
/// (cargo-allow/intent-compiler, cargo-proof/proof-orchestrator) were false
/// — both packages rename their crate root via `[lib] name` and are used
/// under those spellings — and were reclassified Used by the lib-identity
/// repair. The third (allow-policy/serde_json) was removed in #3909 PR C
/// after the mandated compile check (cargo check + 526 allow-policy tests
/// green without the row), so the table is currently empty: an unused
/// dependency that is truly unused has no row to carry.
const DISPOSITIONS: [DispositionRow; 0] = [];

fn require(condition: bool, message: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.to_string())
    }
}

/// Normalize intake text: CRLF and lone CR become LF so comparisons never
/// depend on the checkout's autocrlf smudging.
fn normalize_text(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// The repository root, derived from this crate's manifest directory
/// (`crates/allow-inventory`) two levels up.
fn repo_root() -> Result<PathBuf, String> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| "CARGO_MANIFEST_DIR has no repository grandparent".to_string())
}

/// The configuration id for one family package. The spelling is uniform:
/// `allow-rust`, `allow-files`, and `cargo-proof` are the #3905 matrix
/// default rows, and the featureless packages use the same `<package>.default`
/// spelling for the featureless default configuration outside the matrix.
fn configuration_id(package: &str) -> String {
    format!("{package}.default")
}

/// Walk one declared source root with an explicit stack (no recursion, no
/// indexing), collecting every `.rs` file as (package-relative path, LF-only
/// text). Paths use forward slashes regardless of host separators.
fn collect_rs_inputs(
    source_root: &Path,
    root_name: &str,
    pairs: &mut Vec<(String, String)>,
) -> Result<(), String> {
    let mut stack: Vec<PathBuf> = vec![source_root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries =
            fs::read_dir(&dir).map_err(|error| format!("read_dir {}: {error}", dir.display()))?;
        for entry in entries {
            let entry = entry
                .map_err(|error| format!("read_dir entry under {}: {error}", dir.display()))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|error| format!("file_type {}: {error}", path.display()))?;
            if file_type.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
                let relative_tail = path
                    .strip_prefix(source_root)
                    .map_err(|error| error.to_string())?
                    .to_string_lossy()
                    .replace('\\', "/");
                let text = fs::read_to_string(&path)
                    .map_err(|error| format!("read {}: {error}", path.display()))?;
                pairs.push((
                    format!("{root_name}/{relative_tail}"),
                    normalize_text(&text),
                ));
            }
        }
    }
    Ok(())
}

/// Collect the declared source inputs for one package: every `.rs` file under
/// `src/`, `tests/`, `benches/`, and `examples/` plus `build.rs` when
/// present, sorted by package-relative path for determinism.
fn collect_source_inputs(package_dir: &Path) -> Result<Vec<UnusedDependencySourceInputV1>, String> {
    let mut pairs: Vec<(String, String)> = Vec::new();
    for root_name in ["src", "tests", "benches", "examples"] {
        let source_root = package_dir.join(root_name);
        if source_root.is_dir() {
            collect_rs_inputs(&source_root, root_name, &mut pairs)?;
        }
    }
    let build_script = package_dir.join("build.rs");
    if build_script.is_file() {
        let text = fs::read_to_string(&build_script)
            .map_err(|error| format!("read {}: {error}", build_script.display()))?;
        pairs.push(("build.rs".to_string(), normalize_text(&text)));
    }
    pairs.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(pairs
        .into_iter()
        .map(|(relative_path, text)| UnusedDependencySourceInputV1 {
            relative_path,
            text,
        })
        .collect())
}

/// Resolve the request version honestly: the package manifest's own
/// `[package] version` when declared, else the workspace-inherited
/// `[workspace.package] version` from the root manifest. The analyzer does
/// not consume the version; it only completes the request identity.
fn resolve_package_version(manifest_text: &str, root_manifest_text: &str) -> String {
    let from_package = toml::from_str::<toml::Value>(manifest_text)
        .ok()
        .and_then(|value| {
            value
                .get("package")
                .and_then(|package| package.get("version"))
                .and_then(toml::Value::as_str)
                .map(str::to_string)
        });
    from_package.unwrap_or_else(|| {
        toml::from_str::<toml::Value>(root_manifest_text)
            .ok()
            .and_then(|value| {
                value
                    .get("workspace")
                    .and_then(|workspace| workspace.get("package"))
                    .and_then(|package| package.get("version"))
                    .and_then(toml::Value::as_str)
                    .map(str::to_string)
            })
            .unwrap_or_default()
    })
}

/// Build one request per family package from the real repository files.
fn build_family_requests() -> Result<Vec<UnusedDependencyRequestV1>, String> {
    let root = repo_root()?;
    let root_manifest_text = normalize_text(
        &fs::read_to_string(root.join("Cargo.toml"))
            .map_err(|error| format!("read workspace manifest: {error}"))?,
    );
    let lib_identities = workspace_lib_identities(&root)?;
    let mut requests = Vec::new();
    for (package, directory) in FAMILY_PACKAGES {
        let package_dir = root.join("crates").join(directory);
        let manifest_text = normalize_text(
            &fs::read_to_string(package_dir.join("Cargo.toml"))
                .map_err(|error| format!("read {package} manifest: {error}"))?,
        );
        let source_inputs = collect_source_inputs(&package_dir)?;
        let build_script_present = package_dir.join("build.rs").is_file();
        requests.push(UnusedDependencyRequestV1 {
            package_name: package.to_string(),
            package_version: resolve_package_version(&manifest_text, &root_manifest_text),
            configuration_id: configuration_id(package),
            manifest_text,
            source_inputs,
            build_script_present,
            dependency_lib_identities: lib_identities.clone(),
        });
    }
    Ok(requests)
}

/// Resolve workspace lib identities: every crates/ member that renames its
/// crate root via `[lib] name` contributes one identity, so dependency rows
/// naming that package are also scanned under the real lib spelling. The
/// analyzer folds only the package name on its own, which is exactly how a
/// used dependency can look unused (the intent-compiler -> intent_engine
/// and proof-orchestrator -> proof_engine remaps both hid live use).
fn workspace_lib_identities(root: &Path) -> Result<Vec<UnusedDependencyLibIdentityV1>, String> {
    let crates_dir = root.join("crates");
    let mut entries: Vec<std::fs::DirEntry> = fs::read_dir(&crates_dir)
        .map_err(|error| format!("read crates dir: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("read crates dir entry: {error}"))?;
    entries.sort_by_key(|entry| entry.file_name());
    let mut identities = Vec::new();
    for entry in entries {
        let manifest_path = entry.path().join("Cargo.toml");
        if !manifest_path.is_file() {
            continue;
        }
        let manifest_text = normalize_text(
            &fs::read_to_string(&manifest_path)
                .map_err(|error| format!("read {}: {error}", manifest_path.display()))?,
        );
        let manifest: toml::Value = toml::from_str(&manifest_text)
            .map_err(|error| format!("parse {}: {error}", manifest_path.display()))?;
        let package_name = manifest
            .get("package")
            .and_then(|package| package.get("name"))
            .and_then(|name| name.as_str())
            .ok_or_else(|| format!("{} has no package name", manifest_path.display()))?
            .to_string();
        let lib_name = manifest
            .get("lib")
            .and_then(|lib| lib.get("name"))
            .and_then(|name| name.as_str())
            .map(str::to_string)
            .filter(|lib_name| *lib_name != package_name.replace('-', "_"));
        if let Some(lib_name) = lib_name {
            identities.push(UnusedDependencyLibIdentityV1 {
                package_name,
                lib_name,
            });
        }
    }
    Ok(identities)
}

/// Inventory the whole family, receipts sorted by package name for stable
/// downstream rendering.
fn inventory_family() -> Result<Vec<UnusedDependencyReceiptV1>, String> {
    let mut receipts = inventory_packages(&build_family_requests()?);
    receipts.sort_by(|left, right| left.package_name.cmp(&right.package_name));
    Ok(receipts)
}

/// True when a live finding and a disposition row name the same
/// (package, dependency, class) triple.
fn row_matches_finding(row: &DispositionRow, finding: &UnusedDependencyFindingV1) -> bool {
    row.package == finding.package_name
        && row.dependency == finding.manifest_row.dependency_name
        && row.class == finding.manifest_row.class
}

/// Live findings that require a reviewed disposition: everything except
/// `Used` (implicit retain), `InstrumentFailure` (a failure description
/// with no dependency identity; the posture test blocks those separately),
/// and incomplete-scan findings (rows whose evidence is the
/// `no_source_inputs_supplied` marker — the scan saw nothing, so absence
/// there is noise; those stay review-visible in the artifacts instead,
/// which is the #3909 PR D severity split).
fn dispositionable_findings(
    receipts: &[UnusedDependencyReceiptV1],
) -> Vec<&UnusedDependencyFindingV1> {
    receipts
        .iter()
        .flat_map(|receipt| receipt.findings.iter())
        .filter(|finding| {
            !matches!(
                finding.disposition,
                UnusedDependencyDispositionV1::Used
                    | UnusedDependencyDispositionV1::InstrumentFailure
            )
        })
        .filter(|finding| {
            !finding
                .evidence
                .iter()
                .any(|entry| entry == INCOMPLETE_SCAN_EVIDENCE_MARKER)
        })
        .collect()
}

/// The reviewed (verdict, note) for one finding: the matching disposition
/// row for non-`Used` findings (exactly one row required), the implicit
/// retain for `Used` findings, and — for incomplete-scan findings (#3909
/// PR D severity split) — the fixed review-visible verdict with no
/// disposition row required. Incomplete-scan findings are exempt from
/// coverage in BOTH directions: no row is demanded of them, and no row can
/// match them (the stale-row law checks `dispositionable_findings`, which
/// excludes them), so an incomplete scan always has a legal artifact.
fn reviewed_disposition(finding: &UnusedDependencyFindingV1) -> Result<(String, String), String> {
    if finding.disposition == UnusedDependencyDispositionV1::Used {
        return Ok((
            DispositionVerdict::RetainWithEvidence.as_str().to_string(),
            IMPLICIT_RETAIN_NOTE.to_string(),
        ));
    }
    if finding
        .evidence
        .iter()
        .any(|entry| entry == INCOMPLETE_SCAN_EVIDENCE_MARKER)
    {
        return Ok((
            "review_visible".to_string(),
            "incomplete-scan finding, exempt from enforcement: the scan saw no \
             inputs, so absence here is noise; keep it visible and re-scan before \
             judging"
                .to_string(),
        ));
    }
    let matches: Vec<&DispositionRow> = DISPOSITIONS
        .iter()
        .filter(|row| row_matches_finding(row, finding))
        .collect();
    require(
        matches.len() == 1,
        &format!(
            "unlisted non-Used finding (package '{}' dependency '{}' class '{}'): add exactly \
             one DISPOSITIONS row covering the issue's disposition classes before rendering \
             the artifacts",
            finding.package_name,
            finding.manifest_row.dependency_name,
            finding.manifest_row.class.as_str()
        ),
    )?;
    let row = matches
        .first()
        .ok_or_else(|| "coverage demanded exactly one row but found none".to_string())?;
    Ok((row.verdict.as_str().to_string(), row.note.to_string()))
}

/// Rendered JSON shape of the family inventory. Field order is declaration
/// order and the artifact is line-free: no evidence file:line entries, so
/// the artifact stays stable across unrelated code churn.
#[derive(Serialize)]
struct FamilyInventoryArtifact {
    schema_id: &'static str,
    schema_version: u32,
    analyzer_identity: String,
    family_definition_note: String,
    packages: Vec<PackageInventoryArtifact>,
}

#[derive(Serialize)]
struct PackageInventoryArtifact {
    package_name: String,
    configuration_id: String,
    packages_inspected: u32,
    instrument_posture: String,
    findings: Vec<FindingArtifact>,
}

#[derive(Serialize)]
struct FindingArtifact {
    dependency_name: String,
    class: String,
    optional: bool,
    target: Option<String>,
    disposition: String,
    inventory_verdict: String,
    note: String,
}

/// Render both artifacts from the live receipts. Fails if any non-`Used`
/// finding lacks exactly one disposition row.
fn render_artifacts(receipts: &[UnusedDependencyReceiptV1]) -> Result<(String, String), String> {
    let mut packages = Vec::new();
    for receipt in receipts {
        require(
            receipt.instrument_posture == UnusedDependencyInstrumentPostureV1::Complete,
            &format!(
                "the family inventory cannot render over an InstrumentFailure posture \
                 (package '{}'); repair the instrument failure first",
                receipt.package_name
            ),
        )?;
        let mut findings = Vec::new();
        for finding in &receipt.findings {
            let (disposition, note) = reviewed_disposition(finding)?;
            findings.push(FindingArtifact {
                dependency_name: finding.manifest_row.dependency_name.clone(),
                class: finding.manifest_row.class.as_str().to_string(),
                optional: finding.manifest_row.optional,
                target: finding.manifest_row.target.clone(),
                inventory_verdict: finding.disposition.as_str().to_string(),
                disposition,
                note,
            });
        }
        findings.sort_by(|left, right| {
            (&left.dependency_name, &left.class).cmp(&(&right.dependency_name, &right.class))
        });
        packages.push(PackageInventoryArtifact {
            package_name: receipt.package_name.clone(),
            configuration_id: receipt.configuration_id.clone(),
            packages_inspected: receipt.packages_inspected,
            instrument_posture: receipt.instrument_posture.as_str().to_string(),
            findings,
        });
    }
    packages.sort_by(|left, right| left.package_name.cmp(&right.package_name));
    let artifact = FamilyInventoryArtifact {
        schema_id: FAMILY_INVENTORY_SCHEMA_ID,
        schema_version: FAMILY_INVENTORY_SCHEMA_VERSION,
        analyzer_identity: UNUSED_DEPENDENCY_ANALYZER_IDENTITY.to_string(),
        family_definition_note: FAMILY_DEFINITION_NOTE.to_string(),
        packages,
    };
    let json = serde_json::to_string_pretty(&artifact)
        .map_err(|error| format!("render family artifact json: {error}"))?;
    Ok((format!("{json}\n"), render_markdown(&artifact)))
}

/// Render the human markdown table for the same rows as the JSON artifact.
fn render_markdown(artifact: &FamilyInventoryArtifact) -> String {
    let mut markdown =
        String::from("# Unused-dependency family inventory (advisory, #3909 PR B)\n");
    markdown.push_str("\n- schema_id: ");
    markdown.push_str(artifact.schema_id);
    markdown.push_str("\n- schema_version: ");
    markdown.push_str(&artifact.schema_version.to_string());
    markdown.push_str("\n- analyzer_identity: ");
    markdown.push_str(&artifact.analyzer_identity);
    markdown.push_str("\n- claim_boundary: ");
    markdown.push_str(UNUSED_DEPENDENCY_CLAIM_BOUNDARY);
    markdown.push_str("\n- family_definition: ");
    markdown.push_str(&artifact.family_definition_note);
    markdown.push_str(
        "\n- disposition_law: Used rows are implicit retains with evidence; every other live \
         finding carries exactly one reviewed row (remove, retain_with_evidence, transition, \
         unsupported, tool_defect); advisory only, nothing is removed and no CI enforcement \
         exists\n",
    );
    markdown.push_str(
        "\n| package | configuration | dependency | class | optional | target | inventory \
         verdict | disposition | note |\n",
    );
    markdown.push_str("| --- | --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for package in &artifact.packages {
        for finding in &package.findings {
            markdown.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
                package.package_name,
                package.configuration_id,
                finding.dependency_name,
                finding.class,
                if finding.optional { "true" } else { "false" },
                finding.target.as_deref().unwrap_or(""),
                finding.inventory_verdict,
                finding.disposition,
                finding.note,
            ));
        }
    }
    markdown
}

/// Read both checked-in artifacts, normalized to LF so autocrlf smudging
/// never changes the comparison.
fn checked_in_artifacts() -> Result<(String, String), String> {
    let receipts_dir = repo_root()?.join(ARTIFACT_RECEIPTS_DIR);
    let json = fs::read_to_string(receipts_dir.join(ARTIFACT_JSON_FILE_NAME)).map_err(|error| {
        format!(
            "read {ARTIFACT_RECEIPTS_DIR}/{ARTIFACT_JSON_FILE_NAME}: {error} (generate it with \
             the --ignored regenerate_artifacts test)"
        )
    })?;
    let markdown =
        fs::read_to_string(receipts_dir.join(ARTIFACT_MD_FILE_NAME)).map_err(|error| {
            format!(
                "read {ARTIFACT_RECEIPTS_DIR}/{ARTIFACT_MD_FILE_NAME}: {error} (generate it with \
             the --ignored regenerate_artifacts test)"
            )
        })?;
    Ok((normalize_text(&json), normalize_text(&markdown)))
}

const ARTIFACT_DRIFT_INSTRUCTIONS: &str = "artifacts drifted from the live inventory. If the \
     inventory change is intended, re-review the DISPOSITIONS table, then refresh the \
     checked-in artifacts with: cargo test -p allow-inventory --test \
     unused_dependency_family_inventory -- --ignored regenerate_artifacts";

/// (1) Every family package inventories with `Complete` posture and exactly
/// one inspected package.
#[test]
fn every_family_package_inventories_with_complete_posture() -> Result<(), String> {
    let requests = build_family_requests()?;
    require(
        requests.len() == FAMILY_PACKAGES.len(),
        "one request must be built per family package",
    )?;
    let receipts = inventory_packages(&requests);
    require(
        receipts.len() == FAMILY_PACKAGES.len(),
        "one receipt must come back per family package",
    )?;
    for receipt in &receipts {
        require(
            receipt.instrument_posture == UnusedDependencyInstrumentPostureV1::Complete,
            &format!(
                "package '{}' must inventory with Complete posture, got {}",
                receipt.package_name,
                receipt.instrument_posture.as_str()
            ),
        )?;
        require(
            receipt.packages_inspected == 1,
            &format!(
                "package '{}' must inspect exactly one package",
                receipt.package_name
            ),
        )?;
    }
    // Family-definition consistency: every #3905 matrix package named in the
    // rendered family_definition note must remain a family package.
    for package in MATRIX_DEFAULT_ROW_PACKAGES {
        require(
            FAMILY_PACKAGES
                .iter()
                .any(|(family_package, _)| *family_package == package),
            &format!(
                "matrix package '{package}' is named in the family definition note but is not \
                 in FAMILY_PACKAGES"
            ),
        )?;
    }
    Ok(())
}

/// (2) Every live non-`Used` finding has exactly one disposition row, and
/// every row matches a live non-`Used` finding (stale rows fail).
#[test]
fn dispositions_cover_the_live_inventory_exactly() -> Result<(), String> {
    let receipts = inventory_family()?;
    let live = dispositionable_findings(&receipts);
    for finding in &live {
        let matching = DISPOSITIONS
            .iter()
            .filter(|row| row_matches_finding(row, finding))
            .count();
        require(
            matching == 1,
            &format!(
                "live non-Used finding (package '{}' dependency '{}' class '{}') has {} \
                 disposition rows, expected exactly 1: add a DISPOSITIONS row with one of the \
                 five verdicts (Remove, RetainWithEvidence, Transition, Unsupported, \
                 ToolDefect)",
                finding.package_name,
                finding.manifest_row.dependency_name,
                finding.manifest_row.class.as_str(),
                matching
            ),
        )?;
    }
    for row in &DISPOSITIONS {
        let still_live = live.iter().any(|finding| row_matches_finding(row, finding));
        require(
            still_live,
            &format!(
                "disposition row (package '{}' dependency '{}' class '{}') no longer matches a \
                 live non-Used finding: remove the stale row to keep the table honest",
                row.package,
                row.dependency,
                row.class.as_str()
            ),
        )?;
    }
    Ok(())
}

/// (2b) Review-visibility law (#3909 PR D): findings from incomplete scans
/// (evidence carries the `no_source_inputs_supplied` marker) are exempt
/// from the disposition-coverage law above in BOTH directions — no row is
/// demanded of them and no row may match them — so an incomplete scan
/// always has a legal artifact state. The law is pinned on a synthetic
/// incomplete scan (the live family currently has zero marker findings, so
/// a live-only probe would be vacuous): the synthetic finding must render
/// review-visible, must be excluded from coverage, and the receipt must
/// still validate.
#[test]
fn incomplete_scan_findings_stay_review_visible_without_enforcement() -> Result<(), String> {
    let request = UnusedDependencyRequestV1 {
        package_name: "synthetic".to_string(),
        package_version: "0.1.0".to_string(),
        configuration_id: "synthetic.default".to_string(),
        manifest_text: [
            "[package]",
            "name = \"synthetic\"",
            "version = \"0.1.0\"",
            "",
            "[dependencies]",
            "proptest = \"1\"",
        ]
        .join("\n"),
        source_inputs: Vec::new(),
        build_script_present: false,
        dependency_lib_identities: Vec::new(),
    };
    let receipts = [inventory_packages(std::slice::from_ref(&request))];
    let receipt = receipts
        .first()
        .and_then(|batch| batch.first())
        .ok_or_else(|| "the synthetic batch lost its receipt".to_string())?;
    require(
        !receipt_scan_is_complete(receipt),
        "a receipt over zero inputs is an incomplete scan",
    )?;
    let marker_findings: Vec<&UnusedDependencyFindingV1> = receipt
        .findings
        .iter()
        .filter(|finding| {
            finding
                .evidence
                .iter()
                .any(|entry| entry == INCOMPLETE_SCAN_EVIDENCE_MARKER)
        })
        .collect();
    require(
        marker_findings.len() == 1,
        "the synthetic incomplete scan must carry exactly the proptest marker finding",
    )?;
    let finding = marker_findings
        .first()
        .ok_or_else(|| "the synthetic scan lost its marker finding".to_string())?;
    let excluded = dispositionable_findings(std::slice::from_ref(receipt))
        .iter()
        .all(|live| !std::ptr::eq(*live, *finding));
    require(
        excluded,
        "incomplete-scan findings must be exempt from the disposition coverage law",
    )?;
    let (verdict, note) = reviewed_disposition(finding)?;
    require(
        verdict == "review_visible",
        "an incomplete-scan finding must render the review_visible verdict",
    )?;
    require(
        note.contains("exempt from enforcement"),
        "the rendered note must state the enforcement exemption",
    )?;
    validate_receipt(receipt)?;
    Ok(())
}

/// (3) Drift guard: the rendered artifacts must equal the checked-in files
/// (LF-normalized on read).
#[test]
fn rendered_artifacts_match_the_checked_in_files() -> Result<(), String> {
    let receipts = inventory_family()?;
    let (json, markdown) = render_artifacts(&receipts)?;
    let (checked_json, checked_markdown) = checked_in_artifacts()?;
    require(
        json == checked_json,
        &format!("{ARTIFACT_JSON_FILE_NAME}: {ARTIFACT_DRIFT_INSTRUCTIONS}"),
    )?;
    require(
        markdown == checked_markdown,
        &format!("{ARTIFACT_MD_FILE_NAME}: {ARTIFACT_DRIFT_INSTRUCTIONS}"),
    )?;
    Ok(())
}

/// (4) Regeneration generator: refreshes the checked-in artifacts when the
/// live inventory change is intended and the DISPOSITIONS table has been
/// re-reviewed. Run explicitly with:
/// `cargo test -p allow-inventory --test unused_dependency_family_inventory -- --ignored regenerate_artifacts`
#[test]
#[ignore = "generator: run with --ignored to refresh the checked-in artifacts"]
fn regenerate_artifacts() -> Result<(), String> {
    let receipts = inventory_family()?;
    let (json, markdown) = render_artifacts(&receipts)?;
    let receipts_dir = repo_root()?.join(ARTIFACT_RECEIPTS_DIR);
    fs::create_dir_all(&receipts_dir)
        .map_err(|error| format!("create {}: {error}", receipts_dir.display()))?;
    let json_path = receipts_dir.join(ARTIFACT_JSON_FILE_NAME);
    fs::write(&json_path, json)
        .map_err(|error| format!("write {}: {error}", json_path.display()))?;
    let markdown_path = receipts_dir.join(ARTIFACT_MD_FILE_NAME);
    fs::write(&markdown_path, markdown)
        .map_err(|error| format!("write {}: {error}", markdown_path.display()))?;
    Ok(())
}

/// (5) Determinism: two in-process inventories of the same family render
/// byte-identical artifacts.
#[test]
fn two_in_process_inventories_render_identical_artifacts() -> Result<(), String> {
    let first = inventory_family()?;
    let second = inventory_family()?;
    let first_render = render_artifacts(&first)?;
    let second_render = render_artifacts(&second)?;
    require(
        first_render == second_render,
        "two in-process inventories of the same family must render identical artifacts",
    )?;
    Ok(())
}

/// (6) Closed-vocabulary pin: the five verdict labels are the issue's
/// disposition classes and render into both artifacts, so they stay exact
/// even while only one of them is live today.
#[test]
fn disposition_verdicts_pin_all_five_issue_classes() -> Result<(), String> {
    let samples = [
        (DispositionVerdict::Remove, "remove"),
        (
            DispositionVerdict::RetainWithEvidence,
            "retain_with_evidence",
        ),
        (DispositionVerdict::Transition, "transition"),
        (DispositionVerdict::Unsupported, "unsupported"),
        (DispositionVerdict::ToolDefect, "tool_defect"),
    ];
    for (verdict, label) in samples {
        require(
            verdict.as_str() == label,
            "the disposition verdict label must match the pinned artifact vocabulary",
        )?;
    }
    Ok(())
}
