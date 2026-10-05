//! Remove selected array-table entries without rendering unrelated policy text.

use allow_core::{
    CargoAllowError, CargoAllowErrorKind, CargoAllowResult, POLICY_NAME, SOURCE_FILE_READ_MAX_BYTES,
};
use std::collections::BTreeSet;
use toml::de::{DeTable, DeValue};

/// Remove selected `[[allow]]` entries while preserving every other source byte.
///
/// Parser-owned header and value spans bound each removal. Inline allow arrays
/// and entries interleaved with unrelated tables refuse explicitly; this never
/// falls back to canonical rendering. The input must fit the normal loader cap.
/// A blank remainder receives only an explicit policy header, preserving its
/// whitespace and BOM while leaving a readable empty ledger.
pub fn prune_policy_entries(input: &str, ids: &[&str]) -> CargoAllowResult<String> {
    if input.len() as u64 > SOURCE_FILE_READ_MAX_BYTES {
        return Err(invalid("policy exceeds the normal loader byte limit"));
    }
    let cfg = crate::parse_policy_with_reportable_evidence(input)?;
    let requested: BTreeSet<&str> = ids.iter().copied().collect();
    if requested.len() != ids.len() {
        return Err(invalid("duplicate selected allow IDs"));
    }
    for id in &requested {
        if !cfg.allow.iter().any(|entry| entry.id == *id) {
            return Err(invalid(format!("unknown selected allow ID: {id}")));
        }
    }
    if requested.is_empty() {
        return Ok(input.to_owned());
    }
    let source = input.strip_prefix('\u{feff}').unwrap_or(input);
    let offset = input.len() - source.len();
    let document = DeTable::parse(source).map_err(|error| invalid(error.to_string()))?;
    let entries = document
        .get_ref()
        .iter()
        .find_map(|(key, value)| (key.get_ref().as_ref() == "allow").then_some(value))
        .and_then(|value| value.get_ref().as_array())
        .ok_or_else(|| invalid("allow entries have no source array"))?;
    let mut ranges = Vec::new();
    if entries.len() != cfg.allow.len() {
        return Err(invalid("allow source and policy entry counts differ"));
    }
    for (entry, policy_entry) in entries.iter().zip(&cfg.allow) {
        if entry.get_ref().as_table().is_none() {
            return Err(invalid("allow entry has no source table"));
        }
        if !requested.contains(policy_entry.id.as_str()) {
            continue;
        }
        let span = entry.span();
        let header = source
            .get(span.clone())
            .ok_or_else(|| invalid("allow header source span is invalid"))?;
        if !header.starts_with("[[") {
            return Err(invalid(
                "cannot prune inline allow arrays without rewriting policy",
            ));
        }
        let end = value_end(entry);
        let tail = source
            .get(end..)
            .ok_or_else(|| invalid("allow value source span is invalid"))?;
        // Include the last owned statement's trailing comment and line ending,
        // preserving blank lines and comments after that statement.
        let end = tail.find('\n').map_or(source.len(), |n| end + n + 1);
        ranges.push(span.start + offset..end + offset);
    }
    if ranges.len() != requested.len() {
        return Err(invalid("selected allow source spans are incomplete"));
    }
    ranges.sort_unstable_by_key(|range| range.start);
    if ranges
        .iter()
        .zip(ranges.iter().skip(1))
        .any(|(left, right)| left.end > right.start)
    {
        return Err(invalid("selected allow source spans overlap"));
    }
    let mut out = String::with_capacity(input.len());
    let mut cursor = 0;
    for range in ranges {
        out.push_str(
            input
                .get(cursor..range.start)
                .ok_or_else(|| invalid("allow removal source span is invalid"))?,
        );
        cursor = range.end;
    }
    out.push_str(
        input
            .get(cursor..)
            .ok_or_else(|| invalid("allow removal tail is invalid"))?,
    );
    let mut expected: toml::Value =
        toml::from_str(source).map_err(|error| invalid(error.to_string()))?;
    let root = expected
        .as_table_mut()
        .ok_or_else(|| invalid("policy has no root table"))?;
    let allow = root
        .get_mut("allow")
        .and_then(toml::Value::as_array_mut)
        .ok_or_else(|| invalid("policy has no allow array"))?;
    let mut index = 0;
    allow.retain(|_| {
        let keep = cfg
            .allow
            .get(index)
            .is_some_and(|entry| !requested.contains(entry.id.as_str()));
        index += 1;
        keep
    });
    if allow.is_empty() {
        root.remove("allow");
    }
    // Materialize only the already validated default policy identity when
    // removing entries would leave blank text. Requiring an independently
    // empty raw root prevents scaffolding from hiding swallowed unrelated data.
    if root.is_empty()
        && out
            .strip_prefix('\u{feff}')
            .unwrap_or(&out)
            .trim()
            .is_empty()
    {
        out.push_str(&format!("policy = \"{POLICY_NAME}\"\n"));
        root.insert(
            "policy".to_owned(),
            toml::Value::String(POLICY_NAME.to_owned()),
        );
    }
    let actual_cfg = crate::parse_policy(&out).map_err(|error| {
        error.with_message_prefix("cannot prune selected entries without rewriting policy: ")
    })?;
    let mut expected_cfg = cfg.clone();
    expected_cfg
        .allow
        .retain(|entry| !requested.contains(entry.id.as_str()));
    if crate::render_policy(&actual_cfg) != crate::render_policy(&expected_cfg) {
        return Err(invalid(
            "pruning would change surviving entry identities; assign explicit IDs before retrying",
        ));
    }
    let actual: toml::Value = toml::from_str(out.strip_prefix('\u{feff}').unwrap_or(&out))
        .map_err(|error| invalid(error.to_string()))?;
    if actual != expected {
        return Err(invalid(
            "selected entries are interleaved with unrelated policy data",
        ));
    }
    Ok(out)
}

fn value_end(value: &toml::Spanned<DeValue<'_>>) -> usize {
    let nested_end = match value.get_ref() {
        DeValue::Table(table) => table.values().map(value_end).max().unwrap_or(0),
        DeValue::Array(array) => array.iter().map(value_end).max().unwrap_or(0),
        _ => 0,
    };
    value.span().end.max(nested_end)
}

fn invalid(message: impl Into<String>) -> CargoAllowError {
    CargoAllowError::with_kind(CargoAllowErrorKind::InvalidPolicy, message)
}

#[cfg(test)]
#[path = "prune_text_tests.rs"]
mod tests;
