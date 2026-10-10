//! One-way process delegation to cargo-intent for staged precommit (#2601-B).

use crate::check::CheckArgs;
use crate::intent_provider::{
    IntentDelegationSettings, IntentProviderFailureClass, IntentProviderRequest,
    discover_intent_provider, load_intent_delegation_settings,
};
use crate::spec_precommit::{DelegatedPrecommitOutcome, complete_delegated_precommit};
use crate::{current_dir, resolve_source_tree_root};
use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult, sha256_v1_bytes};
use effortless_repo_protocol::{
    ANALYSIS_RECEIPT_SCHEMA_ID, AnalysisReceiptEnvelopeV1, REPOSITORY_SNAPSHOT_SCHEMA_ID,
};
use effortless_repo_snapshot::{StagedRepositorySnapshot, staged_repository_snapshot};
use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub const INTENT_PROVIDER_ID: &str = "cargo-intent";
pub const CHANGE_STATUS_PAYLOAD_SCHEMA: &str = "cargo-intent.change-status.v1";

const PROVIDER_STDOUT_LIMIT: usize = 1024 * 1024;
const PROVIDER_STDERR_LIMIT: usize = 64 * 1024;
const PROVIDER_READ_CHUNK: usize = 8 * 1024;
const PROVIDER_READER_SETTLE_TIMEOUT: Duration = Duration::from_secs(1);

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntentDelegateFailureCode {
    ProviderAbsent,
    WrongProduct,
    WrongProtocol,
    MalformedProviderOutput,
    ProviderOutputTooLarge,
    ProviderDiagnosticTooLarge,
    ReaderSettleTimeout,
    ProviderTimeout,
    StaleSource,
    IdentityMismatch,
    ProviderInstrumentFailure,
}

impl IntentDelegateFailureCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProviderAbsent => "provider_absent",
            Self::WrongProduct => "wrong_product",
            Self::WrongProtocol => "wrong_protocol",
            Self::MalformedProviderOutput => "malformed_provider_output",
            Self::ProviderOutputTooLarge => "provider_output_too_large",
            Self::ProviderDiagnosticTooLarge => "provider_diagnostic_too_large",
            Self::ReaderSettleTimeout => "reader_settle_timeout",
            Self::ProviderTimeout => "provider_timeout",
            Self::StaleSource => "stale_source",
            Self::IdentityMismatch => "identity_mismatch",
            Self::ProviderInstrumentFailure => "provider_instrument_failure",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentDelegateFailure {
    pub class: IntentDelegateFailureClass,
    pub code: IntentDelegateFailureCode,
    pub detail: String,
}

impl IntentDelegateFailure {
    fn new(class: IntentDelegateFailureClass, detail: impl Into<String>) -> Self {
        Self {
            class,
            code: default_failure_code(class),
            detail: detail.into(),
        }
    }

    fn with_code(
        class: IntentDelegateFailureClass,
        code: IntentDelegateFailureCode,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            class,
            code,
            detail: detail.into(),
        }
    }
}

const fn default_failure_code(class: IntentDelegateFailureClass) -> IntentDelegateFailureCode {
    match class {
        IntentDelegateFailureClass::ProviderAbsent => IntentDelegateFailureCode::ProviderAbsent,
        IntentDelegateFailureClass::WrongProduct => IntentDelegateFailureCode::WrongProduct,
        IntentDelegateFailureClass::WrongProtocol => IntentDelegateFailureCode::WrongProtocol,
        IntentDelegateFailureClass::MalformedOutput => {
            IntentDelegateFailureCode::MalformedProviderOutput
        }
        IntentDelegateFailureClass::Timeout => IntentDelegateFailureCode::ProviderTimeout,
        IntentDelegateFailureClass::StaleSource => IntentDelegateFailureCode::StaleSource,
        IntentDelegateFailureClass::IdentityMismatch => IntentDelegateFailureCode::IdentityMismatch,
        IntentDelegateFailureClass::InstrumentFailure => {
            IntentDelegateFailureCode::ProviderInstrumentFailure
        }
    }
}

impl std::fmt::Display for IntentDelegateFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.detail)
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
    let root = resolve_source_tree_root(args.root.root.as_deref(), &current_dir()?)?;
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
    let snapshot = crate::command_support::snapshot_result(staged_repository_snapshot(root))?;
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
    let mut command = provider_change_status_command(executable, root);
    run_with_timeout(&mut command, timeout)
}

fn provider_change_status_command(executable: &Path, root: &Path) -> Command {
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
    command
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

struct ReaderTask {
    receiver: Receiver<Result<BoundedRead, IntentDelegateFailure>>,
    // Dropping a JoinHandle detaches the thread. Settlement is driven by the
    // bounded receiver below; an inherited provider pipe can never force an
    // unbounded join in the parent.
    _handle: JoinHandle<()>,
    stream: &'static str,
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

fn spawn_bounded_reader<R>(reader: R, limit: usize, stream: &'static str) -> ReaderTask
where
    R: Read + Send + 'static,
{
    let (sender, receiver) = mpsc::sync_channel(1);
    let handle = std::thread::spawn(move || {
        let result = read_bounded(reader, limit).map_err(|err| {
            IntentDelegateFailure::new(
                IntentDelegateFailureClass::InstrumentFailure,
                format!("failed draining cargo-intent provider {stream}: {err}"),
            )
        });
        // A disconnected receiver means the parent already made a bounded
        // cleanup decision. No secondary error path can improve that result.
        let _ = sender.send(result);
    });
    ReaderTask {
        receiver,
        _handle: handle,
        stream,
    }
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
        bytes.extend(buffer.iter().copied().take(retained));
        if retained < read {
            exceeded = true;
        }
    }
    Ok(BoundedRead { bytes, exceeded })
}

fn settle_readers(
    stdout_reader: ReaderTask,
    stderr_reader: ReaderTask,
) -> Result<(BoundedRead, BoundedRead), IntentDelegateFailure> {
    let deadline = Instant::now() + PROVIDER_READER_SETTLE_TIMEOUT;
    let stdout = receive_reader(stdout_reader, deadline);
    let stderr = receive_reader(stderr_reader, deadline);
    match (stdout, stderr) {
        (Ok(stdout), Ok(stderr)) => Ok((stdout, stderr)),
        (Err(stdout_error), Ok(_)) => Err(stdout_error),
        (Ok(_), Err(stderr_error)) => Err(stderr_error),
        (Err(stdout_error), Err(stderr_error)) => Err(IntentDelegateFailure::with_code(
            IntentDelegateFailureClass::InstrumentFailure,
            if stdout_error.code == IntentDelegateFailureCode::ReaderSettleTimeout
                || stderr_error.code == IntentDelegateFailureCode::ReaderSettleTimeout
            {
                IntentDelegateFailureCode::ReaderSettleTimeout
            } else {
                IntentDelegateFailureCode::ProviderInstrumentFailure
            },
            format!("{stdout_error}; {stderr_error}"),
        )),
    }
}

fn receive_reader(
    task: ReaderTask,
    deadline: Instant,
) -> Result<BoundedRead, IntentDelegateFailure> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    match task.receiver.recv_timeout(remaining) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => Err(IntentDelegateFailure::with_code(
            IntentDelegateFailureClass::InstrumentFailure,
            IntentDelegateFailureCode::ReaderSettleTimeout,
            format!(
                "cargo-intent provider {} reader did not settle within {}ms; a descendant may still hold the pipe open",
                task.stream,
                PROVIDER_READER_SETTLE_TIMEOUT.as_millis()
            ),
        )),
        Err(RecvTimeoutError::Disconnected) => Err(IntentDelegateFailure::new(
            IntentDelegateFailureClass::InstrumentFailure,
            format!(
                "cargo-intent provider {} reader ended without a result",
                task.stream
            ),
        )),
    }
}

fn settle_reader_summary(stdout_reader: ReaderTask, stderr_reader: ReaderTask) -> String {
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

fn validate_provider_output(
    output: &BoundedProcessOutput,
) -> Result<AnalysisReceiptEnvelopeV1, IntentDelegateFailure> {
    if output.stdout_exceeded {
        return Err(IntentDelegateFailure::with_code(
            IntentDelegateFailureClass::InstrumentFailure,
            IntentDelegateFailureCode::ProviderOutputTooLarge,
            format!("cargo-intent stdout exceeded {PROVIDER_STDOUT_LIMIT} bytes"),
        ));
    }
    if output.stderr_exceeded {
        return Err(IntentDelegateFailure::with_code(
            IntentDelegateFailureClass::InstrumentFailure,
            IntentDelegateFailureCode::ProviderDiagnosticTooLarge,
            format!("cargo-intent stderr exceeded {PROVIDER_STDERR_LIMIT} bytes"),
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
    // Surface the bounded user-facing action so absent/incompatible-version
    // failures arrive with the canonical command, version range, and the
    // explicit "NOT confirmed clean" statement instead of a bare detail line.
    IntentDelegateFailure::new(class, failure.bounded_action())
}

fn fail_delegated(
    args: &CheckArgs,
    root: &Path,
    snapshot: &StagedRepositorySnapshot,
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
    use effortless_repo_protocol::{
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
            Command::new("true").status().map_err(|err| err.to_string())
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
    fn rejects_provider_stdout_over_budget_with_typed_code() -> Result<(), String> {
        let mut output = sample_output(b"{}".to_vec(), Vec::new())?;
        output.stdout_exceeded = true;
        let failure = match validate_provider_output(&output) {
            Err(failure) => failure,
            Ok(_) => return Err("oversized stdout should fail validation".to_string()),
        };
        if failure.code != IntentDelegateFailureCode::ProviderOutputTooLarge {
            return Err(format!("unexpected failure: {failure}"));
        }
        Ok(())
    }

    #[test]
    fn rejects_provider_stderr_over_budget_with_typed_code() -> Result<(), String> {
        let envelope = sample_envelope(INTENT_PROVIDER_ID, CHANGE_STATUS_PAYLOAD_SCHEMA);
        let json = serde_json::to_vec(&envelope).map_err(|err| err.to_string())?;
        let mut output = sample_output(json, Vec::new())?;
        output.stderr_exceeded = true;
        let failure = match validate_provider_output(&output) {
            Err(failure) => failure,
            Ok(_) => return Err("oversized stderr should fail validation".to_string()),
        };
        if failure.code != IntentDelegateFailureCode::ProviderDiagnosticTooLarge {
            return Err(format!("unexpected failure: {failure}"));
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn run_provider_change_status_preserves_non_utf8_root_arg() -> Result<(), String> {
        assert_native_root_probe(&native_probe_root())
    }

    #[cfg(unix)]
    fn native_probe_root() -> std::path::PathBuf {
        use std::os::unix::ffi::OsStringExt;

        // argv has no filesystem UTF-8 restriction, including on macOS. Spaces,
        // quotes and a newline also discriminate splitting and shell evaluation.
        std::ffi::OsString::from_vec(b"/cargo-allow native '$probe'\nrepo-\xff".to_vec()).into()
    }

    #[cfg(unix)]
    fn assert_native_root_probe(root: &Path) -> Result<(), String> {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStrExt;

        let expected: Vec<OsString> = vec![
            "--root".into(),
            root.as_os_str().to_owned(),
            "--format".into(),
            "json".into(),
            "change".into(),
            "status".into(),
            "--staged".into(),
            "--phase".into(),
            "precommit".into(),
            "--analysis-receipt".into(),
        ];
        // /bin/echo is an installed immutable probe on the supported Unix CI
        // platforms. Never write or copy an executable for this byte oracle.
        let executable = Path::new("/bin/echo");
        let command = provider_change_status_command(executable, root);
        if !command
            .get_args()
            .eq(expected.iter().map(OsString::as_os_str))
        {
            return Err("provider argument boundaries or native root bytes changed".to_string());
        }
        let output = run_provider_change_status(executable, root, Duration::from_secs(5))
            .map_err(|failure| failure.to_string())?;
        if !output.status.success()
            || output.stdout_exceeded
            || output.stderr_exceeded
            || !output.stderr.is_empty()
        {
            return Err(format!("native argument probe failed: {output:?}"));
        }
        let mut expected_stdout = Vec::new();
        for argument in &expected {
            if !expected_stdout.is_empty() {
                expected_stdout.push(b' ');
            }
            expected_stdout.extend_from_slice(argument.as_os_str().as_bytes());
        }
        expected_stdout.push(b'\n');
        if output.stdout != expected_stdout {
            return Err(
                "bounded provider runner did not preserve exact native argv bytes".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn run_provider_change_status_preserves_typed_spawn_failure() -> Result<(), String> {
        let executable = std::env::current_exe()
            .map_err(|err| err.to_string())?
            .with_file_name(format!(
                "cargo-allow-missing-provider-{}",
                std::process::id()
            ));
        if executable.exists() {
            return Err("missing-provider negative control collided with a file".to_string());
        }
        let failure =
            match run_provider_change_status(&executable, Path::new("."), Duration::from_secs(5)) {
                Err(failure) => failure,
                Ok(output) => return Err(format!("missing provider unexpectedly ran: {output:?}")),
            };
        if failure.class != IntentDelegateFailureClass::InstrumentFailure
            || failure.code != IntentDelegateFailureCode::ProviderInstrumentFailure
        {
            return Err(format!(
                "spawn failure lost its typed instrument classification: {failure}"
            ));
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn immutable_native_root_probe_survives_child_held_writable_executable() -> Result<(), String> {
        use std::os::unix::fs::PermissionsExt;
        use std::time::{SystemTime, UNIX_EPOCH};

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| err.to_string())?
            .as_nanos();
        let fixture_root = std::env::temp_dir().join(format!(
            "cargo-allow-intent-writable-probe-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&fixture_root).map_err(|err| err.to_string())?;
        let result = (|| {
            let script = fixture_root.join("cargo-intent");
            let mut writer = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&script)
                .map_err(|err| err.to_string())?;
            writer
                .write_all(b"#!/bin/sh\nprintf '{}'\n")
                .map_err(|err| err.to_string())?;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
                .map_err(|err| err.to_string())?;
            // The child owns the writable descriptor as stdout. Closing stderr
            // acknowledges admission; the stdin barrier holds that descriptor
            // until parent cleanup. The Command temporary drops the parent copy.
            let mut holder = Command::new("/bin/sh")
                .args(["-c", "printf ready >&2; exec 2>&-; IFS= read -r release"])
                .stdin(Stdio::piped())
                .stdout(Stdio::from(writer))
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|err| format!("spawn descriptor holder: {err}"))?;
            let result = (|| {
                let stderr = holder
                    .stderr
                    .take()
                    .ok_or_else(|| "descriptor holder had no barrier pipe".to_string())?;
                let barrier = receive_reader(
                    spawn_bounded_reader(stderr, 16, "descriptor-holder barrier"),
                    Instant::now() + Duration::from_secs(5),
                )
                .map_err(|err| err.to_string())?;
                if barrier.bytes != b"ready"
                    || barrier.exceeded
                    || holder.try_wait().map_err(|err| err.to_string())?.is_some()
                {
                    return Err("descriptor holder did not reach the live barrier".to_string());
                }
                let root = native_probe_root();
                let failure =
                    match run_provider_change_status(&script, &root, Duration::from_secs(5)) {
                        Err(failure) => failure,
                        Ok(output) => {
                            return Err(format!(
                                "writable executable unexpectedly ran: {output:?}"
                            ));
                        }
                    };
                if failure.class != IntentDelegateFailureClass::InstrumentFailure
                    || failure.code != IntentDelegateFailureCode::ProviderInstrumentFailure
                    || !failure.detail.contains("os error 26")
                {
                    return Err(format!("expected typed Linux ETXTBSY, got {failure}"));
                }
                assert_native_root_probe(&root)
            })();
            let cleanup = terminate_and_reap(&mut holder);
            if cleanup.contains("error:") {
                return Err(format!(
                    "descriptor-holder cleanup failed: {cleanup}; probe={result:?}"
                ));
            }
            result
        })();
        let cleanup = std::fs::remove_dir_all(&fixture_root).map_err(|err| err.to_string());
        result?;
        cleanup?;
        Ok(())
    }

    #[test]
    fn run_with_timeout_drains_large_stdout_and_stderr_concurrently() -> Result<(), String> {
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

    #[cfg(unix)]
    #[test]
    fn run_with_timeout_bounds_inherited_pipe_settlement() -> Result<(), String> {
        let mut command = helper_command("inherited-pipe")?;
        let started = Instant::now();
        let failure = match run_with_timeout(&mut command, Duration::from_secs(10)) {
            Err(failure) => failure,
            Ok(output) => {
                return Err(format!(
                    "provider with inherited descendant pipe unexpectedly settled: {output:?}"
                ));
            }
        };
        if failure.code != IntentDelegateFailureCode::ReaderSettleTimeout {
            return Err(format!("expected ReaderSettleTimeout, got {failure}"));
        }
        if started.elapsed() > Duration::from_secs(3) {
            return Err(format!(
                "reader settlement exceeded bounded allowance: {:?}",
                started.elapsed()
            ));
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
        #[cfg(unix)]
        if mode == "inherited-pipe" {
            Command::new("sh")
                .args(["-c", "sleep 2"])
                .spawn()
                .map_err(|err| format!("spawn inherited-pipe descendant: {err}"))?;
            return Ok(());
        }
        if mode == "large" {
            let stdout_bytes = PROVIDER_STDOUT_LIMIT + (128 * 1024);
            let stderr_bytes = PROVIDER_STDERR_LIMIT + (128 * 1024);
            let stdout_writer = std::thread::spawn(move || {
                write_repeated(std::io::stdout().lock(), b'x', stdout_bytes)
            });
            let stderr_writer = std::thread::spawn(move || {
                write_repeated(std::io::stderr().lock(), b'y', stderr_bytes)
            });
            join_writer(stdout_writer, "stdout")?;
            join_writer(stderr_writer, "stderr")?;
            std::process::exit(0);
        }
        Err(format!("unsupported provider helper mode {mode}"))
    }

    fn join_writer(writer: JoinHandle<Result<(), String>>, stream: &str) -> Result<(), String> {
        writer
            .join()
            .map_err(|_| format!("{stream} helper writer panicked"))?
    }

    fn write_repeated(
        mut writer: impl Write,
        byte: u8,
        mut remaining: usize,
    ) -> Result<(), String> {
        let chunk = [byte; PROVIDER_READ_CHUNK];
        while remaining > 0 {
            let write = remaining.min(chunk.len());
            let bytes = chunk
                .get(..write)
                .ok_or_else(|| "write chunk exceeded provider buffer".to_string())?;
            writer.write_all(bytes).map_err(|err| err.to_string())?;
            remaining -= write;
        }
        writer.flush().map_err(|err| err.to_string())
    }
}
