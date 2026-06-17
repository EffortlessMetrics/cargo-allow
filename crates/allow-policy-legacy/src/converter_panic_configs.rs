use allow_core::{AllowConfig, CargoAllowResult};

use crate::converter_config::config_from_entries;
use crate::converter_panic_entries::{
    entry_from_no_panic_allow_entry, entry_from_no_panic_baseline_entry,
};
use crate::types::{LegacyNoPanicAllowEntry, LegacyNoPanicBaselineEntry};

pub(crate) fn config_from_no_panic_baseline_entries(
    table: &toml::Table,
    entries: &[LegacyNoPanicBaselineEntry],
) -> CargoAllowResult<AllowConfig> {
    config_from_entries(
        table,
        entries.iter().map(entry_from_no_panic_baseline_entry),
    )
}

pub(crate) fn config_from_no_panic_allowlist_entries(
    table: &toml::Table,
    entries: &[LegacyNoPanicAllowEntry],
) -> CargoAllowResult<AllowConfig> {
    config_from_entries(table, entries.iter().map(entry_from_no_panic_allow_entry))
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{FindingKind, LastSeen};
    use std::path::Path;

    fn parse_table(input: &str) -> toml::Table {
        toml::from_str::<toml::Table>(input)
            .unwrap_or_else(|err| std::panic::panic_any(format!("test TOML parses: {err}")))
    }

    #[test]
    fn config_from_no_panic_baseline_entries_preserves_config_and_converted_entry() {
        let table = parse_table(
            r#"
owner = "panic"
status = "active"
"#,
        );
        let entries = vec![LegacyNoPanicBaselineEntry {
            index: 4,
            path: "src\\parser.rs".to_string(),
            family: "expect".to_string(),
            selector_kind: "method-call".to_string(),
            selector_callee: "core::result::Result::expect".to_string(),
            snippet: "value.expect(\"validated\")".to_string(),
            count: 2,
            owner: "unowned".to_string(),
            reason: "Generated from legacy no-panic baseline; requires human review.".to_string(),
            evidence: Vec::new(),
            created: Some(crate::default_baseline_created()),
            review_after: None,
            expires: Some(crate::default_baseline_expires()),
        }];

        let cfg = config_from_no_panic_baseline_entries(&table, &entries)
            .unwrap_or_else(|err| std::panic::panic_any(format!("config converts: {err}")));

        assert_eq!(cfg.schema_version, "0.1");
        assert_eq!(cfg.policy, "cargo-allow");
        assert_eq!(cfg.owner.as_deref(), Some("panic"));
        assert_eq!(cfg.status.as_deref(), Some("active"));
        assert_eq!(cfg.allow.len(), 1);
        let [entry] = cfg.allow.as_slice() else {
            std::panic::panic_any(format!("expected one allow entry, got {}", cfg.allow.len()));
        };
        assert_eq!(entry.id, "panic-baseline-0005");
        assert_eq!(entry.kind, FindingKind::Panic);
        assert_eq!(entry.family.as_deref(), Some("expect"));
        assert_eq!(entry.owner, "unowned");
        assert_eq!(entry.classification, "baseline_debt");
        assert_eq!(entry.occurrence_limit, Some(2));
        assert_eq!(entry.path.as_deref(), Some(Path::new("src/parser.rs")));
        assert_eq!(entry.selector.ast_kind.as_deref(), Some("method_call"));
        assert_eq!(entry.selector.callee.as_deref(), Some("expect"));
        assert_eq!(entry.selector.glob.as_deref(), Some("src/parser.rs"));
        assert_eq!(
            entry.evidence,
            vec![
                "legacy_policy:no-panic-baseline".to_string(),
                "legacy_selector_callee:core::result::Result::expect".to_string(),
                "baseline_count:2".to_string(),
            ]
        );
    }

    #[test]
    fn config_from_no_panic_allowlist_entries_preserves_config_and_converted_entry() {
        let table = parse_table(
            r#"
owner = "panic"
status = "active"
"#,
        );
        let entries = vec![LegacyNoPanicAllowEntry {
            index: 8,
            id: "no-panic-reviewed-unwrap".to_string(),
            path: "src\\parser.rs".to_string(),
            family: "unwrap".to_string(),
            selector_kind: "method-call".to_string(),
            selector_callee: Some("core::option::Option::unwrap".to_string()),
            selector_container: Some("parse_optional".to_string()),
            owner: "parser".to_string(),
            classification: "accepted".to_string(),
            reason: "Parser validates the optional value.".to_string(),
            evidence: vec!["test:parser_validates_optional_value".to_string()],
            created: Some("2026-01-01".to_string()),
            review_after: Some("2026-10-01".to_string()),
            expires: Some("2027-01-01".to_string()),
            line_hint: Some(17),
            last_seen: Some(LastSeen {
                line: 17,
                column: 12,
            }),
        }];

        let cfg = config_from_no_panic_allowlist_entries(&table, &entries)
            .unwrap_or_else(|err| std::panic::panic_any(format!("config converts: {err}")));

        assert_eq!(cfg.schema_version, "0.1");
        assert_eq!(cfg.policy, "cargo-allow");
        assert_eq!(cfg.owner.as_deref(), Some("panic"));
        assert_eq!(cfg.status.as_deref(), Some("active"));
        assert_eq!(cfg.allow.len(), 1);
        let [entry] = cfg.allow.as_slice() else {
            std::panic::panic_any(format!("expected one allow entry, got {}", cfg.allow.len()));
        };
        assert_eq!(entry.id, "no-panic-reviewed-unwrap");
        assert_eq!(entry.kind, FindingKind::Panic);
        assert_eq!(entry.family.as_deref(), Some("unwrap"));
        assert_eq!(entry.owner, "parser");
        assert_eq!(entry.classification, "accepted");
        assert_eq!(entry.path.as_deref(), Some(Path::new("src/parser.rs")));
        assert_eq!(entry.selector.ast_kind.as_deref(), Some("method_call"));
        assert_eq!(entry.selector.container.as_deref(), Some("parse_optional"));
        assert_eq!(entry.selector.callee.as_deref(), Some("unwrap"));
        assert_eq!(entry.selector.line_hint, Some(17));
        assert_eq!(
            entry
                .last_seen
                .as_ref()
                .map(|last_seen| (last_seen.line, last_seen.column)),
            Some((17, 12))
        );
        assert_eq!(
            entry.evidence,
            vec!["test:parser_validates_optional_value".to_string()]
        );
    }
}
