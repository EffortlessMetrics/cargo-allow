use allow_core::{
    AllowConfig, AllowEntry, Finding, FindingKind, MatchOutcome, MatchStatus, SimpleDate,
    glob_matches, maybe_line_distance_score, normalize_path,
};
use std::collections::{BTreeMap, BTreeSet};

pub const STRUCTURAL_MATCH_THRESHOLD: u32 = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckMode {
    Audit,
    NoNew,
    Strict,
    Release,
}

impl CheckMode {
    pub fn parse(input: &str) -> Self {
        match input {
            "strict" => Self::Strict,
            "release" => Self::Release,
            "audit" => Self::Audit,
            _ => Self::NoNew,
        }
    }

    pub fn fails(self, status: MatchStatus) -> bool {
        match self {
            Self::Audit => false,
            Self::NoNew => status.is_failure_in_no_new(),
            Self::Strict | Self::Release => status.is_failure_in_strict(),
        }
    }
}

pub fn evaluate(cfg: &AllowConfig, findings: &[Finding], mode: CheckMode) -> Vec<MatchOutcome> {
    let mut outcomes = Vec::new();
    let mut used_entries = BTreeSet::new();
    let mut entry_occurrences = BTreeMap::<usize, u32>::new();
    let today = SimpleDate::today_utc_approx();

    for (finding_index, finding) in findings.iter().enumerate() {
        let mut candidates = Vec::new();
        for (entry_index, entry) in cfg.allow.iter().enumerate() {
            if let Some(score) = score_match(entry, finding) {
                if score >= STRUCTURAL_MATCH_THRESHOLD {
                    candidates.push((entry_index, score));
                }
            }
        }
        match candidates.as_slice() {
            [] => outcomes.push(MatchOutcome {
                status: MatchStatus::New,
                allow_id: None,
                finding_index: Some(finding_index),
                message: format!(
                    "unreceipted {}{} at {}",
                    finding.kind,
                    family_suffix(finding),
                    finding_location(finding)
                ),
                score: 0,
            }),
            [(entry_index, score)] => {
                let entry = &cfg.allow[*entry_index];
                let current_count = entry_occurrences.get(entry_index).copied().unwrap_or(0);
                if entry
                    .occurrence_limit
                    .is_some_and(|limit| current_count >= limit)
                {
                    used_entries.insert(*entry_index);
                    outcomes.push(MatchOutcome {
                        status: MatchStatus::New,
                        allow_id: Some(entry.id.clone()),
                        finding_index: Some(finding_index),
                        message: format!(
                            "{} occurrence_limit exceeded at {}",
                            entry.id,
                            finding_location(finding)
                        ),
                        score: *score,
                    });
                    continue;
                }
                used_entries.insert(*entry_index);
                entry_occurrences.insert(*entry_index, current_count + 1);
                let (status, message) = classify_matched(entry, finding, *score, today, cfg, mode);
                outcomes.push(MatchOutcome {
                    status,
                    allow_id: Some(entry.id.clone()),
                    finding_index: Some(finding_index),
                    message,
                    score: *score,
                });
            }
            many => {
                let ids = many
                    .iter()
                    .map(|(idx, _)| cfg.allow[*idx].id.clone())
                    .collect::<Vec<_>>()
                    .join(", ");
                outcomes.push(MatchOutcome {
                    status: MatchStatus::Ambiguous,
                    allow_id: None,
                    finding_index: Some(finding_index),
                    message: format!(
                        "finding at {} matched multiple allow entries: {ids}",
                        finding_location(finding)
                    ),
                    score: many.iter().map(|(_, score)| *score).max().unwrap_or(0),
                });
            }
        }
    }

    for (entry_index, entry) in cfg.allow.iter().enumerate() {
        if used_entries.contains(&entry_index) {
            continue;
        }
        outcomes.push(MatchOutcome {
            status: MatchStatus::Stale,
            allow_id: Some(entry.id.clone()),
            finding_index: None,
            message: format!(
                "{} is stale: no current finding matched {}",
                entry.id,
                entry.path_or_glob()
            ),
            score: 0,
        });
    }
    outcomes
}

pub fn score_match(entry: &AllowEntry, finding: &Finding) -> Option<u32> {
    if entry.kind != finding.kind {
        return None;
    }
    if let Some(family) = &entry.family {
        if finding.family.as_deref() != Some(family.as_str()) {
            return None;
        }
    }
    if !path_matches(entry, finding) {
        return None;
    }
    let mut score = 100;
    if entry.family.is_some() {
        score += 30;
    }
    let sel = &entry.selector;
    if let Some(ast_kind) = &sel.ast_kind {
        if &finding.identity.ast_kind != ast_kind {
            return None;
        }
        score += 45;
    }
    if let Some(container) = &sel.container {
        if finding.identity.container.as_deref() != Some(container.as_str()) {
            return None;
        }
        score += 40;
    }
    if let Some(callee) = &sel.callee {
        if finding.identity.callee.as_deref() != Some(callee.as_str()) {
            return None;
        }
        score += 35;
    }
    if let Some(macro_name) = &sel.macro_name {
        if finding.identity.macro_name.as_deref() != Some(macro_name.as_str()) {
            return None;
        }
        score += 35;
    }
    if let Some(lint) = &sel.lint {
        if finding.identity.lint.as_deref() != Some(lint.as_str()) {
            return None;
        }
        score += 35;
    }
    if let Some(symbol) = &sel.symbol {
        if finding
            .identity
            .symbol
            .as_deref()
            .map(|s| s.contains(symbol))
            .unwrap_or(false)
        {
            score += 20;
        } else {
            return None;
        }
    }
    if let Some(receiver) = &sel.receiver_fingerprint {
        if finding.identity.receiver_fingerprint.as_deref() == Some(receiver.as_str()) {
            score += 25;
        } else if finding
            .identity
            .receiver_fingerprint
            .as_deref()
            .map(|s| s.contains(receiver))
            .unwrap_or(false)
        {
            score += 10;
        } else {
            return None;
        }
    }
    if let Some(target) = &sel.target_fingerprint {
        if finding
            .identity
            .target_fingerprint
            .as_deref()
            .map(|s| s.contains(target))
            .unwrap_or(false)
        {
            score += 20;
        } else {
            return None;
        }
    }
    if let Some(hash) = &sel.normalized_snippet_hash {
        if finding.identity.normalized_snippet_hash.as_deref() == Some(hash.as_str()) {
            score += 35;
        } else {
            return None;
        }
    }
    let line = finding.span.as_ref().map(|s| s.line);
    score += maybe_line_distance_score(
        sel.line_hint
            .or_else(|| entry.last_seen.as_ref().map(|l| l.line)),
        line,
    );
    Some(score)
}

fn path_matches(entry: &AllowEntry, finding: &Finding) -> bool {
    if let Some(path) = &entry.path {
        if normalize_path(path) == normalize_path(&finding.path) {
            return true;
        }
    }
    if let Some(glob) = &entry.glob {
        if glob_matches(glob, &finding.path) {
            return true;
        }
    }
    if let Some(glob) = &entry.selector.glob {
        if glob_matches(glob, &finding.path) {
            return true;
        }
    }
    false
}

fn classify_matched(
    entry: &AllowEntry,
    finding: &Finding,
    score: u32,
    today: SimpleDate,
    cfg: &AllowConfig,
    mode: CheckMode,
) -> (MatchStatus, String) {
    if let Some(expires) = &entry.lifecycle.expires {
        if expires != "never" {
            if let Some(expiry) = SimpleDate::parse(expires) {
                if expiry < today {
                    return (
                        MatchStatus::Expired,
                        format!("{} matched but expired on {expires}", entry.id),
                    );
                }
            }
        }
    }
    if cfg.requirements.unsafe_evidence_required
        && entry.kind == FindingKind::Unsafe
        && entry.evidence.is_empty()
    {
        return (
            MatchStatus::EvidenceMissing,
            format!("{} matched unsafe finding but has no evidence", entry.id),
        );
    }
    if cfg.requirements.unsafe_safety_comment_required
        && entry.kind == FindingKind::Unsafe
        && finding.identity.target_fingerprint.as_deref() != Some("safety-comment:present")
    {
        return (
            MatchStatus::EvidenceMissing,
            format!(
                "{} matched unsafe finding but has no nearby SAFETY comment",
                entry.id
            ),
        );
    }
    if entry.kind == FindingKind::LintException
        && !cfg.requirements.allow_bare_allow_attributes
        && finding.family.as_deref() == Some("allow_attribute")
    {
        return (
            MatchStatus::InvalidSelector,
            format!(
                "{} matched bare allow attribute while allow_bare_allow_attributes=false",
                entry.id
            ),
        );
    }
    if entry.kind == FindingKind::LintException {
        let policy_id = finding
            .identity
            .target_fingerprint
            .as_deref()
            .and_then(|value| value.strip_prefix("policy:"));
        if cfg.requirements.lint_policy_id_required && policy_id.is_none() {
            return (
                MatchStatus::InvalidSelector,
                format!(
                    "{} matched lint suppression without required policy:<allow-id> reference",
                    entry.id
                ),
            );
        }
        if let Some(policy_id) = policy_id {
            if policy_id != entry.id {
                return (
                    MatchStatus::InvalidSelector,
                    format!(
                        "{} matched lint suppression that references policy:{policy_id}",
                        entry.id
                    ),
                );
            }
        }
    }
    if entry.classification == "baseline_debt" && matches!(mode, CheckMode::Release) {
        return (
            MatchStatus::BaselineDebt,
            format!("{} is baseline debt and cannot pass release mode", entry.id),
        );
    }
    (
        MatchStatus::Matched,
        format!("{} matched with structural score {score}", entry.id),
    )
}

fn family_suffix(finding: &Finding) -> String {
    finding
        .family
        .as_ref()
        .map(|f| format!(".{f}"))
        .unwrap_or_default()
}

pub fn finding_location(finding: &Finding) -> String {
    match &finding.span {
        Some(span) => format!(
            "{}:{}:{}",
            normalize_path(&finding.path),
            span.line,
            span.column
        ),
        None => normalize_path(&finding.path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{Lifecycle, Selector, Span, StructuralIdentity};
    use std::path::PathBuf;

    #[test]
    fn matches_moved_line_by_structure() {
        let mut id = StructuralIdentity::new("rust", "method_call");
        id.container = Some("load".to_string());
        id.callee = Some("unwrap".to_string());
        let finding = Finding {
            kind: FindingKind::Panic,
            family: Some("unwrap".to_string()),
            path: PathBuf::from("src/lib.rs"),
            span: Some(Span {
                line: 50,
                column: 12,
            }),
            identity: id,
            message: String::new(),
        };
        let entry = AllowEntry {
            id: "allow-1".to_string(),
            kind: FindingKind::Panic,
            family: Some("unwrap".to_string()),
            path: Some(PathBuf::from("src/lib.rs")),
            glob: None,
            owner: "core".to_string(),
            classification: "test".to_string(),
            reason: "reason".to_string(),
            evidence: Vec::new(),
            links: Vec::new(),
            occurrence_limit: None,
            lifecycle: Lifecycle {
                created: None,
                review_after: None,
                expires: Some("2026-12-31".to_string()),
            },
            selector: Selector {
                ast_kind: Some("method_call".to_string()),
                container: Some("load".to_string()),
                callee: Some("unwrap".to_string()),
                line_hint: Some(12),
                ..Selector::default()
            },
            last_seen: None,
        };
        assert!(score_match(&entry, &finding).unwrap() >= 80);
    }

    #[test]
    fn snippet_hash_selector_rejects_different_source() {
        let finding = finding_with_hash("fnv1a64:actual");
        let entry = entry_with_hash("fnv1a64:expected");

        assert_eq!(score_match(&entry, &finding), None);
    }

    #[test]
    fn snippet_hash_selector_accepts_same_source() {
        let finding = finding_with_hash("fnv1a64:actual");
        let entry = entry_with_hash("fnv1a64:actual");

        assert!(score_match(&entry, &finding).is_some());
    }

    #[test]
    fn structural_field_mismatch_rejects_match() {
        let finding = finding_with_hash("fnv1a64:actual");
        let mut entry = entry_with_hash("fnv1a64:actual");
        entry.selector.container = Some("other_container".to_string());

        assert_eq!(score_match(&entry, &finding), None);
    }

    #[test]
    fn unsafe_safety_comment_requirement_fails_without_metadata() {
        let finding = finding_with_hash("fnv1a64:actual");
        let mut cfg = AllowConfig::empty();
        cfg.requirements.unsafe_safety_comment_required = true;
        cfg.allow.push(entry_with_hash("fnv1a64:actual"));

        let outcomes = evaluate(&cfg, &[finding], CheckMode::NoNew);

        assert!(outcomes.iter().any(|outcome| {
            outcome.status == MatchStatus::EvidenceMissing
                && outcome.message.contains("no nearby SAFETY comment")
        }));
    }

    #[test]
    fn unsafe_safety_comment_requirement_passes_with_metadata() {
        let mut finding = finding_with_hash("fnv1a64:actual");
        finding.identity.target_fingerprint = Some("safety-comment:present".to_string());
        let mut cfg = AllowConfig::empty();
        cfg.requirements.unsafe_safety_comment_required = true;
        cfg.allow.push(entry_with_hash("fnv1a64:actual"));

        let outcomes = evaluate(&cfg, &[finding], CheckMode::NoNew);

        assert!(
            outcomes
                .iter()
                .any(|outcome| outcome.status == MatchStatus::Matched)
        );
    }

    #[test]
    fn evaluate_fails_closed_on_ambiguous_structural_matches() {
        let finding = finding_with_hash("fnv1a64:actual");
        let mut first = entry_with_hash("fnv1a64:actual");
        first.id = "allow-1".to_string();
        let mut second = entry_with_hash("fnv1a64:actual");
        second.id = "allow-2".to_string();
        let mut cfg = AllowConfig::empty();
        cfg.allow.push(first);
        cfg.allow.push(second);

        let outcomes = evaluate(&cfg, &[finding], CheckMode::NoNew);

        assert!(
            outcomes
                .iter()
                .any(|outcome| outcome.status == MatchStatus::Ambiguous)
        );
        assert!(
            outcomes
                .iter()
                .find(|outcome| outcome.status == MatchStatus::Ambiguous)
                .map(|outcome| outcome.score >= STRUCTURAL_MATCH_THRESHOLD)
                .unwrap_or(false)
        );
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| outcome.status == MatchStatus::Stale)
                .count(),
            2
        );
    }

    #[test]
    fn lint_policy_reference_must_match_entry_id() {
        let finding = lint_finding_with_policy("allow-other");
        let mut cfg = AllowConfig::empty();
        cfg.allow.push(lint_entry("allow-lint"));

        let outcomes = evaluate(&cfg, &[finding], CheckMode::NoNew);

        assert!(outcomes.iter().any(|outcome| {
            outcome.status == MatchStatus::InvalidSelector
                && outcome.message.contains("policy:allow-other")
        }));
    }

    #[test]
    fn lint_policy_reference_matching_entry_id_passes() {
        let finding = lint_finding_with_policy("allow-lint");
        let mut cfg = AllowConfig::empty();
        cfg.allow.push(lint_entry("allow-lint"));

        let outcomes = evaluate(&cfg, &[finding], CheckMode::NoNew);

        assert!(
            outcomes
                .iter()
                .any(|outcome| outcome.status == MatchStatus::Matched)
        );
    }

    #[test]
    fn bare_allow_attribute_fails_when_policy_disallows_it() {
        let finding = lint_finding("allow_attribute");
        let mut cfg = AllowConfig::empty();
        cfg.requirements.allow_bare_allow_attributes = false;
        cfg.allow
            .push(lint_entry_with_family("allow-lint", "allow_attribute"));

        let outcomes = evaluate(&cfg, &[finding], CheckMode::NoNew);

        assert!(outcomes.iter().any(|outcome| {
            outcome.status == MatchStatus::InvalidSelector
                && outcome
                    .message
                    .contains("allow_bare_allow_attributes=false")
        }));
    }

    #[test]
    fn bare_allow_attribute_passes_when_policy_allows_it() {
        let finding = lint_finding("allow_attribute");
        let mut cfg = AllowConfig::empty();
        cfg.requirements.allow_bare_allow_attributes = true;
        cfg.allow
            .push(lint_entry_with_family("allow-lint", "allow_attribute"));

        let outcomes = evaluate(&cfg, &[finding], CheckMode::NoNew);

        assert!(
            outcomes
                .iter()
                .any(|outcome| outcome.status == MatchStatus::Matched)
        );
    }

    #[test]
    fn lint_policy_id_is_required_when_configured() {
        let finding = lint_finding("expect_attribute");
        let mut cfg = AllowConfig::empty();
        cfg.requirements.lint_policy_id_required = true;
        cfg.allow.push(lint_entry("allow-lint"));

        let outcomes = evaluate(&cfg, &[finding], CheckMode::NoNew);

        assert!(outcomes.iter().any(|outcome| {
            outcome.status == MatchStatus::InvalidSelector
                && outcome
                    .message
                    .contains("without required policy:<allow-id> reference")
        }));
    }

    #[test]
    fn lint_policy_id_is_optional_by_default() {
        let finding = lint_finding("expect_attribute");
        let mut cfg = AllowConfig::empty();
        cfg.allow.push(lint_entry("allow-lint"));

        let outcomes = evaluate(&cfg, &[finding], CheckMode::NoNew);

        assert!(
            outcomes
                .iter()
                .any(|outcome| outcome.status == MatchStatus::Matched)
        );
    }

    #[test]
    fn occurrence_limit_caps_matched_findings() {
        let finding = finding_with_hash("fnv1a64:actual");
        let mut entry = entry_with_hash("fnv1a64:actual");
        entry.occurrence_limit = Some(1);
        let mut cfg = AllowConfig::empty();
        cfg.allow.push(entry);

        let outcomes = evaluate(&cfg, &[finding.clone(), finding], CheckMode::NoNew);

        assert_eq!(outcomes.len(), 2);
        assert!(matches!(
            outcomes.first().map(|outcome| outcome.status),
            Some(MatchStatus::Matched)
        ));
        assert!(outcomes.iter().any(|outcome| {
            outcome.status == MatchStatus::New
                && outcome.message.contains("occurrence_limit exceeded")
        }));
    }

    #[test]
    fn unlimited_entry_matches_repeated_findings() {
        let finding = finding_with_hash("fnv1a64:actual");
        let mut cfg = AllowConfig::empty();
        cfg.allow.push(entry_with_hash("fnv1a64:actual"));

        let outcomes = evaluate(&cfg, &[finding.clone(), finding], CheckMode::NoNew);

        assert_eq!(outcomes.len(), 2);
        assert!(
            outcomes
                .iter()
                .all(|outcome| outcome.status == MatchStatus::Matched)
        );
    }

    fn entry_with_hash(hash: &str) -> AllowEntry {
        AllowEntry {
            id: "allow-1".to_string(),
            kind: FindingKind::Unsafe,
            family: Some("unsafe_fn".to_string()),
            path: Some(PathBuf::from("src/lib.rs")),
            glob: None,
            owner: "core".to_string(),
            classification: "test".to_string(),
            reason: "reason".to_string(),
            evidence: vec!["unsafe-review".to_string()],
            links: Vec::new(),
            occurrence_limit: None,
            lifecycle: Lifecycle {
                created: None,
                review_after: None,
                expires: Some("2026-12-31".to_string()),
            },
            selector: Selector {
                ast_kind: Some("unsafe_fn".to_string()),
                container: Some("scan_line".to_string()),
                normalized_snippet_hash: Some(hash.to_string()),
                ..Selector::default()
            },
            last_seen: None,
        }
    }

    fn finding_with_hash(hash: &str) -> Finding {
        let mut id = StructuralIdentity::new("rust", "unsafe_fn");
        id.container = Some("scan_line".to_string());
        id.normalized_snippet_hash = Some(hash.to_string());
        Finding {
            kind: FindingKind::Unsafe,
            family: Some("unsafe_fn".to_string()),
            path: PathBuf::from("src/lib.rs"),
            span: Some(Span {
                line: 50,
                column: 12,
            }),
            identity: id,
            message: String::new(),
        }
    }

    fn lint_entry(id: &str) -> AllowEntry {
        lint_entry_with_family(id, "expect_attribute")
    }

    fn lint_entry_with_family(id: &str, family: &str) -> AllowEntry {
        AllowEntry {
            id: id.to_string(),
            kind: FindingKind::LintException,
            family: Some(family.to_string()),
            path: Some(PathBuf::from("src/lib.rs")),
            glob: None,
            owner: "core".to_string(),
            classification: "reviewed_exception".to_string(),
            reason: "Lint suppression is linked to policy.".to_string(),
            evidence: Vec::new(),
            links: Vec::new(),
            occurrence_limit: None,
            lifecycle: Lifecycle {
                created: None,
                review_after: None,
                expires: Some("2026-12-31".to_string()),
            },
            selector: Selector {
                ast_kind: Some("attribute".to_string()),
                lint: Some("clippy::unwrap_used".to_string()),
                ..Selector::default()
            },
            last_seen: None,
        }
    }

    fn lint_finding_with_policy(policy_id: &str) -> Finding {
        let mut finding = lint_finding("expect_attribute");
        finding.identity.target_fingerprint = Some(format!("policy:{policy_id}"));
        finding
    }

    fn lint_finding(family: &str) -> Finding {
        let mut id = StructuralIdentity::new("rust", "attribute");
        id.lint = Some("clippy::unwrap_used".to_string());
        Finding {
            kind: FindingKind::LintException,
            family: Some(family.to_string()),
            path: PathBuf::from("src/lib.rs"),
            span: Some(Span {
                line: 10,
                column: 1,
            }),
            identity: id,
            message: String::new(),
        }
    }
}
