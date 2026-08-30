use allow_core::LaneEnforcementMode;

use super::config::{
    FederationConfig, FederationDiagnosticKind, LedgerEntry, LedgerRole, NATIVE_POLICY_DIALECT,
    ValidatedFederationConfig, parse_federation_config, parse_federation_config_at,
};
use super::precedence::ordered_ledgers_by_precedence;
use super::validate::validate_federation_config;
use std::fs;
use std::path::Path;

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
fn ledger_role_parse_is_case_insensitive_and_actionable() -> Result<(), String> {
    let canonical = LedgerRole::parse(" CANONICAL ")
        .map_err(|err| format!("canonical ledger role should parse: {err}"))?;
    let mirror = LedgerRole::parse("Mirror")
        .map_err(|err| format!("mirror ledger role should parse: {err}"))?;
    assert_eq!(canonical, LedgerRole::Canonical);
    assert_eq!(mirror, LedgerRole::Mirror);
    let error = LedgerRole::parse("aggregate")
        .expect_err("unknown ledger role should fail")
        .to_string();
    assert!(error.contains("unsupported ledger role `aggregate`"));
    assert!(error.contains("valid values: canonical, mirror, imported"));
    Ok(())
}

#[test]
fn parse_federation_config_at_preserves_location() -> Result<(), String> {
    let err = match parse_federation_config_at(Some(Path::new(".allow/config.toml")), "mode = [") {
        Ok(_) => return Err("invalid federation TOML unexpectedly parsed".to_string()),
        Err(err) => err,
    };
    assert_eq!(err.kind(), allow_core::CargoAllowErrorKind::InvalidConfig);
    let location = err
        .location()
        .ok_or_else(|| "federation parse error should have a location".to_string())?;
    assert_eq!(location.path.as_deref(), Some(".allow/config.toml"));
    assert_eq!(location.line, 1);
    assert!(location.column > 0);
    Ok(())
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
priority = 10

[[ledgers]]
id = "dup"
path = "policy/cargo-allow.toml"
dialect = "cargo-allow"
role = "imported"
priority = 20
"#,
    );
    assert!(!config.valid);
    let mut saw_duplicate_id = false;
    for diagnostic in config
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.kind == FederationDiagnosticKind::DuplicateId)
    {
        saw_duplicate_id = true;
        assert_eq!(diagnostic.ledger_ids, vec!["dup", "dup"]);
        assert!(
            diagnostic.message.contains("ledgers[0]")
                && diagnostic.message.contains("policy/allow.toml")
                && diagnostic.message.contains("ledgers[1]")
                && diagnostic.message.contains("policy/cargo-allow.toml"),
            "expected duplicate_id diagnostic to name both colliding ledger positions: {:?}",
            diagnostic
        );
    }
    assert!(
        saw_duplicate_id,
        "expected duplicate_id diagnostic: {:?}",
        config.diagnostics
    );
}

#[test]
fn validate_federation_config_rejects_empty_ledger_id() -> std::io::Result<()> {
    let config = parse_validated(
        r#"
schema_version = "1.0"

[[ledgers]]
id = ""
path = "policy/allow.toml"
dialect = "cargo-allow"
role = "canonical"
lanes = ["source-exception"]
priority = 10
"#,
    );
    if config.valid
        || !config.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == FederationDiagnosticKind::EmptyLedgerId
                && diagnostic.message.contains("ledgers[0]")
                && diagnostic.message.contains("policy/allow.toml")
        })
    {
        return Err(std::io::Error::other(format!(
            "empty federation identity was not rejected: {config:?}"
        )));
    }
    Ok(())
}

#[test]
fn evaluate_source_exception_policy_omits_invalid_registry_provenance() -> std::io::Result<()> {
    let root = fixture_root_for_federation_test("invalid-id-provenance");
    std::fs::create_dir_all(root.join(".allow"))?;
    std::fs::create_dir_all(root.join("policy"))?;
    std::fs::write(
        root.join(".allow/config.toml"),
        r#"schema_version = "1.0"

[[ledgers]]
id = "   "
path = "policy/allow.toml"
dialect = "cargo-allow"
role = "canonical"
lanes = ["source-exception"]
priority = 10
"#,
    )?;
    std::fs::write(root.join("policy/allow.toml"), "schema_version = \"0.1\"\n")?;

    let (path, evaluation) = super::evaluate::evaluate_source_exception_policy(&root, None)
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    if path != root.join("policy/allow.toml").canonicalize()?
        || evaluation.precedence_applied != super::evaluate::PrecedenceTier::DiscoveryFallback
        || evaluation.active_provenance.is_some()
        || !evaluation.ledger_contributors.is_empty()
    {
        cleanup_fixture_root(&root);
        return Err(std::io::Error::other(format!(
            "invalid registry supplied selection or provenance: {path:?} {evaluation:?}"
        )));
    }
    cleanup_fixture_root(&root);
    Ok(())
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
priority = 10
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
priority = 10
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
priority = 10
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
        drain_windows: Vec::new(),
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

#[test]
fn detect_mirror_divergence_reports_entry_id_mismatch() {
    let root = fixture_root_for_federation_test("mirror-divergence");
    std::fs::create_dir_all(root.join(".allow/mirror"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("mirror dir: {err}")));
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/federation/canonical-mirror-drain-config.toml");
    std::fs::copy(&fixture, root.join(".allow/config.toml")).unwrap_or_else(|err| {
        std::panic::panic_any(format!(
            "copy federation fixture from {}: {err}",
            fixture.display()
        ))
    });
    std::fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy dir: {err}")));
    std::fs::write(
        root.join("policy/allow.toml"),
        r#"schema_version = "0.1"
policy = "cargo-allow"

[[allow]]
id = "canonical-only"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "core"
classification = "reviewed"
reason = "canonical entry"
review_after = "2027-01-01"

[allow.selector]
ast_kind = "method_call"
callee = "unwrap"
container = "load"
"#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("canonical policy write: {err}")));
    fs::write(
        root.join(".allow/mirror/policy.toml"),
        "schema_version = \"0.1\"\n",
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("mirror policy write: {err}")));

    let validated = parse_validated(
        &std::fs::read_to_string(root.join(".allow/config.toml"))
            .unwrap_or_else(|err| std::panic::panic_any(format!("read config: {err}"))),
    );
    assert!(validated.valid);
    let divergences = super::divergence::detect_mirror_divergences(&root, &validated.config)
        .unwrap_or_else(|err| std::panic::panic_any(format!("detect divergences: {err}")));
    assert!(
        divergences.iter().any(|record| {
            record.kind == super::divergence::FederationDivergenceKind::MirrorDivergence
                && record
                    .sample_entry_ids
                    .contains(&"canonical-only".to_string())
        }),
        "expected mirror_divergence for canonical-only entry: {:?}",
        divergences
    );

    let (_path, evaluation) = super::evaluate::evaluate_source_exception_policy(&root, None)
        .unwrap_or_else(|err| std::panic::panic_any(format!("evaluate with divergences: {err}")));
    assert!(!evaluation.divergences.is_empty());
    cleanup_fixture_root(&root);
}

#[test]
fn validate_drain_window_requires_known_mirror_ledger() {
    let config = parse_validated(
        r#"
schema_version = "1.0"

[[ledgers]]
id = "source-policy"
path = "policy/allow.toml"
dialect = "cargo-allow"
role = "canonical"
priority = 10

[[drain_windows]]
mirror_ledger = "missing-mirror"
drain_owner = "repo-infra"
drain_reason = "test"
review_after = "2026-12-01"
linked_closeout = "plans/federation/closeouts/f2-evaluation.md"
"#,
    );
    assert!(!config.valid);
    assert!(
        config.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == FederationDiagnosticKind::UnknownDrainMirrorLedger
        }),
        "expected unknown_drain_mirror_ledger: {:?}",
        config.diagnostics
    );
}

#[test]
fn validate_drain_window_rejects_mirror_ledger_with_wrong_role() {
    // #1838: a drain window whose mirror_ledger exists but has a non-mirror
    // role must emit a distinct DrainWindowNotMirror diagnostic, not
    // UnknownDrainMirrorLedger (which is for truly unknown ids).
    let config = parse_validated(
        r#"
schema_version = "1.0"

[[ledgers]]
id = "source-policy"
path = "policy/allow.toml"
dialect = "cargo-allow"
role = "canonical"
priority = 10

[[ledgers]]
id = "source-policy-mirror"
path = ".allow/mirror/policy.toml"
dialect = "cargo-allow"
role = "canonical"
mirrors = "source-policy"
priority = 20

[[drain_windows]]
mirror_ledger = "source-policy-mirror"
drain_owner = "repo-infra"
drain_reason = "test"
review_after = "2026-12-01"
expiry = "2026-12-01"
linked_closeout = "plans/federation/closeouts/f2-evaluation.md"
"#,
    );
    assert!(!config.valid);
    let role_mismatch = config
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.kind == FederationDiagnosticKind::DrainWindowNotMirror);
    assert!(
        role_mismatch.is_some(),
        "expected drain_window_not_mirror for non-mirror role: {:?}",
        config.diagnostics
    );
    // The role mismatch must NOT reuse UnknownDrainMirrorLedger.
    assert!(
        !config
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind == FederationDiagnosticKind::UnknownDrainMirrorLedger),
        "role mismatch must not emit unknown_drain_mirror_ledger: {:?}",
        config.diagnostics
    );
}

#[test]
fn validate_drain_window_missing_field_and_role_mismatch_emit_distinct_kinds() {
    // #1838: a drain window with missing fields AND a non-mirror ledger
    // emits two diagnostics with distinct kinds (DrainWindowMissingField and
    // DrainWindowNotMirror), not a confusing duplicate of the same kind.
    // Previously the role mismatch reused UnknownDrainMirrorLedger,
    // conflating "unknown id" with "wrong role".
    let config = parse_validated(
        r#"
schema_version = "1.0"

[[ledgers]]
id = "source-policy"
path = "policy/allow.toml"
dialect = "cargo-allow"
role = "canonical"
priority = 10

[[ledgers]]
id = "source-policy-mirror"
path = ".allow/mirror/policy.toml"
dialect = "cargo-allow"
role = "canonical"
mirrors = "source-policy"
priority = 20

[[drain_windows]]
mirror_ledger = "source-policy-mirror"
drain_owner = "repo-infra"
drain_reason = "test"
review_after = "2026-12-01"
linked_closeout = "plans/federation/closeouts/f2-evaluation.md"
"#,
    );
    assert!(!config.valid);
    // Both diagnostics are present with distinct kinds.
    assert!(
        config.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == FederationDiagnosticKind::DrainWindowMissingField
        }),
        "expected drain_window_missing_field: {:?}",
        config.diagnostics
    );
    assert!(
        config.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == FederationDiagnosticKind::DrainWindowNotMirror
        }),
        "expected drain_window_not_mirror: {:?}",
        config.diagnostics
    );
    // No UnknownDrainMirrorLedger — the ledger IS known, just wrong role.
    assert!(
        !config
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind == FederationDiagnosticKind::UnknownDrainMirrorLedger),
        "known-but-wrong-role must not emit unknown_drain_mirror_ledger: {:?}",
        config.diagnostics
    );
}

#[test]
fn validate_drain_window_requires_expiry_so_it_cannot_be_permanent() {
    // #2006: a drain window without `expiry` never reports DrainExpired
    // (has_passed_date_str(None, _) is false), so the mirror ledger would live
    // forever. `expiry` must be a required field.
    let config = parse_validated(
        r#"
schema_version = "1.0"

[[ledgers]]
id = "source-policy"
path = "policy/allow.toml"
dialect = "cargo-allow"
role = "canonical"
priority = 10

[[ledgers]]
id = "source-policy-mirror"
path = ".allow/mirror/policy.toml"
dialect = "cargo-allow"
role = "mirror"
mirrors = "source-policy"
priority = 20

[[drain_windows]]
mirror_ledger = "source-policy-mirror"
drain_owner = "repo-infra"
drain_reason = "test"
review_after = "2026-12-01"
linked_closeout = "plans/federation/closeouts/f2-evaluation.md"
"#,
    );
    assert!(
        !config.valid,
        "drain window without expiry must be invalid (would be permanent): {:?}",
        config.diagnostics
    );
    assert!(
        config.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == FederationDiagnosticKind::DrainWindowMissingField
        }),
        "expected drain_window_missing_field for missing expiry: {:?}",
        config.diagnostics
    );
}

#[test]
fn validate_drain_window_with_expiry_is_accepted() {
    // A complete drain window (including expiry) must validate cleanly.
    let config = parse_validated(
        r#"
schema_version = "1.0"

[[ledgers]]
id = "source-policy"
path = "policy/allow.toml"
dialect = "cargo-allow"
role = "canonical"
priority = 10

[[ledgers]]
id = "source-policy-mirror"
path = ".allow/mirror/policy.toml"
dialect = "cargo-allow"
role = "mirror"
mirrors = "source-policy"
priority = 20

[[drain_windows]]
mirror_ledger = "source-policy-mirror"
drain_owner = "repo-infra"
drain_reason = "test"
review_after = "2026-12-01"
expiry = "2027-12-31"
linked_closeout = "plans/federation/closeouts/f2-evaluation.md"
"#,
    );
    assert!(
        config.valid,
        "complete drain window should validate: {:?}",
        config.diagnostics
    );
}

#[test]
fn validate_drain_window_rejects_malformed_expiry() {
    // #2007: a malformed expiry silently never fires DrainExpired. Each must be
    // a blocking validation error naming the field, value, and required format.
    for malformed in [
        "Dec 31 2026",
        "2026/12/31",
        "2026.12.31",
        "soon",
        "2026-13-99",
    ] {
        let drain = format!(
            r#"
[[drain_windows]]
mirror_ledger = "source-policy-mirror"
drain_owner = "repo-infra"
drain_reason = "test"
review_after = "2026-12-01"
expiry = "{malformed}"
linked_closeout = "plans/federation/closeouts/f2-evaluation.md"
"#
        );
        let config = parse_validated(&mirror_drain_config(&drain));
        assert!(
            !config.valid,
            "malformed expiry `{malformed}` should be invalid: {:?}",
            config.diagnostics
        );
        assert!(
            config.diagnostics.iter().any(|diagnostic| {
                diagnostic.kind == FederationDiagnosticKind::DrainWindowInvalidDate
                    && diagnostic.message.contains("expiry")
                    && diagnostic.message.contains(malformed)
                    && diagnostic.message.contains("YYYY-MM-DD")
            }),
            "expected drain_window_invalid_date naming expiry `{malformed}` and YYYY-MM-DD: {:?}",
            config.diagnostics
        );
    }
}

#[test]
fn validate_drain_window_rejects_malformed_review_after() {
    for malformed in ["soon", "2026.12.31", "2026/12/31", "Dec 1 2026"] {
        let drain = format!(
            r#"
[[drain_windows]]
mirror_ledger = "source-policy-mirror"
drain_owner = "repo-infra"
drain_reason = "test"
review_after = "{malformed}"
expiry = "2027-12-31"
linked_closeout = "plans/federation/closeouts/f2-evaluation.md"
"#
        );
        let config = parse_validated(&mirror_drain_config(&drain));
        assert!(
            !config.valid,
            "malformed review_after `{malformed}` should be invalid: {:?}",
            config.diagnostics
        );
        assert!(
            config.diagnostics.iter().any(|diagnostic| {
                diagnostic.kind == FederationDiagnosticKind::DrainWindowInvalidDate
                    && diagnostic.message.contains("review_after")
                    && diagnostic.message.contains(malformed)
            }),
            "expected drain_window_invalid_date naming review_after `{malformed}`: {:?}",
            config.diagnostics
        );
    }
}

#[test]
fn validate_drain_window_accepts_well_formed_dates() {
    // Valid YYYY-MM-DD dates (even an expired expiry) must not trip the
    // malformed-date diagnostic. Expired-vs-future is deadline evaluation, not
    // validation, and is covered by the existing DrainExpired behavior.
    let config = parse_validated(&mirror_drain_config(
        r#"
[[drain_windows]]
mirror_ledger = "source-policy-mirror"
drain_owner = "repo-infra"
drain_reason = "test"
review_after = "2026-12-01"
expiry = "2026-12-31"
linked_closeout = "plans/federation/closeouts/f2-evaluation.md"
"#,
    ));
    assert!(
        config.valid,
        "well-formed dates should validate (expiry being past is a deadline concern, not validation): {:?}",
        config.diagnostics
    );
    assert!(
        !config
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind == FederationDiagnosticKind::DrainWindowInvalidDate),
        "no drain_window_invalid_date for valid dates: {:?}",
        config.diagnostics
    );
}

#[test]
fn validate_federation_rejects_ledger_without_explicit_priority() {
    // #2044: a missing `priority` defaulted to the array index, so reordering
    // the [[ledgers]] array silently flipped precedence. It must be a blocking
    // parse/validation error naming the ledger instead.
    let err = parse_federation_config(
        r#"
schema_version = "1.0"

[[ledgers]]
id = "source-policy"
path = "policy/allow.toml"
dialect = "cargo-allow"
role = "canonical"
lanes = ["source-exception"]
"#,
    )
    .expect_err("ledger without explicit priority should fail to parse");
    let msg = err.to_string();
    assert!(
        msg.contains("missing required explicit `priority`"),
        "expected missing-priority error: {msg}"
    );
    assert!(
        msg.contains("source-policy"),
        "error should name the offending ledger: {msg}"
    );
}

#[test]
fn validate_federation_accepts_explicit_priorities() {
    // When every ledger has an explicit priority, the config is valid and
    // reordering the array does not change precedence.
    let config = parse_validated(
        r#"
schema_version = "1.0"

[[ledgers]]
id = "alpha"
path = "policy/alpha.toml"
dialect = "cargo-allow"
role = "canonical"
priority = 10

[[ledgers]]
id = "beta"
path = "policy/beta.toml"
dialect = "cargo-allow"
role = "canonical"
priority = 20
"#,
    );
    assert!(
        config.valid,
        "explicit priorities should validate: {:?}",
        config.diagnostics
    );
    let ordered: Vec<&str> = ordered_ledgers_by_precedence(&config.config.ledgers)
        .iter()
        .map(|ledger| ledger.id.as_str())
        .collect();
    assert_eq!(
        ordered,
        vec!["alpha", "beta"],
        "lower explicit priority wins first"
    );
}

#[test]
fn validate_federation_rejects_absolute_ledger_path() {
    // #2011: an absolute ledger path silently escapes the source-tree root on
    // `root.join` (Path::join with an absolute arg replaces the base). It must
    // be a parse error — federation ledger paths must be repo-relative.
    let err = parse_federation_config(
        r#"
schema_version = "1.0"

[[ledgers]]
id = "escape"
path = "/etc/passwd"
dialect = "cargo-allow"
role = "canonical"
priority = 10
"#,
    )
    .expect_err("absolute ledger path should fail to parse");
    let msg = err.to_string();
    assert!(
        msg.contains("/etc/passwd"),
        "error should name the offending path: {msg}"
    );
    assert!(
        msg.contains("must be relative to the repository root"),
        "error should explain the repo-relative contract: {msg}"
    );
}

#[test]
fn validate_federation_rejects_tilde_ledger_path() {
    let err = parse_federation_config(
        r#"
schema_version = "1.0"

[[ledgers]]
id = "home"
path = "~/secret/policy.toml"
dialect = "cargo-allow"
role = "canonical"
priority = 10
"#,
    )
    .expect_err("tilde ledger path should fail to parse");
    assert!(
        err.to_string().contains("~/secret/policy.toml"),
        "error should name the path: {}",
        err
    );
}

#[test]
fn validate_federation_rejects_parent_traversal_ledger_path() {
    // `..` traversal could escape the root after join; reject it.
    let err = parse_federation_config(
        r#"
schema_version = "1.0"

[[ledgers]]
id = "traversal"
path = "policy/../../escape.toml"
dialect = "cargo-allow"
role = "canonical"
priority = 10
"#,
    )
    .expect_err("parent-traversal ledger path should fail to parse");
    assert!(
        err.to_string().contains("parent directory"),
        "error should name the parent-traversal problem: {}",
        err
    );
}

#[test]
fn validate_federation_accepts_relative_ledger_path() {
    let config = parse_validated(
        r#"
schema_version = "1.0"

[[ledgers]]
id = "ok"
path = "policy/allow.toml"
dialect = "cargo-allow"
role = "canonical"
priority = 10
"#,
    );
    assert!(
        config.valid,
        "a repo-relative ledger path should validate: {:?}",
        config.diagnostics
    );
}

#[test]
fn explicit_priority_makes_precedence_independent_of_array_order() {
    // Same two ledgers, opposite declaration order, same explicit priorities:
    // the precedence winner must be identical — proving a reorder cannot flip
    // precedence (#2044's core acceptance).
    let first = r#"
schema_version = "1.0"

[[ledgers]]
id = "alpha"
path = "policy/alpha.toml"
dialect = "cargo-allow"
role = "canonical"
priority = 10

[[ledgers]]
id = "beta"
path = "policy/beta.toml"
dialect = "cargo-allow"
role = "canonical"
priority = 20
"#;
    let reordered = r#"
schema_version = "1.0"

[[ledgers]]
id = "beta"
path = "policy/beta.toml"
dialect = "cargo-allow"
role = "canonical"
priority = 20

[[ledgers]]
id = "alpha"
path = "policy/alpha.toml"
dialect = "cargo-allow"
role = "canonical"
priority = 10
"#;
    let winner_first = precedence_winner_id(first);
    let winner_reordered = precedence_winner_id(reordered);
    assert_eq!(
        winner_first, winner_reordered,
        "reordering [[ledgers]] with explicit priorities must not change the precedence winner"
    );
}

/// Parse `config` and return the id of the precedence winner (first ledger by
/// `ordered_ledgers_by_precedence`), failing the test if there is no winner.
fn precedence_winner_id(config: &str) -> String {
    let validated = parse_validated(config);
    let ordered = ordered_ledgers_by_precedence(&validated.config.ledgers);
    ordered
        .first()
        .map(|ledger| ledger.id.clone())
        .unwrap_or_else(|| std::panic::panic_any("precedence ordering produced no ledgers"))
}

/// A canonical+mirror ledger pair so a drain window for `source-policy-mirror`
/// resolves to a real mirror ledger. `drain` is appended.
fn mirror_drain_config(drain: &str) -> String {
    format!(
        r#"
schema_version = "1.0"

[[ledgers]]
id = "source-policy"
path = "policy/allow.toml"
dialect = "cargo-allow"
role = "canonical"
priority = 10

[[ledgers]]
id = "source-policy-mirror"
path = ".allow/mirror/policy.toml"
dialect = "cargo-allow"
role = "mirror"
mirrors = "source-policy"
priority = 20
{drain}
"#
    )
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
