use allow_core::{Finding, MatchOutcome, MatchStatus};
use std::collections::BTreeMap;

use crate::text::markdown_cell;

#[derive(Debug, Clone, Default)]
struct SourceInventoryRow {
    total: usize,
    matched: usize,
    new: usize,
    review_items: usize,
}

impl SourceInventoryRow {
    fn add_finding(&mut self) {
        self.total += 1;
    }

    fn add_status(&mut self, status: MatchStatus) {
        match status {
            MatchStatus::Matched => self.matched += 1,
            MatchStatus::New => {
                self.new += 1;
                self.review_items += 1;
            }
            _ => {
                if status != MatchStatus::Matched {
                    self.review_items += 1;
                }
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
struct SourceInventory {
    total: usize,
    by_kind: BTreeMap<String, SourceInventoryRow>,
    by_family: BTreeMap<String, SourceInventoryRow>,
}

impl SourceInventory {
    fn from_report(findings: &[Finding], outcomes: &[MatchOutcome]) -> Self {
        let mut inventory = Self::default();
        for finding in findings {
            inventory.total += 1;
            inventory.kind_row_mut(finding).add_finding();
            inventory.family_row_mut(finding).add_finding();
        }
        for outcome in outcomes {
            let Some(finding) = outcome.finding_index.and_then(|index| findings.get(index)) else {
                continue;
            };
            inventory.kind_row_mut(finding).add_status(outcome.status);
            inventory.family_row_mut(finding).add_status(outcome.status);
        }
        inventory
    }

    fn kind_row_mut(&mut self, finding: &Finding) -> &mut SourceInventoryRow {
        self.by_kind
            .entry(finding.kind.as_str().to_string())
            .or_default()
    }

    fn family_row_mut(&mut self, finding: &Finding) -> &mut SourceInventoryRow {
        self.by_family
            .entry(finding_family_key(finding))
            .or_default()
    }

    fn has_findings(&self) -> bool {
        self.total > 0
    }
}

pub(crate) fn render_source_inventory_human(
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    out: &mut String,
) {
    let inventory = SourceInventory::from_report(findings, outcomes);
    if !inventory.has_findings() {
        return;
    }
    out.push('\n');
    out.push_str("Source exception inventory:\n");
    out.push_str(&format!("  findings                 {}\n", inventory.total));
    out.push_str("  by kind:\n");
    for (kind, row) in &inventory.by_kind {
        append_human_row(out, kind, row);
    }
    out.push_str("  by family:\n");
    for (family, row) in &inventory.by_family {
        append_human_row(out, family, row);
    }
}

fn append_human_row(out: &mut String, label: &str, row: &SourceInventoryRow) {
    out.push_str(&format!(
        "    {:24} total={} matched={} new={} review_items={}\n",
        label, row.total, row.matched, row.new, row.review_items
    ));
}

pub(crate) fn render_source_inventory_markdown(
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    out: &mut String,
) {
    let inventory = SourceInventory::from_report(findings, outcomes);
    if !inventory.has_findings() {
        return;
    }
    out.push_str("\n## Source Exception Inventory\n\n");
    out.push_str(&format!("Findings inventoried: `{}`\n\n", inventory.total));
    out.push_str("| Kind | Total | Matched | New | Review items |\n|---|---:|---:|---:|---:|\n");
    for (kind, row) in &inventory.by_kind {
        append_markdown_row(out, kind, row);
    }
    out.push_str(
        "\n| Family | Total | Matched | New | Review items |\n|---|---:|---:|---:|---:|\n",
    );
    for (family, row) in &inventory.by_family {
        append_markdown_row(out, family, row);
    }
}

fn append_markdown_row(out: &mut String, label: &str, row: &SourceInventoryRow) {
    out.push_str(&format!(
        "| `{}` | {} | {} | {} | {} |\n",
        markdown_cell(label),
        row.total,
        row.matched,
        row.new,
        row.review_items
    ));
}

fn finding_family_key(finding: &Finding) -> String {
    match finding.family.as_deref() {
        Some(family) if !family.trim().is_empty() => {
            format!("{}.{}", finding.kind.as_str(), family.trim())
        }
        _ => format!("{}.unknown", finding.kind.as_str()),
    }
}
