use allow_core::{AllowConfig, CargoAllowResult};

use crate::entries_validation::{
    validate_allow_entries, validate_allow_entries_with_reportable_evidence,
};
use crate::lane_validation::validate_lanes;
use crate::policy_header::validate_policy_header;
use crate::scope_validation::validate_workspace;

pub fn validate_policy(cfg: &AllowConfig) -> CargoAllowResult<()> {
    validate_policy_header(cfg)?;
    validate_workspace(&cfg.workspace)?;
    validate_lanes(cfg)?;
    validate_allow_entries(&cfg.allow, &cfg.requirements)
}

pub(crate) fn validate_policy_with_reportable_evidence(cfg: &AllowConfig) -> CargoAllowResult<()> {
    validate_policy_header(cfg)?;
    validate_workspace(&cfg.workspace)?;
    validate_lanes(cfg)?;
    validate_allow_entries_with_reportable_evidence(&cfg.allow, &cfg.requirements)
}

#[cfg(test)]
#[path = "validation_orchestration_tests.rs"]
mod tests;
