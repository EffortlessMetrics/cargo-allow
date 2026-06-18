use allow_core::LaneEnforcementMode;

use super::config::{
    FederationConfig, FederationDiagnosticKind, LedgerEntry, LedgerRole, NATIVE_POLICY_DIALECT,
    ValidatedFederationConfig, parse_federation_config,
};
use super::precedence::ordered_ledgers_by_precedence;
use super::validate::validate_federation_config;

const VALID_CONFIG: &str = r#"
schema_version = "1.0"

[[ledgers]]
id = "source-policy"
path = "policy/allow.toml"
dialect = "cargo-allow"
role = "canonical"
lanes = ["source-exception"]
mode = "blocking"
priority = 10

[[ledgers]]
id = "doc-artifacts"
path = ".allow/artifacts/doc-artifacts.toml"
dialect = "cargo-allow-doc-artifacts"
role = "canonical"
lanes = ["spec-system"]
priority = 20
"#;

fn parse_validated(input: &str) -> ValidatedFederationConfig {
    validate_federation_config(parse_federation_config(input).expect("parse federation config"))
}

#[test]
fn parse_federation_config_reads_ledgers_table() {
    let validated = parse_validated(VALID_CONFIG);
    assert!(validated.valid);
    assert_eq!(validated.config.ledgers.len(), 2);
    assert_eq!(validated.config.ledgers[0].id, "source-policy");
    assert_eq!(validated.config.ledgers[0].role, LedgerRole::Canonical);
    assert_eq!(validated.config.ledgers[1].mode, LaneEnforcementMode::Blocking);
}

#[test]
fn validate_federation_config_rejects_duplicate_ids() {
    let config = parse_validated(
        r#"
schema_version = "1.0"

[[ledgers]]
id = "dup"
path = "policy/allow.toml"
dialect = "cargo-allow"
role = "canonical"

[[ledgers]]
id = "dup"
path = "policy/cargo-allow.toml"
dialect = "cargo-allow"
role = "imported"
"#,
    );
    assert!(!config.valid);
    assert!(
        config
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind == FederationDiagnosticKind::DuplicateId),
        "expected duplicate_id diagnostic: {:?}",
        config.diagnostics
    );
}

#[test]
fn validate_federation_config_reports_foreign_dialect_on_canonical() {
    let config = parse_validated(
        r#"
schema_version = "1.0"

[[ledgers]]
id = "legacy"
path = "policy/non-rust-allowlist.toml"
dialect = "non-rust-allowlist"
role = "canonical"
"#,
    );
    assert!(!config.valid);
    assert!(
        config.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == FederationDiagnosticKind::DialectConflict
                && diagnostic.message.contains("non-rust-allowlist")
        }),
        "expected dialect_conflict: {:?}",
        config.diagnostics
    );
}

#[test]
fn validate_federation_config_skips_foreign_dialect_on_imported() {
    let config = parse_validated(
        r#"
schema_version = "1.0"

[[ledgers]]
id = "legacy"
path = "policy/non-rust-allowlist.toml"
dialect = "non-rust-allowlist"
role = "imported"
"#,
    );
    assert!(config.valid);
    assert!(
        config.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == FederationDiagnosticKind::DialectSkipped
        }),
        "expected dialect_skipped: {:?}",
        config.diagnostics
    );
}

#[test]
fn validate_federation_config_requires_mirror_target() {
    let config = parse_validated(
        r#"
schema_version = "1.0"

[[ledgers]]
id = "mirror"
path = ".allow/mirror/policy.toml"
dialect = "cargo-allow"
role = "mirror"
"#,
    );
    assert!(!config.valid);
    assert!(
        config.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == FederationDiagnosticKind::MirrorMissingTarget
        }),
        "expected mirror_missing_target: {:?}",
        config.diagnostics
    );
}

#[test]
fn federation_precedence_orders_by_priority_then_declaration() {
    let config = FederationConfig {
        schema_version: "1.0".to_string(),
        ledgers: vec![
            LedgerEntry {
                id: "second".to_string(),
                path: "b.toml".to_string(),
                dialect: NATIVE_POLICY_DIALECT.to_string(),
                role: LedgerRole::Imported,
                lanes: Vec::new(),
                mode: LaneEnforcementMode::Advisory,
                priority: 20,
                mirrors: None,
            },
            LedgerEntry {
                id: "first".to_string(),
                path: "a.toml".to_string(),
                dialect: NATIVE_POLICY_DIALECT.to_string(),
                role: LedgerRole::Canonical,
                lanes: vec!["source-exception".to_string()],
                mode: LaneEnforcementMode::Blocking,
                priority: 10,
                mirrors: None,
            },
        ],
    };
    let validated = validate_federation_config(config);
    let ordered = ordered_ledgers_by_precedence(&validated.config.ledgers);
    assert_eq!(ordered[0].id, "first");
    assert_eq!(ordered[1].id, "second");
}
