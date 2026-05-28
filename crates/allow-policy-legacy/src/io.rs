use allow_core::{CargoAllowError, CargoAllowResult};
use std::fs;
use std::path::Path;

pub(crate) fn read_policy(path: &Path) -> CargoAllowResult<String> {
    fs::read_to_string(path).map_err(|e| {
        CargoAllowError::new(format!(
            "failed to read legacy policy {}: {e}",
            path.display()
        ))
    })
}

pub(crate) fn legacy_table(input: &str) -> CargoAllowResult<Option<toml::Table>> {
    toml::from_str::<toml::Table>(input)
        .map(Some)
        .map_err(|e| CargoAllowError::new(format!("failed to parse legacy policy TOML: {e}")))
}
