//! Dry-run projections for reviewed command specs (#2603-B).
//!
//! Emits structured argv projections only. Never converts issue/spec prose into
//! pasteable shell commands.

use serde::{Deserialize, Serialize};

use super::command_spec::CommandInvocationSpecV1;

pub const DRY_RUN_COMMAND_REPORT_SCHEMA_ID: &str = "proof.dry-run-command-report.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShellProjectionKindV1 {
    StructuredArgv,
}

impl ShellProjectionKindV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StructuredArgv => "structured_argv",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DryRunCommandReportV1 {
    pub schema_id: String,
    pub command_id: String,
    pub program: String,
    pub argv: Vec<String>,
    pub cwd_policy: super::command_registry::CwdPolicyV1,
    pub env_allowlist: Vec<String>,
    pub would_read: Vec<String>,
    pub would_write: Vec<String>,
    pub network: super::command_registry::NetworkAccessV1,
    pub timeout_ms: u64,
    pub cancellation: super::command_registry::CancellationPostureV1,
    pub shell_projection_kind: ShellProjectionKindV1,
}

impl DryRunCommandReportV1 {
    pub fn from_invocation_spec(spec: &CommandInvocationSpecV1) -> Self {
        Self {
            schema_id: DRY_RUN_COMMAND_REPORT_SCHEMA_ID.to_string(),
            command_id: spec.command_id.clone(),
            program: spec.program.clone(),
            argv: spec.argv.clone(),
            cwd_policy: spec.cwd_policy,
            env_allowlist: spec.env_allowlist.clone(),
            would_read: spec.read_paths.clone(),
            would_write: spec.write_paths.clone(),
            network: spec.network,
            timeout_ms: spec.timeout_ms,
            cancellation: spec.cancellation,
            shell_projection_kind: ShellProjectionKindV1::StructuredArgv,
        }
    }
}

pub fn render_structured_argv(report: &DryRunCommandReportV1) -> String {
    let mut parts = Vec::with_capacity(report.argv.len() + 1);
    parts.push(report.program.clone());
    parts.extend(report.argv.iter().cloned());
    format!("[structured argv] {}", parts.join(" | "))
}
