//! Module surface markers for intent-protocol parity (#2585).

/// Identity/query envelope surface (#2585-A).
pub struct IdentityQuerySurface;

impl IdentityQuerySurface {
    pub const MODULE_ID: &'static str = "intent-protocol::identity_query";
}

/// View/diff/closure envelope surface (#2585-B).
pub struct ViewDiffClosureSurface;

impl ViewDiffClosureSurface {
    pub const MODULE_ID: &'static str = "intent-protocol::view_diff_closure";
}

/// Obligation-plan envelope surface (#2585-C).
pub struct ObligationPlanSurface;

impl ObligationPlanSurface {
    pub const MODULE_ID: &'static str = "intent-protocol::obligation_plan";
}
