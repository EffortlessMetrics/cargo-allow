/// Completeness facts for one side of a diff.
///
/// `scoped` inventory is valid when the caller intentionally selected a
/// source closure. Fallback and partial inventory are not sufficient to
/// classify movement between revisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffScanCoverage {
    pub inventory_complete: bool,
    pub scanner_complete: bool,
}

impl DiffScanCoverage {
    pub const fn complete() -> Self {
        Self {
            inventory_complete: true,
            scanner_complete: true,
        }
    }

    pub const fn partial() -> Self {
        Self {
            inventory_complete: true,
            scanner_complete: false,
        }
    }

    pub const fn inventory_partial() -> Self {
        Self {
            inventory_complete: false,
            scanner_complete: true,
        }
    }

    pub const fn is_complete(self) -> bool {
        self.inventory_complete && self.scanner_complete
    }
}

/// The bounded result class for a base/head diff evaluation.
///
/// The non-complete classes are deliberately distinct even though they all
/// block an enforcing posture. A caller can therefore explain which side
/// needs repair without treating missing input as scanner evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffResultClass {
    Complete,
    BasePartial,
    HeadPartial,
    BothPartial,
    StaleInput,
    Unsupported,
    InstrumentFailure,
}

impl DiffResultClass {
    pub const fn is_complete(self) -> bool {
        matches!(self, Self::Complete)
    }

    pub const fn is_blocking(self) -> bool {
        !self.is_complete()
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::BasePartial => "base_partial",
            Self::HeadPartial => "head_partial",
            Self::BothPartial => "both_partial",
            Self::StaleInput => "stale_input",
            Self::Unsupported => "unsupported",
            Self::InstrumentFailure => "instrument_failure",
        }
    }
}

/// Classify the two independently evaluated sides without collapsing their
/// completeness into one boolean.
pub const fn classify_diff_result(
    base: DiffScanCoverage,
    head: DiffScanCoverage,
) -> DiffResultClass {
    match (base.is_complete(), head.is_complete()) {
        (true, true) => DiffResultClass::Complete,
        (false, true) => DiffResultClass::BasePartial,
        (true, false) => DiffResultClass::HeadPartial,
        (false, false) => DiffResultClass::BothPartial,
    }
}

/// Keep only movement that is safe to report when scanner coverage is partial.
/// A removed finding may be absent because the partial side omitted its file,
/// so it must never become an improvement or resolved count.
pub fn retain_confident_finding_changes(
    result: DiffResultClass,
    changes: Vec<crate::FindingPostureChange>,
) -> Vec<crate::FindingPostureChange> {
    if result.is_complete() {
        return changes;
    }
    changes
        .into_iter()
        .filter(|change| !matches!(change.kind, crate::FindingPostureKind::Removed))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_each_base_head_partial_combination() {
        let complete = DiffScanCoverage::complete();
        let partial = DiffScanCoverage::partial();

        assert_eq!(
            classify_diff_result(complete, complete),
            DiffResultClass::Complete
        );
        assert_eq!(
            classify_diff_result(partial, complete),
            DiffResultClass::BasePartial
        );
        assert_eq!(
            classify_diff_result(complete, partial),
            DiffResultClass::HeadPartial
        );
        assert_eq!(
            classify_diff_result(partial, partial),
            DiffResultClass::BothPartial
        );
    }

    #[test]
    fn every_non_complete_class_is_blocking() {
        for class in [
            DiffResultClass::BasePartial,
            DiffResultClass::HeadPartial,
            DiffResultClass::BothPartial,
            DiffResultClass::StaleInput,
            DiffResultClass::Unsupported,
            DiffResultClass::InstrumentFailure,
        ] {
            assert!(class.is_blocking());
            assert!(!class.is_complete());
        }
    }

    #[test]
    fn partial_results_never_report_removed_findings_as_improvements() {
        let changes = vec![
            crate::FindingPostureChange {
                kind: crate::FindingPostureKind::New,
                key: "new".to_string(),
                finding_kind: "panic".to_string(),
                family: None,
                path: "src/new.rs".to_string(),
                line: None,
                column: None,
                source_package: None,
                identity: allow_core::StructuralIdentity::new("rust", "call"),
            },
            crate::FindingPostureChange {
                kind: crate::FindingPostureKind::Removed,
                key: "removed".to_string(),
                finding_kind: "panic".to_string(),
                family: None,
                path: "src/removed.rs".to_string(),
                line: None,
                column: None,
                source_package: None,
                identity: allow_core::StructuralIdentity::new("rust", "call"),
            },
        ];

        let retained = retain_confident_finding_changes(DiffResultClass::HeadPartial, changes);
        assert_eq!(retained.len(), 1);
        assert_eq!(retained[0].kind, crate::FindingPostureKind::New);
    }

    #[test]
    fn exposes_coverage_and_result_labels_for_each_contract_class() {
        assert!(!DiffScanCoverage::inventory_partial().is_complete());
        assert!(retain_confident_finding_changes(DiffResultClass::Complete, Vec::new()).is_empty());

        let labels = [
            (DiffResultClass::Complete, "complete"),
            (DiffResultClass::BasePartial, "base_partial"),
            (DiffResultClass::HeadPartial, "head_partial"),
            (DiffResultClass::BothPartial, "both_partial"),
            (DiffResultClass::StaleInput, "stale_input"),
            (DiffResultClass::Unsupported, "unsupported"),
            (DiffResultClass::InstrumentFailure, "instrument_failure"),
        ];
        for (class, expected) in labels {
            assert_eq!(class.as_str(), expected);
        }
    }
}
