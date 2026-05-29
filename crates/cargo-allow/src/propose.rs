use allow_core::{CargoAllowResult, MatchStatus};
use allow_match::{CheckMode, evaluate};
use allow_policy::render_policy;

use crate::{SourceTreeReportContext, emit_stderr_text, load_world, write_file_no_overwrite};

#[path = "propose_args.rs"]
mod propose_args;
#[path = "propose_baseline.rs"]
mod propose_baseline;
#[path = "propose_render.rs"]
mod propose_render;
#[path = "propose_types.rs"]
mod propose_types;
pub(crate) use propose_args::ProposeArgs;
use propose_args::ProposeSummaryFormat;
use propose_baseline::{default_baseline_expiry, entry_from_finding};
use propose_render::{render_propose_summary, render_propose_summary_json};
pub(super) use propose_types::ProposeContext;

#[cfg(test)]
use allow_core::{Finding, FindingKind, SimpleDate};
#[cfg(test)]
use propose_baseline::BASELINE_DEBT_DEFAULT_DAYS;

pub(crate) fn cmd_propose(args: &ProposeArgs) -> CargoAllowResult<()> {
    let (root, cfg, findings, inventory_facts) = load_world(
        args.root.root.as_deref(),
        args.config.as_deref(),
        false,
        args.kind.as_deref(),
        args.include_untracked,
    )?;
    let outcomes = evaluate(&cfg, &findings, CheckMode::Audit);
    let mut proposed = cfg.clone();
    let start = proposed.allow.len() + 1;
    let mut proposed_entries = 0;
    let expires = args.expires.clone().unwrap_or_else(default_baseline_expiry);
    for (n, outcome) in outcomes
        .iter()
        .filter(|o| o.status == MatchStatus::New)
        .enumerate()
    {
        if let Some(finding) = outcome.finding_index.and_then(|idx| findings.get(idx)) {
            proposed
                .allow
                .push(entry_from_finding(finding, start + n, &expires));
            proposed_entries += 1;
        }
    }
    let rendered = render_policy(&proposed);
    if let Some(path) = &args.write {
        write_file_no_overwrite(path, &rendered, args.force)?;
    } else {
        println!("{rendered}");
    }
    let source_context = SourceTreeReportContext::new(&root, inventory_facts);
    let context = ProposeContext {
        inventory: source_context.inventory(),
        kind_filter: args.kind.as_deref(),
    };
    let summary = match args.summary_format {
        ProposeSummaryFormat::Human => render_propose_summary(
            findings.len(),
            proposed_entries,
            expires.as_str(),
            args.write.as_deref(),
        ),
        ProposeSummaryFormat::Json => render_propose_summary_json(
            findings.len(),
            proposed_entries,
            expires.as_str(),
            args.write.as_deref(),
            args.force,
            context,
        ),
    };
    emit_stderr_text(args.summary_output.as_deref(), &summary)?;
    Ok(())
}

#[cfg(test)]
pub(crate) fn sample_propose_json_for_contract_test() -> String {
    use std::path::Path;

    render_propose_summary_json(
        12,
        3,
        "2026-08-01",
        Some(Path::new("policy/allow.proposed.toml")),
        true,
        ProposeContext {
            inventory: allow_report::InventoryContext::source_syntax(
                "git_tracked",
                Some("H:/Code/Rust/cargo-allow"),
                Some(51),
            ),
            kind_filter: Some("panic"),
        },
    )
}

#[cfg(test)]
#[path = "propose_tests.rs"]
mod tests;
