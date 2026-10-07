//! Refresh selected coordinate values without rendering unrelated policy text.

use allow_core::{
    CargoAllowError, CargoAllowErrorKind, CargoAllowResult, LastSeen, SOURCE_FILE_READ_MAX_BYTES,
};
use toml::de::{DeTable, DeValue};

/// Replace only the selected entry's existing `last_seen` coordinate values.
///
/// Array-table entries and explicit last_seen tables have parser-owned spans.
/// Inline/dotted tables and escaped or multiline numeric strings refuse rather
/// than falling back to rendering. Legacy selector.line_hint remains untouched.
pub fn refresh_policy_entry(
    input: &str,
    id: &str,
    location: &LastSeen,
) -> CargoAllowResult<String> {
    if input.len() as u64 > SOURCE_FILE_READ_MAX_BYTES {
        return Err(invalid("policy exceeds the normal loader byte limit"));
    }
    let mut expected_cfg = crate::parse_policy_with_reportable_evidence(input)?;
    let index = expected_cfg
        .allow
        .iter()
        .position(|entry| entry.id == id)
        .ok_or_else(|| invalid(format!("unknown selected allow ID: {id}")))?;
    let policy_entry = expected_cfg
        .allow
        .get_mut(index)
        .ok_or_else(|| invalid("selected policy entry is missing"))?;
    if policy_entry.last_seen.is_none() {
        return Err(invalid(
            "selected entry has no existing last_seen coordinates",
        ));
    }
    policy_entry.last_seen = Some(location.clone());

    let source = input.strip_prefix('\u{feff}').unwrap_or(input);
    let offset = input.len() - source.len();
    let document = DeTable::parse(source).map_err(|error| invalid(error.to_string()))?;
    let entries = document
        .get_ref()
        .iter()
        .find_map(|(key, value)| (key.get_ref().as_ref() == "allow").then_some(value))
        .and_then(|value| value.get_ref().as_array())
        .ok_or_else(|| invalid("allow entries have no source array"))?;
    if entries.len() != expected_cfg.allow.len() {
        return Err(invalid("allow source and policy entry counts differ"));
    }
    let entry = entries
        .get(index)
        .ok_or_else(|| invalid("selected source entry is missing"))?;
    if !source
        .get(entry.span())
        .is_some_and(|header| header.starts_with("[["))
    {
        return Err(invalid(
            "cannot refresh inline allow arrays without rewriting policy",
        ));
    }
    let last_seen = field(entry, "last_seen")?;
    if !source
        .get(last_seen.span())
        .is_some_and(|header| header.starts_with('['))
    {
        return Err(invalid(
            "cannot refresh inline or dotted last_seen tables without rewriting policy",
        ));
    }

    // Independently compute the intended raw change: keep all root/entry data,
    // omitted defaults, inert legacy hints and integer/string representation.
    let mut expected: toml::Value =
        toml::from_str(source).map_err(|error| invalid(error.to_string()))?;
    let raw_location = expected
        .get_mut("allow")
        .and_then(toml::Value::as_array_mut)
        .and_then(|entries| entries.get_mut(index))
        .and_then(|entry| entry.get_mut("last_seen"))
        .ok_or_else(|| invalid("selected raw last_seen table is missing"))?;
    let mut patches = Vec::new();
    for (key, coordinate) in [("line", location.line), ("column", location.column)] {
        let value = field(last_seen, key)?;
        let span = value.span();
        let token = source
            .get(span.clone())
            .ok_or_else(|| invalid("coordinate source span is invalid"))?;
        let raw_value = raw_location
            .get_mut(key)
            .ok_or_else(|| invalid("selected raw coordinate is missing"))?;
        let replacement = match raw_value {
            toml::Value::Integer(old) => {
                if *old == i64::from(coordinate) {
                    continue;
                }
                *old = i64::from(coordinate);
                coordinate.to_string()
            }
            toml::Value::String(old) => {
                let quote = if token.starts_with('"') { '"' } else { '\'' };
                let digits = token
                    .strip_prefix(quote)
                    .and_then(|token| token.strip_suffix(quote))
                    .filter(|digits| digits.parse::<u32>().is_ok())
                    .ok_or_else(|| invalid("unsupported numeric string coordinate source"))?;
                if digits != old.as_str() {
                    return Err(invalid(
                        "numeric string source differs from parsed coordinate",
                    ));
                }
                if old.parse::<u32>().ok() == Some(coordinate) {
                    continue;
                }
                *old = coordinate.to_string();
                format!("{quote}{coordinate}{quote}")
            }
            _ => return Err(invalid("unsupported coordinate value")),
        };
        patches.push((span.start + offset..span.end + offset, replacement));
    }
    patches.sort_unstable_by_key(|(span, _)| span.start);
    let mut out = String::with_capacity(input.len());
    let mut cursor = 0;
    for (span, replacement) in patches {
        if span.start < cursor || span.end < span.start {
            return Err(invalid("coordinate source spans overlap"));
        }
        out.push_str(
            input
                .get(cursor..span.start)
                .ok_or_else(|| invalid("coordinate source prefix is invalid"))?,
        );
        out.push_str(&replacement);
        cursor = span.end;
    }
    out.push_str(
        input
            .get(cursor..)
            .ok_or_else(|| invalid("coordinate source tail is invalid"))?,
    );
    if out.len() as u64 > SOURCE_FILE_READ_MAX_BYTES {
        return Err(invalid(
            "refreshed policy exceeds the normal loader byte limit (policy unchanged)",
        ));
    }
    let actual_cfg = crate::parse_policy(&out)?;
    if actual_cfg != expected_cfg {
        return Err(invalid("refresh would change unrelated policy semantics"));
    }
    let actual: toml::Value = toml::from_str(out.strip_prefix('\u{feff}').unwrap_or(&out))
        .map_err(|error| invalid(error.to_string()))?;
    if actual != expected {
        return Err(invalid("refresh would change unrelated raw policy values"));
    }
    Ok(out)
}

fn field<'a, 'b>(
    value: &'b toml::Spanned<DeValue<'a>>,
    name: &str,
) -> CargoAllowResult<&'b toml::Spanned<DeValue<'a>>> {
    value
        .get_ref()
        .as_table()
        .and_then(|table| {
            table
                .iter()
                .find_map(|(key, value)| (key.get_ref().as_ref() == name).then_some(value))
        })
        .ok_or_else(|| invalid(format!("selected source field is missing: {name}")))
}

fn invalid(message: impl Into<String>) -> CargoAllowError {
    CargoAllowError::with_kind(CargoAllowErrorKind::InvalidPolicy, message)
}

#[cfg(test)]
#[path = "refresh_text_tests.rs"]
mod tests;
