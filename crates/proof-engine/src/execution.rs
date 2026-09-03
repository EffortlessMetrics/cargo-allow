//! Explicit execution gate and bounded provider-neutral runner.

use proof_protocol::{ProofPlanV1, validate_proof_plan};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionSpecV1 {
    pub plan_id: String,
    pub command_id: String,
    pub program: String,
    pub argv: Vec<String>,
    pub cwd: PathBuf,
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
    Spawn(String),
    Io(String),
}

impl RunnerError {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InvalidSpec(_) => "malformed_execution_spec",
            Self::Spawn(_) => "spawn_failed",
            Self::Io(_) => "instrument_failure",
        }
    }
}

pub fn execute_bounded(spec: &ExecutionSpecV1) -> Result<ExecutionReceiptV1, RunnerError> {
    validate_execution_spec(spec)?;
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
    let mut child = command
        .spawn()
        .map_err(|error| RunnerError::Spawn(error.to_string()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| RunnerError::Io("stdout pipe unavailable".to_string()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| RunnerError::Io("stderr pipe unavailable".to_string()))?;
    let out = Arc::new(Mutex::new((Vec::new(), false)));
    let err = Arc::new(Mutex::new((Vec::new(), false)));
    let out_reader = spawn_reader(stdout, Arc::clone(&out), spec.stdout_limit);
    let err_reader = spawn_reader(stderr, Arc::clone(&err), spec.stderr_limit);
    let deadline = Instant::now() + spec.timeout;
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| RunnerError::Io(error.to_string()))?
        {
            break status;
        }
        if Instant::now() >= deadline {
            timed_out = true;
            child
                .kill()
                .map_err(|error| RunnerError::Io(error.to_string()))?;
            break child
                .wait()
                .map_err(|error| RunnerError::Io(error.to_string()))?;
        }
        thread::sleep(Duration::from_millis(5));
    };
    out_reader
        .join()
        .map_err(|_| RunnerError::Io("stdout reader panicked".to_string()))?;
    err_reader
        .join()
        .map_err(|_| RunnerError::Io("stderr reader panicked".to_string()))?;
    let (stdout, stdout_truncated) = out
        .lock()
        .map_err(|_| RunnerError::Io("stdout lock poisoned".to_string()))?
        .clone();
    let (stderr, stderr_truncated) = err
        .lock()
        .map_err(|_| RunnerError::Io("stderr lock poisoned".to_string()))?
        .clone();
    let observation = if timed_out {
        ProcessObservationStatusV1::TimedOut
    } else if stdout_truncated || stderr_truncated {
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
    if spec.timeout.is_zero() || spec.stdout_limit == 0 || spec.stderr_limit == 0 {
        return Err(RunnerError::InvalidSpec(
            "timeout and output limits must be positive".to_string(),
        ));
    }
    if spec.program.ends_with("sh")
        || spec.program.ends_with("cmd.exe")
        || spec.program.ends_with("powershell.exe")
    {
        return Err(RunnerError::InvalidSpec(
            "shell programs are not accepted by the structured runner".to_string(),
        ));
    }
    Ok(())
}

fn spawn_reader<R: std::io::Read + Send + 'static>(
    mut reader: R,
    target: Arc<Mutex<(Vec<u8>, bool)>>,
    limit: usize,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut buffer = [0_u8; 4096];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(count) => {
                    if let Ok(mut captured) = target.lock() {
                        let remaining = limit.saturating_sub(captured.0.len());
                        let capped = count.min(remaining);
                        if let Some(chunk) = buffer.get(..capped) {
                            captured.0.extend_from_slice(chunk);
                        } else {
                            captured.1 = true;
                        }
                        if count > remaining {
                            captured.1 = true;
                        }
                    }
                }
            }
        }
    })
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

    fn spec() -> Result<ExecutionSpecV1, String> {
        let program = std::env::current_exe()
            .map_err(|error| error.to_string())?
            .display()
            .to_string();
        let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
        Ok(ExecutionSpecV1 {
            plan_id: "plan-1".to_string(),
            command_id: "test".to_string(),
            program,
            argv: vec!["--list".to_string()],
            cwd,
            env_allowlist: vec![],
            timeout: Duration::from_secs(5),
            stdout_limit: 64 * 1024,
            stderr_limit: 64 * 1024,
        })
    }

    #[test]
    fn bounded_runner_observes_a_completed_process() -> Result<(), String> {
        let receipt = execute_bounded(&spec()?).map_err(|error| error.as_str().to_string())?;
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
        let error = match execute_bounded(&candidate) {
            Ok(_) => return Err("shell program unexpectedly spawned".to_string()),
            Err(error) => error,
        };
        if error.as_str() != "malformed_execution_spec" {
            return Err(format!("unexpected runner error: {}", error.as_str()));
        }
        Ok(())
    }
}
