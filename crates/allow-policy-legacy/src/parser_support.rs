use toml::Value;

pub(crate) fn normalize_unsafe_family(kind: &str) -> String {
    match kind.trim() {
        "unsafe-block" | "unsafe block" => "unsafe_block".to_string(),
        "unsafe-fn" | "unsafe function" | "unsafe_fn" => "unsafe_fn".to_string(),
        "unsafe-impl" | "unsafe impl" => "unsafe_impl".to_string(),
        "unsafe-trait" | "unsafe trait" => "unsafe_trait".to_string(),
        "unsafe-extern" | "unsafe extern" | "unsafe-extern-block" | "unsafe extern block" => {
            "unsafe_extern_block".to_string()
        }
        "unsafe-attr" | "unsafe attribute" | "unsafe-attribute" => "unsafe_attr".to_string(),
        other => other.replace('-', "_"),
    }
}

pub(crate) fn has_glob_meta(input: &str) -> bool {
    input
        .chars()
        .any(|ch| matches!(ch, '*' | '?' | '[' | ']' | '{' | '}' | ','))
}

pub(crate) fn normalize_legacy_expires(expires: Option<String>) -> Option<String> {
    expires.map(|value| {
        if value == "permanent" {
            "never".to_string()
        } else {
            value
        }
    })
}

pub(crate) fn is_clippy_exceptions_policy(table: &toml::Table) -> bool {
    matches!(
        table.get("policy").and_then(Value::as_str),
        Some("clippy-exceptions" | "clippy-exception-allowlist" | "clippy-allowlist")
    )
}

pub(crate) fn normalize_lint_attribute_family(family: &str) -> String {
    match family.trim() {
        "allow" | "allow-attribute" | "allow_attribute" => "allow_attribute".to_string(),
        "expect" | "expect-attribute" | "expect_attribute" => "expect_attribute".to_string(),
        other => other.to_string(),
    }
}
