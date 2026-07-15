use allow_core::{AllowEntry, Finding, MatchOutcome, MatchStatus, normalize_path};
use allow_match::finding_location;

pub(super) struct WhyCandidate<'a> {
    pub entry: &'a AllowEntry,
    pub reasons: Vec<String>,
}

pub(super) fn render_why_text(
    finding: &Finding,
    outcome: &MatchOutcome,
    candidates: &[WhyCandidate<'_>],
) -> String {
    let mut out = String::new();
    out.push_str("# Why this finding is unreceipted\n\n");
    out.push_str("## Finding\n\n");
    out.push_str(&format!("- location: {}\n", finding_location(finding)));
    out.push_str(&format!("- kind: {}\n", finding.kind.as_str()));
    match &finding.family {
        Some(family) => out.push_str(&format!("- family: {family}\n")),
        None => out.push_str("- family: <none>\n"),
    }
    out.push_str(&format!("- message: {}\n", finding.message));
    out.push_str(&format!(
        "- identity.ast_kind: {}\n",
        finding.identity.ast_kind
    ));
    if let Some(container) = &finding.identity.container {
        out.push_str(&format!("- identity.container: {container}\n"));
    }
    if let Some(callee) = &finding.identity.callee {
        out.push_str(&format!("- identity.callee: {callee}\n"));
    }
    if let Some(hash) = &finding.identity.normalized_snippet_hash {
        out.push_str(&format!("- identity.normalized_snippet_hash: {hash}\n"));
    }
    out.push('\n');

    out.push_str("## Current posture\n\n");
    out.push_str(&format!("- status: {}\n", outcome.status.as_str()));
    match &outcome.allow_id {
        Some(id) => out.push_str(&format!("- allow_id: {id}\n")),
        None => out.push_str("- allow_id: <none>\n"),
    }
    if !outcome.candidate_ids.is_empty() {
        out.push_str(&format!(
            "- candidate_ids: {}\n",
            outcome.candidate_ids.join(", ")
        ));
    }
    if !outcome.message.is_empty() {
        out.push_str(&format!("- message: {}\n", outcome.message));
    }
    out.push('\n');

    match outcome.status {
        MatchStatus::New => {
            out.push_str("## Candidate allow entries\n\n");
            if candidates.is_empty() {
                out.push_str(
                    "No same-kind allow entries look related to this finding. \
The finding is unreceipted because no policy entry covers it.\n\n",
                );
            } else {
                out.push_str(
                    "Nearby same-kind entries that do not match, with selector mismatch reasons:\n\n",
                );
                for candidate in candidates {
                    out.push_str(&format!("### `{}`\n\n", candidate.entry.id));
                    out.push_str(&format!(
                        "- kind/family: {} / {}\n",
                        candidate.entry.kind.as_str(),
                        candidate
                            .entry
                            .family
                            .as_deref()
                            .unwrap_or("<none>")
                    ));
                    out.push_str(&format!(
                        "- path: {}\n",
                        candidate
                            .entry
                            .path
                            .as_ref()
                            .map(normalize_path)
                            .unwrap_or_else(|| "<none>".to_string())
                    ));
                    if let Some(glob) = &candidate.entry.glob {
                        out.push_str(&format!("- glob: {glob}\n"));
                    }
                    if let Some(glob) = &candidate.entry.selector.glob {
                        out.push_str(&format!("- selector.glob: {glob}\n"));
                    }
                    out.push_str("- mismatch reasons:\n");
                    for reason in &candidate.reasons {
                        out.push_str(&format!("  - {reason}\n"));
                    }
                    out.push('\n');
                }
            }
        }
        MatchStatus::Matched
        | MatchStatus::LocationDrift
        | MatchStatus::ReviewDue
        | MatchStatus::BaselineDebt
        | MatchStatus::EvidenceMissing => {
            out.push_str("## Already receipted\n\n");
            if let Some(id) = &outcome.allow_id {
                out.push_str(&format!(
                    "This finding is already linked to `{id}`. \
Use `cargo-allow explain {id}` for the entry-facing inverse view.\n\n"
                ));
            } else {
                out.push_str(
                    "This finding is not in a plain `new` posture. \
Use `cargo-allow list` or `cargo-allow check` for the broader ledger view.\n\n",
                );
            }
        }
        MatchStatus::Ambiguous => {
            out.push_str("## Ambiguous match\n\n");
            out.push_str(
                "Multiple allow entries compete for this finding. \
Inspect the candidate IDs with `cargo-allow explain <id>` and narrow selectors.\n\n",
            );
        }
        MatchStatus::Stale
        | MatchStatus::Expired
        | MatchStatus::InvalidSelector
        | MatchStatus::MissingRequiredField => {
            out.push_str("## Policy health issue\n\n");
            out.push_str(
                "The selected finding is blocked by a policy-health status rather than a simple unreceipted gap. \
Use `cargo-allow explain <id>` when an allow ID is present, or `cargo-allow check --mode strict` for the full gate view.\n\n",
            );
        }
    }

    out.push_str("## Suggested next steps\n\n");
    match outcome.status {
        MatchStatus::New => {
            out.push_str(&format!(
                "1. Receipt this occurrence: `cargo-allow add --kind {} --path {} --line {} --owner <owner> --reason \"...\" --evidence <ref> --write policy/allow.toml`\n",
                finding.kind.as_str(),
                normalize_path(&finding.path),
                finding.span.as_ref().map(|s| s.line).unwrap_or(1)
            ));
            out.push_str(
                "2. Or repair the source so the finding disappears, then re-run `cargo-allow check --mode no-new`.\n",
            );
            if let Some(first) = candidates.first() {
                out.push_str(&format!(
                    "3. Inspect a near-miss entry: `cargo-allow explain {}`\n",
                    first.entry.id
                ));
            }
        }
        _ => {
            if let Some(id) = &outcome.allow_id {
                out.push_str(&format!("1. `cargo-allow explain {id}`\n"));
            }
            out.push_str("2. `cargo-allow check --mode no-new`\n");
            out.push_str("3. `cargo-allow list`\n");
        }
    }
    out.push('\n');

    out.push_str("## Claim boundary\n\n");
    out.push_str(
        "`why` reports source-tree / source-syntax matching posture only. \
It does not prove that an exception is safe, that tests are adequate, or that \
macro-expanded / type-aware behavior would match.\n",
    );
    out
}
