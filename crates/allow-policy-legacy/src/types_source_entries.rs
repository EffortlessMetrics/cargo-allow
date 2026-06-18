use allow_core::{Finding, FindingKind, glob_matches, normalize_path};

#[derive(Debug, Clone)]
pub(crate) struct LegacyNonRustRule {
    pub(crate) id: String,
    pub(crate) pattern: String,
    pub(crate) is_path: bool,
    pub(crate) owner: String,
    pub(crate) classification: String,
    pub(crate) reason: String,
    pub(crate) evidence: Vec<String>,
    pub(crate) created: Option<String>,
    pub(crate) review_after: Option<String>,
    pub(crate) expires: Option<String>,
}

impl LegacyNonRustRule {
    pub(crate) fn matches(&self, finding: &Finding) -> bool {
        if !matches!(
            finding.kind,
            FindingKind::NonRustFile | FindingKind::GeneratedCode
        ) {
            return false;
        }
        if self.is_path {
            normalize_path(&self.pattern) == normalize_path(&finding.path)
        } else {
            glob_matches(&self.pattern, &finding.path)
        }
    }

    pub(crate) fn specificity(&self) -> usize {
        let literal_chars = self
            .pattern
            .chars()
            .filter(|ch| !matches!(ch, '*' | '?' | '[' | ']' | '{' | '}' | ',' | '!'))
            .count();
        literal_chars + if self.is_path { 10_000 } else { 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{Span, StructuralIdentity};
    use std::path::PathBuf;

    #[test]
    fn non_rust_rule_matches_file_findings_by_path_or_glob() {
        let path_rule = legacy_rule("README.md", true);
        let glob_rule = legacy_rule("docs/**", false);

        assert!(path_rule.matches(&finding(FindingKind::NonRustFile, "README.md")));
        assert!(glob_rule.matches(&finding(FindingKind::GeneratedCode, "docs/schema.json")));
        assert!(!path_rule.matches(&finding(FindingKind::NonRustFile, "docs/README.md")));
        assert!(!glob_rule.matches(&finding(FindingKind::Panic, "docs/panic.md")));
    }

    #[test]
    fn non_rust_rule_specificity_prefers_paths_over_equal_globs() {
        let path_specificity = legacy_rule("docs/guide.md", true).specificity();
        let glob_specificity = legacy_rule("docs/guide.md", false).specificity();
        let broader_glob_specificity = legacy_rule("docs/**", false).specificity();

        assert!(path_specificity > glob_specificity);
        assert!(glob_specificity > broader_glob_specificity);
    }

    fn legacy_rule(pattern: &str, is_path: bool) -> LegacyNonRustRule {
        LegacyNonRustRule {
            id: "non-rust-doc".to_string(),
            pattern: pattern.to_string(),
            is_path,
            owner: "docs".to_string(),
            classification: "documentation".to_string(),
            reason: "Documentation files are intentionally tracked.".to_string(),
            evidence: Vec::new(),
            created: Some("2026-05-09".to_string()),
            review_after: Some("2026-09-09".to_string()),
            expires: Some("never".to_string()),
        }
    }

    fn finding(kind: FindingKind, path: &str) -> Finding {
        Finding {
            kind,
            family: Some(kind.as_str().to_string()),
            path: PathBuf::from(path),
            span: Some(Span { line: 1, column: 1 }),
            identity: StructuralIdentity::new("file", "tracked_file"),
            message: format!("tracked file: {path}"),
            ledger: None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct LegacyGeneratedRule {
    pub(crate) id: String,
    pub(crate) path: String,
    pub(crate) owner: String,
    pub(crate) reason: String,
    pub(crate) generator: Option<String>,
    pub(crate) regenerate_command: Option<String>,
    pub(crate) evidence: Vec<String>,
    pub(crate) created: Option<String>,
    pub(crate) expires: Option<String>,
}
