use allow_core::{CargoAllowError, CargoAllowResult};
use toml::Value;

use crate::fields::{legacy_evidence, string_field};
use crate::parser_support::{has_glob_meta, normalize_legacy_date_field, normalize_legacy_expires};
use crate::types::LegacyDependencySurfaceRule;

pub(crate) fn parse_dependency_surface_rules(
    table: &toml::Table,
) -> CargoAllowResult<Vec<LegacyDependencySurfaceRule>> {
    let entries = table
        .get("allow")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            CargoAllowError::new("dependency-surface-allowlist missing allow entries")
        })?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_dependency_surface_rule(index, entry))
        .collect()
}

fn parse_dependency_surface_rule(
    index: usize,
    entry: &Value,
) -> CargoAllowResult<LegacyDependencySurfaceRule> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!(
            "dependency-surface allow entry {index} is not a table"
        ))
    })?;
    let id = string_field(table, "id").unwrap_or_else(|| format!("legacy-dependency-{index:04}"));
    let pattern = string_field(table, "path")
        .or_else(|| string_field(table, "glob"))
        .ok_or_else(|| CargoAllowError::new(format!("{id} missing path or glob")))?;
    Ok(LegacyDependencySurfaceRule {
        id,
        is_glob: has_glob_meta(&pattern),
        pattern,
        surface: string_field(table, "surface").unwrap_or_else(|| "dependency_surface".to_string()),
        owner: string_field(table, "owner").unwrap_or_default(),
        reason: string_field(table, "reason").unwrap_or_default(),
        broad_glob_reason: string_field(table, "broad_glob_reason"),
        dep_count_at_baseline: table
            .get("dep_count_at_baseline")
            .and_then(Value::as_integer),
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
    fn parse_dependency_surface_rules_preserves_path_glob_and_metadata() {
        let table = parse_table(
            r#"
[[allow]]
path = "Cargo.toml"
evidence = ["doc:docs/ci.md"]
dep_count_at_baseline = 42
created = "2026-05-09"
review_after = "2026-09-09"
expires = "permanent"

[[allow]]
id = "dependency-workspace-crates"
glob = "crates/*/Cargo.toml"
surface = "workspace_manifests"
owner = "build"
reason = "Workspace crate manifests own dependency declarations."
broad_glob_reason = "Only crate manifest files are in scope."
covered_by = "test:dependency_surface_manifest_scope"
created = "2026-05-10"
"#,
        );

        let mut rules = parse_dependency_surface_rules(&table).unwrap_or_else(|err| {
            std::panic::panic_any(format!("dependency surface entries parse: {err}"))
        });

        assert_eq!(rules.len(), 2);
        let path_rule = rules.remove(0);
        assert_eq!(path_rule.id, "legacy-dependency-0000");
        assert_eq!(path_rule.pattern, "Cargo.toml");
        assert!(!path_rule.is_glob);
        assert_eq!(path_rule.surface, "dependency_surface");
        assert!(path_rule.owner.is_empty());
        assert!(path_rule.reason.is_empty());
        assert_eq!(path_rule.broad_glob_reason, None);
        assert_eq!(path_rule.dep_count_at_baseline, Some(42));
        assert_eq!(path_rule.evidence, vec!["doc:docs/ci.md".to_string()]);
        assert_eq!(path_rule.created.as_deref(), Some("2026-05-09"));
        assert_eq!(path_rule.review_after.as_deref(), Some("2026-09-09"));
        assert_eq!(path_rule.expires.as_deref(), Some("never"));

        let glob_rule = rules.remove(0);
        assert_eq!(glob_rule.id, "dependency-workspace-crates");
        assert_eq!(glob_rule.pattern, "crates/*/Cargo.toml");
        assert!(glob_rule.is_glob);
        assert_eq!(glob_rule.surface, "workspace_manifests");
        assert_eq!(glob_rule.owner, "build");
        assert_eq!(
            glob_rule.reason,
            "Workspace crate manifests own dependency declarations."
        );
        assert_eq!(
            glob_rule.broad_glob_reason.as_deref(),
            Some("Only crate manifest files are in scope.")
        );
        assert_eq!(glob_rule.dep_count_at_baseline, None);
        assert_eq!(
            glob_rule.evidence,
            vec!["test:dependency_surface_manifest_scope".to_string()]
        );
        assert_eq!(glob_rule.created.as_deref(), Some("2026-05-10"));
        assert!(glob_rule.review_after.is_none());
        assert!(glob_rule.expires.is_none());
    }

    #[test]
    fn parse_dependency_surface_rules_reports_expected_errors() {
        let missing_entries = parse_table("policy = \"dependency-surface-allowlist\"");
        let err = parse_dependency_surface_rules(&missing_entries)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("entries are required"));
        assert!(
            err.to_string()
                .contains("dependency-surface-allowlist missing allow entries")
        );

        let non_table = parse_table("allow = [\"not a table\"]");
        let err = parse_dependency_surface_rules(&non_table)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("entry must be a table"));
        assert!(
            err.to_string()
                .contains("dependency-surface allow entry 0 is not a table")
        );

        let missing_path = parse_table(
            r#"
[[allow]]
id = "dependency-missing-pattern"
owner = "build"
"#,
        );
        let err = parse_dependency_surface_rules(&missing_path)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("path or glob is required"));
        assert!(
            err.to_string()
                .contains("dependency-missing-pattern missing path or glob")
        );
    }
}
