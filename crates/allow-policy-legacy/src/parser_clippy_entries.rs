use allow_core::{CargoAllowError, CargoAllowResult};
use toml::Value;

use crate::fields::{legacy_evidence, required_string_field, string_field};
use crate::parser_support::{normalize_legacy_expires, normalize_lint_attribute_family};
use crate::types::LegacyClippyRule;
use crate::{default_baseline_created, default_baseline_expires};

pub(crate) fn parse_clippy_rules(table: &toml::Table) -> CargoAllowResult<Vec<LegacyClippyRule>> {
    let entries = table
        .get("allow")
        .or_else(|| table.get("entry"))
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("clippy-exceptions missing allow entries"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_clippy_rule(index, entry))
        .collect()
}

fn parse_clippy_rule(index: usize, entry: &Value) -> CargoAllowResult<LegacyClippyRule> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("clippy exception entry {index} is not a table"))
    })?;
    let id = string_field(table, "id").unwrap_or_else(|| format!("legacy-clippy-{index:04}"));
    let review_after = string_field(table, "review_after");
    let expires = normalize_legacy_expires(string_field(table, "expires"))
        .or_else(|| review_after.is_none().then(default_baseline_expires));
    Ok(LegacyClippyRule {
        path: required_string_field(table, "path", &id)?,
        lint: required_string_field(table, "lint", &id)?,
        family: string_field(table, "family")
            .or_else(|| string_field(table, "attribute"))
            .map(|family| normalize_lint_attribute_family(&family))
            .unwrap_or_else(|| "expect_attribute".to_string()),
        owner: string_field(table, "owner").unwrap_or_else(|| "unowned".to_string()),
        classification: string_field(table, "classification")
            .unwrap_or_else(|| "baseline_debt".to_string()),
        reason: string_field(table, "reason").unwrap_or_else(|| {
            "Generated from legacy Clippy exceptions policy; requires human review.".to_string()
        }),
        evidence: legacy_evidence(table),
        symbol: string_field(table, "symbol"),
        target_fingerprint: string_field(table, "target_fingerprint")
            .or_else(|| string_field(table, "policy_id").map(|id| format!("policy:{id}"))),
        created: string_field(table, "created").or_else(|| Some(default_baseline_created())),
        review_after,
        expires,
        id,
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
    fn parse_clippy_rules_accepts_allow_entries_and_preserves_fields() {
        let table = parse_table(
            r#"
[[allow]]
id = "clippy-unwrap-policy"
path = "src/lib.rs"
lint = "clippy::unwrap_used"
attribute = "expect"
owner = "lint"
classification = "reviewed_lint_exception"
reason = "Fixture keeps an explicit lint suppression linked to policy."
evidence = ["test:lint_policy_is_linked", "issue:#123"]
symbol = "parse_optional"
policy_id = "clippy-unwrap-policy"
created = "2026-05-09"
review_after = "2026-09-09"
expires = "permanent"

[[allow]]
path = "src/legacy.rs"
lint = "clippy::panic"
family = "allow-attribute"
covered_by = "test:legacy_panic_policy"
"#,
        );

        let mut rules = parse_clippy_rules(&table).unwrap_or_else(|err| {
            std::panic::panic_any(format!("clippy allow entries parse: {err}"))
        });

        assert_eq!(rules.len(), 2);
        let reviewed = rules.remove(0);
        assert_eq!(reviewed.id, "clippy-unwrap-policy");
        assert_eq!(reviewed.path, "src/lib.rs");
        assert_eq!(reviewed.lint, "clippy::unwrap_used");
        assert_eq!(reviewed.family, "expect_attribute");
        assert_eq!(reviewed.owner, "lint");
        assert_eq!(reviewed.classification, "reviewed_lint_exception");
        assert_eq!(
            reviewed.reason,
            "Fixture keeps an explicit lint suppression linked to policy."
        );
        assert_eq!(
            reviewed.evidence,
            vec![
                "test:lint_policy_is_linked".to_string(),
                "issue:#123".to_string(),
            ]
        );
        assert_eq!(reviewed.symbol.as_deref(), Some("parse_optional"));
        assert_eq!(
            reviewed.target_fingerprint.as_deref(),
            Some("policy:clippy-unwrap-policy")
        );
        assert_eq!(reviewed.created.as_deref(), Some("2026-05-09"));
        assert_eq!(reviewed.review_after.as_deref(), Some("2026-09-09"));
        assert_eq!(reviewed.expires.as_deref(), Some("never"));

        let generated = rules.remove(0);
        assert_eq!(generated.id, "legacy-clippy-0001");
        assert_eq!(generated.path, "src/legacy.rs");
        assert_eq!(generated.lint, "clippy::panic");
        assert_eq!(generated.family, "allow_attribute");
        assert_eq!(generated.owner, "unowned");
        assert_eq!(generated.classification, "baseline_debt");
        assert!(generated.reason.contains("legacy Clippy exceptions policy"));
        assert_eq!(
            generated.evidence,
            vec!["test:legacy_panic_policy".to_string()]
        );
        assert_eq!(generated.symbol, None);
        assert_eq!(generated.target_fingerprint, None);
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
    }

    #[test]
    fn parse_clippy_rules_accepts_entry_root_and_target_fingerprint_precedence() {
        let table = parse_table(
            r#"
[[entry]]
id = "clippy-debug"
path = "src/debug.rs"
lint = "clippy::dbg_macro"
family = "expect-attribute"
target_fingerprint = "fingerprint:explicit"
policy_id = "ignored-policy-id"
created = "2026-05-09"
review_after = "2026-09-09"
"#,
        );

        let mut rules = parse_clippy_rules(&table).unwrap_or_else(|err| {
            std::panic::panic_any(format!("entry-root clippy rules parse: {err}"))
        });

        assert_eq!(rules.len(), 1);
        let rule = rules.remove(0);
        assert_eq!(rule.id, "clippy-debug");
        assert_eq!(rule.path, "src/debug.rs");
        assert_eq!(rule.lint, "clippy::dbg_macro");
        assert_eq!(rule.family, "expect_attribute");
        assert_eq!(
            rule.target_fingerprint.as_deref(),
            Some("fingerprint:explicit")
        );
        assert_eq!(rule.created.as_deref(), Some("2026-05-09"));
        assert_eq!(rule.review_after.as_deref(), Some("2026-09-09"));
        assert!(rule.expires.is_none());
    }

    #[test]
    fn parse_clippy_rules_reports_expected_errors() {
        let missing_entries = parse_table("policy = \"clippy-exceptions\"");
        let err = parse_clippy_rules(&missing_entries)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("entries are required"));
        assert!(
            err.to_string()
                .contains("clippy-exceptions missing allow entries")
        );

        let non_table = parse_table("allow = [\"not a table\"]");
        let err = parse_clippy_rules(&non_table)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("entry must be a table"));
        assert!(
            err.to_string()
                .contains("clippy exception entry 0 is not a table")
        );

        let missing_path = parse_table(
            r#"
[[allow]]
id = "clippy-missing-path"
lint = "clippy::unwrap_used"
"#,
        );
        let err = parse_clippy_rules(&missing_path)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("path is required"));
        assert!(err.to_string().contains("clippy-missing-path missing path"));

        let missing_lint = parse_table(
            r#"
[[allow]]
id = "clippy-missing-lint"
path = "src/lib.rs"
"#,
        );
        let err = parse_clippy_rules(&missing_lint)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("lint is required"));
        assert!(err.to_string().contains("clippy-missing-lint missing lint"));
    }
}
