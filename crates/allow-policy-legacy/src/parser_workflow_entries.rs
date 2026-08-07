use allow_core::{CargoAllowError, CargoAllowResult};
use toml::Value;

use crate::fields::{legacy_evidence, string_array_field, string_field};
use crate::parser_support::normalize_legacy_date_field;
use crate::parser_support::normalize_legacy_expires;
use crate::types::LegacyWorkflowRule;

pub(crate) fn parse_workflow_rules(
    table: &toml::Table,
) -> CargoAllowResult<Vec<LegacyWorkflowRule>> {
    let entries = table
        .get("entry")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("workflow-allowlist missing entry records"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_workflow_rule(index, entry))
        .collect()
}

fn parse_workflow_rule(index: usize, entry: &Value) -> CargoAllowResult<LegacyWorkflowRule> {
    let table = entry
        .as_table()
        .ok_or_else(|| CargoAllowError::new(format!("workflow entry {index} is not a table")))?;
    let path = string_field(table, "path")
        .ok_or_else(|| CargoAllowError::new(format!("workflow entry {index} missing path")))?;
    Ok(LegacyWorkflowRule {
        path,
        owner: string_field(table, "owner").unwrap_or_default(),
        reason: string_field(table, "reason").unwrap_or_default(),
        permissions: string_array_field(table, "permissions"),
        secrets_used: string_array_field(table, "secrets_used"),
        external_actions: string_array_field(table, "external_actions"),
        duplicate_of_lane: string_field(table, "duplicate_of_lane"),
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
    fn parse_workflow_rules_preserves_metadata_lifecycle_and_evidence() {
        let table = parse_table(
            r#"
[[entry]]
path = ".github/workflows/ci.yml"
owner = "ci"
reason = "Primary CI workflow."
permissions = [" contents: read ", "checks: write", ""]
secrets_used = ["CARGO_REGISTRY_TOKEN"]
external_actions = ["actions/checkout@v4", "dtolnay/rust-toolchain@stable"]
duplicate_of_lane = "ci-main"
evidence = ["doc:docs/ci.md", "test:ci-workflow"]
created = "2026-05-09"
review_after = "2026-09-09"
expires = "permanent"

[[entry]]
path = ".github/workflows/docs.yml"
covered_by = "test:docs-workflow"
"#,
        );

        let mut rules = parse_workflow_rules(&table)
            .unwrap_or_else(|err| std::panic::panic_any(format!("workflow rules parse: {err}")));

        assert_eq!(rules.len(), 2);
        let full_rule = rules.remove(0);
        assert_eq!(full_rule.path, ".github/workflows/ci.yml");
        assert_eq!(full_rule.owner, "ci");
        assert_eq!(full_rule.reason, "Primary CI workflow.");
        assert_eq!(
            full_rule.permissions,
            vec!["contents: read".to_string(), "checks: write".to_string()]
        );
        assert_eq!(
            full_rule.secrets_used,
            vec!["CARGO_REGISTRY_TOKEN".to_string()]
        );
        assert_eq!(
            full_rule.external_actions,
            vec![
                "actions/checkout@v4".to_string(),
                "dtolnay/rust-toolchain@stable".to_string(),
            ]
        );
        assert_eq!(full_rule.duplicate_of_lane.as_deref(), Some("ci-main"));
        assert_eq!(
            full_rule.evidence,
            vec!["doc:docs/ci.md".to_string(), "test:ci-workflow".to_string()]
        );
        assert_eq!(full_rule.created.as_deref(), Some("2026-05-09"));
        assert_eq!(full_rule.review_after.as_deref(), Some("2026-09-09"));
        assert_eq!(full_rule.expires.as_deref(), Some("never"));

        let minimal_rule = rules.remove(0);
        assert_eq!(minimal_rule.path, ".github/workflows/docs.yml");
        assert!(minimal_rule.owner.is_empty());
        assert!(minimal_rule.reason.is_empty());
        assert!(minimal_rule.permissions.is_empty());
        assert!(minimal_rule.secrets_used.is_empty());
        assert!(minimal_rule.external_actions.is_empty());
        assert!(minimal_rule.duplicate_of_lane.is_none());
        assert_eq!(
            minimal_rule.evidence,
            vec!["test:docs-workflow".to_string()]
        );
        assert!(minimal_rule.created.is_none());
        assert!(minimal_rule.review_after.is_none());
        assert!(minimal_rule.expires.is_none());
    }

    #[test]
    fn parse_workflow_rules_reports_expected_errors() {
        let missing_entries = parse_table("policy = \"workflow-allowlist\"");
        let err = parse_workflow_rules(&missing_entries)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("entries are required"));
        assert!(
            err.to_string()
                .contains("workflow-allowlist missing entry records")
        );

        let non_table = parse_table("entry = [\"not a table\"]");
        let err = parse_workflow_rules(&non_table)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("entry must be a table"));
        assert!(err.to_string().contains("workflow entry 0 is not a table"));

        let missing_path = parse_table(
            r#"
[[entry]]
owner = "ci"
"#,
        );
        let err = parse_workflow_rules(&missing_path)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("path is required"));
        assert!(err.to_string().contains("workflow entry 0 missing path"));
    }
}
