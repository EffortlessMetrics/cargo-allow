//! Row-level unused-dependency exception contract (#3909 PR A, schema
//! only).
//!
//! An accepted exception retains the exact package/row identity (class,
//! target, features), ownership, evidence-or-limitation, controlling issue,
//! review dates, selected configuration IDs, and a claim boundary — all
//! validated together. Broad package-set or workspace-wide ignores are
//! structurally inexpressible: the type carries exactly one package name
//! and one manifest row, and no field can hold a package set (#3909
//! negative control 11).

use super::UnusedDependencyDependencyClassV1;
use serde::{Deserialize, Serialize};

/// The substring every exception claim boundary must carry: one product's
/// use never retains another package's dependency, and one package's
/// exception never retains another.
const NON_TRANSFERABILITY_PHRASE: &str = "one package's exception never retains another";

/// A reviewed, row-scoped exception to an unused-dependency finding.
///
/// PR A defines the schema and its validation only; wiring into a durable
/// ledger is later PR work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnusedDependencyExceptionV1 {
    /// Exactly one package; a package set is structurally inexpressible.
    pub package_name: String,
    /// Registry package name of the retained dependency row.
    pub manifest_dependency_name: String,
    pub class: UnusedDependencyDependencyClassV1,
    /// Target spec for target-specific rows.
    pub target: Option<String>,
    pub features_selected: Vec<String>,
    /// Owner accountable for the retention.
    pub owner: String,
    pub reason: String,
    /// Exact use evidence, or the analyzer limitation relied upon.
    pub use_evidence_or_limitation: String,
    /// Issue reference (starts with `#`) controlling removal or expiry.
    pub controlling_issue: String,
    /// Creation date, `YYYY-MM-DD`.
    pub created: String,
    /// Scheduled review date, `YYYY-MM-DD`, on or after `created`.
    pub review_after: String,
    /// Optional expiry date, `YYYY-MM-DD`, on or after `review_after`.
    pub expiry: Option<String>,
    /// Configuration IDs (#3905) the exception applies to; never empty.
    pub selected_configuration_ids: Vec<String>,
    /// Must state non-transferability by containing the phrase
    /// `one package's exception never retains another`.
    pub claim_boundary: String,
}

/// Validate one exception. Laws: non-empty owner/reason, a controlling
/// issue reference starting with `#`, `review_after` parsing as YYYY-MM-DD
/// and on or after `created`, `expiry` (when present) on or after
/// `review_after`, a non-empty configuration selection, and a claim
/// boundary that states non-transferability.
pub fn validate_exception(exception: &UnusedDependencyExceptionV1) -> Result<(), String> {
    if exception.owner.trim().is_empty() {
        return Err("exception owner must be non-empty".to_string());
    }
    if exception.reason.trim().is_empty() {
        return Err("exception reason must be non-empty".to_string());
    }
    if exception.use_evidence_or_limitation.trim().is_empty() {
        return Err("exception must carry use evidence or an analyzer limitation".to_string());
    }
    if !exception.controlling_issue.starts_with('#') {
        return Err(format!(
            "controlling_issue must be an issue reference starting with '#', got {}",
            exception.controlling_issue
        ));
    }
    let created = parse_iso_date(&exception.created)
        .ok_or_else(|| format!("created must parse as YYYY-MM-DD: {}", exception.created))?;
    let review_after = parse_iso_date(&exception.review_after).ok_or_else(|| {
        format!(
            "review_after must parse as YYYY-MM-DD: {}",
            exception.review_after
        )
    })?;
    if review_after < created {
        return Err(format!(
            "review_after {} must be on or after created {}",
            exception.review_after, exception.created
        ));
    }
    if let Some(expiry) = exception.expiry.as_deref() {
        let expiry_date = parse_iso_date(expiry)
            .ok_or_else(|| format!("expiry must parse as YYYY-MM-DD: {expiry}"))?;
        if expiry_date < review_after {
            return Err(format!(
                "expiry {expiry} must be on or after review_after {}",
                exception.review_after
            ));
        }
    }
    if exception.selected_configuration_ids.is_empty() {
        return Err("exception must select at least one configuration ID".to_string());
    }
    if exception
        .selected_configuration_ids
        .iter()
        .any(|configuration_id| configuration_id.trim().is_empty())
    {
        return Err("exception configuration selection carries a blank ID".to_string());
    }
    if !exception
        .claim_boundary
        .contains(NON_TRANSFERABILITY_PHRASE)
    {
        return Err(format!(
            "claim_boundary must state non-transferability by containing: \
             {NON_TRANSFERABILITY_PHRASE}"
        ));
    }
    Ok(())
}

/// Parse a strict `YYYY-MM-DD` date into a comparable triple. No external
/// date dependency: four-digit year, two-digit month (1-12), two-digit day
/// that exists in that month (leap years honored), nothing else. Impossible
/// dates like `2026-02-31` are rejected rather than merely bounded.
fn parse_iso_date(value: &str) -> Option<(u32, u32, u32)> {
    let mut parts = value.split('-');
    let year_text = parts.next()?;
    let month_text = parts.next()?;
    let day_text = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    if year_text.len() != 4
        || !year_text
            .chars()
            .all(|character| character.is_ascii_digit())
    {
        return None;
    }
    let year = year_text.parse::<u32>().ok()?;
    let month = bounded_two_digits(month_text, 1, 12)?;
    let day = bounded_two_digits(day_text, 1, days_in_month(year, month)?)?;
    Some((year, month, day))
}

/// Days in one month of one year, or `None` for a non-leap February 29
/// attempt: the leap rule is divisibility by 4, excluding centuries unless
/// divisible by 400.
fn days_in_month(year: u32, month: u32) -> Option<u32> {
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Some(31),
        4 | 6 | 9 | 11 => Some(30),
        2 if leap => Some(29),
        2 => Some(28),
        _ => None,
    }
}

fn bounded_two_digits(text: &str, minimum: u32, maximum: u32) -> Option<u32> {
    if text.len() != 2 || !text.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    let number = text.parse::<u32>().ok()?;
    if (minimum..=maximum).contains(&number) {
        Some(number)
    } else {
        None
    }
}
