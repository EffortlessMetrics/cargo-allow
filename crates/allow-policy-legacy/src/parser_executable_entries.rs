use allow_core::{CargoAllowError, CargoAllowResult};
use toml::Value;

use crate::fields::{legacy_evidence, string_field};
use crate::parser_support::normalize_legacy_date_field;
use crate::parser_support::normalize_legacy_expires;
use crate::types::LegacyExecutableRule;

pub(crate) fn parse_executable_rules(
    table: &toml::Table,
) -> CargoAllowResult<Vec<LegacyExecutableRule>> {
    let entries = table
        .get("allow")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("executable-allowlist missing allow entries"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_executable_rule(index, entry))
        .collect()
}

fn parse_executable_rule(index: usize, entry: &Value) -> CargoAllowResult<LegacyExecutableRule> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("executable allow entry {index} is not a table"))
    })?;
    let id = string_field(table, "id").unwrap_or_else(|| format!("legacy-executable-{index:04}"));
    let path = string_field(table, "path")
        .ok_or_else(|| CargoAllowError::new(format!("{id} missing path")))?;
    Ok(LegacyExecutableRule {
        id,
        path,
        owner: string_field(table, "owner").unwrap_or_default(),
        reason: string_field(table, "reason").unwrap_or_default(),
        interpreter: string_field(table, "interpreter"),
        evidence: legacy_evidence(table),
        created: normalize_legacy_date_field(string_field(table, "created")),
        review_after: normalize_legacy_date_field(string_field(table, "review_after")),
        expires: normalize_legacy_expires(string_field(table, "expires")),
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
    fn parse_executable_rules_preserves_metadata_lifecycle_and_evidence() {
        let table = parse_table(
            r#"
[[allow]]
id = "exec-release-script"
path = "scripts/release.ps1"
owner = "release"
reason = "Release script keeps executable mode."
interpreter = "powershell"
evidence = ["doc:docs/release.md", "test:package-proof"]
created = "2026-05-09"
review_after = "2026-09-09"
expires = "permanent"

[[allow]]
path = "scripts/tool"
covered_by = "test:script-mode"
"#,
        );

        let mut rules = parse_executable_rules(&table)
            .unwrap_or_else(|err| std::panic::panic_any(format!("executable rules parse: {err}")));

        assert_eq!(rules.len(), 2);
        let full_rule = rules.remove(0);
        assert_eq!(full_rule.id, "exec-release-script");
        assert_eq!(full_rule.path, "scripts/release.ps1");
        assert_eq!(full_rule.owner, "release");
        assert_eq!(full_rule.reason, "Release script keeps executable mode.");
        assert_eq!(full_rule.interpreter.as_deref(), Some("powershell"));
        assert_eq!(
            full_rule.evidence,
            vec![
                "doc:docs/release.md".to_string(),
                "test:package-proof".to_string(),
            ]
        );
        assert_eq!(full_rule.created.as_deref(), Some("2026-05-09"));
        assert_eq!(full_rule.review_after.as_deref(), Some("2026-09-09"));
        assert_eq!(full_rule.expires.as_deref(), Some("never"));

        let minimal_rule = rules.remove(0);
        assert_eq!(minimal_rule.id, "legacy-executable-0001");
        assert_eq!(minimal_rule.path, "scripts/tool");
        assert!(minimal_rule.owner.is_empty());
        assert!(minimal_rule.reason.is_empty());
        assert!(minimal_rule.interpreter.is_none());
        assert_eq!(minimal_rule.evidence, vec!["test:script-mode".to_string()]);
        assert!(minimal_rule.created.is_none());
        assert!(minimal_rule.review_after.is_none());
        assert!(minimal_rule.expires.is_none());
    }

    #[test]
    fn parse_executable_rules_reports_expected_errors() {
        let missing_entries = parse_table("policy = \"executable-allowlist\"");
        let err = parse_executable_rules(&missing_entries)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("entries are required"));
        assert!(
            err.to_string()
                .contains("executable-allowlist missing allow entries")
        );

        let non_table = parse_table("allow = [\"not a table\"]");
        let err = parse_executable_rules(&non_table)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("entry must be a table"));
        assert!(
            err.to_string()
                .contains("executable allow entry 0 is not a table")
        );

        let missing_path = parse_table(
            r#"
[[allow]]
id = "exec-missing"
owner = "release"
"#,
        );
        let err = parse_executable_rules(&missing_path)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("path is required"));
        assert!(err.to_string().contains("exec-missing missing path"));
    }
}
