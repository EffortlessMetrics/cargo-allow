pub(crate) fn cargo_allow_panic_family(family: &str) -> String {
    if family == "panic" {
        "panic_macro".to_string()
    } else {
        family.to_string()
    }
}

pub(crate) fn normalize_selector_kind(kind: &str) -> String {
    kind.replace('-', "_")
}

pub(crate) fn no_panic_macro_name(family: &str) -> String {
    if family == "panic" {
        "panic".to_string()
    } else {
        family.to_string()
    }
}

pub(crate) fn no_panic_method_callee(family: &str, selector_callee: Option<&str>) -> String {
    match selector_callee.map(str::trim) {
        Some(callee) if callee.ends_with("unwrap") || callee.contains("::unwrap") => {
            "unwrap".to_string()
        }
        Some(callee) if callee.ends_with("expect") || callee.contains("::expect") => {
            "expect".to_string()
        }
        Some(callee) if !callee.is_empty() => callee.to_string(),
        _ => family.to_string(),
    }
}
