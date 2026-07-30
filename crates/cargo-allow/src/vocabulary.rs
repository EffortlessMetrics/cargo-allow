use allow_core::{CargoAllowResult, MatchStatus};
use allow_policy::{
    canonical_evidence_prefixes, local_file_evidence_prefixes, recognized_evidence_prefixes,
    traceability_evidence_prefixes,
};
use clap::{Parser, ValueEnum};

use crate::{emit_text, kind_filter::KIND_GROUPS};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum VocabularyFormat {
    Human,
    Json,
}

/// Discover the vocabulary cargo-allow understands: finding kinds (with
/// aliases), evidence prefixes (with categories), and match statuses.
///
/// Run `cargo-allow check --format json` first to find the path, line, and
/// kind of an unreceipted finding, then use the vocabulary here to choose
/// the right kind filter or evidence prefix for `why` and `add`.
#[derive(Debug, Clone, Parser)]
pub(crate) struct VocabularyArgs {
    /// Output format.
    #[arg(long, value_enum, default_value_t = VocabularyFormat::Human)]
    pub(crate) format: VocabularyFormat,
    /// Write vocabulary output to a file instead of stdout.
    #[arg(long)]
    pub(crate) output: Option<std::path::PathBuf>,
}

pub(crate) fn cmd_vocabulary(args: &VocabularyArgs) -> CargoAllowResult<()> {
    let text = match args.format {
        VocabularyFormat::Human => {
            let style = if args.output.is_none() {
                crate::reporting::output_style()
            } else {
                allow_report::Style::PLAIN
            };
            render_vocabulary_human_styled(style)
        }
        VocabularyFormat::Json => render_vocabulary_json(),
    };
    emit_text(args.output.as_deref(), &text)?;
    Ok(())
}

fn render_vocabulary_human_styled(style: allow_report::Style) -> String {
    let mut out = String::new();
    out.push_str(&style.strong("cargo-allow vocabulary"));
    out.push_str("\n\n");

    out.push_str(&style.strong("Finding kinds (--kind for check, audit, diff, why, add):"));
    out.push('\n');
    for group in KIND_GROUPS {
        out.push_str(&format!("  {}", group.canonical));
        let extra: Vec<&str> = group
            .aliases
            .iter()
            .filter(|alias| **alias != group.canonical)
            .copied()
            .collect();
        if !extra.is_empty() {
            out.push_str(&format!(" (aliases: {})", extra.join(", ")));
        }
        out.push('\n');
    }
    out.push('\n');

    out.push_str(&style.strong("Evidence prefixes (--evidence for add):"));
    out.push('\n');
    let local_file: Vec<String> = local_file_evidence_prefixes().map(String::from).collect();
    let traceability: Vec<String> = traceability_evidence_prefixes().map(String::from).collect();
    out.push_str(&format!(
        "  Local-file (must point at a source-tree path):\n    {}\n",
        local_file.join(", ")
    ));
    out.push_str(&format!(
        "  Traceability (external reference, no local file needed):\n    {}\n",
        traceability.join(", ")
    ));
    let recognized: Vec<String> = recognized_evidence_prefixes().map(String::from).collect();
    out.push_str(&format!(
        "  All recognized prefixes: {}\n\n",
        recognized.join(", ")
    ));

    out.push_str(&style.strong("Match statuses (--status for list, worklist):"));
    out.push('\n');
    for status in MatchStatus::ALL {
        out.push_str(&format!(
            "  {}\n",
            style.status(status.as_str(), status.as_str())
        ));
    }

    out
}

fn render_vocabulary_json() -> String {
    let mut out = String::new();
    out.push_str("{\n");

    out.push_str("  \"kinds\": [\n");
    for (i, group) in KIND_GROUPS.iter().enumerate() {
        out.push_str("    {\n");
        out.push_str(&format!(
            "      \"canonical\": \"{}\",\n",
            json_escape(group.canonical)
        ));
        out.push_str("      \"aliases\": [");
        let aliases: Vec<String> = group
            .aliases
            .iter()
            .map(|a| format!("\"{}\"", json_escape(a)))
            .collect();
        out.push_str(&aliases.join(", "));
        out.push_str("]\n");
        out.push_str("    }");
        if i + 1 < KIND_GROUPS.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("  ],\n");

    out.push_str("  \"evidence_prefixes\": {\n");
    let canonical: Vec<String> = canonical_evidence_prefixes()
        .map(|p| format!("\"{}\"", json_escape(p)))
        .collect();
    out.push_str(&format!("    \"canonical\": [{}],\n", canonical.join(", ")));
    let local: Vec<String> = local_file_evidence_prefixes()
        .map(|p| format!("\"{}\"", json_escape(p)))
        .collect();
    out.push_str(&format!("    \"local_file\": [{}],\n", local.join(", ")));
    let trace: Vec<String> = traceability_evidence_prefixes()
        .map(|p| format!("\"{}\"", json_escape(p)))
        .collect();
    out.push_str(&format!("    \"traceability\": [{}],\n", trace.join(", ")));
    let recognized: Vec<String> = recognized_evidence_prefixes()
        .map(|p| format!("\"{}\"", json_escape(p)))
        .collect();
    out.push_str(&format!(
        "    \"recognized\": [{}]\n",
        recognized.join(", ")
    ));
    out.push_str("  },\n");

    out.push_str("  \"statuses\": [");
    let statuses: Vec<String> = MatchStatus::ALL
        .iter()
        .map(|s| format!("\"{}\"", json_escape(s.as_str())))
        .collect();
    out.push_str(&statuses.join(", "));
    out.push_str("]\n");

    out.push_str("}\n");
    out
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
#[path = "vocabulary_tests.rs"]
mod tests;
