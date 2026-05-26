use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult};
use allow_policy::parse_policy;
use std::fs;
use std::path::Path;

pub fn load_legacy_or_canonical(path: impl AsRef<Path>) -> CargoAllowResult<AllowConfig> {
    // MVP adapter: legacy files whose entries already look like [[allow]] parse directly.
    // Dedicated no-panic/non-rust/clippy/unsafe mappers should be filled in PR-15..PR-18.
    let text = fs::read_to_string(path.as_ref()).map_err(|e| {
        CargoAllowError::new(format!(
            "failed to read legacy policy {}: {e}",
            path.as_ref().display()
        ))
    })?;
    parse_policy(&text)
}

pub fn migration_notes() -> &'static str {
    "Legacy migration MVP accepts legacy files that already use [[allow]]. PR-15..PR-18 should add dedicated mappings for [[exception]], covered_by, explanation, selector.kind, and surface fields."
}
