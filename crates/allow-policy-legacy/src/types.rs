use allow_core::{Finding, FindingKind, LastSeen, glob_matches, normalize_path};

#[derive(Debug, Clone)]
pub(crate) struct LegacyNonRustRule {
    pub(crate) id: String,
    pub(crate) pattern: String,
    pub(crate) is_path: bool,
    pub(crate) owner: String,
    pub(crate) classification: String,
    pub(crate) reason: String,
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
    pub(crate) created: Option<String>,
    pub(crate) expires: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct LegacyNoPanicBaselineEntry {
    pub(crate) index: usize,
    pub(crate) path: String,
    pub(crate) family: String,
    pub(crate) selector_kind: String,
    pub(crate) selector_callee: String,
    pub(crate) snippet: String,
    pub(crate) count: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct LegacyNoPanicAllowEntry {
    pub(crate) index: usize,
    pub(crate) id: String,
    pub(crate) path: String,
    pub(crate) family: String,
    pub(crate) selector_kind: String,
    pub(crate) selector_callee: Option<String>,
    pub(crate) selector_container: Option<String>,
    pub(crate) owner: String,
    pub(crate) classification: String,
    pub(crate) reason: String,
    pub(crate) created: Option<String>,
    pub(crate) review_after: Option<String>,
    pub(crate) expires: Option<String>,
    pub(crate) line_hint: Option<u32>,
    pub(crate) last_seen: Option<LastSeen>,
}

#[derive(Debug, Clone)]
pub(crate) struct LegacyClippyRule {
    pub(crate) id: String,
    pub(crate) path: String,
    pub(crate) lint: String,
    pub(crate) family: String,
    pub(crate) owner: String,
    pub(crate) classification: String,
    pub(crate) reason: String,
    pub(crate) symbol: Option<String>,
    pub(crate) target_fingerprint: Option<String>,
    pub(crate) created: Option<String>,
    pub(crate) review_after: Option<String>,
    pub(crate) expires: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct LegacyUnsafeRule {
    pub(crate) id: String,
    pub(crate) path: String,
    pub(crate) family: String,
    pub(crate) selector_kind: String,
    pub(crate) selector_container: Option<String>,
    pub(crate) owner: String,
    pub(crate) classification: String,
    pub(crate) reason: String,
    pub(crate) evidence: Vec<String>,
    pub(crate) created: Option<String>,
    pub(crate) review_after: Option<String>,
    pub(crate) expires: Option<String>,
    pub(crate) line_hint: Option<u32>,
    pub(crate) last_seen: Option<LastSeen>,
}

#[derive(Debug, Clone)]
pub(crate) struct LegacyExecutableRule {
    pub(crate) id: String,
    pub(crate) path: String,
    pub(crate) owner: String,
    pub(crate) reason: String,
    pub(crate) interpreter: Option<String>,
    pub(crate) created: Option<String>,
    pub(crate) review_after: Option<String>,
    pub(crate) expires: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct LegacyWorkflowRule {
    pub(crate) path: String,
    pub(crate) owner: String,
    pub(crate) reason: String,
    pub(crate) permissions: Vec<String>,
    pub(crate) secrets_used: Vec<String>,
    pub(crate) external_actions: Vec<String>,
    pub(crate) duplicate_of_lane: Option<String>,
    pub(crate) created: Option<String>,
    pub(crate) review_after: Option<String>,
    pub(crate) expires: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct LegacyDependencySurfaceRule {
    pub(crate) id: String,
    pub(crate) pattern: String,
    pub(crate) is_glob: bool,
    pub(crate) surface: String,
    pub(crate) owner: String,
    pub(crate) reason: String,
    pub(crate) broad_glob_reason: Option<String>,
    pub(crate) dep_count_at_baseline: Option<i64>,
    pub(crate) created: Option<String>,
    pub(crate) review_after: Option<String>,
    pub(crate) expires: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct LegacyProcessRule {
    pub(crate) id: String,
    pub(crate) binary: String,
    pub(crate) argv_shape: Vec<String>,
    pub(crate) network_reach: bool,
    pub(crate) called_by: Vec<String>,
    pub(crate) owner: String,
    pub(crate) reason: String,
    pub(crate) created: Option<String>,
    pub(crate) review_after: Option<String>,
    pub(crate) expires: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct LegacyNetworkRule {
    pub(crate) id: String,
    pub(crate) destination: String,
    pub(crate) auth_required: bool,
    pub(crate) auth_secret: Option<String>,
    pub(crate) lane: String,
    pub(crate) owner: String,
    pub(crate) reason: String,
    pub(crate) created: Option<String>,
    pub(crate) review_after: Option<String>,
    pub(crate) expires: Option<String>,
}
