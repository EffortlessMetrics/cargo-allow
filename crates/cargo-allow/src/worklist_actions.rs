use allow_core::{AllowEntry, Finding, FindingKind};

use super::worklist_item_kind::{
    AMBIGUOUS_SELECTOR, BASELINE_DEBT, BROAD_SCOPE, BROKEN_EVIDENCE_LINK, EXPIRED_ALLOW,
    INVALID_SELECTOR, MIRROR_DIVERGENCE, MISSING_EVIDENCE, MISSING_REQUIRED_FIELD,
    NEW_UNRECEIPTED_FINDING, OCCURRENCE_HEADROOM, OCCURRENCE_LIMIT_EXCEEDED, REVIEW_DUE,
    STALE_ALLOW, UNSAFE_MISSING_EVIDENCE, WEAK_EVIDENCE_REFERENCE,
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
        OCCURRENCE_HEADROOM => vec![
            "reduce occurrence_limit to the current matched count".to_string(),
            "run cargo-allow diff after tightening to confirm policy improvement".to_string(),
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
            vec![
                "review the retained exception and update evidence or remove it".to_string(),
                "if the entry drifted to a new line, run `cargo-allow refresh <allow-id>` to update last_seen".to_string(),
            ]
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
        MIRROR_DIVERGENCE => vec![
            "sync mirror ledger from canonical or document intentional drain posture".to_string(),
            "review the active drain window closeout and mirror fingerprint".to_string(),
        ],
        _ => vec!["inspect the outcome and update policy or source accordingly".to_string()],
    }
}

pub(crate) fn suggested_actions_for_context(
    kind: &str,
    finding: Option<&Finding>,
    entry: Option<&AllowEntry>,
) -> Vec<String> {
    if kind == MISSING_EVIDENCE
        && let Some(family) = high_risk_policy_exception_family(finding, entry)
    {
        return high_risk_policy_missing_evidence_actions(family);
    }
    if kind == WEAK_EVIDENCE_REFERENCE {
        if unsafe_exception(finding, entry) {
            return unsafe_weak_evidence_actions();
        }
        if let Some(family) = high_risk_policy_exception_family(finding, entry) {
            return high_risk_policy_weak_evidence_actions(family);
        }
    }
    suggested_actions(kind)
}

fn unsafe_exception(finding: Option<&Finding>, entry: Option<&AllowEntry>) -> bool {
    finding
        .map(|finding| finding.kind)
        .or_else(|| entry.map(|entry| entry.kind))
        == Some(FindingKind::Unsafe)
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

fn unsafe_weak_evidence_actions() -> Vec<String> {
    vec![
        "replace weak evidence with unsafe-review, test, spec, or boundary evidence for the unsafe exception".to_string(),
        "keep weak legacy notes only as supporting context after typed unsafe evidence exists"
            .to_string(),
        "keep the selector scoped to the reviewed unsafe boundary".to_string(),
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
    append_closeout_commands(kind, &mut commands);
    append_resolution_commands(kind, finding, entry, &mut commands);
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
        if let Some(list_shortcut_arg) = list_shortcut_arg(kind) {
            commands.push(format!(
                "cargo-allow list --{list_shortcut_arg} --format json"
            ));
        }
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
        if let Some(list_shortcut_arg) = list_shortcut_arg(kind) {
            commands.push(format!(
                "cargo-allow list --{list_shortcut_arg} --format json"
            ));
        }
        commands.push("cargo-allow worklist --format json".to_string());
    }
    if kind == UNSAFE_MISSING_EVIDENCE && !has_unsafe_kind_check {
        commands.push("cargo-allow check --kind unsafe --mode no-new".to_string());
    }
    commands
}

fn append_closeout_commands(kind: &str, commands: &mut Vec<String>) {
    if kind == STALE_ALLOW {
        commands.push("cargo-allow prune --stale --dry-run".to_string());
        commands.push("cargo-allow prune --stale --format json".to_string());
    }
    if kind == REVIEW_DUE {
        commands.push("cargo-allow refresh <allow-id> --dry-run".to_string());
        commands.push("cargo-allow refresh <allow-id> --format json".to_string());
    }
}

/// Add actionable resolution commands for item kinds that currently route only
/// to re-listings. The operator gets a concrete next step toward closing the
/// item, not just another query.
fn append_resolution_commands(
    kind: &str,
    finding: Option<&Finding>,
    entry: Option<&AllowEntry>,
    commands: &mut Vec<String>,
) {
    if kind == NEW_UNRECEIPTED_FINDING
        && let Some(kind_arg) = worklist_kind_arg(finding, entry)
        && let Some(finding) = finding
        && let Some(line) = finding.span.as_ref().map(|span| span.line)
    {
        let path = finding.path.to_string_lossy();
        commands.push(format!(
            "cargo-allow why --kind {kind_arg} --path {path} --line {line} --format json"
        ));
        // Provide a concrete add template with the finding's kind and path
        // pre-filled, leaving --owner and --reason as required placeholders (#3220).
        commands.push(format!(
            "cargo-allow add --kind {kind_arg} --path {path} --line {line} --owner <owner> --reason <reason>"
        ));
    }
}

fn list_shortcut_arg(kind: &str) -> Option<&'static str> {
    match kind {
        EXPIRED_ALLOW => Some("expired"),
        STALE_ALLOW => Some("stale"),
        BASELINE_DEBT => Some("baseline-debt"),
        REVIEW_DUE => Some("review-due"),
        BROAD_SCOPE => Some("broad-scope"),
        MISSING_EVIDENCE | UNSAFE_MISSING_EVIDENCE => Some("missing-evidence"),
        BROKEN_EVIDENCE_LINK => Some("broken-evidence"),
        WEAK_EVIDENCE_REFERENCE => Some("weak-evidence"),
        _ => None,
    }
}

fn worklist_shortcut_arg(kind: &str) -> Option<&'static str> {
    match kind {
        BASELINE_DEBT => Some("baseline-debt"),
        BROAD_SCOPE => Some("broad-scope"),
        MISSING_EVIDENCE | UNSAFE_MISSING_EVIDENCE => Some("missing-evidence"),
        BROKEN_EVIDENCE_LINK => Some("broken-evidence"),
        WEAK_EVIDENCE_REFERENCE => Some("weak-evidence"),
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

#[cfg(test)]
mod tests {
    use allow_core::{
        AllowEntry, Finding, FindingKind, Lifecycle, Selector, Span, StructuralIdentity,
    };
    use std::path::PathBuf;

    use super::*;

    fn entry(kind: FindingKind, family: Option<&str>) -> AllowEntry {
        AllowEntry {
            id: "allow-test".to_string(),
            kind,
            family: family.map(str::to_string),
            path: Some(PathBuf::from("src/lib.rs")),
            glob: None,
            owner: "repo-infra".to_string(),
            classification: "reviewed".to_string(),
            reason: "fixture reason".to_string(),
            evidence: Vec::new(),
            links: Vec::new(),
            occurrence_limit: None,
            lifecycle: Lifecycle::empty(),
            selector: Selector {
                ast_kind: Some("call".to_string()),
                ..Selector::default()
            },
            last_seen: None,
        }
    }

    fn finding(kind: FindingKind, family: Option<&str>) -> Finding {
        Finding {
            kind,
            family: family.map(str::to_string),
            path: PathBuf::from("src/lib.rs"),
            span: Some(Span { line: 1, column: 1 }),
            identity: StructuralIdentity::new("rust", "call"),
            message: "fixture finding".to_string(),
            ledger: None,
        }
    }

    #[test]
    fn context_actions_route_high_risk_and_unsafe_evidence_guidance() {
        let process = finding(FindingKind::PolicyException, Some("process_spawn"));
        assert_eq!(
            suggested_actions_for_context(MISSING_EVIDENCE, Some(&process), None),
            vec![
                "add typed evidence for the policy_exception.process_spawn exception".to_string(),
                "review whether the policy exception can be removed or narrowed before retaining it"
                    .to_string(),
            ]
        );

        let network = entry(FindingKind::PolicyException, Some("network_destination"));
        assert_eq!(
            suggested_actions_for_context(WEAK_EVIDENCE_REFERENCE, None, Some(&network)),
            vec![
                "replace weak evidence with typed evidence for policy_exception.network_destination"
                    .to_string(),
                "keep custom legacy facts only as supporting context after a typed receipt exists"
                    .to_string(),
                "review whether the policy exception can be removed or narrowed before retaining it"
                    .to_string(),
            ]
        );

        let unsafe_finding = finding(FindingKind::Unsafe, None);
        assert_eq!(
            suggested_actions_for_context(WEAK_EVIDENCE_REFERENCE, Some(&unsafe_finding), None),
            vec![
                "replace weak evidence with unsafe-review, test, spec, or boundary evidence for the unsafe exception".to_string(),
                "keep weak legacy notes only as supporting context after typed unsafe evidence exists".to_string(),
                "keep the selector scoped to the reviewed unsafe boundary".to_string(),
            ]
        );

        assert_eq!(
            suggested_actions_for_context(MISSING_EVIDENCE, None, None),
            suggested_actions(MISSING_EVIDENCE)
        );
    }

    #[test]
    fn link_actions_route_traceability_guidance_by_context() {
        let process = entry(FindingKind::PolicyException, Some("process_spawn"));
        assert_eq!(
            suggested_link_actions_for_context(WEAK_EVIDENCE_REFERENCE, None, Some(&process)),
            vec![
                "replace weak traceability with typed traceability for policy_exception.process_spawn"
                    .to_string(),
                "keep custom legacy notes only as supporting context after a typed traceability reference exists"
                    .to_string(),
                "review whether the policy exception can be removed or narrowed before retaining it"
                    .to_string(),
            ]
        );

        assert_eq!(
            suggested_link_actions_for_context(BROKEN_EVIDENCE_LINK, None, None),
            vec![
                "restore or commit the referenced local traceability file".to_string(),
                "or update the link reference to a valid source-tree-relative path".to_string(),
            ]
        );

        let generic_weak = suggested_link_actions_for_context(WEAK_EVIDENCE_REFERENCE, None, None);
        assert_eq!(
            generic_weak.first().map(String::as_str),
            Some("replace the weak link string with a typed traceability reference")
        );
        assert_eq!(
            suggested_link_actions_for_context(REVIEW_DUE, None, None),
            suggested_actions(REVIEW_DUE)
        );
    }

    #[test]
    fn high_risk_policy_family_uses_finding_before_entry_and_filters_other_kinds() {
        let process_finding = finding(FindingKind::PolicyException, Some("process_spawn"));
        let network_entry = entry(FindingKind::PolicyException, Some("network_destination"));
        assert_eq!(
            high_risk_policy_exception_family(Some(&process_finding), Some(&network_entry)),
            Some("process_spawn")
        );

        let dependency = finding(FindingKind::PolicyException, Some("dependency_surface"));
        assert_eq!(
            high_risk_policy_exception_family(Some(&dependency), Some(&network_entry)),
            None
        );

        let unsafe_entry = entry(FindingKind::Unsafe, Some("process_spawn"));
        assert_eq!(
            high_risk_policy_exception_family(None, Some(&unsafe_entry)),
            None
        );
    }

    #[test]
    fn unsafe_exception_detects_finding_or_entry_kind() {
        let unsafe_finding = finding(FindingKind::Unsafe, None);
        let panic_entry = entry(FindingKind::Panic, None);
        assert!(unsafe_exception(Some(&unsafe_finding), Some(&panic_entry)));

        let unsafe_entry = entry(FindingKind::Unsafe, None);
        assert!(unsafe_exception(None, Some(&unsafe_entry)));

        let panic_finding = finding(FindingKind::Panic, None);
        assert!(!unsafe_exception(Some(&panic_finding), Some(&unsafe_entry)));
        assert!(!unsafe_exception(None, None));
    }

    #[test]
    fn list_shortcut_arg_maps_all_listing_shortcuts_and_unknowns() {
        let cases = [
            (EXPIRED_ALLOW, Some("expired")),
            (STALE_ALLOW, Some("stale")),
            (BASELINE_DEBT, Some("baseline-debt")),
            (REVIEW_DUE, Some("review-due")),
            (BROAD_SCOPE, Some("broad-scope")),
            (MISSING_EVIDENCE, Some("missing-evidence")),
            (UNSAFE_MISSING_EVIDENCE, Some("missing-evidence")),
            (BROKEN_EVIDENCE_LINK, Some("broken-evidence")),
            (WEAK_EVIDENCE_REFERENCE, Some("weak-evidence")),
            (NEW_UNRECEIPTED_FINDING, None),
            ("future_kind", None),
        ];

        for (kind, expected) in cases {
            assert_eq!(list_shortcut_arg(kind), expected, "{kind}");
        }
    }

    #[test]
    fn worklist_shortcut_arg_maps_actionable_shortcuts_and_unknowns() {
        let cases = [
            (BASELINE_DEBT, Some("baseline-debt")),
            (BROAD_SCOPE, Some("broad-scope")),
            (MISSING_EVIDENCE, Some("missing-evidence")),
            (UNSAFE_MISSING_EVIDENCE, Some("missing-evidence")),
            (BROKEN_EVIDENCE_LINK, Some("broken-evidence")),
            (WEAK_EVIDENCE_REFERENCE, Some("weak-evidence")),
            (EXPIRED_ALLOW, None),
            (STALE_ALLOW, None),
            ("future_kind", None),
        ];

        for (kind, expected) in cases {
            assert_eq!(worklist_shortcut_arg(kind), expected, "{kind}");
        }
    }

    #[test]
    fn worklist_kind_arg_maps_source_finding_kinds_and_absent_context() {
        let cases = [
            (FindingKind::Panic, None, Some("panic")),
            (FindingKind::Unsafe, None, Some("unsafe")),
            (FindingKind::LintException, None, Some("lint-exception")),
            (FindingKind::NonRustFile, None, Some("non-rust")),
            (FindingKind::GeneratedCode, None, Some("generated")),
            (
                FindingKind::PolicyException,
                Some("dependency_surface"),
                Some("dependency-surface"),
            ),
            (FindingKind::PolicyException, Some("unknown"), None),
        ];

        for (kind, family, expected) in cases {
            let finding = finding(kind, family);
            assert_eq!(worklist_kind_arg(Some(&finding), None), expected);

            let entry = entry(kind, family);
            assert_eq!(worklist_kind_arg(None, Some(&entry)), expected);
        }

        assert_eq!(worklist_kind_arg(None, None), None);
    }

    #[test]
    fn policy_exception_kind_arg_maps_known_policy_families() {
        let cases = [
            (Some("executable_file"), Some("executable")),
            (Some("github_workflow"), Some("workflow")),
            (Some("workflow_external_action"), Some("workflow")),
            (Some("dependency_surface"), Some("dependency-surface")),
            (Some("process_spawn"), Some("process")),
            (Some("network_destination"), Some("network")),
            (Some("other_policy"), None),
            (None, None),
        ];

        for (family, expected) in cases {
            assert_eq!(policy_exception_kind_arg(family), expected, "{family:?}");
        }
    }
}
