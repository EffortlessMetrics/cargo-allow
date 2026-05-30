#[path = "saved_artifact_outputs/core.rs"]
mod core;
#[path = "saved_artifact_outputs/doctor.rs"]
mod doctor;
#[path = "saved_artifact_outputs/explain.rs"]
mod explain;
#[path = "saved_artifact_outputs/fixture.rs"]
mod fixture;
#[path = "saved_artifact_outputs/list.rs"]
mod list;
#[path = "saved_artifact_outputs/propose.rs"]
mod propose;
#[path = "saved_artifact_outputs/prune.rs"]
mod prune;
mod support;
#[path = "saved_artifact_outputs/worklist.rs"]
mod worklist;

use fixture::{
    SourceTreeFixture, assert_policy_migration_artifact, assert_policy_output,
    assert_source_syntax_artifact, assert_source_syntax_artifact_with_inventory,
    commit_fixture_base, path_arg, run_cargo_allow,
};
