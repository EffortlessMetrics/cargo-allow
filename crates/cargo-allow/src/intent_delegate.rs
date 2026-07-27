//! One-way process delegation to cargo-intent for staged precommit (#2601-B).

use crate::check::CheckArgs;
use crate::intent_provider::{
    IntentDelegationSettings, IntentProviderFailureClass, IntentProviderRequest,
    discover_intent_provider, load_intent_delegation_settings,
};
use crate::resolve_source_tree_root;
use crate::spec_precommit::{DelegatedPrecommitOutcome, complete_delegated_precommit};
use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult, sha256_v1_bytes};
use allow_diff::staged_repository_snapshot;
use repo_protocol::{
    ANALYSIS_RECEIPT_SCHEMA_ID, AnalysisReceiptEnvelopeV1, REPOSITORY_SNAPSHOT_SCHEMA_ID,
};
use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

pub const INTENT_PROVIDER_ID: &str = "cargo-intent";
pub const CHANGE_STATUS_PAYLOAD_SCHEMA: &str = "cargo-intent.change-status.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntentDelegateFailureClass {
    ProviderAbsent,
    WrongProduct,
    WrongProtocol,
    MalformedOutput,
    Timeout,
    StaleSource,
    IdentityMismatch,
    InstrumentFailure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentDelegateFailure {
    pub class: IntentDelegateFailureClass,
    pub detail: String,
}

impl IntentDelegateFailure {
    fn new(class: IntentDelegateFailureClass, detail: impl Into<String>) -> Self {
        Self {
            class,
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for IntentDelegateFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.class, self.detail)
    }
}

pub enum DelegationDisposition {
    Disabled,
    Handle(CargoAllowResult<()>),
}

pub fn embedded_spec_system_cutover_active(
    root: &Path,
) -> Result<bool, crate::intent_provider::IntentProviderFailure> {
    Ok(
        match crate::intent_provider::load_intent_delegation_settings(root, None)? {
            Some(settings) => settings.delegate_spec_system,
            None => false,
        },
    )
}

pub fn reject_embedded_spec_system_authority(root: &Path, surface: &str) -> CargoAllowResult<()> {
    if embedded_spec_system_cutover_active(root).map_err(|failure| {
        CargoAllowError::with_kind(CargoAllowErrorKind::InvalidConfig, failure.to_string())
    })? {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!(
                "embedded spec-system {surface} authority is disabled while delegate_spec_system is enabled in {}; use cargo-intent or disable delegate_spec_system",
                crate::intent_provider::DEFAULT_INTENT_DELEGATION_CONFIG
            ),
        ));
    }
    Ok(())
}

pub fn reject_embedded_precommit_authority(root: &Path) -> CargoAllowResult<()> {
    reject_embedded_spec_system_authority(root, "precommit evaluator")
}

pub fn try_delegate_staged_precommit(
    args: &CheckArgs,
    started: Instant,
) -> CargoAllowResult<DelegationDisposition> {
    let root = resolve_source_tree_root(
        args.root.root.as_deref(),
        &std::env::current_dir()
            .map_err(|err| CargoAllowError::new(format!("failed to read cwd: {err}")))?,
    )?;
    let settings = match load_intent_delegation_settings(&root, None) {
        Ok(Some(settings)) if settings.delegate_staged_precommit => settings,
        Ok(_) => return Ok(DelegationDisposition::Disabled),
        Err(failure) => {
            return Ok(DelegationDisposition::Handle(Err(
                CargoAllowError::with_kind(CargoAllowErrorKind::InvalidConfig, failure.to_string()),
            )));
        }
    };
    Ok(DelegationDisposition::Handle(delegate_staged_precommit(
        args, &root, &settings, started,
    )))
}

fn delegate_staged_precommit(
    args: &CheckArgs,
    root: &Path,
    settings: &IntentDelegationSettings,
    started: Instant,
) -> CargoAllowResult<()> {
    let snapshot = staged_repository_snapshot(root)?;
    let provider = match discover_intent_provider(&IntentProviderRequest {
        root,
        config_path: Some(&settings.config_path),
        explicit_executable: None,
    }) {
        Ok(provider) => provider,
        Err(failure) => {
            return fail_delegated(
                args,
                root,
                &snapshot,
                map_provider_failure(failure),
                started,
            );
        }
    };
    if provider.executable_digest
        != match digest_executable(&provider.executable) {
            Ok(digest) => digest,
            Err(failure) => return fail_delegated(args, root, &snapshot, failure, started),
        }
    {
        return fail_delegated(
            args,
            root,
            &snapshot,
            IntentDelegateFailure::new(
                IntentDelegateFailureClass::IdentityMismatch,
                "provider executable digest changed before invocation",
            ),
            started,
        );
    }
    let output = match run_provider_change_status(
        &provider.executable,
        root,
        Duration::from_secs(settings.timeout_secs),
    ) {
        Ok(output) => output,
        Err(failure) => return fail_delegated(args, root, &snapshot, failure, started),
    };
    if let Some(expected) = args.expect_staged_identity.as_deref() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Ok(envelope) = parse_envelope(stdout.as_ref()) {
            let payload_identity = envelope
                .provider_payload
                .get("staged_identity")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            if payload_identity != expected {
                return fail_delegated(
                    args,
                    root,
                    &snapshot,
                    IntentDelegateFailure::new(
                        IntentDelegateFailureClass::StaleSource,
                        "provider staged_identity did not match --expect-staged-identity",
                    ),
                    started,
                );
            }
        }
    }
    let envelope = match validate_provider_output(&output) {
        Ok(envelope) => envelope,
        Err(failure) => return fail_delegated(args, root, &snapshot, failure, started),
    };
    if match digest_executable(&provider.executable) {
        Ok(digest) => digest,
        Err(failure) => return fail_delegated(args, root, &snapshot, failure, started),
    } != provider.executable_digest
    {
        return fail_delegated(
            args,
            root,
            &snapshot,
            IntentDelegateFailure::new(
                IntentDelegateFailureClass::IdentityMismatch,
                "provider executable digest changed after invocation",
            ),
            started,
        );
    }
    let outcome = map_envelope_to_outcome(&envelope, output.status.success());
    complete_delegated_precommit(args, root, &snapshot, outcome, started.elapsed())
}

fn run_provider_change_status(
    executable: &Path,
    root: &Path,
    timeout: Duration,
) -> Result<Output, IntentDelegateFailure> {
    let root_text = root
        .to_str()
        .ok_or_else(|| {
            IntentDelegateFailure::new(
                IntentDelegateFailureClass::MalformedOutput,
                "repository root is not UTF-8",
            )
        })?
        .to_string();
    let mut command = Command::new(executable);
    command
        .arg("--root")
        .arg(root_text)
        .arg("--format")
        .arg("json")
        .arg("change")
        .arg("status")
        .arg("--staged")
        .arg("--phase")
        .arg("precommit")
        .arg("--analysis-receipt")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    run_with_timeout(&mut command, timeout)
}

fn run_with_timeout(
    command: &mut Command,
    timeout: Duration,
) -> Result<Output, IntentDelegateFailure> {
    let mut child = command.spawn().map_err(|err| {
        IntentDelegateFailure::new(
            IntentDelegateFailureClass::InstrumentFailure,
            format!("failed to spawn cargo-intent provider: {err}"),
        )
    })?;
    let started = Instant::now();
    loop {
        match child.try_wait().map_err(|err| {
            IntentDelegateFailure::new(
                IntentDelegateFailureClass::InstrumentFailure,
                format!("failed waiting for cargo-intent provider: {err}"),
            )
        })? {
            Some(_) => {
                return child.wait_with_output().map_err(|err| {
                    IntentDelegateFailure::new(
                        IntentDelegateFailureClass::InstrumentFailure,
                        format!("failed reading cargo-intent provider output: {err}"),
                    )
                });
            }
            None if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(IntentDelegateFailure::new(
                    IntentDelegateFailureClass::Timeout,
                    format!(
                        "cargo-intent provider exceeded {}s timeout",
                        timeout.as_secs()
                    ),
                ));
            }
            None => std::thread::sleep(Duration::from_millis(25)),
        }
    }
}

pub(crate) fn validate_provider_output(
    output: &Output,
) -> Result<AnalysisReceiptEnvelopeV1, IntentDelegateFailure> {
    if output.stdout.is_empty() {
        return Err(IntentDelegateFailure::new(
            IntentDelegateFailureClass::MalformedOutput,
            format!(
                "cargo-intent provider returned empty stdout; stderr={}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ));
    }
    let stdout = String::from_utf8(output.stdout.clone()).map_err(|err| {
        IntentDelegateFailure::new(
            IntentDelegateFailureClass::MalformedOutput,
            format!("cargo-intent provider stdout is not UTF-8: {err}"),
        )
    })?;
    validate_envelope_text(&stdout)
}

pub(crate) fn validate_envelope_text(
    stdout: &str,
) -> Result<AnalysisReceiptEnvelopeV1, IntentDelegateFailure> {
    let envelope: AnalysisReceiptEnvelopeV1 = parse_envelope(stdout)?;
    if envelope.schema_id != ANALYSIS_RECEIPT_SCHEMA_ID {
        return Err(IntentDelegateFailure::new(
            IntentDelegateFailureClass::WrongProtocol,
            format!(
                "expected envelope schema_id {ANALYSIS_RECEIPT_SCHEMA_ID}, got {}",
                envelope.schema_id
            ),
        ));
    }
    if envelope.provider != INTENT_PROVIDER_ID {
        return Err(IntentDelegateFailure::new(
            IntentDelegateFailureClass::WrongProduct,
            format!(
                "expected provider {INTENT_PROVIDER_ID}, got {}",
                envelope.provider
            ),
        ));
    }
    if envelope.provider_payload_schema != CHANGE_STATUS_PAYLOAD_SCHEMA {
        return Err(IntentDelegateFailure::new(
            IntentDelegateFailureClass::WrongProtocol,
            format!(
                "expected provider_payload_schema {CHANGE_STATUS_PAYLOAD_SCHEMA}, got {}",
                envelope.provider_payload_schema
            ),
        ));
    }
    if envelope.snapshot.schema_id != REPOSITORY_SNAPSHOT_SCHEMA_ID {
        return Err(IntentDelegateFailure::new(
            IntentDelegateFailureClass::WrongProtocol,
            format!(
                "expected snapshot schema_id {REPOSITORY_SNAPSHOT_SCHEMA_ID}, got {}",
                envelope.snapshot.schema_id
            ),
        ));
    }
    if !envelope.provider_payload.is_object() {
        return Err(IntentDelegateFailure::new(
            IntentDelegateFailureClass::MalformedOutput,
            "provider_payload must be a JSON object",
        ));
    }
    Ok(envelope)
}

fn parse_envelope(stdout: &str) -> Result<AnalysisReceiptEnvelopeV1, IntentDelegateFailure> {
    serde_json::from_str(stdout).map_err(|err| {
        IntentDelegateFailure::new(
            IntentDelegateFailureClass::MalformedOutput,
            format!("failed to parse repo.analysis-receipt.v1 envelope: {err}"),
        )
    })
}

fn map_envelope_to_outcome(
    envelope: &AnalysisReceiptEnvelopeV1,
    exit_success: bool,
) -> DelegatedPrecommitOutcome {
    let payload = &envelope.provider_payload;
    let staged_identity = payload
        .get("staged_identity")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let process_exit_family = payload
        .get("process_exit_family")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("instrument_failure")
        .to_string();
    let provider_claim = payload
        .get("claim_boundary")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let unmapped = payload
        .get("unmapped_staged_surface")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    DelegatedPrecommitOutcome {
        result_class: envelope.result_class,
        exit_success,
        staged_identity,
        process_exit_family,
        provider_claim_boundary: provider_claim,
        unmapped_staged_surface: unmapped,
        error: None,
    }
}

fn digest_executable(path: &Path) -> Result<String, IntentDelegateFailure> {
    let bytes = fs::read(path).map_err(|err| {
        IntentDelegateFailure::new(
            IntentDelegateFailureClass::InstrumentFailure,
            format!("read provider executable {}: {err}", path.display()),
        )
    })?;
    Ok(sha256_v1_bytes(&bytes))
}

fn map_provider_failure(
    failure: crate::intent_provider::IntentProviderFailure,
) -> IntentDelegateFailure {
    let class = match failure.class {
        IntentProviderFailureClass::Absent => IntentDelegateFailureClass::ProviderAbsent,
        IntentProviderFailureClass::WrongProductName => IntentDelegateFailureClass::WrongProduct,
        IntentProviderFailureClass::MalformedConfig => IntentDelegateFailureClass::WrongProtocol,
        IntentProviderFailureClass::ForbiddenWorkspaceTarget
        | IntentProviderFailureClass::ForbiddenWorkspaceCrate
        | IntentProviderFailureClass::NotExecutable => {
            IntentDelegateFailureClass::InstrumentFailure
        }
    };
    IntentDelegateFailure::new(class, failure.to_string())
}

fn fail_delegated(
    args: &CheckArgs,
    root: &Path,
    snapshot: &allow_diff::StagedRepositorySnapshot,
    failure: IntentDelegateFailure,
    started: Instant,
) -> CargoAllowResult<()> {
    if let Err(report_error) = complete_delegated_precommit(
        args,
        root,
        snapshot,
        DelegatedPrecommitOutcome::from_delegate_failure(&failure),
        started.elapsed(),
    ) {
        eprintln!("warning: failed to write delegated precommit report: {report_error}");
    }
    Err(CargoAllowError::with_kind(
        delegate_error_kind(failure.class),
        failure.to_string(),
    ))
}

fn delegate_error_kind(class: IntentDelegateFailureClass) -> CargoAllowErrorKind {
    match class {
        IntentDelegateFailureClass::ProviderAbsent => CargoAllowErrorKind::InvalidConfig,
        IntentDelegateFailureClass::WrongProduct
        | IntentDelegateFailureClass::WrongProtocol
        | IntentDelegateFailureClass::MalformedOutput => CargoAllowErrorKind::InvalidConfig,
        IntentDelegateFailureClass::Timeout | IntentDelegateFailureClass::InstrumentFailure => {
            CargoAllowErrorKind::Internal
        }
        IntentDelegateFailureClass::StaleSource => CargoAllowErrorKind::Inventory,
        IntentDelegateFailureClass::IdentityMismatch => CargoAllowErrorKind::InvalidConfig,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use repo_protocol::{
        ClaimBoundaryV1, CompletenessV1, CurrentnessV1, RepositorySnapshotV1, ResolvedRevisionV1,
        ResultClassV1,
    };

    fn sample_envelope(provider: &str, payload_schema: &str) -> AnalysisReceiptEnvelopeV1 {
        AnalysisReceiptEnvelopeV1 {
            schema_id: ANALYSIS_RECEIPT_SCHEMA_ID.to_string(),
            provider: provider.to_string(),
            snapshot: RepositorySnapshotV1::new_committed_head(
                "identity",
                "sha1",
                ResolvedRevisionV1 {
                    requested: "HEAD".to_string(),
                    commit: "0000000000000000000000000000000000000000".to_string(),
                    tree: String::new(),
                },
            ),
            result_class: ResultClassV1::Completed,
            completeness: CompletenessV1::Complete,
            currentness: CurrentnessV1::Current,
            provider_payload_schema: payload_schema.to_string(),
            provider_payload: serde_json::json!({
                "staged_identity": "abc",
                "process_exit_family": "success",
                "claim_boundary": "test",
                "unmapped_staged_surface": false,
            }),
            claim_boundary: ClaimBoundaryV1::new("test"),
        }
    }

    #[test]
    fn rejects_wrong_provider_product() -> Result<(), String> {
        let envelope = sample_envelope("cargo-proof", CHANGE_STATUS_PAYLOAD_SCHEMA);
        let json = serde_json::to_string(&envelope).map_err(|err| err.to_string())?;
        let failure = match validate_envelope_text(&json) {
            Err(failure) => failure,
            Ok(_) => return Err("wrong provider should fail validation".to_string()),
        };
        if failure.class != IntentDelegateFailureClass::WrongProduct {
            return Err(format!("expected WrongProduct, got {:?}", failure.class));
        }
        Ok(())
    }

    #[test]
    fn rejects_wrong_payload_schema() -> Result<(), String> {
        let envelope = sample_envelope(INTENT_PROVIDER_ID, "cargo-proof.payload.v1");
        let json = serde_json::to_string(&envelope).map_err(|err| err.to_string())?;
        let failure = match validate_envelope_text(&json) {
            Err(failure) => failure,
            Ok(_) => return Err("wrong schema should fail validation".to_string()),
        };
        if failure.class != IntentDelegateFailureClass::WrongProtocol {
            return Err(format!("expected WrongProtocol, got {:?}", failure.class));
        }
        Ok(())
    }

    #[test]
    fn rejects_malformed_json() -> Result<(), String> {
        let failure = match validate_envelope_text("{not-json") {
            Err(failure) => failure,
            Ok(_) => return Err("malformed json should fail validation".to_string()),
        };
        if failure.class != IntentDelegateFailureClass::MalformedOutput {
            return Err(format!("expected MalformedOutput, got {:?}", failure.class));
        }
        Ok(())
    }

    #[test]
    fn rejects_empty_provider_stdout() -> Result<(), String> {
        let status = if cfg!(windows) {
            std::process::Command::new("cmd")
                .args(["/C", "exit", "0"])
                .status()
                .map_err(|err| err.to_string())?
        } else {
            std::process::Command::new("true")
                .status()
                .map_err(|err| err.to_string())?
        };
        let output = Output {
            status,
            stdout: Vec::new(),
            stderr: b"provider error".to_vec(),
        };
        let failure = match validate_provider_output(&output) {
            Err(failure) => failure,
            Ok(_) => return Err("empty stdout should fail validation".to_string()),
        };
        if failure.class != IntentDelegateFailureClass::MalformedOutput {
            return Err(format!("expected MalformedOutput, got {:?}", failure.class));
        }
        Ok(())
    }
}
