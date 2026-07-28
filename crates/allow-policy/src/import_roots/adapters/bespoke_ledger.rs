//! Read-only importer for xtask/ripr bespoke ledger dialect (#2685, #2686).
//!
//! Maps flat selector triples, owner, reason, and optional `last_seen` onto
//! canonical `AllowEntry` values. Does not execute xtasks, scan source trees,
//! or claim full #1466 import-mode parity.

use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowResult, FindingKind, LastSeen, Lifecycle,
    Selector, normalize_path, read_text_file_capped,
};
use std::path::{Path, PathBuf};
use toml::Value;

pub const BESPOKE_LEDGER_DIALECT: &str = "xtask-ripr";
const BESPOKE_LEDGER_LINK: &str = "bespoke-ledger:xtask-ripr";

/// Returns true when `table` declares the xtask/ripr bespoke ledger dialect.
pub fn is_bespoke_ledger_dialect(table: &toml::Table) -> bool {
    table
        .get("dialect")
        .and_then(Value::as_str)
        .is_some_and(|dialect| dialect.trim() == BESPOKE_LEDGER_DIALECT)
}

/// Import a bespoke ledger TOML table into canonical policy entries.
pub fn import_bespoke_ledger_table(table: &toml::Table) -> CargoAllowResult<AllowConfig> {
    if !is_bespoke_ledger_dialect(table) {
        return Err(CargoAllowError::new(format!(
            "expected dialect `{BESPOKE_LEDGER_DIALECT}` for bespoke ledger import"
        )));
    }
    let default_kind = table
        .get("default_kind")
        .and_then(Value::as_str)
        .map(parse_finding_kind)
        .transpose()?;
    let entries = table
        .get("entries")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("bespoke ledger missing `entries` array"))?;
    let mut allow = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        allow.push(parse_bespoke_entry(index, entry, default_kind)?);
    }
    Ok(AllowConfig {
        policy: "cargo-allow".to_string(),
        allow,
        ..AllowConfig::empty()
    })
}

/// Import a bespoke ledger file when the dialect marker is present.
pub fn import_bespoke_ledger_at(path: &Path) -> CargoAllowResult<Option<AllowConfig>> {
    let text = read_text_file_capped(path).map_err(|err| {
        CargoAllowError::new(format!(
            "failed to read bespoke ledger {}: {err}",
            path.display()
        ))
    })?;
    import_bespoke_ledger_text(Some(path), &text)
}

/// Import bespoke ledger text when the dialect marker is present.
pub fn import_bespoke_ledger_text(
    path: Option<&Path>,
    text: &str,
) -> CargoAllowResult<Option<AllowConfig>> {
    let table = toml::from_str::<toml::Table>(text).map_err(|err| {
        CargoAllowError::with_kind(
            allow_core::CargoAllowErrorKind::InvalidPolicy,
            format!("failed to parse bespoke ledger TOML: {err}"),
        )
        .with_toml_span(path, text, err.span())
    })?;
    if !is_bespoke_ledger_dialect(&table) {
        return Ok(None);
    }
    import_bespoke_ledger_table(&table).map(Some)
}

fn parse_bespoke_entry(
    index: usize,
    entry: &Value,
    default_kind: Option<FindingKind>,
) -> CargoAllowResult<AllowEntry> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("bespoke ledger entry {index} is not a table"))
    })?;
    let context = string_field(table, "id").unwrap_or_else(|| format!("bespoke-{index:04}"));
    let owner = required_non_empty_field(table, "owner", &context)?;
    let reason = required_non_empty_field(table, "reason", &context)?;
    let kind = match string_field(table, "kind") {
        Some(kind) => parse_finding_kind(&kind)?,
        None => default_kind.ok_or_else(|| {
            CargoAllowError::new(format!(
                "{context} missing `kind` and no file-level `default_kind` is set"
            ))
        })?,
    };
    let path = string_field(table, "path").map(|path| normalize_path(&path));
    let family = string_field(table, "family");
    let classification =
        string_field(table, "classification").unwrap_or_else(|| "baseline_debt".to_string());
    let evidence = string_array_field(table, "evidence");
    let links = bespoke_links(table, &context);
    let lifecycle = Lifecycle {
        created: string_field(table, "created"),
        review_after: string_field(table, "review_after"),
        expires: string_field(table, "expires"),
    };
    let occurrence_limit = table
        .get("occurrence_limit")
        .and_then(Value::as_integer)
        .and_then(|value| u32::try_from(value).ok());
    let last_seen = optional_last_seen(table.get("last_seen").and_then(Value::as_table));
    let selector = selector_from_entry_table(table, path.as_deref(), kind, last_seen.as_ref())?;
    Ok(AllowEntry {
        id: context.clone(),
        kind,
        family,
        path: path.map(PathBuf::from),
        glob: string_field(table, "glob"),
        owner,
        classification,
        reason,
        evidence: bespoke_evidence(&context, index, evidence),
        links,
        occurrence_limit,
        lifecycle,
        selector,
        last_seen,
    })
}

fn selector_from_entry_table(
    table: &toml::Table,
    path: Option<&str>,
    kind: FindingKind,
    _last_seen: Option<&LastSeen>,
) -> CargoAllowResult<Selector> {
    let ast_kind = string_field(table, "selector")
        .or_else(|| string_field(table, "ast_kind"))
        .map(|kind| normalize_selector_kind(&kind))
        .ok_or_else(|| {
            CargoAllowError::new(
                "bespoke ledger entry missing selector identity (`selector` or `ast_kind`)",
            )
        })?;
    let container = string_field(table, "container");
    let callee = string_field(table, "callee");
    let macro_name = string_field(table, "macro_name");
    let lint = string_field(table, "lint");
    let symbol = string_field(table, "symbol");
    let receiver_fingerprint =
        string_field(table, "receiver_fingerprint").or_else(|| string_field(table, "receiver"));
    let target_fingerprint =
        string_field(table, "target_fingerprint").or_else(|| string_field(table, "target"));
    let normalized_snippet_hash = string_field(table, "normalized_snippet_hash");
    // line_hint is intentionally not propagated: the parser discards it (#2512)
    // and the renderer no longer emits it. Keeping it None makes it truly inert.
    let glob = string_field(table, "selector_glob").or_else(|| path.map(str::to_string));
    let selector = Selector {
        ast_kind: Some(ast_kind.clone()),
        container,
        callee: method_callee(kind, &ast_kind, callee.as_deref()),
        macro_name: macro_callee(kind, &ast_kind, macro_name.as_deref(), callee.as_deref()),
        lint,
        symbol,
        receiver_fingerprint,
        target_fingerprint,
        normalized_snippet_hash,
        line_hint: None,
        glob,
    };
    if !selector.has_structural_identity() {
        return Err(CargoAllowError::new(
            "bespoke ledger entry selector has no structural identity fields",
        ));
    }
    Ok(selector)
}

fn method_callee(kind: FindingKind, ast_kind: &str, callee: Option<&str>) -> Option<String> {
    if ast_kind != "method_call" {
        return None;
    }
    match kind {
        FindingKind::Panic => Some(normalize_panic_method_callee(callee)),
        _ => callee.map(str::to_string),
    }
}

fn macro_callee(
    kind: FindingKind,
    ast_kind: &str,
    macro_name: Option<&str>,
    callee: Option<&str>,
) -> Option<String> {
    if ast_kind != "macro_call" {
        return None;
    }
    if let Some(name) = macro_name {
        return Some(name.to_string());
    }
    if kind == FindingKind::Panic {
        return callee.map(str::to_string);
    }
    None
}

fn normalize_panic_method_callee(callee: Option<&str>) -> String {
    match callee.map(str::trim) {
        Some(callee) if callee.ends_with("unwrap") || callee.contains("::unwrap") => {
            "unwrap".to_string()
        }
        Some(callee) if callee.ends_with("expect") || callee.contains("::expect") => {
            "expect".to_string()
        }
        Some(callee) if !callee.is_empty() => callee.to_string(),
        _ => "unwrap".to_string(),
    }
}

fn normalize_selector_kind(kind: &str) -> String {
    kind.replace('-', "_")
}

fn parse_finding_kind(kind: &str) -> CargoAllowResult<FindingKind> {
    kind.parse()
}

fn bespoke_links(table: &toml::Table, id: &str) -> Vec<String> {
    let mut links = vec![BESPOKE_LEDGER_LINK.to_string(), format!("bespoke-id:{id}")];
    links.extend(string_array_field(table, "links"));
    links
}

fn bespoke_evidence(id: &str, index: usize, evidence: Vec<String>) -> Vec<String> {
    if evidence.is_empty() {
        vec![
            BESPOKE_LEDGER_LINK.to_string(),
            format!("bespoke_index:{index}"),
            format!("bespoke_id:{id}"),
        ]
    } else {
        evidence
    }
}

fn required_non_empty_field(
    table: &toml::Table,
    key: &str,
    context: &str,
) -> CargoAllowResult<String> {
    let value = string_field(table, key)
        .ok_or_else(|| CargoAllowError::new(format!("{context} missing required field `{key}`")))?;
    if value.trim().is_empty() {
        return Err(CargoAllowError::new(format!(
            "{context} field `{key}` must not be empty"
        )));
    }
    Ok(value)
}

fn optional_u32_field(table: &toml::Table, key: &str) -> Option<u32> {
    table
        .get(key)
        .and_then(Value::as_integer)
        .filter(|value| *value > 0)
        .and_then(|value| u32::try_from(value).ok())
}

fn optional_last_seen(table: Option<&toml::Table>) -> Option<LastSeen> {
    let table = table?;
    Some(LastSeen {
        line: optional_u32_field(table, "line")?,
        column: optional_u32_field(table, "column").unwrap_or(1),
    })
}

fn string_field(table: &toml::Table, key: &str) -> Option<String> {
    table.get(key).and_then(|value| match value {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Integer(number) => Some(number.to_string()),
        Value::Boolean(flag) => Some(flag.to_string()),
        _ => None,
    })
}

fn string_array_field(table: &toml::Table, key: &str) -> Vec<String> {
    table
        .get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    const SEMANTIC_FIXTURE: &str = r#"
schema_version = 1
dialect = "xtask-ripr"

[[entries]]
id = "fixture-semantic-unwrap"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "parser"
reason = "Semantic selector pins unwrap on optional after validation."
classification = "reviewed_panic_exception"
evidence = ["test:bespoke_semantic_round_trip"]
created = "2026-06-18"
review_after = "2026-12-18"
selector = "method_call"
container = "load"
callee = "unwrap"
receiver = "optional_value"
"#;

    const MISSING_OWNER_FIXTURE: &str = r#"
dialect = "xtask-ripr"

[[entries]]
id = "missing-owner"
kind = "panic"
reason = "owner is required"
selector = "method_call"
container = "load"
callee = "unwrap"
"#;

    #[test]
    fn is_bespoke_ledger_dialect_matches_marker() {
        let table = parse_table("dialect = \"xtask-ripr\"");
        assert!(is_bespoke_ledger_dialect(&table));
        let other = parse_table("dialect = \"legacy-policy\"");
        assert!(!is_bespoke_ledger_dialect(&other));
    }

    #[test]
    fn import_preserves_selector_triple_owner_and_reason() {
        let table = parse_table(SEMANTIC_FIXTURE);
        let cfg = import_bespoke_ledger_table(&table)
            .unwrap_or_else(|err| std::panic::panic_any(format!("import bespoke: {err}")));
        assert_eq!(cfg.policy, "cargo-allow");
        let [entry] = cfg.allow.as_slice() else {
            std::panic::panic_any(format!("expected one entry, got {}", cfg.allow.len()));
        };
        assert_eq!(entry.id, "fixture-semantic-unwrap");
        assert_eq!(entry.kind, FindingKind::Panic);
        assert_eq!(entry.owner, "parser");
        assert_eq!(
            entry.reason,
            "Semantic selector pins unwrap on optional after validation."
        );
        assert_eq!(entry.selector.ast_kind.as_deref(), Some("method_call"));
        assert_eq!(entry.selector.container.as_deref(), Some("load"));
        assert_eq!(entry.selector.callee.as_deref(), Some("unwrap"));
        assert_eq!(
            entry.selector.receiver_fingerprint.as_deref(),
            Some("optional_value")
        );
        assert!(entry.links.iter().any(|link| link == BESPOKE_LEDGER_LINK));
    }

    #[test]
    fn import_rejects_missing_owner() {
        let table = parse_table(MISSING_OWNER_FIXTURE);
        let err = import_bespoke_ledger_table(&table)
            .expect_err("missing owner should fail bespoke import");
        assert!(err.to_string().contains("missing required field `owner`"));
    }

    #[test]
    fn import_bespoke_ledger_text_returns_none_for_other_dialects() {
        let none = import_bespoke_ledger_text(None, "policy = \"no-panic-allowlist\"")
            .unwrap_or_else(|err| std::panic::panic_any(format!("non-bespoke probe: {err}")));
        assert!(none.is_none());
    }

    #[test]
    fn import_bespoke_ledger_at_reads_fixture_file() {
        let path = stage_fixture(
            "bespoke-ledger-semantic",
            "tests/fixtures/migration/bespoke-ledger-semantic-v1.toml",
        );
        let cfg = import_bespoke_ledger_at(&path)
            .unwrap_or_else(|err| std::panic::panic_any(format!("fixture import: {err}")))
            .unwrap_or_else(|| std::panic::panic_any("fixture should import as bespoke ledger"));
        assert_eq!(cfg.allow.len(), 1);
        let _ = fs::remove_file(&path);
    }

    const ADVISORY_DRIFT_FIXTURE: &str = r#"
schema_version = 1
dialect = "xtask-ripr"

[[entries]]
id = "fixture-bespoke-drift"
kind = "panic"
family = "unwrap"
path = "src/lib.rs"
owner = "parser"
reason = "Fixture keeps unwrap with advisory drift hints."
classification = "reviewed_panic_exception"
selector = "method_call"
container = "load"
callee = "unwrap"
receiver = "optional_value"
line_hint = 14

[entries.last_seen]
line = 14
column = 8
"#;

    #[test]
    fn import_preserves_last_seen_but_not_line_hint_for_bespoke() {
        let table = parse_table(ADVISORY_DRIFT_FIXTURE);
        let cfg = import_bespoke_ledger_table(&table).unwrap_or_else(|err| {
            std::panic::panic_any(format!("bespoke advisory drift import: {err}"))
        });

        let entry = cfg
            .allow
            .iter()
            .find(|entry| entry.id == "fixture-bespoke-drift")
            .unwrap_or_else(|| std::panic::panic_any("expected fixture-bespoke-drift entry"));

        // line_hint is intentionally not propagated from bespoke ledgers (#2512).
        assert_eq!(entry.selector.line_hint, None);
        assert_eq!(
            entry
                .last_seen
                .as_ref()
                .map(|last_seen| (last_seen.line, last_seen.column)),
            Some((14, 8))
        );
        assert_eq!(
            entry.selector.receiver_fingerprint.as_deref(),
            Some("optional_value")
        );
    }

    #[test]
    fn import_bespoke_ledger_at_reads_advisory_drift_fixture_file() {
        let path = stage_fixture(
            "bespoke-ledger-advisory-drift",
            "tests/fixtures/migration/bespoke-ledger-advisory-drift-v1.toml",
        );
        let cfg = import_bespoke_ledger_at(&path)
            .unwrap_or_else(|err| std::panic::panic_any(format!("fixture import: {err}")))
            .unwrap_or_else(|| std::panic::panic_any("fixture should import as bespoke ledger"));
        let entry = cfg
            .allow
            .iter()
            .find(|entry| entry.id == "fixture-bespoke-drift")
            .unwrap_or_else(|| std::panic::panic_any("expected fixture-bespoke-drift entry"));
        assert_eq!(
            entry
                .last_seen
                .as_ref()
                .map(|last_seen| (last_seen.line, last_seen.column)),
            Some((14, 8))
        );
        let _ = fs::remove_file(&path);
    }

    fn parse_table(input: &str) -> toml::Table {
        toml::from_str(input)
            .unwrap_or_else(|err| std::panic::panic_any(format!("test TOML parses: {err}")))
    }

    fn stage_fixture(slug: &str, fixture_rel: &str) -> PathBuf {
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../")
            .join(fixture_rel);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!(
            "cargo-allow-{slug}-{}-{stamp}.toml",
            std::process::id()
        ));
        let text = fs::read_to_string(&source).unwrap_or_else(|err| {
            std::panic::panic_any(format!("read fixture {fixture_rel}: {err}"))
        });
        fs::write(&path, text)
            .unwrap_or_else(|err| std::panic::panic_any(format!("stage fixture: {err}")));
        path
    }
}
