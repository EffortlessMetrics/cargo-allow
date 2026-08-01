use allow_core::{Finding, MatchOutcome};

use crate::InventoryContext;

/// Scope evidence carried by `why` and its optional add-finding plan.
#[derive(Debug, Clone, Copy)]
pub struct EvaluationContext<'a> {
    pub scope: &'a str,
    pub locality: &'a str,
    pub reasons: &'a [String],
}

impl EvaluationContext<'_> {
    /// Derive the stable result class without changing the public struct
    /// shape used by downstream Rust consumers.
    pub fn result_class(self, inventory: InventoryContext<'_>) -> Option<&'static str> {
        self.result_class_with_scanner_completeness(inventory, None)
    }

    /// Derive the result class when the caller has independent evidence about
    /// the scanner used for the evaluation. A scoped `why` run inventories the
    /// repository but scans only the target file, so repository inventory
    /// partiality must not be confused with a target-scanner omission.
    pub fn result_class_with_scanner_completeness(
        self,
        inventory: InventoryContext<'_>,
        scanner_completeness: Option<&str>,
    ) -> Option<&'static str> {
        let scanner_completeness = scanner_completeness.or(inventory.completeness);
        match inventory.completeness {
            Some("complete" | "scoped" | "partial")
                if (self.scope, self.locality) == ("scoped", "proven") =>
            {
                match scanner_completeness {
                    Some("complete" | "scoped") => Some("exact_scoped"),
                    Some("partial") => Some("target_scanner_partial"),
                    _ => None,
                }
            }
            Some("complete" | "scoped")
                if (self.scope, self.locality) == ("full_fallback", "global_dependency") =>
            {
                match scanner_completeness {
                    Some("complete" | "scoped") => Some("exact_after_full_fallback"),
                    Some("partial") => Some("full_fallback_unavailable"),
                    _ => None,
                }
            }
            Some("partial" | "fallback")
                if (self.scope, self.locality) == ("full_fallback", "global_dependency") =>
            {
                Some("full_fallback_unavailable")
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct WhyCandidateEntry<'a> {
    pub id: &'a str,
    pub kind: &'a str,
    pub family: Option<&'a str>,
    pub path: Option<&'a str>,
    pub glob: Option<&'a str>,
    pub selector_glob: Option<&'a str>,
    pub mismatch_reasons: &'a [String],
}

/// Structured proof-plan argv for machine consumers of `cargo-allow.why.v1`.
#[derive(Debug, Clone, Copy)]
pub struct WhyProofPlan<'a> {
    pub program: &'a str,
    pub args: &'a [String],
}

#[derive(Debug, Clone, Copy)]
pub struct WhyReport<'a> {
    pub inventory: InventoryContext<'a>,
    pub evaluation: EvaluationContext<'a>,
    pub finding: &'a Finding,
    pub outcome: &'a MatchOutcome,
    pub candidate_entries: &'a [WhyCandidateEntry<'a>],
    pub suggested_actions: &'a [String],
    pub proof_commands: &'a [String],
    pub proof_plans: &'a [WhyProofPlan<'a>],
}
