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
}

/// Output rendering for the preparation result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum PrepOutputFormat {
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
    format: PrepOutputFormat,
    /// Optional add-finding plan supplying governed policy changes.
    #[arg(long)]
    policy_plan: Option<PathBuf>,
}

pub(super) fn cmd_prep_candidate(args: &PrepCandidateArgs) -> CargoAllowResult<()> {
    match &args.command {
        PrepCandidateSubcommand::Plan(plan_args) => cmd_prep_candidate_plan(plan_args),
    }
}

fn cmd_prep_candidate_plan(args: &PrepCandidatePlanArgs) -> CargoAllowResult<()> {
    let result = build_preparation_result(&args.version, args.policy_plan.as_deref())?;
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
        schema: allow_report::CANDIDATE_PREPARATION_RESULT_SCHEMA_V1,
        readiness: CandidatePreparationReadinessV1::InstrumentFailure,
        reasons,
        input_identity: None,
        plan: None,
        operations: None,
        human_summary: "candidate preparation inputs could not be trusted".to_string(),
    }
}

pub(crate) fn build_preparation_result(
    target_version: &str,
    policy_plan: Option<&Path>,
) -> CargoAllowResult<CandidatePreparationResultV1> {
    let root = match git_root() {
        Ok(root) => root,
        Err(reason) => {
            return Ok(instrument_failure_result(vec![format!(
                "repository worktree: {reason}"
            )]));
        }
    };

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
        repository_identity(&root),
        &mut failures,
    );
    let branch = collect(
        "branch",
        git_text(&root, &["rev-parse", "--abbrev-ref", "HEAD"]),
        &mut failures,
    );
    let head_commit = collect(
        "HEAD commit",
        git_text(&root, &["rev-parse", "HEAD"]),
        &mut failures,
    );
    let tree = collect(
        "HEAD tree",
        git_text(&root, &["rev-parse", "HEAD^{tree}"]),
        &mut failures,
    );
    let dirty_state: Option<CandidatePreparationDirtyStateV1> = match dirty_state_class(&root) {
        Ok(dirty_state) => Some(dirty_state),
        Err(reason) => {
            failures.push(format!("working-tree state: {reason}"));
            None
        }
    };

    let cargo_lock_digest = collect(
        "Cargo.lock digest",
        file_digest(&root, CARGO_LOCK_PATH),
        &mut failures,
    );
    let workspace_manifest_digest = collect(
        "workspace manifest digest",
        file_digest(&root, WORKSPACE_MANIFEST_PATH),
        &mut failures,
    );
    let topology_bytes = read_repo_file(&root, TOPOLOGY_PATH);
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
        file_digest(&root, SUPPORT_MATRIX_PATH),
        &mut failures,
    );
    let changie_config_digest = collect(
        "Changie configuration digest",
        file_digest(&root, CHANGIE_CONFIG_PATH),
        &mut failures,
    );
    let changie_history_digest = collect(
        "Changie history corpus digest",
        changie_history_digest(&root),
        &mut failures,
    );
    let allow_policy_bytes = read_repo_file(&root, ALLOW_POLICY_PATH);
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
    let member_manifest_digests = match member_manifest_digests(&root) {
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
                &root,
                &format!("docs/release/{}.md", target.as_str()),
                &mut failures,
            );
            let github_release_note = corpus_source(
                &root,
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
        parse_support_matrix_postures(&read_repo_file(&root, SUPPORT_MATRIX_PATH));
    let internal_requirements =
        parse_workspace_requirements(&read_repo_file(&root, WORKSPACE_MANIFEST_PATH));

    let mut external_observations = vec![CandidateExternalObservationV1 {
        observation_id: "public_prerelease_line".to_string(),
        subject: "0.2.0-rc.1".to_string(),
        detail: "Public rc.1 is usable pilot evidence with incident lineage; not reusable as final package bytes (#3768 claim boundaries). Inputs only.".to_string(),
    }];
    if let Ok(digest) = file_digest(&root, INCIDENT_EVIDENCE_PATH) {
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
        match gather_surface_inputs(&root, &source_version, &target, policy_plan) {
            Ok(mut surfaces) => {
                let warnings = resolve_surface_collisions(&root, &mut surfaces);
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
fn render_token_swap(
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

/// Swap every occurrence of a token in a version-derived file.
fn render_token_swap_all(bytes: &[u8], from: &str, to: &str, path: &str) -> Fact<Vec<u8>> {
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
fn asset_roots_from_topology(topology: &[u8]) -> Fact<Vec<String>> {
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

        let live =
            crate::cli::candidate_preparation_command::build_preparation_result("0.2.0", None)
                .expect("live projection builds");
        let rendered = render_text_summary(&live);
        let plan = live.plan.as_ref().expect("plan projects");
        assert!(rendered.contains(&plan.plan_digest));
        assert!(rendered.contains("decision required [confirm-frozen-candidate-basis]"));
        assert!(rendered.contains(plan.claim_boundary));
    }

    #[test]
    fn command_layer_renders_and_fails_closed_by_readiness() {
        let ready = PrepCandidatePlanArgs {
            version: "0.2.0".to_string(),
            format: PrepOutputFormat::Text,
            policy_plan: None,
        };
        cmd_prep_candidate_plan(&ready).expect("decision-required plan exits ready");

        let unsupported = PrepCandidatePlanArgs {
            version: "0.2.0-beta.9".to_string(),
            format: PrepOutputFormat::Json,
            policy_plan: None,
        };
        let error =
            cmd_prep_candidate_plan(&unsupported).expect_err("unsupported target fails closed");
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
