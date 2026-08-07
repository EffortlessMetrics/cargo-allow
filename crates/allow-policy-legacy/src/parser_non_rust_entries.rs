use allow_core::{CargoAllowError, CargoAllowResult};
use toml::Value;

use crate::fields::{legacy_evidence, raw_string_field, string_field};
use crate::parser_support::normalize_legacy_date_field;
use crate::parser_support::normalize_legacy_expires;
use crate::types::LegacyNonRustRule;

pub(crate) fn parse_non_rust_rules(
    table: &toml::Table,
) -> CargoAllowResult<Vec<LegacyNonRustRule>> {
    let entries = table
        .get("allow")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("non-rust-allowlist missing allow entries"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_non_rust_rule(index, entry))
        .collect()
}

fn parse_non_rust_rule(index: usize, entry: &Value) -> CargoAllowResult<LegacyNonRustRule> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("non-rust allow entry {index} is not a table"))
    })?;
    let id = string_field(table, "id").unwrap_or_else(|| format!("legacy-non-rust-{index:04}"));
    let (pattern, is_path) = match (string_field(table, "path"), string_field(table, "glob")) {
        (Some(path), None) => (path, true),
        (None, Some(glob)) => (glob, false),
        (Some(path), Some(_)) => (path, true),
        (None, None) => {
            return Err(CargoAllowError::new(format!("{id} missing path or glob")));
        }
    };
    let reason_field = string_field(table, "reason");
    let raw_broad_glob_reason = raw_string_field(table, "broad_glob_reason");
    let broad_glob_reason = raw_broad_glob_reason
        .as_deref()
        .map(str::trim)
        .filter(|reason| !reason.is_empty())
        .map(str::to_string);
    if !is_path && is_broad_legacy_glob(&pattern) {
        match raw_broad_glob_reason.as_deref() {
            None => {
                return Err(CargoAllowError::new(format!(
                    "{id} broad glob `{pattern}` requires broad_glob_reason"
                )));
            }
            Some(reason) if reason.trim().is_empty() => {
                return Err(CargoAllowError::new(format!(
                    "{id} broad glob `{pattern}` has empty broad_glob_reason"
                )));
            }
            Some(_) => {}
        }
    }
    let reason = match (reason_field, broad_glob_reason) {
        (Some(reason), Some(scope_reason)) if !scope_reason.trim().is_empty() => {
            format!("{reason} Scope note: {scope_reason}")
        }
        (Some(reason), _) => reason,
        (None, Some(scope_reason)) => scope_reason,
        (None, None) => String::new(),
    };
    Ok(LegacyNonRustRule {
        id: id.clone(),
        pattern,
        is_path,
        owner: string_field(table, "owner").unwrap_or_default(),
        classification: string_field(table, "category")
            .unwrap_or_else(|| "legacy_non_rust".to_string()),
        reason,
        evidence: legacy_evidence(table),
        created: normalize_legacy_date_field(string_field(table, "created")),
        review_after: normalize_legacy_date_field(string_field(table, "review_after")),
        expires: normalize_legacy_expires(string_field(table, "expires")),
    })
}

fn is_broad_legacy_glob(pattern: &str) -> bool {
    pattern.contains('*')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_table(input: &str) -> toml::Table {
        toml::from_str::<toml::Table>(input)
            .unwrap_or_else(|err| std::panic::panic_any(format!("test TOML parses: {err}")))
    }

    #[test]
    fn parse_non_rust_rules_preserves_path_and_glob_entries() {
        let table = parse_table(
            r#"
[[allow]]
id = "non-rust-readme"
path = "README.md"
glob = "*.md"
category = "documentation"
owner = "docs"
reason = "Front door docs."
evidence = ["doc:README.md"]
created = "2026-05-09"
review_after = "2026-09-09"
expires = "permanent"

[[allow]]
glob = "docs/**"
owner = "docs"
reason = "Docs tree."
broad_glob_reason = "Docs are maintained as source-tree governance."
covered_by = ["doc:docs/README.md"]
"#,
        );

        let mut rules = parse_non_rust_rules(&table).unwrap_or_else(|err| {
            std::panic::panic_any(format!("non-rust allow entries parse: {err}"))
        });

        assert_eq!(rules.len(), 2);
        let path_rule = rules.remove(0);
        assert_eq!(path_rule.id, "non-rust-readme");
        assert_eq!(path_rule.pattern, "README.md");
        assert!(path_rule.is_path);
        assert_eq!(path_rule.owner, "docs");
        assert_eq!(path_rule.classification, "documentation");
        assert_eq!(path_rule.reason, "Front door docs.");
        assert_eq!(path_rule.evidence, vec!["doc:README.md".to_string()]);
        assert_eq!(path_rule.created.as_deref(), Some("2026-05-09"));
        assert_eq!(path_rule.review_after.as_deref(), Some("2026-09-09"));
        assert_eq!(path_rule.expires.as_deref(), Some("never"));

        let glob_rule = rules.remove(0);
        assert_eq!(glob_rule.id, "legacy-non-rust-0001");
        assert_eq!(glob_rule.pattern, "docs/**");
        assert!(!glob_rule.is_path);
        assert_eq!(glob_rule.owner, "docs");
        assert_eq!(glob_rule.classification, "legacy_non_rust");
        assert_eq!(
            glob_rule.reason,
            "Docs tree. Scope note: Docs are maintained as source-tree governance."
        );
        assert_eq!(glob_rule.evidence, vec!["doc:docs/README.md".to_string()]);
        assert!(glob_rule.created.is_none());
        assert!(glob_rule.review_after.is_none());
        assert!(glob_rule.expires.is_none());
    }

    #[test]
    fn parse_non_rust_rule_composes_reason_boundaries() {
        let table = parse_table(
            r#"
[[allow]]
id = "reason-only"
path = "Cargo.toml"
reason = "Package metadata."

[[allow]]
id = "scope-only"
path = ".github/config.yml"
broad_glob_reason = "Configuration is repo metadata."

[[allow]]
id = "empty-reason"
path = "rustfmt.toml"
"#,
        );

        let mut rules = parse_non_rust_rules(&table)
            .unwrap_or_else(|err| std::panic::panic_any(format!("reason cases parse: {err}")));

        assert_eq!(rules.len(), 3);
        assert_eq!(rules.remove(0).reason, "Package metadata.");
        assert_eq!(rules.remove(0).reason, "Configuration is repo metadata.");
        assert_eq!(rules.remove(0).reason, "");
    }

    #[test]
    fn parse_non_rust_rule_reports_expected_errors() {
        let missing_entries = parse_table("policy = \"non-rust-allowlist\"");
        let err = parse_non_rust_rules(&missing_entries)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("entries are required"));
        assert!(
            err.to_string()
                .contains("non-rust-allowlist missing allow entries")
        );

        let non_table = parse_table("allow = [\"not a table\"]");
        let err = parse_non_rust_rules(&non_table)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("entry must be a table"));
        assert!(
            err.to_string()
                .contains("non-rust allow entry 0 is not a table")
        );

        let missing_path_or_glob = parse_table(
            r#"
[[allow]]
id = "non-rust-missing-target"
"#,
        );
        let err = parse_non_rust_rules(&missing_path_or_glob)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("path or glob is required"));
        assert!(
            err.to_string()
                .contains("non-rust-missing-target missing path or glob")
        );

        let broad_glob_without_reason = parse_table(
            r#"
[[allow]]
id = "non-rust-docs"
glob = "docs/**"
"#,
        );
        let err = parse_non_rust_rules(&broad_glob_without_reason)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("broad glob reason is required"));
        assert!(
            err.to_string()
                .contains("non-rust-docs broad glob `docs/**` requires broad_glob_reason")
        );

        let broad_glob_empty_reason = parse_table(
            r#"
[[allow]]
id = "non-rust-docs"
glob = "docs/**"
broad_glob_reason = "  "
"#,
        );
        let err = parse_non_rust_rules(&broad_glob_empty_reason)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("broad glob reason cannot be empty"));
        assert!(
            err.to_string()
                .contains("non-rust-docs broad glob `docs/**` has empty broad_glob_reason")
        );
    }
}
