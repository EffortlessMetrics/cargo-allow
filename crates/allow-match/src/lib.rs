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
mod tests;
