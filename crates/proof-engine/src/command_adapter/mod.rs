//! Reviewed proof command registry and adapter contracts (#2603-B).
//!
//! Absorbed into proof-engine from the standalone `proof-adapter-command` crate
//! (#2937). Defines reviewed command registry entries, structured program/argv
//! invocation specs, dry-run projections, and receipt interpretation.

mod boundary;
mod command_registry;
mod command_registry_surface;
mod command_spec;
mod command_spec_surface;
mod dry_run;
mod dry_run_surface;
mod parity;
mod receipt_interpretation;
mod receipt_interpretation_surface;

#[cfg(test)]
mod tests;

pub use boundary::{
    ALLOWED_UPSTREAM_CRATES, BoundarySurface, FORBIDDEN_DEPENDENCY_EDGES, upstream_surface_markers,
};
pub use command_registry::{
    COMMAND_REGISTRY_SCHEMA_ID, CancellationPostureV1, CommandRegistryError, CwdPolicyV1,
    NetworkAccessV1, ReviewedCommandEntryV1, ReviewedCommandRegistryV1,
    default_cargo_allow_registry, validate_command_registry,
};
pub use command_registry_surface::CommandRegistrySurface;
pub use command_spec::{
    COMMAND_INVOCATION_SPEC_SCHEMA_ID, CommandInvocationSpecV1, CommandSourceKindV1,
    CommandSpecError, compile_invocation_spec, reject_prose_as_executable,
};
pub use command_spec_surface::CommandSpecSurface;
pub use dry_run::{
    DRY_RUN_COMMAND_REPORT_SCHEMA_ID, DryRunCommandReportV1, ShellProjectionKindV1,
    render_structured_argv,
};
pub use dry_run_surface::DryRunSurface;
pub use parity::{
    command_registry_parity_contract_path, command_registry_parity_contract_paths,
    load_command_registry_parity_contract, parity_contract_path, parity_contract_paths,
};
pub use receipt_interpretation::{
    COMMAND_RECEIPT_OUTCOME_SCHEMA_ID, CommandReceiptOutcomeV1, CommandReceiptStatusV1,
    interpret_receipt_binding,
};
pub use receipt_interpretation_surface::ReceiptInterpretationSurface;
