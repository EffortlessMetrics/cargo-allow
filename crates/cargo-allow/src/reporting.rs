use allow_core::{AllowConfig, CargoAllowResult, Finding, MatchOutcome};
use std::path::Path;

use crate::{InventoryFacts, OutputFormat, emit_text, parse_kind_filter};

pub(crate) struct ReportRenderArgs<'a> {
    pub(crate) command: &'a str,
    pub(crate) format: OutputFormat,
    pub(crate) baseline_debt_entries: usize,
    pub(crate) broken_evidence_links: usize,
    pub(crate) findings: &'a [Finding],
    pub(crate) outcomes: &'a [MatchOutcome],
    pub(crate) failed: bool,
    pub(crate) output: Option<&'a Path>,
    pub(crate) root: &'a Path,
    pub(crate) inventory_facts: InventoryFacts,
}

#[derive(Debug, Clone)]
pub(crate) struct SourceTreeReportContext {
    root: String,
    inventory_facts: InventoryFacts,
}

impl SourceTreeReportContext {
    pub(crate) fn new(root: &Path, inventory_facts: InventoryFacts) -> Self {
        Self {
            root: allow_report::source_tree_path_text(root),
            inventory_facts,
        }
    }

    pub(crate) fn inventory(&self) -> allow_report::InventoryContext<'_> {
        allow_report::InventoryContext::source_syntax(
            self.inventory_source(),
            Some(self.source_tree_root()),
            self.inventory_files(),
        )
    }

    pub(crate) fn report(
        &self,
        baseline_debt_entries: Option<usize>,
    ) -> allow_report::ReportContext<'_> {
        allow_report::ReportContext::source_syntax(
            self.inventory_source(),
            Some(self.source_tree_root()),
            self.inventory_files(),
            baseline_debt_entries,
        )
    }

    pub(crate) fn source_tree_root(&self) -> &str {
        &self.root
    }

    pub(crate) fn inventory_source(&self) -> &str {
        self.inventory_facts.source.as_str()
    }

    pub(crate) fn inventory_files(&self) -> Option<usize> {
        self.inventory_facts.files_scanned
    }
}

pub(crate) fn print_report(args: ReportRenderArgs<'_>) -> CargoAllowResult<()> {
    let source_context = SourceTreeReportContext::new(args.root, args.inventory_facts);
    let mut context = source_context.report(Some(args.baseline_debt_entries));
    context.broken_evidence_links =
        (args.broken_evidence_links > 0).then_some(args.broken_evidence_links);
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
    emit_text(args.output, &text)
}

pub(crate) fn report_config(
    cfg: &AllowConfig,
    kind_filter: Option<&str>,
) -> CargoAllowResult<AllowConfig> {
    let Some(kind) = kind_filter else {
        return Ok(cfg.clone());
    };
    let parsed = parse_kind_filter(kind)?;
    let mut filtered = cfg.clone();
    filtered.allow.retain(|entry| parsed.matches_entry(entry));
    Ok(filtered)
}
