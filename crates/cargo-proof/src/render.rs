//! Human and JSON renderer framework (#2589-B).

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Human,
    Json,
}

impl OutputFormat {
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "human" | "text" => Ok(Self::Human),
            "json" => Ok(Self::Json),
            other => Err(format!("unsupported output format: {other}")),
        }
    }
}

pub trait RenderFrame {
    fn summary_line(&self) -> String;
}

pub fn emit_frame<T: RenderFrame + Serialize>(
    frame: &T,
    format: OutputFormat,
) -> Result<String, String> {
    match format {
        OutputFormat::Human => Ok(format!("{}\n", frame.summary_line())),
        OutputFormat::Json => serde_json::to_string_pretty(frame)
            .map_err(|err| format!("serialize json frame: {err}")),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IdentityFrameV1 {
    pub schema_id: String,
    pub product_id: String,
    pub crate_version: String,
    pub claim_boundary: String,
}

impl RenderFrame for IdentityFrameV1 {
    fn summary_line(&self) -> String {
        format!("cargo-proof {} ({})", self.crate_version, self.product_id)
    }
}

impl IdentityFrameV1 {
    pub fn from_identity(identity: &crate::identity::ProductIdentityV1) -> Self {
        Self {
            schema_id: identity.schema_id.clone(),
            product_id: identity.product_id.clone(),
            crate_version: identity.crate_version.clone(),
            claim_boundary: identity.claim_boundary.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlanFrameV1 {
    pub schema_id: String,
    pub plan_id: String,
    pub intent_plan_digest: String,
    pub command_count: usize,
    pub claim_boundary: String,
}

impl RenderFrame for PlanFrameV1 {
    fn summary_line(&self) -> String {
        format!(
            "proof plan {} ({} commands)",
            self.plan_id, self.command_count
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DryRunFrameV1 {
    pub schema_id: String,
    pub plan_id: String,
    pub line_count: usize,
    pub structured_lines: Vec<String>,
    pub claim_boundary: String,
}

impl RenderFrame for DryRunFrameV1 {
    fn summary_line(&self) -> String {
        format!(
            "dry-run {} ({} structured argv lines)",
            self.plan_id, self.line_count
        )
    }
}
