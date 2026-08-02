use super::*;

#[test]
fn parses_policy_with_allow() {
    let cfg = parse_policy(
        r#"
                schema_version = "0.1"
                policy = "cargo-allow"

                [requirements]
                expires_or_review_after_required = true
                lint_policy_id_required = true

                [[allow]]
                id = "allow-0001"
                kind = "panic"
                family = "unwrap"
                path = "src/lib.rs"
                owner = "core"
                classification = "test"
                reason = "fixture"
                expires = "2026-08-01"

                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
                container = "load"
            "#,
    )
    .expect("policy parses");
    assert_eq!(cfg.allow.len(), 1);
    assert!(cfg.requirements.lint_policy_id_required);
    assert_eq!(cfg.allow[0].selector.callee.as_deref(), Some("unwrap"));
}

#[test]
fn parses_unsafe_safety_comment_requirement() {
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"

                [requirements.unsafe]
                safety_comment_required = true

                [[allow]]
                id = "allow-unsafe"
                kind = "unsafe"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = ["test:unsafe_boundary"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "unsafe_block"
                container = "load"
            "#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy should parse: {err}")));

    assert!(cfg.requirements.unsafe_safety_comment_required);
}

#[test]
fn parses_general_evidence_requirement() {
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"

                [requirements]
                evidence_required = true
            "#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy should parse: {err}")));

    assert!(cfg.requirements.evidence_required);
}

#[test]
fn parses_legacy_aliases_and_scalar_arrays() {
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"

                [workspace]
                ignored = ".git/**"

                [requirements]
                owner_required = "true"

                [[allow]]
                id = "allow-legacy"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "legacy"
                explanation = "legacy reason field"
                covered_by = "test:legacy"
                count = 2
                expires = "2026-08-01"

                [allow.selector]
                ast_kind = "macro_call"
                macro_name = "panic"
                line_hint = "12"
            "#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("legacy aliases parse: {err}")));

    assert_eq!(cfg.workspace.ignored, vec![".git/**"]);
    let entry = cfg
        .allow
        .first()
        .unwrap_or_else(|| std::panic::panic_any("expected one allow entry"));
    assert_eq!(entry.reason, "legacy reason field");
    assert_eq!(entry.evidence, vec!["test:legacy"]);
    assert_eq!(entry.occurrence_limit, Some(2));
    assert_eq!(entry.selector.ast_kind.as_deref(), Some("macro_call"));
    assert_eq!(entry.selector.macro_name.as_deref(), Some("panic"));
    assert_eq!(entry.selector.line_hint, None);
}

#[test]
fn reports_toml_parse_errors() {
    let err = parse_policy("policy = [").unwrap_err();

    assert!(err.to_string().contains("failed to parse policy TOML"));
}

#[test]
fn parses_current_repository_policy() {
    let cfg = parse_policy(include_str!("../../../policy/allow.toml"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("repo policy parses: {err}")));

    assert_eq!(cfg.policy, "cargo-allow");
    assert!(cfg.allow.iter().any(|entry| entry.id == "allow-0076"));
    assert!(cfg.allow.iter().any(|entry| entry.id == "allow-0088"));
    for removed in [
        "allow-0001",
        "allow-0002",
        "allow-0003",
        "allow-0004",
        "allow-0005",
        "allow-0006",
        "allow-0007",
        "allow-0008",
        "allow-0009",
        "allow-0011",
        "allow-0012",
        "allow-0013",
        "allow-0014",
        "allow-0015",
        "allow-0016",
        "allow-0017",
        "allow-0018",
        "allow-0019",
        "allow-0020",
        "allow-0031",
        "allow-0032",
        "allow-0033",
        "allow-0039",
        "allow-0041",
        "allow-0042",
        "allow-0043",
        "allow-0044",
        "allow-0045",
        "allow-0046",
        "allow-0047",
        "allow-0048",
        "allow-0049",
        "allow-0050",
        "allow-0051",
        "allow-0052",
        "allow-0053",
        "allow-0054",
        "allow-0055",
        "allow-0056",
        "allow-0057",
        "allow-0058",
        "allow-0059",
        "allow-0060",
        "allow-0061",
        "allow-0062",
        "allow-0063",
        "allow-0064",
        "allow-0065",
        "allow-0066",
    ] {
        assert!(
            !cfg.allow.iter().any(|entry| entry.id == removed),
            "{removed} should stay pruned from the repository policy"
        );
    }
}

#[test]
fn find_config_walks_up_to_nearest_supported_policy_file() -> std::io::Result<()> {
    let root = TempRoot::new("find-config-nearest")?;
    let workspace = root.path().join("workspace");
    let start = workspace.join("member/src");
    let root_policy = root.path().join("policy/allow.toml");
    let workspace_policy = workspace.join(".cargo/allow.toml");
    std::fs::create_dir_all(&start)?;
    write_fixture_file(&root_policy)?;
    write_fixture_file(&workspace_policy)?;

    let found = find_config(&start).unwrap_or_else(|| {
        std::panic::panic_any(format!("expected config for {}", start.display()))
    });

    assert_eq!(found.canonicalize()?, workspace_policy.canonicalize()?);
    Ok(())
}

#[test]
fn find_config_returns_none_when_no_supported_policy_file_exists() -> std::io::Result<()> {
    let root = TempRoot::new("find-config-none")?;
    let start = root.path().join("member/src");
    std::fs::create_dir_all(&start)?;

    assert_eq!(find_config(&start), None);
    Ok(())
}

#[test]
fn load_policy_with_reportable_evidence_reads_file_and_keeps_invalid_links() -> std::io::Result<()>
{
    let root = TempRoot::new("load-reportable-evidence")?;
    let policy_path = root.path().join("policy/allow.toml");
    write_fixture_file_with_contents(
        &policy_path,
        r#"
                policy = "cargo-allow"

                [[allow]]
                id = "allow-invalid-link"
                kind = "unsafe"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = ["test:load_reportable"]
                links = ["doc:docs/./safety.md"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "unsafe_block"
                container = "load"
            "#,
    )?;

    let cfg = load_policy_with_reportable_evidence(&policy_path).unwrap_or_else(|err| {
        std::panic::panic_any(format!("reportable policy load should parse: {err}"))
    });

    assert_eq!(cfg.allow.len(), 1);
    assert_eq!(cfg.allow[0].id, "allow-invalid-link");
    assert_eq!(cfg.allow[0].links, vec!["doc:docs/./safety.md"]);
    Ok(())
}

#[test]
fn load_policy_reports_path_when_read_fails() -> std::io::Result<()> {
    let root = TempRoot::new("load-policy-read-error")?;
    let policy_path = root.path().join("missing/allow.toml");

    let error = load_policy(&policy_path).expect_err("missing policy should fail to read");
    let message = error.to_string();
    assert!(message.contains("failed to read"));
    assert!(message.contains(&policy_path.display().to_string()));
    Ok(())
}

#[test]
fn load_policy_rejects_oversized_policy_files() -> std::io::Result<()> {
    let root = TempRoot::new("load-policy-oversized")?;
    let policy_path = root.path().join("policy/allow.toml");
    if let Some(parent) = policy_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let oversized_len = (allow_core::SOURCE_FILE_READ_MAX_BYTES as usize).saturating_add(1);
    let mut bytes = Vec::with_capacity(oversized_len);
    bytes.extend_from_slice(b"policy = \"cargo-allow\"\n#");
    bytes.resize(oversized_len, b'x');
    std::fs::write(&policy_path, bytes)?;

    let err = load_policy(&policy_path).expect_err("oversized policy should fail closed");
    let message = err.to_string();
    assert!(
        message.contains("source-read limit") || message.contains("exceeds"),
        "expected size-limit diagnostic, got: {message}"
    );
    Ok(())
}

#[test]
fn load_federation_config_rejects_oversized_config() -> std::io::Result<()> {
    let root = TempRoot::new("federation-oversized")?;
    let config_path = root.path().join(".allow/config.toml");
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let oversized_len = (allow_core::SOURCE_FILE_READ_MAX_BYTES as usize).saturating_add(1);
    let mut bytes = Vec::with_capacity(oversized_len);
    bytes.extend_from_slice(b"schema_version = \"0.1\"\n#");
    bytes.resize(oversized_len, b'y');
    std::fs::write(&config_path, bytes)?;

    let err = load_federation_config(root.path())
        .expect_err("oversized federation config should fail closed");
    let message = err.to_string();
    assert!(
        message.contains("source-read limit") || message.contains("exceeds"),
        "expected size-limit diagnostic, got: {message}"
    );
    Ok(())
}

#[test]
fn load_federation_config_is_valid_distinguishes_parsed_from_valid() -> std::io::Result<()> {
    // #1837: found()/parsed() returns true when the file was found and parsed,
    // regardless of validation. is_valid() returns true only when validation
    // also passed. A config with blocking diagnostics (DuplicateId) should
    // parse but fail is_valid().
    let root = TempRoot::new("federation-invalid")?;
    let config_path = root.path().join(".allow/config.toml");
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Duplicate ledger ids — a blocking diagnostic.
    std::fs::write(
        &config_path,
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
path = ".allow/mirror/policy.toml"
dialect = "cargo-allow"
role = "mirror"
mirrors = "dup"
priority = 20
"#,
    )?;

    let loaded = load_federation_config(root.path()).unwrap_or_else(|err| {
        std::panic::panic_any(format!("invalid config should still parse: {err}"))
    });
    assert!(loaded.parsed(), "parsed() should be true (file was found)");
    assert!(
        !loaded.is_valid(),
        "is_valid() should be false (DuplicateId is blocking): {:?}",
        loaded.validated().map(|v| &v.diagnostics)
    );
    let validated = loaded.validated().unwrap_or_else(|| {
        std::panic::panic_any("validated() should return the parsed config even when invalid")
    });
    assert!(!validated.valid, "validated.valid should be false");
    assert!(
        !validated.diagnostics.is_empty(),
        "diagnostics should be non-empty for DuplicateId"
    );

    Ok(())
}

struct TempRoot {
    path: std::path::PathBuf,
}

impl TempRoot {
    fn new(label: &str) -> std::io::Result<Self> {
        let unique = format!(
            "cargo-allow-policy-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_else(|err| {
                    std::panic::panic_any(format!("system time before epoch: {err}"))
                })
                .as_nanos()
        );
        let path = std::env::temp_dir().join(unique);
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn write_fixture_file(path: &std::path::Path) -> std::io::Result<()> {
    write_fixture_file_with_contents(path, "policy = \"cargo-allow\"\n")
}

fn write_fixture_file_with_contents(path: &std::path::Path, contents: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents)
}
