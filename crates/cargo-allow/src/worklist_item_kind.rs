pub(crate) const NEW_UNRECEIPTED_FINDING: &str = "new_unreceipted_finding";
pub(crate) const OCCURRENCE_LIMIT_EXCEEDED: &str = "occurrence_limit_exceeded";
pub(crate) const OCCURRENCE_HEADROOM: &str = "occurrence_headroom";
pub(crate) const EXPIRED_ALLOW: &str = "expired_allow";
pub(crate) const STALE_ALLOW: &str = "stale_allow";
pub(crate) const AMBIGUOUS_SELECTOR: &str = "ambiguous_selector";
pub(crate) const UNSAFE_MISSING_EVIDENCE: &str = "unsafe_missing_evidence";
pub(crate) const MISSING_EVIDENCE: &str = "missing_evidence";
pub(crate) const MISSING_REQUIRED_FIELD: &str = "missing_required_field";
pub(crate) const INVALID_SELECTOR: &str = "invalid_selector";
pub(crate) const BASELINE_DEBT: &str = "baseline_debt";
pub(crate) const REVIEW_DUE: &str = "review_due";
pub(crate) const MATCHED: &str = "matched";
pub(crate) const BROAD_SCOPE: &str = "broad_scope";
pub(crate) const BROKEN_EVIDENCE_LINK: &str = "broken_evidence_link";
pub(crate) const WEAK_EVIDENCE_REFERENCE: &str = "weak_evidence_reference";
pub(crate) const MIRROR_DIVERGENCE: &str = "mirror_divergence";

pub(crate) const WORK_ITEM_KINDS: &[&str] = &[
    NEW_UNRECEIPTED_FINDING,
    OCCURRENCE_LIMIT_EXCEEDED,
    OCCURRENCE_HEADROOM,
    EXPIRED_ALLOW,
    STALE_ALLOW,
    AMBIGUOUS_SELECTOR,
    UNSAFE_MISSING_EVIDENCE,
    MISSING_EVIDENCE,
    MISSING_REQUIRED_FIELD,
    INVALID_SELECTOR,
    BASELINE_DEBT,
    REVIEW_DUE,
    MATCHED,
    BROAD_SCOPE,
    BROKEN_EVIDENCE_LINK,
    WEAK_EVIDENCE_REFERENCE,
    MIRROR_DIVERGENCE,
];

pub(crate) fn parse_work_item_kind_filter(value: &str) -> Result<String, String> {
    let canonical = value.replace('-', "_");
    if WORK_ITEM_KINDS.iter().any(|kind| *kind == canonical) {
        return Ok(canonical);
    }
    Err(format!(
        "unknown work item kind `{value}`; supported kinds: {}",
        WORK_ITEM_KINDS.join(", ")
    ))
}

#[cfg(test)]
#[path = "worklist_item_kind_tests.rs"]
mod tests;
