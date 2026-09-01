use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use allow_report::{PackageRowClassV1, PublicationStateV1, ReconciledPackagePublicationV1};
use clap::Parser;
use serde::Serialize;

const RECONCILIATION_SCHEMA: &str = "cargo-allow.reconciled-package-publication.v1";

/// Classify one package publication for manifest construction (#3761).
#[derive(Debug, Clone, Parser)]
#[command(disable_version_flag = true)]
pub(crate) struct ReconcilePackagePublicationArgs {
    /// Logical identifier of the package row.
    #[arg(long)]
    logical_id: String,
    /// Cargo package name.
    #[arg(long)]
    package_name: String,
    /// Exact package version.
    #[arg(long)]
    package_version: String,
    /// Release order within the candidate.
    #[arg(long)]
    release_order: u32,
    /// Row class: cargo_allow_candidate or published_shared_prerequisite.
    #[arg(long)]
    row_class: String,
    /// Observed publication state: missing, published_verified,
    /// verified_existing, or provider_unavailable.
    #[arg(long)]
    state: String,
    /// Expected checksum per the row-class authority.
    #[arg(long)]
    expected_checksum: String,
    /// Observed registry checksum, when the registry was reachable.
    #[arg(long)]
    observed_registry_checksum: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ReconciliationProjectionV1 {
    schema: &'static str,
    logical_id: String,
    package_name: String,
    package_version: String,
    release_order: u32,
    row_class: String,
    state: String,
    classification: String,
    manifest_ready: bool,
}

fn parse_row_class(value: &str) -> Result<PackageRowClassV1, String> {
    match value {
        "cargo_allow_candidate" => Ok(PackageRowClassV1::CargoAllowCandidate),
        "published_shared_prerequisite" => Ok(PackageRowClassV1::PublishedSharedPrerequisite),
        other => Err(format!(
            "unknown row class {other:?}; expected cargo_allow_candidate or published_shared_prerequisite"
        )),
    }
}

fn parse_state(value: &str) -> Result<PublicationStateV1, String> {
    match value {
        "missing" => Ok(PublicationStateV1::Missing),
        "published_verified" => Ok(PublicationStateV1::PublishedVerified),
        "verified_existing" => Ok(PublicationStateV1::VerifiedExisting),
        "provider_unavailable" => Ok(PublicationStateV1::ProviderUnavailable),
        other => Err(format!(
            "unknown publication state {other:?}; expected missing, published_verified, verified_existing, or provider_unavailable"
        )),
    }
}

fn classification_name(classification: allow_report::PublicationClassificationV1) -> &'static str {
    use allow_report::PublicationClassificationV1 as C;
    match classification {
        C::CompleteExact => "complete_exact",
        C::Missing => "missing",
        C::Conflict => "checksum_conflict",
        C::Unavailable => "provider_unavailable",
        C::Stale => "stale_candidate",
        C::Mismatch => "evidence_mismatch",
    }
}

fn cmd(args: &ReconcilePackagePublicationArgs) -> CargoAllowResult<ReconciliationProjectionV1> {
    let row_class = parse_row_class(&args.row_class).map_err(|message| {
        CargoAllowError::with_kind(CargoAllowErrorKind::InvalidConfig, message)
    })?;
    let state = parse_state(&args.state).map_err(|message| {
        CargoAllowError::with_kind(CargoAllowErrorKind::InvalidConfig, message)
    })?;
    let publication = ReconciledPackagePublicationV1 {
        logical_id: args.logical_id.clone(),
        package_name: args.package_name.clone(),
        package_version: args.package_version.clone(),
        release_order: args.release_order,
        row_class,
        state,
        expected_checksum: args.expected_checksum.clone(),
        observed_registry_checksum: args.observed_registry_checksum.clone(),
    };
    let classification = publication.classify();
    Ok(ReconciliationProjectionV1 {
        schema: RECONCILIATION_SCHEMA,
        logical_id: publication.logical_id.clone(),
        package_name: publication.package_name.clone(),
        package_version: publication.package_version.clone(),
        release_order: publication.release_order,
        row_class: args.row_class.clone(),
        state: args.state.clone(),
        classification: classification_name(classification).to_string(),
        manifest_ready: classification == allow_report::PublicationClassificationV1::CompleteExact,
    })
}

pub(super) fn cmd_reconcile_package_publication(
    args: &ReconcilePackagePublicationArgs,
) -> CargoAllowResult<()> {
    let projection = cmd(args)?;
    let rendered = serde_json::to_string_pretty(&projection).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Internal,
            format!("failed to render reconciliation projection: {error}"),
        )
    })?;
    println!("{rendered}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_candidate_projection_is_manifest_ready() {
        let args = ReconcilePackagePublicationArgs {
            logical_id: "cargo-allow".to_string(),
            package_name: "cargo-allow".to_string(),
            package_version: "0.2.0-rc.1".to_string(),
            release_order: 100,
            row_class: "cargo_allow_candidate".to_string(),
            state: "published_verified".to_string(),
            expected_checksum: format!("sha256:{}", "a".repeat(64)),
            observed_registry_checksum: Some(format!("sha256:{}", "a".repeat(64))),
        };
        let projection = cmd(&args).expect("exact candidate must project");
        assert_eq!(projection.classification, "complete_exact");
        assert!(projection.manifest_ready);
    }

    #[test]
    fn conflicting_shared_projection_is_not_manifest_ready() {
        let args = ReconcilePackagePublicationArgs {
            logical_id: "effortless-repo-edit".to_string(),
            package_name: "effortless-repo-edit".to_string(),
            package_version: "0.1.0".to_string(),
            release_order: 120,
            row_class: "published_shared_prerequisite".to_string(),
            state: "verified_existing".to_string(),
            expected_checksum: format!("sha256:{}", "a".repeat(64)),
            observed_registry_checksum: Some(format!("sha256:{}", "b".repeat(64))),
        };
        let projection = cmd(&args).expect("well-formed input must project");
        assert_eq!(projection.classification, "checksum_conflict");
        assert!(!projection.manifest_ready);
    }

    #[test]
    fn unknown_state_fails_closed() {
        let args = ReconcilePackagePublicationArgs {
            logical_id: "x".to_string(),
            package_name: "x".to_string(),
            package_version: "0.1.0".to_string(),
            release_order: 1,
            row_class: "cargo_allow_candidate".to_string(),
            state: "mystery".to_string(),
            expected_checksum: "sha256:a".to_string(),
            observed_registry_checksum: None,
        };
        let error = cmd(&args).expect_err("unknown state must fail");
        assert!(error.to_string().contains("unknown publication state"));
    }

    #[test]
    fn missing_observation_with_provider_unavailable_projects_unavailable() {
        let args = ReconcilePackagePublicationArgs {
            logical_id: "effortless-repo-edit".to_string(),
            package_name: "effortless-repo-edit".to_string(),
            package_version: "0.1.0".to_string(),
            release_order: 120,
            row_class: "published_shared_prerequisite".to_string(),
            state: "provider_unavailable".to_string(),
            expected_checksum: format!("sha256:{}", "a".repeat(64)),
            observed_registry_checksum: None,
        };
        let projection = cmd(&args).expect("well-formed input must project");
        assert_eq!(projection.classification, "provider_unavailable");
        assert!(!projection.manifest_ready);
    }

    #[test]
    fn unknown_row_class_fails_closed() {
        let args = ReconcilePackagePublicationArgs {
            logical_id: "x".to_string(),
            package_name: "x".to_string(),
            package_version: "0.1.0".to_string(),
            release_order: 1,
            row_class: "mystery".to_string(),
            state: "verified_existing".to_string(),
            expected_checksum: "sha256:a".to_string(),
            observed_registry_checksum: None,
        };
        let error = cmd(&args).expect_err("unknown class must fail");
        assert!(error.to_string().contains("unknown row class"));
    }
}
