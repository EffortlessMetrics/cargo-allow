//! Reviewed proof command registry and adapter contracts (#2603-B).
//!
//! Absorbed into proof-engine from the standalone `proof-adapter-command` crate
//! (#2937). Defines reviewed command registry entries, structured program/argv
//! invocation specs, dry-run projections, and receipt interpretation.

#[cfg(test)]
mod boundary;
mod command_registry;
mod command_spec;
mod dry_run;
mod parity;
mod receipt_interpretation;

#[cfg(test)]
mod tests;

pub use command_registry::{
    COMMAND_REGISTRY_SCHEMA_ID, CancellationPostureV1, CommandRegistryError, CwdPolicyV1,
    NetworkAccessV1, ReviewedCommandEntryV1, ReviewedCommandRegistryV1,
    default_cargo_allow_registry, validate_command_registry,
};
pub use command_spec::{
    COMMAND_INVOCATION_SPEC_SCHEMA_ID, CommandInvocationSpecV1, CommandSourceKindV1,
    CommandSpecError, compile_invocation_spec, reject_prose_as_executable,
};
pub use dry_run::{
    DRY_RUN_COMMAND_REPORT_SCHEMA_ID, DryRunCommandReportV1, ShellProjectionKindV1,
    render_structured_argv,
};
pub use parity::{
    command_registry_parity_contract_path, command_registry_parity_contract_paths,
    load_command_registry_parity_contract, parity_contract_path, parity_contract_paths,
};
pub use receipt_interpretation::{
    COMMAND_RECEIPT_OUTCOME_SCHEMA_ID, CommandReceiptOutcomeV1, CommandReceiptStatusV1,
    interpret_receipt_binding,
};
