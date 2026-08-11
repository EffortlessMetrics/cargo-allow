use allow_core::CargoAllowResult;
use allow_match::CheckMode;
use allow_policy::extraction_shims::extraction_shim_registry_blocks_enforced_check;
use std::path::Path;

pub(crate) fn extraction_shim_registry_fails_check(
    root: &Path,
    mode: CheckMode,
) -> CargoAllowResult<bool> {
    if mode != CheckMode::NoNew && mode != CheckMode::Strict {
        return Ok(false);
    }
    extraction_shim_registry_blocks_enforced_check(root)
}

#[cfg(test)]
#[path = "check_extraction_shim_guard_tests.rs"]
mod tests;
