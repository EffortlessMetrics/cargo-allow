//! Lifecycle cadence artifact model (#4239): the typed V1 result behind the
//! read-only `cadence` command, the lifecycle counterpart of the worklist
//! artifact.
//!
//! Cadence law: the classification is a pure function of (policy, as_of).
//! Same inputs produce the same rows, byte-identical JSON, and the same
//! renderings on every run. Every policy entry lands in exactly one of six
//! classes, first-match-wins at `as_of`, and every lifecycle boundary
//! predicate is REUSED from the crate that owns it — the classification
//! itself lives in the `cargo-allow` command crate (which already depends on
//! `allow-match` and `allow-policy`), so no calendar or boundary law is
//! duplicated here:
//!
//! 1. `invalid` — the entry fails the policy loader's own lifecycle law,
//!    `allow_policy::lifecycle::validate_lifecycle`, reused verbatim: an
//!    unparseable `expires`/`review_after`/`created` date, a descending
//!    date order, or a `baseline_debt` expiry outside its allowed range.
//!    Loaded policies can never carry these — the loader rejects them at
//!    parse — so a CLI cadence run fails loudly on such a policy and this
//!    class is typed defense-in-depth for in-memory entries (#1804
//!    fail-closed law).
//! 2. `expired` — the match-engine
//!    `allow_match::lifecycle::entry_is_expired` predicate, reused
//!    verbatim: `expires` present, parseable, and STRICTLY before `as_of`
//!    (`expires = "never"` never expires). The expires day itself is
//!    therefore NOT expired; it is `expiring` with zero days remaining.
//!    This preserves the pre-existing `<` versus `<=` boundary asymmetry
//!    recorded in #2008: the match engine owns the classification, so the
//!    cadence must not disagree with it.
//! 3. `expiring` — `expires` within [`EXPIRING_SOON_DAYS`] days of
//!    `as_of` (inclusive of the expires day itself).
//! 4. `review_overdue` — the match-engine
//!    `allow_match::lifecycle::entry_review_is_due` predicate, reused
//!    verbatim: `review_after` reached, INCLUSIVE of the deadline day
//!    (`review_after <= as_of`, #2008).
//! 5. `review_due_soon` — `review_after` within [`REVIEW_DUE_SOON_DAYS`]
//!    days after `as_of`.
//! 6. `current` — everything else.
//!
//! Claim boundary: this surface schedules owner work on the default
//! branch. It never feeds `check`/`diff` verdicts, never mutates policy,
//! receipts, or candidate evaluation state, and never extends an expiry.
//! It is the lifecycle counterpart of the worklist command, not a gate.

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::text::markdown_cell;
use crate::{CADENCE_SCHEMA_ID, CADENCE_SCHEMA_VERSION, InventoryContext};
use allow_core::{AllowEntry, SimpleDate};

/// Review horizon in days: `review_after` within this many days after
/// `as_of` is `review_due_soon`. House value; no prior repo convention
/// defined a due-soon window (#4239).
pub const REVIEW_DUE_SOON_DAYS: i64 = 14;

/// Expiry horizon in days: `expires` within this many days after `as_of`
/// is `expiring`. Shares the house 14-day convention with the review
/// horizon; the two stay named separately so they can drift deliberately.
pub const EXPIRING_SOON_DAYS: i64 = 14;

/// Prose claim boundary for the cadence surface. The JSON artifact carries
/// the shared flag-array claim boundary like every command artifact; this
/// prose states the cadence-specific law in human output and docs.
pub const CADENCE_CLAIM_BOUNDARY: &str = "A read-only lifecycle cadence classification of every policy entry at one explicit as-of date. It schedules owner review work; it never feeds check or diff verdicts, never mutates policy, receipts, or candidate evaluation state, and never extends an expiry.";

/// Closed lifecycle class vocabulary. Every policy entry lands in exactly
/// one class; the row's `days_remaining` and `driving_date` name the date
/// that produced the class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleCadenceClassV1 {
    Current,
    ReviewDueSoon,
    ReviewOverdue,
    Expiring,
    Expired,
    Invalid,
}

impl LifecycleCadenceClassV1 {
    /// Every class, in classification precedence order after the
    /// `current` fallback.
    pub const ALL: [LifecycleCadenceClassV1; 6] = [
        Self::Current,
        Self::ReviewDueSoon,
        Self::ReviewOverdue,
        Self::Expiring,
        Self::Expired,
        Self::Invalid,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::ReviewDueSoon => "review_due_soon",
            Self::ReviewOverdue => "review_overdue",
            Self::Expiring => "expiring",
            Self::Expired => "expired",
            Self::Invalid => "invalid",
        }
    }

    /// Stable render order: fail-closed and actionable classes first,
    /// `current` last. Rows sort by (rank, allow_id) so a fixed
    /// (policy, as_of) always renders byte-identically.
    #[must_use]
    pub const fn rank(self) -> u8 {
        match self {
            Self::Invalid => 0,
            Self::Expired => 1,
            Self::Expiring => 2,
            Self::ReviewOverdue => 3,
            Self::ReviewDueSoon => 4,
            Self::Current => 5,
        }
    }

    /// Required operator disposition per class. One canonical mapping;
    /// row text and the human renderer read this single source.
    #[must_use]
    pub const fn required_disposition(self) -> &'static str {
        match self {
            Self::Current => "none",
            Self::ReviewDueSoon => "schedule review",
            Self::ReviewOverdue => "review now or narrow",
            Self::Expiring => "renew, narrow, or plan removal",
            Self::Expired => "renew via reviewed commit or remove",
            Self::Invalid => "fix malformed lifecycle date (fail-closed)",
        }
    }
}

impl fmt::Display for LifecycleCadenceClassV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.as_str())
    }
}

/// Which lifecycle date drove the row's classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleCadenceDrivingDateV1 {
    Expires,
    ReviewAfter,
    None,
}

impl LifecycleCadenceDrivingDateV1 {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Expires => "expires",
            Self::ReviewAfter => "review_after",
            Self::None => "none",
        }
    }
}

/// Whether the as-of date came from the operator or from the ambient UTC
/// day. Explicit inputs are the deterministic mode; the ambient default is
/// documented as approximate UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleCadenceAsOfSourceV1 {
    Explicit,
    AmbientUtcDay,
}

impl LifecycleCadenceAsOfSourceV1 {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Explicit => "explicit",
            Self::AmbientUtcDay => "ambient_utc_day",
        }
    }
}

/// The two named horizons, echoed in the artifact so consumers can read
/// the classification windows without trusting compiled defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LifecycleCadenceHorizonsV1 {
    pub review_due_soon_days: i64,
    pub expiring_soon_days: i64,
}

/// Source-tree inventory context for the run, mirroring the shared
/// inventory vocabulary of the other command artifacts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LifecycleCadenceInventoryV1 {
    pub scope: String,
    pub scanner: String,
    pub source: String,
    pub root: Option<String>,
    pub files_scanned: Option<usize>,
    pub empty_git_tracked: bool,
    pub completeness: Option<String>,
}

impl From<&InventoryContext<'_>> for LifecycleCadenceInventoryV1 {
    fn from(inventory: &InventoryContext<'_>) -> Self {
        Self {
            scope: inventory.scope.to_string(),
            scanner: inventory.scanner.to_string(),
            source: inventory.source.to_string(),
            root: inventory.root.map(str::to_string),
            files_scanned: inventory.files_scanned,
            empty_git_tracked: inventory.empty_git_tracked,
            completeness: inventory.completeness.map(str::to_string),
        }
    }
}

/// One classified policy entry. The row retains the durable identity an
/// owner needs to act: entry id, class, owner, policy classification
/// (including the `baseline_debt` marker), path/glob source identity, a
/// deterministic selector summary, evidence references, the raw lifecycle
/// dates, the signed day delta to the driving date, which date drove the
/// class, and the required disposition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LifecycleCadenceRowV1 {
    pub allow_id: String,
    pub class: LifecycleCadenceClassV1,
    pub owner: String,
    pub classification: String,
    pub source_path: Option<String>,
    pub source_glob: Option<String>,
    pub selector_summary: String,
    pub evidence_refs: Vec<String>,
    pub review_after: Option<String>,
    pub expires: Option<String>,
    /// Signed days from `as_of` to the driving date (review rows:
    /// `review_after - as_of`; expiry rows: `expires - as_of`). Negative
    /// means overdue. `None` when no date drove the classification
    /// (`current` fallback or `invalid`).
    pub days_remaining: Option<i64>,
    pub driving_date: LifecycleCadenceDrivingDateV1,
    pub required_disposition: String,
}

/// Per-class counts over the classified rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LifecycleCadenceSummaryV1 {
    pub total_entries: usize,
    pub current: usize,
    pub review_due_soon: usize,
    pub review_overdue: usize,
    pub expiring: usize,
    pub expired: usize,
    pub invalid: usize,
}

/// The one semantic cadence result. Both the JSON artifact and the human
/// rendering derive from this single typed value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LifecycleCadenceReportV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub tool: String,
    pub command: String,
    pub as_of: String,
    pub as_of_source: LifecycleCadenceAsOfSourceV1,
    pub horizons: LifecycleCadenceHorizonsV1,
    pub policy_path: String,
    pub inventory: LifecycleCadenceInventoryV1,
    pub rows: Vec<LifecycleCadenceRowV1>,
    pub summary: LifecycleCadenceSummaryV1,
    pub claim_boundary: Vec<String>,
    pub scanner_limitations: Vec<String>,
}

/// Build one cadence row for an entry already classified as `class` at
/// `as_of`. The CLASS DECISION lives in the `cargo-allow` command crate,
/// where the match-engine and loader lifecycle predicates are reused
/// verbatim; this constructor owns everything the row retains: identity
/// fields, the signed day delta to the driving date, and the required
/// disposition. All date math goes through `allow_core::SimpleDate`; no
/// second calendar implementation lives here.
///
/// The class is trusted input: a `days_remaining` fallback of zero (when a
/// driving date is unexpectedly absent for a date-driven class) is
/// unreachable defense for a broken invariant rather than a silent
/// misclassification path.
#[must_use]
pub fn lifecycle_cadence_row(
    entry: &AllowEntry,
    class: LifecycleCadenceClassV1,
    as_of: SimpleDate,
) -> LifecycleCadenceRowV1 {
    let (days_remaining, driving_date) = driving_delta(entry, as_of, class);
    LifecycleCadenceRowV1 {
        allow_id: entry.id.clone(),
        class,
        owner: entry.owner.clone(),
        classification: entry.classification.clone(),
        source_path: entry
            .path
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned()),
        source_glob: entry.glob.clone().or_else(|| entry.selector.glob.clone()),
        selector_summary: selector_summary(entry),
        evidence_refs: entry.evidence.clone(),
        review_after: entry.lifecycle.review_after.clone(),
        expires: entry.lifecycle.expires.clone(),
        days_remaining,
        driving_date,
        required_disposition: class.required_disposition().to_string(),
    }
}

/// Parse `expires`, honoring the documented immortal form `never`
/// (mirrors `entry_is_expired`).
fn expires_date(entry: &AllowEntry) -> Option<SimpleDate> {
    match entry.lifecycle.expires.as_deref() {
        Some("never") => None,
        Some(expires) => SimpleDate::parse(expires),
        None => None,
    }
}

/// Parse `review_after`.
fn review_after_date(entry: &AllowEntry) -> Option<SimpleDate> {
    entry
        .lifecycle
        .review_after
        .as_deref()
        .and_then(SimpleDate::parse)
}

/// Signed day delta to the date that drove the classification.
fn driving_delta(
    entry: &AllowEntry,
    as_of: SimpleDate,
    class: LifecycleCadenceClassV1,
) -> (Option<i64>, LifecycleCadenceDrivingDateV1) {
    match class {
        LifecycleCadenceClassV1::Expired | LifecycleCadenceClassV1::Expiring => {
            let delta = expires_date(entry)
                .map(|date| as_of.days_until(date))
                .unwrap_or(0);
            (Some(delta), LifecycleCadenceDrivingDateV1::Expires)
        }
        LifecycleCadenceClassV1::ReviewOverdue | LifecycleCadenceClassV1::ReviewDueSoon => {
            let delta = review_after_date(entry)
                .map(|date| as_of.days_until(date))
                .unwrap_or(0);
            (Some(delta), LifecycleCadenceDrivingDateV1::ReviewAfter)
        }
        LifecycleCadenceClassV1::Current | LifecycleCadenceClassV1::Invalid => {
            (None, LifecycleCadenceDrivingDateV1::None)
        }
    }
}

/// Deterministic selector identity summary for one entry: governed kind,
/// optional family, then the selector's identity fields in fixed order.
/// Text is source-derived and travels in artifacts like every other
/// identity field.
fn selector_summary(entry: &AllowEntry) -> String {
    let mut parts = vec![entry.kind.as_str().to_string()];
    if let Some(family) = entry.family.as_deref() {
        parts.push(family.to_string());
    }
    let selector = &entry.selector;
    for (label, value) in [
        ("ast_kind", selector.ast_kind.as_deref()),
        ("symbol", selector.symbol.as_deref()),
        ("callee", selector.callee.as_deref()),
        ("macro_name", selector.macro_name.as_deref()),
        ("lint", selector.lint.as_deref()),
        ("container", selector.container.as_deref()),
    ] {
        if let Some(value) = value {
            parts.push(format!("{label}={value}"));
        }
    }
    parts.join("/")
}

/// Order rows deterministically: class precedence rank, then allow id. A
/// fixed (policy, as_of) always yields this exact order.
#[must_use]
pub fn sort_lifecycle_cadence_rows(
    mut rows: Vec<LifecycleCadenceRowV1>,
) -> Vec<LifecycleCadenceRowV1> {
    rows.sort_by(|left, right| {
        left.class
            .rank()
            .cmp(&right.class.rank())
            .then_with(|| left.allow_id.cmp(&right.allow_id))
    });
    rows
}

/// Summary counts for a classified row set.
#[must_use]
pub fn lifecycle_cadence_summary(rows: &[LifecycleCadenceRowV1]) -> LifecycleCadenceSummaryV1 {
    let mut summary = LifecycleCadenceSummaryV1 {
        total_entries: rows.len(),
        current: 0,
        review_due_soon: 0,
        review_overdue: 0,
        expiring: 0,
        expired: 0,
        invalid: 0,
    };
    for row in rows {
        match row.class {
            LifecycleCadenceClassV1::Current => summary.current += 1,
            LifecycleCadenceClassV1::ReviewDueSoon => summary.review_due_soon += 1,
            LifecycleCadenceClassV1::ReviewOverdue => summary.review_overdue += 1,
            LifecycleCadenceClassV1::Expiring => summary.expiring += 1,
            LifecycleCadenceClassV1::Expired => summary.expired += 1,
            LifecycleCadenceClassV1::Invalid => summary.invalid += 1,
        }
    }
    summary
}

/// Render the one semantic result as the cadence JSON artifact.
///
/// # Errors
/// Returns the serde error when serialization fails; the caller maps it
/// to an artifact failure.
pub fn render_lifecycle_cadence_report_v1(
    report: &LifecycleCadenceReportV1,
) -> Result<String, serde_json::Error> {
    let mut json = serde_json::to_string_pretty(report)?;
    json.push('\n');
    Ok(json)
}

/// Render the human cadence view from the same typed result the JSON
/// artifact renders, so the two renderings cannot disagree.
#[must_use]
pub fn render_lifecycle_cadence_report_human(report: &LifecycleCadenceReportV1) -> String {
    let mut out = String::new();
    out.push_str("cargo-allow cadence\n\n");
    out.push_str(&format!(
        "as_of: {} ({})\n",
        report.as_of,
        report.as_of_source.as_str()
    ));
    out.push_str(&format!("policy: {}\n", report.policy_path));
    out.push_str(&format!(
        "horizons: review_due_soon within {} days; expiring within {} days\n",
        report.horizons.review_due_soon_days, report.horizons.expiring_soon_days
    ));
    out.push_str(&format!(
        "inventory: {}/{} via {}{}\n",
        report.inventory.scope,
        report.inventory.scanner,
        report.inventory.source,
        report
            .inventory
            .files_scanned
            .map(|files| format!("; files scanned: {files}"))
            .unwrap_or_default()
    ));
    let summary = &report.summary;
    out.push_str(&format!("\nentries: {}\n", summary.total_entries));
    for class in LifecycleCadenceClassV1::ALL {
        let count = class_count(summary, class);
        out.push_str(&format!("  {:<16} {count}\n", class.as_str()));
    }
    if report.rows.is_empty() {
        out.push_str("\n(no allow entries are configured)\n");
    } else {
        out.push_str("\nentries:\n");
        for row in &report.rows {
            out.push_str(&format!("- [{}] {}\n", row.class.as_str(), row.allow_id));
            out.push_str(&format!("  owner: {}\n", row.owner));
            if let Some(path) = row.source_path.as_deref() {
                out.push_str(&format!("  path: {path}\n"));
            }
            if let Some(glob) = row.source_glob.as_deref() {
                out.push_str(&format!("  glob: {glob}\n"));
            }
            if let Some(review_after) = row.review_after.as_deref() {
                out.push_str(&format!("  review_after: {review_after}\n"));
            }
            if let Some(expires) = row.expires.as_deref() {
                out.push_str(&format!("  expires: {expires}\n"));
            }
            if let Some(days) = row.days_remaining {
                let delta = if days < 0 {
                    format!("{} days overdue", days.saturating_neg())
                } else {
                    format!("{days} days remaining")
                };
                out.push_str(&format!(
                    "  driving date: {} ({delta})\n",
                    row.driving_date.as_str()
                ));
            }
            out.push_str(&format!(
                "  required disposition: {}\n",
                row.required_disposition
            ));
        }
    }
    out.push_str(&format!("\nClaim boundary: {CADENCE_CLAIM_BOUNDARY}\n"));
    out
}

/// Render the markdown cadence view from the same typed result the JSON
/// artifact and the human view render, so all three renderings cannot
/// disagree. Policy-derived text passes through [`markdown_cell`] so table
/// structure survives owner, path, and evidence content.
#[must_use]
pub fn render_lifecycle_cadence_report_markdown(report: &LifecycleCadenceReportV1) -> String {
    let mut out = String::new();
    out.push_str("# cargo-allow cadence\n\n");
    out.push_str(
        "Read-only lifecycle classification of every policy entry at one as-of \
         date. This schedules owner review work; it never feeds check or diff \
         verdicts and never mutates policy.\n\n",
    );
    out.push_str(&format!(
        "- as_of: `{}` ({})\n",
        report.as_of,
        report.as_of_source.as_str()
    ));
    out.push_str(&format!(
        "- policy: `{}`\n",
        markdown_cell(&report.policy_path)
    ));
    out.push_str(&format!(
        "- horizons: review_due_soon within {} days; expiring within {} days\n",
        report.horizons.review_due_soon_days, report.horizons.expiring_soon_days
    ));
    out.push_str("\n## Summary\n\n| Class | Entries |\n|---|---:|\n");
    for class in LifecycleCadenceClassV1::ALL {
        out.push_str(&format!(
            "| {} | {} |\n",
            class.as_str(),
            class_count(&report.summary, class)
        ));
    }
    out.push_str(&format!(
        "| total_entries | {} |\n",
        report.summary.total_entries
    ));
    out.push_str("\n## Rows\n\n");
    if report.rows.is_empty() {
        out.push_str("No allow entries are configured.\n");
    } else {
        out.push_str(
            "| Allow ID | Class | Owner | Days | Driving date | Required disposition |\n\
             |---|---|---|---:|---|---|\n",
        );
        for row in &report.rows {
            let days = row
                .days_remaining
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string());
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} |\n",
                markdown_cell(&row.allow_id),
                row.class.as_str(),
                markdown_cell(&row.owner),
                days,
                row.driving_date.as_str(),
                markdown_cell(&row.required_disposition),
            ));
        }
    }
    out.push_str(&format!(
        "\n## Claim boundary\n\n{}\n",
        CADENCE_CLAIM_BOUNDARY
    ));
    out
}

fn class_count(summary: &LifecycleCadenceSummaryV1, class: LifecycleCadenceClassV1) -> usize {
    match class {
        LifecycleCadenceClassV1::Current => summary.current,
        LifecycleCadenceClassV1::ReviewDueSoon => summary.review_due_soon,
        LifecycleCadenceClassV1::ReviewOverdue => summary.review_overdue,
        LifecycleCadenceClassV1::Expiring => summary.expiring,
        LifecycleCadenceClassV1::Expired => summary.expired,
        LifecycleCadenceClassV1::Invalid => summary.invalid,
    }
}

/// Assemble the typed report from classified rows and run context. Claim
/// boundary and scanner limitation flags come from the shared command
/// artifact contract so the cadence artifact states the same boundary as
/// every other command artifact.
#[must_use]
pub fn lifecycle_cadence_report(
    as_of: SimpleDate,
    as_of_source: LifecycleCadenceAsOfSourceV1,
    policy_path: &str,
    inventory: LifecycleCadenceInventoryV1,
    rows: Vec<LifecycleCadenceRowV1>,
) -> LifecycleCadenceReportV1 {
    let summary = lifecycle_cadence_summary(&rows);
    LifecycleCadenceReportV1 {
        schema_id: CADENCE_SCHEMA_ID.to_string(),
        schema_version: CADENCE_SCHEMA_VERSION,
        tool: "cargo-allow".to_string(),
        command: "cadence".to_string(),
        as_of: as_of.to_string(),
        as_of_source,
        horizons: LifecycleCadenceHorizonsV1 {
            review_due_soon_days: REVIEW_DUE_SOON_DAYS,
            expiring_soon_days: EXPIRING_SOON_DAYS,
        },
        policy_path: policy_path.to_string(),
        inventory,
        rows,
        summary,
        claim_boundary: crate::CLAIM_BOUNDARY
            .iter()
            .map(|flag| (*flag).to_string())
            .collect(),
        scanner_limitations: crate::SCANNER_LIMITATIONS
            .iter()
            .map(|flag| (*flag).to_string())
            .collect(),
    }
}
