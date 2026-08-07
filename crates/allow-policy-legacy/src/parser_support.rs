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
            canonicalize_legacy_date(&value)
        }
    })
}

/// Normalize a legacy `created` or `review_after` date field through the same
/// canonicalization as `expires` (#1870). Without this, RFC3339 timestamps in
/// `created`/`review_after` produce byte-different migration output across
/// ripr versions.
pub(crate) fn normalize_legacy_date_field(value: Option<String>) -> Option<String> {
    value.map(|v| canonicalize_legacy_date(&v))
}

/// Canonicalize a legacy date/timestamp to YYYY-MM-DD for deterministic
/// migration output (#1870).
///
/// cargo-allow's `SimpleDate` only parses `YYYY-MM-DD`. Legacy ledgers may
/// use RFC3339 timestamps with timezones (e.g. `2025-12-01T00:00:00Z` or
/// `2025-12-01T00:00:00-05:00`). Without canonicalization, two ledgers
/// expressing the same logical date in different encodings produce
/// byte-different migration output.
///
/// This function strips any time/timezone component and returns just the
/// date portion. If the input is already a plain date, it passes through
/// unchanged.
pub(crate) fn canonicalize_legacy_date(value: &str) -> String {
    // If it's already a plain YYYY-MM-DD (10 chars, no 'T'), pass through.
    if value.len() == 10 && !value.contains('T') {
        return value.to_string();
    }
    // Try to extract the date portion before any 'T' separator.
    if let Some(date_part) = value.split('T').next()
        && date_part.len() == 10
    {
        // Validate it looks like a date (basic sanity check — the downstream
        // SimpleDate::parse will reject truly malformed values).
        return date_part.to_string();
    }
    // Unknown format — pass through and let downstream validation catch it.
    value.to_string()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_unsafe_family_aliases_and_fallbacks() {
        let cases = [
            (" unsafe-block ", "unsafe_block"),
            ("unsafe block", "unsafe_block"),
            ("unsafe-fn", "unsafe_fn"),
            ("unsafe function", "unsafe_fn"),
            ("unsafe_fn", "unsafe_fn"),
            ("unsafe-impl", "unsafe_impl"),
            ("unsafe impl", "unsafe_impl"),
            ("unsafe-trait", "unsafe_trait"),
            ("unsafe trait", "unsafe_trait"),
            ("unsafe-extern", "unsafe_extern_block"),
            ("unsafe extern", "unsafe_extern_block"),
            ("unsafe-extern-block", "unsafe_extern_block"),
            ("unsafe extern block", "unsafe_extern_block"),
            ("unsafe-attr", "unsafe_attr"),
            ("unsafe attribute", "unsafe_attr"),
            ("unsafe-attribute", "unsafe_attr"),
            ("custom-family", "custom_family"),
        ];

        for (input, expected) in cases {
            assert_eq!(normalize_unsafe_family(input), expected);
        }
    }

    #[test]
    fn detects_glob_meta_characters() {
        for input in [
            "src/*.rs",
            "src/file?.rs",
            "src/[ab].rs",
            "src/file].rs",
            "src/{lib,main}.rs",
            "src/file}.rs",
            "src/lib.rs,src/main.rs",
        ] {
            assert!(has_glob_meta(input));
        }

        assert!(!has_glob_meta("src/lib.rs"));
        assert!(!has_glob_meta("src/path-with-dashes.rs"));
    }

    #[test]
    fn normalizes_legacy_permanent_expiry_only() {
        assert_eq!(
            normalize_legacy_expires(Some("permanent".to_string())),
            Some("never".to_string())
        );
        assert_eq!(
            normalize_legacy_expires(Some("2026-12-31".to_string())),
            Some("2026-12-31".to_string())
        );
        assert_eq!(normalize_legacy_expires(None), None);
    }

    #[test]
    fn normalizes_legacy_expires_strips_timestamps_to_date_only() {
        // #1870: RFC3339 timestamps with timezones must canonicalize to the
        // same YYYY-MM-DD for deterministic migration output.
        assert_eq!(
            normalize_legacy_expires(Some("2025-12-01T00:00:00Z".to_string())),
            Some("2025-12-01".to_string())
        );
        assert_eq!(
            normalize_legacy_expires(Some("2025-12-01T00:00:00-05:00".to_string())),
            Some("2025-12-01".to_string())
        );
        assert_eq!(
            normalize_legacy_expires(Some("2025-12-01T23:59:59+09:00".to_string())),
            Some("2025-12-01".to_string())
        );
        // Plain dates pass through unchanged.
        assert_eq!(
            normalize_legacy_expires(Some("2025-12-01".to_string())),
            Some("2025-12-01".to_string())
        );
    }

    #[test]
    fn recognizes_clippy_exception_policy_aliases() {
        for policy in [
            "clippy-exceptions",
            "clippy-exception-allowlist",
            "clippy-allowlist",
        ] {
            let mut table = toml::Table::new();
            table.insert("policy".to_string(), Value::String(policy.to_string()));

            assert!(is_clippy_exceptions_policy(&table));
        }

        let mut table = toml::Table::new();
        table.insert("policy".to_string(), Value::String("other".to_string()));
        assert!(!is_clippy_exceptions_policy(&table));
        assert!(!is_clippy_exceptions_policy(&toml::Table::new()));
    }

    #[test]
    fn normalizes_lint_attribute_family_aliases_and_fallbacks() {
        let cases = [
            (" allow ", "allow_attribute"),
            ("allow-attribute", "allow_attribute"),
            ("allow_attribute", "allow_attribute"),
            ("expect", "expect_attribute"),
            ("expect-attribute", "expect_attribute"),
            ("expect_attribute", "expect_attribute"),
            ("custom", "custom"),
        ];

        for (input, expected) in cases {
            assert_eq!(normalize_lint_attribute_family(input), expected);
        }
    }
}
