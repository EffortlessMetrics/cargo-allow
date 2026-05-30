use allow_core::{CargoAllowError, CargoAllowResult, LastSeen};
use serde::Deserialize;

use crate::toml_de::option_u32_or_string;

#[derive(Debug, Default, Deserialize)]
pub(crate) struct LastSeenToml {
    #[serde(default, deserialize_with = "option_u32_or_string")]
    line: Option<u32>,
    #[serde(default, deserialize_with = "option_u32_or_string")]
    column: Option<u32>,
}

impl LastSeenToml {
    pub(crate) fn into_last_seen(self, id: &str) -> CargoAllowResult<Option<LastSeen>> {
        match (self.line, self.column) {
            (Some(line), Some(column)) => Ok(Some(LastSeen { line, column })),
            (None, None) => Ok(None),
            _ => Err(CargoAllowError::new(format!(
                "{id} last_seen must include both line and column"
            ))),
        }
    }
}
