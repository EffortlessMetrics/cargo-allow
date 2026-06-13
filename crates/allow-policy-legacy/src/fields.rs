use allow_core::{CargoAllowError, CargoAllowResult, LastSeen};
use toml::Value;

pub(crate) fn string_field(table: &toml::Table, field: &str) -> Option<String> {
    table
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(crate) fn raw_string_field(table: &toml::Table, field: &str) -> Option<String> {
    table.get(field).and_then(Value::as_str).map(str::to_string)
}

pub(crate) fn string_array_field(table: &toml::Table, field: &str) -> Vec<String> {
    table
        .get(field)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn string_or_array_field(table: &toml::Table, field: &str) -> Vec<String> {
    match table.get(field) {
        Some(Value::String(value)) if !value.trim().is_empty() => vec![value.trim().to_string()],
        Some(Value::Array(_)) => string_array_field(table, field),
        _ => Vec::new(),
    }
}

pub(crate) fn legacy_evidence(table: &toml::Table) -> Vec<String> {
    let mut evidence = string_or_array_field(table, "evidence");
    if evidence.is_empty() {
        evidence = string_or_array_field(table, "covered_by");
    }
    evidence
}

pub(crate) fn required_string_field(
    table: &toml::Table,
    field: &str,
    context: &str,
) -> CargoAllowResult<String> {
    string_field(table, field)
        .ok_or_else(|| CargoAllowError::new(format!("{context} missing {field}")))
}

pub(crate) fn required_string_array_field(
    table: &toml::Table,
    field: &str,
    context: &str,
) -> CargoAllowResult<Vec<String>> {
    let values = string_array_field(table, field);
    if values.is_empty() {
        Err(CargoAllowError::new(format!("{context} missing {field}")))
    } else {
        Ok(values)
    }
}

pub(crate) fn required_bool_field(
    table: &toml::Table,
    field: &str,
    context: &str,
) -> CargoAllowResult<bool> {
    table
        .get(field)
        .and_then(Value::as_bool)
        .ok_or_else(|| CargoAllowError::new(format!("{context} missing {field}")))
}

pub(crate) fn optional_u32_field(table: &toml::Table, field: &str) -> Option<u32> {
    table
        .get(field)
        .and_then(Value::as_integer)
        .filter(|value| *value > 0)
        .and_then(|value| u32::try_from(value).ok())
}

pub(crate) fn optional_last_seen(table: Option<&toml::Table>) -> Option<LastSeen> {
    let table = table?;
    Some(LastSeen {
        line: optional_u32_field(table, "line")?,
        column: optional_u32_field(table, "column").unwrap_or(1),
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
    fn string_helpers_trim_filter_and_preserve_raw_values() {
        let table = parse_table(
            r#"
name = "  reviewed-owner  "
blank = "   "
raw = "  keep surrounding whitespace  "
items = [" one ", "", " two ", 42, "   "]
single = " one "
empty_single = "   "
number = 42
"#,
        );

        assert_eq!(
            string_field(&table, "name").as_deref(),
            Some("reviewed-owner")
        );
        assert!(string_field(&table, "blank").is_none());
        assert!(string_field(&table, "missing").is_none());
        assert_eq!(
            raw_string_field(&table, "raw").as_deref(),
            Some("  keep surrounding whitespace  ")
        );
        assert!(raw_string_field(&table, "number").is_none());
        assert_eq!(
            string_array_field(&table, "items"),
            vec!["one".to_string(), "two".to_string()]
        );
        assert!(string_array_field(&table, "single").is_empty());
        assert_eq!(
            string_or_array_field(&table, "single"),
            vec!["one".to_string()]
        );
        assert!(string_or_array_field(&table, "empty_single").is_empty());
        assert_eq!(
            string_or_array_field(&table, "items"),
            vec!["one".to_string(), "two".to_string()]
        );
        assert!(string_or_array_field(&table, "number").is_empty());
    }

    #[test]
    fn legacy_evidence_prefers_explicit_evidence_then_covered_by() {
        let explicit = parse_table(
            r#"
evidence = [" doc:docs/ci.md ", "test:policy"]
covered_by = ["ignored:fallback"]
"#,
        );
        assert_eq!(
            legacy_evidence(&explicit),
            vec!["doc:docs/ci.md".to_string(), "test:policy".to_string()]
        );

        let fallback = parse_table(
            r#"
covered_by = " test:fallback "
"#,
        );
        assert_eq!(
            legacy_evidence(&fallback),
            vec!["test:fallback".to_string()]
        );

        let empty = parse_table(
            r#"
evidence = [" "]
covered_by = [" "]
"#,
        );
        assert!(legacy_evidence(&empty).is_empty());
    }

    #[test]
    fn required_helpers_report_missing_or_empty_fields() {
        let table = parse_table(
            r#"
name = "value"
blank = "   "
items = [" one ", "two"]
empty_items = [" ", 7]
enabled = true
enabled_text = "true"
"#,
        );

        assert_eq!(
            required_string_field(&table, "name", "entry")
                .unwrap_or_else(|err| std::panic::panic_any(format!("name exists: {err}"))),
            "value"
        );
        let err = required_string_field(&table, "blank", "entry")
            .err()
            .unwrap_or_else(|| std::panic::panic_any("blank string is missing"));
        assert!(err.to_string().contains("entry missing blank"));
        let err = required_string_field(&table, "missing", "entry")
            .err()
            .unwrap_or_else(|| std::panic::panic_any("missing string is required"));
        assert!(err.to_string().contains("entry missing missing"));

        assert_eq!(
            required_string_array_field(&table, "items", "entry")
                .unwrap_or_else(|err| std::panic::panic_any(format!("items exist: {err}"))),
            vec!["one".to_string(), "two".to_string()]
        );
        let err = required_string_array_field(&table, "empty_items", "entry")
            .err()
            .unwrap_or_else(|| std::panic::panic_any("empty array is missing"));
        assert!(err.to_string().contains("entry missing empty_items"));

        assert!(
            required_bool_field(&table, "enabled", "entry")
                .unwrap_or_else(|err| std::panic::panic_any(format!("bool exists: {err}")))
        );
        let err = required_bool_field(&table, "enabled_text", "entry")
            .err()
            .unwrap_or_else(|| std::panic::panic_any("string bool is not accepted here"));
        assert!(err.to_string().contains("entry missing enabled_text"));
    }

    #[test]
    fn optional_numeric_and_last_seen_helpers_accept_only_positive_lines() {
        let table = parse_table(
            r#"
positive = 7
zero = 0
negative = -2
too_large = 4294967296
text = "7"
"#,
        );

        assert_eq!(optional_u32_field(&table, "positive"), Some(7));
        assert_eq!(optional_u32_field(&table, "zero"), None);
        assert_eq!(optional_u32_field(&table, "negative"), None);
        assert_eq!(optional_u32_field(&table, "too_large"), None);
        assert_eq!(optional_u32_field(&table, "text"), None);
        assert_eq!(optional_u32_field(&table, "missing"), None);

        let with_column = parse_table("line = 12\ncolumn = 3\n");
        let seen = optional_last_seen(Some(&with_column))
            .unwrap_or_else(|| std::panic::panic_any("positive line yields last_seen"));
        assert_eq!(seen.line, 12);
        assert_eq!(seen.column, 3);

        let default_column = parse_table("line = 8\ncolumn = 0\n");
        let seen = optional_last_seen(Some(&default_column)).unwrap_or_else(|| {
            std::panic::panic_any("line without positive column yields default")
        });
        assert_eq!(seen.line, 8);
        assert_eq!(seen.column, 1);

        let missing_line = parse_table("column = 5\n");
        assert!(optional_last_seen(Some(&missing_line)).is_none());
        assert!(optional_last_seen(None).is_none());
    }
}
