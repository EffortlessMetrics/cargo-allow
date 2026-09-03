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
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.argv)
        .current_dir(&spec.cwd)
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
    let mut output_limit_exceeded = false;
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
            if let Err(error) = child.kill() {
                if let Some(status) = child
                    .try_wait()
                    .map_err(|wait_error| RunnerError::Io(wait_error.to_string()))?
                {
                    break status;
                }
                return Err(RunnerError::Io(format!(
                    "timed-out process could not be terminated: {error}"
                )));
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
            output_limit_exceeded = true;
            if let Err(error) = child.kill() {
                if let Some(status) = child
                    .try_wait()
                    .map_err(|wait_error| RunnerError::Io(wait_error.to_string()))?
                {
                    break status;
                }
                return Err(RunnerError::Io(format!(
                    "output-limited process could not be terminated: {error}"
                )));
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
    let stdout = if timed_out || output_limit_exceeded {
        stdout_result.and_then(Result::ok).unwrap_or_default()
    } else {
        stdout_result
            .or_else(|| stdout_rx.recv().ok())
            .ok_or_else(|| RunnerError::Io("stdout capture worker failed".to_string()))?
            .map_err(RunnerError::Io)?
    };
    let stderr = if timed_out || output_limit_exceeded {
        stderr_result.and_then(Result::ok).unwrap_or_default()
    } else {
        stderr_result
            .or_else(|| stderr_rx.recv().ok())
            .ok_or_else(|| RunnerError::Io("stderr capture worker failed".to_string()))?
            .map_err(RunnerError::Io)?
    };
    let stdout_truncated = output_limit_exceeded || timed_out || stdout.len() > spec.stdout_limit;
    let stderr_truncated = output_limit_exceeded || timed_out || stderr.len() > spec.stderr_limit;
    let observation = if timed_out {
        ProcessObservationStatusV1::TimedOut
    } else if output_limit_exceeded || stdout_truncated || stderr_truncated {
        ProcessObservationStatusV1::OutputLimitExceeded
    } else if status.success() {
        ProcessObservationStatusV1::Completed
    } else {
        ProcessObservationStatusV1::NonzeroExit
    };
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
        limitations: vec![
            "process-tree termination is limited to the spawned process on this platform"
                .to_string(),
            "timed-out output capture is explicitly incomplete".to_string(),
        ],
    })
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
    if spec.read_roots.is_empty()
        || !spec
            .read_roots
            .iter()
            .any(|root| spec.cwd.starts_with(root))
    {
        return Err(RunnerError::InvalidSpec(
            "cwd must be contained by an explicit read root".to_string(),
        ));
    }
    if spec.reviewed_invocation.command_id != spec.command_id
        || spec.reviewed_invocation.program != spec.program
        || spec.reviewed_invocation.argv != spec.argv
    {
        return Err(RunnerError::InvalidSpec(
            "execution spec must match its reviewed invocation".to_string(),
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
    if matches!(
        program.as_str(),
        "sh" | "bash"
            | "zsh"
            | "fish"
            | "cmd"
            | "cmd.exe"
            | "powershell"
            | "powershell.exe"
            | "pwsh"
            | "pwsh.exe"
    ) {
        return Err(RunnerError::InvalidSpec(
            "shell programs are not accepted by the structured runner".to_string(),
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
        candidate.program = if cfg!(windows) { "cmd.exe" } else { "/bin/sh" }.to_string();
        let error = match execute_bounded(&candidate, ExecutionApprovalV1::Explicit) {
            Ok(_) => return Err("shell program unexpectedly spawned".to_string()),
            Err(error) => error,
        };
        if error.as_str() != "malformed_execution_spec" {
            return Err(format!("unexpected runner error: {}", error.as_str()));
        }
        Ok(())
    }

    #[test]
    fn timeout_and_output_limits_are_observed_without_shells() -> Result<(), String> {
        let mut timeout = spec()?;
        timeout.timeout = Duration::from_nanos(1);
        let timed = execute_bounded(&timeout, ExecutionApprovalV1::Explicit)
            .map_err(|error| error.as_str().to_string())?;
        if timed.status != ProcessObservationStatusV1::TimedOut {
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
        Ok(())
    }
}
