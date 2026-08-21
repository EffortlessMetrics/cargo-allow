mod authority_compiler;
mod command_dispatch;
mod composition;
mod paired_precommit;
mod source_views;

pub use authority_compiler::{
    AUTHORITY_COMPILE_PLAN_SCHEMA_ID, AuthorityCompilePlanV1, AuthoritySourceRoleV1,
    AuthoritySourceV1, composition_sources_present, plan_authority_compile,
};
pub use command_dispatch::{
    SPEC_SYSTEM_COMMAND_DISPATCH_SCHEMA_ID, SPEC_SYSTEM_COMMANDS, SpecSystemCommandV1,
    embedded_authority_surface, spec_system_command,
};
pub use composition::{
    SELF_HOSTED_RUNTIME_PROMOTION_COMPOSITION_ID, WorkspaceCompositionV1,
    load_workspace_composition_toml,
};
pub use paired_precommit::{
    GraphDiagnosticV1, evaluate_paired_precommit_objectives_v1, graph_movement_kind_to_precommit,
    subject_resolution_from_diagnostic,
};
pub use source_views::{
    WorkspaceCompositionSources, composition_sources_present_in_view,
    read_workspace_composition_sources,
};
