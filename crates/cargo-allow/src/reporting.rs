use allow_core::{AllowConfig, CargoAllowResult, Finding, MatchOutcome};
use std::collections::BTreeSet;
use std::path::Path;

use crate::evidence_inventory::policy_reference_diagnostics_for_source_tree;
use crate::{InventoryFacts, OutputFormat, emit_text, parse_kind_filter};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct EvidenceReportSummary {
    pub(crate) policy_missing_evidence_entries: usize,
    pub(crate) broken_evidence_links: usize,
    pub(crate) weak_evidence_references: usize,
    pub(crate) occurrence_headroom_entries: usize,
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
                policy_reference_diagnostics_for_source_tree(root, entry, source_tree_files)
            })
            .collect::<Vec<_>>();
        Self {
            policy_missing_evidence_entries: allow_report::matched_policy_missing_evidence_entries(
                cfg, outcomes,
            ),
            broken_evidence_links: diagnostics
                .iter()
                .filter(|reference| reference.diagnostic.status.is_broken_local_link())
                .count(),
            weak_evidence_references: diagnostics
                .iter()
                .filter(|reference| reference.diagnostic.status.is_weak_reference())
                .count(),
            occurrence_headroom_entries: allow_report::occurrence_headroom_entries(cfg, outcomes),
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
        context.occurrence_headroom_entries =
            (self.occurrence_headroom_entries > 0).then_some(self.occurrence_headroom_entries);
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
    pub(crate) inventory_source_identity: Option<&'a str>,
    /// `"enforcing"` or `"advisory"`, so the rendered result line states the
    /// mode that produced a pass. Previously this reached the receipt only,
    /// which left every report claiming `advisory` regardless of mode (#2832).
    pub(crate) enforcement: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub(crate) struct SourceTreeReportContext {
    root: String,
    inventory_facts: InventoryFacts,
    source_identity: Option<String>,
}

impl SourceTreeReportContext {
    pub(crate) fn new(root: &Path, inventory_facts: InventoryFacts) -> Self {
        Self::new_with_identity(root, inventory_facts, None)
    }

    pub(crate) fn new_with_identity(
        root: &Path,
        inventory_facts: InventoryFacts,
        source_identity: Option<&str>,
    ) -> Self {
        Self {
            root: allow_report::source_tree_path_text(root),
            inventory_facts,
            source_identity: source_identity.map(str::to_string),
        }
    }

    pub(crate) fn inventory(&self) -> allow_report::InventoryContext<'_> {
        allow_report::InventoryContext::source_syntax(
            self.inventory_source(),
            Some(self.source_tree_root()),
            self.inventory_files(),
        )
        .with_empty_git_tracked(self.inventory_facts.empty_git_tracked)
        .with_completeness(self.inventory_completeness())
        .with_source_identity(self.source_identity.as_deref())
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
        .with_empty_git_tracked(self.inventory_facts.empty_git_tracked)
        .with_inventory_completeness(self.inventory_completeness())
        .with_inventory_source_identity(self.source_identity.as_deref())
        .with_rust_scanner_facts(
            self.inventory_facts.rust_files_considered,
            self.inventory_facts.rust_files_skipped,
            self.inventory_facts.rust_files_with_parse_errors,
        )
    }

    pub(crate) fn source_tree_root(&self) -> &str {
        &self.root
    }

    pub(crate) fn inventory_source(&self) -> &str {
        self.inventory_facts.source.as_str()
    }

    pub(crate) fn inventory_completeness(&self) -> &str {
        if self.inventory_facts.rust_files_skipped > 0
            || self.inventory_facts.rust_files_with_parse_errors > 0
        {
            "partial"
        } else {
            self.inventory_facts.completeness.as_str()
        }
    }

    pub(crate) fn inventory_files(&self) -> Option<usize> {
        self.inventory_facts.files_scanned
    }
}

/// Process-wide output style, decided once by the CLI (#2572).
///
/// A `OnceLock` rather than an env var: the decision is made from the parsed
/// flag plus the environment plus terminal capability, and storing the result
/// means no renderer re-reads the environment and reaches a different answer.
static OUTPUT_STYLE: std::sync::OnceLock<allow_report::Style> = std::sync::OnceLock::new();

pub(crate) fn set_output_style(style: allow_report::Style) {
    let _ = OUTPUT_STYLE.set(style);
}

/// Defaults to plain, so any path that never called `set_output_style`
/// (tests, library use) is unstyled rather than accidentally coloured.
pub(crate) fn output_style() -> allow_report::Style {
    OUTPUT_STYLE
        .get()
        .copied()
        .unwrap_or(allow_report::Style::PLAIN)
}

/// Styling applies only to human output written to stdout.
///
/// Machine formats are excluded here as well as structurally (their renderers
/// never read `context.style`), and `--output` files stay plain so a committed
/// or shared report is portable.
fn style_for(format: OutputFormat, output: Option<&Path>) -> allow_report::Style {
    if format == OutputFormat::Human && output.is_none() {
        output_style()
    } else {
        allow_report::Style::PLAIN
    }
}

pub(crate) fn print_report(args: ReportRenderArgs<'_>) -> CargoAllowResult<()> {
    let source_context = SourceTreeReportContext::new_with_identity(
        args.root,
        args.inventory_facts,
        args.inventory_source_identity,
    );
    let mut context = source_context.report(Some(args.baseline_debt_entries));
    args.evidence.apply_to(&mut context);
    context.enforcement = args.enforcement;
    context.style = style_for(args.format, args.output);
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

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{AllowEntry, FindingKind, Lifecycle, MatchStatus, Selector};
    use allow_inventory::InventorySource;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn outcome(status: MatchStatus, allow_id: Option<&str>) -> MatchOutcome {
        MatchOutcome {
            status,
            allow_id: allow_id.map(str::to_string),
            candidate_ids: Vec::new(),
            finding_index: None,
            message: status.as_str().to_string(),
            score: 0,
        }
    }

    fn entry(id: &str, evidence: Vec<String>) -> AllowEntry {
        AllowEntry {
            id: id.to_string(),
            kind: FindingKind::NonRustFile,
            family: Some("documentation".to_string()),
            path: Some(PathBuf::from("docs/source.md")),
            glob: None,
            owner: "docs".to_string(),
            classification: "reviewed_exception".to_string(),
            reason: "Reporting test entry.".to_string(),
            evidence,
            links: Vec::new(),
            occurrence_limit: None,
            lifecycle: Lifecycle::empty(),
            selector: Selector {
                ast_kind: Some("tracked_file".to_string()),
                ..Selector::default()
            },
            last_seen: None,
        }
    }

    #[test]
    fn evidence_report_summary_counts_policy_and_reference_diagnostics() {
        let root = temp_root("evidence-summary");
        let docs = root.join("docs");
        fs::create_dir_all(&docs)
            .unwrap_or_else(|err| std::panic::panic_any(format!("docs dir: {err}")));
        fs::write(docs.join("present.md"), "present evidence")
            .unwrap_or_else(|err| std::panic::panic_any(format!("present evidence: {err}")));
        fs::write(docs.join("not-in-source-tree.md"), "untracked evidence")
            .unwrap_or_else(|err| std::panic::panic_any(format!("untracked evidence: {err}")));

        let mut cfg = AllowConfig::empty();
        cfg.allow
            .push(entry("matched-missing-evidence", Vec::new()));
        cfg.allow.push(entry(
            "matched-evidence-diagnostics",
            vec![
                "doc:docs/present.md".to_string(),
                "doc:docs/not-in-source-tree.md".to_string(),
                "doc:docs/missing.md".to_string(),
                "loose manual note".to_string(),
            ],
        ));
        cfg.allow
            .push(entry("unmatched-missing-evidence", Vec::new()));
        let outcomes = vec![
            outcome(MatchStatus::Matched, Some("matched-missing-evidence")),
            outcome(MatchStatus::Matched, Some("matched-evidence-diagnostics")),
            outcome(MatchStatus::Stale, Some("unmatched-missing-evidence")),
        ];
        let source_tree_files = BTreeSet::from(["docs/present.md".to_string()]);

        let summary = EvidenceReportSummary::from_policy_with_source_tree_files(
            &root,
            &cfg,
            &outcomes,
            Some(&source_tree_files),
        );

        assert_eq!(summary.policy_missing_evidence_entries, 1);
        assert_eq!(summary.broken_evidence_links, 2);
        assert_eq!(summary.weak_evidence_references, 1);
        assert!(summary.has_broken_evidence_links());

        fs::remove_dir_all(&root)
            .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture: {err}")));
    }

    #[test]
    fn evidence_report_summary_apply_to_sets_only_positive_counts() {
        let mut empty_context =
            allow_report::ReportContext::source_syntax("git_tracked", None, None, None);
        EvidenceReportSummary::default().apply_to(&mut empty_context);

        assert_eq!(empty_context.policy_missing_evidence_entries, None);
        assert_eq!(empty_context.broken_evidence_links, None);
        assert_eq!(empty_context.weak_evidence_references, None);

        let mut populated_context =
            allow_report::ReportContext::source_syntax("git_tracked", None, None, None);
        EvidenceReportSummary {
            policy_missing_evidence_entries: 3,
            broken_evidence_links: 2,
            weak_evidence_references: 1,
            occurrence_headroom_entries: 0,
        }
        .apply_to(&mut populated_context);

        assert_eq!(populated_context.policy_missing_evidence_entries, Some(3));
        assert_eq!(populated_context.broken_evidence_links, Some(2));
        assert_eq!(populated_context.weak_evidence_references, Some(1));
    }

    #[test]
    fn source_context_promotes_scanner_omissions_to_partial_inventory() {
        let root = temp_root("scanner-partial");
        let facts =
            InventoryFacts::scanned(InventorySource::GitTracked, 3).with_rust_files_skipped(1);
        let context = SourceTreeReportContext::new(&root, facts);

        assert_eq!(context.inventory_completeness(), "partial");
        assert_eq!(context.inventory().completeness, Some("partial"));
        assert_eq!(context.report(None).inventory.completeness, Some("partial"));

        fs::remove_dir_all(&root)
            .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture: {err}")));
    }

    #[test]
    fn print_report_dispatches_all_formats_and_writes_outputs() {
        let root = temp_root("print-report");
        let cases = [
            (OutputFormat::Human, "human.txt", "cargo-allow audit"),
            (
                OutputFormat::Json,
                "report.json",
                "\"schema_id\": \"cargo-allow.report.v1\"",
            ),
            (OutputFormat::Html, "report.html", "<!doctype html>"),
            (
                OutputFormat::Sarif,
                "report.sarif",
                "\"version\": \"2.1.0\"",
            ),
            (OutputFormat::Markdown, "report.md", "# cargo-allow audit"),
        ];

        for (format, file_name, marker) in cases {
            let output = root.join(file_name);
            print_report(ReportRenderArgs {
                command: "audit",
                format,
                baseline_debt_entries: 0,
                evidence: EvidenceReportSummary {
                    policy_missing_evidence_entries: 1,
                    broken_evidence_links: 1,
                    weak_evidence_references: 1,
                    occurrence_headroom_entries: 0,
                },
                findings: &[],
                outcomes: &[],
                failed: false,
                output: Some(&output),
                root: &root,
                inventory_facts: InventoryFacts::scanned(InventorySource::GitTracked, 7),
                inventory_source_identity: None,
                enforcement: None,
            })
            .unwrap_or_else(|err| std::panic::panic_any(format!("print {file_name}: {err}")));

            let text = fs::read_to_string(&output)
                .unwrap_or_else(|err| std::panic::panic_any(format!("read {file_name}: {err}")));
            assert!(
                text.contains(marker),
                "{file_name} should contain marker {marker}; got {text}"
            );
        }

        fs::remove_dir_all(&root)
            .unwrap_or_else(|err| std::panic::panic_any(format!("remove fixture: {err}")));
    }

    static NEXT_TEMP_ROOT: AtomicUsize = AtomicUsize::new(0);

    fn temp_root(label: &str) -> PathBuf {
        let id = NEXT_TEMP_ROOT.fetch_add(1, Ordering::Relaxed);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!(
            "cargo-allow-reporting-{label}-{}-{stamp}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&root)
            .unwrap_or_else(|err| std::panic::panic_any(format!("temp root: {err}")));
        root
    }
}
