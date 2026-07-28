use allow_core::{Finding, FindingKind, MatchOutcome, MatchStatus, normalize_path};
use std::collections::BTreeMap;

use crate::text::markdown_cell;

const HUMAN_FILE_ROW_LIMIT: usize = 20;
const MARKDOWN_FILE_ROW_LIMIT: usize = 30;

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
    // The per-file listing is the matched inventory `--quiet` exists to drop:
    // on this repository it is 20 rows plus an omission note, against 4 lines
    // of counts. The counts above stay either way (#2785).
    if crate::contracts::is_quiet() {
        return;
    }
    let rows = non_rust_file_rows(findings, outcomes);
    if !rows.is_empty() {
        out.push_str("  files:\n");
        let row_count = rows.len();
        for row in rows.into_iter().take(HUMAN_FILE_ROW_LIMIT) {
            out.push_str(&format!(
                "    {:12} {:24} {}\n",
                row.status, row.family, row.path
            ));
        }
        append_human_omitted_file_note(out, row_count);
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
        let row_count = rows.len();
        for row in rows.into_iter().take(MARKDOWN_FILE_ROW_LIMIT) {
            out.push_str(&format!(
                "| `{}` | `{}` | `{}` |\n",
                markdown_cell(row.status),
                markdown_cell(&row.family),
                markdown_cell(&row.path)
            ));
        }
        append_markdown_omitted_file_note(out, row_count);
    }
}

fn append_human_omitted_file_note(out: &mut String, row_count: usize) {
    if row_count > HUMAN_FILE_ROW_LIMIT {
        let omitted = row_count - HUMAN_FILE_ROW_LIMIT;
        let plural = if omitted == 1 { "" } else { "s" };
        out.push_str(&format!(
            "    ... {omitted} additional non-Rust file{plural} omitted from this listing\n"
        ));
    }
}

fn append_markdown_omitted_file_note(out: &mut String, row_count: usize) {
    if row_count > MARKDOWN_FILE_ROW_LIMIT {
        let omitted = row_count - MARKDOWN_FILE_ROW_LIMIT;
        let plural = if omitted == 1 { "" } else { "s" };
        out.push_str(&format!(
            "\n{omitted} additional non-Rust file{plural} omitted from this listing.\n"
        ));
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

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{Span, StructuralIdentity};
    use std::path::PathBuf;

    #[test]
    fn render_non_rust_human_reports_file_inventory_and_rows() {
        let findings = vec![
            file_finding(FindingKind::NonRustFile, "policy", "policy/allow.toml"),
            file_finding(FindingKind::GeneratedCode, "generated", "src/generated.rs"),
            file_finding(FindingKind::Panic, "panic", "src/lib.rs"),
        ];
        let outcomes = vec![
            outcome(MatchStatus::Matched, Some(0)),
            outcome(MatchStatus::New, Some(1)),
            outcome(MatchStatus::Matched, Some(2)),
        ];
        let mut out = String::new();

        render_non_rust_human(&findings, &outcomes, &mut out);

        assert!(out.contains("Non-Rust file inventory:"));
        assert!(out.contains("  files scanned              2"));
        assert!(out.contains("  matched                    1"));
        assert!(out.contains("  new                        1"));
        assert!(out.contains("  generated                  1"));
        assert!(out.contains("    generated                1"));
        assert!(out.contains("    policy                   1"));
        assert!(out.contains("    matched      policy                   policy/allow.toml"));
        assert!(out.contains("    new          generated                src/generated.rs"));
        assert!(!out.contains("src/lib.rs"));
    }

    #[test]
    fn render_non_rust_human_leaves_output_unchanged_without_file_findings() {
        let findings = vec![file_finding(FindingKind::Panic, "panic", "src/lib.rs")];
        let outcomes = vec![outcome(MatchStatus::New, Some(0))];
        let mut out = String::from("prefix");

        render_non_rust_human(&findings, &outcomes, &mut out);

        assert_eq!(out, "prefix");
    }

    #[test]
    fn render_non_rust_markdown_call_presence_observer() {
        let findings = vec![
            file_finding(FindingKind::NonRustFile, "docs|guide", "docs/guide.md"),
            file_finding(FindingKind::GeneratedCode, "generated", "src/generated.rs"),
        ];
        let outcomes = vec![
            outcome(MatchStatus::Matched, Some(0)),
            outcome(MatchStatus::New, Some(1)),
        ];
        let mut out = String::new();

        render_non_rust_markdown(&findings, &outcomes, &mut out);

        assert!(out.contains("## Non-Rust File Inventory"));
        assert!(out.contains("| Files scanned | 2 |"));
        assert!(out.contains("| Matched | 1 |"));
        assert!(out.contains("| New | 1 |"));
        assert!(out.contains("| Generated | 1 |"));
        assert!(out.contains("| `docs\\|guide` | 1 |"));
        assert!(out.contains("| `generated` | 1 |"));
        assert!(out.contains("| `matched` | `docs\\|guide` | `docs/guide.md` |"));
        assert!(out.contains("| `new` | `generated` | `src/generated.rs` |"));
    }

    #[test]
    fn render_non_rust_markdown_return_value_discriminator() {
        let findings = Vec::new();
        let outcomes = Vec::new();
        let mut out = String::from("existing");

        render_non_rust_markdown(&findings, &outcomes, &mut out);

        assert_eq!(out, "existing");
    }

    #[test]
    fn append_human_omitted_file_note_boundary_discriminator() {
        let mut at_limit = String::new();
        append_human_omitted_file_note(&mut at_limit, HUMAN_FILE_ROW_LIMIT);
        assert_eq!(at_limit, "");

        let mut one_over = String::new();
        append_human_omitted_file_note(&mut one_over, HUMAN_FILE_ROW_LIMIT + 1);
        assert_eq!(
            one_over,
            "    ... 1 additional non-Rust file omitted from this listing\n"
        );

        let mut two_over = String::new();
        append_human_omitted_file_note(&mut two_over, HUMAN_FILE_ROW_LIMIT + 2);
        assert_eq!(
            two_over,
            "    ... 2 additional non-Rust files omitted from this listing\n"
        );
    }

    #[test]
    fn append_human_omitted_file_note_call_presence_observer() {
        let mut out = String::from("files:\n");

        append_human_omitted_file_note(&mut out, HUMAN_FILE_ROW_LIMIT + 1);

        assert!(out.ends_with("additional non-Rust file omitted from this listing\n"));
    }

    #[test]
    fn append_markdown_omitted_file_note_boundary_discriminator() {
        let mut at_limit = String::new();
        append_markdown_omitted_file_note(&mut at_limit, MARKDOWN_FILE_ROW_LIMIT);
        assert_eq!(at_limit, "");

        let mut one_over = String::new();
        append_markdown_omitted_file_note(&mut one_over, MARKDOWN_FILE_ROW_LIMIT + 1);
        assert_eq!(
            one_over,
            "\n1 additional non-Rust file omitted from this listing.\n"
        );

        let mut two_over = String::new();
        append_markdown_omitted_file_note(&mut two_over, MARKDOWN_FILE_ROW_LIMIT + 2);
        assert_eq!(
            two_over,
            "\n2 additional non-Rust files omitted from this listing.\n"
        );
    }

    #[test]
    fn append_markdown_omitted_file_note_call_presence_observer() {
        let mut out = String::from("| Status | Family | Path |\n");

        append_markdown_omitted_file_note(&mut out, MARKDOWN_FILE_ROW_LIMIT + 1);

        assert!(out.ends_with("additional non-Rust file omitted from this listing.\n"));
    }

    #[test]
    fn non_rust_file_rows_call_presence_observer() {
        let findings = vec![
            file_finding(FindingKind::Panic, "panic", "src/lib.rs"),
            file_finding(FindingKind::GeneratedCode, "generated", "src/generated.rs"),
            file_finding(FindingKind::NonRustFile, "docs", "docs/guide.md"),
            file_finding(FindingKind::NonRustFile, "docs", "docs/readme.md"),
        ];
        let outcomes = vec![
            outcome(MatchStatus::Matched, Some(2)),
            outcome(MatchStatus::New, Some(1)),
        ];

        let rows = non_rust_file_rows(&findings, &outcomes);

        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].path, "docs/guide.md");
        assert_eq!(rows[0].family, "docs");
        assert_eq!(rows[0].status, "matched");
        assert_eq!(rows[1].path, "docs/readme.md");
        assert_eq!(rows[1].family, "docs");
        assert_eq!(rows[1].status, "unmatched");
        assert_eq!(rows[2].path, "src/generated.rs");
        assert_eq!(rows[2].family, "generated");
        assert_eq!(rows[2].status, "new");
    }

    fn file_finding(kind: FindingKind, family: &str, path: &str) -> Finding {
        Finding {
            kind,
            family: Some(family.to_string()),
            path: PathBuf::from(path),
            span: Some(Span { line: 1, column: 1 }),
            identity: StructuralIdentity::new("file", "tracked_file"),
            message: "tracked file".to_string(),
            ledger: None,
        }
    }

    fn outcome(status: MatchStatus, finding_index: Option<usize>) -> MatchOutcome {
        MatchOutcome {
            status,
            allow_id: None,
            candidate_ids: Vec::new(),
            finding_index,
            message: String::new(),
            score: 0,
        }
    }
}
