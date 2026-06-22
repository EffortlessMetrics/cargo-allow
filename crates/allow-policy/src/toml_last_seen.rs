use allow_core::{CargoAllowError, CargoAllowResult, LastSeen};
use serde::Deserialize;

use crate::toml_de::option_u32_or_string;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn into_last_seen_returns_coordinates_when_line_and_column_are_present() {
        let actual = LastSeenToml {
            line: Some(42),
            column: Some(7),
        }
        .into_last_seen("allow-id");

        assert!(matches!(
            actual,
            Ok(Some(LastSeen {
                line: 42,
                column: 7
            }))
        ));
    }

    #[test]
    fn into_last_seen_returns_none_when_line_and_column_are_absent() {
        let actual = LastSeenToml {
            line: None,
            column: None,
        }
        .into_last_seen("allow-id");

        assert!(matches!(actual, Ok(None)));
    }

    #[test]
    fn into_last_seen_rejects_partial_coordinates() {
        for partial in [
            LastSeenToml {
                line: Some(42),
                column: None,
            },
            LastSeenToml {
                line: None,
                column: Some(7),
            },
        ] {
            let message = partial
                .into_last_seen("allow-id")
                .err()
                .map(|err| err.to_string())
                .unwrap_or_default();

            assert!(message.contains("allow-id last_seen must include both line and column"));
        }
    }
}
