use allow_core::{Finding, FindingKind, MatchOutcome, MatchStatus, normalize_path};
use std::collections::BTreeMap;

use crate::text::markdown_cell;

#[derive(Debug, Default)]
pub(crate) struct FilePosture {
    pub(crate) total: usize,
    pub(crate) by_family: BTreeMap<String, usize>,
    pub(crate) matched: usize,
    pub(crate) new: usize,
    pub(crate) generated: usize,
}

impl FilePosture {
    pub(crate) fn from_report(findings: &[Finding], outcomes: &[MatchOutcome]) -> Self {
        let mut posture = Self::default();
        for finding in findings.iter().filter(|finding| is_file_finding(finding)) {
            posture.total += 1;
            if finding.kind == FindingKind::GeneratedCode {
                posture.generated += 1;
            }
            *posture
                .by_family
                .entry(
                    finding
                        .family
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string()),
                )
                .or_insert(0) += 1;
        }
        for outcome in outcomes {
            let applies_to_file = outcome
                .finding_index
                .and_then(|idx| findings.get(idx))
                .map(is_file_finding)
                .unwrap_or(false);
            match outcome.status {
                MatchStatus::Matched if applies_to_file => posture.matched += 1,
                MatchStatus::New if applies_to_file => posture.new += 1,
                _ => {}
            }
        }
        posture
    }

    pub(crate) fn has_files(&self) -> bool {
        self.total > 0
    }
}

pub(crate) fn render_non_rust_human(
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    out: &mut String,
) {
    let posture = FilePosture::from_report(findings, outcomes);
    if !posture.has_files() {
        return;
    }
    out.push('\n');
    out.push_str("Non-Rust file inventory:\n");
    out.push_str(&format!("  files scanned              {}\n", posture.total));
    out.push_str(&format!(
        "  matched                    {}\n",
        posture.matched
    ));
    out.push_str(&format!("  new                        {}\n", posture.new));
    out.push_str(&format!(
        "  generated                  {}\n",
        posture.generated
    ));
    if !posture.by_family.is_empty() {
        out.push_str("  by family:\n");
        for (family, count) in posture.by_family {
            out.push_str(&format!("    {:24} {}\n", family, count));
        }
    }
    let rows = non_rust_file_rows(findings, outcomes);
    if !rows.is_empty() {
        out.push_str("  files:\n");
        for row in rows.into_iter().take(40) {
            out.push_str(&format!(
                "    {:12} {:24} {}\n",
                row.status, row.family, row.path
            ));
        }
    }
}

pub(crate) fn render_non_rust_markdown(
    findings: &[Finding],
    outcomes: &[MatchOutcome],
    out: &mut String,
) {
    let posture = FilePosture::from_report(findings, outcomes);
    if !posture.has_files() {
        return;
    }
    out.push_str("\n## Non-Rust File Inventory\n\n");
    out.push_str("| Metric | Count |\n|---|---:|\n");
    out.push_str(&format!("| Files scanned | {} |\n", posture.total));
    out.push_str(&format!("| Matched | {} |\n", posture.matched));
    out.push_str(&format!("| New | {} |\n", posture.new));
    out.push_str(&format!("| Generated | {} |\n", posture.generated));
    if !posture.by_family.is_empty() {
        out.push_str("\n| Family | Count |\n|---|---:|\n");
        for (family, count) in posture.by_family {
            out.push_str(&format!("| `{}` | {} |\n", markdown_cell(&family), count));
        }
    }
    let rows = non_rust_file_rows(findings, outcomes);
    if !rows.is_empty() {
        out.push_str("\n| Status | Family | Path |\n|---|---|---|\n");
        for row in rows.into_iter().take(60) {
            out.push_str(&format!(
                "| `{}` | `{}` | `{}` |\n",
                markdown_cell(row.status),
                markdown_cell(&row.family),
                markdown_cell(&row.path)
            ));
        }
    }
}

fn is_file_finding(finding: &Finding) -> bool {
    matches!(
        finding.kind,
        FindingKind::NonRustFile | FindingKind::GeneratedCode
    )
}

#[derive(Debug)]
pub(crate) struct FileRow {
    pub(crate) status: &'static str,
    pub(crate) family: String,
    pub(crate) path: String,
}

pub(crate) fn non_rust_file_rows(findings: &[Finding], outcomes: &[MatchOutcome]) -> Vec<FileRow> {
    let mut status_by_index = BTreeMap::new();
    for outcome in outcomes {
        if let Some(index) = outcome.finding_index {
            status_by_index.insert(index, outcome.status.as_str());
        }
    }
    let mut rows = findings
        .iter()
        .enumerate()
        .filter(|(_, finding)| is_file_finding(finding))
        .map(|(index, finding)| FileRow {
            status: status_by_index.get(&index).copied().unwrap_or("unmatched"),
            family: finding
                .family
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            path: normalize_path(&finding.path),
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.family.cmp(&right.family))
            .then_with(|| left.status.cmp(right.status))
    });
    rows
}
