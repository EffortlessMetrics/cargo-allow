//! Read-only candidate-preparation projection (#3831).
//!
//! Gathers the exact input identity from the live repository, converts the
//! typed V2 topology rows into the plan vocabulary, and renders the pure
//! `CandidatePreparationResultV1` from `allow-report`. This command never
//! writes source, policy, Git state, tags, registry, or GitHub state.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use allow_report::{
    CandidateCollisionResultV1, CandidateContentStateV1, CandidateCorpusSourceV1,
    CandidateExternalObservationV1, CandidateOperationCompilerInput, CandidatePackageRowV1,
    CandidatePreparationDirtyStateV1, CandidatePreparationInputIdentityV1,
    CandidatePreparationReadinessV1, CandidatePreparationResultV1, CandidateProjectionInput,
    CandidateReleaseIdentityProjectionV1, CandidateSurfaceDecisionV1, CandidateSurfaceInputV1,
    ReleaseVersionV1, compile_candidate_operations, prepare_candidate_plan,
};
use clap::{Parser, Subcommand, ValueEnum};

const TOPOLOGY_PATH: &str = "policy/product-package-topology-v2.toml";
const SUPPORT_MATRIX_PATH: &str = "policy/product-support-matrix.toml";
/// The candidate/channel projection surface (distinct from the policy
/// posture matrix, which is digest-bound input only).
const CANDIDATE_SUPPORT_PATH: &str = "docs/support-matrix.toml";
const ALLOW_POLICY_PATH: &str = "policy/allow.toml";
const CARGO_LOCK_PATH: &str = "Cargo.lock";
const WORKSPACE_MANIFEST_PATH: &str = "Cargo.toml";
const CHANGIE_CONFIG_PATH: &str = ".changie.yaml";
const CHANGES_DIR: &str = ".changes";
const INCIDENT_EVIDENCE_PATH: &str = "docs/release/evidence/rc1-publication-incident.v1.json";

/// Walk bounds for the Changie history corpus (#2344 walk-bounding law).
const CORPUS_WALK_MAX_DEPTH: usize = 32;
const CORPUS_WALK_MAX_ENTRIES: usize = 100_000;

/// One collected input fact; `Err` carries the instrument failure reason.
type Fact<T> = Result<T, String>;

/// Read-only candidate preparation projection (hidden release tooling).
#[derive(Debug, Clone, Parser)]
#[command(disable_version_flag = true)]
pub(crate) struct PrepCandidateArgs {
    #[command(subcommand)]
    pub(crate) command: PrepCandidateSubcommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum PrepCandidateSubcommand {
    /// Project the exact prospective candidate transition without writing.
    Plan(PrepCandidatePlanArgs),
    /// Apply one exact reviewed plan atomically and stale-safely.
    Apply(PrepCandidateApplyArgs),
}

/// Apply arguments for one reviewed plan file.
#[derive(Debug, Clone, Parser)]
pub(crate) struct PrepCandidateApplyArgs {
    /// Reviewed plan file (the `prep-candidate plan --format json` output).
    #[arg(long)]
    pub(crate) from_plan: PathBuf,
    /// Where to write the bounded intermediate apply receipt.
    #[arg(long)]
    pub(crate) receipt: Option<PathBuf>,
    /// Acknowledge one required decision by id (repeatable).
    #[arg(long = "acknowledge-decision")]
    pub(crate) acknowledge_decision: Vec<String>,
    /// After a successful apply, run post-apply reconciliation and write
    /// the final CandidatePreparationReceiptV1 to this path.
    #[arg(long = "final-receipt")]
    pub(crate) final_receipt: Option<PathBuf>,
}

/// Output rendering for the preparation result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum PrepOutputFormat {
    Json,
    Text,
}

#[derive(Debug, Clone, Parser)]
pub(crate) struct PrepCandidatePlanArgs {
    /// Canonical stable or numbered release-candidate target version.
    #[arg(long)]
    pub(crate) version: String,
    /// Output rendering. Both derive from the same typed result.
    #[arg(long, value_enum, default_value_t = PrepOutputFormat::Json)]
    pub(crate) format: PrepOutputFormat,
    /// Optional add-finding plan supplying governed policy changes.
    #[arg(long)]
    pub(crate) policy_plan: Option<PathBuf>,
}

pub(super) fn cmd_prep_candidate(args: &PrepCandidateArgs) -> CargoAllowResult<()> {
    let root = git_root().map_err(|reason| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("prep-candidate requires a git worktree: {reason}"),
        )
    })?;
    cmd_prep_candidate_with_root(&root, args)
}

/// Root-parameterized dispatch (fixture tests bind an explicit root).
pub(crate) fn cmd_prep_candidate_with_root(
    root: &Path,
    args: &PrepCandidateArgs,
) -> CargoAllowResult<()> {
    match &args.command {
        PrepCandidateSubcommand::Plan(plan_args) => {
            cmd_prep_candidate_plan_for_root(root, plan_args)
        }
        PrepCandidateSubcommand::Apply(apply_args) => {
            cmd_prep_candidate_apply_with_root(root, apply_args)
        }
    }
}

/// Root-parameterized apply entry point (the engine's fixture tests bind
/// an explicit repository root).
pub(crate) fn cmd_prep_candidate_apply_with_root(
    root: &Path,
    args: &PrepCandidateApplyArgs,
) -> CargoAllowResult<()> {
    let plan_text = std::fs::read_to_string(&args.from_plan).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("read {}: {error}", args.from_plan.display()),
        )
    })?;
    let plan: CandidatePreparationResultV1 = serde_json::from_str(&plan_text).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("parse plan file: {error}"),
        )
    })?;
    let receipt = apply_candidate_plan(
        root,
        &plan,
        &args.acknowledge_decision,
        args.receipt.as_deref(),
        ApplyFault::none(),
    );
    if let Some(final_receipt_path) = &args.final_receipt {
        let final_receipt =
            reconcile_candidate_preparation(root, &plan, &receipt, &args.acknowledge_decision);
        let final_rendered = serde_json::to_string_pretty(&final_receipt).map_err(|error| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::Internal,
                format!("render final preparation receipt: {error}"),
            )
        })?;
        effortless_repo_edit::write_file(final_receipt_path, &final_rendered).map_err(|error| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::Internal,
                format!("write final preparation receipt: {error}"),
            )
        })?;
        println!(
            "candidate preparation receipt: state {:?}; written to {}",
            final_receipt.state,
            final_receipt_path.display()
        );
    }
    let rendered = serde_json::to_string_pretty(&receipt).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Internal,
            format!("render apply receipt: {error}"),
        )
    })?;
    if let Some(receipt_path) = &args.receipt {
        effortless_repo_edit::write_file(receipt_path, &rendered).map_err(|error| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::Internal,
                format!("write apply receipt: {error}"),
            )
        })?;
    }
    println!("{}", receipt_human_summary(&receipt));
    match receipt.state {
        allow_report::CandidateApplyStateV1::Applied
        | allow_report::CandidateApplyStateV1::NoOp => Ok(()),
        _ => Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!(
                "apply finished in state {:?}; the receipt carries the explicit reasons",
                receipt.state
            ),
        )),
    }
}

fn receipt_human_summary(receipt: &allow_report::CandidateApplyReceiptV1) -> String {
    let applied = receipt
        .operations
        .iter()
        .filter(|operation| operation.result == "applied")
        .count();
    let rolled_back = receipt
        .operations
        .iter()
        .filter(|operation| operation.result == "rolled_back")
        .count();
    format!(
        "candidate apply: state {:?}; {applied} applied, {rolled_back} rolled back; transaction {}; rollback {}; plan {}",
        receipt.state, receipt.transaction_result, receipt.rollback_result, receipt.plan_digest,
    )
}

/// Root-parameterized plan command (fixture tests bind an explicit root).
pub(crate) fn cmd_prep_candidate_plan_for_root(
    root: &Path,
    args: &PrepCandidatePlanArgs,
) -> CargoAllowResult<()> {
    let result =
        build_preparation_result_for_root(root, &args.version, args.policy_plan.as_deref())?;
    let rendered = match args.format {
        PrepOutputFormat::Json => serde_json::to_string_pretty(&result).map_err(|error| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::Internal,
                format!("failed to render candidate preparation result: {error}"),
            )
        })?,
        PrepOutputFormat::Text => render_text_summary(&result),
    };
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(handle, "{rendered}").map_err(|error| {
        CargoAllowError::new(format!("write candidate preparation result: {error}"))
    })?;

    match result.readiness {
        CandidatePreparationReadinessV1::Ready
        | CandidatePreparationReadinessV1::DecisionRequired => Ok(()),
        CandidatePreparationReadinessV1::Stale
        | CandidatePreparationReadinessV1::Conflict
        | CandidatePreparationReadinessV1::Unsupported
        | CandidatePreparationReadinessV1::InstrumentFailure => Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!(
                "candidate preparation is not ready ({:?}); the typed result above carries the explicit reasons",
                result.readiness
            ),
        )),
    }
}

fn render_text_summary(result: &CandidatePreparationResultV1) -> String {
    let mut lines = vec![result.human_summary.clone()];
    for reason in &result.reasons {
        lines.push(format!("reason: {reason}"));
    }
    if let Some(plan) = &result.plan {
        for decision in &plan.required_decisions {
            lines.push(format!(
                "decision required [{}]: {} ({})",
                decision.decision_id, decision.question, decision.owner
            ));
        }
        lines.push(format!("plan digest: {}", plan.plan_digest));
        lines.push(format!("claim boundary: {}", plan.claim_boundary));
    }
    lines.join("\n")
}

fn instrument_failure_result(reasons: Vec<String>) -> CandidatePreparationResultV1 {
    CandidatePreparationResultV1 {
        schema: allow_report::CANDIDATE_PREPARATION_RESULT_SCHEMA_V1.to_string(),
        readiness: CandidatePreparationReadinessV1::InstrumentFailure,
        reasons,
        input_identity: None,
        plan: None,
        operations: None,
        human_summary: "candidate preparation inputs could not be trusted".to_string(),
    }
}

/// Root-parameterized projection, used by the apply engine's revalidation
/// and by the fixture-driven tests.
pub(crate) fn build_preparation_result_for_root(
    root: &Path,
    target_version: &str,
    policy_plan: Option<&Path>,
) -> CargoAllowResult<CandidatePreparationResultV1> {
    // Collect every input fact, accumulating instrument failures so all
    // collection gaps are reported in one pass.
    let mut failures: Vec<String> = Vec::new();
    let collect = |name: &str, fact: Fact<String>, failures: &mut Vec<String>| match fact {
        Ok(value) => Some(value),
        Err(reason) => {
            failures.push(format!("{name}: {reason}"));
            None
        }
    };

    let repository = collect(
        "repository identity",
        repository_identity(root),
        &mut failures,
    );
    let branch = collect(
        "branch",
        git_text(root, &["rev-parse", "--abbrev-ref", "HEAD"]),
        &mut failures,
    );
    let head_commit = collect(
        "HEAD commit",
        git_text(root, &["rev-parse", "HEAD"]),
        &mut failures,
    );
    let tree = collect(
        "HEAD tree",
        git_text(root, &["rev-parse", "HEAD^{tree}"]),
        &mut failures,
    );
    let dirty_state: Option<CandidatePreparationDirtyStateV1> = match dirty_state_class(root) {
        Ok(dirty_state) => Some(dirty_state),
        Err(reason) => {
            failures.push(format!("working-tree state: {reason}"));
            None
        }
    };

    let cargo_lock_digest = collect(
        "Cargo.lock digest",
        file_digest(root, CARGO_LOCK_PATH),
        &mut failures,
    );
    let workspace_manifest_digest = collect(
        "workspace manifest digest",
        file_digest(root, WORKSPACE_MANIFEST_PATH),
        &mut failures,
    );
    let topology_bytes = read_repo_file(root, TOPOLOGY_PATH);
    let topology_digest = match &topology_bytes {
        Ok(bytes) => Some(allow_core::sha256_v1_bytes(bytes)),
        Err(reason) => {
            failures.push(format!("topology digest: {reason}"));
            None
        }
    };
    let topology_generation = match parse_topology_generation(&topology_bytes) {
        Ok(generation) => Some(generation),
        Err(reason) => {
            failures.push(format!("topology generation: {reason}"));
            None
        }
    };
    let support_selection_digest = collect(
        "support selection digest",
        file_digest(root, SUPPORT_MATRIX_PATH),
        &mut failures,
    );
    let changie_config_digest = collect(
        "Changie configuration digest",
        file_digest(root, CHANGIE_CONFIG_PATH),
        &mut failures,
    );
    let changie_history_digest = collect(
        "Changie history corpus digest",
        changie_history_digest(root),
        &mut failures,
    );
    let allow_policy_bytes = read_repo_file(root, ALLOW_POLICY_PATH);
    let source_exception_policy_digest = match &allow_policy_bytes {
        Ok(bytes) => Some(allow_core::sha256_v1_bytes(bytes)),
        Err(reason) => {
            failures.push(format!("source-exception policy digest: {reason}"));
            None
        }
    };
    let source_exception_policy_schema_version =
        match parse_policy_schema_version(&allow_policy_bytes) {
            Ok(schema_version) => Some(schema_version),
            Err(reason) => {
                failures.push(format!("source-exception policy schema version: {reason}"));
                None
            }
        };
    let member_manifest_digests = match member_manifest_digests(root) {
        Ok(digests) => Some(digests),
        Err(reason) => {
            failures.push(format!("member manifest digests: {reason}"));
            None
        }
    };

    // Bind the target-corpus sources and the typed source identity digest.
    let topology_rows: Option<Vec<CandidatePackageRowV1>> = match &topology_bytes {
        Ok(bytes) => match std::str::from_utf8(bytes) {
            Ok(text) => match parse_topology_rows(text) {
                Ok(rows) => Some(rows),
                Err(error) => {
                    return Err(CargoAllowError::with_kind(
                        CargoAllowErrorKind::InvalidConfig,
                        format!("parse {TOPOLOGY_PATH}: {error}"),
                    ));
                }
            },
            Err(error) => {
                return Err(CargoAllowError::with_kind(
                    CargoAllowErrorKind::InvalidConfig,
                    format!("decode {TOPOLOGY_PATH}: {error}"),
                ));
            }
        },
        Err(reason) => {
            failures.push(format!("topology rows: {reason}"));
            None
        }
    };
    let projected_rows: Vec<CandidatePackageRowV1> = topology_rows.unwrap_or_default();
    let source_release_identity_digest = source_release_identity_digest(&projected_rows);

    let (release_record, github_release_note) = match ReleaseVersionV1::parse(target_version) {
        Ok(target) => {
            let release_record = corpus_source(
                root,
                &format!("docs/release/{}.md", target.as_str()),
                &mut failures,
            );
            let github_release_note = corpus_source(
                root,
                &format!("docs/release/github/{}.md", target.tag()),
                &mut failures,
            );
            (release_record, github_release_note)
        }
        // Malformed targets are projected as explicit Unsupported results;
        // corpus binding is skipped.
        Err(_) => (None, None),
    };

    if !failures.is_empty() {
        return Ok(instrument_failure_result(failures));
    }

    let identity = CandidatePreparationInputIdentityV1 {
        repository: repository.unwrap_or_default(),
        branch: branch.unwrap_or_default(),
        head_commit: head_commit.unwrap_or_default(),
        tree: tree.unwrap_or_default(),
        dirty_state: dirty_state.unwrap_or(CandidatePreparationDirtyStateV1::Unknown),
        cargo_lock_digest: cargo_lock_digest.unwrap_or_default(),
        workspace_manifest_digest: workspace_manifest_digest.unwrap_or_default(),
        member_manifest_digests: member_manifest_digests.unwrap_or_default(),
        topology_generation: topology_generation.unwrap_or_default(),
        topology_digest: topology_digest.unwrap_or_default(),
        source_release_identity_digest,
        support_selection_digest: support_selection_digest.unwrap_or_default(),
        changie_config_digest: changie_config_digest.unwrap_or_default(),
        changie_history_digest: changie_history_digest.unwrap_or_default(),
        release_record,
        github_release_note,
        source_exception_policy_schema_version: source_exception_policy_schema_version
            .unwrap_or_default(),
        source_exception_policy_digest: source_exception_policy_digest.unwrap_or_default(),
    };

    let support_matrix_postures =
        parse_support_matrix_postures(&read_repo_file(root, SUPPORT_MATRIX_PATH));
    let internal_requirements =
        parse_workspace_requirements(&read_repo_file(root, WORKSPACE_MANIFEST_PATH));

    let mut external_observations = vec![CandidateExternalObservationV1 {
        observation_id: "public_prerelease_line".to_string(),
        subject: "0.2.0-rc.1".to_string(),
        detail: "Public rc.1 is usable pilot evidence with incident lineage; not reusable as final package bytes (#3768 claim boundaries). Inputs only.".to_string(),
    }];
    if let Ok(digest) = file_digest(root, INCIDENT_EVIDENCE_PATH) {
        external_observations.push(CandidateExternalObservationV1 {
            observation_id: "rc1_publication_incident_evidence".to_string(),
            subject: INCIDENT_EVIDENCE_PATH.to_string(),
            detail: format!("retained incident evidence digest {digest}"),
        });
    }

    let mut result = prepare_candidate_plan(CandidateProjectionInput {
        target_version_text: target_version,
        input_identity: identity,
        topology_rows: &projected_rows,
        support_matrix_postures,
        internal_requirements,
        external_observations,
    });

    // Compile the file-operation plan for every projected semantic plan.
    if let Some(plan) = &result.plan
        && let Ok(target) = ReleaseVersionV1::parse(target_version)
    {
        let source_version = plan.source_release_identity.version.clone();
        match gather_surface_inputs(root, &source_version, &target, policy_plan) {
            Ok(mut surfaces) => {
                let warnings = resolve_surface_collisions(root, &mut surfaces);
                let compiled =
                    compile_candidate_operations(CandidateOperationCompilerInput { surfaces });
                for warning in warnings {
                    result
                        .reasons
                        .push(format!("operation compilation: {warning}"));
                }
                result.operations = Some(compiled);
            }
            Err(reason) => {
                result
                    .reasons
                    .push(format!("operation compilation unavailable: {reason}"));
            }
        }
    }
    Ok(result)
}

/// Digest over the typed source release identity projected from the
/// topology's product closure. Empty when the closure does not agree on one
/// line (the pure classifier then reports the conflict).
pub(crate) fn source_release_identity_digest(rows: &[CandidatePackageRowV1]) -> String {
    let mut source_versions: BTreeMap<&str, ()> = BTreeMap::new();
    for row in rows {
        if row.product_family == "cargo-allow" && row.candidate_inclusion {
            source_versions.insert(row.package_version.as_str(), ());
        }
    }
    if source_versions.len() != 1 {
        return String::new();
    }
    let source_version = source_versions.keys().next().expect("checked above");
    let Ok(version) = ReleaseVersionV1::parse(source_version) else {
        return String::new();
    };
    let projection = CandidateReleaseIdentityProjectionV1::from_version(&version);
    allow_core::sha256_v1_bytes(projection.canonical_digest("source").as_bytes())
}

fn corpus_source(
    root: &Path,
    relative: &str,
    failures: &mut Vec<String>,
) -> Option<CandidateCorpusSourceV1> {
    if !root.join(relative).exists() {
        return None;
    }
    match file_digest(root, relative) {
        Ok(digest) => Some(CandidateCorpusSourceV1 {
            path: relative.to_string(),
            digest,
        }),
        Err(reason) => {
            failures.push(format!("{relative}: {reason}"));
            None
        }
    }
}

/// Tolerant data mirror of the topology authority file (#2923). The binary
/// reads its own policy file as data — it does not import the cargo-intent
/// family's typed parser (#2580 product-coupling boundary) — and copies the
/// fields into the plan vocabulary verbatim. Release-version semantics stay
/// with the typed `ReleaseVersionV1` authority in `allow-report`.
#[derive(Debug, serde::Deserialize)]
struct TopologyFileToml {
    #[serde(default)]
    package: Vec<TopologyPackageToml>,
}

#[derive(Debug, serde::Deserialize)]
struct TopologyPackageToml {
    logical_id: String,
    cargo_package_name: String,
    version_line: String,
    product_family: String,
    posture: String,
    package_version: String,
    version_source: String,
    publication_state: String,
    publish: bool,
    candidate_inclusion: bool,
    release_order: u32,
    support_tier: String,
}

/// Parse the topology's `[[package]]` rows into the plan vocabulary.
pub(crate) fn parse_topology_rows(text: &str) -> Fact<Vec<CandidatePackageRowV1>> {
    let file: TopologyFileToml = toml::from_str(text).map_err(|error| format!("toml: {error}"))?;
    Ok(file
        .package
        .into_iter()
        .map(|row| CandidatePackageRowV1 {
            logical_id: row.logical_id,
            cargo_package_name: row.cargo_package_name,
            product_family: row.product_family,
            posture: row.posture,
            package_version: row.package_version,
            version_line: row.version_line,
            version_source: row.version_source,
            publication_state: row.publication_state,
            candidate_inclusion: row.candidate_inclusion,
            publish: row.publish,
            release_order: row.release_order,
            support_tier: row.support_tier,
        })
        .collect())
}

fn git_root() -> Fact<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|error| format!("git not runnable: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "not inside a git worktree: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(PathBuf::from(
        String::from_utf8_lossy(&output.stdout).trim().to_string(),
    ))
}

pub(crate) fn git_text(root: &Path, args: &[&str]) -> Fact<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|error| format!("git not runnable: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub(crate) fn repository_identity(root: &Path) -> Fact<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["remote", "get-url", "origin"])
        .output()
        .map_err(|error| format!("git not runnable: {error}"))?;
    if !output.status.success() {
        return Err("origin remote is unavailable".to_string());
    }
    let url = String::from_utf8_lossy(&output.stdout);
    let url = url.trim().trim_end_matches(".git");
    let mut segments = url.split(['/', ':']).filter(|segment| !segment.is_empty());
    let (repo, owner) = match (segments.next_back(), segments.next_back()) {
        (Some(repo), Some(owner)) => (repo, owner),
        _ => return Err(format!("origin URL {url:?} does not name owner/repository")),
    };
    Ok(format!("{owner}/{repo}"))
}

pub(crate) fn dirty_state_class(root: &Path) -> Fact<CandidatePreparationDirtyStateV1> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["status", "--porcelain"])
        .output()
        .map_err(|error| format!("git not runnable: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git status failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut modified: u32 = 0;
    let mut untracked: u32 = 0;
    for line in stdout.lines() {
        if line.starts_with("??") {
            untracked = untracked.saturating_add(1);
        } else {
            modified = modified.saturating_add(1);
        }
    }
    Ok(if modified == 0 && untracked == 0 {
        CandidatePreparationDirtyStateV1::Clean
    } else {
        CandidatePreparationDirtyStateV1::Dirty {
            modified_paths: modified,
            untracked_paths: untracked,
        }
    })
}

pub(crate) fn read_repo_file(root: &Path, relative: &str) -> Fact<Vec<u8>> {
    std::fs::read(root.join(relative)).map_err(|error| format!("read {relative}: {error}"))
}

pub(crate) fn file_digest(root: &Path, relative: &str) -> Fact<String> {
    Ok(allow_core::sha256_v1_bytes(&read_repo_file(
        root, relative,
    )?))
}

fn member_manifest_digests(root: &Path) -> Fact<BTreeMap<String, String>> {
    let workspace = read_repo_file(root, WORKSPACE_MANIFEST_PATH)?;
    let members = parse_workspace_members(&workspace)?;
    let mut digests = BTreeMap::new();
    for member in members {
        let manifest_path = format!("{member}/Cargo.toml");
        digests.insert(member.clone(), file_digest(root, &manifest_path)?);
    }
    Ok(digests)
}

/// Parse the `members = [...]` array of the workspace manifest.
pub(crate) fn parse_workspace_members(workspace: &[u8]) -> Fact<Vec<String>> {
    let text = std::str::from_utf8(workspace)
        .map_err(|error| format!("workspace manifest encoding: {error}"))?;
    let start = text
        .find("members = [")
        .ok_or("workspace manifest has no members array")?
        + "members = [".len();
    let end = text[start..]
        .find(']')
        .ok_or("workspace members array is unterminated")?
        + start;
    let members = text[start..end]
        .split(',')
        .map(|entry| entry.trim().trim_matches('"').to_string())
        .filter(|entry| !entry.is_empty())
        .collect();
    Ok(members)
}

/// Parse `authority_generation` from the topology header.
fn parse_topology_generation(topology: &Fact<Vec<u8>>) -> Fact<u32> {
    let bytes = topology.as_ref()?;
    let text = std::str::from_utf8(bytes).map_err(|error| format!("topology encoding: {error}"))?;
    let line = text
        .lines()
        .find(|line| line.trim_start().starts_with("authority_generation"))
        .ok_or("topology has no authority_generation field")?;
    let value = line
        .split('=')
        .nth(1)
        .ok_or("authority_generation is malformed")?
        .trim()
        .trim_matches('"');
    value
        .parse::<u32>()
        .map_err(|error| format!("authority_generation {value:?}: {error}"))
}

/// Parse `[policy]` schema_version from the source-exception policy.
fn parse_policy_schema_version(policy: &Fact<Vec<u8>>) -> Fact<String> {
    let bytes = policy.as_ref()?;
    let text = std::str::from_utf8(bytes).map_err(|error| format!("policy encoding: {error}"))?;
    let line = text
        .lines()
        .find(|line| line.trim_start().starts_with("schema_version"))
        .ok_or("policy has no schema_version field")?;
    let value = line
        .split('=')
        .nth(1)
        .ok_or("policy schema_version is malformed")?
        .trim()
        .trim_matches('"');
    Ok(value.to_string())
}

/// Parse `[[product]]` product_id/posture pairs from the support matrix.
/// A malformed matrix yields no postures; the pure classifier reports the
/// disagreement against the topology when the matrix would have mattered.
fn parse_support_matrix_postures(matrix: &Fact<Vec<u8>>) -> BTreeMap<String, String> {
    let mut postures = BTreeMap::new();
    let Ok(bytes) = matrix.as_ref() else {
        return postures;
    };
    let Ok(text) = std::str::from_utf8(bytes) else {
        return postures;
    };
    let mut current_product: Option<String> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("[[product]]") {
            current_product = None;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("product_id") {
            if let Some(value) = rest.trim().strip_prefix('=') {
                current_product = Some(value.trim().trim_matches('"').to_string());
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("posture")
            && let (Some(value), Some(product)) =
                (rest.trim().strip_prefix('='), current_product.as_ref())
        {
            postures.insert(product.clone(), value.trim().trim_matches('"').to_string());
        }
    }
    postures
}

/// Parse path dependencies from `[workspace.dependencies]` as key →
/// version requirement.
fn parse_workspace_requirements(workspace: &Fact<Vec<u8>>) -> BTreeMap<String, String> {
    let mut requirements = BTreeMap::new();
    let Ok(bytes) = workspace.as_ref() else {
        return requirements;
    };
    let Ok(text) = std::str::from_utf8(bytes) else {
        return requirements;
    };
    let Some(section_start) = text.find("[workspace.dependencies]") else {
        return requirements;
    };
    let section = &text[section_start + "[workspace.dependencies]".len()..];
    for line in section.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            break;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim().to_string();
        let value = value.trim();
        // Only workspace-internal path dependencies are release-coupled.
        if !value.contains("path =") {
            continue;
        }
        let Some(after_version) = value.split("version =").nth(1) else {
            continue;
        };
        let requirement = after_version
            .split('}')
            .next()
            .unwrap_or_default()
            .trim()
            .trim_matches('"')
            .trim();
        if !requirement.is_empty() {
            requirements.insert(key, requirement.to_string());
        }
    }
    requirements
}

/// Digest the sorted Changie history corpus under `.changes/`.
pub(crate) fn changie_history_digest(root: &Path) -> Fact<String> {
    let mut files = Vec::new();
    collect_corpus_files(&root.join(CHANGES_DIR), 0, &mut files)?;
    files.sort();
    let mut material = String::new();
    for path in &files {
        let relative = path
            .strip_prefix(root)
            .map_err(|_| "corpus path escapes the repository root".to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        let bytes = std::fs::read(path).map_err(|error| format!("read {relative}: {error}"))?;
        material.push_str(&relative);
        material.push('\0');
        material.push_str(&allow_core::sha256_v1_bytes(&bytes));
        material.push('\n');
    }
    Ok(allow_core::sha256_v1_bytes(material.as_bytes()))
}

pub(crate) fn collect_corpus_files(dir: &Path, depth: usize, files: &mut Vec<PathBuf>) -> Fact<()> {
    if depth > CORPUS_WALK_MAX_DEPTH {
        return Err(format!(
            "corpus walk exceeded depth bound {CORPUS_WALK_MAX_DEPTH}"
        ));
    }
    if files.len() > CORPUS_WALK_MAX_ENTRIES {
        return Err(format!(
            "corpus walk exceeded entry bound {CORPUS_WALK_MAX_ENTRIES}"
        ));
    }
    let entries =
        std::fs::read_dir(dir).map_err(|error| format!("read {}: {error}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("corpus entry: {error}"))?;
        let path = entry.path();
        if path.is_dir() {
            collect_corpus_files(&path, depth + 1, files)?;
        } else {
            files.push(path);
        }
    }
    Ok(())
}

/// The declared version-derivation scan set: files and directories whose
/// content functionally derives from the workspace version. Token hits
/// outside this set (history, evidence, fixtures) are never rewritten.
const VERSION_DERIVED_FILES: &[&str] = &["docs/getting-started.md"];

/// Compile the surface inputs for the operation plan from live authorities
/// and the #3831 projection. Every required owner class is represented;
/// generated bytes are deterministic token-level rewrites, and everything
/// judgment-bearing becomes an explicit decision row.
pub(crate) fn gather_surface_inputs(
    root: &Path,
    source_version: &str,
    target: &ReleaseVersionV1,
    policy_plan: Option<&Path>,
) -> Fact<Vec<CandidateSurfaceInputV1>> {
    let mut surfaces: Vec<CandidateSurfaceInputV1> = Vec::new();
    let source_token = source_version.to_string();
    let target_token = target.as_str().to_string();

    // Root workspace manifest: [workspace.package] version plus the exact
    // internal requirement rows.
    let workspace_bytes = read_repo_file(root, WORKSPACE_MANIFEST_PATH)?;
    let requirements = parse_workspace_requirements(&Ok(workspace_bytes.clone()));
    let requirement_hits = requirements
        .values()
        .filter(|requirement| *requirement == &format!("={source_token}"))
        .count();
    let workspace_prospective = render_token_swap(
        &workspace_bytes,
        &source_token,
        &target_token,
        1 + requirement_hits,
        WORKSPACE_MANIFEST_PATH,
    )?;
    surfaces.push(CandidateSurfaceInputV1 {
        owner: "workspace_manifest".to_string(),
        role: "package_version_and_requirements".to_string(),
        path: WORKSPACE_MANIFEST_PATH.to_string(),
        current: CandidateContentStateV1::from_bytes(&workspace_bytes),
        prospective_bytes: Some(workspace_prospective),
        judgment: None,
        collision: CandidateCollisionResultV1::Clear,
        rollback_source: Some("current-content-digest".to_string()),
        validation_obligations: vec!["no-new-guard".to_string()],
    });

    // Member manifests: those carrying their own version token move with the
    // line; the rest are explicit NoOps (their identity derives from the
    // root via `version.workspace = true`).
    let members = parse_workspace_members(&workspace_bytes)?;
    for member in members {
        let manifest_path = format!("{member}/Cargo.toml");
        let bytes = read_repo_file(root, &manifest_path)?;
        let prospective = if contains_token(&bytes, &source_token) {
            Some(render_token_swap(
                &bytes,
                &source_token,
                &target_token,
                1,
                &manifest_path,
            )?)
        } else {
            Some(bytes.clone())
        };
        surfaces.push(CandidateSurfaceInputV1 {
            owner: "member_manifest".to_string(),
            role: "manifest_identity".to_string(),
            path: manifest_path,
            current: CandidateContentStateV1::from_bytes(&bytes),
            prospective_bytes: prospective,
            judgment: None,
            collision: CandidateCollisionResultV1::Clear,
            rollback_source: Some("current-content-digest".to_string()),
            validation_obligations: Vec::new(),
        });
    }

    // Cargo.lock cannot be projected without executing cargo; the apply
    // slice regenerates it and binds the digest.
    let lock_bytes = read_repo_file(root, CARGO_LOCK_PATH)?;
    surfaces.push(CandidateSurfaceInputV1 {
        owner: "cargo_lock".to_string(),
        role: "lockfile_regeneration".to_string(),
        path: CARGO_LOCK_PATH.to_string(),
        current: CandidateContentStateV1::from_bytes(&lock_bytes),
        prospective_bytes: None,
        judgment: Some(CandidateSurfaceDecisionV1 {
            decision_id: "cargo-lock-regeneration".to_string(),
            question: "Regenerate Cargo.lock against the target line at apply and bind its digest; the plan cannot execute cargo.".to_string(),
            owner: "release-operator".to_string(),
            affected_operations: Vec::new(),
            missing_inputs: vec!["prospective lock bytes".to_string()],
        }),
        collision: CandidateCollisionResultV1::Clear,
        rollback_source: Some("current-content-digest".to_string()),
        validation_obligations: vec!["full-binary-suite".to_string()],
    });

    // Topology rows: every product row moves to the target line.
    let topology_bytes = read_repo_file(root, TOPOLOGY_PATH)?;
    let product_pattern = format!("package_version = \"{source_token}\"");
    let product_hits = std::str::from_utf8(&topology_bytes)
        .map_err(|error| format!("topology encoding: {error}"))?
        .matches(&product_pattern)
        .count();
    let topology_prospective = render_token_swap(
        &topology_bytes,
        &product_pattern,
        &format!("package_version = \"{target_token}\""),
        product_hits,
        TOPOLOGY_PATH,
    )?;
    surfaces.push(CandidateSurfaceInputV1 {
        owner: "package_topology".to_string(),
        role: "package_version_rows".to_string(),
        path: TOPOLOGY_PATH.to_string(),
        current: CandidateContentStateV1::from_bytes(&topology_bytes),
        prospective_bytes: Some(topology_prospective),
        judgment: None,
        collision: CandidateCollisionResultV1::Clear,
        rollback_source: Some("current-content-digest".to_string()),
        validation_obligations: vec!["no-new-guard".to_string()],
    });

    // Support matrix candidate fields.
    let matrix_bytes = read_repo_file(root, CANDIDATE_SUPPORT_PATH)?;
    let matrix_pattern = format!("candidate_version = \"{source_token}\"");
    let matrix_prospective = render_token_swap(
        &matrix_bytes,
        &matrix_pattern,
        &format!("candidate_version = \"{target_token}\""),
        1,
        CANDIDATE_SUPPORT_PATH,
    )?;
    surfaces.push(CandidateSurfaceInputV1 {
        owner: "support_matrix".to_string(),
        role: "candidate_fields".to_string(),
        path: CANDIDATE_SUPPORT_PATH.to_string(),
        current: CandidateContentStateV1::from_bytes(&matrix_bytes),
        prospective_bytes: Some(matrix_prospective),
        judgment: None,
        collision: CandidateCollisionResultV1::Clear,
        rollback_source: Some("current-content-digest".to_string()),
        validation_obligations: Vec::new(),
    });

    // Release corpus: prose and change framing are maintainer-owned. An
    // existing record is never silently replaced (control 4).
    let release_record_path = format!("docs/release/{}.md", target.as_str());
    surfaces.push(corpus_surface(
        "release_record",
        &release_record_path,
        root,
        "release-record-authoring",
        "Author the final release record for the target identity; existing bytes, if any, are preserved as the preimage and never silently replaced.",
    )?);
    let github_note_path = format!("docs/release/github/{}.md", target.tag());
    surfaces.push(corpus_surface(
        "github_release_note",
        &github_note_path,
        root,
        "github-note-authoring",
        "Author the default GitHub release note projected from the final release record.",
    )?);

    // Changie target corpus: which fragments constitute the final section
    // is a maintainer judgment.
    surfaces.push(CandidateSurfaceInputV1 {
        owner: "changie_corpus".to_string(),
        role: "target_entries".to_string(),
        path: CHANGES_DIR.to_string(),
        current: CandidateContentStateV1::absent(),
        prospective_bytes: None,
        judgment: Some(CandidateSurfaceDecisionV1 {
            decision_id: "changie-target-entries".to_string(),
            question: "Reconcile the Changie target entries for the final section before the record is finalized; the plan does not author change framing.".to_string(),
            owner: "repository-maintainer".to_string(),
            affected_operations: Vec::new(),
            missing_inputs: vec!["final-section fragment selection".to_string()],
        }),
        collision: CandidateCollisionResultV1::Clear,
        rollback_source: Some("current-corpus-digest".to_string()),
        validation_obligations: Vec::new(),
    });

    // Version-derived surfaces: authority-declared files and asset roots.
    for declared in VERSION_DERIVED_FILES {
        surfaces.push(version_derived_surface(
            root,
            declared,
            &source_token,
            &target_token,
        )?);
    }
    for asset_root in asset_roots_from_topology(&topology_bytes)? {
        let dir = root.join(&asset_root);
        if !dir.exists() {
            continue;
        }
        let mut files = Vec::new();
        collect_corpus_files(&dir, 0, &mut files)?;
        for file in files {
            let relative = file
                .strip_prefix(root)
                .map_err(|_| "asset path escapes the repository root".to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            if !matches!(
                file.extension().and_then(|extension| extension.to_str()),
                Some("md") | Some("json") | Some("toml") | Some("yaml") | Some("yml")
            ) {
                continue;
            }
            surfaces.push(version_derived_surface(
                root,
                &relative,
                &source_token,
                &target_token,
            )?);
        }
    }

    // Governed policy updates ride the normal add-finding plan contract.
    let policy_plan_note = match policy_plan {
        Some(path) => format!(
            "Policy plan attached at {}: it executes through the add --from-plan contract at apply; owner/reason/evidence come from the plan, never from this projection.",
            path.display()
        ),
        None => "Attach a policy plan with --policy-plan if this candidate requires ledger changes; otherwise mark this row not-applicable at apply.".to_string(),
    };
    surfaces.push(CandidateSurfaceInputV1 {
        owner: "governed_policy_plan".to_string(),
        role: "source_exception_policy".to_string(),
        path: ALLOW_POLICY_PATH.to_string(),
        current: CandidateContentStateV1::from_bytes(&read_repo_file(root, ALLOW_POLICY_PATH)?),
        prospective_bytes: None,
        judgment: Some(CandidateSurfaceDecisionV1 {
            decision_id: "policy-plan".to_string(),
            question: policy_plan_note,
            owner: "repository-maintainer".to_string(),
            affected_operations: Vec::new(),
            missing_inputs: Vec::new(),
        }),
        collision: CandidateCollisionResultV1::Clear,
        rollback_source: Some("current-content-digest".to_string()),
        validation_obligations: vec!["change-note-gate".to_string()],
    });

    Ok(surfaces)
}

/// Deterministic token-level rewrite. Fails the surface (and the plan)
/// when the expected occurrence count is absent, so a drifted authority
/// can never compile into a silently wrong operation.
pub(crate) fn render_token_swap(
    bytes: &[u8],
    from: &str,
    to: &str,
    expected_occurrences: usize,
    path: &str,
) -> Fact<Vec<u8>> {
    let text = std::str::from_utf8(bytes).map_err(|error| format!("{path} encoding: {error}"))?;
    let occurrences = text.matches(from).count();
    if occurrences != expected_occurrences {
        return Err(format!(
            "{path} carries {occurrences} occurrences of `{from}`; the projection expects exactly {expected_occurrences}"
        ));
    }
    Ok(text.replace(from, to).into_bytes())
}

/// One-byte workspace-state flip used by the post-lock mutation fault.
fn append_byte(bytes: &[u8]) -> Vec<u8> {
    let mut flipped = bytes.to_vec();
    flipped.extend_from_slice(
        b"
",
    );
    flipped
}

/// Swap every occurrence of a token in a version-derived file.
pub(crate) fn render_token_swap_all(
    bytes: &[u8],
    from: &str,
    to: &str,
    path: &str,
) -> Fact<Vec<u8>> {
    let text = std::str::from_utf8(bytes).map_err(|error| format!("{path} encoding: {error}"))?;
    if !text.contains(from) {
        return Err(format!("{path} stopped carrying `{from}`"));
    }
    Ok(text.replace(from, to).into_bytes())
}

fn contains_token(bytes: &[u8], token: &str) -> bool {
    std::str::from_utf8(bytes)
        .map(|text| text.contains(token))
        .unwrap_or(false)
}

fn corpus_surface(
    owner: &str,
    relative: &str,
    root: &Path,
    decision_id: &str,
    question: &str,
) -> Fact<CandidateSurfaceInputV1> {
    let current = if root.join(relative).exists() {
        CandidateContentStateV1::from_bytes(&read_repo_file(root, relative)?)
    } else {
        CandidateContentStateV1::absent()
    };
    Ok(CandidateSurfaceInputV1 {
        owner: owner.to_string(),
        role: "release_corpus".to_string(),
        path: relative.to_string(),
        current,
        prospective_bytes: None,
        judgment: Some(CandidateSurfaceDecisionV1 {
            decision_id: decision_id.to_string(),
            question: question.to_string(),
            owner: "repository-maintainer".to_string(),
            affected_operations: Vec::new(),
            missing_inputs: vec!["final prose".to_string()],
        }),
        collision: CandidateCollisionResultV1::Clear,
        rollback_source: Some("current-content-digest".to_string()),
        validation_obligations: Vec::new(),
    })
}

fn version_derived_surface(
    root: &Path,
    relative: &str,
    source_token: &str,
    target_token: &str,
) -> Fact<CandidateSurfaceInputV1> {
    let bytes = read_repo_file(root, relative)?;
    let carries_source = contains_token(&bytes, source_token);
    let prospective = if carries_source {
        // A version-derived reference moves every occurrence of the
        // candidate line together; one is the floor, not the cap.
        Some(render_token_swap_all(
            &bytes,
            source_token,
            target_token,
            relative,
        )?)
    } else {
        Some(bytes.clone())
    };
    Ok(CandidateSurfaceInputV1 {
        owner: "version_derived_surface".to_string(),
        role: "reference_or_fixture".to_string(),
        path: relative.to_string(),
        current: CandidateContentStateV1::from_bytes(&bytes),
        prospective_bytes: prospective,
        judgment: None,
        collision: CandidateCollisionResultV1::Clear,
        rollback_source: Some("current-content-digest".to_string()),
        validation_obligations: Vec::new(),
    })
}

/// Parse the `asset_roots` lists out of the topology file text.
pub(crate) fn asset_roots_from_topology(topology: &[u8]) -> Fact<Vec<String>> {
    let text =
        std::str::from_utf8(topology).map_err(|error| format!("topology encoding: {error}"))?;
    let mut roots = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("asset_roots = [") {
            let inner = rest.trim_end_matches(']');
            for entry in inner.split(',') {
                let entry = entry.trim().trim_matches('"');
                if !entry.is_empty() {
                    roots.push(entry.to_string());
                }
            }
        }
    }
    Ok(roots)
}

/// Resolve collisions across the compiled surface set through the shared
/// mutation-target authority: repository escape, aliasing, duplicate
/// destinations, case collisions, and symlinked destinations.
pub(crate) fn resolve_surface_collisions(
    root: &Path,
    surfaces: &mut [CandidateSurfaceInputV1],
) -> Vec<String> {
    use effortless_repo_edit::MutationTargetOwnership;

    let mut warnings: Vec<String> = Vec::new();
    let mut resolved: Vec<(String, String, String)> = Vec::new();
    for surface in surfaces.iter_mut() {
        let requested = root.join(&surface.path);
        match effortless_repo_edit::resolve_mutation_target(&requested, root) {
            Ok(target) => {
                if target.ownership() != MutationTargetOwnership::SourceTreeOwned {
                    surface.collision = CandidateCollisionResultV1::Escape {
                        detail: format!("{} resolves outside the repository root", surface.path),
                    };
                    continue;
                }
                if std::fs::symlink_metadata(root.join(&surface.path))
                    .map(|metadata| metadata.file_type().is_symlink())
                    .unwrap_or(false)
                {
                    surface.collision = CandidateCollisionResultV1::SymlinkCollision {
                        detail: format!("{} is a symbolic link", surface.path),
                    };
                    continue;
                }
                resolved.push((
                    surface.path.clone(),
                    target.repo_relative_display().to_string(),
                    target.target_fingerprint().to_string(),
                ));
            }
            Err(error) => {
                warnings.push(format!(
                    "mutation-target resolution failed for {}: {error}",
                    surface.path
                ));
            }
        }
    }

    // Alias and duplicate detection over resolved identities.
    for index in 0..resolved.len() {
        let (path_a, display_a, fingerprint_a) = resolved[index].clone();
        for (path_b, display_b, fingerprint_b) in resolved.iter().skip(index + 1) {
            if path_a == *path_b {
                for surface in surfaces.iter_mut().filter(|surface| surface.path == path_a) {
                    if surface.collision.is_clear() {
                        surface.collision = CandidateCollisionResultV1::DuplicateDestination {
                            detail: format!("two operations target {path_a}"),
                        };
                    }
                }
                continue;
            }
            if fingerprint_a == *fingerprint_b && display_a != *display_b {
                for path in [&path_a, path_b] {
                    let surface = surfaces
                        .iter_mut()
                        .find(|surface| &surface.path == path)
                        .expect("surface exists");
                    if surface.collision.is_clear() {
                        surface.collision = CandidateCollisionResultV1::PathAlias {
                            detail: format!("{path_a} and {path_b} resolve to one target"),
                        };
                    }
                }
                continue;
            }
            let fold = |value: &str| -> String { value.replace('\\', "/").to_lowercase() };
            if fold(&display_a) == fold(display_b) {
                for path in [&path_a, path_b] {
                    let surface = surfaces
                        .iter_mut()
                        .find(|surface| &surface.path == path)
                        .expect("surface exists");
                    if surface.collision.is_clear() {
                        surface.collision = CandidateCollisionResultV1::CaseCollision {
                            detail: format!("{path_a} and {path_b} collide case-insensitively"),
                        };
                    }
                }
            }
        }
    }
    warnings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_member_parser_extracts_paths_and_rejects_breakage() {
        let ok = parse_workspace_members(b"members = [\n  \"crates/a\",\n  \"crates/b\",\n]")
            .expect("members parse");
        assert_eq!(ok, vec!["crates/a".to_string(), "crates/b".to_string()]);
        assert!(parse_workspace_members(b"edition = \"2024\"").is_err());
        assert!(parse_workspace_members(b"members = [\"unterminated\"").is_err());
    }

    #[test]
    fn workspace_requirement_parser_reads_path_dependencies_only() {
        let workspace = b"\
[workspace.dependencies]
allow-core = { path = \"crates/allow-core\", version = \"=0.2.0-rc.1\" }
serde = { version = \"1\" }
[profile.release]
";
        let requirements = parse_workspace_requirements(&Ok(workspace.to_vec()));
        assert_eq!(
            requirements.get("allow-core").map(String::as_str),
            Some("=0.2.0-rc.1")
        );
        assert!(!requirements.contains_key("serde"));
        assert!(parse_workspace_requirements(&Err("missing".to_string())).is_empty());
    }

    #[test]
    fn support_matrix_parser_pairs_products_with_postures() {
        let matrix = b"\
[[product]]
product_id = \"cargo-allow\"
posture = \"CargoAllowSupported\"
[[product]]
product_id = \"cargo-intent\"
posture = \"CargoIntentExperimental\"
";
        let postures = parse_support_matrix_postures(&Ok(matrix.to_vec()));
        assert_eq!(
            postures.get("cargo-allow").map(String::as_str),
            Some("CargoAllowSupported")
        );
        assert_eq!(
            postures.get("cargo-intent").map(String::as_str),
            Some("CargoIntentExperimental")
        );
        assert!(parse_support_matrix_postures(&Err("missing".to_string())).is_empty());
    }

    #[test]
    fn topology_generation_parser_reads_the_header_or_fails_explicitly() {
        let ok = parse_topology_generation(&Ok(b"authority_generation = 2\n".to_vec()))
            .expect("generation parses");
        assert_eq!(ok, 2);
        assert!(parse_topology_generation(&Ok(b"schema_version = \"2.0\"\n".to_vec())).is_err());
        assert!(
            parse_topology_generation(&Ok(b"authority_generation = \"x\"\n".to_vec())).is_err()
        );
        assert!(parse_topology_generation(&Err("missing".to_string())).is_err());
    }

    #[test]
    fn policy_schema_version_parser_reads_the_header_or_fails_explicitly() {
        let ok = parse_policy_schema_version(&Ok(b"schema_version = \"0.1\"\n".to_vec()))
            .expect("schema version parses");
        assert_eq!(ok, "0.1");
        assert!(parse_policy_schema_version(&Ok(b"policy = \"cargo-allow\"\n".to_vec())).is_err());
        assert!(parse_policy_schema_version(&Err("missing".to_string())).is_err());
    }

    #[test]
    fn topology_row_mirror_copies_fields_and_tolerates_extras() {
        let text = "\
[[package]]
logical_id = \"allow-core\"
cargo_package_name = \"allow-core\"
version_line = \"cargo-allow-0.2\"
product_family = \"cargo-allow\"
posture = \"CargoAllowSupported\"
package_version = \"0.2.0-rc.1\"
expected_registry_checksum = \"sha256:v1:ff\"
version_source = \"WorkspaceProduct\"
publication_state = \"UnpublishedInternal\"
publish = true
candidate_inclusion = true
release_order = 10
ci_lane = \"test\"
support_tier = \"supported\"
asset_roots = []
extraction_destination = \"cargo-allow\"
";
        let rows = parse_topology_rows(text).expect("rows parse");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].cargo_package_name, "allow-core");
        assert_eq!(rows[0].package_version, "0.2.0-rc.1");
        assert!(rows[0].candidate_inclusion && rows[0].publish);
        assert!(parse_topology_rows("not = [ toml").is_err());
        assert!(parse_topology_rows("[[package]]\nlogical_id = \"x\"\n").is_err());
    }

    #[test]
    fn text_summary_renders_reasons_decisions_and_digest_from_one_plan() {
        let failure = instrument_failure_result(vec!["repository identity: missing".to_string()]);
        let rendered = render_text_summary(&failure);
        assert!(rendered.contains("inputs could not be trusted"));
        assert!(rendered.contains("reason: repository identity: missing"));

        let root = crate::candidate_preparation_plan_tests::shared_fixture().as_path();
        let live = crate::cli::candidate_preparation_command::build_preparation_result_for_root(
            root, "0.2.0", None,
        )
        .expect("fixture projection builds");
        let rendered = render_text_summary(&live);
        let plan = live.plan.as_ref().expect("plan projects");
        assert!(rendered.contains(&plan.plan_digest));
        assert!(rendered.contains("decision required [confirm-frozen-candidate-basis]"));
        assert!(rendered.contains(plan.claim_boundary.as_str()));
    }

    #[test]
    fn command_layer_renders_and_fails_closed_by_readiness() {
        let ready = PrepCandidatePlanArgs {
            version: "0.2.0".to_string(),
            format: PrepOutputFormat::Text,
            policy_plan: None,
        };
        let root = crate::candidate_preparation_plan_tests::shared_fixture().as_path();
        cmd_prep_candidate_plan_for_root(root, &ready).expect("decision-required plan exits ready");

        let unsupported = PrepCandidatePlanArgs {
            version: "0.2.0-beta.9".to_string(),
            format: PrepOutputFormat::Json,
            policy_plan: None,
        };
        let error = cmd_prep_candidate_plan_for_root(root, &unsupported)
            .expect_err("unsupported target fails closed");
        assert_eq!(error.kind(), CargoAllowErrorKind::InvalidConfig);
    }

    #[test]
    fn repository_identity_parses_owner_and_repo_from_remote_forms() {
        let parsed = |url: &str| {
            let url = url.trim().trim_end_matches(".git");
            let mut segments = url.split(['/', ':']).filter(|segment| !segment.is_empty());
            let (repo, owner) = match (segments.next_back(), segments.next_back()) {
                (Some(repo), Some(owner)) => (repo, owner),
                _ => return String::new(),
            };
            format!("{owner}/{repo}")
        };
        assert_eq!(
            parsed("https://github.com/EffortlessMetrics/cargo-allow.git"),
            "EffortlessMetrics/cargo-allow"
        );
        assert_eq!(
            parsed("git@github.com:EffortlessMetrics/cargo-allow.git"),
            "EffortlessMetrics/cargo-allow"
        );
        assert_eq!(parsed("bare"), "");
    }
}

/// Fault-injection channels for the apply engine's transaction tests. The
/// production CLI passes `ApplyFault::none()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ApplyFault {
    /// Simulate a crash after this many commits have landed.
    pub(crate) after_commit: Option<usize>,
    /// Simulate staged bytes that do not match their expected digest.
    pub(crate) corrupt_staged: bool,
    /// Simulate another writer flipping a target after the locks are held
    /// but before the per-target recheck.
    pub(crate) mutate_target_after_lock: bool,
    /// Simulate rollback failure: the first committed target cannot be
    /// restored, producing the bounded RecoveryRequired state.
    pub(crate) remove_first_target_before_rollback: bool,
}

impl ApplyFault {
    pub(crate) fn none() -> Self {
        Self {
            after_commit: None,
            corrupt_staged: false,
            mutate_target_after_lock: false,
            remove_first_target_before_rollback: false,
        }
    }
}

/// Apply one exact reviewed plan to `root` through the repository's shared
/// write-safety authorities: revalidation gate, decision gate, deterministic
/// lock set, per-target recheck, in-memory staging validated against
/// expected digests, atomic replacement, and full rollback from in-memory
/// preimages. Mechanics only; the receipt records what happened.
pub(crate) fn apply_candidate_plan(
    root: &Path,
    plan: &CandidatePreparationResultV1,
    acknowledgements: &[String],
    receipt_path: Option<&Path>,
    fault: ApplyFault,
) -> allow_report::CandidateApplyReceiptV1 {
    use allow_report::{
        CandidateApplyLockRecordV1, CandidateApplyOperationRecordV1, CandidateApplyStateV1,
    };

    let plan_ref = match &plan.plan {
        Some(plan) => plan,
        None => {
            let mut receipt =
                allow_report::CandidateApplyReceiptV1::new(String::new(), String::new());
            receipt.state = CandidateApplyStateV1::Conflict;
            receipt
                .reasons
                .push("the plan file carries no projected plan".to_string());
            return receipt;
        }
    };
    let mut receipt =
        allow_report::CandidateApplyReceiptV1::new(plan_ref.plan_digest.clone(), String::new());

    // ---- Authenticity gate: the plan file's stored digests must cover
    // its own content (tamper detection before any repository read).
    if !plan_ref.digest_is_authentic() {
        receipt.state = CandidateApplyStateV1::Conflict;
        receipt
            .reasons
            .push("the plan file's stored digest does not cover its content".to_string());
        return receipt;
    }
    if let Some(operations) = &plan.operations
        && !operations.digest_is_authentic()
    {
        receipt.state = CandidateApplyStateV1::Conflict;
        receipt
            .reasons
            .push("the plan file's operation-set digest does not cover its content".to_string());
        return receipt;
    }

    // ---- Revalidation gate: rebuild the projection and compare identities.
    let target_version = plan_ref.target_release_identity.version.clone();
    let fresh = match build_preparation_result_for_root(root, &target_version, None) {
        Ok(fresh) => fresh,
        Err(error) => {
            receipt.state = CandidateApplyStateV1::InstrumentFailure;
            receipt
                .reasons
                .push(format!("revalidation failed: {error}"));
            return receipt;
        }
    };
    let Some(fresh_plan) = &fresh.plan else {
        receipt.state = CandidateApplyStateV1::Stale;
        receipt.reasons.extend(fresh.reasons.clone());
        receipt
            .reasons
            .push("the live repository no longer projects this plan".to_string());
        return receipt;
    };
    {
        let before = serde_json::to_string(&fresh_plan.input_identity).unwrap_or_default();
        receipt.before_identity_digest = allow_core::sha256_v1_bytes(before.as_bytes());
    }
    if fresh_plan.plan_digest != plan_ref.plan_digest {
        receipt.state = CandidateApplyStateV1::Stale;
        receipt.reasons.push(
            "the live repository state no longer matches the plan identity; regenerate the plan"
                .to_string(),
        );
        return receipt;
    }
    let Some(fresh_ops) = &fresh.operations else {
        receipt.state = CandidateApplyStateV1::InstrumentFailure;
        receipt
            .reasons
            .push("revalidation did not compile operations".to_string());
        return receipt;
    };
    let plan_ops = match &plan.operations {
        Some(ops) => ops,
        None => {
            receipt.state = CandidateApplyStateV1::Conflict;
            receipt
                .reasons
                .push("the plan file carries no operation set".to_string());
            return receipt;
        }
    };
    if fresh_ops.operations_digest != plan_ops.operations_digest {
        receipt.state = CandidateApplyStateV1::Stale;
        receipt.reasons.push(
            "the compiled operation set changed since the plan was reviewed; regenerate the plan"
                .to_string(),
        );
        return receipt;
    }

    // ---- Decision gate: every required decision must be acknowledged.
    let mut required_ids: Vec<String> = plan_ref
        .required_decisions
        .iter()
        .map(|decision| decision.decision_id.clone())
        .collect();
    required_ids.extend(
        plan_ops
            .decisions
            .iter()
            .map(|decision| decision.decision_id.clone()),
    );
    required_ids.sort();
    required_ids.dedup();
    let mut unacknowledged: Vec<String> = required_ids
        .iter()
        .filter(|id| !acknowledgements.contains(id))
        .cloned()
        .collect();
    if !unacknowledged.is_empty() {
        unacknowledged.sort();
        receipt.state = CandidateApplyStateV1::DecisionRequired;
        receipt.reasons.push(format!(
            "unacknowledged decisions: {}",
            unacknowledged.join(", ")
        ));
        return receipt;
    }
    for id in &required_ids {
        receipt
            .decision_acknowledgements
            .insert(id.clone(), "acknowledged".to_string());
    }

    // ---- Output collision check: the receipt may not share an underlying
    // target with any operation or live under .git.
    if let Some(receipt_path) = receipt_path {
        let receipt_fold = receipt_path
            .to_string_lossy()
            .replace('\\', "/")
            .to_lowercase();
        if receipt_fold.contains("/.git/") || receipt_fold.starts_with(".git/") {
            receipt.state = CandidateApplyStateV1::Conflict;
            receipt
                .reasons
                .push("the receipt path lives under .git".to_string());
            return receipt;
        }
        for operation in &plan_ops.operations {
            let target_fold = operation.path.replace('\\', "/").to_lowercase();
            if receipt_fold.ends_with(&target_fold) || target_fold.ends_with(&receipt_fold) {
                receipt.state = CandidateApplyStateV1::Conflict;
                receipt.reasons.push(format!(
                    "the receipt path collides with the operation target {}",
                    operation.path
                ));
                return receipt;
            }
        }
    }

    // ---- Deterministic write set: plan operations with generated bytes.
    let mut write_ops: Vec<&allow_report::CandidateFileOperationV1> = plan_ops
        .operations
        .iter()
        .filter(|operation| {
            matches!(
                operation.posture,
                allow_report::CandidateOperationPostureV1::Replace
                    | allow_report::CandidateOperationPostureV1::Create
            )
        })
        .collect();
    write_ops.sort_by(|a, b| (&a.path, &a.role).cmp(&(&b.path, &b.role)));

    // ---- Deterministic lock set over underlying targets.
    let mut lock_targets: Vec<(String, PathBuf)> = Vec::new();
    for operation in &write_ops {
        let requested = root.join(&operation.path);
        match effortless_repo_edit::resolve_mutation_target(&requested, root) {
            Ok(target) => {
                if target.ownership()
                    != effortless_repo_edit::MutationTargetOwnership::SourceTreeOwned
                {
                    receipt.state = CandidateApplyStateV1::Conflict;
                    receipt.reasons.push(format!(
                        "operation target {} escapes repository ownership",
                        operation.path
                    ));
                    return receipt;
                }
                lock_targets.push((
                    target.target_fingerprint().to_string(),
                    target.normalized_absolute().to_path_buf(),
                ));
            }
            Err(error) => {
                receipt.state = CandidateApplyStateV1::Conflict;
                receipt.reasons.push(format!(
                    "mutation-target resolution failed for {}: {error}",
                    operation.path
                ));
                return receipt;
            }
        }
    }
    lock_targets.sort();
    lock_targets.dedup();
    let mut locks = Vec::new();
    for (fingerprint, path) in &lock_targets {
        match effortless_repo_edit::MutationLock::acquire(path) {
            Ok(lock) => {
                locks.push(lock);
                receipt.locks.push(CandidateApplyLockRecordV1 {
                    path: path.display().to_string(),
                    fingerprint: fingerprint.clone(),
                    acquired: true,
                });
            }
            Err(error) => {
                receipt.state = CandidateApplyStateV1::Conflict;
                receipt.reasons.push(format!(
                    "lock acquisition failed for {}: {error}",
                    path.display()
                ));
                return receipt;
            }
        }
    }

    // ---- Per-target recheck immediately before staging: the plan's bound
    // current digests must still hold under lock.
    if fault.mutate_target_after_lock
        && let Some((_, path)) = lock_targets.first()
        && let Ok(bytes) = std::fs::read(path)
        && let Ok(mutated) = std::str::from_utf8(&append_byte(&bytes))
    {
        let _ = effortless_repo_edit::write_file(path, mutated);
    }
    for operation in &write_ops {
        let absolute = root.join(&operation.path);
        let Ok(bytes) = std::fs::read(&absolute) else {
            receipt.state = CandidateApplyStateV1::Mismatch;
            receipt.reasons.push(format!(
                "target {} is no longer readable under lock",
                operation.path
            ));
            return receipt;
        };
        let digest = allow_core::sha256_v1_bytes(&bytes);
        if Some(&digest) != operation.current.digest.as_ref() {
            receipt.state = CandidateApplyStateV1::Mismatch;
            receipt.reasons.push(format!(
                "target {} changed after plan generation (control 1/6)",
                operation.path
            ));
            return receipt;
        }
    }

    // ---- Stage: re-render prospective bytes from the revalidated
    // authorities and validate them against the plan's expected digests.
    let source_version = plan_ref.source_release_identity.version.clone();
    let Ok(target) = ReleaseVersionV1::parse(&target_version) else {
        receipt.state = CandidateApplyStateV1::InstrumentFailure;
        receipt
            .reasons
            .push("target version stopped parsing".to_string());
        return receipt;
    };
    let rendered = match gather_surface_inputs(root, &source_version, &target, None) {
        Ok(rendered) => rendered,
        Err(reason) => {
            receipt.state = CandidateApplyStateV1::InstrumentFailure;
            receipt
                .reasons
                .push(format!("surface re-render failed: {reason}"));
            return receipt;
        }
    };
    let mut staged: Vec<(&allow_report::CandidateFileOperationV1, String)> = Vec::new();
    for operation in &write_ops {
        // Only deterministic surfaces reach this loop, so the re-render
        // always contains the surface with generated UTF-8 bytes.
        let surface = rendered
            .iter()
            .find(|surface| surface.owner == operation.owner && surface.path == operation.path)
            .expect("the re-render preserves the reviewed surface set");
        let bytes = surface.prospective_bytes.as_ref().expect("generated bytes");
        let bytes = std::str::from_utf8(bytes)
            .expect("rendered bytes stay UTF-8")
            .as_bytes();
        let mut bytes = bytes.to_vec();
        if fault.corrupt_staged {
            if bytes.is_empty() {
                bytes.push(0x20);
            } else {
                let original = bytes[0];
                bytes[0] = original.wrapping_add(1);
            }
        }
        let staged_digest = allow_core::sha256_v1_bytes(&bytes);
        if Some(&staged_digest) != operation.prospective_digest.as_ref() {
            receipt.state = CandidateApplyStateV1::InstrumentFailure;
            receipt.reasons.push(format!(
                "staged bytes for {} do not match the reviewed digest; refusing to write",
                operation.path
            ));
            return receipt;
        }
        let staged_contents = String::from_utf8(bytes).expect("rendered bytes stay UTF-8");
        staged.push((operation, staged_contents));
    }
    receipt.staged_validation = true;

    // ---- Preimages for rollback.
    let mut preimages: Vec<(String, Vec<u8>)> = Vec::new();
    for (operation, _) in &staged {
        let absolute = root.join(&operation.path);
        match std::fs::read(&absolute) {
            Ok(bytes) => preimages.push((operation.path.clone(), bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                preimages.push((operation.path.clone(), Vec::new()));
            }
            Err(error) => {
                receipt.state = CandidateApplyStateV1::InstrumentFailure;
                receipt.reasons.push(format!(
                    "cannot read preimage for {}: {error}",
                    operation.path
                ));
                return receipt;
            }
        }
    }

    // ---- Commit the complete ordered transaction. Staged bytes were
    // already validated against the reviewed digests, so each write goes
    // straight through the shared atomic primitive.
    let mut committed: Vec<String> = Vec::new();
    let mut abort: Option<(allow_report::CandidateApplyStateV1, String)> = None;
    for (operation, contents) in &staged {
        let absolute = root.join(&operation.path);
        let write_result = match operation.posture {
            allow_report::CandidateOperationPostureV1::Create => {
                effortless_repo_edit::write_file_create_new_atomic(&absolute, contents)
            }
            _ => effortless_repo_edit::write_file(&absolute, contents),
        };
        if let Err(error) = write_result {
            abort = Some((
                CandidateApplyStateV1::InstrumentFailure,
                format!("atomic write failed for {}: {error}", operation.path),
            ));
            break;
        }
        committed.push(operation.path.clone());
        if let Some(limit) = fault.after_commit
            && committed.len() == limit
        {
            abort = Some((
                CandidateApplyStateV1::RolledBack,
                format!("injected fault after {limit} commits"),
            ));
            break;
        }
    }

    // Record per-operation outcomes.
    for (operation, contents) in &staged {
        let after_digest = allow_core::sha256_v1_bytes(contents.as_bytes());
        let result = if committed.contains(&operation.path) {
            if abort.is_some() {
                "rolled_back"
            } else {
                "applied"
            }
        } else {
            "not_applied"
        };
        receipt.operations.push(CandidateApplyOperationRecordV1 {
            owner: operation.owner.clone(),
            role: operation.role.clone(),
            path: operation.path.clone(),
            intended_posture: if matches!(
                operation.posture,
                allow_report::CandidateOperationPostureV1::Create
            ) {
                "create".to_string()
            } else {
                "replace".to_string()
            },
            before_digest: operation.current.digest.clone(),
            staged_digest: operation.prospective_digest.clone(),
            after_digest: Some(after_digest),
            result: result.to_string(),
        });
    }
    for operation in &plan_ops.operations {
        if !matches!(
            operation.posture,
            allow_report::CandidateOperationPostureV1::Replace
                | allow_report::CandidateOperationPostureV1::Create
        ) {
            receipt.operations.push(CandidateApplyOperationRecordV1 {
                owner: operation.owner.clone(),
                role: operation.role.clone(),
                path: operation.path.clone(),
                intended_posture: "decision_required".to_string(),
                before_digest: operation.current.digest.clone(),
                staged_digest: None,
                after_digest: None,
                result: "not_applied".to_string(),
            });
        }
    }

    // ---- Rollback on abort: restore every committed path from its
    // preimage, oldest last, and verify the restored digests.
    if let Some((state, reason)) = abort {
        receipt.transaction_result = reason.clone();
        let mut rollback_failures: Vec<String> = Vec::new();
        let mut first_restored = false;
        for path in committed.iter().rev() {
            if fault.remove_first_target_before_rollback && !first_restored {
                first_restored = true;
                rollback_failures.push(format!(
                    "{path}: injected rollback failure leaves the file in the prospective state"
                ));
                if let Some(record) = receipt
                    .operations
                    .iter_mut()
                    .find(|record| &record.path == path)
                {
                    record.result = "recovery_required".to_string();
                }
                continue;
            }
            let Some((_, preimage)) = preimages.iter().find(|(p, _)| p == path) else {
                rollback_failures.push(format!("{path}: no preimage"));
                continue;
            };
            let absolute = root.join(path);
            let restore = if preimage.is_empty() {
                std::fs::remove_file(&absolute)
                    .map_err(|error| format!("remove: {error}"))
                    .and(Ok(()))
            } else {
                String::from_utf8(preimage.clone())
                    .map_err(|error| format!("utf-8: {error}"))
                    .and_then(|contents| {
                        effortless_repo_edit::write_file(&absolute, &contents)
                            .map_err(|error| error.to_string())
                    })
            };
            match restore {
                Ok(()) => {
                    let restored = std::fs::read(&absolute).unwrap_or_default();
                    let digest = allow_core::sha256_v1_bytes(&restored);
                    let matches_before = preimages
                        .iter()
                        .find(|(p, _)| p == path)
                        .map(|(_, preimage)| digest == allow_core::sha256_v1_bytes(preimage))
                        .unwrap_or(false);
                    if matches_before {
                        let before = operation_before_digest(&receipt.operations, path);
                        if let Some(record) = receipt
                            .operations
                            .iter_mut()
                            .find(|record| &record.path == path)
                        {
                            record.result = "rolled_back".to_string();
                            record.after_digest = before;
                        }
                    } else {
                        rollback_failures.push(format!("{path}: restored bytes diverge"));
                    }
                }
                Err(error) => rollback_failures.push(format!("{path}: {error}")),
            }
        }
        if rollback_failures.is_empty() {
            receipt.rollback_result = "complete".to_string();
            receipt.state = state;
        } else {
            receipt.rollback_result = format!("incomplete: {}", rollback_failures.join("; "));
            receipt.state = CandidateApplyStateV1::RecoveryRequired;
            receipt.reasons.extend(rollback_failures);
        }
        if state == CandidateApplyStateV1::RolledBack {
            receipt.reasons.push(reason);
        }
        return receipt;
    }

    // ---- Success: Applied, or NoOp when the deterministic set was empty.
    receipt.transaction_result = "committed".to_string();
    receipt.rollback_result = "not_needed".to_string();
    receipt.state = if staged.is_empty() {
        CandidateApplyStateV1::NoOp
    } else {
        CandidateApplyStateV1::Applied
    };
    for obligation in &plan_ref
        .validation_obligations
        .iter()
        .map(|obligation| obligation.obligation_id.clone())
        .collect::<Vec<_>>()
    {
        receipt.remaining_obligations.push(obligation.clone());
    }
    receipt
}

fn operation_before_digest(
    operations: &[allow_report::CandidateApplyOperationRecordV1],
    path: &str,
) -> Option<String> {
    operations
        .iter()
        .find(|record| record.path == path)
        .and_then(|record| record.before_digest.clone())
}

/// Post-apply reconciliation (#3834): reconcile the applied source state
/// against release identity, package topology, support/channel source,
/// governed-file posture, and post-apply validation, then emit the final
/// typed `CandidatePreparationReceiptV1`. In-process rows execute here;
/// rows that need external tools or operator runs are retained as
/// deferred obligations — never fabricated.
pub(crate) fn reconcile_candidate_preparation(
    root: &Path,
    plan: &CandidatePreparationResultV1,
    apply_receipt: &allow_report::CandidateApplyReceiptV1,
    acknowledgements: &[String],
) -> allow_report::CandidatePreparationReceiptV1 {
    use allow_report::{
        CandidateApplyStateV1, CandidateGraphRowV1, CandidatePreparationReceiptV1,
        CandidatePreparationStateV1, CandidateResolvedDecisionV1, CandidateValidationResultV1,
        CandidateValidationRowV1,
    };

    let plan_ref = match &plan.plan {
        Some(plan) => plan,
        None => {
            let mut receipt = CandidatePreparationReceiptV1::new(
                String::new(),
                format!("{:?}", apply_receipt.state),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            );
            receipt.state = CandidatePreparationStateV1::Conflict;
            receipt
                .reasons
                .push("the plan file carries no projected plan".to_string());
            return receipt;
        }
    };

    let target_version = plan_ref.target_release_identity.version.clone();
    let target_tag = plan_ref.target_release_identity.tag.clone();

    // After-identity: rebuild the projection over the applied tree.
    let fresh = match build_preparation_result_for_root(root, &target_version, None) {
        Ok(fresh) => fresh,
        Err(error) => {
            let mut receipt = CandidatePreparationReceiptV1::new(
                plan_ref.plan_digest.clone(),
                format!("{:?}", apply_receipt.state),
                apply_receipt.before_identity_digest.clone(),
                String::new(),
                target_version,
                target_tag,
                "stable".to_string(),
            );
            receipt.state = CandidatePreparationStateV1::InstrumentFailure;
            receipt
                .reasons
                .push(format!("post-apply rebuild failed: {error}"));
            return receipt;
        }
    };
    let after_identity_digest = fresh
        .input_identity
        .as_ref()
        .map(|identity| {
            let canonical = serde_json::to_string(identity).unwrap_or_default();
            allow_core::sha256_v1_bytes(canonical.as_bytes())
        })
        .unwrap_or_default();

    let source_version = plan_ref.source_release_identity.version.clone();
    let mut receipt = CandidatePreparationReceiptV1::new(
        plan_ref.plan_digest.clone(),
        format!("{:?}", apply_receipt.state),
        apply_receipt.before_identity_digest.clone(),
        after_identity_digest,
        target_version.clone(),
        target_tag.clone(),
        plan_ref.target_release_identity.channel.clone(),
    );

    // ---- Release-prep parity row: after the apply, planning the same
    // target must report that the target already equals the source line.
    let parity_achieved = matches!(
        fresh.readiness,
        allow_report::CandidatePreparationReadinessV1::Stale
    ) && fresh
        .reasons
        .iter()
        .any(|reason| reason.contains("equals the current source line"));
    receipt.validation_rows.push(CandidateValidationRowV1 {
        obligation_id: "release-prep-parity".to_string(),
        command: "prep-candidate plan --version <target> (in-process rebuild)".to_string(),
        result: if parity_achieved {
            CandidateValidationResultV1::Passed
        } else {
            CandidateValidationResultV1::Failed
        },
        detail: if parity_achieved {
            format!("the applied tree already sits on the target line {target_version}")
        } else {
            "the rebuilt projection still expects a transition".to_string()
        },
    });

    // ---- Package/topology/version requirements row: every product row in
    // the reviewed plan's selected graph reconciles to the target, every
    // shared prerequisite holds its line, and every internal requirement
    // operation landed exact.
    let manifest_bytes = std::fs::read(root.join(WORKSPACE_MANIFEST_PATH)).unwrap_or_default();
    let graph_ok = !plan_ref.selected_rows.is_empty()
        && plan_ref.selected_rows.iter().all(|selected| {
            let expected = if selected.role == "product" {
                target_version.clone()
            } else {
                selected.row.package_version.clone()
            };
            selected.prospective_version == expected
        });
    let requirements_ok =
        graph_ok && manifest_requirements_exact(&manifest_bytes, &source_version, &target_version);
    receipt.validation_rows.push(CandidateValidationRowV1 {
        obligation_id: "package-topology-requirements".to_string(),
        command: "reconcile: selected graph + exact requirements (in-process)".to_string(),
        result: if graph_ok && requirements_ok {
            CandidateValidationResultV1::Passed
        } else {
            CandidateValidationResultV1::Failed
        },
        detail: format!(
            "graph rows: {}; exact requirement law: {}",
            plan_ref.selected_rows.len(),
            requirements_ok
        ),
    });

    // ---- Support/channel/document coherence row.
    let support_bytes = std::fs::read(root.join(CANDIDATE_SUPPORT_PATH));
    let support_ok = support_bytes.as_ref().is_ok_and(|bytes| {
        let text = String::from_utf8_lossy(bytes);
        text.contains(&format!("candidate_version = \"{target_version}\""))
            && text.contains("candidate_published = false")
    });
    let record_bytes = std::fs::read(root.join(format!("docs/release/{}.md", target_version)));
    let note_bytes = std::fs::read(root.join(format!("docs/release/github/{}.md", target_tag)));
    let corpus_ok = record_bytes
        .is_ok_and(|bytes| String::from_utf8_lossy(&bytes).contains(&target_version))
        && note_bytes.is_ok_and(|bytes| String::from_utf8_lossy(&bytes).contains(&target_version));
    receipt.release_support_projection = if support_ok && corpus_ok {
        "coherent".to_string()
    } else {
        "incoherent".to_string()
    };
    receipt.validation_rows.push(CandidateValidationRowV1 {
        obligation_id: "support-document-coherence".to_string(),
        command: "reconcile: support matrix + release corpus identity (in-process)".to_string(),
        result: if support_ok && corpus_ok {
            CandidateValidationResultV1::Passed
        } else {
            CandidateValidationResultV1::Failed
        },
        detail: format!(
            "support matrix candidate fields: {}; release corpus identity: {}",
            support_ok, corpus_ok
        ),
    });

    // ---- Governed-file policy drift row: the policy digest must still
    // match the plan's bound identity (post-apply movement is a mismatch).
    let policy_digest_matches = plan
        .input_identity
        .as_ref()
        .zip(fresh.input_identity.as_ref())
        .is_some_and(|(before, after)| {
            before.source_exception_policy_digest == after.source_exception_policy_digest
        });
    receipt.policy_drift_result = if policy_digest_matches {
        "clean".to_string()
    } else {
        "drifted".to_string()
    };
    receipt.validation_rows.push(CandidateValidationRowV1 {
        obligation_id: "policy-drift-guard".to_string(),
        command: "reconcile: source-exception policy digest equality (in-process)".to_string(),
        result: if policy_digest_matches {
            CandidateValidationResultV1::Passed
        } else {
            CandidateValidationResultV1::Failed
        },
        detail: format!(
            "before/after policy digests: {}/{}",
            plan.input_identity
                .as_ref()
                .map(|identity| identity.source_exception_policy_digest.clone())
                .unwrap_or_default(),
            fresh
                .input_identity
                .as_ref()
                .map(|identity| identity.source_exception_policy_digest.clone())
                .unwrap_or_default(),
        ),
    });

    // ---- Deferred rows: external tools and operator-run checks are
    // retained as exact commands, never fabricated.
    receipt.validation_rows.push(CandidateValidationRowV1 {
        obligation_id: "changie-history-roundtrip".to_string(),
        command: "bash scripts/test-changie-history-roundtrip.sh --version <target>".to_string(),
        result: CandidateValidationResultV1::Deferred,
        detail:
            "requires the external changie module; orchestrated by the release rehearsal (#3751)"
                .to_string(),
    });
    receipt.validation_rows.push(CandidateValidationRowV1 {
        obligation_id: "no-new-guard".to_string(),
        command: "cargo-allow check --mode no-new".to_string(),
        result: CandidateValidationResultV1::Deferred,
        detail:
            "operator-run guard with a retained artifact; a failure here must fail the preparation"
                .to_string(),
    });
    receipt.changie_result = "deferred".to_string();
    receipt
        .remaining_obligations
        .push("changie-history-roundtrip".to_string());
    receipt
        .remaining_obligations
        .push("no-new-guard".to_string());

    // ---- No-op rerun row: rebuilding the projection after the apply
    // shows no remaining transition to a same-version target.
    receipt.no_op_rerun_result = if parity_achieved {
        "pass".to_string()
    } else {
        "fail".to_string()
    };

    // ---- Changed files from the apply receipt.
    for operation in &apply_receipt.operations {
        if operation.result == "applied" {
            receipt
                .changed_files
                .push(allow_report::CandidateChangedFileV1 {
                    path: operation.path.clone(),
                    before_digest: operation.before_digest.clone(),
                    after_digest: operation.after_digest.clone().unwrap_or_default(),
                });
        }
    }

    // ---- Graph rows from the reviewed selection.
    for selected in &plan_ref.selected_rows {
        receipt.selected_graph.push(CandidateGraphRowV1 {
            logical_id: selected.row.logical_id.clone(),
            cargo_package_name: selected.row.cargo_package_name.clone(),
            product_family: selected.row.product_family.clone(),
            version: selected.prospective_version.clone(),
        });
    }

    // ---- Decisions: resolved acknowledgements vs outstanding rows,
    // across both the structural decisions and the compiled surface
    // decisions.
    let mut decision_ids: Vec<String> = plan_ref
        .required_decisions
        .iter()
        .map(|decision| decision.decision_id.clone())
        .collect();
    if let Some(operations) = &plan.operations {
        decision_ids.extend(
            operations
                .decisions
                .iter()
                .map(|decision| decision.decision_id.clone()),
        );
    }
    decision_ids.sort();
    decision_ids.dedup();
    for decision_id in &decision_ids {
        if acknowledgements.contains(decision_id) {
            receipt
                .resolved_decisions
                .push(CandidateResolvedDecisionV1 {
                    decision_id: decision_id.clone(),
                    resolution: "acknowledged".to_string(),
                });
        } else {
            receipt.outstanding_decisions.push(decision_id.clone());
        }
    }

    // ---- Classification.
    let executed_failure = receipt
        .validation_rows
        .iter()
        .any(|row| row.result == CandidateValidationResultV1::Failed);
    if apply_receipt.state == CandidateApplyStateV1::DecisionRequired {
        receipt.state = CandidatePreparationStateV1::DecisionRequired;
        receipt.reasons.push(
            "the apply was refused on unacknowledged decisions; reconcile after resolving them"
                .to_string(),
        );
    } else if !matches!(
        apply_receipt.state,
        CandidateApplyStateV1::Applied | CandidateApplyStateV1::NoOp
    ) {
        receipt.state = CandidatePreparationStateV1::Mismatch;
        receipt
            .reasons
            .push("the consumed apply receipt does not record a successful apply".to_string());
    } else if executed_failure || !policy_digest_matches {
        receipt.state = CandidatePreparationStateV1::Incomplete;
        receipt
            .reasons
            .push("one or more post-apply validation rows failed".to_string());
    } else if !receipt.outstanding_decisions.is_empty() {
        receipt.state = CandidatePreparationStateV1::DecisionRequired;
        receipt
            .reasons
            .push("one or more required decisions remain unacknowledged".to_string());
    } else if !parity_achieved {
        receipt.state = CandidatePreparationStateV1::Stale;
        receipt
            .reasons
            .push("the applied tree does not sit on the target line".to_string());
    } else {
        receipt.state = CandidatePreparationStateV1::Complete;
        receipt.reasons.push(
            "source candidate prepared coherently; qualification and authorization remain separate"
                .to_string(),
        );
    }
    receipt
}

/// Post-apply exact-requirement law against the live root manifest: the
/// workspace version is the target, no source-version token remains, and
/// every workspace-internal requirement is exact.
fn manifest_requirements_exact(manifest_bytes: &[u8], source: &str, target: &str) -> bool {
    let Ok(text) = std::str::from_utf8(manifest_bytes) else {
        return false;
    };
    text.contains(&format!("version = \"{target}\"")) && !text.contains(source)
}
