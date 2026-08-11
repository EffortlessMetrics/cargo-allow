//! Property tests for diff result classification (#1905).
//!
//! Verifies the classify_diff_result function is consistent with the
//! per-side completeness booleans for all possible combinations.

use crate::{DiffScanCoverage, classify_diff_result};
use proptest::prelude::*;

proptest! {
    /// For any combination of base/head completeness, the classified result
    /// must match the exhaustive truth table: both complete -> Complete,
    /// base-only partial -> BasePartial, head-only partial -> HeadPartial,
    /// both partial -> BothPartial.
    #[test]
    fn prop_classify_diff_result_matches_truth_table(
        base_inv in any::<bool>(),
        base_scan in any::<bool>(),
        head_inv in any::<bool>(),
        head_scan in any::<bool>(),
    ) {
        let base = DiffScanCoverage {
            inventory_complete: base_inv,
            scanner_complete: base_scan,
        };
        let head = DiffScanCoverage {
            inventory_complete: head_inv,
            scanner_complete: head_scan,
        };
        let result = classify_diff_result(base, head);
        // classify_diff_result only considers is_complete() (both flags true).
        let base_ok = base.is_complete();
        let head_ok = head.is_complete();
        match (base_ok, head_ok) {
            (true, true) => prop_assert!(result.is_complete()),
            (false, true) => prop_assert!(matches!(result, crate::DiffResultClass::BasePartial)),
            (true, false) => prop_assert!(matches!(result, crate::DiffResultClass::HeadPartial)),
            (false, false) => prop_assert!(matches!(result, crate::DiffResultClass::BothPartial)),
        }
    }

    /// DiffScanCoverage::complete() must always be complete regardless of
    /// how it was constructed.
    #[test]
    fn prop_complete_coverage_is_always_complete(_ in any::<u8>()) {
        let c = DiffScanCoverage::complete();
        prop_assert!(c.is_complete());
        prop_assert!(c.inventory_complete);
        prop_assert!(c.scanner_complete);
    }

    /// DiffScanCoverage::partial() must never be complete (scanner is false).
    #[test]
    fn prop_partial_coverage_is_never_complete(_ in any::<u8>()) {
        let c = DiffScanCoverage::partial();
        prop_assert!(!c.is_complete());
        prop_assert!(!c.scanner_complete);
    }
}
