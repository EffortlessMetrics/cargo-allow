use allow_core::{AllowEntry, Finding, FindingKind};

pub(crate) fn suggested_actions(kind: &str) -> Vec<String> {
    match kind {
        "new_unreceipted_finding" => vec![
            "remove the new source exception if it is accidental".to_string(),
            "or add a reviewed allow entry with owner, reason, scope, evidence, and lifecycle"
                .to_string(),
        ],
        "occurrence_limit_exceeded" => vec![
            "reduce the current findings back to the baseline count".to_string(),
            "or split the added occurrence into a reviewed allow entry".to_string(),
        ],
        "expired_allow" => vec![
            "remove the expired allow if the exception is gone".to_string(),
            "or re-review with fresh evidence before changing lifecycle dates".to_string(),
        ],
        "stale_allow" => vec![
            "remove the stale allow entry if the exception no longer exists".to_string(),
            "or narrow/update the selector if the code moved without broadening scope".to_string(),
        ],
        "ambiguous_selector" => vec![
            "narrow selectors so each finding matches exactly one allow entry".to_string(),
            "prefer structural fields such as container, callee, lint, and snippet hash"
                .to_string(),
        ],
        "unsafe_missing_evidence" => vec![
            "add unsafe-review, test, spec, or boundary evidence for the unsafe exception"
                .to_string(),
            "keep the selector scoped to the reviewed unsafe boundary".to_string(),
        ],
        "missing_evidence" => {
            vec!["add evidence that supports the exception reason".to_string()]
        }
        "missing_required_field" => vec![
            "fill the required owner, reason, classification, lifecycle, or evidence field"
                .to_string(),
        ],
        "invalid_selector" => {
            vec!["replace line-only or invalid selector data with structural identity".to_string()]
        }
        "baseline_debt" => vec![
            "replace generated baseline debt with a reviewed allow entry".to_string(),
            "or remove the underlying exception".to_string(),
        ],
        "review_due" => {
            vec!["review the retained exception and update evidence or remove it".to_string()]
        }
        "broad_scope" => vec![
            "replace the broad glob with exact paths or a narrower glob where practical"
                .to_string(),
            "keep broad source-tree scope intentional, reviewed, and evidenced".to_string(),
        ],
        _ => vec!["inspect the outcome and update policy or source accordingly".to_string()],
    }
}

pub(crate) fn proof_commands(
    kind: &str,
    finding: Option<&Finding>,
    entry: Option<&AllowEntry>,
) -> Vec<String> {
    let mut commands = Vec::new();
    if let Some(allow_id) = entry.map(|entry| entry.id.as_str()) {
        commands.push(format!("cargo-allow explain {allow_id}"));
    }
    if let Some(kind_arg) = worklist_kind_arg(finding, entry) {
        commands.push(format!("cargo-allow check --kind {kind_arg} --mode no-new"));
        if let Some(shortcut_arg) = worklist_shortcut_arg(kind) {
            commands.push(format!(
                "cargo-allow worklist --{shortcut_arg} --format json"
            ));
        }
        commands.push(format!(
            "cargo-allow worklist --kind {kind_arg} --format json"
        ));
    } else {
        commands.push("cargo-allow check --mode no-new".to_string());
        commands.push("cargo-allow worklist --format json".to_string());
    }
    if kind == "unsafe_missing_evidence" && !commands.iter().any(|cmd| cmd.contains("unsafe")) {
        commands.push("cargo-allow check --kind unsafe --mode no-new".to_string());
    }
    commands
}

fn worklist_shortcut_arg(kind: &str) -> Option<&'static str> {
    match kind {
        "baseline_debt" => Some("baseline-debt"),
        "broad_scope" => Some("broad-scope"),
        _ => None,
    }
}

fn worklist_kind_arg(
    finding: Option<&Finding>,
    entry: Option<&AllowEntry>,
) -> Option<&'static str> {
    let exception_kind = finding
        .map(|finding| finding.kind)
        .or_else(|| entry.map(|entry| entry.kind))?;
    match exception_kind {
        FindingKind::Panic => Some("panic"),
        FindingKind::Unsafe => Some("unsafe"),
        FindingKind::LintException => Some("lint-exception"),
        FindingKind::NonRustFile => Some("non-rust"),
        FindingKind::GeneratedCode => Some("generated"),
        FindingKind::PolicyException => policy_exception_kind_arg(
            finding
                .and_then(|finding| finding.family.as_deref())
                .or_else(|| entry.and_then(|entry| entry.family.as_deref())),
        ),
    }
}

fn policy_exception_kind_arg(family: Option<&str>) -> Option<&'static str> {
    match family {
        Some("executable_file") => Some("executable"),
        Some("github_workflow" | "workflow_external_action") => Some("workflow"),
        Some("dependency_surface") => Some("dependency-surface"),
        Some("process_spawn") => Some("process"),
        Some("network_destination") => Some("network"),
        _ => None,
    }
}
