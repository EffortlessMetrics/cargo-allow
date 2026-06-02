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
