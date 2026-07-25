use allow_core::{CargoAllowError, CargoAllowResult};
use toml::Value;

use crate::fields::{
    legacy_evidence, optional_last_seen, optional_u32_field, required_string_field, string_field,
};
use crate::parser_support::{normalize_legacy_expires, normalize_unsafe_family};
use crate::types::LegacyUnsafeRule;
use crate::{default_baseline_created, default_baseline_expires};

pub(crate) fn parse_unsafe_rules(table: &toml::Table) -> CargoAllowResult<Vec<LegacyUnsafeRule>> {
    let entries = table
        .get("allow")
        .or_else(|| table.get("entry"))
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("unsafe-allowlist missing allow entries"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_unsafe_rule(index, entry))
        .collect()
}

fn parse_unsafe_rule(index: usize, entry: &Value) -> CargoAllowResult<LegacyUnsafeRule> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("unsafe allow entry {index} is not a table"))
    })?;
    let id = string_field(table, "id").unwrap_or_else(|| format!("legacy-unsafe-{index:04}"));
    let selector = table.get("selector").and_then(Value::as_table);
    let last_seen_table = table.get("last_seen").and_then(Value::as_table);
    let review_after = string_field(table, "review_after");
    let expires = normalize_legacy_expires(string_field(table, "expires"))
        .or_else(|| review_after.is_none().then(default_baseline_expires));
    let family = string_field(table, "family")
        .or_else(|| {
            selector.and_then(|selector| {
                string_field(selector, "kind").or_else(|| string_field(selector, "ast_kind"))
            })
        })
        .ok_or_else(|| CargoAllowError::new(format!("{id} missing family or selector.kind")))?;
    let family = normalize_unsafe_family(&family);
    let selector_kind = selector
        .and_then(|selector| {
            string_field(selector, "kind").or_else(|| string_field(selector, "ast_kind"))
        })
        .map(|kind| normalize_unsafe_family(&kind))
        .unwrap_or_else(|| family.clone());
    let last_seen = optional_last_seen(last_seen_table);
    Ok(LegacyUnsafeRule {
        id: id.clone(),
        path: required_string_field(table, "path", &id)?,
        family,
        selector_kind,
        selector_container: selector.and_then(|selector| string_field(selector, "container")),
        owner: string_field(table, "owner").unwrap_or_else(|| "unowned".to_string()),
        classification: string_field(table, "classification")
            .unwrap_or_else(|| "baseline_debt".to_string()),
        reason: string_field(table, "reason")
            .or_else(|| string_field(table, "explanation"))
            .unwrap_or_else(|| {
                "Generated from legacy unsafe allowlist; requires human review.".to_string()
            }),
        evidence: legacy_evidence(table),
        created: string_field(table, "created").or_else(|| Some(default_baseline_created())),
        review_after,
        expires,
        line_hint: selector
            .and_then(|selector| optional_u32_field(selector, "line_hint"))
            .or_else(|| last_seen.as_ref().map(|seen| seen.line)),
        last_seen,
        scope: string_field(table, "scope"),
        justification: string_field(table, "justification"),
        audit_url: string_field(table, "audit_url"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::SimpleDate;

    fn parse_table(input: &str) -> toml::Table {
        toml::from_str::<toml::Table>(input)
            .unwrap_or_else(|err| std::panic::panic_any(format!("test TOML parses: {err}")))
    }

    #[test]
    fn parse_unsafe_rules_accepts_allow_entries_and_preserves_fields() {
        let table = parse_table(
            r#"
[[allow]]
id = "unsafe-read"
path = "src/lib.rs"
family = "unsafe-block"
owner = "runtime"
classification = "reviewed_unsafe_boundary"
reason = "Caller checks pointer."
evidence = ["unsafe-review:read.json"]
created = "2026-05-09"
review_after = "2026-09-09"
expires = "permanent"

[allow.selector]
kind = "unsafe-fn"
container = "read"
line_hint = 12

[allow.last_seen]
line = 14
column = 3

[[allow]]
path = "src/ffi.rs"
covered_by = "test:ffi_boundaries"

[allow.selector]
ast_kind = "unsafe extern block"
line_hint = 0

[allow.last_seen]
line = 22
"#,
        );

        let mut rules = parse_unsafe_rules(&table).unwrap_or_else(|err| {
            std::panic::panic_any(format!("unsafe allow entries parse: {err}"))
        });

        assert_eq!(rules.len(), 2);
        let reviewed = rules.remove(0);
        assert_eq!(reviewed.id, "unsafe-read");
        assert_eq!(reviewed.path, "src/lib.rs");
        assert_eq!(reviewed.family, "unsafe_block");
        assert_eq!(reviewed.selector_kind, "unsafe_fn");
        assert_eq!(reviewed.selector_container.as_deref(), Some("read"));
        assert_eq!(reviewed.owner, "runtime");
        assert_eq!(reviewed.classification, "reviewed_unsafe_boundary");
        assert_eq!(reviewed.reason, "Caller checks pointer.");
        assert_eq!(
            reviewed.evidence,
            vec!["unsafe-review:read.json".to_string()]
        );
        assert_eq!(reviewed.created.as_deref(), Some("2026-05-09"));
        assert_eq!(reviewed.review_after.as_deref(), Some("2026-09-09"));
        assert_eq!(reviewed.expires.as_deref(), Some("never"));
        assert_eq!(reviewed.line_hint, Some(12));
        assert_eq!(
            reviewed
                .last_seen
                .as_ref()
                .map(|seen| (seen.line, seen.column)),
            Some((14, 3))
        );

        let generated = rules.remove(0);
        assert_eq!(generated.id, "legacy-unsafe-0001");
        assert_eq!(generated.path, "src/ffi.rs");
        assert_eq!(generated.family, "unsafe_extern_block");
        assert_eq!(generated.selector_kind, "unsafe_extern_block");
        assert_eq!(generated.selector_container, None);
        assert_eq!(generated.owner, "unowned");
        assert_eq!(generated.classification, "baseline_debt");
        assert!(generated.reason.contains("legacy unsafe allowlist"));
        assert_eq!(generated.evidence, vec!["test:ffi_boundaries".to_string()]);
        assert!(
            generated
                .created
                .as_deref()
                .and_then(SimpleDate::parse)
                .is_some()
        );
        assert!(generated.review_after.is_none());
        assert!(
            generated
                .expires
                .as_deref()
                .and_then(SimpleDate::parse)
                .is_some()
        );
        assert_eq!(generated.line_hint, Some(22));
        assert_eq!(
            generated
                .last_seen
                .as_ref()
                .map(|seen| (seen.line, seen.column)),
            Some((22, 1))
        );
    }

    #[test]
    fn parse_unsafe_rules_accepts_entry_root_and_family_fallback() {
        let table = parse_table(
            r#"
[[entry]]
path = "src/trait.rs"
family = "unsafe-trait"
explanation = "Trait contract is reviewed."
created = "2026-05-09"
review_after = "2026-09-09"
"#,
        );

        let mut rules = parse_unsafe_rules(&table).unwrap_or_else(|err| {
            std::panic::panic_any(format!("entry-root unsafe rules parse: {err}"))
        });

        assert_eq!(rules.len(), 1);
        let rule = rules.remove(0);
        assert_eq!(rule.id, "legacy-unsafe-0000");
        assert_eq!(rule.path, "src/trait.rs");
        assert_eq!(rule.family, "unsafe_trait");
        assert_eq!(rule.selector_kind, "unsafe_trait");
        assert_eq!(rule.reason, "Trait contract is reviewed.");
        assert_eq!(rule.created.as_deref(), Some("2026-05-09"));
        assert_eq!(rule.review_after.as_deref(), Some("2026-09-09"));
        assert!(rule.expires.is_none());
        assert!(rule.last_seen.is_none());
        assert!(rule.line_hint.is_none());
    }

    #[test]
    fn parse_unsafe_rule_reports_expected_errors() {
        let missing_entries = parse_table("policy = \"unsafe-allowlist\"");
        let err = parse_unsafe_rules(&missing_entries)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("entries are required"));
        assert!(
            err.to_string()
                .contains("unsafe-allowlist missing allow entries")
        );

        let non_table = parse_table("allow = [\"not a table\"]");
        let err = parse_unsafe_rules(&non_table)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("entry must be a table"));
        assert!(
            err.to_string()
                .contains("unsafe allow entry 0 is not a table")
        );

        let missing_family = parse_table(
            r#"
[[allow]]
id = "unsafe-missing-family"
path = "src/lib.rs"
"#,
        );
        let err = parse_unsafe_rules(&missing_family)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("family is required"));
        assert!(
            err.to_string()
                .contains("unsafe-missing-family missing family or selector.kind")
        );

        let missing_path = parse_table(
            r#"
[[allow]]
id = "unsafe-missing-path"
family = "unsafe-block"
"#,
        );
        let err = parse_unsafe_rules(&missing_path)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("path is required"));
        assert!(err.to_string().contains("unsafe-missing-path missing path"));
    }
}
