use allow_core::{CargoAllowResult, Finding, MatchOutcome};
use std::path::Path;

use crate::{InventoryFacts, OutputFormat, source_tree_root_text, write_file};

pub(crate) struct ReportRenderArgs<'a> {
    pub(crate) command: &'a str,
    pub(crate) format: OutputFormat,
    pub(crate) baseline_debt_entries: usize,
    pub(crate) findings: &'a [Finding],
    pub(crate) outcomes: &'a [MatchOutcome],
    pub(crate) failed: bool,
    pub(crate) output: Option<&'a Path>,
    pub(crate) root: &'a Path,
    pub(crate) inventory_facts: InventoryFacts,
}

pub(crate) fn print_report(args: ReportRenderArgs<'_>) -> CargoAllowResult<()> {
    let root_text = source_tree_root_text(args.root);
    let context = allow_report::ReportContext {
        inventory_source: args.inventory_facts.source.as_str(),
        source_tree_root: Some(&root_text),
        inventory_files: args.inventory_facts.files_scanned,
        baseline_debt_entries: Some(args.baseline_debt_entries),
    };
    let text = match args.format {
        OutputFormat::Human => allow_report::render_human_with_context(
            args.command,
            args.findings,
            args.outcomes,
            args.failed,
            context,
        ),
        OutputFormat::Json => allow_report::render_json_with_context(
            args.command,
            args.findings,
            args.outcomes,
            args.failed,
            context,
        ),
        OutputFormat::Html => allow_report::render_html_with_context(
            args.command,
            args.findings,
            args.outcomes,
            args.failed,
            context,
        ),
        OutputFormat::Sarif => allow_report::render_sarif_with_context(
            args.command,
            args.findings,
            args.outcomes,
            args.failed,
            context,
        ),
        OutputFormat::Markdown => allow_report::render_markdown_with_context(
            args.command,
            args.findings,
            args.outcomes,
            args.failed,
            context,
        ),
    };
    if let Some(path) = args.output {
        write_file(path, &text)?;
    } else {
        println!("{text}");
    }
    Ok(())
}
