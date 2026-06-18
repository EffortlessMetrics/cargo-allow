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
    let parsed = parse_federation_config(input)
        .unwrap_or_else(|err| std::panic::panic_any(format!("parse federation config: {err}")));
    validate_federation_config(parsed)
}

#[test]
fn parse_federation_config_reads_ledgers_table() {
    let validated = parse_validated(VALID_CONFIG);
    assert!(validated.valid);
    assert_eq!(validated.config.ledgers.len(), 2);
    assert_eq!(validated.config.ledgers[0].id, "source-policy");
    assert_eq!(validated.config.ledgers[0].role, LedgerRole::Canonical);
    assert_eq!(
        validated.config.ledgers[1].mode,
        LaneEnforcementMode::Blocking
    );
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
        config
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.kind == FederationDiagnosticKind::DialectSkipped }),
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
        config
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.kind == FederationDiagnosticKind::MirrorMissingTarget }),
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

#[test]
fn evaluate_two_canonical_ledgers_from_fixture_config() {
    let root = fixture_root_for_federation_test("two-canonical-ledgers");
    std::fs::create_dir_all(root.join(".allow"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("allow dir: {err}")));
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/federation/multi-ledger-config.toml");
    std::fs::copy(&fixture, root.join(".allow/config.toml")).unwrap_or_else(|err| {
        std::panic::panic_any(format!(
            "copy federation fixture from {}: {err}",
            fixture.display()
        ))
    });
    std::fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    std::fs::write(root.join("policy/allow.toml"), "schema_version = \"1.0\"\n")
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy write: {err}")));

    let (path, evaluation) = super::evaluate::evaluate_source_exception_policy(&root, None)
        .unwrap_or_else(|err| std::panic::panic_any(format!("evaluate fixture config: {err}")));

    assert_eq!(path, root.join("policy/allow.toml"));
    assert_eq!(
        evaluation.ledger_contributors.len(),
        2,
        "fixture registers two canonical ledgers"
    );
    assert_eq!(evaluation.ledger_contributors[0].id, "source-policy");
    assert_eq!(evaluation.ledger_contributors[1].id, "doc-artifacts");
    cleanup_fixture_root(&root);
}

fn fixture_root_for_federation_test(label: &str) -> std::path::PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "cargo-allow-federation-test-{label}-{}-{stamp}",
        std::process::id()
    ));
    if dir.exists() {
        std::fs::remove_dir_all(&dir)
            .unwrap_or_else(|err| std::panic::panic_any(format!("reset fixture dir: {err}")));
    }
    std::fs::create_dir_all(&dir)
        .unwrap_or_else(|err| std::panic::panic_any(format!("fixture dir: {err}")));
    dir
}

fn cleanup_fixture_root(root: &std::path::Path) {
    let _ = std::fs::remove_dir_all(root);
}
