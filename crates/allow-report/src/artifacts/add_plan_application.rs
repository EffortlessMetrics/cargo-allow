use crate::InventoryContext;

/// Structured payload for the `cargo-allow.add-plan-application.v1` receipt.
///
/// Emitted by `add --from-plan` after a versioned add-finding plan has been
/// verified against a fresh source-tree scan and atomically applied to the live
/// ledger. Every field is an identity binding tying the applied mutation back to
/// the exact plan it consumed and the exact policy states it moved between, so a
/// reviewer can confirm what changed without trusting the operator's narration.
#[derive(Debug, Clone)]
pub struct AddPlanApplicationV1<'a> {
    pub tool_version: String,
    /// Source-tree inventory context of the scan that verified the plan.
    pub inventory: InventoryContext<'a>,
    /// SHA-256 of the exact plan bytes consumed (`sha256:v1:...`).
    pub plan_digest: String,
    /// Recomputed repository identity that matched the plan.
    pub repository_identity: String,
    /// Recomputed finding identity digest that matched the plan.
    pub finding_digest: String,
    /// Discovered ledger path the mutation replaced.
    pub target_ledger: String,
    /// SHA-256 of the policy file before the atomic replace.
    pub policy_before_digest: String,
    /// SHA-256 of the policy file after the atomic replace.
    pub policy_after_digest: String,
    /// Allow ID added to the ledger.
    pub added_allow_id: String,
    /// Targeted recheck result. After writing the entry, `add --from-plan`
    /// re-evaluates the target finding against the mutated policy (reusing the
    /// loaded findings) and reports whether the finding now matches:
    /// `matched`, `still_new`, `no_outcome`, or `unexpected:<status>`.
    /// This is NOT a full check — the operator must still run `full_check_argv`
    /// for CI-grade proof.
    pub targeted_recheck: String,
    /// Authoritative program plus ordered argv for the full-repository check the
    /// operator should run next. Consumers must not shell-split or reconstruct
    /// these arguments from human text.
    pub full_check_argv: Vec<String>,
}
