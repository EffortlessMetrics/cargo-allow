//! Explicit execution gate and bounded provider-neutral runner.

use crate::CommandInvocationSpecV1;
use proof_protocol::{ProofPlanV1, validate_proof_plan};
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

pub const EXECUTION_GATE_SCHEMA_ID: &str = "proof.execution-gate.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionApprovalV1 {
    Denied,
    Explicit,
}

impl ExecutionApprovalV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Denied => "denied",
            Self::Explicit => "explicit",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionGateReportV1 {
    pub schema_id: String,
    pub plan_id: String,
    pub approval: ExecutionApprovalV1,
    pub would_execute: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionError {
    ProofPlan(String),
    ApprovalRequired,
}

impl ExecutionError {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::ProofPlan(_) => "proof_plan_invalid",
            Self::ApprovalRequired => "approval_required",
        }
    }
}

pub fn evaluate_execution_gate(
    plan: &ProofPlanV1,
    approval: ExecutionApprovalV1,
) -> Result<ExecutionGateReportV1, ExecutionError> {
    validate_proof_plan(plan).map_err(|err| ExecutionError::ProofPlan(err.as_str().to_string()))?;
    let would_execute = approval == ExecutionApprovalV1::Explicit;
    Ok(ExecutionGateReportV1 {
        schema_id: EXECUTION_GATE_SCHEMA_ID.to_string(),
        plan_id: plan.plan_id.clone(),
        approval,
        would_execute,
    })
}

pub fn require_explicit_execution(approval: ExecutionApprovalV1) -> Result<(), ExecutionError> {
    if approval == ExecutionApprovalV1::Explicit {
        Ok(())
    } else {
        Err(ExecutionError::ApprovalRequired)
    }
}

pub const EXECUTION_RECEIPT_SCHEMA_ID: &str = "proof.execution-receipt.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessObservationStatusV1 {
    Completed,
    NonzeroExit,
    TimedOut,
    SpawnFailed,
    OutputLimitExceeded,
    InstrumentFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionSpecV1 {
    pub reviewed_invocation: CommandInvocationSpecV1,
    pub reviewed_plan_id: String,
    pub plan_id: String,
    pub command_id: String,
    pub program: String,
    pub argv: Vec<String>,
    pub cwd: PathBuf,
    pub read_roots: Vec<PathBuf>,
    pub write_roots: Vec<PathBuf>,
    pub env_allowlist: Vec<(String, String)>,
    pub timeout: Duration,
    pub stdout_limit: usize,
    pub stderr_limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionReceiptV1 {
    pub schema_id: String,
    pub plan_id: String,
    pub command_id: String,
    pub program: String,
    pub argv: Vec<String>,
    pub status: ProcessObservationStatusV1,
    pub exit_code: Option<i32>,
    pub stdout_len: usize,
    pub stderr_len: usize,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub stdout_digest: String,
    pub stderr_digest: String,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerError {
    InvalidSpec(String),
    ApprovalRequired,
    Spawn(String),
    Io(String),
}

impl RunnerError {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InvalidSpec(_) => "malformed_execution_spec",
            Self::ApprovalRequired => "approval_required",
            Self::Spawn(_) => "spawn_failed",
            Self::Io(_) => "instrument_failure",
        }
    }
}

pub fn execute_bounded(
    spec: &ExecutionSpecV1,
    approval: ExecutionApprovalV1,
) -> Result<ExecutionReceiptV1, RunnerError> {
    require_explicit_execution(approval).map_err(|_| RunnerError::ApprovalRequired)?;
    validate_execution_spec(spec)?;
    let deadline = Instant::now().checked_add(spec.timeout).ok_or_else(|| {
        RunnerError::InvalidSpec("timeout exceeds the platform clock range".to_string())
    })?;
    let canonical_cwd = std::fs::canonicalize(&spec.cwd).map_err(|error| {
        RunnerError::InvalidSpec(format!("cwd cannot be canonicalized: {error}"))
    })?;
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.argv)
        .current_dir(&canonical_cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear();
    for (key, value) in &spec.env_allowlist {
        command.env(key, value);
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return Ok(ExecutionReceiptV1 {
                schema_id: EXECUTION_RECEIPT_SCHEMA_ID.to_string(),
                plan_id: spec.plan_id.clone(),
                command_id: spec.command_id.clone(),
                program: spec.program.clone(),
                argv: spec.argv.clone(),
                status: ProcessObservationStatusV1::SpawnFailed,
                exit_code: None,
                stdout_len: 0,
                stderr_len: 0,
                stdout_truncated: false,
                stderr_truncated: false,
                stdout_digest: digest(&[]),
                stderr_digest: digest(&[]),
                limitations: vec![format!("process spawn failed: {error}")],
            });
        }
    };
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| RunnerError::Io("stdout pipe unavailable".to_string()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| RunnerError::Io("stderr pipe unavailable".to_string()))?;
    let (stdout_tx, stdout_rx) = mpsc::channel();
    let (stderr_tx, stderr_rx) = mpsc::channel();
    let stdout_limit = spec.stdout_limit;
    let stderr_limit = spec.stderr_limit;
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout
            .take((stdout_limit.saturating_add(1)) as u64)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
            .map_err(|error| error.to_string());
        let _ = stdout_tx.send(result);
    });
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stderr
            .take((stderr_limit.saturating_add(1)) as u64)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
            .map_err(|error| error.to_string());
        let _ = stderr_tx.send(result);
    });
    let mut timed_out = false;
    let mut stdout_limit_exceeded = false;
    let mut stderr_limit_exceeded = false;
    let mut stdout_result = None;
    let mut stderr_result = None;
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| RunnerError::Io(error.to_string()))?
        {
            break status;
        }
        if Instant::now() >= deadline {
            timed_out = true;
            if let Some(error) = termination_error(&mut child)? {
                return Ok(instrument_failure_receipt(
                    spec,
                    format!("timed-out process could not be terminated: {error}"),
                ));
            }
            break child
                .wait()
                .map_err(|error| RunnerError::Io(error.to_string()))?;
        }
        if stdout_result.is_none() {
            stdout_result = stdout_rx.try_recv().ok();
        }
        if stderr_result.is_none() {
            stderr_result = stderr_rx.try_recv().ok();
        }
        let stdout_over = stdout_result
            .as_ref()
            .and_then(|result| result.as_ref().ok())
            .is_some_and(|bytes| bytes.len() > spec.stdout_limit);
        let stderr_over = stderr_result
            .as_ref()
            .and_then(|result| result.as_ref().ok())
            .is_some_and(|bytes| bytes.len() > spec.stderr_limit);
        if stdout_over || stderr_over {
            stdout_limit_exceeded |= stdout_over;
            stderr_limit_exceeded |= stderr_over;
            if let Some(error) = termination_error(&mut child)? {
                return Ok(instrument_failure_receipt(
                    spec,
                    format!("output-limited process could not be terminated: {error}"),
                ));
            }
            break child
                .wait()
                .map_err(|error| RunnerError::Io(error.to_string()))?;
        }
        thread::sleep(Duration::from_millis(5));
    };
    if stdout_result.is_none() {
        stdout_result = stdout_rx.try_recv().ok();
    }
    if stderr_result.is_none() {
        stderr_result = stderr_rx.try_recv().ok();
    }
    let mut capture_incomplete = false;
    let mut stdout_capture_missing = false;
    let mut stderr_capture_missing = false;
    let output_limit_exceeded = stdout_limit_exceeded || stderr_limit_exceeded;
    let stdout = if timed_out || output_limit_exceeded {
        match stdout_result.and_then(Result::ok) {
            Some(bytes) => bytes,
            None => {
                capture_incomplete = true;
                stdout_capture_missing = true;
                Vec::new()
            }
        }
    } else {
        match stdout_result.or_else(|| stdout_rx.recv_timeout(Duration::from_millis(50)).ok()) {
            Some(Ok(bytes)) => bytes,
            Some(Err(_)) | None => {
                capture_incomplete = true;
                Vec::new()
            }
        }
    };
    let stderr = if timed_out || output_limit_exceeded {
        match stderr_result.and_then(Result::ok) {
            Some(bytes) => bytes,
            None => {
                capture_incomplete = true;
                stderr_capture_missing = true;
                Vec::new()
            }
        }
    } else {
        match stderr_result.or_else(|| stderr_rx.recv_timeout(Duration::from_millis(50)).ok()) {
            Some(Ok(bytes)) => bytes,
            Some(Err(_)) | None => {
                capture_incomplete = true;
                Vec::new()
            }
        }
    };
    let stdout_truncated = stdout_limit_exceeded
        || timed_out
        || stdout_capture_missing
        || stdout.len() > spec.stdout_limit;
    let stderr_truncated = stderr_limit_exceeded
        || timed_out
        || stderr_capture_missing
        || stderr.len() > spec.stderr_limit;
    let observation = if timed_out {
        ProcessObservationStatusV1::TimedOut
    } else if output_limit_exceeded || stdout_truncated || stderr_truncated {
        ProcessObservationStatusV1::OutputLimitExceeded
    } else if capture_incomplete {
        ProcessObservationStatusV1::InstrumentFailure
    } else if status.success() {
        ProcessObservationStatusV1::Completed
    } else {
        ProcessObservationStatusV1::NonzeroExit
    };
    let mut limitations = vec![
        "process-tree termination is limited to the spawned process on this platform".to_string(),
        "timed-out output capture is explicitly incomplete".to_string(),
    ];
    if capture_incomplete {
        limitations
            .push("output capture did not settle before the bounded grace period".to_string());
    }
    Ok(ExecutionReceiptV1 {
        schema_id: EXECUTION_RECEIPT_SCHEMA_ID.to_string(),
        plan_id: spec.plan_id.clone(),
        command_id: spec.command_id.clone(),
        program: spec.program.clone(),
        argv: spec.argv.clone(),
        status: observation,
        exit_code: status.code(),
        stdout_len: stdout.len(),
        stderr_len: stderr.len(),
        stdout_truncated,
        stderr_truncated,
        stdout_digest: digest(&stdout),
        stderr_digest: digest(&stderr),
        limitations,
    })
}

fn termination_error(child: &mut std::process::Child) -> Result<Option<String>, RunnerError> {
    match child.kill() {
        Ok(()) => Ok(None),
        Err(error) => {
            let still_running = child
                .try_wait()
                .map_err(|wait_error| RunnerError::Io(wait_error.to_string()))?
                .is_none();
            if still_running {
                Ok(Some(error.to_string()))
            } else {
                Ok(None)
            }
        }
    }
}

fn instrument_failure_receipt(spec: &ExecutionSpecV1, reason: String) -> ExecutionReceiptV1 {
    ExecutionReceiptV1 {
        schema_id: EXECUTION_RECEIPT_SCHEMA_ID.to_string(),
        plan_id: spec.plan_id.clone(),
        command_id: spec.command_id.clone(),
        program: spec.program.clone(),
        argv: spec.argv.clone(),
        status: ProcessObservationStatusV1::InstrumentFailure,
        exit_code: None,
        stdout_len: 0,
        stderr_len: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        stdout_digest: digest(&[]),
        stderr_digest: digest(&[]),
        limitations: vec![reason],
    }
}

fn validate_execution_spec(spec: &ExecutionSpecV1) -> Result<(), RunnerError> {
    if spec.plan_id.trim().is_empty()
        || spec.command_id.trim().is_empty()
        || spec.program.trim().is_empty()
        || spec.cwd.as_os_str().is_empty()
    {
        return Err(RunnerError::InvalidSpec(
            "plan, command, program, and cwd are required".to_string(),
        ));
    }
    if !Path::new(&spec.program).is_absolute() {
        return Err(RunnerError::InvalidSpec(
            "program must be an absolute reviewed executable path".to_string(),
        ));
    }
    if !spec.write_roots.is_empty() {
        return Err(RunnerError::InvalidSpec(
            "the initial runner accepts read-only execution only".to_string(),
        ));
    }
    let has_unsafe_component = |path: &Path| {
        path.components().any(|component| {
            matches!(
                component,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
    };
    if !Path::new(&spec.cwd).is_absolute()
        || has_unsafe_component(&spec.cwd)
        || spec.read_roots.is_empty()
        || spec
            .read_roots
            .iter()
            .any(|root| !root.is_absolute() || has_unsafe_component(root))
        || !spec
            .read_roots
            .iter()
            .any(|root| spec.cwd.starts_with(root))
    {
        return Err(RunnerError::InvalidSpec(
            "cwd must be contained by an explicit read root".to_string(),
        ));
    }
    let canonical_cwd = std::fs::canonicalize(&spec.cwd).map_err(|error| {
        RunnerError::InvalidSpec(format!("cwd cannot be canonicalized: {error}"))
    })?;
    let contained = spec.read_roots.iter().any(|root| {
        std::fs::canonicalize(root)
            .map(|canonical_root| canonical_cwd.starts_with(canonical_root))
            .unwrap_or(false)
    });
    if !contained {
        return Err(RunnerError::InvalidSpec(
            "cwd must be contained by an explicit read root".to_string(),
        ));
    }
    if spec.reviewed_invocation.command_id != spec.command_id
        || spec.reviewed_plan_id != spec.plan_id
        || spec.reviewed_invocation.program != spec.program
        || spec.reviewed_invocation.argv != spec.argv
        || spec.reviewed_invocation.schema_id != crate::COMMAND_INVOCATION_SPEC_SCHEMA_ID
    {
        return Err(RunnerError::InvalidSpec(
            "execution spec must match its reviewed invocation".to_string(),
        ));
    }
    let env_keys: Vec<&str> = spec
        .env_allowlist
        .iter()
        .map(|(key, _)| key.as_str())
        .collect();
    if env_keys
        != spec
            .reviewed_invocation
            .env_allowlist
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
        || spec.timeout > Duration::from_millis(spec.reviewed_invocation.timeout_ms)
        || !spec.reviewed_invocation.write_paths.is_empty()
        || spec.reviewed_invocation.network != crate::NetworkAccessV1::None
        || spec.reviewed_invocation.cancellation != crate::CancellationPostureV1::Cooperative
    {
        return Err(RunnerError::InvalidSpec(
            "execution spec exceeds reviewed invocation policy".to_string(),
        ));
    }
    if spec.timeout.is_zero() || spec.stdout_limit == 0 || spec.stderr_limit == 0 {
        return Err(RunnerError::InvalidSpec(
            "timeout and output limits must be positive".to_string(),
        ));
    }
    let program = Path::new(&spec.program)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if program.ends_with("sh")
        || matches!(
            program.as_str(),
            "sh" | "bash"
                | "zsh"
                | "fish"
                | "dash"
                | "ash"
                | "ksh"
                | "csh"
                | "tcsh"
                | "nu"
                | "nushell"
                | "cmd"
                | "cmd.exe"
                | "powershell"
                | "powershell.exe"
                | "pwsh"
                | "pwsh.exe"
        )
    {
        return Err(RunnerError::InvalidSpec(
            "shell programs are not accepted by the structured runner".to_string(),
        ));
    }
    if spec.argv.iter().any(|arg| {
        matches!(
            arg.to_ascii_lowercase().as_str(),
            "-c" | "/c" | "-command" | "/command" | "-commandandexit"
        )
    }) {
        return Err(RunnerError::InvalidSpec(
            "shell command arguments are not accepted by the structured runner".to_string(),
        ));
    }
    Ok(())
}

fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let encoded: String = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("sha256:v1:{encoded}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        COMMAND_INVOCATION_SPEC_SCHEMA_ID, CancellationPostureV1, CommandSourceKindV1, CwdPolicyV1,
        NetworkAccessV1,
    };

    fn spec() -> Result<ExecutionSpecV1, String> {
        let program = std::env::current_exe()
            .map_err(|error| error.to_string())?
            .display()
            .to_string();
        let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
        let reviewed_invocation = CommandInvocationSpecV1 {
            schema_id: COMMAND_INVOCATION_SPEC_SCHEMA_ID.to_string(),
            command_id: "test".to_string(),
            source_kind: CommandSourceKindV1::ReviewedRegistry,
            program: program.clone(),
            argv: vec!["--list".to_string()],
            cwd_policy: CwdPolicyV1::RepositoryRoot,
            env_allowlist: vec![],
            read_paths: vec![],
            write_paths: vec![],
            network: NetworkAccessV1::None,
            timeout_ms: 5_000,
            cancellation: CancellationPostureV1::Cooperative,
        };
        Ok(ExecutionSpecV1 {
            reviewed_invocation,
            reviewed_plan_id: "plan-1".to_string(),
            plan_id: "plan-1".to_string(),
            command_id: "test".to_string(),
            program,
            argv: vec!["--list".to_string()],
            cwd: cwd.clone(),
            read_roots: vec![cwd.clone()],
            write_roots: vec![],
            env_allowlist: vec![],
            timeout: Duration::from_secs(5),
            stdout_limit: 64 * 1024,
            stderr_limit: 64 * 1024,
        })
    }

    #[test]
    fn bounded_runner_observes_a_completed_process() -> Result<(), String> {
        let receipt = execute_bounded(&spec()?, ExecutionApprovalV1::Explicit)
            .map_err(|error| error.as_str().to_string())?;
        if receipt.status != ProcessObservationStatusV1::Completed {
            return Err(format!(
                "expected completed process, got {:?}",
                receipt.status
            ));
        }
        if receipt.stdout_digest.is_empty() || receipt.schema_id != EXECUTION_RECEIPT_SCHEMA_ID {
            return Err("receipt must retain schema and output digest".to_string());
        }
        Ok(())
    }

    #[test]
    fn shell_programs_are_rejected_before_spawn() -> Result<(), String> {
        let mut candidate = spec()?;
        candidate.program = if cfg!(windows) {
            std::path::PathBuf::from(r"C:\Windows\System32\cmd.exe")
        } else {
            std::path::PathBuf::from("/bin/sh")
        }
        .display()
        .to_string();
        candidate.reviewed_invocation.program = candidate.program.clone();
        let error = match execute_bounded(&candidate, ExecutionApprovalV1::Explicit) {
            Ok(_) => return Err("shell program unexpectedly spawned".to_string()),
            Err(error) => error,
        };
        if !matches!(&error, RunnerError::InvalidSpec(message) if message.contains("shell programs"))
        {
            return Err(format!("unexpected runner error: {}", error.as_str()));
        }
        Ok(())
    }

    #[test]
    fn shell_aliases_and_command_flags_are_rejected() -> Result<(), String> {
        let mut candidate = spec()?;
        candidate.program = if cfg!(windows) {
            r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe".to_string()
        } else {
            "/bin/dash".to_string()
        };
        candidate.argv = vec!["-c".to_string(), "echo unsafe".to_string()];
        candidate.reviewed_invocation.program = candidate.program.clone();
        candidate.reviewed_invocation.argv = candidate.argv.clone();
        let error = match execute_bounded(&candidate, ExecutionApprovalV1::Explicit) {
            Ok(_) => return Err("shell alias unexpectedly accepted".to_string()),
            Err(error) => error,
        };
        if !matches!(&error, RunnerError::InvalidSpec(message) if message.contains("shell")) {
            return Err(format!("unexpected runner error: {}", error.as_str()));
        }
        Ok(())
    }

    #[test]
    fn traversal_paths_are_rejected_before_spawn() -> Result<(), String> {
        let mut candidate = spec()?;
        candidate.cwd = candidate.cwd.join("..");
        let error = match execute_bounded(&candidate, ExecutionApprovalV1::Explicit) {
            Ok(_) => return Err("traversal cwd unexpectedly accepted".to_string()),
            Err(error) => error,
        };
        if !matches!(&error, RunnerError::InvalidSpec(message) if message.contains("contained")) {
            return Err(format!("unexpected runner error: {}", error.as_str()));
        }
        Ok(())
    }

    #[test]
    fn instrument_failure_receipt_is_typed_and_bounded() -> Result<(), String> {
        let candidate = spec()?;
        let receipt = instrument_failure_receipt(&candidate, "capture incomplete".to_string());
        if receipt.status != ProcessObservationStatusV1::InstrumentFailure
            || receipt.limitations != ["capture incomplete"]
            || receipt.plan_id != candidate.plan_id
        {
            return Err("instrument failure receipt lost its bounded identity".to_string());
        }
        Ok(())
    }

    #[test]
    fn timeout_and_output_limits_are_observed_without_shells() -> Result<(), String> {
        let mut timeout = spec()?;
        timeout.timeout = Duration::from_nanos(1);
        let timed = execute_bounded(&timeout, ExecutionApprovalV1::Explicit)
            .map_err(|error| error.as_str().to_string())?;
        if !matches!(
            timed.status,
            ProcessObservationStatusV1::TimedOut | ProcessObservationStatusV1::Completed
        ) {
            return Err(format!("expected timeout, got {:?}", timed.status));
        }

        let mut limited = spec()?;
        limited.stdout_limit = 1;
        let observed = execute_bounded(&limited, ExecutionApprovalV1::Explicit)
            .map_err(|error| error.as_str().to_string())?;
        if observed.status != ProcessObservationStatusV1::OutputLimitExceeded {
            return Err(format!("expected output limit, got {:?}", observed.status));
        }
        Ok(())
    }

    #[test]
    fn spawn_failure_is_a_typed_observation() -> Result<(), String> {
        let mut candidate = spec()?;
        candidate.program = if cfg!(windows) {
            "C:\\cargo-proof-missing-executable.exe"
        } else {
            "/definitely-missing/cargo-proof-executable"
        }
        .to_string();
        candidate.reviewed_invocation.program = candidate.program.clone();
        let receipt = execute_bounded(&candidate, ExecutionApprovalV1::Explicit)
            .map_err(|error| error.as_str().to_string())?;
        if receipt.status != ProcessObservationStatusV1::SpawnFailed {
            return Err(format!("expected spawn failure, got {:?}", receipt.status));
        }
        Ok(())
    }

    #[test]
    fn execution_spec_round_trips_as_strict_json() -> Result<(), String> {
        let original = spec()?;
        let encoded = serde_json::to_string(&original).map_err(|error| error.to_string())?;
        let decoded: ExecutionSpecV1 =
            serde_json::from_str(&encoded).map_err(|error| error.to_string())?;
        if decoded != original {
            return Err("execution spec JSON round-trip changed the reviewed request".to_string());
        }
        let mut unknown = serde_json::to_value(&original)
            .map_err(|error| error.to_string())?
            .as_object()
            .cloned()
            .ok_or_else(|| "execution spec did not serialize as an object".to_string())?;
        unknown.insert("unexpected".to_string(), serde_json::Value::Bool(true));
        let strict_error =
            serde_json::from_value::<ExecutionSpecV1>(serde_json::Value::Object(unknown));
        if strict_error.is_ok() {
            return Err("execution spec accepted an unknown field".to_string());
        }
        Ok(())
    }
}
