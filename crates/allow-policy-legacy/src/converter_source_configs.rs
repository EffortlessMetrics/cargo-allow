use allow_core::{AllowConfig, CargoAllowResult};

use crate::converter_clippy_entries::entry_from_clippy_rule;
use crate::converter_config::config_from_entries;
use crate::converter_unsafe_entries::entry_from_unsafe_rule;
use crate::types::{LegacyClippyRule, LegacyUnsafeRule};

pub(crate) fn config_from_clippy_rules(
    table: &toml::Table,
    rules: &[LegacyClippyRule],
) -> CargoAllowResult<AllowConfig> {
    config_from_entries(table, rules.iter().map(entry_from_clippy_rule))
}

pub(crate) fn config_from_unsafe_rules(
    table: &toml::Table,
    rules: &[LegacyUnsafeRule],
) -> CargoAllowResult<AllowConfig> {
    config_from_entries(table, rules.iter().map(entry_from_unsafe_rule))
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{FindingKind, LastSeen};

    fn parse_table(input: &str) -> toml::Table {
        toml::from_str::<toml::Table>(input)
            .unwrap_or_else(|err| std::panic::panic_any(format!("test TOML parses: {err}")))
    }

    #[test]
    fn config_from_clippy_rules_preserves_config_and_converted_entry() {
        let table = parse_table(
            r#"
owner = "policy"
status = "active"
"#,
        );
        let rules = vec![LegacyClippyRule {
            id: "clippy-unwrap-policy".to_string(),
            path: "src\\lib.rs".to_string(),
            lint: "clippy::unwrap_used".to_string(),
            family: "expect_attribute".to_string(),
            owner: "lint".to_string(),
            classification: "reviewed_lint_exception".to_string(),
            reason: "Fixture keeps an explicit lint suppression linked to policy.".to_string(),
            evidence: vec!["test:lint_policy_is_linked".to_string()],
            symbol: Some("parse_optional".to_string()),
            target_fingerprint: Some("policy:clippy-unwrap-policy".to_string()),
            created: Some("2026-05-09".to_string()),
            review_after: Some("2026-09-09".to_string()),
            expires: Some("2027-05-09".to_string()),
            line_hint: None,
            last_seen: None,
        }];

        let cfg = config_from_clippy_rules(&table, &rules)
            .unwrap_or_else(|err| std::panic::panic_any(format!("config converts: {err}")));

        assert_eq!(cfg.schema_version, "0.1");
        assert_eq!(cfg.policy, "cargo-allow");
        assert_eq!(cfg.owner.as_deref(), Some("policy"));
        assert_eq!(cfg.status.as_deref(), Some("active"));
        assert_eq!(cfg.allow.len(), 1);
        let [entry] = cfg.allow.as_slice() else {
            std::panic::panic_any(format!("expected one allow entry, got {}", cfg.allow.len()));
        };
        assert_eq!(entry.id, "clippy-unwrap-policy");
        assert_eq!(entry.kind, FindingKind::LintException);
        assert_eq!(entry.family.as_deref(), Some("expect_attribute"));
        assert_eq!(entry.owner, "lint");
        assert_eq!(entry.classification, "reviewed_lint_exception");
        assert_eq!(entry.selector.lint.as_deref(), Some("clippy::unwrap_used"));
        assert_eq!(entry.selector.symbol.as_deref(), Some("parse_optional"));
        assert_eq!(
            entry.selector.target_fingerprint.as_deref(),
            Some("policy:clippy-unwrap-policy")
        );
        assert_eq!(entry.selector.glob.as_deref(), Some("src/lib.rs"));
        assert_eq!(
            entry.evidence,
            vec!["test:lint_policy_is_linked".to_string()]
        );
    }

    #[test]
    fn config_from_unsafe_rules_preserves_config_and_converted_entry() {
        let table = parse_table(
            r#"
owner = "safety"
status = "active"
"#,
        );
        let rules = vec![LegacyUnsafeRule {
            id: "unsafe-read".to_string(),
            path: "src\\ffi.rs".to_string(),
            family: "unsafe_block".to_string(),
            selector_kind: "unsafe_block".to_string(),
            selector_container: Some("read".to_string()),
            owner: "runtime".to_string(),
            classification: "accepted".to_string(),
            reason: "Unsafe read is bounded by caller invariants.".to_string(),
            evidence: vec!["unsafe-review:docs/evidence/unsafe/read.json".to_string()],
            created: Some("2026-01-01".to_string()),
            review_after: Some("2026-10-01".to_string()),
            expires: Some("2027-01-01".to_string()),
            line_hint: Some(7),
            last_seen: Some(LastSeen {
                line: 7,
                column: 12,
            }),
        }];

        let cfg = config_from_unsafe_rules(&table, &rules)
            .unwrap_or_else(|err| std::panic::panic_any(format!("config converts: {err}")));

        assert_eq!(cfg.schema_version, "0.1");
        assert_eq!(cfg.policy, "cargo-allow");
        assert_eq!(cfg.owner.as_deref(), Some("safety"));
        assert_eq!(cfg.status.as_deref(), Some("active"));
        assert_eq!(cfg.allow.len(), 1);
        let [entry] = cfg.allow.as_slice() else {
            std::panic::panic_any(format!("expected one allow entry, got {}", cfg.allow.len()));
        };
        assert_eq!(entry.id, "unsafe-read");
        assert_eq!(entry.kind, FindingKind::Unsafe);
        assert_eq!(entry.family.as_deref(), Some("unsafe_block"));
        assert_eq!(entry.owner, "runtime");
        assert_eq!(entry.classification, "accepted");
        assert_eq!(entry.selector.ast_kind.as_deref(), Some("unsafe_block"));
        assert_eq!(entry.selector.container.as_deref(), Some("read"));
        assert_eq!(entry.selector.line_hint, Some(7));
        assert_eq!(entry.selector.glob.as_deref(), Some("src/ffi.rs"));
        assert_eq!(
            entry
                .last_seen
                .as_ref()
                .map(|last_seen| (last_seen.line, last_seen.column)),
            Some((7, 12))
        );
        assert_eq!(
            entry.evidence,
            vec!["unsafe-review:docs/evidence/unsafe/read.json".to_string()]
        );
    }
}
