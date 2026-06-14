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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panic_family_maps_macro_family_and_preserves_others() {
        assert_eq!(cargo_allow_panic_family("panic"), "panic_macro");
        assert_eq!(cargo_allow_panic_family("unwrap"), "unwrap");
    }

    #[test]
    fn selector_kind_replaces_legacy_hyphen_separator() {
        assert_eq!(normalize_selector_kind("method-call"), "method_call");
        assert_eq!(normalize_selector_kind("macro-call"), "macro_call");
        assert_eq!(
            normalize_selector_kind("already_normalized"),
            "already_normalized"
        );
    }

    #[test]
    fn macro_name_maps_panic_family_and_preserves_other_macros() {
        assert_eq!(no_panic_macro_name("panic"), "panic");
        assert_eq!(no_panic_macro_name("todo"), "todo");
    }

    #[test]
    fn method_callee_recognizes_unwrap_and_expect_aliases() {
        assert_eq!(
            no_panic_method_callee("unwrap", Some("core::option::Option::unwrap")),
            "unwrap"
        );
        assert_eq!(
            no_panic_method_callee("unwrap", Some("Result::unwrap")),
            "unwrap"
        );
        assert_eq!(
            no_panic_method_callee("expect", Some("std::result::Result::expect")),
            "expect"
        );
        assert_eq!(
            no_panic_method_callee("expect", Some("Option::expect")),
            "expect"
        );
    }

    #[test]
    fn method_callee_uses_custom_non_empty_value_or_family_fallback() {
        assert_eq!(
            no_panic_method_callee("panic_macro", Some("custom_abort")),
            "custom_abort"
        );
        assert_eq!(no_panic_method_callee("unwrap", Some("  ")), "unwrap");
        assert_eq!(no_panic_method_callee("expect", None), "expect");
    }
}
