use allow_core::{AllowEntry, Finding, FindingKind};

use super::worklist_item_kind::{
    AMBIGUOUS_SELECTOR, BASELINE_DEBT, BROAD_SCOPE, BROKEN_EVIDENCE_LINK, EXPIRED_ALLOW,
    INVALID_SELECTOR, MISSING_EVIDENCE, MISSING_REQUIRED_FIELD, NEW_UNRECEIPTED_FINDING,
    OCCURRENCE_LIMIT_EXCEEDED, REVIEW_DUE, STALE_ALLOW, UNSAFE_MISSING_EVIDENCE,
    WEAK_EVIDENCE_REFERENCE,
};

pub(crate) fn suggested_actions(kind: &str) -> Vec<String> {
    match kind {
        NEW_UNRECEIPTED_FINDING => vec![
            "remove the new source exception if it is accidental".to_string(),
            "or add a reviewed allow entry with owner, reason, scope, evidence, and lifecycle"
                .to_string(),
        ],
        OCCURRENCE_LIMIT_EXCEEDED => vec![
            "reduce the current findings back to the baseline count".to_string(),
            "or split the added occurrence into a reviewed allow entry".to_string(),
        ],
        EXPIRED_ALLOW => vec![
            "remove the expired allow if the exception is gone".to_string(),
            "or re-review with fresh evidence before changing lifecycle dates".to_string(),
        ],
        STALE_ALLOW => vec![
            "remove the stale allow entry if the exception no longer exists".to_string(),
            "or narrow/update the selector if the code moved without broadening scope".to_string(),
        ],
        AMBIGUOUS_SELECTOR => vec![
            "narrow selectors so each finding matches exactly one allow entry".to_string(),
            "prefer structural fields such as container, callee, lint, and snippet hash"
                .to_string(),
        ],
        UNSAFE_MISSING_EVIDENCE => vec![
            "add unsafe-review, test, spec, or boundary evidence for the unsafe exception"
                .to_string(),
            "keep the selector scoped to the reviewed unsafe boundary".to_string(),
        ],
        MISSING_EVIDENCE => {
            vec!["add evidence that supports the exception reason".to_string()]
        }
        MISSING_REQUIRED_FIELD => vec![
            "fill the required owner, reason, classification, lifecycle, or evidence field"
                .to_string(),
        ],
        INVALID_SELECTOR => {
            vec!["replace line-only or invalid selector data with structural identity".to_string()]
        }
        BASELINE_DEBT => vec![
            "replace generated baseline debt with a reviewed allow entry".to_string(),
            "or remove the underlying exception".to_string(),
        ],
        REVIEW_DUE => {
            vec!["review the retained exception and update evidence or remove it".to_string()]
        }
        BROAD_SCOPE => vec![
            "replace the broad glob with exact paths or a narrower glob where practical"
                .to_string(),
            "keep broad source-tree scope intentional, reviewed, and evidenced".to_string(),
        ],
        BROKEN_EVIDENCE_LINK => vec![
            "restore or commit the referenced local evidence artifact".to_string(),
            "or update the evidence reference to a valid source-tree-relative path".to_string(),
        ],
        WEAK_EVIDENCE_REFERENCE => vec![
            "replace the weak evidence string with a typed evidence reference".to_string(),
            format!(
                "use a recognized prefix such as {}",
                evidence_prefix_examples()
            ),
        ],
        _ => vec!["inspect the outcome and update policy or source accordingly".to_string()],
    }
}

pub(crate) fn suggested_actions_for_context(
    kind: &str,
    finding: Option<&Finding>,
    entry: Option<&AllowEntry>,
) -> Vec<String> {
    if kind == MISSING_EVIDENCE {
        if let Some(family) = high_risk_policy_exception_family(finding, entry) {
            return high_risk_policy_missing_evidence_actions(family);
        }
    }
    if kind == WEAK_EVIDENCE_REFERENCE {
        if let Some(family) = high_risk_policy_exception_family(finding, entry) {
            return high_risk_policy_weak_evidence_actions(family);
        }
    }
    suggested_actions(kind)
}

pub(crate) fn suggested_link_actions_for_context(
    kind: &str,
    finding: Option<&Finding>,
    entry: Option<&AllowEntry>,
) -> Vec<String> {
    match kind {
        BROKEN_EVIDENCE_LINK => vec![
            "restore or commit the referenced local traceability file".to_string(),
            "or update the link reference to a valid source-tree-relative path".to_string(),
        ],
        WEAK_EVIDENCE_REFERENCE => {
            if let Some(family) = high_risk_policy_exception_family(finding, entry) {
                return high_risk_policy_weak_link_actions(family);
            }
            vec![
                "replace the weak link string with a typed traceability reference".to_string(),
                format!(
                    "use a recognized prefix such as {}",
                    evidence_prefix_examples()
                ),
            ]
        }
        _ => suggested_actions(kind),
    }
}

pub(super) fn evidence_prefix_examples() -> String {
    let prefixes = allow_policy::canonical_evidence_prefixes()
        .map(|prefix| format!("{prefix}:"))
        .collect::<Vec<_>>();
    english_join(&prefixes)
}

fn english_join(values: &[String]) -> String {
    match values {
        [] => String::new(),
        [one] => one.clone(),
        [one, two] => format!("{one} or {two}"),
        _ => {
            let mut out = String::new();
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    if index + 1 == values.len() {
                        out.push_str(", or ");
                    } else {
                        out.push_str(", ");
                    }
                }
                out.push_str(value);
            }
            out
        }
    }
}

fn high_risk_policy_exception_family<'a>(
    finding: Option<&'a Finding>,
    entry: Option<&'a AllowEntry>,
) -> Option<&'a str> {
    let exception_kind = finding
        .map(|finding| finding.kind)
        .or_else(|| entry.map(|entry| entry.kind));
    if exception_kind != Some(FindingKind::PolicyException) {
        return None;
    }
    let family = finding
        .and_then(|finding| finding.family.as_deref())
        .or_else(|| entry.and_then(|entry| entry.family.as_deref()))?;
    matches!(family, "process_spawn" | "network_destination").then_some(family)
}

fn high_risk_policy_missing_evidence_actions(family: &str) -> Vec<String> {
    vec![
        format!("add typed evidence for the policy_exception.{family} exception"),
        "review whether the policy exception can be removed or narrowed before retaining it"
            .to_string(),
    ]
}

fn high_risk_policy_weak_evidence_actions(family: &str) -> Vec<String> {
    vec![
        format!("replace weak evidence with typed evidence for policy_exception.{family}"),
        "keep custom legacy facts only as supporting context after a typed receipt exists"
            .to_string(),
        "review whether the policy exception can be removed or narrowed before retaining it"
            .to_string(),
    ]
}

fn high_risk_policy_weak_link_actions(family: &str) -> Vec<String> {
    vec![
        format!(
            "replace weak traceability with typed traceability for policy_exception.{family}"
        ),
        "keep custom legacy notes only as supporting context after a typed traceability reference exists"
            .to_string(),
        "review whether the policy exception can be removed or narrowed before retaining it"
            .to_string(),
    ]
}

pub(crate) fn proof_commands(
    kind: &str,
    finding: Option<&Finding>,
    entry: Option<&AllowEntry>,
) -> Vec<String> {
    let mut commands = Vec::new();
    if let Some(allow_id) = entry.map(|entry| entry.id.as_str()) {
        commands.push(format!("cargo-allow explain {allow_id}"));
        commands.push(format!(
            "cargo-allow list --allow-id {allow_id} --format json"
        ));
        commands.push(format!(
            "cargo-allow worklist --allow-id {allow_id} --format json"
        ));
    }
    let kind_arg = worklist_kind_arg(finding, entry);
    let has_unsafe_kind_check = kind_arg == Some("unsafe");
    let shortcut_arg = worklist_shortcut_arg(kind);
    if let Some(kind_arg) = kind_arg {
        if let Some(shortcut_arg) = shortcut_arg {
            commands.push(format!(
                "cargo-allow worklist --{shortcut_arg} --format json"
            ));
        }
        commands.push(format!("cargo-allow check --kind {kind_arg} --mode no-new"));
        commands.push(format!(
            "cargo-allow worklist --item-kind {kind} --format json"
        ));
        commands.push(format!(
            "cargo-allow worklist --kind {kind_arg} --format json"
        ));
    } else {
        if let Some(shortcut_arg) = shortcut_arg {
            commands.push(format!(
                "cargo-allow worklist --{shortcut_arg} --format json"
            ));
        }
        commands.push("cargo-allow check --mode no-new".to_string());
        commands.push(format!(
            "cargo-allow worklist --item-kind {kind} --format json"
        ));
        commands.push("cargo-allow worklist --format json".to_string());
    }
    if kind == UNSAFE_MISSING_EVIDENCE && !has_unsafe_kind_check {
        commands.push("cargo-allow check --kind unsafe --mode no-new".to_string());
    }
    commands
}

fn worklist_shortcut_arg(kind: &str) -> Option<&'static str> {
    match kind {
        BASELINE_DEBT => Some("baseline-debt"),
        BROAD_SCOPE => Some("broad-scope"),
        MISSING_EVIDENCE | UNSAFE_MISSING_EVIDENCE => Some("missing-evidence"),
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
