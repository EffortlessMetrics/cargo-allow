use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowResult, Finding, LastSeen, Lifecycle,
    MatchStatus, SimpleDate, normalize_path,
};
use std::path::Path;

use crate::{KindFilter, selector_from_finding};

pub(super) fn select_add_finding<'a>(
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
        return Err(CargoAllowError::new(format!(
            "no current {} finding found near {}:{}",
            kind.kind, normalized_path, line
        )));
    };
    let tied = candidates
        .iter()
        .filter(|(candidate_distance, _, _)| *candidate_distance == distance)
        .count();
    if tied > 1 {
        return Err(CargoAllowError::new(format!(
            "ambiguous add request: {tied} findings are equally near {}:{}",
            normalized_path, line
        )));
    }
    Ok((index, finding))
}

pub(super) fn ensure_addable_outcome(status: MatchStatus) -> CargoAllowResult<()> {
    if status == MatchStatus::New {
        return Ok(());
    }
    Err(CargoAllowError::new(format!(
        "selected finding is already receipted or blocked with status `{}`; use list or explain before editing policy",
        status.as_str()
    )))
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

pub(super) fn next_allow_id(cfg: &AllowConfig) -> String {
    // Start from the max existing numeric suffix + 1 so IDs only ever
    // increase. Previously used cfg.allow.len() + 1, which produced
    // non-monotonic IDs after deletions (filling backwards) and could
    // collide with existing higher IDs (#1820).
    let max_existing = cfg
        .allow
        .iter()
        .filter_map(|entry| {
            entry
                .id
                .strip_prefix("allow-")
                .and_then(|n| n.parse::<u32>().ok())
        })
        .max()
        .unwrap_or(0);
    let mut index = max_existing + 1;
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
