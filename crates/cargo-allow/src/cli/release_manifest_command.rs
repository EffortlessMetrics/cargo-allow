//! Typed ReleaseManifestV2 producer and validator for the release workflow
//! (#2497).
//!
//! This command is the manifest's semantic authority: it reads the exact
//! topology publication receipt (plus optional binary receipts), reconciles
//! every published package row through the typed publication adapter
//! (#3761), assembles the governed ReleaseManifestEnvelopeV2, and re-validates
//! the exact written bytes with `validate_release_manifest_v2`. The emitted
//! typed result receipt is the only progression evidence: the workflow gates
//! attestation and release creation on it. Shell orchestration stages files
//! and invokes this command; it never decides version, equality, or result.

use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult, SimpleDate};
use allow_report::{
    ConsumedEvidenceV1, PackageRowClassV1, PublicationStateV1, ReconciledPackagePublicationV1,
    ReleaseManifestAuthenticationV2, ReleaseManifestEnvelopeV2, ReleaseManifestOperationV2,
    ReleaseManifestPackageRowV2, ReleaseManifestPayloadV2, ReleaseManifestPublicationPostureV2,
    ReleaseManifestResultV2, ReleaseManifestSupportPostureV2, ReleaseVersionV1,
    validate_release_manifest_v2,
};
use clap::Parser;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const RESULT_RECEIPT_SCHEMA: &str = "cargo-allow.release-manifest-result.v1";
const TOPOLOGY_RECEIPT_SCHEMA: &str = "cargo-allow.topology-publish-receipt.v1";
const BINARY_PACKAGE_RECEIPT_SCHEMA: &str = "cargo-allow.release-binary-package.v1";
const BINARY_INSTALL_RECEIPT_SCHEMA: &str = "cargo-allow.release-binary-install.v1";
const MANIFEST_WORKFLOW_PATH: &str = ".github/workflows/release.yml";
const CLAIM_BOUNDARY: &str = "topology-derived package identity and release evidence only; this manifest does not authorize publication or claim source, runtime, platform, or support completeness";
const LIMITATIONS: [&str; 2] = [
    "candidate rows come from the exact topology publisher receipt",
    "binary evidence is limited to the referenced receipt and verified target",
];

/// Produce or validate the typed ReleaseManifestV2 release evidence (#2497).
#[derive(Debug, Clone, Parser)]
#[command(disable_version_flag = true)]
pub(crate) struct ReleaseManifestArgs {
    /// Canonical release version every manifest row must carry.
    #[arg(long)]
    version: Option<String>,
    /// GitHub repository full name.
    #[arg(long)]
    repository: Option<String>,
    /// Observed release tag (for example v0.2.0).
    #[arg(long)]
    tag: Option<String>,
    /// Exact candidate commit identity.
    #[arg(long)]
    commit: Option<String>,
    /// Exact candidate tree identity.
    #[arg(long)]
    tree: Option<String>,
    /// Authentication class; only crates_io_api_token is supported.
    #[arg(long)]
    auth_source: Option<String>,
    /// Compiled one-use authorization digest from the release gate.
    #[arg(long)]
    authorization_digest: Option<String>,
    /// GitHub Actions run id recorded on the envelope.
    #[arg(long)]
    workflow_run_id: Option<u64>,
    /// Space-separated proven platforms.
    #[arg(long, default_value = "linux")]
    platforms: String,
    /// Toolchain used for the release binary evidence.
    #[arg(long, default_value = "stable")]
    rust_toolchain: String,
    /// Runner used for the release binary evidence.
    #[arg(long, default_value = "ubuntu-latest")]
    runner: String,
    /// Verified binary package receipt (requires the install receipt too).
    #[arg(long)]
    binary_package_receipt: Option<PathBuf>,
    /// Clean-install binary receipt (requires the package receipt too).
    #[arg(long)]
    binary_install_receipt: Option<PathBuf>,
    /// Exact topology publisher receipt.
    #[arg(
        long,
        default_value = "target/cargo-allow/topology-publish.receipt.json"
    )]
    topology_receipt: PathBuf,
    /// Require the published-topology law (public release entry).
    #[arg(long)]
    require_published_topology: bool,
    /// Expected digest of the downloaded topology receipt bytes.
    #[arg(long)]
    expected_topology_receipt_sha256: Option<String>,
    /// Validate an existing manifest instead of generating one.
    #[arg(long)]
    manifest: Option<PathBuf>,
    /// Output manifest path (generate mode).
    #[arg(long, default_value = "target/cargo-allow/release-manifest-v2.json")]
    output: PathBuf,
    /// Required typed result for progression; a different result fails closed.
    #[arg(long)]
    require_result: Option<String>,
    /// Typed result receipt output path.
    #[arg(
        long,
        default_value = "target/cargo-allow/release-manifest-v2.result.json"
    )]
    result_out: PathBuf,
    /// GitHub Actions output file that receives a `result=<name>` line.
    #[arg(long)]
    github_output: Option<PathBuf>,
}

/// The typed progression receipt: schema identity, the exact manifest bytes
/// it names, the required phase result, and the validator's typed outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ReleaseManifestResultReceiptV1 {
    schema: &'static str,
    mode: &'static str,
    manifest_path: String,
    manifest_sha256: Option<String>,
    required_result: Option<String>,
    result: String,
    gaps: Vec<String>,
}

/// One topology receipt row after the candidate identity law held.
struct CandidateRow {
    logical_id: String,
    name: String,
    version: String,
    order: u32,
    local_checksum: String,
    registry_checksum: Option<String>,
    state: String,
}

/// Binary receipt evidence retained per the #3761 evidence-reference law.
struct BinaryEvidence {
    package: ConsumedEvidenceV1,
    install: ConsumedEvidenceV1,
    attestation_verified: bool,
}

struct LoadedReceipt {
    digest: String,
    document: Value,
}

fn refusal(message: impl std::fmt::Display) -> CargoAllowError {
    CargoAllowError::with_kind(CargoAllowErrorKind::InvalidConfig, message.to_string())
}

fn artifact_failure(message: impl std::fmt::Display) -> CargoAllowError {
    CargoAllowError::with_kind(CargoAllowErrorKind::Artifact, message.to_string())
}

fn canonical_sha256(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("sha256:{hex}")
}

fn result_name(result: &ReleaseManifestResultV2) -> CargoAllowResult<String> {
    serde_json::to_value(result)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .ok_or_else(|| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::Internal,
                "release-manifest: typed result does not render a name",
            )
        })
}

fn parse_required_result(value: &str) -> CargoAllowResult<ReleaseManifestResultV2> {
    serde_json::from_value::<ReleaseManifestResultV2>(Value::String(value.to_string())).map_err(
        |_| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::Usage,
                format!(
                    "release-manifest: unknown required result {value:?}; use a typed ReleaseManifestResultV2 snake_case name"
                ),
            )
        },
    )
}

fn load_receipt(path: &Path, schema: &str) -> CargoAllowResult<LoadedReceipt> {
    let bytes = std::fs::read(path).map_err(|error| {
        refusal(format!(
            "release-manifest: invalid receipt {}: {error}",
            path.display()
        ))
    })?;
    let digest = canonical_sha256(&bytes);
    let document: Value = serde_json::from_slice(&bytes).map_err(|error| {
        refusal(format!(
            "release-manifest: invalid receipt {}: {error}",
            path.display()
        ))
    })?;
    if !document.is_object() {
        return Err(refusal(format!(
            "release-manifest: invalid receipt {}: JSON payload is not an object",
            path.display()
        )));
    }
    let observed_schema = document
        .get("schema_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if observed_schema != schema {
        return Err(refusal(format!(
            "release-manifest: unsupported receipt schema in {}",
            path.display()
        )));
    }
    let schema_version = document
        .get("schema_version")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    if schema_version != 1 {
        return Err(refusal(format!(
            "release-manifest: unsupported receipt schema in {}",
            path.display()
        )));
    }
    Ok(LoadedReceipt { digest, document })
}

fn str_field<'a>(document: &'a Value, field: &str) -> Option<&'a str> {
    document.get(field).and_then(Value::as_str)
}

fn valid_prefixed_digest(value: &str) -> bool {
    let digest = value.strip_prefix("sha256:").unwrap_or_default();
    value.starts_with("sha256:")
        && digest.len() == 64
        && digest
            .chars()
            .all(|character| character.is_ascii_hexdigit())
}

fn valid_unprefixed_digest(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|character| character.is_ascii_hexdigit())
}

fn parse_publication_state(value: &str) -> CargoAllowResult<PublicationStateV1> {
    match value {
        "missing" => Ok(PublicationStateV1::Missing),
        "published_verified" => Ok(PublicationStateV1::PublishedVerified),
        "verified_existing" => Ok(PublicationStateV1::VerifiedExisting),
        "provider_unavailable" => Ok(PublicationStateV1::ProviderUnavailable),
        other => Err(refusal(format!(
            "release-manifest: unknown publication state {other:?}; expected missing, published_verified, verified_existing, or provider_unavailable"
        ))),
    }
}

fn classification_name(classification: allow_report::PublicationClassificationV1) -> String {
    serde_json::to_value(classification)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unclassified".to_string())
}

fn generated_at_utc(now: SystemTime) -> CargoAllowResult<String> {
    let elapsed = now.duration_since(UNIX_EPOCH).map_err(|_| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InstrumentFailure,
            "release-manifest: clock precedes the Unix epoch",
        )
    })?;
    let total_seconds = elapsed.as_secs();
    let date = SimpleDate::from_days_since_unix_epoch((total_seconds / 86_400) as i64);
    if !(1970..=9999).contains(&date.year) {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InstrumentFailure,
            "release-manifest: UTC timestamp exceeds the supported calendar",
        ));
    }
    let seconds_of_day = total_seconds % 86_400;
    Ok(format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        date.year,
        date.month,
        date.day,
        seconds_of_day / 3_600,
        (seconds_of_day % 3_600) / 60,
        seconds_of_day % 60
    ))
}

fn artifact_reference(path: &Path) -> String {
    if path.is_absolute() {
        if let Ok(relative) = path.strip_prefix(std::env::current_dir().unwrap_or_default()) {
            return relative.to_string_lossy().replace('\\', "/");
        }
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        return format!("artifact/{name}");
    }
    path.to_string_lossy().replace('\\', "/")
}

/// The full topology receipt law: schema, digest binding, release identity,
/// publication posture, and the irreversible-row marker contract.
fn load_topology_evidence(args: &ReleaseManifestArgs) -> CargoAllowResult<LoadedReceipt> {
    let topology = load_receipt(&args.topology_receipt, TOPOLOGY_RECEIPT_SCHEMA)?;
    let require_published = args.require_published_topology;
    let expected_digest = args
        .expected_topology_receipt_sha256
        .as_deref()
        .unwrap_or_default();
    if require_published && expected_digest.is_empty() {
        return Err(refusal(
            "release-manifest: public release requires the downloaded topology receipt digest",
        ));
    }
    if !expected_digest.is_empty() && expected_digest != topology.digest {
        return Err(refusal(
            "release-manifest: topology receipt digest differs from the downloaded evidence",
        ));
    }
    let commit = args.commit.as_deref().unwrap_or_default();
    let tree = args.tree.as_deref().unwrap_or_default();
    if str_field(&topology.document, "commit") != Some(commit)
        || str_field(&topology.document, "tree") != Some(tree)
    {
        return Err(refusal(
            "release-manifest: topology receipt identity disagrees with release",
        ));
    }
    if str_field(&topology.document, "mode") != Some("cargo-allow")
        || topology.document.get("complete").and_then(Value::as_bool) != Some(true)
    {
        return Err(refusal(
            "release-manifest: topology receipt is not a complete cargo-allow candidate",
        ));
    }
    let raw_rows = topology
        .document
        .get("rows")
        .and_then(Value::as_array)
        .ok_or_else(|| refusal("release-manifest: topology receipt has no package rows"))?;
    if raw_rows.is_empty() {
        return Err(refusal(
            "release-manifest: topology receipt has no package rows",
        ));
    }
    let publish = topology
        .document
        .get("publish")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            refusal("release-manifest: topology receipt publish field must be a Boolean")
        })?;
    if require_published && !publish {
        return Err(refusal(
            "release-manifest: public release requires published topology evidence",
        ));
    }
    if publish {
        if topology.document.get("complete").and_then(Value::as_bool) != Some(true) {
            return Err(refusal(
                "release-manifest: published topology receipt is not complete",
            ));
        }
        if str_field(&topology.document, "incident_state") != Some("none") {
            return Err(refusal(
                "release-manifest: published topology receipt records an incident",
            ));
        }
        require_irreversible_row_marker(&topology.document, raw_rows)?;
    }
    let topology_sha = str_field(&topology.document, "topology_sha256").unwrap_or_default();
    let lock_sha = str_field(&topology.document, "cargo_lock_sha256").unwrap_or_default();
    if !valid_unprefixed_digest(topology_sha) {
        return Err(refusal("release-manifest: malformed topology digest"));
    }
    if !valid_unprefixed_digest(lock_sha) {
        return Err(refusal("release-manifest: malformed Cargo.lock digest"));
    }
    if require_published {
        let authorization = args.authorization_digest.as_deref().unwrap_or_default();
        if !valid_prefixed_digest(authorization)
            || str_field(&topology.document, "authorization") != Some(authorization)
        {
            return Err(refusal(
                "release-manifest: publication authorization differs from the release gate",
            ));
        }
    }
    Ok(topology)
}

/// The published-marker contract: a null marker means every row already
/// existed exactly; a present marker must be a positive row order.
fn require_irreversible_row_marker(document: &Value, raw_rows: &[Value]) -> CargoAllowResult<()> {
    let Some(marker) = document.get("first_irreversible_row") else {
        return Err(refusal(
            "release-manifest: published topology receipt lacks an irreversible-row marker",
        ));
    };
    if marker.is_null() {
        let all_verified_existing = raw_rows
            .iter()
            .all(|row| str_field(row, "state") == Some("verified_existing"));
        if !all_verified_existing {
            return Err(refusal(
                "release-manifest: newly published rows require an irreversible-row marker",
            ));
        }
        return Ok(());
    }
    let order = marker.as_u64().filter(|_| !marker.is_boolean());
    if !matches!(order, Some(order) if order > 0) {
        return Err(refusal(
            "release-manifest: published topology receipt has an invalid irreversible-row marker",
        ));
    }
    Ok(())
}

/// The candidate identity law every row must satisfy regardless of posture.
fn candidate_row(
    raw: &Value,
    version: &str,
    seen_names: &mut Vec<String>,
    seen_logical_ids: &mut Vec<String>,
    seen_orders: &mut Vec<u64>,
) -> CargoAllowResult<CandidateRow> {
    let logical_id = str_field(raw, "logical_id").unwrap_or_default();
    let name = str_field(raw, "name").unwrap_or_default();
    let row_version = str_field(raw, "version").unwrap_or_default();
    let order = raw.get("release_order").and_then(Value::as_u64);
    let local_checksum = str_field(raw, "local_checksum").unwrap_or_default();
    if logical_id.is_empty()
        || name.is_empty()
        || row_version.is_empty()
        || order.is_none()
        || local_checksum.is_empty()
    {
        return Err(refusal(
            "release-manifest: topology receipt row is incomplete",
        ));
    }
    if ReleaseVersionV1::parse(row_version).is_err() {
        return Err(refusal(format!(
            "release-manifest: package version {row_version} for {name} is not a validated typed release identity"
        )));
    }
    if row_version != version {
        return Err(refusal(format!(
            "release-manifest: package version {row_version} for {name} is not the selected release identity {version}"
        )));
    }
    let order = order.unwrap_or_default();
    if order == 0
        || seen_names.iter().any(|seen| seen == name)
        || seen_logical_ids.iter().any(|seen| seen == logical_id)
        || seen_orders.contains(&order)
    {
        return Err(refusal(
            "release-manifest: topology receipt contains duplicate package identity",
        ));
    }
    seen_names.push(name.to_string());
    seen_logical_ids.push(logical_id.to_string());
    seen_orders.push(order);
    if !valid_prefixed_digest(local_checksum) {
        return Err(refusal(format!(
            "release-manifest: malformed crate digest for {name}"
        )));
    }
    Ok(CandidateRow {
        logical_id: logical_id.to_string(),
        name: name.to_string(),
        version: row_version.to_string(),
        order: u32::try_from(order).map_err(|_| {
            refusal(format!(
                "release-manifest: release order {order} for {name} exceeds the manifest order range"
            ))
        })?,
        local_checksum: local_checksum.to_string(),
        registry_checksum: raw
            .get("registry_checksum")
            .and_then(Value::as_str)
            .map(str::to_string),
        state: str_field(raw, "state").unwrap_or_default().to_string(),
    })
}

/// The published-row law: registry evidence, exact equality, packaged archive
/// bytes on the public entry, and the typed reconciliation classification.
fn manifest_row_from_candidate(
    args: &ReleaseManifestArgs,
    row: &CandidateRow,
) -> CargoAllowResult<ReleaseManifestPackageRowV2> {
    let state = parse_publication_state(&row.state)?;
    if !matches!(
        state,
        PublicationStateV1::PublishedVerified | PublicationStateV1::VerifiedExisting
    ) {
        return Err(refusal(format!(
            "release-manifest: published topology row {} is not registry-verified",
            row.name
        )));
    }
    let registry_checksum = row
        .registry_checksum
        .as_deref()
        .filter(|value| valid_prefixed_digest(value))
        .ok_or_else(|| {
            refusal(format!(
                "release-manifest: registry checksum missing or malformed for published row {}",
                row.name
            ))
        })?;
    if registry_checksum != row.local_checksum {
        return Err(refusal(format!(
            "release-manifest: registry checksum disagrees with the candidate package for {} ({})",
            row.name, row.state
        )));
    }
    if args.require_published_topology {
        // The publisher pins package_target_dir() to this checkout's target;
        // ambient Cargo settings cannot select different evidence bytes.
        let package_directory = std::fs::canonicalize("target/package").map_err(|error| {
            refusal(format!(
                "release-manifest: package archive unavailable for {}: {error}",
                row.name
            ))
        })?;
        let crate_relative =
            PathBuf::from(format!("target/package/{}-{}.crate", row.name, row.version));
        let crate_file = crate_relative.canonicalize().map_err(|error| {
            refusal(format!(
                "release-manifest: package archive unavailable for {}: {error}",
                row.name
            ))
        })?;
        if crate_file.parent() != Some(package_directory.as_path()) {
            return Err(refusal(
                "release-manifest: invalid package archive identity",
            ));
        }
        let crate_bytes = std::fs::read(&crate_file).map_err(|error| {
            refusal(format!(
                "release-manifest: package archive unavailable for {}: {error}",
                row.name
            ))
        })?;
        if canonical_sha256(&crate_bytes) != registry_checksum {
            return Err(refusal(format!(
                "release-manifest: package archive differs from publication for {}",
                row.name
            )));
        }
    }
    let publication = ReconciledPackagePublicationV1 {
        logical_id: row.logical_id.clone(),
        package_name: row.name.clone(),
        package_version: row.version.clone(),
        release_order: row.order,
        row_class: PackageRowClassV1::CargoAllowCandidate,
        state,
        expected_checksum: row.local_checksum.clone(),
        observed_registry_checksum: Some(registry_checksum.to_string()),
    };
    let classification = publication.classify();
    if !publication.is_manifest_ready() {
        return Err(refusal(format!(
            "release-manifest: row {} classified {}; only complete_exact publications may enter the manifest",
            row.name,
            classification_name(classification)
        )));
    }
    publication.manifest_row().map_err(refusal)
}

/// The unpublished-candidate row law: checksum form only, no publication
/// evidence is claimed.
fn candidate_manifest_row(row: &CandidateRow) -> CargoAllowResult<ReleaseManifestPackageRowV2> {
    let registry_checksum = match row.registry_checksum.as_deref() {
        Some(value) => {
            if !valid_prefixed_digest(value) {
                return Err(refusal(format!(
                    "release-manifest: malformed registry checksum for {}",
                    row.name
                )));
            }
            Some(value.to_string())
        }
        None => None,
    };
    Ok(ReleaseManifestPackageRowV2 {
        logical_id: row.logical_id.clone(),
        package_name: row.name.clone(),
        package_version: row.version.clone(),
        release_order: row.order,
        crate_digest: Some(row.local_checksum.clone()),
        registry_checksum,
    })
}

/// Binary receipt law: both receipts agree with each other and with the
/// release identity; attestation state drives the envelope diagnostics.
fn load_binary_evidence(args: &ReleaseManifestArgs) -> CargoAllowResult<Option<BinaryEvidence>> {
    let (Some(package_path), Some(install_path)) = (
        args.binary_package_receipt.as_deref(),
        args.binary_install_receipt.as_deref(),
    ) else {
        return Ok(None);
    };
    let package = load_receipt(package_path, BINARY_PACKAGE_RECEIPT_SCHEMA)?;
    let install = load_receipt(install_path, BINARY_INSTALL_RECEIPT_SCHEMA)?;
    let fields = [
        "version",
        "target_triple",
        "archive_name",
        "archive_sha256",
        "executable_sha256",
        "tag",
        "commit",
        "tree",
    ];
    for field in fields {
        let package_value = package.document.get(field);
        let install_value = install.document.get(field);
        if package_value.is_none() || install_value.is_none() {
            return Err(refusal(
                "release-manifest: binary receipt is missing a required field",
            ));
        }
        if package_value != install_value {
            return Err(refusal(format!(
                "release-manifest: binary receipt disagreement in {field}"
            )));
        }
    }
    let version = args.version.as_deref().unwrap_or_default();
    let tag = args.tag.as_deref().unwrap_or_default();
    let commit = args.commit.as_deref().unwrap_or_default();
    let tree = args.tree.as_deref().unwrap_or_default();
    if str_field(&package.document, "version") != Some(version) {
        return Err(refusal(
            "release-manifest: binary receipt version disagrees with manifest",
        ));
    }
    for (field, expected) in [("tag", tag), ("commit", commit), ("tree", tree)] {
        if str_field(&package.document, field) != Some(expected) {
            return Err(refusal(format!(
                "release-manifest: binary receipt {field} disagrees with manifest identity"
            )));
        }
    }
    if str_field(&package.document, "target_triple") != Some("x86_64-unknown-linux-gnu") {
        return Err(refusal("release-manifest: unsupported binary target"));
    }
    if !args
        .platforms
        .split_whitespace()
        .any(|platform| platform == "linux")
    {
        return Err(refusal(
            "release-manifest: Linux binary asset requires linux in PLATFORMS",
        ));
    }
    let expected_archive = format!("cargo-allow-v{version}-x86_64-unknown-linux-gnu.tar.gz");
    if str_field(&package.document, "archive_name") != Some(expected_archive.as_str()) {
        return Err(refusal(
            "release-manifest: binary archive name is not the stable Linux name",
        ));
    }
    if str_field(&package.document, "archive_format") != Some("tar.gz") {
        return Err(refusal(
            "release-manifest: Linux binary archive format must be tar.gz",
        ));
    }
    if str_field(&package.document, "executable_name") != Some("cargo-allow") {
        return Err(refusal(
            "release-manifest: binary executable name must be cargo-allow",
        ));
    }
    if args.rust_toolchain.is_empty() || args.runner.is_empty() {
        return Err(refusal(
            "release-manifest: binary toolchain and runner are required",
        ));
    }
    for field in ["archive_sha256", "executable_sha256"] {
        let value = str_field(&package.document, field).unwrap_or_default();
        if !valid_prefixed_digest(value) {
            return Err(refusal(format!(
                "release-manifest: malformed binary digest {field}"
            )));
        }
    }
    let attestation_verified = install
        .document
        .get("attestation_verified")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Ok(Some(BinaryEvidence {
        package: ConsumedEvidenceV1 {
            schema_id: BINARY_PACKAGE_RECEIPT_SCHEMA.to_string(),
            path: artifact_reference(package_path),
            sha256: package.digest,
            producer: "scripts/package-release-binary.sh".to_string(),
            result_class: "verified".to_string(),
        },
        install: ConsumedEvidenceV1 {
            schema_id: BINARY_INSTALL_RECEIPT_SCHEMA.to_string(),
            path: artifact_reference(install_path),
            sha256: install.digest,
            producer: "scripts/verify-release-binary.sh".to_string(),
            result_class: if attestation_verified {
                "verified".to_string()
            } else {
                "attestation_unverified".to_string()
            },
        },
        attestation_verified,
    }))
}

fn build_envelope(
    args: &ReleaseManifestArgs,
    topology: &LoadedReceipt,
    rows: Vec<ReleaseManifestPackageRowV2>,
    binary: Option<BinaryEvidence>,
    generated_at: &str,
) -> CargoAllowResult<ReleaseManifestEnvelopeV2> {
    let publish = topology
        .document
        .get("publish")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut consumed_evidence = vec![ConsumedEvidenceV1 {
        schema_id: TOPOLOGY_RECEIPT_SCHEMA.to_string(),
        path: artifact_reference(&args.topology_receipt),
        sha256: topology.digest.clone(),
        producer: "scripts/release-topology-publisher.py".to_string(),
        result_class: if topology.document.get("complete").and_then(Value::as_bool) == Some(true) {
            "complete".to_string()
        } else {
            "incomplete".to_string()
        },
    }];
    let mut instrument_diagnostics = Vec::new();
    let mut artifact_references = vec![artifact_reference(&args.topology_receipt)];
    if let Some(binary) = binary {
        artifact_references.push(binary.package.path.clone());
        artifact_references.push(binary.install.path.clone());
        consumed_evidence.push(binary.package);
        consumed_evidence.push(binary.install);
        if !binary.attestation_verified {
            instrument_diagnostics.push("binary attestation is not verified".to_string());
        }
    }
    let authorization_reference = match args.workflow_run_id {
        Some(run_id) => format!("workflow/{run_id}/crates-io"),
        None => "workflow/crates-io".to_string(),
    };
    Ok(ReleaseManifestEnvelopeV2 {
        payload: ReleaseManifestPayloadV2 {
            schema_id: allow_report::RELEASE_MANIFEST_V2_SCHEMA_ID.to_string(),
            schema_version: allow_report::RELEASE_MANIFEST_V2_SCHEMA_VERSION,
            operation: ReleaseManifestOperationV2::CargoAllowRelease,
            repository: args.repository.clone().unwrap_or_default(),
            tag_or_authorization: args.tag.clone().unwrap_or_default(),
            commit: args.commit.clone().unwrap_or_default(),
            tree: args.tree.clone().unwrap_or_default(),
            cargo_lock_digest: Some(format!(
                "sha256:{}",
                str_field(&topology.document, "cargo_lock_sha256").unwrap_or_default()
            )),
            architecture_digest: Some(format!(
                "sha256:{}",
                str_field(&topology.document, "topology_sha256").unwrap_or_default()
            )),
            candidate_digest: Some(topology.digest.clone()),
            package_rows: rows,
            authentication: ReleaseManifestAuthenticationV2::CratesIoApiToken,
            publication_posture: if publish {
                ReleaseManifestPublicationPostureV2::Published
            } else {
                ReleaseManifestPublicationPostureV2::Unpublished
            },
            support_posture: ReleaseManifestSupportPostureV2::Experimental,
            limitations: LIMITATIONS.iter().map(|line| line.to_string()).collect(),
            claim_boundary: CLAIM_BOUNDARY.to_string(),
            consumed_evidence,
        },
        generated_at: generated_at.to_string(),
        workflow_path: MANIFEST_WORKFLOW_PATH.to_string(),
        workflow_run_id: args.workflow_run_id,
        workflow_attempt: None,
        event: Some("release".to_string()),
        github_ref: args.tag.clone(),
        artifact_references,
        authorization_reference: Some(authorization_reference),
        instrument_diagnostics,
    })
}

fn render_envelope_bytes(envelope: &ReleaseManifestEnvelopeV2) -> CargoAllowResult<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(envelope).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Internal,
            format!("release-manifest: cannot render manifest envelope: {error}"),
        )
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn write_manifest_and_sidecar(output: &Path, bytes: &[u8]) -> CargoAllowResult<String> {
    if let Some(parent) = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|error| {
            artifact_failure(format!(
                "release-manifest: cannot create manifest directory {}: {error}",
                parent.display()
            ))
        })?;
    }
    std::fs::write(output, bytes).map_err(|error| {
        artifact_failure(format!(
            "release-manifest: cannot write manifest {}: {error}",
            output.display()
        ))
    })?;
    let digest = canonical_sha256(bytes);
    let hex = digest
        .strip_prefix("sha256:")
        .unwrap_or_default()
        .to_string();
    let sidecar = output.with_extension("sha256");
    let basename = output
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    std::fs::write(&sidecar, format!("{hex}  {basename}\n")).map_err(|error| {
        artifact_failure(format!(
            "release-manifest: cannot write manifest checksum {}: {error}",
            sidecar.display()
        ))
    })?;
    Ok(digest)
}

/// Validate the exact manifest bytes and build the typed result receipt. A
/// phase requirement that the typed result does not meet fails closed.
fn validate_manifest_bytes(
    bytes: &[u8],
    mode: &'static str,
    manifest_path: &Path,
    required: Option<&ReleaseManifestResultV2>,
) -> CargoAllowResult<(ReleaseManifestResultReceiptV1, ReleaseManifestResultV2)> {
    let envelope: ReleaseManifestEnvelopeV2 = serde_json::from_slice(bytes).map_err(|error| {
        refusal(format!(
            "release-manifest: manifest {} does not parse against the typed contract: {error}",
            manifest_path.display()
        ))
    })?;
    let validation = validate_release_manifest_v2(&envelope);
    let result = result_name(&validation.result)?;
    let receipt = ReleaseManifestResultReceiptV1 {
        schema: RESULT_RECEIPT_SCHEMA,
        mode,
        manifest_path: manifest_path.to_string_lossy().replace('\\', "/"),
        manifest_sha256: Some(canonical_sha256(bytes)),
        required_result: match required {
            Some(required) => Some(result_name(required)?),
            None => None,
        },
        result,
        gaps: validation.gaps.clone(),
    };
    if let Some(required) = required
        .cloned()
        .filter(|required| validation.result != *required)
    {
        let required_name = result_name(&required)?;
        let gaps = if validation.gaps.is_empty() {
            "none".to_string()
        } else {
            validation.gaps.join("; ")
        };
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::PolicyViolation,
            format!(
                "release-manifest: typed result is {}; required {required_name}; gaps: {gaps}",
                receipt.result
            ),
        ));
    }
    Ok((receipt, validation.result))
}

fn emit_result_receipt(
    args: &ReleaseManifestArgs,
    receipt: &ReleaseManifestResultReceiptV1,
) -> CargoAllowResult<()> {
    let rendered = serde_json::to_string_pretty(receipt).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Internal,
            format!("release-manifest: cannot render result receipt: {error}"),
        )
    })?;
    if let Some(parent) = args
        .result_out
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|error| {
            artifact_failure(format!(
                "release-manifest: cannot create result receipt directory {}: {error}",
                parent.display()
            ))
        })?;
    }
    std::fs::write(&args.result_out, format!("{rendered}\n")).map_err(|error| {
        artifact_failure(format!(
            "release-manifest: cannot write result receipt {}: {error}",
            args.result_out.display()
        ))
    })?;
    println!("{rendered}");
    if let Some(github_output) = args.github_output.as_deref() {
        use std::io::Write as _;
        let mut output = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(github_output)
            .map_err(|error| {
                artifact_failure(format!(
                    "release-manifest: cannot open GitHub output {}: {error}",
                    github_output.display()
                ))
            })?;
        writeln!(output, "result={}", receipt.result).map_err(|error| {
            artifact_failure(format!(
                "release-manifest: cannot write GitHub output: {error}"
            ))
        })?;
    }
    Ok(())
}

fn require_generate_inputs(args: &ReleaseManifestArgs) -> CargoAllowResult<()> {
    for (field, value) in [
        ("version", args.version.as_deref()),
        ("repository", args.repository.as_deref()),
        ("tag", args.tag.as_deref()),
        ("commit", args.commit.as_deref()),
        ("tree", args.tree.as_deref()),
        ("auth-source", args.auth_source.as_deref()),
    ] {
        if value.unwrap_or_default().trim().is_empty() {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::Usage,
                format!("release-manifest: --{field} is required in generate mode"),
            ));
        }
    }
    if args.binary_package_receipt.is_some() != args.binary_install_receipt.is_some() {
        return Err(refusal(
            "release-manifest: binary package and install receipts must be supplied together",
        ));
    }
    if args.auth_source.as_deref() != Some("crates_io_api_token") {
        return Err(refusal(
            "release-manifest: only crates_io_api_token is supported",
        ));
    }
    Ok(())
}

fn cmd_generate(args: &ReleaseManifestArgs) -> CargoAllowResult<()> {
    require_generate_inputs(args)?;
    let topology = load_topology_evidence(args)?;
    let publish = topology
        .document
        .get("publish")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let version = args.version.as_deref().unwrap_or_default();
    let raw_rows = topology
        .document
        .get("rows")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .collect::<Vec<&Value>>();
    let mut seen_names = Vec::new();
    let mut seen_logical_ids = Vec::new();
    let mut seen_orders = Vec::new();
    let mut rows = Vec::with_capacity(raw_rows.len());
    for raw in &raw_rows {
        let candidate = candidate_row(
            raw,
            version,
            &mut seen_names,
            &mut seen_logical_ids,
            &mut seen_orders,
        )?;
        rows.push(if publish {
            manifest_row_from_candidate(args, &candidate)?
        } else {
            candidate_manifest_row(&candidate)?
        });
    }
    rows.sort_by_key(|row| row.release_order);
    let binary = load_binary_evidence(args)?;
    let generated_at = generated_at_utc(SystemTime::now())?;
    let envelope = build_envelope(args, &topology, rows, binary, &generated_at)?;
    let bytes = render_envelope_bytes(&envelope)?;
    let manifest_digest = write_manifest_and_sidecar(&args.output, &bytes)?;
    // The exact bytes written are the exact bytes validated: re-read and
    // re-validate them so the emitted result names the retained evidence.
    let written = std::fs::read(&args.output).map_err(|error| {
        artifact_failure(format!(
            "release-manifest: cannot read back manifest {}: {error}",
            args.output.display()
        ))
    })?;
    if canonical_sha256(&written) != manifest_digest {
        return Err(artifact_failure(
            "release-manifest: written manifest bytes differ from the rendered envelope",
        ));
    }
    let required = args
        .require_result
        .as_deref()
        .map(parse_required_result)
        .transpose()?;
    let outcome = validate_manifest_bytes(&written, "generate", &args.output, required.as_ref());
    match outcome {
        Ok((receipt, _)) => emit_result_receipt(args, &receipt),
        Err(error) => {
            // Diagnostics first: the failed reconciliation receipt records the
            // typed result even when the phase requirement fails the command.
            let validation = validate_release_manifest_v2(&envelope);
            let failed = ReleaseManifestResultReceiptV1 {
                schema: RESULT_RECEIPT_SCHEMA,
                mode: "generate",
                manifest_path: args.output.to_string_lossy().replace('\\', "/"),
                manifest_sha256: Some(manifest_digest),
                required_result: args.require_result.as_deref().map(str::to_string),
                result: result_name(&validation.result)?,
                gaps: validation.gaps.clone(),
            };
            emit_result_receipt(args, &failed)?;
            Err(error)
        }
    }
}

fn cmd_validate(args: &ReleaseManifestArgs) -> CargoAllowResult<()> {
    let manifest_path = args.manifest.as_deref().unwrap_or_else(|| Path::new(""));
    let bytes = std::fs::read(manifest_path).map_err(|error| {
        refusal(format!(
            "release-manifest: cannot read manifest {}: {error}",
            manifest_path.display()
        ))
    })?;
    let required = args
        .require_result
        .as_deref()
        .map(parse_required_result)
        .transpose()?;
    let outcome = validate_manifest_bytes(&bytes, "validate", manifest_path, required.as_ref());
    match outcome {
        Ok((receipt, _)) => emit_result_receipt(args, &receipt),
        Err(error) => {
            // Retain the typed diagnostics for the failed validation when the
            // manifest still parses against the contract.
            if let Ok(envelope) = serde_json::from_slice::<ReleaseManifestEnvelopeV2>(&bytes) {
                let validation = validate_release_manifest_v2(&envelope);
                let failed = ReleaseManifestResultReceiptV1 {
                    schema: RESULT_RECEIPT_SCHEMA,
                    mode: "validate",
                    manifest_path: manifest_path.to_string_lossy().replace('\\', "/"),
                    manifest_sha256: Some(canonical_sha256(&bytes)),
                    required_result: args.require_result.as_deref().map(str::to_string),
                    result: result_name(&validation.result)?,
                    gaps: validation.gaps.clone(),
                };
                emit_result_receipt(args, &failed)?;
            }
            Err(error)
        }
    }
}

pub(super) fn cmd_release_manifest(args: &ReleaseManifestArgs) -> CargoAllowResult<()> {
    if args.manifest.is_some() {
        cmd_validate(args)
    } else {
        cmd_generate(args)
    }
}

#[cfg(test)]
#[path = "release_manifest_command_tests.rs"]
mod tests;
