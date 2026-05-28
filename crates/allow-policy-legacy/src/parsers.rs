use allow_core::{CargoAllowError, CargoAllowResult};
use toml::Value;

use crate::fields::{
    legacy_evidence, optional_last_seen, optional_u32_field, raw_string_field, required_bool_field,
    required_string_array_field, required_string_field, string_array_field, string_field,
};
pub(crate) use crate::parser_support::is_clippy_exceptions_policy;
use crate::parser_support::{
    has_glob_meta, normalize_legacy_expires, normalize_lint_attribute_family,
    normalize_unsafe_family,
};
use crate::types::{
    LegacyClippyRule, LegacyDependencySurfaceRule, LegacyExecutableRule, LegacyGeneratedRule,
    LegacyNetworkRule, LegacyNoPanicAllowEntry, LegacyNoPanicBaselineEntry, LegacyNonRustRule,
    LegacyProcessRule, LegacyUnsafeRule, LegacyWorkflowRule,
};
use crate::{default_baseline_created, default_baseline_expires};

pub(crate) fn parse_non_rust_rules(
    table: &toml::Table,
) -> CargoAllowResult<Vec<LegacyNonRustRule>> {
    let entries = table
        .get("allow")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("non-rust-allowlist missing allow entries"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_non_rust_rule(index, entry))
        .collect()
}

fn parse_non_rust_rule(index: usize, entry: &Value) -> CargoAllowResult<LegacyNonRustRule> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("non-rust allow entry {index} is not a table"))
    })?;
    let id = string_field(table, "id").unwrap_or_else(|| format!("legacy-non-rust-{index:04}"));
    let (pattern, is_path) = match (string_field(table, "path"), string_field(table, "glob")) {
        (Some(path), None) => (path, true),
        (None, Some(glob)) => (glob, false),
        (Some(path), Some(_)) => (path, true),
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
    Ok(LegacyNonRustRule {
        id: id.clone(),
        pattern,
        is_path,
        owner: string_field(table, "owner").unwrap_or_default(),
        classification: string_field(table, "category")
            .unwrap_or_else(|| "legacy_non_rust".to_string()),
        reason,
        created: string_field(table, "created"),
        review_after: string_field(table, "review_after"),
        expires: normalize_legacy_expires(string_field(table, "expires")),
    })
}

fn is_broad_legacy_glob(pattern: &str) -> bool {
    pattern.contains('*')
}

pub(crate) fn parse_generated_rules(
    table: &toml::Table,
) -> CargoAllowResult<Vec<LegacyGeneratedRule>> {
    let entries = table
        .get("allow")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("generated-allowlist missing allow entries"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_generated_rule(index, entry))
        .collect()
}

fn parse_generated_rule(index: usize, entry: &Value) -> CargoAllowResult<LegacyGeneratedRule> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("generated allow entry {index} is not a table"))
    })?;
    let id = string_field(table, "id").unwrap_or_else(|| format!("legacy-generated-{index:04}"));
    let path = string_field(table, "path")
        .ok_or_else(|| CargoAllowError::new(format!("{id} missing path")))?;
    Ok(LegacyGeneratedRule {
        id,
        path,
        owner: string_field(table, "owner").unwrap_or_default(),
        reason: string_field(table, "reason").unwrap_or_default(),
        generator: string_field(table, "generator"),
        regenerate_command: string_field(table, "regenerate_command"),
        created: string_field(table, "created"),
        expires: normalize_legacy_expires(string_field(table, "expires")),
    })
}

pub(crate) fn parse_no_panic_baseline_entries(
    table: &toml::Table,
) -> CargoAllowResult<Vec<LegacyNoPanicBaselineEntry>> {
    let entries = table
        .get("entry")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("no-panic-baseline missing entry records"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_no_panic_baseline_entry(index, entry))
        .collect()
}

fn parse_no_panic_baseline_entry(
    index: usize,
    entry: &Value,
) -> CargoAllowResult<LegacyNoPanicBaselineEntry> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("no-panic baseline entry {index} is not a table"))
    })?;
    let context = format!("no-panic baseline entry {index}");
    let count = table
        .get("count")
        .and_then(Value::as_integer)
        .filter(|value| *value > 0)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| CargoAllowError::new(format!("{context} missing count")))?;
    Ok(LegacyNoPanicBaselineEntry {
        index,
        path: required_string_field(table, "path", &context)?,
        family: required_string_field(table, "family", &context)?,
        selector_kind: required_string_field(table, "selector_kind", &context)?,
        selector_callee: required_string_field(table, "selector_callee", &context)?,
        snippet: required_string_field(table, "snippet", &context)?,
        count,
    })
}

pub(crate) fn parse_no_panic_allowlist_entries(
    table: &toml::Table,
) -> CargoAllowResult<Vec<LegacyNoPanicAllowEntry>> {
    let entries = table
        .get("allow")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("no-panic-allowlist missing allow entries"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_no_panic_allowlist_entry(index, entry))
        .collect()
}

fn parse_no_panic_allowlist_entry(
    index: usize,
    entry: &Value,
) -> CargoAllowResult<LegacyNoPanicAllowEntry> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("no-panic allow entry {index} is not a table"))
    })?;
    let id = string_field(table, "id").unwrap_or_else(|| format!("legacy-no-panic-{index:04}"));
    let selector = table.get("selector").and_then(Value::as_table);
    let last_seen_table = table.get("last_seen").and_then(Value::as_table);
    let review_after = string_field(table, "review_after");
    let expires = normalize_legacy_expires(string_field(table, "expires"))
        .or_else(|| review_after.is_none().then(default_baseline_expires));
    let last_seen = optional_last_seen(last_seen_table);
    Ok(LegacyNoPanicAllowEntry {
        index,
        id: id.clone(),
        path: required_string_field(table, "path", &id)?,
        family: required_string_field(table, "family", &id)?,
        selector_kind: selector
            .and_then(|selector| {
                string_field(selector, "kind").or_else(|| string_field(selector, "ast_kind"))
            })
            .ok_or_else(|| CargoAllowError::new(format!("{id} missing selector.kind")))?,
        selector_callee: selector.and_then(|selector| string_field(selector, "callee")),
        selector_container: selector.and_then(|selector| string_field(selector, "container")),
        owner: string_field(table, "owner").unwrap_or_else(|| "unowned".to_string()),
        classification: string_field(table, "classification")
            .unwrap_or_else(|| "baseline_debt".to_string()),
        reason: string_field(table, "reason")
            .or_else(|| string_field(table, "explanation"))
            .unwrap_or_else(|| {
                "Generated from legacy no-panic allowlist; requires human review.".to_string()
            }),
        created: string_field(table, "created").or_else(|| Some(default_baseline_created())),
        review_after,
        expires,
        line_hint: selector
            .and_then(|selector| optional_u32_field(selector, "line_hint"))
            .or_else(|| last_seen.as_ref().map(|seen| seen.line)),
        last_seen,
    })
}

pub(crate) fn parse_clippy_rules(table: &toml::Table) -> CargoAllowResult<Vec<LegacyClippyRule>> {
    let entries = table
        .get("allow")
        .or_else(|| table.get("entry"))
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("clippy-exceptions missing allow entries"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_clippy_rule(index, entry))
        .collect()
}

fn parse_clippy_rule(index: usize, entry: &Value) -> CargoAllowResult<LegacyClippyRule> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("clippy exception entry {index} is not a table"))
    })?;
    let id = string_field(table, "id").unwrap_or_else(|| format!("legacy-clippy-{index:04}"));
    let review_after = string_field(table, "review_after");
    let expires = normalize_legacy_expires(string_field(table, "expires"))
        .or_else(|| review_after.is_none().then(default_baseline_expires));
    Ok(LegacyClippyRule {
        path: required_string_field(table, "path", &id)?,
        lint: required_string_field(table, "lint", &id)?,
        family: string_field(table, "family")
            .or_else(|| string_field(table, "attribute"))
            .map(|family| normalize_lint_attribute_family(&family))
            .unwrap_or_else(|| "expect_attribute".to_string()),
        owner: string_field(table, "owner").unwrap_or_else(|| "unowned".to_string()),
        classification: string_field(table, "classification")
            .unwrap_or_else(|| "baseline_debt".to_string()),
        reason: string_field(table, "reason").unwrap_or_else(|| {
            "Generated from legacy Clippy exceptions policy; requires human review.".to_string()
        }),
        symbol: string_field(table, "symbol"),
        target_fingerprint: string_field(table, "target_fingerprint")
            .or_else(|| string_field(table, "policy_id").map(|id| format!("policy:{id}"))),
        created: string_field(table, "created").or_else(|| Some(default_baseline_created())),
        review_after,
        expires,
        id,
    })
}

pub(crate) fn parse_unsafe_rules(table: &toml::Table) -> CargoAllowResult<Vec<LegacyUnsafeRule>> {
    let entries = table
        .get("allow")
        .or_else(|| table.get("entry"))
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("unsafe-allowlist missing allow entries"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_unsafe_rule(index, entry))
        .collect()
}

fn parse_unsafe_rule(index: usize, entry: &Value) -> CargoAllowResult<LegacyUnsafeRule> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("unsafe allow entry {index} is not a table"))
    })?;
    let id = string_field(table, "id").unwrap_or_else(|| format!("legacy-unsafe-{index:04}"));
    let selector = table.get("selector").and_then(Value::as_table);
    let last_seen_table = table.get("last_seen").and_then(Value::as_table);
    let review_after = string_field(table, "review_after");
    let expires = normalize_legacy_expires(string_field(table, "expires"))
        .or_else(|| review_after.is_none().then(default_baseline_expires));
    let family = string_field(table, "family")
        .or_else(|| {
            selector.and_then(|selector| {
                string_field(selector, "kind").or_else(|| string_field(selector, "ast_kind"))
            })
        })
        .ok_or_else(|| CargoAllowError::new(format!("{id} missing family or selector.kind")))?;
    let family = normalize_unsafe_family(&family);
    let selector_kind = selector
        .and_then(|selector| {
            string_field(selector, "kind").or_else(|| string_field(selector, "ast_kind"))
        })
        .map(|kind| normalize_unsafe_family(&kind))
        .unwrap_or_else(|| family.clone());
    let last_seen = optional_last_seen(last_seen_table);
    Ok(LegacyUnsafeRule {
        id: id.clone(),
        path: required_string_field(table, "path", &id)?,
        family,
        selector_kind,
        selector_container: selector.and_then(|selector| string_field(selector, "container")),
        owner: string_field(table, "owner").unwrap_or_else(|| "unowned".to_string()),
        classification: string_field(table, "classification")
            .unwrap_or_else(|| "baseline_debt".to_string()),
        reason: string_field(table, "reason")
            .or_else(|| string_field(table, "explanation"))
            .unwrap_or_else(|| {
                "Generated from legacy unsafe allowlist; requires human review.".to_string()
            }),
        evidence: legacy_evidence(table),
        created: string_field(table, "created").or_else(|| Some(default_baseline_created())),
        review_after,
        expires,
        line_hint: selector
            .and_then(|selector| optional_u32_field(selector, "line_hint"))
            .or_else(|| last_seen.as_ref().map(|seen| seen.line)),
        last_seen,
    })
}

pub(crate) fn parse_executable_rules(
    table: &toml::Table,
) -> CargoAllowResult<Vec<LegacyExecutableRule>> {
    let entries = table
        .get("allow")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("executable-allowlist missing allow entries"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_executable_rule(index, entry))
        .collect()
}

fn parse_executable_rule(index: usize, entry: &Value) -> CargoAllowResult<LegacyExecutableRule> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("executable allow entry {index} is not a table"))
    })?;
    let id = string_field(table, "id").unwrap_or_else(|| format!("legacy-executable-{index:04}"));
    let path = string_field(table, "path")
        .ok_or_else(|| CargoAllowError::new(format!("{id} missing path")))?;
    Ok(LegacyExecutableRule {
        id,
        path,
        owner: string_field(table, "owner").unwrap_or_default(),
        reason: string_field(table, "reason").unwrap_or_default(),
        interpreter: string_field(table, "interpreter"),
        created: string_field(table, "created"),
        review_after: string_field(table, "review_after"),
        expires: normalize_legacy_expires(string_field(table, "expires")),
    })
}

pub(crate) fn parse_workflow_rules(
    table: &toml::Table,
) -> CargoAllowResult<Vec<LegacyWorkflowRule>> {
    let entries = table
        .get("entry")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("workflow-allowlist missing entry records"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_workflow_rule(index, entry))
        .collect()
}

fn parse_workflow_rule(index: usize, entry: &Value) -> CargoAllowResult<LegacyWorkflowRule> {
    let table = entry
        .as_table()
        .ok_or_else(|| CargoAllowError::new(format!("workflow entry {index} is not a table")))?;
    let path = string_field(table, "path")
        .ok_or_else(|| CargoAllowError::new(format!("workflow entry {index} missing path")))?;
    Ok(LegacyWorkflowRule {
        path,
        owner: string_field(table, "owner").unwrap_or_default(),
        reason: string_field(table, "reason").unwrap_or_default(),
        permissions: string_array_field(table, "permissions"),
        secrets_used: string_array_field(table, "secrets_used"),
        external_actions: string_array_field(table, "external_actions"),
        duplicate_of_lane: string_field(table, "duplicate_of_lane"),
        created: string_field(table, "created"),
        review_after: string_field(table, "review_after"),
        expires: normalize_legacy_expires(string_field(table, "expires")),
    })
}

pub(crate) fn parse_dependency_surface_rules(
    table: &toml::Table,
) -> CargoAllowResult<Vec<LegacyDependencySurfaceRule>> {
    let entries = table
        .get("allow")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            CargoAllowError::new("dependency-surface-allowlist missing allow entries")
        })?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_dependency_surface_rule(index, entry))
        .collect()
}

fn parse_dependency_surface_rule(
    index: usize,
    entry: &Value,
) -> CargoAllowResult<LegacyDependencySurfaceRule> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!(
            "dependency-surface allow entry {index} is not a table"
        ))
    })?;
    let id = string_field(table, "id").unwrap_or_else(|| format!("legacy-dependency-{index:04}"));
    let pattern = string_field(table, "path")
        .or_else(|| string_field(table, "glob"))
        .ok_or_else(|| CargoAllowError::new(format!("{id} missing path or glob")))?;
    Ok(LegacyDependencySurfaceRule {
        id,
        is_glob: has_glob_meta(&pattern),
        pattern,
        surface: string_field(table, "surface").unwrap_or_else(|| "dependency_surface".to_string()),
        owner: string_field(table, "owner").unwrap_or_default(),
        reason: string_field(table, "reason").unwrap_or_default(),
        broad_glob_reason: string_field(table, "broad_glob_reason"),
        dep_count_at_baseline: table
            .get("dep_count_at_baseline")
            .and_then(Value::as_integer),
        created: string_field(table, "created"),
        review_after: string_field(table, "review_after"),
        expires: normalize_legacy_expires(string_field(table, "expires")),
    })
}

pub(crate) fn parse_process_rules(table: &toml::Table) -> CargoAllowResult<Vec<LegacyProcessRule>> {
    let entries = table
        .get("allow")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("process-allowlist missing allow entries"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_process_rule(index, entry))
        .collect()
}

fn parse_process_rule(index: usize, entry: &Value) -> CargoAllowResult<LegacyProcessRule> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("process allow entry {index} is not a table"))
    })?;
    let id = required_string_field(table, "id", &format!("process allow entry {index}"))?;
    Ok(LegacyProcessRule {
        binary: required_string_field(table, "binary", &id)?,
        argv_shape: required_string_array_field(table, "argv_shape", &id)?,
        network_reach: required_bool_field(table, "network_reach", &id)?,
        called_by: string_array_field(table, "called_by"),
        owner: required_string_field(table, "owner", &id)?,
        reason: required_string_field(table, "reason", &id)?,
        created: Some(required_string_field(table, "created", &id)?),
        review_after: string_field(table, "review_after"),
        expires: normalize_legacy_expires(string_field(table, "expires")),
        id,
    })
}

pub(crate) fn parse_network_rules(table: &toml::Table) -> CargoAllowResult<Vec<LegacyNetworkRule>> {
    let entries = table
        .get("allow")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("network-allowlist missing allow entries"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_network_rule(index, entry))
        .collect()
}

fn parse_network_rule(index: usize, entry: &Value) -> CargoAllowResult<LegacyNetworkRule> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("network allow entry {index} is not a table"))
    })?;
    let id = required_string_field(table, "id", &format!("network allow entry {index}"))?;
    Ok(LegacyNetworkRule {
        destination: required_string_field(table, "destination", &id)?,
        auth_required: required_bool_field(table, "auth_required", &id)?,
        auth_secret: string_field(table, "auth_secret"),
        lane: required_string_field(table, "lane", &id)?,
        owner: required_string_field(table, "owner", &id)?,
        reason: required_string_field(table, "reason", &id)?,
        created: Some(required_string_field(table, "created", &id)?),
        review_after: string_field(table, "review_after"),
        expires: normalize_legacy_expires(string_field(table, "expires")),
        id,
    })
}
