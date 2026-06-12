use allow_core::{CargoAllowError, CargoAllowResult};
use serde::Deserialize;

use super::{ArtifactKind, ArtifactStatus, DocArtifact, DocArtifactLedger};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveGoalManifest {
    pub schema_version: String,
    pub id: String,
    pub title: String,
    pub status: ActiveGoalStatus,
    pub owner: String,
    pub created: String,
    pub objective: Option<String>,
    pub linked_proposal: Option<String>,
    pub linked_spec: Option<String>,
    pub linked_support_tier: Option<String>,
    pub linked_plan: Option<String>,
    pub linked_plan_status: Option<ArtifactStatus>,
    pub claim_boundary: Option<String>,
    #[serde(default)]
    pub work_item: Vec<ActiveGoalWorkItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActiveGoalStatus {
    Active,
    Done,
    Blocked,
    Archived,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveGoalWorkItem {
    pub id: String,
    pub status: ActiveGoalWorkItemStatus,
    pub title: String,
    pub owner: Option<String>,
    pub agent: Option<String>,
    pub linked_proposal: Option<String>,
    pub linked_spec: Option<String>,
    pub linked_plan: Option<String>,
    pub linked_support_tier: Option<String>,
    pub linked_closeout: Option<String>,
    pub closeout: Option<String>,
    #[serde(default)]
    pub proof_commands: Vec<String>,
    #[serde(default)]
    pub evidence_notes: Vec<String>,
    pub blocker_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActiveGoalWorkItemStatus {
    Ready,
    InProgress,
    Done,
    Blocked,
}

pub fn parse_active_goal_manifest(input: &str) -> CargoAllowResult<ActiveGoalManifest> {
    toml::from_str::<ActiveGoalManifest>(input)
        .map_err(|e| CargoAllowError::new(format!("failed to parse active goal TOML: {e}")))
}

pub fn validate_active_goal_manifest(
    manifest: &ActiveGoalManifest,
    ledger: &DocArtifactLedger,
) -> CargoAllowResult<()> {
    ensure_non_empty("active goal schema_version", &manifest.schema_version)?;
    ensure_non_empty("active goal id", &manifest.id)?;
    ensure_non_empty("active goal title", &manifest.title)?;
    ensure_non_empty("active goal owner", &manifest.owner)?;
    ensure_non_empty("active goal created", &manifest.created)?;

    let registered_goal = resolve_required_target(
        ledger,
        "active goal id",
        Some(manifest.id.as_str()),
        &[ArtifactKind::ActiveGoal],
    )?;
    if registered_goal.status != manifest.status.into() {
        return Err(CargoAllowError::new(format!(
            "active goal {} status {} does not match registered status {}",
            manifest.id,
            active_goal_status_name(manifest.status),
            artifact_status_name(registered_goal.status)
        )));
    }

    resolve_required_target(
        ledger,
        "active goal linked_proposal",
        manifest.linked_proposal.as_deref(),
        &[ArtifactKind::Proposal],
    )?;
    resolve_required_target(
        ledger,
        "active goal linked_spec",
        manifest.linked_spec.as_deref(),
        &[ArtifactKind::Spec],
    )?;
    resolve_optional_target(
        ledger,
        "active goal linked_support_tier",
        manifest.linked_support_tier.as_deref(),
        &[ArtifactKind::SupportTier],
    )?;
    let linked_plan = resolve_required_target(
        ledger,
        "active goal linked_plan",
        manifest.linked_plan.as_deref(),
        &[ArtifactKind::ImplementationPlan, ArtifactKind::PlanItem],
    )?;
    if let Some(status) = manifest.linked_plan_status {
        if linked_plan.status != status {
            return Err(CargoAllowError::new(format!(
                "active goal linked_plan_status {} does not match {} status {}",
                artifact_status_name(status),
                linked_plan.id,
                artifact_status_name(linked_plan.status)
            )));
        }
    }

    for item in &manifest.work_item {
        validate_active_goal_work_item(item, ledger)?;
    }

    Ok(())
}

pub fn validate_active_goal_manifest_text(
    input: &str,
    ledger: &DocArtifactLedger,
) -> CargoAllowResult<ActiveGoalManifest> {
    let manifest = parse_active_goal_manifest(input)?;
    validate_active_goal_manifest(&manifest, ledger)?;
    Ok(manifest)
}

fn validate_active_goal_work_item(
    item: &ActiveGoalWorkItem,
    ledger: &DocArtifactLedger,
) -> CargoAllowResult<()> {
    ensure_non_empty("active goal work item id", &item.id)?;
    ensure_non_empty(&format!("{} title", item.id), &item.title)?;
    resolve_optional_target(
        ledger,
        &format!("{} linked_proposal", item.id),
        item.linked_proposal.as_deref(),
        &[ArtifactKind::Proposal],
    )?;
    resolve_optional_target(
        ledger,
        &format!("{} linked_spec", item.id),
        item.linked_spec.as_deref(),
        &[ArtifactKind::Spec],
    )?;
    resolve_optional_target(
        ledger,
        &format!("{} linked_plan", item.id),
        item.linked_plan.as_deref(),
        &[ArtifactKind::ImplementationPlan, ArtifactKind::PlanItem],
    )?;
    resolve_optional_target(
        ledger,
        &format!("{} linked_support_tier", item.id),
        item.linked_support_tier.as_deref(),
        &[ArtifactKind::SupportTier],
    )?;
    let linked_closeout = resolve_optional_target(
        ledger,
        &format!("{} linked_closeout", item.id),
        item.linked_closeout.as_deref(),
        &[ArtifactKind::Closeout],
    )?;
    let closeout = resolve_optional_target(
        ledger,
        &format!("{} closeout", item.id),
        item.closeout.as_deref(),
        &[ArtifactKind::Closeout],
    )?;

    match item.status {
        ActiveGoalWorkItemStatus::Ready
        | ActiveGoalWorkItemStatus::InProgress
        | ActiveGoalWorkItemStatus::Done => {
            if item.linked_plan.as_deref().is_none() && item.linked_spec.as_deref().is_none() {
                return Err(CargoAllowError::new(format!(
                    "{} {} work item requires linked_plan or linked_spec",
                    item.id,
                    active_goal_work_item_status_name(item.status)
                )));
            }
            validate_proof_commands(item)?;
        }
        ActiveGoalWorkItemStatus::Blocked => {
            ensure_non_empty(
                &format!("{} blocker_reason", item.id),
                item.blocker_reason.as_deref().unwrap_or(""),
            )?;
        }
    }

    if item.status == ActiveGoalWorkItemStatus::Done
        && linked_closeout.is_none()
        && closeout.is_none()
    {
        return Err(CargoAllowError::new(format!(
            "{} linked_closeout must not be empty",
            item.id
        )));
    }

    Ok(())
}

fn validate_proof_commands(item: &ActiveGoalWorkItem) -> CargoAllowResult<()> {
    if item.proof_commands.is_empty() {
        return Err(CargoAllowError::new(format!(
            "{} {} work item requires proof_commands",
            item.id,
            active_goal_work_item_status_name(item.status)
        )));
    }
    for command in &item.proof_commands {
        ensure_non_empty(&format!("{} proof command", item.id), command)?;
    }
    Ok(())
}

fn resolve_required_target<'a>(
    ledger: &'a DocArtifactLedger,
    field: &str,
    value: Option<&str>,
    expected: &[ArtifactKind],
) -> CargoAllowResult<&'a DocArtifact> {
    let Some(value) = value.filter(|value| !value.trim().is_empty()) else {
        return Err(CargoAllowError::new(format!("{field} must not be empty")));
    };
    resolve_target(ledger, field, value, expected)
}

fn resolve_optional_target<'a>(
    ledger: &'a DocArtifactLedger,
    field: &str,
    value: Option<&str>,
    expected: &[ArtifactKind],
) -> CargoAllowResult<Option<&'a DocArtifact>> {
    let Some(value) = value.filter(|value| !value.trim().is_empty()) else {
        return Ok(None);
    };
    resolve_target(ledger, field, value, expected).map(Some)
}

fn resolve_target<'a>(
    ledger: &'a DocArtifactLedger,
    field: &str,
    value: &str,
    expected: &[ArtifactKind],
) -> CargoAllowResult<&'a DocArtifact> {
    let Some(target) = ledger.artifact.iter().find(|artifact| {
        artifact.id == value
            || normalize_source_path(&artifact.path) == normalize_source_path(value)
    }) else {
        return Err(CargoAllowError::new(format!(
            "{field} target {value} is not registered by id or path"
        )));
    };
    if !expected.contains(&target.kind) {
        return Err(CargoAllowError::new(format!(
            "{field} target {value} is a {}; expected {}",
            artifact_kind_name(target.kind),
            expected
                .iter()
                .copied()
                .map(artifact_kind_name)
                .collect::<Vec<_>>()
                .join(" or ")
        )));
    }
    Ok(target)
}

fn normalize_source_path(path: &str) -> String {
    let mut normalized = path.replace('\\', "/");
    while let Some(stripped) = normalized.strip_prefix("./") {
        normalized = stripped.to_string();
    }
    normalized
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

impl From<ActiveGoalStatus> for ArtifactStatus {
    fn from(status: ActiveGoalStatus) -> Self {
        match status {
            ActiveGoalStatus::Active => Self::Active,
            ActiveGoalStatus::Done => Self::Done,
            ActiveGoalStatus::Blocked => Self::Active,
            ActiveGoalStatus::Archived => Self::Superseded,
        }
    }
}

fn active_goal_status_name(status: ActiveGoalStatus) -> &'static str {
    match status {
        ActiveGoalStatus::Active => "active",
        ActiveGoalStatus::Done => "done",
        ActiveGoalStatus::Blocked => "blocked",
        ActiveGoalStatus::Archived => "archived",
    }
}

fn active_goal_work_item_status_name(status: ActiveGoalWorkItemStatus) -> &'static str {
    match status {
        ActiveGoalWorkItemStatus::Ready => "ready",
        ActiveGoalWorkItemStatus::InProgress => "in_progress",
        ActiveGoalWorkItemStatus::Done => "done",
        ActiveGoalWorkItemStatus::Blocked => "blocked",
    }
}

fn artifact_kind_name(kind: ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::Proposal => "proposal",
        ArtifactKind::Spec => "spec",
        ArtifactKind::Adr => "adr",
        ArtifactKind::ImplementationPlan => "implementation_plan",
        ArtifactKind::PlanItem => "plan_item",
        ArtifactKind::ActiveGoal => "active_goal",
        ArtifactKind::SupportTier => "support_tier",
        ArtifactKind::PolicyLedger => "policy_ledger",
        ArtifactKind::Closeout => "closeout",
        ArtifactKind::ReleaseRecord => "release_record",
    }
}

fn artifact_status_name(status: ArtifactStatus) -> &'static str {
    match status {
        ArtifactStatus::Draft => "draft",
        ArtifactStatus::Proposed => "proposed",
        ArtifactStatus::Accepted => "accepted",
        ArtifactStatus::Active => "active",
        ArtifactStatus::Done => "done",
        ArtifactStatus::Superseded => "superseded",
    }
}
