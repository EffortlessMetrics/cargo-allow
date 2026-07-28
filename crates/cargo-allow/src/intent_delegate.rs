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
use std::io::{self, Read};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub const INTENT_PROVIDER_ID: &str = "cargo-intent";
pub const CHANGE_STATUS_PAYLOAD_SCHEMA: &str = "cargo-intent.change-status.v1";
const MAX_PROVIDER_STDOUT_BYTES: usize = 1024 * 1024;
const MAX_PROVIDER_STDERR_BYTES: usize = 64 * 1024;
const MAX_PROVIDER_ERROR_DIAGNOSTIC_CHARS: usize = 4096;
const PROVIDER_READ_CHUNK_BYTES: usize = 8192;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntentDelegateFailureClass {
    ProviderAbsent,
    WrongProduct,
    WrongProtocol,
    MalformedOutput,
    ProviderOutputTooLarge,
    ProviderDiagnosticTooLarge,
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
) -> Result<BoundedProviderOutput, IntentDelegateFailure> {
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
        .arg("--analysis-receipt")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    run_with_timeout(&mut command, timeout)
}

fn run_with_timeout(
    command: &mut Command,
    timeout: Duration,
) -> Result<BoundedProviderOutput, IntentDelegateFailure> {
    let mut child = command.spawn().map_err(|err| {
        IntentDelegateFailure::new(
            IntentDelegateFailureClass::InstrumentFailure,
            format!("failed to spawn cargo-intent provider: {err}"),
        )
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        terminate_child(&mut child);
        IntentDelegateFailure::new(
            IntentDelegateFailureClass::InstrumentFailure,
            "cargo-intent provider stdout pipe was not available",
        )
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        terminate_child(&mut child);
        IntentDelegateFailure::new(
            IntentDelegateFailureClass::InstrumentFailure,
            "cargo-intent provider stderr pipe was not available",
        )
    })?;
    let stdout_reader = spawn_bounded_reader(stdout, MAX_PROVIDER_STDOUT_BYTES);
    let stderr_reader = spawn_bounded_reader(stderr, MAX_PROVIDER_STDERR_BYTES);
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() >= timeout => {
                terminate_child(&mut child);
                let _ = join_bounded_reader(stdout_reader, "stdout");
                let _ = join_bounded_reader(stderr_reader, "stderr");
                return Err(IntentDelegateFailure::new(
                    IntentDelegateFailureClass::Timeout,
                    format!(
                        "cargo-intent provider exceeded {}s timeout",
                        timeout.as_secs()
                    ),
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(25)),
            Err(error) => {
                terminate_child(&mut child);
                let _ = join_bounded_reader(stdout_reader, "stdout");
                let _ = join_bounded_reader(stderr_reader, "stderr");
                return Err(IntentDelegateFailure::new(
                    IntentDelegateFailureClass::InstrumentFailure,
                    format!("failed waiting for cargo-intent provider: {error}"),
                ));
            }
        }
    };
    let stdout = join_bounded_reader(stdout_reader, "stdout")?;
    let stderr = join_bounded_reader(stderr_reader, "stderr")?;
    if stdout.exceeded_limit {
        return Err(IntentDelegateFailure::new(
            IntentDelegateFailureClass::ProviderOutputTooLarge,
            format!(
                "cargo-intent provider stdout exceeded the {} byte limit",
                MAX_PROVIDER_STDOUT_BYTES
            ),
        ));
    }
    if stderr.exceeded_limit {
        return Err(IntentDelegateFailure::new(
            IntentDelegateFailureClass::ProviderDiagnosticTooLarge,
            format!(
                "cargo-intent provider stderr exceeded the {} byte limit",
                MAX_PROVIDER_STDERR_BYTES
            ),
        ));
    }
    let stdout_exceeded_limit = stdout.exceeded_limit;
    let stderr_exceeded_limit = stderr.exceeded_limit;
    Ok(BoundedProviderOutput {
        status,
        stdout: stdout.bytes,
        stderr: stderr.bytes,
        stdout_exceeded_limit,
        stderr_exceeded_limit,
    })
}

#[derive(Debug)]
struct BoundedProviderOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    stdout_exceeded_limit: bool,
    stderr_exceeded_limit: bool,
}

#[derive(Debug)]
struct BoundedStream {
    bytes: Vec<u8>,
    exceeded_limit: bool,
}

fn spawn_bounded_reader<R>(reader: R, limit: usize) -> JoinHandle<io::Result<BoundedStream>>
where
    R: Read + Send + 'static,
{
    std::thread::spawn(move || read_bounded(reader, limit))
}

fn read_bounded<R>(mut reader: R, limit: usize) -> io::Result<BoundedStream>
where
    R: Read,
{
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; PROVIDER_READ_CHUNK_BYTES];
    let mut exceeded_limit = false;
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let remaining = limit.saturating_sub(bytes.len());
        let retained = read.min(remaining);
        let retained_bytes = buffer.get(..retained).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "bounded provider read exceeded its buffer",
            )
        })?;
        bytes.extend_from_slice(retained_bytes);
        if retained < read {
            exceeded_limit = true;
        }
    }
    Ok(BoundedStream {
        bytes,
        exceeded_limit,
    })
}

fn join_bounded_reader(
    reader: JoinHandle<io::Result<BoundedStream>>,
    stream: &str,
) -> Result<BoundedStream, IntentDelegateFailure> {
    reader
        .join()
        .map_err(|_| {
            IntentDelegateFailure::new(
                IntentDelegateFailureClass::InstrumentFailure,
                format!("cargo-intent provider {stream} reader panicked"),
            )
        })?
        .map_err(|error| {
            IntentDelegateFailure::new(
                IntentDelegateFailureClass::InstrumentFailure,
                format!("failed reading cargo-intent provider {stream}: {error}"),
            )
        })
}

fn terminate_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn validate_provider_output(
    output: &BoundedProviderOutput,
) -> Result<AnalysisReceiptEnvelopeV1, IntentDelegateFailure> {
    if output.stdout_exceeded_limit {
        return Err(IntentDelegateFailure::new(
            IntentDelegateFailureClass::ProviderOutputTooLarge,
            format!(
                "cargo-intent provider stdout exceeded the {} byte limit",
                MAX_PROVIDER_STDOUT_BYTES
            ),
        ));
    }
    if output.stderr_exceeded_limit {
        return Err(IntentDelegateFailure::new(
            IntentDelegateFailureClass::ProviderDiagnosticTooLarge,
            format!(
                "cargo-intent provider stderr exceeded the {} byte limit",
                MAX_PROVIDER_STDERR_BYTES
            ),
        ));
    }
    if output.stdout.is_empty() {
        return Err(IntentDelegateFailure::new(
            IntentDelegateFailureClass::MalformedOutput,
            format!(
                "cargo-intent provider returned empty stdout; stderr={}",
                bounded_diagnostic(&output.stderr)
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

fn bounded_diagnostic(stderr: &[u8]) -> String {
    String::from_utf8_lossy(stderr)
        .chars()
        .take(MAX_PROVIDER_ERROR_DIAGNOSTIC_CHARS)
        .collect()
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
        | IntentDelegateFailureClass::MalformedOutput
        | IntentDelegateFailureClass::ProviderOutputTooLarge
        | IntentDelegateFailureClass::ProviderDiagnosticTooLarge => {
            CargoAllowErrorKind::InvalidConfig
        }
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
    use std::io::Write;

    fn bounded_output(stdout: Vec<u8>, stderr: Vec<u8>) -> Result<BoundedProviderOutput, String> {
        let status = if cfg!(windows) {
            std::process::Command::new("cmd")
                .args(["/C", "exit", "0"])
                .status()
                .map_err(|error| error.to_string())?
        } else {
            std::process::Command::new("true")
                .status()
                .map_err(|error| error.to_string())?
        };
        Ok(BoundedProviderOutput {
            status,
            stdout,
            stderr,
            stdout_exceeded_limit: false,
            stderr_exceeded_limit: false,
        })
    }

    fn provider_test_command(mode: &str) -> Result<Command, String> {
        let executable = std::env::current_exe().map_err(|error| error.to_string())?;
        let mut command = Command::new(executable);
        command
            .args([
                "--exact",
                "intent_delegate::tests::provider_test_child",
                "--nocapture",
            ])
            .env("CARGO_ALLOW_PROVIDER_TEST_CHILD", mode)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        Ok(command)
    }

    #[test]
    fn provider_test_child() -> Result<(), String> {
        let Some(mode) = std::env::var_os("CARGO_ALLOW_PROVIDER_TEST_CHILD") else {
            return Ok(());
        };
        let mode = mode.to_string_lossy();
        match mode.as_ref() {
            "large-both" => {
                let chunk = [b'x'; PROVIDER_READ_CHUNK_BYTES];
                let mut stdout = io::stdout().lock();
                let mut stderr = io::stderr().lock();
                for _ in 0..((MAX_PROVIDER_STDOUT_BYTES / PROVIDER_READ_CHUNK_BYTES) + 2) {
                    stdout
                        .write_all(&chunk)
                        .map_err(|error| error.to_string())?;
                    stderr
                        .write_all(&chunk)
                        .map_err(|error| error.to_string())?;
                }
                stdout.flush().map_err(|error| error.to_string())?;
                stderr.flush().map_err(|error| error.to_string())?;
            }
            "large-stderr" => {
                let chunk = [b'x'; PROVIDER_READ_CHUNK_BYTES];
                let mut stderr = io::stderr().lock();
                for _ in 0..((MAX_PROVIDER_STDERR_BYTES / PROVIDER_READ_CHUNK_BYTES) + 2) {
                    stderr
                        .write_all(&chunk)
                        .map_err(|error| error.to_string())?;
                }
                stderr.flush().map_err(|error| error.to_string())?;
            }
            "timeout" => loop {
                std::thread::sleep(Duration::from_millis(10));
            },
            other => return Err(format!("unknown provider test mode: {other}")),
        }
        Ok(())
    }

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
        let output = bounded_output(Vec::new(), b"provider error".to_vec())?;
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
    fn bounded_reader_retains_only_the_configured_limit() -> Result<(), String> {
        let stream = read_bounded(b"abcdefghij".as_ref(), 4).map_err(|error| error.to_string())?;
        if stream.bytes != b"abcd" || !stream.exceeded_limit {
            return Err(
                "bounded reader did not retain and classify overflow correctly".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn bounded_runner_drains_both_streams_and_rejects_large_stdout() -> Result<(), String> {
        let mut command = provider_test_command("large-both")?;
        let failure = match run_with_timeout(&mut command, Duration::from_secs(5)) {
            Err(failure) => failure,
            Ok(_) => return Err("large stdout should be rejected".to_string()),
        };
        if failure.class != IntentDelegateFailureClass::ProviderOutputTooLarge {
            return Err(format!("expected stdout overflow, got {:?}", failure.class));
        }
        Ok(())
    }

    #[test]
    fn bounded_runner_rejects_large_stderr_separately() -> Result<(), String> {
        let mut command = provider_test_command("large-stderr")?;
        let failure = match run_with_timeout(&mut command, Duration::from_secs(5)) {
            Err(failure) => failure,
            Ok(_) => return Err("large stderr should be rejected".to_string()),
        };
        if failure.class != IntentDelegateFailureClass::ProviderDiagnosticTooLarge {
            return Err(format!("expected stderr overflow, got {:?}", failure.class));
        }
        Ok(())
    }

    #[test]
    fn bounded_runner_terminates_and_reaps_a_timed_out_provider() -> Result<(), String> {
        let mut command = provider_test_command("timeout")?;
        let failure = match run_with_timeout(&mut command, Duration::from_millis(100)) {
            Err(failure) => failure,
            Ok(_) => return Err("provider should time out".to_string()),
        };
        if failure.class != IntentDelegateFailureClass::Timeout {
            return Err(format!("expected timeout, got {:?}", failure.class));
        }
        Ok(())
    }
}
