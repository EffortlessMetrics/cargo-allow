use allow_core::{AllowConfig, CargoAllowResult, Finding, MatchOutcome};
use std::collections::BTreeSet;
use std::path::Path;

use crate::evidence_inventory::evidence_reference_diagnostics_for_source_tree;
use crate::{InventoryFacts, OutputFormat, emit_text, parse_kind_filter};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct EvidenceReportSummary {
    pub(crate) policy_missing_evidence_entries: usize,
    pub(crate) broken_evidence_links: usize,
    pub(crate) weak_evidence_references: usize,
}

impl EvidenceReportSummary {
    pub(crate) fn from_policy(root: &Path, cfg: &AllowConfig, outcomes: &[MatchOutcome]) -> Self {
        Self::from_policy_with_source_tree_files(root, cfg, outcomes, None)
    }

    pub(crate) fn from_policy_with_source_tree_files(
        root: &Path,
        cfg: &AllowConfig,
        outcomes: &[MatchOutcome],
        source_tree_files: Option<&BTreeSet<String>>,
    ) -> Self {
        let diagnostics = cfg
            .allow
            .iter()
            .flat_map(|entry| {
                let mut diagnostics =
                    evidence_reference_diagnostics_for_source_tree(root, entry, source_tree_files);
                let mut link_entry = entry.clone();
                link_entry.evidence = entry.links.clone();
                diagnostics.extend(evidence_reference_diagnostics_for_source_tree(
                    root,
                    &link_entry,
                    source_tree_files,
                ));
                diagnostics
            })
            .collect::<Vec<_>>();
        Self {
            policy_missing_evidence_entries: allow_report::matched_policy_missing_evidence_entries(
                cfg, outcomes,
            ),
            broken_evidence_links: diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.status.is_broken_local_link())
                .count(),
            weak_evidence_references: diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.status.is_weak_reference())
                .count(),
        }
    }

    pub(crate) fn has_broken_evidence_links(self) -> bool {
        self.broken_evidence_links > 0
    }

    pub(crate) fn apply_to(self, context: &mut allow_report::ReportContext<'_>) {
        context.broken_evidence_links =
            (self.broken_evidence_links > 0).then_some(self.broken_evidence_links);
        context.weak_evidence_references =
            (self.weak_evidence_references > 0).then_some(self.weak_evidence_references);
        context.policy_missing_evidence_entries = (self.policy_missing_evidence_entries > 0)
            .then_some(self.policy_missing_evidence_entries);
    }
}

pub(crate) struct ReportRenderArgs<'a> {
    pub(crate) command: &'a str,
    pub(crate) format: OutputFormat,
    pub(crate) baseline_debt_entries: usize,
    pub(crate) evidence: EvidenceReportSummary,
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
    args.evidence.apply_to(&mut context);
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
