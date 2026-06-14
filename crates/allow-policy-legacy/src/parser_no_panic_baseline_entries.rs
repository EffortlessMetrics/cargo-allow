use allow_core::{CargoAllowError, CargoAllowResult};
use toml::Value;

use crate::fields::required_string_field;
use crate::types::LegacyNoPanicBaselineEntry;

pub(crate) fn parse_no_panic_baseline_entries(
    table: &toml::Table,
) -> CargoAllowResult<Vec<LegacyNoPanicBaselineEntry>> {
    let entries = table
        .get("entry")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("no-panic-baseline missing entry records"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_no_panic_baseline_entry(index, entry))
        .collect()
}

fn parse_no_panic_baseline_entry(
    index: usize,
    entry: &Value,
) -> CargoAllowResult<LegacyNoPanicBaselineEntry> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("no-panic baseline entry {index} is not a table"))
    })?;
    let context = format!("no-panic baseline entry {index}");
    let count = table
        .get("count")
        .and_then(Value::as_integer)
        .filter(|value| *value > 0)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| CargoAllowError::new(format!("{context} missing count")))?;
    Ok(LegacyNoPanicBaselineEntry {
        index,
        path: required_string_field(table, "path", &context)?,
        family: required_string_field(table, "family", &context)?,
        selector_kind: required_string_field(table, "selector_kind", &context)?,
        selector_callee: required_string_field(table, "selector_callee", &context)?,
        snippet: required_string_field(table, "snippet", &context)?,
        count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_table(input: &str) -> toml::Table {
        toml::from_str::<toml::Table>(input)
            .unwrap_or_else(|err| std::panic::panic_any(format!("test TOML parses: {err}")))
    }

    #[test]
    fn parse_no_panic_baseline_entries_preserves_index_fields_and_count() {
        let table = parse_table(
            r#"
[[entry]]
path = "src/lib.rs"
family = "unwrap"
selector_kind = "call"
selector_callee = "unwrap"
snippet = "value.unwrap()"
count = 2

[[entry]]
path = "src/main.rs"
family = "expect"
selector_kind = "method_call"
selector_callee = "expect"
snippet = "value.expect(\"ready\")"
count = 1
"#,
        );

        let mut entries = parse_no_panic_baseline_entries(&table).unwrap_or_else(|err| {
            std::panic::panic_any(format!("no-panic baseline entries parse: {err}"))
        });

        assert_eq!(entries.len(), 2);
        let first = entries.remove(0);
        assert_eq!(first.index, 0);
        assert_eq!(first.path, "src/lib.rs");
        assert_eq!(first.family, "unwrap");
        assert_eq!(first.selector_kind, "call");
        assert_eq!(first.selector_callee, "unwrap");
        assert_eq!(first.snippet, "value.unwrap()");
        assert_eq!(first.count, 2);

        let second = entries.remove(0);
        assert_eq!(second.index, 1);
        assert_eq!(second.path, "src/main.rs");
        assert_eq!(second.family, "expect");
        assert_eq!(second.selector_kind, "method_call");
        assert_eq!(second.selector_callee, "expect");
        assert_eq!(second.snippet, "value.expect(\"ready\")");
        assert_eq!(second.count, 1);
    }

    #[test]
    fn parse_no_panic_baseline_entries_reports_expected_errors() {
        let missing_entries = parse_table("policy = \"no-panic-baseline\"");
        let err = parse_no_panic_baseline_entries(&missing_entries)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("entries are required"));
        assert!(
            err.to_string()
                .contains("no-panic-baseline missing entry records")
        );

        let non_table = parse_table("entry = [\"not a table\"]");
        let err = parse_no_panic_baseline_entries(&non_table)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("entry must be a table"));
        assert!(
            err.to_string()
                .contains("no-panic baseline entry 0 is not a table")
        );

        let missing_path = parse_table(
            r#"
[[entry]]
family = "unwrap"
selector_kind = "call"
selector_callee = "unwrap"
snippet = "value.unwrap()"
count = 1
"#,
        );
        let err = parse_no_panic_baseline_entries(&missing_path)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("path is required"));
        assert!(
            err.to_string()
                .contains("no-panic baseline entry 0 missing path")
        );
    }

    #[test]
    fn parse_no_panic_baseline_entries_requires_positive_count() {
        for count in ["0", "-1", "4294967296"] {
            let input = format!(
                r#"
[[entry]]
path = "src/lib.rs"
family = "unwrap"
selector_kind = "call"
selector_callee = "unwrap"
snippet = "value.unwrap()"
count = {count}
"#,
            );
            let table = parse_table(&input);
            let err = parse_no_panic_baseline_entries(&table)
                .err()
                .unwrap_or_else(|| std::panic::panic_any("invalid count is rejected"));
            assert!(
                err.to_string()
                    .contains("no-panic baseline entry 0 missing count")
            );
        }
    }
}
