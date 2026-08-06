use allow_core::json_escape;

use crate::artifacts::MigrateReport;
use crate::migrate_closeout_queues::build_migrate_closeout_next_queues;

/// Migration closeout item-kind/signal vocabulary. These are rendered field
/// values (also used by `cargo-allow worklist --item-kind <X>`). They used to
/// be imported from `allow-policy-legacy`; owning them here lets `allow-report`
/// render migration closeout queues without depending on the legacy crate.
/// See #2941.
pub const BASELINE_DEBT_ITEM_KIND: &str = "baseline_debt";
pub const MISSING_EVIDENCE_ITEM_KIND: &str = "missing_evidence";
pub const NO_NEW_GATE_SIGNAL: &str = "no_new_gate";
pub const NO_NEW_GATE_ITEM_KIND: &str = "no_new";

/// Baseline-debt closeout metadata projected out of the legacy crate.
///
/// `allow-report` renders this without importing `allow-policy-legacy`; the
/// projection is computed by the caller (cargo-allow's migrate load, which
/// already depends on legacy) and threaded in via `MigrateCloseoutInput`.
/// See #2941.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MigrateBaselineDebtProjection {
    pub signal: &'static str,
    pub label: &'static str,
}

impl MigrateBaselineDebtProjection {
    /// The default projection when no legacy lane descriptor resolves the
    /// compat kinds (mirrors `baseline_debt_closeout_metadata(None)`).
    pub const fn default_projection() -> Self {
        Self {
            signal: BASELINE_DEBT_ITEM_KIND,
            label: "baseline debt entries",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrateLegacySource {
    pub file_name: String,
    pub compat_kind: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct MigrateCloseoutInput<'a> {
    pub report: MigrateReport<'a>,
    pub missing_evidence_entries: usize,
    pub legacy_sources: &'a [MigrateLegacySource],
    /// Baseline-debt closeout projection computed by the caller from the
    /// legacy lane descriptors. Replaces the former `legacy_compat_kind_ids`
    /// lookup that forced `allow-report` to depend on `allow-policy-legacy`.
    pub baseline_debt_projection: MigrateBaselineDebtProjection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrateCloseout<'a> {
    pub preserved: MigratePreserved,
    pub baseline_debt: MigrateBaselineDebtCloseout,
    pub evidence_debt: MigrateEvidenceDebt,
    pub next_queues: Vec<MigrateCloseoutQueue>,
    pub legacy_retirement: MigrateLegacyRetirement<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MigratePreserved {
    pub allow_entries: usize,
    pub reviewed_entries: usize,
    pub entries_with_evidence: usize,
    pub evidence_entries: usize,
    pub entries_with_links: usize,
    pub link_entries: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MigrateBaselineDebtCloseout {
    pub entries: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MigrateEvidenceDebt {
    pub broken_evidence_links: usize,
    pub unsafe_broken_evidence_links: usize,
    pub weak_evidence_references: usize,
    pub unsafe_weak_evidence_references: usize,
    pub missing_evidence_entries: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrateCloseoutQueue {
    pub phase: usize,
    pub signal: &'static str,
    pub label: &'static str,
    pub route_kind: &'static str,
    pub item_kind: &'static str,
    pub count: usize,
    pub command: &'static str,
    pub unsafe_command: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrateLegacyRetirementSource<'a> {
    pub file_name: &'a str,
    pub compat_kind: &'a str,
    pub status: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrateLegacyRetirement<'a> {
    pub ready: bool,
    pub blockers: Vec<&'static str>,
    pub sources: Vec<MigrateLegacyRetirementSource<'a>>,
}

pub fn migrate_closeout_from_input(input: MigrateCloseoutInput<'_>) -> MigrateCloseout<'_> {
    let report = input.report;
    let evidence_debt = MigrateEvidenceDebt {
        broken_evidence_links: report.broken_evidence_links.unwrap_or(0),
        unsafe_broken_evidence_links: report.unsafe_broken_evidence_links.unwrap_or(0),
        weak_evidence_references: report.weak_evidence_references.unwrap_or(0),
        unsafe_weak_evidence_references: report.unsafe_weak_evidence_references.unwrap_or(0),
        missing_evidence_entries: input.missing_evidence_entries,
    };
    let blockers = migrate_closeout_blockers(report.baseline_debt, &evidence_debt);
    let next_queues =
        build_migrate_closeout_next_queues(report, &evidence_debt, input.baseline_debt_projection);
    let retirement_status = if blockers.is_empty() {
        "ready"
    } else {
        "blocked"
    };
    let sources = input
        .legacy_sources
        .iter()
        .map(|source| MigrateLegacyRetirementSource {
            file_name: source.file_name.as_str(),
            compat_kind: source.compat_kind,
            status: retirement_status,
        })
        .collect();
    MigrateCloseout {
        preserved: MigratePreserved {
            allow_entries: report.allow_entries,
            reviewed_entries: report.allow_entries.saturating_sub(report.baseline_debt),
            entries_with_evidence: report.entries_with_evidence,
            evidence_entries: report.evidence_entries,
            entries_with_links: report.entries_with_links,
            link_entries: report.link_entries,
        },
        baseline_debt: MigrateBaselineDebtCloseout {
            entries: report.baseline_debt,
        },
        evidence_debt,
        next_queues,
        legacy_retirement: MigrateLegacyRetirement {
            ready: blockers.is_empty(),
            blockers,
            sources,
        },
    }
}

fn migrate_closeout_blockers(
    baseline_debt: usize,
    evidence_debt: &MigrateEvidenceDebt,
) -> Vec<&'static str> {
    let mut blockers = Vec::new();
    if baseline_debt > 0 {
        blockers.push("baseline_debt");
    }
    if evidence_debt.broken_evidence_links > 0 {
        blockers.push("broken_evidence_link");
    }
    if evidence_debt.missing_evidence_entries > 0 {
        blockers.push("missing_evidence");
    }
    if evidence_debt.weak_evidence_references > 0 {
        blockers.push("weak_evidence_reference");
    }
    blockers
}

pub fn append_migrate_closeout_human(closeout: &MigrateCloseout<'_>, out: &mut String) {
    out.push_str("closeout:\n");
    out.push_str(&format!(
        "  preserved.allow_entries: {}\n",
        closeout.preserved.allow_entries
    ));
    out.push_str(&format!(
        "  preserved.reviewed_entries: {}\n",
        closeout.preserved.reviewed_entries
    ));
    out.push_str(&format!(
        "  preserved.entries_with_evidence: {}\n",
        closeout.preserved.entries_with_evidence
    ));
    out.push_str(&format!(
        "  preserved.evidence_entries: {}\n",
        closeout.preserved.evidence_entries
    ));
    out.push_str(&format!(
        "  preserved.entries_with_links: {}\n",
        closeout.preserved.entries_with_links
    ));
    out.push_str(&format!(
        "  preserved.link_entries: {}\n",
        closeout.preserved.link_entries
    ));
    out.push_str(&format!(
        "  baseline_debt.entries: {}\n",
        closeout.baseline_debt.entries
    ));
    append_evidence_debt_human(&closeout.evidence_debt, out);
    if !closeout.next_queues.is_empty() {
        out.push_str("  next_queues:\n");
        for queue in &closeout.next_queues {
            out.push_str(&format!("    {} {}\n", queue.phase, queue.command));
            if let Some(unsafe_command) = queue.unsafe_command {
                out.push_str(&format!("    {} {}\n", queue.phase, unsafe_command));
            }
        }
    }
    out.push_str(&format!(
        "  legacy_retirement.ready: {}\n",
        closeout.legacy_retirement.ready
    ));
    if !closeout.legacy_retirement.blockers.is_empty() {
        out.push_str("  legacy_retirement.blockers:");
        for blocker in &closeout.legacy_retirement.blockers {
            out.push_str(&format!(" {blocker}"));
        }
        out.push('\n');
    }
    if !closeout.legacy_retirement.sources.is_empty() {
        out.push_str("  legacy_retirement.sources:\n");
        for source in &closeout.legacy_retirement.sources {
            out.push_str(&format!(
                "    {} ({}) {}\n",
                source.file_name, source.compat_kind, source.status
            ));
        }
    }
}

fn append_evidence_debt_human(evidence_debt: &MigrateEvidenceDebt, out: &mut String) {
    if evidence_debt.broken_evidence_links > 0 {
        out.push_str(&format!(
            "  evidence_debt.broken_evidence_links: {}\n",
            evidence_debt.broken_evidence_links
        ));
    }
    if evidence_debt.unsafe_broken_evidence_links > 0 {
        out.push_str(&format!(
            "  evidence_debt.unsafe_broken_evidence_links: {}\n",
            evidence_debt.unsafe_broken_evidence_links
        ));
    }
    if evidence_debt.weak_evidence_references > 0 {
        out.push_str(&format!(
            "  evidence_debt.weak_evidence_references: {}\n",
            evidence_debt.weak_evidence_references
        ));
    }
    if evidence_debt.unsafe_weak_evidence_references > 0 {
        out.push_str(&format!(
            "  evidence_debt.unsafe_weak_evidence_references: {}\n",
            evidence_debt.unsafe_weak_evidence_references
        ));
    }
    if evidence_debt.missing_evidence_entries > 0 {
        out.push_str(&format!(
            "  evidence_debt.missing_evidence_entries: {}\n",
            evidence_debt.missing_evidence_entries
        ));
    }
}

pub fn append_migrate_closeout_json(closeout: &MigrateCloseout<'_>, out: &mut String) {
    out.push_str("  \"closeout\": {\n");
    out.push_str("    \"preserved\": {\n");
    out.push_str(&format!(
        "      \"allow_entries\": {},\n",
        closeout.preserved.allow_entries
    ));
    out.push_str(&format!(
        "      \"reviewed_entries\": {},\n",
        closeout.preserved.reviewed_entries
    ));
    out.push_str(&format!(
        "      \"entries_with_evidence\": {},\n",
        closeout.preserved.entries_with_evidence
    ));
    out.push_str(&format!(
        "      \"evidence_entries\": {},\n",
        closeout.preserved.evidence_entries
    ));
    out.push_str(&format!(
        "      \"entries_with_links\": {},\n",
        closeout.preserved.entries_with_links
    ));
    out.push_str(&format!(
        "      \"link_entries\": {}\n",
        closeout.preserved.link_entries
    ));
    out.push_str("    },\n");
    out.push_str("    \"baseline_debt\": {\n");
    out.push_str(&format!(
        "      \"entries\": {}\n",
        closeout.baseline_debt.entries
    ));
    out.push_str("    },\n");
    append_evidence_debt_json(&closeout.evidence_debt, out);
    append_next_queues_json(&closeout.next_queues, out);
    append_legacy_retirement_json(&closeout.legacy_retirement, out);
    out.push_str("  },\n");
}

fn append_evidence_debt_json(evidence_debt: &MigrateEvidenceDebt, out: &mut String) {
    out.push_str("    \"evidence_debt\": {\n");
    let mut fields = Vec::new();
    if evidence_debt.broken_evidence_links > 0 {
        fields.push(format!(
            "      \"broken_evidence_links\": {}",
            evidence_debt.broken_evidence_links
        ));
    }
    if evidence_debt.unsafe_broken_evidence_links > 0 {
        fields.push(format!(
            "      \"unsafe_broken_evidence_links\": {}",
            evidence_debt.unsafe_broken_evidence_links
        ));
    }
    if evidence_debt.weak_evidence_references > 0 {
        fields.push(format!(
            "      \"weak_evidence_references\": {}",
            evidence_debt.weak_evidence_references
        ));
    }
    if evidence_debt.unsafe_weak_evidence_references > 0 {
        fields.push(format!(
            "      \"unsafe_weak_evidence_references\": {}",
            evidence_debt.unsafe_weak_evidence_references
        ));
    }
    if evidence_debt.missing_evidence_entries > 0 {
        fields.push(format!(
            "      \"missing_evidence_entries\": {}",
            evidence_debt.missing_evidence_entries
        ));
    }
    out.push_str(&fields.join(",\n"));
    if !fields.is_empty() {
        out.push('\n');
    }
    out.push_str("    },\n");
}

fn append_next_queues_json(queues: &[MigrateCloseoutQueue], out: &mut String) {
    out.push_str("    \"next_queues\": [\n");
    for (index, queue) in queues.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str("      {\n");
        out.push_str(&format!("        \"phase\": {},\n", queue.phase));
        out.push_str(&format!(
            "        \"signal\": \"{}\",\n",
            json_escape(queue.signal)
        ));
        out.push_str(&format!(
            "        \"label\": \"{}\",\n",
            json_escape(queue.label)
        ));
        out.push_str(&format!(
            "        \"route_kind\": \"{}\",\n",
            json_escape(queue.route_kind)
        ));
        out.push_str(&format!(
            "        \"item_kind\": \"{}\",\n",
            json_escape(queue.item_kind)
        ));
        out.push_str(&format!("        \"count\": {},\n", queue.count));
        out.push_str(&format!(
            "        \"command\": \"{}\"",
            json_escape(queue.command)
        ));
        if let Some(unsafe_command) = queue.unsafe_command {
            out.push_str(",\n");
            out.push_str(&format!(
                "        \"unsafe_command\": \"{}\"",
                json_escape(unsafe_command)
            ));
        }
        out.push_str("\n      }");
    }
    out.push_str("\n    ],\n");
}

fn append_legacy_retirement_json(retirement: &MigrateLegacyRetirement<'_>, out: &mut String) {
    out.push_str("    \"legacy_retirement\": {\n");
    out.push_str(&format!("      \"ready\": {},\n", retirement.ready));
    out.push_str("      \"blockers\": [");
    for (index, blocker) in retirement.blockers.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        out.push_str(&format!("\"{blocker}\""));
    }
    out.push_str("],\n");
    out.push_str("      \"sources\": [\n");
    for (index, source) in retirement.sources.iter().enumerate() {
        if index > 0 {
            out.push_str(",\n");
        }
        out.push_str("        {\n");
        out.push_str(&format!(
            "          \"file_name\": \"{}\",\n",
            json_escape(source.file_name)
        ));
        out.push_str(&format!(
            "          \"compat_kind\": \"{}\",\n",
            json_escape(source.compat_kind)
        ));
        out.push_str(&format!(
            "          \"status\": \"{}\"\n",
            json_escape(source.status)
        ));
        out.push_str("        }");
    }
    out.push_str("\n      ]\n");
    out.push_str("    }\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InventoryContext;

    #[test]
    fn closeout_routes_panic_baseline_with_evidence_as_ready() {
        let legacy_sources = [MigrateLegacySource {
            file_name: "no-panic-baseline.toml".to_string(),
            compat_kind: "panic",
        }];
        let report = clean_panic_baseline_report();
        let closeout = migrate_closeout_from_input(MigrateCloseoutInput {
            report,
            missing_evidence_entries: 0,
            legacy_sources: &legacy_sources,
            baseline_debt_projection: MigrateBaselineDebtProjection {
                signal: BASELINE_DEBT_ITEM_KIND,
                label: "panic baseline debt entries",
            },
        });

        assert_eq!(closeout.preserved.allow_entries, 1);
        assert_eq!(closeout.preserved.reviewed_entries, 1);
        assert_eq!(closeout.preserved.entries_with_evidence, 1);
        assert_eq!(closeout.baseline_debt.entries, 0);
        assert_eq!(closeout.evidence_debt.missing_evidence_entries, 0);
        assert!(closeout.next_queues.is_empty());
        assert!(closeout.legacy_retirement.ready);
        assert!(closeout.legacy_retirement.blockers.is_empty());
        assert_eq!(closeout.legacy_retirement.sources.len(), 1);
        assert_eq!(closeout.legacy_retirement.sources[0].status, "ready");
    }

    #[test]
    fn closeout_routes_panic_baseline_without_evidence_through_baseline_debt_queue() {
        let legacy_sources = [MigrateLegacySource {
            file_name: "no-panic-baseline.toml".to_string(),
            compat_kind: "panic",
        }];
        let report = baseline_debt_panic_report();
        let closeout = migrate_closeout_from_input(MigrateCloseoutInput {
            report,
            missing_evidence_entries: 1,
            legacy_sources: &legacy_sources,
            baseline_debt_projection: MigrateBaselineDebtProjection {
                signal: BASELINE_DEBT_ITEM_KIND,
                label: "panic baseline debt entries",
            },
        });

        assert_eq!(closeout.preserved.reviewed_entries, 0);
        assert_eq!(closeout.baseline_debt.entries, 1);
        assert_eq!(closeout.evidence_debt.missing_evidence_entries, 1);
        assert!(!closeout.legacy_retirement.ready);
        assert_eq!(
            closeout.legacy_retirement.blockers,
            vec!["baseline_debt", "missing_evidence"]
        );
        assert_eq!(closeout.legacy_retirement.sources[0].status, "blocked");
        let [baseline, missing, no_new] = closeout.next_queues.as_slice() else {
            std::panic::panic_any(format!(
                "expected baseline, missing-evidence, and no-new queues, got {:?}",
                closeout.next_queues
            ));
        };
        assert_eq!(baseline.item_kind, "baseline_debt");
        assert_eq!(baseline.label, "panic baseline debt entries");
        assert_eq!(missing.item_kind, "missing_evidence");
        assert_eq!(
            missing.command,
            "cargo-allow worklist --item-kind missing_evidence --format json"
        );
        assert_eq!(no_new.item_kind, "no_new");
    }

    fn clean_panic_baseline_report() -> MigrateReport<'static> {
        MigrateReport {
            inventory: InventoryContext::new(
                "source_tree",
                "policy_migration",
                "filesystem_fallback",
                None,
                None,
            ),
            input_kind: "from",
            input_path: "policy/no-panic-baseline.toml",
            output_path: "policy/allow.toml",
            force: false,
            allow_entries: 1,
            baseline_debt: 0,
            unsafe_entries: 0,
            lint_exception_entries: 0,
            entries_with_evidence: 1,
            evidence_entries: 2,
            entries_with_links: 1,
            link_entries: 1,
            broken_evidence_links: None,
            unsafe_broken_evidence_links: None,
            weak_evidence_references: None,
            unsafe_weak_evidence_references: None,
            notes: "migration notes",
        }
    }

    fn baseline_debt_panic_report() -> MigrateReport<'static> {
        MigrateReport {
            inventory: clean_panic_baseline_report().inventory,
            baseline_debt: 1,
            entries_with_evidence: 0,
            evidence_entries: 0,
            entries_with_links: 1,
            ..clean_panic_baseline_report()
        }
    }
}
