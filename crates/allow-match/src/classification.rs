use allow_core::{AllowConfig, AllowEntry, Finding, FindingKind, MatchStatus, SimpleDate};

use crate::CheckMode;

pub(crate) fn classify_matched(
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
