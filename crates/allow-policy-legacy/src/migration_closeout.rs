//! Closeout queue routing metadata derived from migration lane descriptors.

use crate::{DebtPolicy, LegacyLaneDescriptor, descriptor_for_compat_kind_id};

pub const BASELINE_DEBT_ITEM_KIND: &str = "baseline_debt";
pub const MISSING_EVIDENCE_ITEM_KIND: &str = "missing_evidence";
pub const NO_NEW_GATE_SIGNAL: &str = "no_new_gate";
pub const NO_NEW_GATE_ITEM_KIND: &str = "no_new";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MigrationCloseoutBaselineDebt {
    pub signal: &'static str,
    pub label: &'static str,
    pub queue_id: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationDebtClass {
    BaselineDebt,
    MissingEvidence,
}

pub fn migration_closeout_baseline_debt(
    descriptor: &LegacyLaneDescriptor,
) -> MigrationCloseoutBaselineDebt {
    let signal = descriptor
        .closeout_queue
        .map(|hints| hints.phase)
        .unwrap_or(BASELINE_DEBT_ITEM_KIND);
    let queue_id = descriptor.closeout_queue.map(|hints| hints.queue_id);
    MigrationCloseoutBaselineDebt {
        signal,
        label: baseline_debt_label(queue_id),
        queue_id,
    }
}

pub fn baseline_debt_closeout_metadata(
    descriptor: Option<&LegacyLaneDescriptor>,
) -> MigrationCloseoutBaselineDebt {
    descriptor
        .map(migration_closeout_baseline_debt)
        .unwrap_or(MigrationCloseoutBaselineDebt {
            signal: BASELINE_DEBT_ITEM_KIND,
            label: "baseline debt entries",
            queue_id: None,
        })
}

/// Project the baseline-debt closeout `(signal, label)` for a set of legacy
/// compat-kind ids, without forcing the caller to depend on legacy internals.
///
/// Callers that render migration closeout queues (e.g. `allow-report` via
/// `cargo-allow`) use this to obtain the projection at load time and pass it
/// in, so `allow-report` no longer needs a direct dependency on this crate.
/// See #2941.
pub fn baseline_debt_projection(compat_kind_ids: &[&str]) -> (&'static str, &'static str) {
    let metadata = baseline_debt_closeout_metadata(primary_legacy_descriptor(compat_kind_ids));
    (metadata.signal, metadata.label)
}

pub fn migration_debt_classes(descriptor: &LegacyLaneDescriptor) -> &'static [MigrationDebtClass] {
    match descriptor.debt_policy {
        DebtPolicy::None => &[],
        DebtPolicy::VisibleBaselineDebt => &[MigrationDebtClass::BaselineDebt],
        DebtPolicy::MissingEvidenceTodo => &[MigrationDebtClass::MissingEvidence],
    }
}

pub fn primary_legacy_descriptor(
    compat_kind_ids: &[&str],
) -> Option<&'static LegacyLaneDescriptor> {
    compat_kind_ids
        .iter()
        .find_map(|compat_kind| descriptor_for_compat_kind_id(compat_kind))
}

fn baseline_debt_label(queue_id: Option<&str>) -> &'static str {
    match queue_id {
        Some("panic-baseline") => "panic baseline debt entries",
        _ => "baseline debt entries",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CompatKind, legacy_lane_descriptor};

    #[test]
    fn panic_baseline_descriptor_routes_panic_baseline_queue_metadata() {
        let descriptor = legacy_lane_descriptor(CompatKind::PanicBaseline)
            .unwrap_or_else(|| std::panic::panic_any("panic baseline descriptor missing"));
        let metadata = migration_closeout_baseline_debt(descriptor);

        assert_eq!(metadata.signal, "baseline_debt");
        assert_eq!(metadata.queue_id, Some("panic-baseline"));
        assert_eq!(metadata.label, "panic baseline debt entries");
    }

    #[test]
    fn lint_exception_descriptor_uses_generic_baseline_debt_metadata() {
        let descriptor = legacy_lane_descriptor(CompatKind::LintException)
            .unwrap_or_else(|| std::panic::panic_any("lint exception descriptor missing"));
        let metadata = migration_closeout_baseline_debt(descriptor);

        assert_eq!(metadata.signal, "baseline_debt");
        assert_eq!(metadata.queue_id, None);
        assert_eq!(metadata.label, "baseline debt entries");
        assert_eq!(
            migration_debt_classes(descriptor),
            &[MigrationDebtClass::BaselineDebt]
        );
    }

    #[test]
    fn unsafe_descriptor_classifies_missing_evidence_debt() {
        let descriptor = legacy_lane_descriptor(CompatKind::Unsafe)
            .unwrap_or_else(|| std::panic::panic_any("unsafe descriptor missing"));

        assert_eq!(
            migration_debt_classes(descriptor),
            &[MigrationDebtClass::MissingEvidence]
        );
    }

    #[test]
    fn primary_legacy_descriptor_prefers_first_known_compat_kind() {
        let descriptor = primary_legacy_descriptor(&["unknown", "panic"])
            .unwrap_or_else(|| std::panic::panic_any("panic descriptor lookup missing"));

        assert_eq!(descriptor.lane, CompatKind::PanicBaseline);
    }
}
