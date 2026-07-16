use allow_core::{AllowEntry, Finding, MatchOutcome, MatchStatus, normalize_path};
use allow_match::finding_location;

pub(super) struct WhyCandidate<'a> {
    pub entry: &'a AllowEntry,
    pub reasons: Vec<String>,
}

pub(super) struct WhyNextSteps {
    pub suggested_actions: Vec<String>,
    pub proof_commands: Vec<String>,
}

pub(super) fn why_next_steps(
    finding: &Finding,
    outcome: &MatchOutcome,
    candidates: &[WhyCandidate<'_>],
) -> WhyNextSteps {
    match outcome.status {
        MatchStatus::New => {
            let mut suggested_actions = vec![
                "Receipt this occurrence with cargo-allow add.".to_string(),
                "Or repair the source so the finding disappears, then re-run cargo-allow check --mode no-new."
                    .to_string(),
            ];
            let mut proof_commands = vec![format!(
                "cargo-allow add --kind {} --path {} --line {} --owner <owner> --reason \"...\" --evidence <ref> --write policy/allow.toml",
                finding.kind.as_str(),
                normalize_path(&finding.path),
                finding.span.as_ref().map(|s| s.line).unwrap_or(1)
            )];
            proof_commands.push("cargo-allow check --mode no-new".to_string());
            if let Some(first) = candidates.first() {
                suggested_actions.push(format!(
                    "Inspect near-miss allow entry `{}` with cargo-allow explain.",
                    first.entry.id
                ));
                proof_commands.push(format!("cargo-allow explain {}", first.entry.id));
            }
            WhyNextSteps {
                suggested_actions,
                proof_commands,
            }
        }
        _ => {
            let mut suggested_actions = Vec::new();
            let mut proof_commands = Vec::new();
            if let Some(id) = &outcome.allow_id {
                suggested_actions.push(format!(
                    "Inspect the linked allow entry `{id}` with cargo-allow explain."
                ));
                proof_commands.push(format!("cargo-allow explain {id}"));
            } else {
                suggested_actions.push(
                    "Inspect the broader ledger with cargo-allow list or cargo-allow check."
                        .to_string(),
                );
            }
            proof_commands.push("cargo-allow check --mode no-new".to_string());
            proof_commands.push("cargo-allow list".to_string());
            WhyNextSteps {
                suggested_actions,
                proof_commands,
            }
        }
    }
}

pub(super) fn render_why_text(
    finding: &Finding,
    outcome: &MatchOutcome,
    candidates: &[WhyCandidate<'_>],
) -> String {
    let next = why_next_steps(finding, outcome, candidates);
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
                        candidate.entry.family.as_deref().unwrap_or("<none>")
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
    for (index, action) in next.suggested_actions.iter().enumerate() {
        out.push_str(&format!("{}. {action}\n", index + 1));
    }
    out.push('\n');
    out.push_str("## Proof commands\n\n");
    for command in &next.proof_commands {
        out.push_str(&format!("- `{command}`\n"));
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

pub(super) fn render_why_json(
    inventory: allow_report::InventoryContext<'_>,
    finding: &Finding,
    outcome: &MatchOutcome,
    candidates: &[WhyCandidate<'_>],
) -> String {
    let next = why_next_steps(finding, outcome, candidates);
    let path_texts = candidates
        .iter()
        .map(|candidate| candidate.entry.path.as_ref().map(normalize_path))
        .collect::<Vec<_>>();
    let candidate_entries = candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| allow_report::WhyCandidateEntry {
            id: candidate.entry.id.as_str(),
            kind: candidate.entry.kind.as_str(),
            family: candidate.entry.family.as_deref(),
            path: path_texts.get(index).and_then(|path| path.as_deref()),
            glob: candidate.entry.glob.as_deref(),
            selector_glob: candidate.entry.selector.glob.as_deref(),
            mismatch_reasons: candidate.reasons.as_slice(),
        })
        .collect::<Vec<_>>();
    allow_report::render_why_json(allow_report::WhyReport {
        inventory,
        finding,
        outcome,
        candidate_entries: &candidate_entries,
        suggested_actions: &next.suggested_actions,
        proof_commands: &next.proof_commands,
    })
}
