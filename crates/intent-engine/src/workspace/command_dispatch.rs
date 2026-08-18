//! Canonical spec-system command dispatch contract (#3523 slice D step ii).
//!
//! The spec-system CLI surface that cargo-allow dispatches today is a
//! contract cargo-intent must reproduce at the promotion cutover: the
//! command vocabulary, the embedded-authority rejection surface each
//! command runs under, and which commands expose a `--mode` override.
//! This table is the canonical statement of that contract; cargo-allow
//! keeps its local dispatch (the dependency law forbids a production
//! dependency in either direction) and the dev-scope parity tests bind
//! the two together.
//!
//! Under the delegation switch (`delegate_spec_system` in
//! `.allow/compatibility/intent-delegation.toml`), every dispatched
//! surface in this table rejects cargo-allow's embedded authority and
//! names cargo-intent as the owner.

pub const SPEC_SYSTEM_COMMAND_DISPATCH_SCHEMA_ID: &str = "intent.spec-system-command-dispatch.v1";

/// One dispatched spec-system command surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpecSystemCommandV1 {
    /// The report command string embedded in rendered reports.
    pub command: &'static str,
    /// The embedded-authority rejection surface for the command.
    pub surface: &'static str,
    /// Whether the command accepts an explicit `--mode` override.
    /// `check` does; `audit` is report-only and deliberately does not.
    pub exposes_mode_override: bool,
}

/// The complete dispatched spec-system command vocabulary. Every entry's
/// surface rejects embedded authority under the delegation switch.
pub const SPEC_SYSTEM_COMMANDS: [SpecSystemCommandV1; 6] = [
    SpecSystemCommandV1 {
        command: "check",
        surface: "check",
        exposes_mode_override: true,
    },
    SpecSystemCommandV1 {
        command: "audit",
        surface: "audit",
        exposes_mode_override: false,
    },
    SpecSystemCommandV1 {
        command: "doctor",
        surface: "doctor",
        exposes_mode_override: false,
    },
    SpecSystemCommandV1 {
        command: "explain",
        surface: "explain",
        exposes_mode_override: false,
    },
    SpecSystemCommandV1 {
        command: "init",
        surface: "init",
        exposes_mode_override: false,
    },
    SpecSystemCommandV1 {
        command: "worklist",
        surface: "worklist",
        exposes_mode_override: false,
    },
];

/// Resolve a command in the dispatched vocabulary, failing closed on
/// unknown commands.
pub fn spec_system_command(command: &str) -> Option<&'static SpecSystemCommandV1> {
    SPEC_SYSTEM_COMMANDS
        .iter()
        .find(|entry| entry.command == command)
}

/// Whether a surface is a dispatched spec-system command surface. Every
/// known surface rejects cargo-allow's embedded authority under the
/// delegation switch; unknown surfaces are not dispatched surfaces.
pub fn embedded_authority_surface(surface: &str) -> bool {
    SPEC_SYSTEM_COMMANDS
        .iter()
        .any(|entry| entry.surface == surface)
}
