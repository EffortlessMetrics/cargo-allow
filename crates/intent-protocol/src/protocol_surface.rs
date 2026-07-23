//! Module surface marker for intent-protocol parity (#2585-A).

/// Identity/query envelope surface (#2585-A).
pub struct IdentityQuerySurface;

impl IdentityQuerySurface {
    pub const MODULE_ID: &'static str = "intent-protocol::identity_query";
}
