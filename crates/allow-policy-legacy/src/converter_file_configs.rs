use allow_core::{AllowConfig, CargoAllowResult, Finding};

use crate::converter_config::config_from_entries;
use crate::converter_file_entries::{entry_from_finding, entry_from_rule};
use crate::converter_file_support::best_rule_index;
use crate::converter_generated_entries::entry_from_generated_rule;
use crate::types::{LegacyGeneratedRule, LegacyNonRustRule};

pub(crate) fn config_from_non_rust_rules(
    table: &toml::Table,
    rules: &[LegacyNonRustRule],
) -> CargoAllowResult<AllowConfig> {
    config_from_entries(table, rules.iter().map(entry_from_rule))
}

pub(crate) fn config_from_generated_rules(
    table: &toml::Table,
    rules: &[LegacyGeneratedRule],
) -> CargoAllowResult<AllowConfig> {
    config_from_entries(table, rules.iter().map(entry_from_generated_rule))
}

pub(crate) fn config_from_current_non_rust_findings(
    table: &toml::Table,
    rules: &[LegacyNonRustRule],
    findings: &[Finding],
) -> CargoAllowResult<AllowConfig> {
    config_from_entries(
        table,
        findings.iter().enumerate().filter_map(|(index, finding)| {
            best_rule_index(rules, finding)
                .and_then(|rule_index| rules.get(rule_index))
                .map(|rule| entry_from_finding(rule, finding, index + 1))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{FindingKind, Span, StructuralIdentity};
    use std::path::{Path, PathBuf};

    fn parse_table(input: &str) -> toml::Table {
        toml::from_str::<toml::Table>(input)
            .unwrap_or_else(|err| std::panic::panic_any(format!("test TOML parses: {err}")))
    }

    #[test]
    fn config_from_non_rust_rules_preserves_config_and_converted_entry() {
        let table = parse_table(
            r#"
owner = "files"
status = "active"
"#,
        );
        let rules = vec![LegacyNonRustRule {
            id: "non-rust-readme".to_string(),
            pattern: "README.md".to_string(),
            is_path: true,
            owner: "docs".to_string(),
            classification: "documentation".to_string(),
            reason: "Front door docs.".to_string(),
            evidence: vec!["doc:README.md".to_string()],
            created: Some("2026-05-09".to_string()),
            review_after: Some("2026-09-09".to_string()),
            expires: Some("never".to_string()),
        }];

        let cfg = config_from_non_rust_rules(&table, &rules)
            .unwrap_or_else(|err| std::panic::panic_any(format!("config converts: {err}")));

        assert_eq!(cfg.schema_version, "0.1");
        assert_eq!(cfg.policy, "cargo-allow");
        assert_eq!(cfg.owner.as_deref(), Some("files"));
        assert_eq!(cfg.status.as_deref(), Some("active"));
        assert_eq!(cfg.allow.len(), 1);
        let [entry] = cfg.allow.as_slice() else {
            std::panic::panic_any(format!("expected one allow entry, got {}", cfg.allow.len()));
        };
        assert_eq!(entry.id, "non-rust-readme");
        assert_eq!(entry.kind, FindingKind::NonRustFile);
        assert_eq!(entry.path.as_deref(), Some(Path::new("README.md")));
        assert_eq!(entry.glob, None);
        assert_eq!(entry.owner, "docs");
        assert_eq!(entry.classification, "documentation");
        assert_eq!(entry.reason, "Front door docs.");
        assert_eq!(entry.evidence, vec!["doc:README.md".to_string()]);
        assert_eq!(entry.selector.glob.as_deref(), Some("README.md"));
        assert_eq!(entry.lifecycle.created.as_deref(), Some("2026-05-09"));
        assert_eq!(entry.lifecycle.review_after.as_deref(), Some("2026-09-09"));
        assert_eq!(entry.lifecycle.expires.as_deref(), Some("never"));
    }

    #[test]
    fn config_from_generated_rules_preserves_config_and_converted_entry() {
        let table = parse_table(
            r#"
owner = "files"
status = "active"
"#,
        );
        let rules = vec![LegacyGeneratedRule {
            id: "generated-schema".to_string(),
            path: "docs\\generated\\schema.JSON".to_string(),
            owner: "policy".to_string(),
            reason: "generated schema fixture".to_string(),
            generator: Some("cargo xtask schema".to_string()),
            regenerate_command: Some("cargo xtask schema --check".to_string()),
            evidence: vec!["docs/schemas/README.md".to_string()],
            created: Some("2026-05-10".to_string()),
            expires: Some("never".to_string()),
        }];

        let cfg = config_from_generated_rules(&table, &rules)
            .unwrap_or_else(|err| std::panic::panic_any(format!("config converts: {err}")));

        assert_eq!(cfg.schema_version, "0.1");
        assert_eq!(cfg.policy, "cargo-allow");
        assert_eq!(cfg.owner.as_deref(), Some("files"));
        assert_eq!(cfg.status.as_deref(), Some("active"));
        assert_eq!(cfg.allow.len(), 1);
        let [entry] = cfg.allow.as_slice() else {
            std::panic::panic_any(format!("expected one allow entry, got {}", cfg.allow.len()));
        };
        assert_eq!(entry.id, "generated-schema");
        assert_eq!(entry.kind, FindingKind::GeneratedCode);
        assert_eq!(entry.family.as_deref(), Some("generated_code"));
        assert_eq!(
            entry.path,
            Some(PathBuf::from("docs/generated/schema.JSON"))
        );
        assert_eq!(entry.owner, "policy");
        assert_eq!(entry.classification, "generated_code");
        assert_eq!(entry.selector.ast_kind.as_deref(), Some("tracked_file"));
        assert_eq!(
            entry.selector.symbol.as_deref(),
            Some("docs/generated/schema.JSON")
        );
        assert_eq!(entry.selector.target_fingerprint.as_deref(), Some("json"));
        assert_eq!(
            entry.evidence,
            vec![
                "docs/schemas/README.md".to_string(),
                "legacy-policy:generated-schema".to_string(),
                "generator:cargo xtask schema".to_string(),
                "cargo:cargo xtask schema --check".to_string(),
            ]
        );
    }

    #[test]
    fn config_from_current_non_rust_findings_uses_best_matching_rule() {
        let table = parse_table(
            r#"
owner = "files"
status = "active"
"#,
        );
        let rules = vec![
            LegacyNonRustRule {
                id: "non-rust-docs".to_string(),
                pattern: "docs/**".to_string(),
                is_path: false,
                owner: "docs".to_string(),
                classification: "documentation".to_string(),
                reason: "Documentation files are retained.".to_string(),
                evidence: Vec::new(),
                created: Some("2026-05-09".to_string()),
                review_after: Some("2026-09-09".to_string()),
                expires: Some("never".to_string()),
            },
            LegacyNonRustRule {
                id: "non-rust-guide".to_string(),
                pattern: "docs\\guide.md".to_string(),
                is_path: true,
                owner: "docs".to_string(),
                classification: "user_guide".to_string(),
                reason: "The guide is the canonical walkthrough.".to_string(),
                evidence: vec!["doc:docs/guide.md".to_string()],
                created: Some("2026-05-10".to_string()),
                review_after: Some("2026-10-10".to_string()),
                expires: Some("2027-05-10".to_string()),
            },
        ];
        let findings = vec![
            file_finding(FindingKind::NonRustFile, "docs\\guide.md", 11),
            file_finding(FindingKind::Panic, "docs\\ignored.md", 12),
        ];

        let cfg = config_from_current_non_rust_findings(&table, &rules, &findings)
            .unwrap_or_else(|err| std::panic::panic_any(format!("config converts: {err}")));

        assert_eq!(cfg.schema_version, "0.1");
        assert_eq!(cfg.policy, "cargo-allow");
        assert_eq!(cfg.owner.as_deref(), Some("files"));
        assert_eq!(cfg.status.as_deref(), Some("active"));
        assert_eq!(cfg.allow.len(), 1);
        let [entry] = cfg.allow.as_slice() else {
            std::panic::panic_any(format!("expected one allow entry, got {}", cfg.allow.len()));
        };
        assert_eq!(entry.id, "non-rust-guide--0001");
        assert_eq!(entry.kind, FindingKind::NonRustFile);
        assert_eq!(entry.path.as_deref(), Some(Path::new("docs/guide.md")));
        assert_eq!(entry.owner, "docs");
        assert_eq!(entry.classification, "user_guide");
        assert_eq!(entry.reason, "The guide is the canonical walkthrough.");
        assert_eq!(entry.evidence, vec!["doc:docs/guide.md".to_string()]);
        assert_eq!(
            entry.links,
            vec!["legacy-policy:non-rust-guide".to_string()]
        );
        assert_eq!(entry.selector.ast_kind.as_deref(), Some("tracked_file"));
        assert_eq!(entry.selector.symbol.as_deref(), Some("docs/guide.md"));
        assert_eq!(entry.selector.glob.as_deref(), Some("docs/guide.md"));
        assert_eq!(
            entry
                .last_seen
                .as_ref()
                .map(|last_seen| (last_seen.line, last_seen.column)),
            Some((11, 1))
        );
        assert_eq!(entry.lifecycle.created.as_deref(), Some("2026-05-10"));
        assert_eq!(entry.lifecycle.review_after.as_deref(), Some("2026-10-10"));
        assert_eq!(entry.lifecycle.expires.as_deref(), Some("2027-05-10"));
    }

    fn file_finding(kind: FindingKind, path: &str, line: u32) -> Finding {
        Finding {
            kind,
            family: Some(kind.as_str().to_string()),
            path: PathBuf::from(path),
            span: Some(Span { line, column: 1 }),
            identity: StructuralIdentity::new("file", "tracked_file"),
            message: format!("tracked file: {path}"),
        }
    }
}
