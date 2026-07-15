use allow_core::{CargoAllowError, CargoAllowResult, read_text_file_capped};
use serde::Deserialize;
use std::collections::HashSet;
use std::path::Path;

use super::SpecSystemMode;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocArtifactLedger {
    pub schema_version: String,
    pub policy: String,
    pub owner: String,
    pub status: SpecSystemMode,
    #[serde(default)]
    pub artifact: Vec<DocArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocArtifact {
    pub id: String,
    pub kind: ArtifactKind,
    pub path: String,
    pub status: ArtifactStatus,
    pub owner: String,
    pub created: String,
    pub linked_proposal: Option<String>,
    pub linked_spec: Option<String>,
    pub linked_adr: Option<String>,
    pub linked_plan: Option<String>,
    pub linked_goal: Option<String>,
    pub linked_support_tier: Option<String>,
    pub linked_closeout: Option<String>,
    pub linked_plan_status: Option<String>,
    pub standalone_reason: Option<String>,
    pub supersedes: Option<String>,
    pub superseded_by: Option<String>,
    pub replaces: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Proposal,
    Spec,
    Adr,
    ImplementationPlan,
    PlanItem,
    ActiveGoal,
    SupportTier,
    PolicyLedger,
    Closeout,
    ReleaseRecord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactStatus {
    Draft,
    Proposed,
    Accepted,
    Active,
    Done,
    Superseded,
}

pub fn parse_doc_artifact_ledger(input: &str) -> CargoAllowResult<DocArtifactLedger> {
    parse_doc_artifact_ledger_at(None, input)
}

pub fn parse_doc_artifact_ledger_at(
    path: Option<&Path>,
    input: &str,
) -> CargoAllowResult<DocArtifactLedger> {
    let ledger = toml::from_str::<DocArtifactLedger>(input).map_err(|e| {
        CargoAllowError::with_kind(
            allow_core::CargoAllowErrorKind::InvalidConfig,
            format!("failed to parse doc artifact ledger TOML: {e}"),
        )
        .with_toml_span(path, input, e.span())
    })?;
    validate_doc_artifact_ledger(&ledger)?;
    Ok(ledger)
}

pub fn load_doc_artifacts(path: impl AsRef<Path>) -> CargoAllowResult<DocArtifactLedger> {
    let text = read_text_file_capped(path.as_ref()).map_err(|e| {
        CargoAllowError::new(format!(
            "failed to read doc artifact ledger {}: {e}",
            path.as_ref().display()
        ))
    })?;
    parse_doc_artifact_ledger_at(Some(path.as_ref()), &text)
}

fn validate_doc_artifact_ledger(ledger: &DocArtifactLedger) -> CargoAllowResult<()> {
    ensure_non_empty("doc artifact ledger schema_version", &ledger.schema_version)?;
    ensure_non_empty("doc artifact ledger policy", &ledger.policy)?;
    ensure_non_empty("doc artifact ledger owner", &ledger.owner)?;

    let mut ids = HashSet::new();
    for artifact in &ledger.artifact {
        ensure_non_empty("doc artifact id", &artifact.id)?;
        ensure_non_empty(&format!("{} path", artifact.id), &artifact.path)?;
        ensure_non_empty(&format!("{} owner", artifact.id), &artifact.owner)?;
        ensure_non_empty(&format!("{} created", artifact.id), &artifact.created)?;
        if !ids.insert(artifact.id.as_str()) {
            return Err(CargoAllowError::new(format!(
                "duplicate doc artifact id {}",
                artifact.id
            )));
        }
    }

    Ok(())
}

fn ensure_non_empty(label: &str, value: &str) -> CargoAllowResult<()> {
    if value.trim().is_empty() {
        return Err(CargoAllowError::new(format!("{label} must not be empty")));
    }
    if value.trim() != value {
        return Err(CargoAllowError::new(format!(
            "{label} must not have leading or trailing whitespace"
        )));
    }
    Ok(())
}
