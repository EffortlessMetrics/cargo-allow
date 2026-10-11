use allow_core::{AllowEntry, CargoAllowResult, SimpleDate};
use allow_inventory::{InventoryOptions, inventory, resolve_source_tree_root};
use allow_match::lifecycle::{entry_is_expired, entry_review_is_due};
use allow_policy::federation::evaluate_source_exception_policy;
use allow_policy::lifecycle::validate_lifecycle;
use allow_report::{
    EXPIRING_SOON_DAYS, LifecycleCadenceClassV1, LifecycleCadenceRowV1, REVIEW_DUE_SOON_DAYS,
    lifecycle_cadence_row, sort_lifecycle_cadence_rows,
};
use clap::Parser;
use std::path::PathBuf;

use crate::{
    CadenceFormat, EvidenceValidationMode, InventoryFacts, RootArgs, SourceTreeReportContext,
    current_dir, emit_text,
};

/// Read-only lifecycle cadence surface (#4239).
///
/// `cadence` is the lifecycle counterpart of `worklist`: it classifies
/// every policy entry into exactly one lifecycle class at one explicit
/// as-of date and states the required disposition per row. It is a pure
/// read over the policy parse plus a source-tree inventory listing — it
/// shares no code path with check/diff verdict computation, never feeds a
/// gate, and never mutates policy, receipts, or candidate evaluation
/// state. Missing `--as-of` uses the ambient UTC day
/// (`SimpleDate::today_utc_approx`); an unparseable `--as-of` value fails
/// the invocation at argument parsing, because the as-of INPUT must be
/// valid (it is entry lifecycle dates that classify as `invalid` rows).
/// Human, JSON, and markdown renderings all derive from the one typed
/// `LifecycleCadenceReportV1` result.
#[derive(Debug, Clone, Parser)]
pub(crate) struct CadenceArgs {
    #[command(flatten)]
    pub(super) root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    pub(crate) config: Option<PathBuf>,
    /// Classification date (YYYY-MM-DD). Defaults to the ambient UTC day.
    #[arg(long, value_parser = parse_as_of_arg, value_name = "YYYY-MM-DD")]
    pub(super) as_of: Option<SimpleDate>,
    /// Output format.
    #[arg(long, value_enum, default_value_t = CadenceFormat::Human)]
    pub(super) format: CadenceFormat,
    /// Write the cadence report to a file instead of stdout.
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
}

/// Strict CLI date parser for `--as-of`. Any value the shared
/// `SimpleDate` grammar rejects fails the invocation loudly (clap exits 2,
/// the same code as every other argument error). This is the input
/// boundary: entry dates classify, input dates reject.
pub(crate) fn parse_as_of_arg(value: &str) -> Result<SimpleDate, String> {
    SimpleDate::parse(value)
        .ok_or_else(|| format!("invalid --as-of date `{value}`; expected YYYY-MM-DD"))
}

/// The cadence class decision, first-match-wins at `as_of`. The contested
/// boundary predicates are the owning crates' own functions, not
/// re-implementations: `invalid` is exactly "the loader would reject this
/// entry" via `allow_policy::lifecycle::validate_lifecycle`, `expired`
/// binds to `allow_match::lifecycle::entry_is_expired` (strict
/// `date < today`, `expires = "never"` never expires) and `review_overdue`
/// to `entry_review_is_due` (inclusive `date <= today`, deliberately
/// preserved so cadence never disagrees with the matcher). All date math
/// goes through `allow_core::SimpleDate`; no second calendar implementation
/// lives here.
#[must_use]
fn classify_lifecycle_class(entry: &AllowEntry, as_of: SimpleDate) -> LifecycleCadenceClassV1 {
    // Loader law, reused verbatim: an entry the policy loader would reject
    // is `invalid`, fail-closed. Loaded policies can never reach here with
    // a failing validate_lifecycle, so this is typed defense-in-depth.
    if validate_lifecycle(entry).is_err() {
        return LifecycleCadenceClassV1::Invalid;
    }
    // Match-engine boundaries, reused verbatim. After the loader
    // check both lifecycle dates parse, so the helpers' unparseable
    // fail-safes are unreachable defense on this path.
    if entry_is_expired(entry, as_of) {
        // Strict `<`: the expires day itself is still `expiring`.
        return LifecycleCadenceClassV1::Expired;
    }
    if let Some(expires) = parse_expires(entry)
        && as_of.days_until(expires) <= EXPIRING_SOON_DAYS
    {
        return LifecycleCadenceClassV1::Expiring;
    }
    if entry_review_is_due(entry, as_of) {
        // Inclusive `<=`: the review deadline day itself is overdue.
        return LifecycleCadenceClassV1::ReviewOverdue;
    }
    if let Some(review_after) = parse_review_after(entry)
        && as_of.days_until(review_after) <= REVIEW_DUE_SOON_DAYS
    {
        return LifecycleCadenceClassV1::ReviewDueSoon;
    }
    LifecycleCadenceClassV1::Current
}

/// Parse `expires`, honoring the documented immortal form `never`
/// (mirrors `entry_is_expired`).
fn parse_expires(entry: &AllowEntry) -> Option<SimpleDate> {
    match entry.lifecycle.expires.as_deref() {
        Some("never") => None,
        Some(expires) => SimpleDate::parse(expires),
        None => None,
    }
}

/// Parse `review_after`.
fn parse_review_after(entry: &AllowEntry) -> Option<SimpleDate> {
    entry
        .lifecycle
        .review_after
        .as_deref()
        .and_then(SimpleDate::parse)
}

/// Classify every entry and order rows deterministically: class
/// precedence rank, then allow id (via the shared
/// `sort_lifecycle_cadence_rows`). A fixed (policy, as_of) always yields
/// this exact order.
#[must_use]
pub(crate) fn lifecycle_cadence_rows(
    entries: &[AllowEntry],
    as_of: SimpleDate,
) -> Vec<LifecycleCadenceRowV1> {
    let rows = entries
        .iter()
        .map(|entry| {
            let class = classify_lifecycle_class(entry, as_of);
            lifecycle_cadence_row(entry, class, as_of)
        })
        .collect();
    sort_lifecycle_cadence_rows(rows)
}

pub(crate) fn cmd_cadence(args: &CadenceArgs) -> CargoAllowResult<()> {
    let (as_of, as_of_source) = match args.as_of {
        Some(as_of) => (as_of, allow_report::LifecycleCadenceAsOfSourceV1::Explicit),
        None => (
            SimpleDate::today_utc_approx(),
            allow_report::LifecycleCadenceAsOfSourceV1::AmbientUtcDay,
        ),
    };
    // Policy-only read: canonical policy selection plus the standard
    // lifecycle-validating parse. No findings scan, no evaluation, and no
    // candidate-state read happens anywhere below.
    let cwd = current_dir()?;
    let root = resolve_source_tree_root(args.root.root.as_deref(), cwd)?;
    let (policy_path, _federation) =
        evaluate_source_exception_policy(&root, args.config.as_deref())?;
    let (cfg, _policy_digest) = crate::policy_config::load_policy_at_path_with_digest(
        policy_path.clone(),
        EvidenceValidationMode::ReportOnly,
    )?;
    // The inventory listing feeds the artifact's shared inventory block
    // only; nothing scans Rust sources and nothing executes.
    let inventory = inventory(
        &root,
        &crate::world::inventory_options_with_tool_cache_ignore(InventoryOptions {
            ignored: cfg.workspace.ignored.clone(),
            generated: cfg.workspace.generated.clone(),
            include_untracked: false,
        }),
    )?;
    let inventory_facts = InventoryFacts::scanned_inventory(&inventory);
    let source_context = SourceTreeReportContext::new(&root, inventory_facts);
    let report = allow_report::lifecycle_cadence_report(
        as_of,
        as_of_source,
        // House identity-field law (#3180): strip the Win32 verbatim prefix
        // and emit the forward-slash form like every other artifact path.
        &allow_core::strip_win32_verbatim_prefix(&policy_path.display().to_string()),
        allow_report::LifecycleCadenceInventoryV1::from(&source_context.inventory()),
        lifecycle_cadence_rows(&cfg.allow, as_of),
    );
    let text = match args.format {
        CadenceFormat::Json => {
            allow_report::render_lifecycle_cadence_report_v1(&report).map_err(|error| {
                allow_core::CargoAllowError::with_kind(
                    allow_core::CargoAllowErrorKind::Artifact,
                    format!("failed to render cadence JSON: {error}"),
                )
            })?
        }
        CadenceFormat::Human => allow_report::render_lifecycle_cadence_report_human(&report),
        CadenceFormat::Markdown => allow_report::render_lifecycle_cadence_report_markdown(&report),
    };
    emit_text(args.output.as_deref(), &text)?;
    Ok(())
}

#[cfg(test)]
pub(crate) fn sample_cadence_json_for_contract_test() -> String {
    use allow_core::{FindingKind, Lifecycle, Selector};

    let entry = |id: &str, review_after: Option<&str>, expires: Option<&str>, created: &str| {
        allow_core::AllowEntry {
            id: id.to_string(),
            kind: FindingKind::Panic,
            family: Some("unwrap".to_string()),
            path: Some(std::path::PathBuf::from("src/lib.rs")),
            glob: None,
            owner: "core".to_string(),
            classification: "reviewed".to_string(),
            reason: "contract sample".to_string(),
            evidence: vec!["test:sample".to_string()],
            links: Vec::new(),
            occurrence_limit: None,
            lifecycle: Lifecycle {
                created: Some(created.to_string()),
                review_after: review_after.map(str::to_string),
                expires: expires.map(str::to_string),
            },
            selector: Selector {
                ast_kind: Some("call".to_string()),
                callee: Some("unwrap".to_string()),
                ..Selector::default()
            },
            last_seen: None,
        }
    };
    let as_of = SimpleDate::parse("2026-10-01")
        .unwrap_or_else(|| std::panic::panic_any("sample as-of must parse"));
    let entries = vec![
        entry("allow-current", Some("2027-06-01"), None, "2026-06-01"),
        entry("allow-due-soon", Some("2026-10-10"), None, "2026-06-01"),
        entry("allow-overdue", Some("2026-09-01"), None, "2026-06-01"),
        entry("allow-expiring", None, Some("2026-10-08"), "2026-06-01"),
        entry("allow-expired", None, Some("2026-09-01"), "2026-06-01"),
        // The invalid row is reachable only through typed in-memory
        // construction: the policy loader rejects malformed dates at parse.
        entry("allow-invalid", Some("not-a-date"), None, "2026-06-01"),
    ];
    let rows = lifecycle_cadence_rows(&entries, as_of);
    let report = allow_report::lifecycle_cadence_report(
        as_of,
        allow_report::LifecycleCadenceAsOfSourceV1::Explicit,
        "policy/allow.toml",
        allow_report::LifecycleCadenceInventoryV1::from(
            &allow_report::InventoryContext::source_syntax(
                "filesystem_fallback",
                Some("fixtures/source-snapshot"),
                Some(5),
            )
            .with_completeness("complete"),
        ),
        rows,
    );
    allow_report::render_lifecycle_cadence_report_v1(&report)
        .unwrap_or_else(|err| std::panic::panic_any(format!("sample cadence JSON: {err}")))
}

#[cfg(test)]
#[path = "cadence_tests.rs"]
mod tests;
