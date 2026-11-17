//! #4840: refuse a single-file non-Rust migration whose selectors overlap in
//! a way the canonical matcher cannot rank.
//!
//! The legacy tool and the compat reader resolve an overlap by continuous
//! literal specificity. The canonical ledger has no rank field, so `migrate`
//! must not silently emit a policy that scans as `Ambiguous`. The one
//! mechanical exception is an exact path shadowing one glob: the per-finding
//! projection resolves that without changing what the operator wrote.

use allow_core::{AllowConfig, CargoAllowError, CargoAllowErrorKind, CargoAllowResult, Finding};
use std::path::Path;
use toml::Value;

use crate::converter_file_configs::{
    config_from_current_non_rust_findings, config_from_non_rust_rules,
};
use crate::io::{legacy_table_at, read_policy};
use crate::loaders::load_legacy_or_canonical;
use crate::parsers::parse_non_rust_rules;
use crate::types::LegacyNonRustRule;

/// What the overlap scan found for one legacy file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NonRustOverlap {
    None,
    /// Only exact-path-shadowing-glob overlaps; the per-finding projection
    /// resolves them.
    PathShadowOnly,
}

/// Scan the current non-Rust findings for selector overlaps.
///
/// A finding matched by exactly two rules, one exact path and one glob, is the
/// mechanical path-shadow case and resolves. Any other multi-rule match is a
/// genuine ambiguity the canonical ledger cannot rank; the error names the
/// first pair and the finding path.
pub(crate) fn scan_non_rust_overlaps(
    rules: &[LegacyNonRustRule],
    findings: &[Finding],
) -> CargoAllowResult<NonRustOverlap> {
    let mut result = NonRustOverlap::None;
    for finding in findings {
        let matching: Vec<&LegacyNonRustRule> =
            rules.iter().filter(|rule| rule.matches(finding)).collect();
        if matching.len() < 2 {
            continue;
        }
        let path_count = matching.iter().filter(|rule| rule.is_path).count();
        let glob_count = matching.len() - path_count;
        if path_count == 1 && glob_count == 1 {
            result = NonRustOverlap::PathShadowOnly;
            continue;
        }
        let Some(first) = matching.first() else {
            continue;
        };
        let Some(second) = matching.get(1) else {
            continue;
        };
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidPolicy,
            format!(
                "non-Rust selectors `{}` ({}) and `{}` ({}) overlap on `{}`; the canonical ledger cannot rank these - keep one",
                first.id,
                first.pattern,
                second.id,
                second.pattern,
                allow_core::normalize_path(&finding.path)
            ),
        )
        .with_next_step("narrow one selector in the legacy file, then re-run migrate"));
    }
    Ok(result)
}

/// Load a single-file legacy policy for `migrate`.
///
/// Non-Rust files run the overlap scan. Other lanes fall through unchanged.
pub fn load_non_rust_migration_config(
    path: &Path,
    findings: &[Finding],
) -> CargoAllowResult<AllowConfig> {
    let text = read_policy(path)?;
    let Some(table) = legacy_table_at(Some(path), &text)? else {
        return load_legacy_or_canonical(path);
    };
    if table.get("policy").and_then(Value::as_str) != Some("non-rust-allowlist") {
        return load_legacy_or_canonical(path);
    }
    let rules = parse_non_rust_rules(&table)?;
    match scan_non_rust_overlaps(&rules, findings)? {
        NonRustOverlap::None => config_from_non_rust_rules(&table, &rules),
        NonRustOverlap::PathShadowOnly => {
            config_from_current_non_rust_findings(&table, &rules, findings)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{FindingKind, Span, StructuralIdentity};
    use std::path::PathBuf;

    fn rule(id: &str, pattern: &str, is_path: bool) -> LegacyNonRustRule {
        LegacyNonRustRule {
            id: id.to_string(),
            pattern: pattern.to_string(),
            is_path,
            owner: "docs".to_string(),
            classification: "documentation".to_string(),
            reason: "tracked file".to_string(),
            evidence: Vec::new(),
            created: None,
            review_after: None,
            expires: Some("never".to_string()),
        }
    }

    fn finding(path: &str) -> Finding {
        Finding {
            kind: FindingKind::NonRustFile,
            family: Some("non_rust_file".to_string()),
            path: PathBuf::from(path),
            span: Some(Span { line: 1, column: 1 }),
            identity: StructuralIdentity::new("file", "tracked_file"),
            message: String::new(),
            ledger: None,
        }
    }

    #[test]
    fn shiplog_glob_pair_refuses_and_names_both_selectors() {
        let rules = vec![
            rule(
                "non-rust-github-workflows",
                ".github/workflows/*.yml",
                false,
            ),
            rule("non-rust-github-meta", ".github/**", false),
        ];
        let error = scan_non_rust_overlaps(&rules, &[finding(".github/workflows/ci.yml")])
            .expect_err("two globs must refuse");
        let message = error.message();
        assert!(message.contains(".github/workflows/*.yml"), "{message}");
        assert!(message.contains(".github/**"), "{message}");
        assert!(message.contains("overlap"), "{message}");
        assert_eq!(error.kind(), CargoAllowErrorKind::InvalidPolicy);
    }

    #[test]
    fn path_shadowing_one_glob_resolves() {
        let rules = vec![
            rule("non-rust-path", "docs/guide.md", true),
            rule("non-rust-broad", "docs/**", false),
        ];
        assert_eq!(
            scan_non_rust_overlaps(&rules, &[finding("docs/guide.md")]).unwrap_or_else(|err| {
                std::panic::panic_any(format!("shadow should resolve: {err}"))
            }),
            NonRustOverlap::PathShadowOnly
        );
    }

    #[test]
    fn equal_literal_globs_refuse() {
        let rules = vec![
            rule("non-rust-r-question", "src/*.r?", false),
            rule("non-rust-question-s", "src/*.?s", false),
        ];
        let error = scan_non_rust_overlaps(&rules, &[finding("src/lib.rs")])
            .expect_err("two globs must refuse");
        assert!(error.message().contains("src/*.r?"), "{error}");
        assert!(error.message().contains("src/*.?s"), "{error}");
    }

    #[test]
    fn non_overlapping_rules_are_clean() {
        let rules = vec![
            rule("non-rust-docs", "docs/**", false),
            rule("non-rust-readme", "README.md", true),
        ];
        assert_eq!(
            scan_non_rust_overlaps(&rules, &[finding("docs/guide.md")])
                .unwrap_or_else(|err| std::panic::panic_any(format!("clean scan: {err}"))),
            NonRustOverlap::None
        );
    }
}
