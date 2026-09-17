//! Validate one release authorization document against independently observed
//! frozen facts (#3790).
//!
//! The document lives outside the frozen source tree. Every load-bearing
//! identity (commit, tree, version, tag, freeze receipt bytes) is supplied by
//! the caller from independent observation — never read from the document —
//! so a mismatched document fails instead of being laundered into agreement.
//! This command performs no network access, reads no credentials, creates no
//! tags, uploads nothing, and mutates no live state.

use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use allow_report::{
    ReleaseAuthorizationConsumptionV1, ReleaseAuthorizationInputV1,
    ReleaseAuthorizationResultV1, compile_release_authorization_v1,
    render_release_authorization_v1, transition_authorization_consumption,
};
use clap::Parser;
use std::path::PathBuf;

fn invalid_authorization(message: impl Into<String>) -> CargoAllowError {
    CargoAllowError::with_kind(CargoAllowErrorKind::InvalidConfig, message.into())
}

fn read_json(path: &PathBuf, what: &str) -> CargoAllowResult<Vec<u8>> {
    std::fs::read(path).map_err(|error| {
        invalid_authorization(format!("{what} at {} reads: {error}", path.display()))
    })
}

/// Validate one out-of-tree authorization document for a single operation.
#[derive(Debug, Clone, Parser)]
#[command(disable_version_flag = true)]
pub(crate) struct ReleaseAuthorizationArgs {
    /// Authorization document JSON outside the frozen source tree.
    #[arg(long)]
    pub(super) document: PathBuf,
    /// Independently observed commit SHA (e.g. from git rev-parse).
    #[arg(long)]
    pub(super) expected_commit: String,
    /// Independently observed tree SHA.
    #[arg(long)]
    pub(super) expected_tree: String,
    /// Independently resolved release version.
    #[arg(long)]
    pub(super) expected_version: String,
    /// Independently resolved release tag.
    #[arg(long)]
    pub(super) expected_tag: String,
    /// Retained freeze receipt bytes; their digest must equal the claim.
    #[arg(long)]
    pub(super) freeze_receipt: PathBuf,
    /// Where to write the canonical compiled receipt JSON.
    #[arg(long)]
    pub(super) out_receipt: PathBuf,
    /// Optional consumption transition to apply to a document copy.
    #[arg(long, value_parser = parse_consumption)]
    pub(super) transition_to: Option<ReleaseAuthorizationConsumptionV1>,
    /// Where to write the transitioned document (required with --transition-to).
    #[arg(long)]
    pub(super) transition_out: Option<PathBuf>,
}

fn parse_consumption(value: &str) -> Result<ReleaseAuthorizationConsumptionV1, String> {
    use ReleaseAuthorizationConsumptionV1 as Consumption;
    match value {
        "available" => Ok(Consumption::Available),
        "selected-for-run" => Ok(Consumption::SelectedForRun),
        "irreversible-started" => Ok(Consumption::IrreversibleOperationStarted),
        "consumed-complete" => Ok(Consumption::ConsumedComplete),
        "consumed-incident" => Ok(Consumption::ConsumedIncident),
        "expired" => Ok(Consumption::Expired),
        "revoked" => Ok(Consumption::Revoked),
        _ => Err(format!(
            "unknown consumption state {value:?}; expected available, selected-for-run, \
             irreversible-started, consumed-complete, consumed-incident, expired, or revoked"
        )),
    }
}

pub(super) fn cmd_release_authorization(args: &ReleaseAuthorizationArgs) -> CargoAllowResult<()> {
    if args.transition_to.is_some() && args.transition_out.is_none() {
        return Err(invalid_authorization("--transition-out is required with --transition-to"));
    }
    let raw = read_json(&args.document, "authorization document")?;
    let document: ReleaseAuthorizationInputV1 = serde_json::from_slice(&raw).map_err(|error| {
        invalid_authorization(format!("authorization document parses: {error}"))
    })?;
    // Independent facts are compared, never merged: a document that disagrees
    // with observation fails instead of being rewritten into agreement.
    if document.freeze.commit != args.expected_commit {
        return Err(invalid_authorization(format!(
            "authorization commit {} differs from observed {}",
            document.freeze.commit, args.expected_commit
        )));
    }
    if document.freeze.tree != args.expected_tree {
        return Err(invalid_authorization(format!(
            "authorization tree {} differs from observed {}",
            document.freeze.tree, args.expected_tree
        )));
    }
    if document.operation.version != args.expected_version {
        return Err(invalid_authorization(format!(
            "authorization version {} differs from resolved {}",
            document.operation.version, args.expected_version
        )));
    }
    if document.operation.tag != args.expected_tag {
        return Err(invalid_authorization(format!(
            "authorization tag {} differs from resolved {}",
            document.operation.tag, args.expected_tag
        )));
    }
    let receipt_bytes = read_json(&args.freeze_receipt, "freeze receipt")?;
    let receipt_digest =
        allow_core::sha256_v1_bytes(&receipt_bytes).replacen("sha256:v1:", "sha256:", 1);
    if !receipt_digest.eq_ignore_ascii_case(&document.freeze.receipt_digest) {
        return Err(invalid_authorization(
            "freeze receipt bytes do not match the authorization claim",
        ));
    }
    let compiled = compile_release_authorization_v1(&document);
    if compiled.result != ReleaseAuthorizationResultV1::Complete {
        let reasons: Vec<&str> =
            compiled.findings.iter().map(|finding| finding.reason.as_str()).collect();
        return Err(invalid_authorization(format!(
            "authorization compiled as {:?}: {}",
            compiled.result,
            reasons.join("; ")
        )));
    }
    write_text(
        &args.out_receipt,
        &render_release_authorization_v1(&compiled).map_err(|error| {
            invalid_authorization(format!("compiled receipt renders: {error}"))
        })?,
    )?;
    let operation_name = document.operation.name.clone();
    if let (Some(next), Some(out_path)) = (args.transition_to, args.transition_out.as_ref()) {        let advanced = transition_authorization_consumption(document.authority.consumption, next)
            .map_err(|error| invalid_authorization(format!("consumption transition: {error}")))?;
        let mut transitioned = document;
        transitioned.authority.consumption = advanced;
        let rendered = serde_json::to_string_pretty(&transitioned).map_err(|error| {
            invalid_authorization(format!("transitioned document renders: {error}"))
        })?;
        write_text(out_path, &rendered)?;
    }
    println!(
        "authorization {} compiled complete for {}",
        compiled.authorization_digest, operation_name
    );
    Ok(())
}

fn write_text(path: &PathBuf, text: &str) -> CargoAllowResult<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|error| {
                invalid_authorization(format!(
                    "receipt parent {} creates: {error}",
                    parent.display()
                ))
            })?;
        }
    }
    std::fs::write(path, format!("{text}\n")).map_err(|error| {
        invalid_authorization(format!("receipt {} writes: {error}", path.display()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_report::{
        ReleaseAuthorizationAuthorityKindV1, ReleaseAuthorizationAuthorityV1,
        ReleaseAuthorizationEvidenceV1, ReleaseAuthorizationFreezeV1,
        ReleaseAuthorizationInputV1, ReleaseAuthorizationOperationV1,
        ReleaseAuthorizationPackageRowV1, ReleaseAuthorizationSecretAvailabilityV1,
        ReleaseAuthorizationSecretStateV1, ReleaseAuthorizationSharedRowV1,
        ReleaseAuthorizationSourceKindV1, ReleaseAuthorizationSourceV1,
        RELEASE_AUTHORIZATION_AUTH_CLASS, RELEASE_AUTHORIZATION_FINAL_OPERATION,
        RELEASE_AUTHORIZATION_FINAL_TAG, RELEASE_AUTHORIZATION_FINAL_VERSION,
        RELEASE_AUTHORIZATION_SCHEMA_ID, RELEASE_AUTHORIZATION_SCHEMA_VERSION,
        RELEASE_AUTHORIZATION_SELECTION, RELEASE_AUTHORIZATION_STABLE_CHANNEL,
        release_authorization_denominator_binding_v1,
    };
    use clap::CommandFactory;

    fn digest(n: u64) -> String {
        format!("sha256:{n:064x}")
    }

    fn synthetic_document() -> Result<ReleaseAuthorizationInputV1, String> {
        let mut packages = Vec::new();
        let mut shared = Vec::new();
        for (logical, package, version, is_shared) in RELEASE_AUTHORIZATION_SELECTION {
            if is_shared {
                shared.push(ReleaseAuthorizationSharedRowV1 {
                    logical_id: logical.to_string(),
                    package_name: package.to_string(),
                    package_version: version.to_string(),
                    expected_checksum: digest(20),
                    authority_digest: digest(21),
                });
            } else {
                packages.push(ReleaseAuthorizationPackageRowV1 {
                    logical_id: logical.to_string(),
                    package_name: package.to_string(),
                    package_version: version.to_string(),
                    package_digest: digest(10),
                    package_size_bytes: 10_000,
                });
            }
        }
        let mut document = ReleaseAuthorizationInputV1 {
            schema_id: RELEASE_AUTHORIZATION_SCHEMA_ID.to_string(),
            schema_version: RELEASE_AUTHORIZATION_SCHEMA_VERSION,
            operation: ReleaseAuthorizationOperationV1 {
                name: RELEASE_AUTHORIZATION_FINAL_OPERATION.to_string(),
                version: RELEASE_AUTHORIZATION_FINAL_VERSION.to_string(),
                tag: RELEASE_AUTHORIZATION_FINAL_TAG.to_string(),
                channel: RELEASE_AUTHORIZATION_STABLE_CHANNEL.to_string(),
                github_prerelease: false,
                authority_kind: ReleaseAuthorizationAuthorityKindV1::Clean,
            },
            freeze: ReleaseAuthorizationFreezeV1 {
                receipt_digest: digest(1),
                candidate_digest: digest(2),
                denominator_digest: String::new(),
                commit: "a".repeat(40),
                tree: "b".repeat(40),
                lock_digest: digest(4),
                topology_id: "CARGO-ALLOW-PKG-TOPOLOGY-V2-0001".to_string(),
                packages,
                shared_prerequisites: shared,
            },
            evidence: ReleaseAuthorizationEvidenceV1 {
                package_docs_digest: digest(30),
                preflight_result: allow_report::FinalRegistryPreflightResultV1::Complete,
                preflight_evaluated_at_unix_seconds: 100,
                preflight_maximum_age_seconds: 30,
                support_digest: digest(31),
                manifest_digest: digest(32),
                rehearsal_complete_except_authorization: true,
                rehearsal_digest: digest(33),
                source_controls_digest: digest(34),
                live_controls_digest: digest(35),
                workflow_digest: digest(36),
                action_inventory_digest: digest(37),
                observed_context_digest: digest(38),
                current_context_digest: digest(38),
            },
            authority: ReleaseAuthorizationAuthorityV1 {
                selected_auth_class: RELEASE_AUTHORIZATION_AUTH_CLASS.to_string(),
                secret_availability: ReleaseAuthorizationSecretAvailabilityV1 {
                    redacted: true,
                    state: ReleaseAuthorizationSecretStateV1::Unknown,
                },
                maintainer_actor: "release-operator".to_string(),
                maintainer_role: "release-maintainer".to_string(),
                source: ReleaseAuthorizationSourceV1 {
                    kind: ReleaseAuthorizationSourceKindV1::IssueComment,
                    repository: "EffortlessMetrics/cargo-allow".to_string(),
                    reference: "issue:2502#comment:1".to_string(),
                    author: "release-operator".to_string(),
                    body_digest: digest(40),
                    statement: "Authorize publish_cargo_allow_final_0_2_0.".to_string(),
                },
                created_at_unix_seconds: 90,
                expires_at_unix_seconds: 200,
                one_run_scope: true,
                nonce: "nonce-test-0001".to_string(),
                prior_consumptions: Vec::new(),
                consumption: ReleaseAuthorizationConsumptionV1::Available,
            },
            frozen_file_digests: vec![digest(50)],
            evaluated_at_unix_seconds: 110,
        };
        document.freeze.denominator_digest =
            release_authorization_denominator_binding_v1(&document.freeze)
                .map_err(|error| error.to_string())?;
        Ok(document)
    }

    fn scratch_dir(name: &str) -> Result<PathBuf, String> {
        let dir = std::env::temp_dir()
            .join(format!("cargo-allow-release-authorization-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
        Ok(dir)
    }

    fn write_args(dir: &PathBuf) -> Result<ReleaseAuthorizationArgs, String> {
        let mut document = synthetic_document()?;
        let document_path = dir.join("authorization.json");
        // The freeze receipt fixture is three bytes; bind its digest into the
        // typed document so byte-equality holds by construction.
        let receipt_bytes = b"{}";
        let receipt_path = dir.join("freeze.receipt.json");
        std::fs::write(&receipt_path, receipt_bytes).map_err(|error| error.to_string())?;
        document.freeze.receipt_digest =
            format!("sha256:{}", sha2_digest_hex(receipt_bytes));
        std::fs::write(
            &document_path,
            serde_json::to_string_pretty(&document).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        Ok(ReleaseAuthorizationArgs {
            document: document_path,
            expected_commit: "a".repeat(40),
            expected_tree: "b".repeat(40),
            expected_version: RELEASE_AUTHORIZATION_FINAL_VERSION.to_string(),
            expected_tag: RELEASE_AUTHORIZATION_FINAL_TAG.to_string(),
            freeze_receipt: receipt_path,
            out_receipt: dir.join("compiled.receipt.json"),
            transition_to: None,
            transition_out: None,
        })
    }

    fn sha2_digest_hex(bytes: &[u8]) -> String {
        use sha2::Digest;
        sha2::Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    }

    #[test]
    fn synthetic_document_validates_end_to_end() -> Result<(), String> {
        let dir = scratch_dir("valid")?;
        let args = write_args(&dir)?;
        cmd_release_authorization(&args).map_err(|error| error.to_string())?;
        let rendered = std::fs::read_to_string(&args.out_receipt).map_err(|error| error.to_string())?;
        let receipt: serde_json::Value =
            serde_json::from_str(&rendered).map_err(|error| error.to_string())?;
        if receipt.get("result").and_then(serde_json::Value::as_str) != Some("complete") {
            return Err(format!("synthetic document did not compile: {receipt}"));
        }
        Ok(())
    }

    #[test]
    fn observed_fact_mismatch_fails_without_laundering() -> Result<(), String> {
        let dir = scratch_dir("mismatch")?;
        let mut args = write_args(&dir)?;
        args.expected_commit = "c".repeat(40);
        let error = cmd_release_authorization(&args)
            .err()
            .ok_or_else(|| "mismatched commit was accepted".to_string())?;
        if error.kind() != CargoAllowErrorKind::InvalidConfig
            || !error.to_string().contains("differs from observed")
        {
            return Err(format!("unexpected mismatch error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn transition_rewrites_a_document_copy() -> Result<(), String> {
        let dir = scratch_dir("transition")?;
        let mut args = write_args(&dir)?;
        let transitioned = dir.join("transitioned.json");
        args.transition_to = Some(ReleaseAuthorizationConsumptionV1::SelectedForRun);
        args.transition_out = Some(transitioned.clone());
        cmd_release_authorization(&args).map_err(|error| error.to_string())?;
        let rewritten: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&transitioned).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        if rewritten.pointer("/authority/consumption").and_then(serde_json::Value::as_str)
            != Some("selected_for_run")
        {
            return Err(format!("transition was not applied: {rewritten}"));
        }
        Ok(())
    }


    #[test]
    fn command_is_installed_but_hidden_from_product_help() -> Result<(), String> {
        let mut command = crate::cli::CargoAllowCli::command();
        command.build();
        let subcommand = command
            .get_subcommands()
            .find(|subcommand| subcommand.get_name() == "release-authorization")
            .ok_or_else(|| "release-authorization command is not installed".to_string())?;
        if !subcommand.is_hide_set() {
            return Err("release-authorization must remain hidden".to_string());
        }
        let help = command.render_help().to_string();
        if help.contains("release-authorization") {
            return Err("hidden release-authorization leaked into root help".to_string());
        }
        Ok(())
    }

    #[test]
    fn transition_out_is_required_with_transition_to() -> Result<(), String> {
        let args = ReleaseAuthorizationArgs {
            document: PathBuf::from("auth.json"),
            expected_commit: "c".to_string(),
            expected_tree: "t".to_string(),
            expected_version: "0.2.0".to_string(),
            expected_tag: "v0.2.0".to_string(),
            freeze_receipt: PathBuf::from("freeze.json"),
            out_receipt: PathBuf::from("receipt.json"),
            transition_to: Some(ReleaseAuthorizationConsumptionV1::SelectedForRun),
            transition_out: None,
        };
        let error = cmd_release_authorization(&args)
            .err()
            .ok_or_else(|| "missing transition-out was accepted".to_string())?;
        if error.kind() != CargoAllowErrorKind::InvalidConfig {
            return Err(format!("unexpected error kind: {error}"));
        }
        Ok(())
    }

    #[test]
    fn unknown_consumption_states_fail_closed() -> Result<(), String> {
        if parse_consumption("published").is_ok() {
            return Err("unknown consumption state was accepted".to_string());
        }
        Ok(())
    }
}
