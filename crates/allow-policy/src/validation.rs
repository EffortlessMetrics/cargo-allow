use allow_core::{AllowConfig, CargoAllowResult};

use crate::entries_validation::validate_allow_entries;
use crate::policy_header::validate_policy_header;
use crate::scope_validation::validate_workspace;

pub fn validate_policy(cfg: &AllowConfig) -> CargoAllowResult<()> {
    validate_policy_header(cfg)?;
    validate_workspace(&cfg.workspace)?;
    validate_allow_entries(&cfg.allow, &cfg.requirements)
}
