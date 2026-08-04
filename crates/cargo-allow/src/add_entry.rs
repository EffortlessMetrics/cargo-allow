use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowErrorKind, CargoAllowResult, Finding,
    FindingKind, LastSeen, Lifecycle, MatchStatus, Selector, SimpleDate, normalize_path,
};
use allow_match::score_match;
use std::path::Path;

use crate::{KindFilter, selector_from_finding};

pub(crate) fn select_add_finding<'a>(
    findings: &'a [Finding],
    kind: KindFilter,
    path: &Path,
    line: u32,
) -> CargoAllowResult<(usize, &'a Finding)> {
    let normalized_path = normalize_path(path);
    let mut candidates = findings
        .iter()
        .enumerate()
        .filter(|(_, finding)| kind.matches_finding(finding))
        .filter(|(_, finding)| normalize_path(&finding.path) == normalized_path)
        .filter_map(|(index, finding)| {
            finding
                .span
                .as_ref()
                .map(|span| (span.line.abs_diff(line), index, finding))
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(distance, _, finding)| (*distance, normalize_path(&finding.path)));
    let Some((distance, index, finding)) = candidates.first().copied() else {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            format!(
                "no current {} finding found near {}:{}; \
                 run `cargo-allow check --kind {} --format json` to list current findings, \
                 or use `--include-untracked` if the source file is not git-tracked",
                kind.kind, normalized_path, line, kind.kind
            ),
        ));
    };
    let tied = candidates
        .iter()
        .filter(|(candidate_distance, _, _)| *candidate_distance == distance)
        .count();
    if tied > 1 {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            format!(
                "ambiguous add request: {tied} findings are equally near {}:{}; \
                 specify an exact --line that matches only one finding",
                normalized_path, line
            ),
        ));
    }
    Ok((index, finding))
}

pub(super) fn ensure_addable_outcome(status: MatchStatus) -> CargoAllowResult<()> {
    if status == MatchStatus::New {
        return Ok(());
    }
    Err(CargoAllowError::with_kind(
        CargoAllowErrorKind::Usage,
        format!(
            "selected finding is already receipted or blocked with status `{}`; use list or explain before editing policy",
            status.as_str()
        ),
    ))
}

pub(super) struct AddEntryRequest<'a> {
    pub finding: &'a Finding,
    pub id: String,
    pub owner: String,
    pub classification: String,
    pub reason: String,
    pub evidence: Vec<String>,
    pub review_after: String,
    pub expires: Option<String>,
}

pub(super) fn allow_entry_from_finding(request: AddEntryRequest<'_>) -> AllowEntry {
    let selector = selector_from_finding(request.finding);
    AllowEntry {
        id: request.id,
        kind: request.finding.kind,
        family: request.finding.family.clone(),
        path: Some(request.finding.path.clone()),
        glob: None,
        owner: request.owner,
        classification: request.classification,
        reason: request.reason,
        evidence: request.evidence,
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: Some(SimpleDate::today_utc_approx().to_string()),
            review_after: Some(request.review_after),
            expires: request.expires,
        },
        selector,
        last_seen: request.finding.span.as_ref().map(|s| LastSeen {
            line: s.line,
            column: s.column,
        }),
    }
}

pub(super) struct AddBroadRequest {
    pub id: String,
    pub kind: FindingKind,
    pub family: Option<String>,
    pub callee: Option<String>,
    pub glob: String,
    pub owner: String,
    pub classification: String,
    pub reason: String,
    pub evidence: Vec<String>,
    pub review_after: String,
    pub expires: Option<String>,
}

/// Build a broad-scope allow entry: a selector keyed on `glob` (+ optional
/// `callee`) with **no** `normalized_snippet_hash`, so it matches every current
/// in-scope occurrence. The caller pins the current in-scope count as
/// `occurrence_limit` so the entry is a ratchet floor, not an unlimited waiver
/// (#2056).
pub(super) fn allow_entry_broad(request: AddBroadRequest) -> AllowEntry {
    let selector = Selector {
        callee: request.callee,
        glob: Some(request.glob.clone()),
        ..Selector::default()
    };
    AllowEntry {
        id: request.id,
        kind: request.kind,
        family: request.family,
        path: None,
        glob: Some(request.glob),
        owner: request.owner,
        classification: request.classification,
        reason: request.reason,
        evidence: request.evidence,
        links: Vec::new(),
        // Pinned by the caller via `count_in_scope_findings`; None here is only
        // a default that `cmd_add` overwrites before validation.
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: Some(SimpleDate::today_utc_approx().to_string()),
            review_after: Some(request.review_after),
            expires: request.expires,
        },
        selector,
        last_seen: None,
    }
}

/// Count how many current findings match `entry`'s selector, using the same
/// match test `evaluate` applies (`score_match` returns `Some` iff every hard
/// gate — kind, family, path/glob, structural identity, exact selector fields
/// — passes). This is the baseline count pinned as `occurrence_limit` so the
/// N+1th in-scope occurrence fails `check --mode no-new`.
pub(super) fn count_in_scope_findings(findings: &[Finding], entry: &AllowEntry) -> u32 {
    findings
        .iter()
        .filter(|finding| score_match(entry, finding).is_some())
        .count() as u32
}

pub(super) fn next_allow_id(cfg: &AllowConfig) -> String {
    let mut index = cfg.allow.len() + 1;
    loop {
        let candidate = format!("allow-{index:04}");
        if !cfg.allow.iter().any(|entry| entry.id == candidate) {
            return candidate;
        }
        index += 1;
    }
}

#[cfg(test)]
#[path = "add_entry_tests.rs"]
mod tests;
