use allow_core::{CargoAllowError, CargoAllowResult};
use toml::Value;

use crate::fields::{raw_string_field, string_field, string_or_array_field};
use crate::parser_support::normalize_legacy_date_field;
use crate::parser_support::normalize_legacy_expires;
use crate::types::LegacyNonRustRule;
use crate::types_source_entries::UnmappedNonRustField;

const NON_RUST_ENTRY_FIELDS: &[&str] = &[
    "id",
    "path",
    "glob",
    "category",
    "classification",
    "owner",
    "reason",
    "broad_glob_reason",
    "evidence",
    "covered_by",
    "created",
    "review_after",
    "expires",
];

pub(crate) fn parse_non_rust_rules(
    table: &toml::Table,
) -> CargoAllowResult<Vec<LegacyNonRustRule>> {
    parse_non_rust_rules_with_direction(table, NonRustParseDirection::Migrate)
        .map(|(rules, _unmapped)| rules)
}

/// Parse a legacy non-Rust allowlist for the read path (`check --compat`).
///
/// Unlike [`parse_non_rust_rules`], fields the parser does not map are
/// tolerated and returned as [`UnmappedNonRustField`] values instead of
/// failing the read. Every other shape error still fails.
pub(crate) fn parse_non_rust_rules_compat(
    table: &toml::Table,
) -> CargoAllowResult<(Vec<LegacyNonRustRule>, Vec<UnmappedNonRustField>)> {
    parse_non_rust_rules_with_direction(table, NonRustParseDirection::Compat)
}

/// Which direction is reading the legacy file.
///
/// #4817: `check --compat` must be able to read a legacy file in order to
/// report on it, while `migrate` must not write a canonical policy that drops
/// a field. The strictness therefore belongs to the direction, not the parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NonRustParseDirection {
    /// Read path: tolerate unmapped fields and report them.
    Compat,
    /// Write path: keep #3508 and refuse unmapped fields.
    Migrate,
}

fn parse_non_rust_rules_with_direction(
    table: &toml::Table,
    direction: NonRustParseDirection,
) -> CargoAllowResult<(Vec<LegacyNonRustRule>, Vec<UnmappedNonRustField>)> {
    let entries = table
        .get("allow")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("non-rust-allowlist missing allow entries"))?;
    let mut rules = Vec::with_capacity(entries.len());
    let mut unmapped = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let (rule, entry_unmapped) = parse_non_rust_rule(index, entry, direction)?;
        unmapped.extend(entry_unmapped);
        rules.push(rule);
    }
    let mut ids = std::collections::BTreeSet::new();
    for (index, rule) in rules.iter().enumerate() {
        if !ids.insert(rule.id.as_str()) {
            return Err(CargoAllowError::new(format!(
                "non-rust allow entry {index} duplicates earlier id `{}`",
                rule.id
            )));
        }
    }
    Ok((rules, unmapped))
}

fn parse_non_rust_rule(
    index: usize,
    entry: &Value,
    direction: NonRustParseDirection,
) -> CargoAllowResult<(LegacyNonRustRule, Vec<UnmappedNonRustField>)> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("non-rust allow entry {index} is not a table"))
    })?;
    let unknown_fields = validate_non_rust_entry_shape(index, table, direction)?;
    let id = string_field(table, "id").unwrap_or_else(|| format!("legacy-non-rust-{index:04}"));
    let unmapped = unknown_fields
        .into_iter()
        .map(|field| UnmappedNonRustField {
            entry_id: id.clone(),
            field,
        })
        .collect();
    let (pattern, is_path) = match (string_field(table, "path"), string_field(table, "glob")) {
        (Some(path), None) => (path, true),
        (None, Some(glob)) => (glob, false),
        (Some(_), Some(_)) => {
            return Err(CargoAllowError::new(format!(
                "{id} defines both path and glob; choose one target form"
            )));
        }
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
    let classification = match (
        string_field(table, "category"),
        string_field(table, "classification"),
    ) {
        (Some(category), Some(classification)) if category != classification => {
            return Err(CargoAllowError::new(format!(
                "{id} defines conflicting category `{category}` and classification `{classification}`"
            )));
        }
        (Some(category), _) => category,
        (None, Some(classification)) => classification,
        (None, None) => "legacy_non_rust".to_string(),
    };
    Ok((
        LegacyNonRustRule {
            id: id.clone(),
            pattern,
            is_path,
            owner: string_field(table, "owner").unwrap_or_default(),
            classification,
            reason,
            evidence: non_rust_evidence(table),
            created: normalize_legacy_date_field(string_field(table, "created")),
            review_after: normalize_legacy_date_field(string_field(table, "review_after")),
            expires: normalize_legacy_expires(string_field(table, "expires")),
        },
        unmapped,
    ))
}

fn non_rust_evidence(table: &toml::Table) -> Vec<String> {
    let mut evidence = string_or_array_field(table, "evidence");
    for reference in string_or_array_field(table, "covered_by") {
        if !evidence.contains(&reference) {
            evidence.push(reference);
        }
    }
    evidence
}

fn validate_non_rust_entry_shape(
    index: usize,
    table: &toml::Table,
    direction: NonRustParseDirection,
) -> CargoAllowResult<Vec<String>> {
    // #4817: the read path (`check --compat`) must be able to read a legacy
    // file that carries fields this parser does not map, so unknown fields are
    // tolerated there and returned as unmapped. The write path (`migrate`)
    // keeps #3508: an unknown field is refused rather than silently dropped on
    // the way into the canonical ledger.
    let mut unknown_fields = Vec::new();
    for key in table.keys() {
        if !NON_RUST_ENTRY_FIELDS.contains(&key.as_str()) {
            if direction == NonRustParseDirection::Migrate {
                return Err(CargoAllowError::new(format!(
                    "non-rust allow entry {index} has unsupported field `{key}`"
                )));
            }
            unknown_fields.push(key.clone());
        }
    }
    for field in [
        "id",
        "path",
        "glob",
        "category",
        "classification",
        "owner",
        "reason",
        "broad_glob_reason",
        "created",
        "review_after",
        "expires",
    ] {
        if table.get(field).is_some_and(|value| !value.is_str()) {
            return Err(CargoAllowError::new(format!(
                "non-rust allow entry {index} field `{field}` must be a string"
            )));
        }
    }
    for field in ["evidence", "covered_by"] {
        let Some(value) = table.get(field) else {
            continue;
        };
        let valid = value.is_str()
            || value
                .as_array()
                .is_some_and(|items| items.iter().all(Value::is_str));
        if !valid {
            return Err(CargoAllowError::new(format!(
                "non-rust allow entry {index} field `{field}` must be a string or string array"
            )));
        }
    }
    Ok(unknown_fields)
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

        let alias = parse_table(
            r#"
[[allow]]
id = "non-rust-classification-alias"
path = "CONTRIBUTING.md"
classification = "documentation"
"#,
        );
        let rules = parse_non_rust_rules(&alias).unwrap_or_else(|err| {
            std::panic::panic_any(format!("classification alias parses: {err}"))
        });
        assert_eq!(rules[0].classification, "documentation");

        let dual_evidence = parse_table(
            r#"
[[allow]]
id = "non-rust-dual-evidence"
path = "SECURITY.md"
evidence = ["test:explicit", "issue:#2469"]
covered_by = ["issue:#2469", "spec:NON-RUST-DUAL"]
"#,
        );
        let rules = parse_non_rust_rules(&dual_evidence).unwrap_or_else(|err| {
            std::panic::panic_any(format!("dual evidence fields parse: {err}"))
        });
        assert_eq!(
            rules[0].evidence,
            vec![
                "test:explicit".to_string(),
                "issue:#2469".to_string(),
                "spec:NON-RUST-DUAL".to_string(),
            ]
        );
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

        let conflicting_target = parse_table(
            r#"
[[allow]]
id = "non-rust-conflict"
path = "docs/guide.md"
glob = "docs/**"
"#,
        );
        let err = parse_non_rust_rules(&conflicting_target)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("path and glob must conflict"));
        assert!(
            err.to_string()
                .contains("non-rust-conflict defines both path and glob")
        );

        let unsupported_field = parse_table(
            r#"
[[allow]]
id = "non-rust-unsupported"
path = "docs/guide.md"
approval_ticket = "SEC-123"
"#,
        );
        let err = parse_non_rust_rules(&unsupported_field)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("unknown field must be rejected"));
        assert!(
            err.to_string()
                .contains("unsupported field `approval_ticket`")
        );

        let malformed_evidence = parse_table(
            r#"
[[allow]]
id = "non-rust-malformed-evidence"
path = "docs/guide.md"
evidence = ["doc:docs/guide.md", 7]
"#,
        );
        let err = parse_non_rust_rules(&malformed_evidence)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("mixed evidence must be rejected"));
        assert!(
            err.to_string()
                .contains("field `evidence` must be a string or string array")
        );

        let duplicate_ids = parse_table(
            r#"
[[allow]]
id = "non-rust-duplicate"
path = "docs/guide.md"

[[allow]]
id = "non-rust-duplicate"
path = "docs/reference.md"
"#,
        );
        let err = parse_non_rust_rules(&duplicate_ids)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("duplicate IDs must be rejected"));
        assert!(
            err.to_string()
                .contains("non-rust allow entry 1 duplicates earlier id `non-rust-duplicate`")
        );

        let conflicting_classification = parse_table(
            r#"
[[allow]]
id = "non-rust-classification-conflict"
path = "docs/guide.md"
category = "documentation"
classification = "generated"
"#,
        );
        let err = parse_non_rust_rules(&conflicting_classification)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("classification conflict must be rejected"));
        assert!(err.to_string().contains(
            "defines conflicting category `documentation` and classification `generated`"
        ));
    }

    // #4817: the read path must be able to read a real legacy file that carries
    // a field this parser does not map, and must say which field it did not map.
    #[test]
    fn compat_read_tolerates_unknown_field_and_reports_it_as_unmapped() {
        let table = parse_table(
            r#"
[[allow]]
id = "shiplog-language"
path = "src/main.rs"
language = "rust"
owner = "shiplog"
reason = "Language field from the legacy schema."
"#,
        );

        let (rules, unmapped) = parse_non_rust_rules_compat(&table).unwrap_or_else(|err| {
            std::panic::panic_any(format!("compat read of unknown field must succeed: {err}"))
        });
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].id, "shiplog-language");
        assert_eq!(
            unmapped,
            vec![UnmappedNonRustField {
                entry_id: "shiplog-language".to_string(),
                field: "language".to_string(),
            }]
        );
    }

    // #4817: the write path must keep #3508 and refuse the same file, because a
    // canonical policy written from it would silently drop `language`.
    #[test]
    fn migrate_write_still_refuses_unknown_field() {
        let table = parse_table(
            r#"
[[allow]]
id = "shiplog-language"
path = "src/main.rs"
language = "rust"
"#,
        );

        let err = parse_non_rust_rules(&table)
            .err()
            .unwrap_or_else(|| std::panic::panic_any("migrate must refuse an unknown field"));
        assert!(
            err.to_string()
                .contains("non-rust allow entry 0 has unsupported field `language`"),
            "unexpected error: {err}"
        );
    }
}
