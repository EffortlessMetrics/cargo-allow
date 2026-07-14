use allow_core::{Finding, MatchOutcome, MatchStatus, json_escape};
use std::collections::BTreeMap;

use crate::text::{html_escape, markdown_cell};

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
    by_family: BTreeMap<SourceInventoryFamilyKey, SourceInventoryRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SourceInventoryFamilyKey {
    kind: String,
    family: String,
}

impl SourceInventoryFamilyKey {
    fn label(&self) -> String {
        format!("{}.{}", self.kind, self.family)
    }
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
        append_human_row(out, &family.label(), row);
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
        append_markdown_row(out, &family.label(), row);
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

pub(crate) fn render_source_inventory_html(
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    out: &mut String,
) {
    let inventory = SourceInventory::from_report(findings, outcomes);
    if !inventory.has_findings() {
        return;
    }
    out.push_str("<h2>Source Exception Inventory</h2>\n");
    out.push_str(&format!(
        "<p>Findings inventoried: <code>{}</code></p>\n",
        inventory.total
    ));
    out.push_str("<table><thead><tr><th>Kind</th><th>Total</th><th>Matched</th><th>New</th><th>Review items</th></tr></thead><tbody>\n");
    for (kind, row) in &inventory.by_kind {
        append_html_row(out, "kind", kind, row);
    }
    out.push_str("</tbody></table>\n");
    out.push_str("<table><thead><tr><th>Family</th><th>Total</th><th>Matched</th><th>New</th><th>Review items</th></tr></thead><tbody>\n");
    for (family, row) in &inventory.by_family {
        append_html_row(out, "family", &family.label(), row);
    }
    out.push_str("</tbody></table>\n");
}

fn append_html_row(out: &mut String, class: &str, label: &str, row: &SourceInventoryRow) {
    out.push_str(&format!(
        "<tr><td><code class=\"{}\">{}</code></td><td class=\"count\">{}</td><td class=\"count\">{}</td><td class=\"count\">{}</td><td class=\"count\">{}</td></tr>\n",
        html_escape(class),
        html_escape(label),
        row.total,
        row.matched,
        row.new,
        row.review_items
    ));
}

pub(crate) fn render_source_inventory_json(
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    indent: &str,
) -> Option<String> {
    let inventory = SourceInventory::from_report(findings, outcomes);
    if !inventory.has_findings() {
        return None;
    }

    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("{indent}  \"findings\": {},\n", inventory.total));
    out.push_str(&format!("{indent}  \"by_kind\": [\n"));
    for (index, (kind, row)) in inventory.by_kind.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str(&format!(
            "{indent}    {{\"kind\": \"{}\", \"total\": {}, \"matched\": {}, \"new\": {}, \"review_items\": {}}}",
            json_escape(kind),
            row.total,
            row.matched,
            row.new,
            row.review_items
        ));
    }
    out.push_str(&format!("\n{indent}  ],\n"));
    out.push_str(&format!("{indent}  \"by_family\": [\n"));
    for (index, (family, row)) in inventory.by_family.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str(&format!(
            "{indent}    {{\"kind\": \"{}\", \"family\": \"{}\", \"label\": \"{}\", \"total\": {}, \"matched\": {}, \"new\": {}, \"review_items\": {}}}",
            json_escape(&family.kind),
            json_escape(&family.family),
            json_escape(&family.label()),
            row.total,
            row.matched,
            row.new,
            row.review_items
        ));
    }
    out.push_str(&format!("\n{indent}  ]\n"));
    out.push_str(&format!("{indent}}}"));
    Some(out)
}

fn finding_family_key(finding: &Finding) -> SourceInventoryFamilyKey {
    SourceInventoryFamilyKey {
        kind: finding.kind.as_str().to_string(),
        family: finding
            .family
            .as_deref()
            .map(str::trim)
            .filter(|family| !family.is_empty())
            .unwrap_or("unknown")
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{FindingKind, Span, StructuralIdentity};
    use std::path::PathBuf;

    fn finding(kind: FindingKind, family: Option<&str>, path: &str) -> Finding {
        Finding {
            kind,
            family: family.map(str::to_string),
            path: PathBuf::from(path),
            span: Some(Span { line: 1, column: 1 }),
            identity: StructuralIdentity::new("test", "source_inventory"),
            message: "source inventory fixture".to_string(),
            ledger: None,
        }
    }

    fn outcome(status: MatchStatus, finding_index: Option<usize>) -> MatchOutcome {
        MatchOutcome {
            status,
            allow_id: None,
            candidate_ids: Vec::new(),
            finding_index,
            message: status.as_str().to_string(),
            score: 0,
        }
    }

    fn inventory_fixture() -> (Vec<Finding>, Vec<MatchOutcome>) {
        let findings = vec![
            finding(FindingKind::Unsafe, Some(" unsafe_block "), "src/ffi.rs"),
            finding(FindingKind::Unsafe, None, "src/raw.rs"),
            finding(FindingKind::Panic, Some(""), "src/lib.rs"),
        ];
        let outcomes = vec![
            outcome(MatchStatus::Matched, Some(0)),
            outcome(MatchStatus::ReviewDue, Some(0)),
            outcome(MatchStatus::New, Some(1)),
            outcome(MatchStatus::Stale, Some(2)),
            outcome(MatchStatus::New, Some(99)),
            outcome(MatchStatus::New, None),
        ];
        (findings, outcomes)
    }

    #[test]
    fn source_inventory_counts_statuses_by_kind_and_family() {
        let (findings, outcomes) = inventory_fixture();

        let inventory = SourceInventory::from_report(&findings, &outcomes);

        assert_eq!(inventory.total, 3);
        assert!(inventory.has_findings());
        assert_eq!(
            inventory.by_kind.get("unsafe").map(|row| (
                row.total,
                row.matched,
                row.new,
                row.review_items
            )),
            Some((2, 1, 1, 2))
        );
        assert_eq!(
            inventory.by_kind.get("panic").map(|row| (
                row.total,
                row.matched,
                row.new,
                row.review_items
            )),
            Some((1, 0, 0, 1))
        );
        assert_eq!(
            inventory
                .by_family
                .get(&SourceInventoryFamilyKey {
                    kind: "unsafe".to_string(),
                    family: "unsafe_block".to_string(),
                })
                .map(|row| (row.total, row.matched, row.new, row.review_items)),
            Some((1, 1, 0, 1))
        );
        assert_eq!(
            inventory
                .by_family
                .get(&SourceInventoryFamilyKey {
                    kind: "unsafe".to_string(),
                    family: "unknown".to_string(),
                })
                .map(|row| (row.total, row.matched, row.new, row.review_items)),
            Some((1, 0, 1, 1))
        );
        assert_eq!(
            inventory
                .by_family
                .get(&SourceInventoryFamilyKey {
                    kind: "panic".to_string(),
                    family: "unknown".to_string(),
                })
                .map(|row| (row.total, row.matched, row.new, row.review_items)),
            Some((1, 0, 0, 1))
        );
    }

    #[test]
    fn source_inventory_renderers_skip_empty_inventory() {
        let mut out = String::from("prefix");

        render_source_inventory_human(&[], &[], &mut out);
        assert_eq!(out, "prefix");

        render_source_inventory_markdown(&[], &[], &mut out);
        assert_eq!(out, "prefix");

        render_source_inventory_html(&[], &[], &mut out);
        assert_eq!(out, "prefix");

        assert_eq!(render_source_inventory_json(&[], &[], "  "), None);
    }

    #[test]
    fn source_inventory_human_and_markdown_render_counts() {
        let (findings, outcomes) = inventory_fixture();
        let mut human = String::new();
        let mut markdown = String::new();

        render_source_inventory_human(&findings, &outcomes, &mut human);
        render_source_inventory_markdown(&findings, &outcomes, &mut markdown);

        assert!(human.contains("Source exception inventory:"));
        assert!(human.contains("findings                 3"));
        assert!(human.contains("unsafe                   total=2 matched=1 new=1 review_items=2"));
        assert!(human.contains("panic                    total=1 matched=0 new=0 review_items=1"));
        assert!(human.contains("unsafe.unsafe_block      total=1 matched=1 new=0 review_items=1"));
        assert!(human.contains("unsafe.unknown           total=1 matched=0 new=1 review_items=1"));
        assert!(human.contains("panic.unknown            total=1 matched=0 new=0 review_items=1"));

        assert!(markdown.contains("## Source Exception Inventory"));
        assert!(markdown.contains("Findings inventoried: `3`"));
        assert!(markdown.contains("| `unsafe` | 2 | 1 | 1 | 2 |"));
        assert!(markdown.contains("| `panic` | 1 | 0 | 0 | 1 |"));
        assert!(markdown.contains("| `unsafe.unsafe_block` | 1 | 1 | 0 | 1 |"));
        assert!(markdown.contains("| `unsafe.unknown` | 1 | 0 | 1 | 1 |"));
        assert!(markdown.contains("| `panic.unknown` | 1 | 0 | 0 | 1 |"));
    }

    #[test]
    fn source_inventory_html_and_json_render_counts() {
        let (findings, outcomes) = inventory_fixture();
        let mut html = String::new();

        render_source_inventory_html(&findings, &outcomes, &mut html);
        let json = render_source_inventory_json(&findings, &outcomes, "  ");

        assert!(html.contains("<h2>Source Exception Inventory</h2>"));
        assert!(html.contains("<p>Findings inventoried: <code>3</code></p>"));
        assert!(html.contains("<code class=\"kind\">unsafe</code>"));
        assert!(html.contains("<code class=\"kind\">panic</code>"));
        assert!(html.contains("<code class=\"family\">unsafe.unsafe_block</code>"));
        assert!(html.contains("<code class=\"family\">unsafe.unknown</code>"));
        assert!(html.contains("<code class=\"family\">panic.unknown</code>"));
        assert!(html.contains("<td class=\"count\">2</td><td class=\"count\">1</td><td class=\"count\">1</td><td class=\"count\">2</td>"));

        assert!(json.as_deref().is_some_and(|text| {
            text.contains("\"findings\": 3")
                && text.contains(
                    "{\"kind\": \"unsafe\", \"total\": 2, \"matched\": 1, \"new\": 1, \"review_items\": 2}"
                )
                && text.contains(
                    "{\"kind\": \"panic\", \"family\": \"unknown\", \"label\": \"panic.unknown\", \"total\": 1, \"matched\": 0, \"new\": 0, \"review_items\": 1}"
                )
        }));
    }

    #[test]
    fn source_inventory_renderers_escape_family_labels() {
        let findings = vec![finding(
            FindingKind::Unsafe,
            Some("ffi|unsafe`<tag>&\""),
            "src/ffi.rs",
        )];
        let outcomes = vec![outcome(MatchStatus::New, Some(0))];
        let mut markdown = String::new();
        let mut html = String::new();

        render_source_inventory_markdown(&findings, &outcomes, &mut markdown);
        render_source_inventory_html(&findings, &outcomes, &mut html);
        let json = render_source_inventory_json(&findings, &outcomes, "  ");

        assert!(markdown.contains("`unsafe.ffi\\|unsafe\\`<tag>&\"`"));
        assert!(html.contains("unsafe.ffi|unsafe`&lt;tag&gt;&amp;&quot;"));
        assert!(
            json.as_deref()
                .is_some_and(|text| text.contains("\"label\": \"unsafe.ffi|unsafe`<tag>&\\\"\""))
        );
    }

    #[test]
    fn source_inventory_row_add_status_routes_review_items() {
        let mut row = SourceInventoryRow::default();

        row.add_status(MatchStatus::Matched);
        row.add_status(MatchStatus::New);
        row.add_status(MatchStatus::ReviewDue);
        row.add_status(MatchStatus::EvidenceMissing);

        assert_eq!(row.matched, 1);
        assert_eq!(row.new, 1);
        assert_eq!(row.review_items, 3);
    }

    #[test]
    fn source_inventory_row_treats_non_matched_status_as_review_item() {
        let mut row = SourceInventoryRow::default();

        row.add_status(MatchStatus::Stale);

        assert_eq!(row.matched, 0);
        assert_eq!(row.new, 0);
        assert_eq!(row.review_items, 1);
    }
}
