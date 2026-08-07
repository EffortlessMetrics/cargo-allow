use allow_core::{CargoAllowError, CargoAllowResult};
use toml::Value;

use crate::fields::{
    legacy_evidence, required_bool_field, required_string_array_field, required_string_field,
    string_array_field, string_field,
};
use crate::parser_support::canonicalize_legacy_date;
use crate::parser_support::normalize_legacy_date_field;
use crate::parser_support::normalize_legacy_expires;
use crate::types::LegacyProcessRule;

pub(crate) fn parse_process_rules(table: &toml::Table) -> CargoAllowResult<Vec<LegacyProcessRule>> {
    let entries = table
        .get("allow")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("process-allowlist missing allow entries"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_process_rule(index, entry))
        .collect()
}

fn parse_process_rule(index: usize, entry: &Value) -> CargoAllowResult<LegacyProcessRule> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("process allow entry {index} is not a table"))
    })?;
    let id = required_string_field(table, "id", &format!("process allow entry {index}"))?;
    Ok(LegacyProcessRule {
        binary: required_string_field(table, "binary", &id)?,
        argv_shape: required_string_array_field(table, "argv_shape", &id)?,
        network_reach: required_bool_field(table, "network_reach", &id)?,
        called_by: string_array_field(table, "called_by"),
        owner: required_string_field(table, "owner", &id)?,
        reason: required_string_field(table, "reason", &id)?,
        evidence: legacy_evidence(table),
        created: Some(canonicalize_legacy_date(&required_string_field(
            table, "created", &id,
        )?)),
        review_after: normalize_legacy_date_field(string_field(table, "review_after")),
        expires: normalize_legacy_expires(string_field(table, "expires")),
        id,
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
    fn parse_process_rules_preserves_local_and_network_process_fields() {
        let table = parse_table(
            r#"
[[allow]]
id = "proc-cargo-fmt"
binary = "cargo"
argv_shape = ["fmt", "--all", "--check"]
network_reach = false
owner = "build"
reason = "Formatting check runs locally."
evidence = ["doc:docs/ci.md"]
created = "2026-05-09"
review_after = "2026-09-09"
expires = "permanent"

[[allow]]
id = "proc-download-tool"
binary = "pwsh"
argv_shape = ["-NoProfile", "-Command", "Invoke-WebRequest"]
network_reach = true
called_by = ["scripts\\fetch.ps1", ".github/workflows/ci.yml"]
owner = "ci"
reason = "CI fetches a pinned tool."
covered_by = "test:tool_download_is_pinned"
created = "2026-05-10"
"#,
        );

        let mut rules = parse_process_rules(&table).unwrap_or_else(|err| {
            std::panic::panic_any(format!("process allow entries parse: {err}"))
        });

        assert_eq!(rules.len(), 2);
        let local = rules.remove(0);
        assert_eq!(local.id, "proc-cargo-fmt");
        assert_eq!(local.binary, "cargo");
        assert_eq!(
            local.argv_shape,
            vec![
                "fmt".to_string(),
                "--all".to_string(),
                "--check".to_string()
            ]
        );
        assert!(!local.network_reach);
        assert!(local.called_by.is_empty());
        assert_eq!(local.owner, "build");
        assert_eq!(local.reason, "Formatting check runs locally.");
        assert_eq!(local.evidence, vec!["doc:docs/ci.md".to_string()]);
        assert_eq!(local.created.as_deref(), Some("2026-05-09"));
        assert_eq!(local.review_after.as_deref(), Some("2026-09-09"));
        assert_eq!(local.expires.as_deref(), Some("never"));

        let network = rules.remove(0);
        assert_eq!(network.id, "proc-download-tool");
        assert_eq!(network.binary, "pwsh");
        assert_eq!(
            network.argv_shape,
            vec![
                "-NoProfile".to_string(),
                "-Command".to_string(),
                "Invoke-WebRequest".to_string(),
            ]
        );
        assert!(network.network_reach);
        assert_eq!(
            network.called_by,
            vec![
                "scripts\\fetch.ps1".to_string(),
                ".github/workflows/ci.yml".to_string(),
            ]
        );
        assert_eq!(network.owner, "ci");
        assert_eq!(network.reason, "CI fetches a pinned tool.");
        assert_eq!(
            network.evidence,
            vec!["test:tool_download_is_pinned".to_string()]
        );
        assert_eq!(network.created.as_deref(), Some("2026-05-10"));
        assert!(network.review_after.is_none());
        assert!(network.expires.is_none());
    }

    #[test]
    fn parse_process_rules_reports_expected_errors() {
        let missing_entries = parse_table("policy = \"process-allowlist\"");
        let err = parse_process_rules(&missing_entries)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("entries are required"));
        assert!(
            err.to_string()
                .contains("process-allowlist missing allow entries")
        );

        let non_table = parse_table("allow = [\"not a table\"]");
        let err = parse_process_rules(&non_table)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("entry must be a table"));
        assert!(
            err.to_string()
                .contains("process allow entry 0 is not a table")
        );

        let missing_id = parse_table(
            r#"
[[allow]]
binary = "cargo"
argv_shape = ["fmt"]
network_reach = false
owner = "build"
reason = "Formatting check runs locally."
created = "2026-05-09"
"#,
        );
        let err = parse_process_rules(&missing_id)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("id is required"));
        assert!(err.to_string().contains("process allow entry 0 missing id"));

        let missing_argv_shape = parse_table(
            r#"
[[allow]]
id = "proc-missing-argv"
binary = "cargo"
network_reach = false
owner = "build"
reason = "Formatting check runs locally."
created = "2026-05-09"
"#,
        );
        let err = parse_process_rules(&missing_argv_shape)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("argv_shape is required"));
        assert!(
            err.to_string()
                .contains("proc-missing-argv missing argv_shape")
        );

        let missing_network_reach = parse_table(
            r#"
[[allow]]
id = "proc-missing-network"
binary = "cargo"
argv_shape = ["fmt"]
owner = "build"
reason = "Formatting check runs locally."
created = "2026-05-09"
"#,
        );
        let err = parse_process_rules(&missing_network_reach)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("network_reach is required"));
        assert!(
            err.to_string()
                .contains("proc-missing-network missing network_reach")
        );
    }
}
