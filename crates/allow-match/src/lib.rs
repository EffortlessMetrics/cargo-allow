use allow_core::{
    AllowConfig, AllowEntry, Finding, FindingKind, MatchOutcome, MatchStatus, SimpleDate,
    glob_matches, maybe_line_distance_score, normalize_path,
};
use std::collections::BTreeSet;

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
    let today = SimpleDate::today_utc_approx();

    for (finding_index, finding) in findings.iter().enumerate() {
        let mut candidates = Vec::new();
        for (entry_index, entry) in cfg.allow.iter().enumerate() {
            if let Some(score) = score_match(entry, finding) {
                if score >= 80 {
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
                used_entries.insert(*entry_index);
                let (status, message) = classify_matched(entry, *score, today, cfg, mode);
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
        let entry = AllowEntry {
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
            lifecycle: Lifecycle {
                created: None,
                review_after: None,
                expires: Some("2026-12-31".to_string()),
            },
            selector: Selector {
                ast_kind: Some("unsafe_fn".to_string()),
                container: Some("scan_line".to_string()),
                normalized_snippet_hash: Some("fnv1a64:expected".to_string()),
                ..Selector::default()
            },
            last_seen: None,
        };

        assert_eq!(score_match(&entry, &finding), None);
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
}
