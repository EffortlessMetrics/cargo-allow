use allow_core::{CargoAllowError, CargoAllowResult};
use toml::Value;

use crate::fields::{
    legacy_evidence, optional_last_seen, optional_u32_field, required_string_field, string_field,
};
use crate::parser_support::normalize_legacy_expires;
use crate::semantic_selector_fields::LegacySemanticSelectorExtras;
use crate::types::LegacyNoPanicAllowEntry;
use crate::{default_baseline_created, default_baseline_expires};

pub(crate) fn parse_no_panic_allowlist_entries(
    table: &toml::Table,
) -> CargoAllowResult<Vec<LegacyNoPanicAllowEntry>> {
    let entries = table
        .get("allow")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("no-panic-allowlist missing allow entries"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_no_panic_allowlist_entry(index, entry))
        .collect()
}

fn parse_no_panic_allowlist_entry(
    index: usize,
    entry: &Value,
) -> CargoAllowResult<LegacyNoPanicAllowEntry> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("no-panic allow entry {index} is not a table"))
    })?;
    let id = string_field(table, "id").unwrap_or_else(|| format!("legacy-no-panic-{index:04}"));
    let selector = table.get("selector").and_then(Value::as_table);
    let last_seen_table = table.get("last_seen").and_then(Value::as_table);
    let review_after = string_field(table, "review_after");
    let expires = normalize_legacy_expires(string_field(table, "expires"))
        .or_else(|| review_after.is_none().then(default_baseline_expires));
    let last_seen = optional_last_seen(last_seen_table);
    Ok(LegacyNoPanicAllowEntry {
        index,
        id: id.clone(),
        path: required_string_field(table, "path", &id)?,
        family: required_string_field(table, "family", &id)?,
        selector_kind: selector
            .and_then(|selector| {
                string_field(selector, "kind").or_else(|| string_field(selector, "ast_kind"))
            })
            .ok_or_else(|| CargoAllowError::new(format!("{id} missing selector.kind")))?,
        selector_callee: selector.and_then(|selector| string_field(selector, "callee")),
        selector_container: selector.and_then(|selector| string_field(selector, "container")),
        owner: string_field(table, "owner").unwrap_or_else(|| "unowned".to_string()),
        classification: string_field(table, "classification")
            .unwrap_or_else(|| "baseline_debt".to_string()),
        reason: string_field(table, "reason")
            .or_else(|| string_field(table, "explanation"))
            .unwrap_or_else(|| {
                "Generated from legacy no-panic allowlist; requires human review.".to_string()
            }),
        evidence: legacy_evidence(table),
        created: string_field(table, "created").or_else(|| Some(default_baseline_created())),
        review_after,
        expires,
        line_hint: selector
            .and_then(|selector| optional_u32_field(selector, "line_hint"))
            .or_else(|| last_seen.as_ref().map(|seen| seen.line)),
        last_seen,
        selector_semantics: LegacySemanticSelectorExtras::from_selector_table(selector),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_requires_allow_entries_array() {
        let table = parse_table(
            r#"
policy = "no-panic-allowlist"
"#,
        );

        let Err(err) = parse_no_panic_allowlist_entries(&table) else {
            std::panic::panic_any("expected missing allow entries error");
        };

        assert!(err.to_string().contains("missing allow entries"));
    }

    #[test]
    fn parser_defaults_missing_optional_no_panic_allow_fields() {
        let table = parse_table(
            r#"
policy = "no-panic-allowlist"

[[allow]]
path = "src/lib.rs"
family = "unwrap"

[allow.selector]
ast_kind = "method_call"
callee = "unwrap"
container = "load"
line_hint = 7
"#,
        );

        let entries = parse_entries(&table);

        let [entry] = entries.as_slice() else {
            std::panic::panic_any(format!("expected one entry, got {}", entries.len()));
        };
        assert_eq!(entry.index, 0);
        assert_eq!(entry.id, "legacy-no-panic-0000");
        assert_eq!(entry.path, "src/lib.rs");
        assert_eq!(entry.family, "unwrap");
        assert_eq!(entry.selector_kind, "method_call");
        assert_eq!(entry.selector_callee.as_deref(), Some("unwrap"));
        assert_eq!(entry.selector_container.as_deref(), Some("load"));
        assert_eq!(entry.owner, "unowned");
        assert_eq!(entry.classification, "baseline_debt");
        assert_eq!(
            entry.reason,
            "Generated from legacy no-panic allowlist; requires human review."
        );
        assert!(entry.evidence.is_empty());
        assert_eq!(
            entry.created.as_deref(),
            Some(default_baseline_created().as_str())
        );
        assert_eq!(entry.review_after, None);
        assert_eq!(
            entry.expires.as_deref(),
            Some(default_baseline_expires().as_str())
        );
        assert_eq!(entry.line_hint, Some(7));
        assert!(entry.last_seen.is_none());
        assert_eq!(
            entry.selector_semantics,
            LegacySemanticSelectorExtras::default()
        );
    }

    #[test]
    fn parser_preserves_legacy_receiver_alias_in_selector_semantics() {
        let table = parse_table(
            r#"
policy = "no-panic-allowlist"

[[allow]]
id = "semantic-receiver"
path = "src/lib.rs"
family = "unwrap"

[allow.selector]
kind = "method_call"
callee = "unwrap"
container = "load"
receiver = "optional_value"
"#,
        );

        let entries = parse_entries(&table);
        let [entry] = entries.as_slice() else {
            std::panic::panic_any(format!("expected one entry, got {}", entries.len()));
        };

        assert_eq!(
            entry.selector_semantics.receiver_fingerprint.as_deref(),
            Some("optional_value")
        );
    }

    #[test]
    fn parser_preserves_explicit_fields_last_seen_and_legacy_evidence() {
        let table = parse_table(
            r#"
policy = "no-panic-allowlist"

[[allow]]
id = "no-panic-reviewed"
path = "src/lib.rs"
family = "panic"
owner = "runtime"
classification = "accepted"
explanation = "Crash path is unreachable."
evidence = ["test:panic_path", "issue:#123"]
created = "2026-01-01"
review_after = "2026-10-01"
expires = "permanent"

[allow.selector]
kind = "macro_call"
callee = "panic"
container = "handler"

[allow.last_seen]
line = 12
column = 4

[[allow]]
path = "src/covered.rs"
family = "expect"
covered_by = "test:covered"

[allow.selector]
kind = "method_call"
"#,
        );

        let entries = parse_entries(&table);

        let [reviewed, covered] = entries.as_slice() else {
            std::panic::panic_any(format!("expected two entries, got {}", entries.len()));
        };
        assert_eq!(reviewed.index, 0);
        assert_eq!(reviewed.id, "no-panic-reviewed");
        assert_eq!(reviewed.path, "src/lib.rs");
        assert_eq!(reviewed.family, "panic");
        assert_eq!(reviewed.selector_kind, "macro_call");
        assert_eq!(reviewed.selector_callee.as_deref(), Some("panic"));
        assert_eq!(reviewed.selector_container.as_deref(), Some("handler"));
        assert_eq!(reviewed.owner, "runtime");
        assert_eq!(reviewed.classification, "accepted");
        assert_eq!(reviewed.reason, "Crash path is unreachable.");
        assert_eq!(
            reviewed.evidence,
            vec!["test:panic_path".to_string(), "issue:#123".to_string()]
        );
        assert_eq!(reviewed.created.as_deref(), Some("2026-01-01"));
        assert_eq!(reviewed.review_after.as_deref(), Some("2026-10-01"));
        assert_eq!(reviewed.expires.as_deref(), Some("never"));
        assert_eq!(reviewed.line_hint, Some(12));
        assert_eq!(
            reviewed
                .last_seen
                .as_ref()
                .map(|last_seen| (last_seen.line, last_seen.column)),
            Some((12, 4))
        );

        assert_eq!(covered.index, 1);
        assert_eq!(covered.id, "legacy-no-panic-0001");
        assert_eq!(covered.path, "src/covered.rs");
        assert_eq!(covered.family, "expect");
        assert_eq!(covered.selector_kind, "method_call");
        assert_eq!(covered.evidence, vec!["test:covered".to_string()]);
        assert_eq!(covered.line_hint, None);
    }

    #[test]
    fn parser_reports_contextual_no_panic_allow_errors() {
        let non_table = parse_table(
            r#"
allow = ["not-a-table"]
"#,
        );
        let Err(non_table_err) = parse_no_panic_allowlist_entries(&non_table) else {
            std::panic::panic_any("expected non-table allow entry error");
        };
        assert!(
            non_table_err
                .to_string()
                .contains("no-panic allow entry 0 is not a table")
        );

        let missing_path = parse_table(
            r#"
[[allow]]
id = "no-panic-missing-path"
family = "unwrap"

[allow.selector]
kind = "method_call"
"#,
        );
        let Err(missing_path_err) = parse_no_panic_allowlist_entries(&missing_path) else {
            std::panic::panic_any("expected missing path error");
        };
        assert!(
            missing_path_err
                .to_string()
                .contains("no-panic-missing-path missing path")
        );

        let missing_selector = parse_table(
            r#"
[[allow]]
id = "no-panic-missing-selector"
path = "src/lib.rs"
family = "unwrap"
"#,
        );
        let Err(missing_selector_err) = parse_no_panic_allowlist_entries(&missing_selector) else {
            std::panic::panic_any("expected missing selector error");
        };
        assert!(
            missing_selector_err
                .to_string()
                .contains("no-panic-missing-selector missing selector.kind")
        );
    }

    fn parse_entries(table: &toml::Table) -> Vec<LegacyNoPanicAllowEntry> {
        match parse_no_panic_allowlist_entries(table) {
            Ok(entries) => entries,
            Err(err) => std::panic::panic_any(format!("parse entries: {err}")),
        }
    }

    fn parse_table(input: &str) -> toml::Table {
        match toml::from_str::<toml::Table>(input) {
            Ok(table) => table,
            Err(err) => std::panic::panic_any(format!("parse TOML: {err}\n{input}")),
        }
    }
}
