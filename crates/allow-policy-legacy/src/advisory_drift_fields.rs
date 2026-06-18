use allow_core::LastSeen;
use toml::Value;

use crate::fields::{optional_last_seen, optional_u32_field};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct LegacyAdvisoryDriftHints {
    pub last_seen: Option<LastSeen>,
    pub line_hint: Option<u32>,
}

impl LegacyAdvisoryDriftHints {
    pub(crate) fn from_legacy_entry(table: &toml::Table, selector: Option<&toml::Table>) -> Self {
        let last_seen = optional_last_seen(table.get("last_seen").and_then(Value::as_table));
        let line_hint = selector
            .and_then(|selector| optional_u32_field(selector, "line_hint"))
            .or_else(|| last_seen.as_ref().map(|seen| seen.line));
        Self { last_seen, line_hint }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_table(input: &str) -> toml::Table {
        toml::from_str::<toml::Table>(input)
            .unwrap_or_else(|err| std::panic::panic_any(format!("test TOML parses: {err}")))
    }

    #[test]
    fn advisory_drift_hints_read_last_seen_and_selector_line_hint() {
        let table = parse_table(
            r#"
[last_seen]
line = 7
column = 12

[selector]
line_hint = 7
"#,
        );
        let selector = table.get("selector").and_then(Value::as_table);

        let hints = LegacyAdvisoryDriftHints::from_legacy_entry(&table, selector);

        assert_eq!(
            hints
                .last_seen
                .as_ref()
                .map(|last_seen| (last_seen.line, last_seen.column)),
            Some((7, 12))
        );
        assert_eq!(hints.line_hint, Some(7));
    }

    #[test]
    fn advisory_drift_hints_fall_back_line_hint_to_last_seen_line() {
        let table = parse_table(
            r#"
[last_seen]
line = 19
column = 3
"#,
        );

        let hints = LegacyAdvisoryDriftHints::from_legacy_entry(&table, None);

        assert_eq!(
            hints
                .last_seen
                .as_ref()
                .map(|last_seen| (last_seen.line, last_seen.column)),
            Some((19, 3))
        );
        assert_eq!(hints.line_hint, Some(19));
    }
}
