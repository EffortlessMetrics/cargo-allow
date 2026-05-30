#[path = "saved_artifact_outputs/add.rs"]
mod add;
#[path = "saved_artifact_outputs/check.rs"]
mod check;
#[path = "saved_artifact_outputs/core.rs"]
mod core;
#[path = "saved_artifact_outputs/diff.rs"]
mod diff;
#[path = "saved_artifact_outputs/doctor.rs"]
mod doctor;
#[path = "saved_artifact_outputs/explain.rs"]
mod explain;
#[path = "saved_artifact_outputs/fixture.rs"]
mod fixture;
#[path = "saved_artifact_outputs/harness.rs"]
mod harness;
#[path = "saved_artifact_outputs/list.rs"]
mod list;
#[path = "saved_artifact_outputs/propose.rs"]
mod propose;
#[path = "saved_artifact_outputs/prune.rs"]
mod prune;
mod support;
#[path = "saved_artifact_outputs/worklist.rs"]
mod worklist;

use fixture::{SourceTreeFixture, commit_fixture_base};
use harness::{
    assert_policy_migration_artifact, assert_policy_output, assert_source_syntax_artifact,
    assert_source_syntax_artifact_with_inventory, path_arg, run_cargo_allow,
};
