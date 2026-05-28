use allow_core::{
    AllowConfig, AllowEntry, Finding, FindingKind, MatchOutcome, MatchStatus, SimpleDate,
    normalize_path,
};
use std::collections::{BTreeMap, BTreeSet};

mod mode;
mod scoring;
pub use mode::CheckMode;
pub use scoring::{STRUCTURAL_MATCH_THRESHOLD, score_match};

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
