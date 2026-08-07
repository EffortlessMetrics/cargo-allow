use crate::MigrateReport;
use crate::migrate::{
    MigrateEvidenceRepairQueue, MigrateFollowUpQueue, migrate_evidence_repair_queues,
};
use crate::migrate_closeout::{
    BASELINE_DEBT_ITEM_KIND, MISSING_EVIDENCE_ITEM_KIND, MigrateBaselineDebtProjection,
    MigrateCloseoutQueue, MigrateEvidenceDebt, NO_NEW_GATE_ITEM_KIND, NO_NEW_GATE_SIGNAL,
};

const CHECK_NO_NEW_COMMAND: &str = "cargo-allow check --mode no-new --format markdown --receipt target/cargo-allow/check.receipt.json --output target/cargo-allow/check.md";
const MISSING_EVIDENCE_COMMAND: &str =
    "cargo-allow worklist --item-kind missing_evidence --format json";

pub(crate) fn baseline_debt_follow_up_queue(
    count: usize,
    projection: MigrateBaselineDebtProjection,
) -> Option<MigrateFollowUpQueue> {
    if count == 0 {
        return None;
    }
    Some(MigrateFollowUpQueue {
        signal: projection.signal,
        label: projection.label,
        route_kind: "worklist_item_kind",
        item_kind: BASELINE_DEBT_ITEM_KIND,
        count,
        command: baseline_debt_command(),
    })
}

pub(crate) fn migrate_follow_up_queues_for_legacy(
    report: MigrateReport<'_>,
    projection: MigrateBaselineDebtProjection,
) -> Vec<MigrateFollowUpQueue> {
    baseline_debt_follow_up_queue(report.baseline_debt, projection)
        .into_iter()
        .collect()
}

pub(crate) fn build_migrate_closeout_next_queues(
    report: MigrateReport<'_>,
    evidence_debt: &MigrateEvidenceDebt,
    projection: MigrateBaselineDebtProjection,
) -> Vec<MigrateCloseoutQueue> {
    let mut queues = Vec::new();
    let mut phase = 1usize;

    for queue in migrate_follow_up_queues_for_legacy(report, projection) {
        queues.push(closeout_queue_from_follow_up(queue, phase));
        phase += 1;
    }
    for queue in migrate_evidence_repair_queues(report) {
        queues.push(closeout_queue_from_evidence_repair(queue, phase));
        phase += 1;
    }
    if evidence_debt.missing_evidence_entries > 0
        && !queues
            .iter()
            .any(|queue| queue.item_kind == MISSING_EVIDENCE_ITEM_KIND)
    {
        queues.push(MigrateCloseoutQueue {
            phase,
            signal: MISSING_EVIDENCE_ITEM_KIND,
            label: "missing evidence entries",
            route_kind: "worklist_item_kind",
            item_kind: MISSING_EVIDENCE_ITEM_KIND,
            count: evidence_debt.missing_evidence_entries,
            command: MISSING_EVIDENCE_COMMAND,
            unsafe_command: None,
        });
        phase += 1;
    }
    if !queues.is_empty() {
        queues.push(MigrateCloseoutQueue {
            phase,
            signal: NO_NEW_GATE_SIGNAL,
            label: "no-new guard after closeout edits",
            route_kind: "check_mode",
            item_kind: NO_NEW_GATE_ITEM_KIND,
            count: 0,
            command: CHECK_NO_NEW_COMMAND,
            unsafe_command: None,
        });
    }
    queues
}

fn baseline_debt_command() -> &'static str {
    "cargo-allow worklist --item-kind baseline_debt --format json"
}

fn closeout_queue_from_follow_up(
    queue: MigrateFollowUpQueue,
    phase: usize,
) -> MigrateCloseoutQueue {
    MigrateCloseoutQueue {
        phase,
        signal: queue.signal,
        label: queue.label,
        route_kind: queue.route_kind,
        item_kind: queue.item_kind,
        count: queue.count,
        command: queue.command,
        unsafe_command: None,
    }
}

fn closeout_queue_from_evidence_repair(
    queue: MigrateEvidenceRepairQueue,
    phase: usize,
) -> MigrateCloseoutQueue {
    MigrateCloseoutQueue {
        phase,
        signal: queue.signal,
        label: queue.label,
        route_kind: queue.route_kind,
        item_kind: queue.item_kind,
        count: queue.count,
        command: queue.command,
        unsafe_command: queue.unsafe_command,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InventoryContext;

    #[test]
    fn closeout_queues_use_panic_baseline_label_from_descriptor() {
        let report = baseline_debt_report();
        let evidence_debt = MigrateEvidenceDebt {
            broken_evidence_links: 0,
            unsafe_broken_evidence_links: 0,
            weak_evidence_references: 0,
            unsafe_weak_evidence_references: 0,
            missing_evidence_entries: 0,
        };

        let queues = build_migrate_closeout_next_queues(
            report,
            &evidence_debt,
            MigrateBaselineDebtProjection {
                signal: BASELINE_DEBT_ITEM_KIND,
                label: "panic baseline debt entries",
            },
        );

        let [baseline, no_new] = queues.as_slice() else {
            std::panic::panic_any(format!(
                "expected baseline and no-new queues, got {queues:?}"
            ));
        };
        assert_eq!(baseline.label, "panic baseline debt entries");
        assert_eq!(baseline.item_kind, BASELINE_DEBT_ITEM_KIND);
        assert_eq!(no_new.item_kind, NO_NEW_GATE_ITEM_KIND);
    }

    fn baseline_debt_report() -> MigrateReport<'static> {
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
            baseline_debt: 1,
            unsafe_entries: 0,
            lint_exception_entries: 0,
            entries_with_evidence: 0,
            evidence_entries: 0,
            entries_with_links: 0,
            link_entries: 0,
            broken_evidence_links: None,
            unsafe_broken_evidence_links: None,
            weak_evidence_references: None,
            unsafe_weak_evidence_references: None,
            notes: "migration notes",
        }
    }
}
