//! Typed crates.io release receipt contracts (#2496).
//!
//! Synthetic records only: no registry call, no token read, no tag, and no
//! publication occurs. These tests prove the fail-closed rejection paths and
//! the positive classification law of the three receipt families the issue
//! found untyped on main:
//!
//! - `CratesIoTrustedPublishingReceiptV1` (#2282): publication authentication
//!   posture (typed replacement for the untyped `auth_source` string);
//! - `CratesIoFirstPublicationBootstrapReceiptV1` (#3145): shared-substrate
//!   bootstrap bytes visible before the tag, which can never complete the
//!   final release manifest;
//! - `CratesIoRegistryInstallJourneyReceiptV1`: the registry-installed
//!   journey (typed replacement for the untyped install-smoke heredoc).
//!
//! They also bind each JSON schema file to its Rust contract across the three
//! registration denominators: schema-index classification, README catalog
//! listing, and the `cargo-allow` house title.

use std::error::Error;
use std::fs;
use std::path::Path;

use allow_report::{
    CRATES_IO_FIRST_PUBLICATION_BOOTSTRAP_RECEIPT_SCHEMA_ID,
    CRATES_IO_FIRST_PUBLICATION_BOOTSTRAP_RECEIPT_SCHEMA_VERSION,
    CRATES_IO_REGISTRY_INSTALL_JOURNEY_RECEIPT_SCHEMA_ID,
    CRATES_IO_REGISTRY_INSTALL_JOURNEY_RECEIPT_SCHEMA_VERSION,
    CRATES_IO_TRUSTED_PUBLISHING_RECEIPT_SCHEMA_ID,
    CRATES_IO_TRUSTED_PUBLISHING_RECEIPT_SCHEMA_VERSION, CratesIoAuthClassV1,
    CratesIoFirstPublicationBootstrapReceiptV1, CratesIoFirstPublicationBootstrapResultV1,
    CratesIoRegistryInstallJourneyReceiptV1, CratesIoRegistryInstallJourneyResultV1,
    CratesIoTrustedPublishingReceiptV1, CratesIoTrustedPublishingResultV1, OidcPostureV1,
    PackageRowClassV1, PublicationStateV1, RELEASE_MANIFEST_V2_SCHEMA_ID,
    RELEASE_MANIFEST_V2_SCHEMA_VERSION, ReconciledPackagePublicationV1, RegistryInstallChannelV1,
    RegistryJourneyStepV1, RegistryResolvedRowV1, ReleaseManifestAuthenticationV2,
    ReleaseManifestEnvelopeV2, ReleaseManifestOperationV2, ReleaseManifestPayloadV2,
    ReleaseManifestPublicationPostureV2, ReleaseManifestResultV2, ReleaseManifestSupportPostureV2,
    validate_crates_io_first_publication_bootstrap_receipt_v1,
    validate_crates_io_registry_install_journey_receipt_v1,
    validate_crates_io_trusted_publishing_receipt_v1, validate_release_manifest_v2,
};

const CANONICAL: &str = "sha256:a3f6c99da45120b96968349bc51c83ddf81e6d7a2c49e06e8727d3ab4195a3b2";
const CONFLICTING: &str = "sha256:ae3f4ed17b8877640c00e8fabd1e4e1778eff52dc102cc5103ee49ca92c5a488";

/// Repo-root-relative paths resolve from the manifest dir because cargo runs
/// integration tests with the package root as the working directory.
fn repo_root(relative: &str) -> Result<String, Box<dyn Error>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(relative);
    fs::read_to_string(&path).map_err(|error| format!("read {}: {error}", path.display()).into())
}

fn commit() -> String {
    "8bdabcd18723e36bbcfada423fa44779eefa599c".to_string()
}

fn tree() -> String {
    "69abc204feedbeefcafe0123456789abcdef0123".to_string()
}

// --- trusted publishing -------------------------------------------------

fn oidc_posture() -> OidcPostureV1 {
    OidcPostureV1 {
        action_repository: "rust-lang/crates-io-auth-action".to_string(),
        action_ref_sha: "e22c1a2b3d4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b".to_string(),
        audience: "crates-io".to_string(),
    }
}

fn trusted_publishing(class: CratesIoAuthClassV1) -> CratesIoTrustedPublishingReceiptV1 {
    CratesIoTrustedPublishingReceiptV1 {
        schema_id: CRATES_IO_TRUSTED_PUBLISHING_RECEIPT_SCHEMA_ID.to_string(),
        schema_version: CRATES_IO_TRUSTED_PUBLISHING_RECEIPT_SCHEMA_VERSION,
        repository: "EffortlessMetrics/cargo-allow".to_string(),
        workflow_name: "release".to_string(),
        workflow_run_id: 3_268_982_190,
        workflow_attempt: 1,
        producer_step: "resolve crates.io API token".to_string(),
        source_commit: commit(),
        source_tree: tree(),
        git_ref: "refs/tags/v0.2.0".to_string(),
        auth_class: class,
        oidc: None,
        authorization_evidence_digest: Some(CANONICAL.to_string()),
        observed_at: "2026-11-04".to_string(),
        limitations: vec!["provider-side token handling is unobserved".to_string()],
        claim_boundary: "Typed publication authentication posture; grants no authority."
            .to_string(),
    }
}

#[test]
fn trusted_publishing_supported_postures_verify() -> Result<(), Box<dyn Error>> {
    let token = trusted_publishing(CratesIoAuthClassV1::CratesIoApiToken);
    let validation = validate_crates_io_trusted_publishing_receipt_v1(&token);
    if validation.result != CratesIoTrustedPublishingResultV1::VerifiedApiToken {
        return Err(format!("token posture classified {:?}", validation.result).into());
    }
    if !validation.gaps.is_empty() {
        return Err(format!("token posture reported gaps {:?}", validation.gaps).into());
    }

    let mut oidc = trusted_publishing(CratesIoAuthClassV1::TrustedPublishingOidc);
    oidc.oidc = Some(oidc_posture());
    oidc.authorization_evidence_digest = None;
    let validation = validate_crates_io_trusted_publishing_receipt_v1(&oidc);
    if validation.result != CratesIoTrustedPublishingResultV1::VerifiedTrustedPublishing {
        return Err(format!("oidc posture classified {:?}", validation.result).into());
    }
    Ok(())
}

#[test]
fn trusted_publishing_rejection_paths_fail_closed() -> Result<(), Box<dyn Error>> {
    let unsupported =
        trusted_publishing(CratesIoAuthClassV1::Unsupported("workflow_env".to_string()));
    let validation = validate_crates_io_trusted_publishing_receipt_v1(&unsupported);
    if validation.result != CratesIoTrustedPublishingResultV1::UnsupportedAuthClass {
        return Err(format!("unsupported classified {:?}", validation.result).into());
    }

    let missing = trusted_publishing(CratesIoAuthClassV1::Missing);
    let validation = validate_crates_io_trusted_publishing_receipt_v1(&missing);
    if validation.result != CratesIoTrustedPublishingResultV1::MissingAuth {
        return Err(format!("missing class classified {:?}", validation.result).into());
    }

    let mut tokenless = trusted_publishing(CratesIoAuthClassV1::CratesIoApiToken);
    tokenless.authorization_evidence_digest = None;
    let validation = validate_crates_io_trusted_publishing_receipt_v1(&tokenless);
    if validation.result != CratesIoTrustedPublishingResultV1::MissingAuth {
        return Err(format!("token without evidence classified {:?}", validation.result).into());
    }

    let mut mutable_tag = trusted_publishing(CratesIoAuthClassV1::TrustedPublishingOidc);
    mutable_tag.oidc = Some(OidcPostureV1 {
        action_repository: "rust-lang/crates-io-auth-action".to_string(),
        action_ref_sha: "v1".to_string(),
        audience: "crates-io".to_string(),
    });
    let validation = validate_crates_io_trusted_publishing_receipt_v1(&mutable_tag);
    if validation.result != CratesIoTrustedPublishingResultV1::Malformed {
        return Err(format!("mutable tag classified {:?}", validation.result).into());
    }

    let mut leaky = trusted_publishing(CratesIoAuthClassV1::CratesIoApiToken);
    leaky.limitations = vec!["fetched with token=ghp_redacted from the runner".to_string()];
    let validation = validate_crates_io_trusted_publishing_receipt_v1(&leaky);
    if validation.result != CratesIoTrustedPublishingResultV1::CredentialMaterialRejected {
        return Err(format!("credential leak classified {:?}", validation.result).into());
    }

    let mut stale_generation = trusted_publishing(CratesIoAuthClassV1::CratesIoApiToken);
    stale_generation.schema_version = 2;
    let validation = validate_crates_io_trusted_publishing_receipt_v1(&stale_generation);
    if validation.result != CratesIoTrustedPublishingResultV1::UnsupportedGeneration {
        return Err(format!("wrong generation classified {:?}", validation.result).into());
    }

    let mut no_run = trusted_publishing(CratesIoAuthClassV1::CratesIoApiToken);
    no_run.workflow_run_id = 0;
    let validation = validate_crates_io_trusted_publishing_receipt_v1(&no_run);
    if validation.result != CratesIoTrustedPublishingResultV1::Malformed {
        return Err(format!("zero run id classified {:?}", validation.result).into());
    }
    Ok(())
}

// --- first-publication bootstrap ---------------------------------------

fn bootstrap_row(name: &str, version: &str, observed: &str) -> allow_report::BootstrapPackageRowV1 {
    allow_report::BootstrapPackageRowV1 {
        package_name: name.to_string(),
        package_version: version.to_string(),
        registry_checksum: observed.to_string(),
        namespace_authority_checksum: CANONICAL.to_string(),
        family: allow_report::BootstrapRowFamilyV1::Shared,
    }
}

fn bootstrap_receipt(
    rows: Vec<allow_report::BootstrapPackageRowV1>,
) -> CratesIoFirstPublicationBootstrapReceiptV1 {
    CratesIoFirstPublicationBootstrapReceiptV1 {
        schema_id: CRATES_IO_FIRST_PUBLICATION_BOOTSTRAP_RECEIPT_SCHEMA_ID.to_string(),
        schema_version: CRATES_IO_FIRST_PUBLICATION_BOOTSTRAP_RECEIPT_SCHEMA_VERSION,
        repository: "EffortlessMetrics/cargo-allow".to_string(),
        source_commit: commit(),
        source_tree: tree(),
        workflow_run_id: 3_268_412_567,
        workflow_attempt: 1,
        observed_at: "2026-08-24".to_string(),
        namespace_authority_generation: "namespace-evidence-2026-08-24".to_string(),
        rows,
        promotion: allow_report::BootstrapPromotionV1::BootstrapOnly,
        retained_incident_digests: vec![CONFLICTING.to_string()],
        limitations: vec!["docs.rs state is unobserved".to_string()],
        claim_boundary: "Bootstrap-published shared bytes; never final-release completeness."
            .to_string(),
    }
}

#[test]
fn bootstrap_exact_rows_classify_and_never_complete_the_manifest() -> Result<(), Box<dyn Error>> {
    let exact = bootstrap_receipt(vec![
        bootstrap_row("effortless-repo-protocol", "0.1.0", CANONICAL),
        bootstrap_row("effortless-repo-edit", "0.1.0", CANONICAL),
    ]);
    let validation = validate_crates_io_first_publication_bootstrap_receipt_v1(&exact);
    if validation.result != CratesIoFirstPublicationBootstrapResultV1::BootstrapPublishedExact {
        return Err(format!("exact bootstrap classified {:?}", validation.result).into());
    }

    // The issue law: bootstrap evidence alone must not complete the final
    // release manifest. A manifest that cites only this bootstrap receipt
    // carries no package rows, so the manifest validator refuses Complete.
    let payload = ReleaseManifestPayloadV2 {
        schema_id: RELEASE_MANIFEST_V2_SCHEMA_ID.to_string(),
        schema_version: RELEASE_MANIFEST_V2_SCHEMA_VERSION,
        operation: ReleaseManifestOperationV2::CargoAllowRelease,
        repository: "EffortlessMetrics/cargo-allow".to_string(),
        tag_or_authorization: "v0.2.0".to_string(),
        commit: commit(),
        tree: tree(),
        cargo_lock_digest: None,
        architecture_digest: None,
        candidate_digest: None,
        package_rows: Vec::new(),
        authentication: ReleaseManifestAuthenticationV2::CratesIoApiToken,
        publication_posture: ReleaseManifestPublicationPostureV2::Unpublished,
        support_posture: ReleaseManifestSupportPostureV2::Supported,
        limitations: Vec::new(),
        claim_boundary: "attempted bootstrap-only promotion".to_string(),
        consumed_evidence: Vec::new(),
    };
    let envelope = ReleaseManifestEnvelopeV2 {
        payload,
        generated_at: "2026-11-04T00:00:00Z".to_string(),
        workflow_path: ".github/workflows/release.yml".to_string(),
        workflow_run_id: Some(3_268_412_567),
        workflow_attempt: Some(1),
        event: Some("push".to_string()),
        github_ref: Some("refs/tags/v0.2.0".to_string()),
        artifact_references: Vec::new(),
        authorization_reference: Some("custody-readback".to_string()),
        instrument_diagnostics: Vec::new(),
    };
    let manifest = validate_release_manifest_v2(&envelope);
    if manifest.result == ReleaseManifestResultV2::Complete {
        return Err("bootstrap-only manifest must not be Complete".into());
    }
    Ok(())
}

#[test]
fn bootstrap_rejection_paths_fail_closed() -> Result<(), Box<dyn Error>> {
    let conflict = bootstrap_receipt(vec![
        bootstrap_row("effortless-repo-protocol", "0.1.0", CANONICAL),
        bootstrap_row("effortless-repo-edit", "0.1.0", CONFLICTING),
    ]);
    let validation = validate_crates_io_first_publication_bootstrap_receipt_v1(&conflict);
    if validation.result != CratesIoFirstPublicationBootstrapResultV1::ChecksumConflict {
        return Err(format!("conflict classified {:?}", validation.result).into());
    }

    let rewritten = bootstrap_receipt(vec![bootstrap_row(
        "effortless-repo-edit",
        "0.2.0",
        CANONICAL,
    )]);
    let validation = validate_crates_io_first_publication_bootstrap_receipt_v1(&rewritten);
    if validation.result != CratesIoFirstPublicationBootstrapResultV1::VersionRewritten {
        return Err(format!("rewritten classified {:?}", validation.result).into());
    }

    let generic = bootstrap_receipt(vec![bootstrap_row("repo-edit", "0.1.0", CANONICAL)]);
    let validation = validate_crates_io_first_publication_bootstrap_receipt_v1(&generic);
    if validation.result != CratesIoFirstPublicationBootstrapResultV1::GenericPackageName {
        return Err(format!("generic name classified {:?}", validation.result).into());
    }

    let mut foreign = bootstrap_row("cargo-intent", "0.1.0", CANONICAL);
    foreign.family = allow_report::BootstrapRowFamilyV1::CargoIntent;
    let mixed = bootstrap_receipt(vec![
        bootstrap_row("effortless-repo-edit", "0.1.0", CANONICAL),
        foreign,
    ]);
    let validation = validate_crates_io_first_publication_bootstrap_receipt_v1(&mixed);
    if validation.result != CratesIoFirstPublicationBootstrapResultV1::ForeignPackage {
        return Err(format!("foreign package classified {:?}", validation.result).into());
    }

    let mut promoted = bootstrap_receipt(vec![bootstrap_row(
        "effortless-repo-edit",
        "0.1.0",
        CANONICAL,
    )]);
    promoted.promotion = allow_report::BootstrapPromotionV1::FinalReleaseEvidence;
    let validation = validate_crates_io_first_publication_bootstrap_receipt_v1(&promoted);
    if validation.result != CratesIoFirstPublicationBootstrapResultV1::PromotionRejected {
        return Err(format!("promotion classified {:?}", validation.result).into());
    }

    let mut stale = bootstrap_receipt(vec![bootstrap_row(
        "effortless-repo-edit",
        "0.1.0",
        CANONICAL,
    )]);
    stale.namespace_authority_generation = String::new();
    let validation = validate_crates_io_first_publication_bootstrap_receipt_v1(&stale);
    if validation.result != CratesIoFirstPublicationBootstrapResultV1::StaleBootstrapEvidence {
        return Err(format!("stale classified {:?}", validation.result).into());
    }
    if !bootstrap_receipt(vec![bootstrap_row(
        "effortless-repo-edit",
        "0.1.0",
        CANONICAL,
    )])
    .is_stale_against("namespace-evidence-2026-11-01")
    {
        return Err("older authority generation must read as stale".into());
    }

    let empty = bootstrap_receipt(Vec::new());
    let validation = validate_crates_io_first_publication_bootstrap_receipt_v1(&empty);
    if validation.result != CratesIoFirstPublicationBootstrapResultV1::Incomplete {
        return Err(format!("empty classified {:?}", validation.result).into());
    }

    let malformed = bootstrap_receipt(vec![bootstrap_row(
        "effortless-repo-edit",
        "0.1.0",
        "sha256:short",
    )]);
    let validation = validate_crates_io_first_publication_bootstrap_receipt_v1(&malformed);
    if validation.result != CratesIoFirstPublicationBootstrapResultV1::Malformed {
        return Err(format!("malformed digest classified {:?}", validation.result).into());
    }
    Ok(())
}

// --- registry install journey ------------------------------------------

fn journey_receipt() -> CratesIoRegistryInstallJourneyReceiptV1 {
    CratesIoRegistryInstallJourneyReceiptV1 {
        schema_id: CRATES_IO_REGISTRY_INSTALL_JOURNEY_RECEIPT_SCHEMA_ID.to_string(),
        schema_version: CRATES_IO_REGISTRY_INSTALL_JOURNEY_RECEIPT_SCHEMA_VERSION,
        repository: "EffortlessMetrics/cargo-allow".to_string(),
        release_version: "0.2.0".to_string(),
        platform: "x86_64-unknown-linux-gnu".to_string(),
        source_commit: commit(),
        source_tree: tree(),
        git_ref: "refs/tags/v0.2.0".to_string(),
        event: "push".to_string(),
        workflow_run_id: 3_269_836_393,
        workflow_attempt: 1,
        channel: RegistryInstallChannelV1::CratesIoRegistry,
        journey_steps: vec![
            RegistryJourneyStepV1 {
                id: "install".to_string(),
                exit_code: 0,
            },
            RegistryJourneyStepV1 {
                id: "version-readback".to_string(),
                exit_code: 0,
            },
            RegistryJourneyStepV1 {
                id: "doctor".to_string(),
                exit_code: 0,
            },
        ],
        resolved_graph: vec![
            RegistryResolvedRowV1 {
                package_name: "cargo-allow".to_string(),
                package_version: "0.2.0".to_string(),
                registry_checksum: Some(CANONICAL.to_string()),
            },
            RegistryResolvedRowV1 {
                package_name: "allow-core".to_string(),
                package_version: "0.1.0".to_string(),
                registry_checksum: Some(CANONICAL.to_string()),
            },
        ],
        limitations: vec!["windows posture is a separate platform row".to_string()],
        claim_boundary: "Registry-installed journey; distinct from candidate bytes.".to_string(),
    }
}

#[test]
fn install_journey_green_path_verifies() -> Result<(), Box<dyn Error>> {
    let validation = validate_crates_io_registry_install_journey_receipt_v1(&journey_receipt());
    if validation.result != CratesIoRegistryInstallJourneyResultV1::InstalledVerified {
        return Err(format!("green journey classified {:?}", validation.result).into());
    }
    if !validation.gaps.is_empty() {
        return Err(format!("green journey reported gaps {:?}", validation.gaps).into());
    }
    Ok(())
}

#[test]
fn install_journey_rejection_paths_fail_closed() -> Result<(), Box<dyn Error>> {
    let mut cancelled = journey_receipt();
    cancelled.journey_steps = vec![RegistryJourneyStepV1 {
        id: "install".to_string(),
        exit_code: 101,
    }];
    let validation = validate_crates_io_registry_install_journey_receipt_v1(&cancelled);
    if validation.result != CratesIoRegistryInstallJourneyResultV1::Incomplete {
        return Err(format!("failed step classified {:?}", validation.result).into());
    }

    let mut unresolved = journey_receipt();
    unresolved.resolved_graph = vec![RegistryResolvedRowV1 {
        package_name: "cargo-allow".to_string(),
        package_version: "0.2.0".to_string(),
        registry_checksum: None,
    }];
    let validation = validate_crates_io_registry_install_journey_receipt_v1(&unresolved);
    if validation.result != CratesIoRegistryInstallJourneyResultV1::ChecksumUnresolved {
        return Err(format!("unresolved classified {:?}", validation.result).into());
    }

    let mut empty_graph = journey_receipt();
    empty_graph.resolved_graph = Vec::new();
    let validation = validate_crates_io_registry_install_journey_receipt_v1(&empty_graph);
    if validation.result != CratesIoRegistryInstallJourneyResultV1::ChecksumUnresolved {
        return Err(format!("empty graph classified {:?}", validation.result).into());
    }

    let mut other_channel = journey_receipt();
    other_channel.channel = RegistryInstallChannelV1::Unsupported("local-path".to_string());
    let validation = validate_crates_io_registry_install_journey_receipt_v1(&other_channel);
    if validation.result != CratesIoRegistryInstallJourneyResultV1::UnsupportedInstallChannel {
        return Err(format!("channel classified {:?}", validation.result).into());
    }

    let mut stale_generation = journey_receipt();
    stale_generation.schema_version = 2;
    let validation = validate_crates_io_registry_install_journey_receipt_v1(&stale_generation);
    if validation.result != CratesIoRegistryInstallJourneyResultV1::UnsupportedGeneration {
        return Err(format!("wrong generation classified {:?}", validation.result).into());
    }

    let mut private = journey_receipt();
    private.claim_boundary = "journey ran from /home/runner/work only".to_string();
    let validation = validate_crates_io_registry_install_journey_receipt_v1(&private);
    if validation.result != CratesIoRegistryInstallJourneyResultV1::Malformed {
        return Err(format!("private path classified {:?}", validation.result).into());
    }
    Ok(())
}

// --- schema registration denominators ----------------------------------

struct SchemaRegistrationLaw {
    file_name: &'static str,
    schema_id: &'static str,
    title: &'static str,
    required: &'static [&'static str],
}

fn schema_registration_laws() -> [SchemaRegistrationLaw; 3] {
    [
        SchemaRegistrationLaw {
            file_name: "cargo-allow.crates-io-trusted-publishing-receipt.v1.schema.json",
            schema_id: CRATES_IO_TRUSTED_PUBLISHING_RECEIPT_SCHEMA_ID,
            title: "cargo-allow CratesIoTrustedPublishingReceiptV1",
            required: &[
                "schema_id",
                "schema_version",
                "repository",
                "workflow_name",
                "workflow_run_id",
                "workflow_attempt",
                "producer_step",
                "source_commit",
                "source_tree",
                "git_ref",
                "auth_class",
                "observed_at",
                "limitations",
                "claim_boundary",
            ],
        },
        SchemaRegistrationLaw {
            file_name: "cargo-allow.crates-io-first-publication-bootstrap-receipt.v1.schema.json",
            schema_id: CRATES_IO_FIRST_PUBLICATION_BOOTSTRAP_RECEIPT_SCHEMA_ID,
            title: "cargo-allow CratesIoFirstPublicationBootstrapReceiptV1",
            required: &[
                "schema_id",
                "schema_version",
                "repository",
                "source_commit",
                "source_tree",
                "workflow_run_id",
                "workflow_attempt",
                "observed_at",
                "namespace_authority_generation",
                "rows",
                "promotion",
                "retained_incident_digests",
                "limitations",
                "claim_boundary",
            ],
        },
        SchemaRegistrationLaw {
            file_name: "cargo-allow.crates-io-registry-install-journey-receipt.v1.schema.json",
            schema_id: CRATES_IO_REGISTRY_INSTALL_JOURNEY_RECEIPT_SCHEMA_ID,
            title: "cargo-allow CratesIoRegistryInstallJourneyReceiptV1",
            required: &[
                "schema_id",
                "schema_version",
                "repository",
                "release_version",
                "platform",
                "source_commit",
                "source_tree",
                "git_ref",
                "event",
                "workflow_run_id",
                "workflow_attempt",
                "channel",
                "journey_steps",
                "resolved_graph",
                "limitations",
                "claim_boundary",
            ],
        },
    ]
}

#[test]
fn schemas_satisfy_the_three_registration_denominators() -> Result<(), Box<dyn Error>> {
    let readme = repo_root("docs/schemas/README.md")?;
    for law in schema_registration_laws() {
        let schema_path = format!("docs/schemas/{}", law.file_name);
        let text = repo_root(&schema_path)?;
        let schema: serde_json::Value = serde_json::from_str(&text)?;
        let schema_id = schema
            .get("properties")
            .and_then(|properties| properties.get("schema_id"))
            .and_then(|schema_id| schema_id.get("const"))
            .and_then(|value| value.as_str())
            .ok_or_else(|| format!("{} pins no schema_id const", law.file_name))?;
        if schema_id != law.schema_id {
            return Err(format!(
                "{} pins {schema_id}, expected {}",
                law.file_name, law.schema_id
            )
            .into());
        }
        let title = schema
            .get("title")
            .and_then(|value| value.as_str())
            .ok_or_else(|| format!("{} carries no cargo-allow title", law.file_name))?;
        if title != law.title {
            return Err(format!("{} title drifted: {title}", law.file_name).into());
        }
        let id = schema
            .get("$id")
            .and_then(|value| value.as_str())
            .ok_or_else(|| format!("{} carries no $id", law.file_name))?;
        if !id.ends_with(law.file_name) {
            return Err(format!("{} $id {id} is not the file name", law.file_name).into());
        }
        let required = schema
            .get("required")
            .and_then(|value| value.as_array())
            .ok_or_else(|| format!("{} carries no required list", law.file_name))?;
        for field in law.required {
            if !required.iter().any(|item| item.as_str() == Some(*field)) {
                return Err(format!("{} does not require {field}", law.file_name).into());
            }
        }
        if schema
            .get("additionalProperties")
            .and_then(|value| value.as_bool())
            != Some(false)
        {
            return Err(format!("{} does not close additional properties", law.file_name).into());
        }
        let catalog_links_schema_file = readme.contains(law.file_name);
        let catalog_pins_schema_id = readme.contains(law.schema_id);
        if !catalog_links_schema_file || !catalog_pins_schema_id {
            return Err(format!(
                "README catalog lacks {} (file linked: {catalog_links_schema_file}, id pinned: {catalog_pins_schema_id})",
                law.file_name
            )
            .into());
        }
    }
    Ok(())
}

#[test]
fn manifest_consumption_remains_refused_for_nonexact_rows() -> Result<(), Box<dyn Error>> {
    // The #2496 adapters compose with the existing reconciled publication
    // law: a bootstrap-stale shared row can never enter a manifest row.
    let stale_shared = ReconciledPackagePublicationV1 {
        logical_id: "effortless-repo-edit".to_string(),
        package_name: "effortless-repo-edit".to_string(),
        package_version: "0.1.0".to_string(),
        release_order: 120,
        row_class: PackageRowClassV1::PublishedSharedPrerequisite,
        state: PublicationStateV1::VerifiedExisting,
        expected_checksum: CANONICAL.to_string(),
        observed_registry_checksum: Some(CONFLICTING.to_string()),
    };
    if stale_shared.is_manifest_ready() {
        return Err("conflicting shared row read as manifest-ready".into());
    }
    Ok(())
}
