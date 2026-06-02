#[derive(Debug, Clone)]
pub(crate) struct LegacyWorkflowRule {
    pub(crate) path: String,
    pub(crate) owner: String,
    pub(crate) reason: String,
    pub(crate) permissions: Vec<String>,
    pub(crate) secrets_used: Vec<String>,
    pub(crate) external_actions: Vec<String>,
    pub(crate) duplicate_of_lane: Option<String>,
    pub(crate) evidence: Vec<String>,
    pub(crate) created: Option<String>,
    pub(crate) review_after: Option<String>,
    pub(crate) expires: Option<String>,
}
