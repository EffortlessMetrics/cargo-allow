pub(crate) use crate::types_executable_entries::LegacyExecutableRule;
pub(crate) use crate::types_lint_entries::LegacyClippyRule;
pub(crate) use crate::types_panic_entries::{LegacyNoPanicAllowEntry, LegacyNoPanicBaselineEntry};
pub(crate) use crate::types_source_entries::{LegacyGeneratedRule, LegacyNonRustRule};
pub(crate) use crate::types_unsafe_entries::LegacyUnsafeRule;
pub(crate) use crate::types_workflow_entries::LegacyWorkflowRule;

#[derive(Debug, Clone)]
pub(crate) struct LegacyDependencySurfaceRule {
    pub(crate) id: String,
    pub(crate) pattern: String,
    pub(crate) is_glob: bool,
    pub(crate) surface: String,
    pub(crate) owner: String,
    pub(crate) reason: String,
    pub(crate) broad_glob_reason: Option<String>,
    pub(crate) dep_count_at_baseline: Option<i64>,
    pub(crate) created: Option<String>,
    pub(crate) review_after: Option<String>,
    pub(crate) expires: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct LegacyProcessRule {
    pub(crate) id: String,
    pub(crate) binary: String,
    pub(crate) argv_shape: Vec<String>,
    pub(crate) network_reach: bool,
    pub(crate) called_by: Vec<String>,
    pub(crate) owner: String,
    pub(crate) reason: String,
    pub(crate) created: Option<String>,
    pub(crate) review_after: Option<String>,
    pub(crate) expires: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct LegacyNetworkRule {
    pub(crate) id: String,
    pub(crate) destination: String,
    pub(crate) auth_required: bool,
    pub(crate) auth_secret: Option<String>,
    pub(crate) lane: String,
    pub(crate) owner: String,
    pub(crate) reason: String,
    pub(crate) created: Option<String>,
    pub(crate) review_after: Option<String>,
    pub(crate) expires: Option<String>,
}
