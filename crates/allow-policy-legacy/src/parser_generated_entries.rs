use allow_core::{CargoAllowError, CargoAllowResult};
use toml::Value;

use crate::fields::{legacy_evidence, string_field};
use crate::parser_support::normalize_legacy_date_field;
use crate::parser_support::normalize_legacy_expires;
use crate::types::LegacyGeneratedRule;

pub(crate) fn parse_generated_rules(
    table: &toml::Table,
) -> CargoAllowResult<Vec<LegacyGeneratedRule>> {
    let entries = table
        .get("allow")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("generated-allowlist missing allow entries"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_generated_rule(index, entry))
        .collect()
}

fn parse_generated_rule(index: usize, entry: &Value) -> CargoAllowResult<LegacyGeneratedRule> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("generated allow entry {index} is not a table"))
    })?;
    let id = string_field(table, "id").unwrap_or_else(|| format!("legacy-generated-{index:04}"));
    let path = string_field(table, "path")
        .ok_or_else(|| CargoAllowError::new(format!("{id} missing path")))?;
    Ok(LegacyGeneratedRule {
        id,
        path,
        owner: string_field(table, "owner").unwrap_or_default(),
        reason: string_field(table, "reason").unwrap_or_default(),
        generator: string_field(table, "generator"),
        regenerate_command: string_field(table, "regenerate_command"),
        evidence: legacy_evidence(table),
        created: normalize_legacy_date_field(string_field(table, "created")),
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
    fn parse_generated_rules_preserves_metadata_lifecycle_and_evidence() {
        let table = parse_table(
            r#"
[[allow]]
id = "gen-cli"
path = "src/generated/cli.rs"
owner = "tools"
reason = "Generated command table."
generator = "xtask codegen"
regenerate_command = "cargo xtask generate-cli"
evidence = ["doc:docs/codegen.md", "test:generated-cli"]
created = "2026-05-09"
expires = "permanent"

[[allow]]
path = "src/generated/fallback.rs"
covered_by = "test:fallback-generated"
"#,
        );

        let mut rules = parse_generated_rules(&table)
            .unwrap_or_else(|err| std::panic::panic_any(format!("generated rules parse: {err}")));

        assert_eq!(rules.len(), 2);
        let full_rule = rules.remove(0);
        assert_eq!(full_rule.id, "gen-cli");
        assert_eq!(full_rule.path, "src/generated/cli.rs");
        assert_eq!(full_rule.owner, "tools");
        assert_eq!(full_rule.reason, "Generated command table.");
        assert_eq!(full_rule.generator.as_deref(), Some("xtask codegen"));
        assert_eq!(
            full_rule.regenerate_command.as_deref(),
            Some("cargo xtask generate-cli")
        );
        assert_eq!(
            full_rule.evidence,
            vec![
                "doc:docs/codegen.md".to_string(),
                "test:generated-cli".to_string(),
            ]
        );
        assert_eq!(full_rule.created.as_deref(), Some("2026-05-09"));
        assert_eq!(full_rule.expires.as_deref(), Some("never"));

        let minimal_rule = rules.remove(0);
        assert_eq!(minimal_rule.id, "legacy-generated-0001");
        assert_eq!(minimal_rule.path, "src/generated/fallback.rs");
        assert!(minimal_rule.owner.is_empty());
        assert!(minimal_rule.reason.is_empty());
        assert!(minimal_rule.generator.is_none());
        assert!(minimal_rule.regenerate_command.is_none());
        assert_eq!(
            minimal_rule.evidence,
            vec!["test:fallback-generated".to_string()]
        );
        assert!(minimal_rule.created.is_none());
        assert!(minimal_rule.expires.is_none());
    }

    #[test]
    fn parse_generated_rules_reports_expected_errors() {
        let missing_entries = parse_table("policy = \"generated-allowlist\"");
        let err = parse_generated_rules(&missing_entries)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("entries are required"));
        assert!(
            err.to_string()
                .contains("generated-allowlist missing allow entries")
        );

        let non_table = parse_table("allow = [\"not a table\"]");
        let err = parse_generated_rules(&non_table)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("entry must be a table"));
        assert!(
            err.to_string()
                .contains("generated allow entry 0 is not a table")
        );

        let missing_path = parse_table(
            r#"
[[allow]]
id = "gen-missing"
owner = "tools"
"#,
        );
        let err = parse_generated_rules(&missing_path)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("path is required"));
        assert!(err.to_string().contains("gen-missing missing path"));
    }
}
