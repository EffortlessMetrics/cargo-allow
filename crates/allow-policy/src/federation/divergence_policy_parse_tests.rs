use super::divergence::detect_mirror_divergences;
use super::{parse_federation_config, validate_federation_config};
use crate::load_policy;
use std::fs;

#[test]
fn mirror_divergence_fixture_policies_load() {
    let root =
        std::env::temp_dir().join(format!("cargo-allow-mirror-parse-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("policy")).unwrap_or_else(|err| {
        std::panic::panic_any(format!("policy dir: {err}"));
    });
    fs::create_dir_all(root.join(".allow/mirror")).unwrap_or_else(|err| {
        std::panic::panic_any(format!("mirror dir: {err}"));
    });
    fs::copy(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/federation/canonical-mirror-drain-config.toml"),
        root.join(".allow/config.toml"),
    )
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!("copy canonical-mirror-drain-config.toml: {err}"));
    });
    let canonical = r#"schema_version = "0.1"
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
"#;
    fs::write(root.join("policy/allow.toml"), canonical).unwrap_or_else(|err| {
        std::panic::panic_any(format!("canonical policy write: {err}"));
    });
    fs::write(
        root.join(".allow/mirror/policy.toml"),
        "schema_version = \"0.1\"\n",
    )
    .unwrap_or_else(|err| {
        std::panic::panic_any(format!("mirror policy write: {err}"));
    });
    load_policy(root.join("policy/allow.toml")).unwrap_or_else(|err| {
        std::panic::panic_any(format!("canonical load failed: {err}"));
    });
    load_policy(root.join(".allow/mirror/policy.toml")).unwrap_or_else(|err| {
        std::panic::panic_any(format!("mirror load failed: {err}"));
    });
    let text = fs::read_to_string(root.join(".allow/config.toml")).unwrap_or_else(|err| {
        std::panic::panic_any(format!("read config: {err}"));
    });
    let config = validate_federation_config(parse_federation_config(&text).unwrap_or_else(|err| {
        std::panic::panic_any(format!("parse federation config: {err}"));
    }));
    let divergences = detect_mirror_divergences(&root, &config.config).unwrap_or_else(|err| {
        std::panic::panic_any(format!("detect divergences: {err}"));
    });
    assert!(
        divergences.iter().any(|record| {
            record.kind == super::divergence::FederationDivergenceKind::MirrorDivergence
                && record
                    .sample_entry_ids
                    .contains(&"canonical-only".to_string())
        }),
        "divergences: {divergences:?}"
    );
    let _ = fs::remove_dir_all(root);
}
