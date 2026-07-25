use allow_core::CargoAllowResult;
use allow_match::CheckMode;
use allow_policy::product_move::product_move_ledger_blocks_enforced_check;
use std::path::Path;

pub(crate) fn product_move_ledger_fails_check(
    root: &Path,
    mode: CheckMode,
) -> CargoAllowResult<bool> {
    if mode != CheckMode::NoNew && mode != CheckMode::Strict {
        return Ok(false);
    }
    product_move_ledger_blocks_enforced_check(root)
}

#[cfg(test)]
#[path = "check_product_move_guard_tests.rs"]
mod tests;
