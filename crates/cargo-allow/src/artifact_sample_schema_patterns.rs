use serde_json::Value;
use std::collections::BTreeSet;

pub(crate) fn sample_string_matches_supported_pattern(value: &str, pattern: &str) -> bool {
    match pattern {
        "^cargo-allow " => value.starts_with("cargo-allow "),
        "^work-[a-z0-9-]+-[0-9]{4}$" => sample_string_matches_work_item_id(value),
        _ => std::panic::panic_any(format!("unsupported schema pattern {pattern:?}")),
    }
}

fn sample_string_matches_work_item_id(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("work-") else {
        return false;
    };
    let Some((kind, number)) = rest.rsplit_once('-') else {
        return false;
    };
    !kind.is_empty()
        && kind
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
        && number.len() == 4
        && number.chars().all(|ch| ch.is_ascii_digit())
}

pub(crate) fn supported_schema_patterns() -> BTreeSet<String> {
    ["^cargo-allow ", "^work-[a-z0-9-]+-[0-9]{4}$"]
        .into_iter()
        .map(std::string::ToString::to_string)
        .collect()
}

pub(crate) fn collect_schema_patterns(value: &Value, patterns: &mut BTreeSet<String>) {
    match value {
        Value::Object(object) => {
            if let Some(pattern) = object.get("pattern").and_then(Value::as_str) {
                patterns.insert(pattern.to_string());
            }
            for child in object.values() {
                collect_schema_patterns(child, patterns);
            }
        }
        Value::Array(items) => {
            for child in items {
                collect_schema_patterns(child, patterns);
            }
        }
        _ => {}
    }
}
