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
use std::io::Read;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub const INTENT_PROVIDER_ID: &str = "cargo-intent";
pub const CHANGE_STATUS_PAYLOAD_SCHEMA: &str = "cargo-intent.change-status.v1";

const PROVIDER_STDOUT_LIMIT: usize = 1024 * 1024;
const PROVIDER_STDERR_LIMIT: usize = 64 * 1024;
const PROVIDER_READ_CHUNK: usize = 8 * 1024;

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
    let envelope = match validate_provider_output(&output) {
        Ok(envelope) => envelope,
        Err(failure) => return fail_delegated(args, root, &snapshot, failure, started),
    };
    if let Some(expected) = args.expect_staged_identity.as_deref() {
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
) -> Result<BoundedProcessOutput, IntentDelegateFailure> {
    let mut command = Command::new(executable);
    command
        .arg("--root")
        .arg(root)
        .arg("--format")
        .arg("json")
        .arg("change")
        .arg("status")
        .arg("--staged")
        .arg("--phase")
        .arg("precommit")
        .arg("--analysis-receipt");
    run_with_timeout(&mut command, timeout)
}

#[derive(Debug)]
struct BoundedProcessOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    stdout_exceeded: bool,
    stderr_exceeded: bool,
}

#[derive(Debug)]
struct BoundedRead {
    bytes: Vec<u8>,
    exceeded: bool,
}

fn run_with_timeout(
    command: &mut Command,
    timeout: Duration,
) -> Result<BoundedProcessOutput, IntentDelegateFailure> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|err| {
        IntentDelegateFailure::new(
            IntentDelegateFailureClass::InstrumentFailure,
            format!("failed to spawn cargo-intent provider: {err}"),
        )
    })?;
    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            let cleanup = terminate_and_reap(&mut child);
            return Err(IntentDelegateFailure::new(
                IntentDelegateFailureClass::InstrumentFailure,
                format!("cargo-intent provider stdout pipe was unavailable; {cleanup}"),
            ));
        }
    };
    let stderr = match child.stderr.take() {
        Some(stderr) => stderr,
        None => {
            let cleanup = terminate_and_reap(&mut child);
            return Err(IntentDelegateFailure::new(
                IntentDelegateFailureClass::InstrumentFailure,
                format!("cargo-intent provider stderr pipe was unavailable; {cleanup}"),
            ));
        }
    };
    let stdout_reader = spawn_bounded_reader(stdout, PROVIDER_STDOUT_LIMIT, "stdout");
    let stderr_reader = spawn_bounded_reader(stderr, PROVIDER_STDERR_LIMIT, "stderr");
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let (stdout, stderr) = settle_readers(stdout_reader, stderr_reader)?;
                return Ok(BoundedProcessOutput {
                    status,
                    stdout: stdout.bytes,
                    stderr: stderr.bytes,
                    stdout_exceeded: stdout.exceeded,
                    stderr_exceeded: stderr.exceeded,
                });
            }
            Ok(None) if started.elapsed() >= timeout => {
                let cleanup = terminate_and_reap(&mut child);
                let reader_summary = settle_reader_summary(stdout_reader, stderr_reader);
                return Err(IntentDelegateFailure::new(
                    IntentDelegateFailureClass::Timeout,
                    format!(
                        "cargo-intent provider exceeded {}s timeout; {cleanup}; {reader_summary}",
                        timeout.as_secs()
                    ),
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(25)),
            Err(err) => {
                let cleanup = terminate_and_reap(&mut child);
                let reader_summary = settle_reader_summary(stdout_reader, stderr_reader);
                return Err(IntentDelegateFailure::new(
                    IntentDelegateFailureClass::InstrumentFailure,
                    format!(
                        "failed waiting for cargo-intent provider: {err}; {cleanup}; {reader_summary}"
                    ),
                ));
            }
        }
    }
}

fn spawn_bounded_reader<R>(
    reader: R,
    limit: usize,
    stream: &'static str,
) -> JoinHandle<Result<BoundedRead, IntentDelegateFailure>>
where
    R: Read + Send + 'static,
{
    std::thread::spawn(move || {
        read_bounded(reader, limit).map_err(|err| {
            IntentDelegateFailure::new(
                IntentDelegateFailureClass::InstrumentFailure,
                format!("failed draining cargo-intent provider {stream}: {err}"),
            )
        })
    })
}

fn read_bounded(mut reader: impl Read, limit: usize) -> std::io::Result<BoundedRead> {
    let mut bytes = Vec::with_capacity(limit.min(PROVIDER_READ_CHUNK));
    let mut exceeded = false;
    let mut buffer = [0_u8; PROVIDER_READ_CHUNK];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let retained = read.min(limit.saturating_sub(bytes.len()));
        bytes.extend_from_slice(&buffer[..retained]);
        if retained < read {
            exceeded = true;
        }
    }
    Ok(BoundedRead { bytes, exceeded })
}

fn settle_readers(
    stdout_reader: JoinHandle<Result<BoundedRead, IntentDelegateFailure>>,
    stderr_reader: JoinHandle<Result<BoundedRead, IntentDelegateFailure>>,
) -> Result<(BoundedRead, BoundedRead), IntentDelegateFailure> {
    let stdout = join_reader(stdout_reader, "stdout");
    let stderr = join_reader(stderr_reader, "stderr");
    match (stdout, stderr) {
        (Ok(stdout), Ok(stderr)) => Ok((stdout, stderr)),
        (Err(stdout_error), Ok(_)) => Err(stdout_error),
        (Ok(_), Err(stderr_error)) => Err(stderr_error),
        (Err(stdout_error), Err(stderr_error)) => Err(IntentDelegateFailure::new(
            IntentDelegateFailureClass::InstrumentFailure,
            format!("{stdout_error}; {stderr_error}"),
        )),
    }
}

fn join_reader(
    reader: JoinHandle<Result<BoundedRead, IntentDelegateFailure>>,
    stream: &'static str,
) -> Result<BoundedRead, IntentDelegateFailure> {
    reader.join().map_err(|_| {
        IntentDelegateFailure::new(
            IntentDelegateFailureClass::InstrumentFailure,
            format!("cargo-intent provider {stream} reader panicked"),
        )
    })?
}

fn settle_reader_summary(
    stdout_reader: JoinHandle<Result<BoundedRead, IntentDelegateFailure>>,
    stderr_reader: JoinHandle<Result<BoundedRead, IntentDelegateFailure>>,
) -> String {
    match settle_readers(stdout_reader, stderr_reader) {
        Ok((stdout, stderr)) => format!(
            "stdout_bytes={}; stdout_exceeded={}; stderr_bytes={}; stderr_exceeded={}",
            stdout.bytes.len(),
            stdout.exceeded,
            stderr.bytes.len(),
            stderr.exceeded
        ),
        Err(error) => format!("reader_cleanup={error}"),
    }
}

fn terminate_and_reap(child: &mut std::process::Child) -> String {
    let kill = match child.kill() {
        Ok(()) => "kill=ok".to_string(),
        Err(error) if error.kind() == std::io::ErrorKind::InvalidInput => {
            "kill=already-exited".to_string()
        }
        Err(error) => format!("kill=error:{error}"),
    };
    let wait = match child.wait() {
        Ok(status) => format!("wait={status}"),
        Err(error) => format!("wait=error:{error}"),
    };
    format!("{kill}; {wait}")
}

pub(crate) fn validate_provider_output(
    output: &BoundedProcessOutput,
) -> Result<AnalysisReceiptEnvelopeV1, IntentDelegateFailure> {
    if output.stdout_exceeded {
        return Err(IntentDelegateFailure::new(
            IntentDelegateFailureClass::MalformedOutput,
            format!(
                "provider_output_too_large: cargo-intent stdout exceeded {PROVIDER_STDOUT_LIMIT} bytes"
            ),
        ));
    }
    if output.stderr_exceeded {
        return Err(IntentDelegateFailure::new(
            IntentDelegateFailureClass::InstrumentFailure,
            format!(
                "provider_diagnostic_too_large: cargo-intent stderr exceeded {PROVIDER_STDERR_LIMIT} bytes"
            ),
        ));
    }
    if output.stdout.is_empty() {
        return Err(IntentDelegateFailure::new(
            IntentDelegateFailureClass::MalformedOutput,
            format!(
                "cargo-intent provider returned empty stdout; stderr_bytes={}",
                output.stderr.len()
            ),
        ));
    }
    let stdout = std::str::from_utf8(&output.stdout).map_err(|err| {
        IntentDelegateFailure::new(
            IntentDelegateFailureClass::MalformedOutput,
            format!("cargo-intent provider stdout is not UTF-8: {err}"),
        )
    })?;
    validate_envelope_text(stdout)
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
    use std::io::{Cursor, Write};

    const HELPER_MODE_ENV: &str = "CARGO_ALLOW_INTENT_PROVIDER_HELPER_MODE";

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

    fn success_status() -> Result<ExitStatus, String> {
        if cfg!(windows) {
            Command::new("cmd")
                .args(["/C", "exit", "0"])
                .status()
                .map_err(|err| err.to_string())
        } else {
            Command::new("true")
                .status()
                .map_err(|err| err.to_string())
        }
    }

    fn sample_output(stdout: Vec<u8>, stderr: Vec<u8>) -> Result<BoundedProcessOutput, String> {
        Ok(BoundedProcessOutput {
            status: success_status()?,
            stdout,
            stderr,
            stdout_exceeded: false,
            stderr_exceeded: false,
        })
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
        let output = sample_output(Vec::new(), b"provider error".to_vec())?;
        let failure = match validate_provider_output(&output) {
            Err(failure) => failure,
            Ok(_) => return Err("empty stdout should fail validation".to_string()),
        };
        if failure.class != IntentDelegateFailureClass::MalformedOutput {
            return Err(format!("expected MalformedOutput, got {:?}", failure.class));
        }
        Ok(())
    }

    #[test]
    fn bounded_reader_discards_bytes_after_limit() -> Result<(), String> {
        let input = vec![b'x'; 64];
        let result = read_bounded(Cursor::new(input), 8).map_err(|err| err.to_string())?;
        if result.bytes != vec![b'x'; 8] || !result.exceeded {
            return Err(format!("unexpected bounded read: {result:?}"));
        }
        Ok(())
    }

    #[test]
    fn rejects_provider_stdout_over_budget() -> Result<(), String> {
        let mut output = sample_output(b"{}".to_vec(), Vec::new())?;
        output.stdout_exceeded = true;
        let failure = match validate_provider_output(&output) {
            Err(failure) => failure,
            Ok(_) => return Err("oversized stdout should fail validation".to_string()),
        };
        if !failure.detail.contains("provider_output_too_large") {
            return Err(format!("unexpected failure: {failure}"));
        }
        Ok(())
    }

    #[test]
    fn rejects_provider_stderr_over_budget() -> Result<(), String> {
        let envelope = sample_envelope(INTENT_PROVIDER_ID, CHANGE_STATUS_PAYLOAD_SCHEMA);
        let json = serde_json::to_vec(&envelope).map_err(|err| err.to_string())?;
        let mut output = sample_output(json, Vec::new())?;
        output.stderr_exceeded = true;
        let failure = match validate_provider_output(&output) {
            Err(failure) => failure,
            Ok(_) => return Err("oversized stderr should fail validation".to_string()),
        };
        if !failure.detail.contains("provider_diagnostic_too_large") {
            return Err(format!("unexpected failure: {failure}"));
        }
        Ok(())
    }

    #[test]
    fn run_with_timeout_drains_large_stdout_and_stderr() -> Result<(), String> {
        let mut command = helper_command("large")?;
        let output = run_with_timeout(&mut command, Duration::from_secs(10))
            .map_err(|failure| failure.to_string())?;
        if !output.stdout_exceeded || !output.stderr_exceeded {
            return Err(format!("expected both streams over budget: {output:?}"));
        }
        Ok(())
    }

    #[test]
    fn run_with_timeout_kills_and_reaps_hung_provider() -> Result<(), String> {
        let mut command = helper_command("hang")?;
        let failure = match run_with_timeout(&mut command, Duration::from_millis(100)) {
            Err(failure) => failure,
            Ok(output) => return Err(format!("hung provider unexpectedly exited: {output:?}")),
        };
        if failure.class != IntentDelegateFailureClass::Timeout {
            return Err(format!("expected Timeout, got {failure}"));
        }
        Ok(())
    }

    fn helper_command(mode: &str) -> Result<Command, String> {
        let executable = std::env::current_exe().map_err(|err| err.to_string())?;
        let mut command = Command::new(executable);
        command
            .arg("provider_process_helper")
            .arg("--nocapture")
            .env(HELPER_MODE_ENV, mode);
        Ok(command)
    }

    #[test]
    fn provider_process_helper() -> Result<(), String> {
        let Ok(mode) = std::env::var(HELPER_MODE_ENV) else {
            return Ok(());
        };
        if mode == "hang" {
            std::thread::sleep(Duration::from_secs(30));
            return Ok(());
        }
        let stdout_bytes = PROVIDER_STDOUT_LIMIT + (128 * 1024);
        let stderr_bytes = PROVIDER_STDERR_LIMIT + (128 * 1024);
        write_repeated(std::io::stdout().lock(), b'x', stdout_bytes)?;
        write_repeated(std::io::stderr().lock(), b'y', stderr_bytes)?;
        std::process::exit(0);
    }

    fn write_repeated(mut writer: impl Write, byte: u8, mut remaining: usize) -> Result<(), String> {
        let chunk = [byte; PROVIDER_READ_CHUNK];
        while remaining > 0 {
            let write = remaining.min(chunk.len());
            writer
                .write_all(&chunk[..write])
                .map_err(|err| err.to_string())?;
            remaining -= write;
        }
        writer.flush().map_err(|err| err.to_string())
    }
}
