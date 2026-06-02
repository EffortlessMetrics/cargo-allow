#[derive(Debug, Clone)]
pub(crate) struct LegacyExecutableRule {
    pub(crate) id: String,
    pub(crate) path: String,
    pub(crate) owner: String,
    pub(crate) reason: String,
    pub(crate) interpreter: Option<String>,
    pub(crate) evidence: Vec<String>,
    pub(crate) created: Option<String>,
    pub(crate) review_after: Option<String>,
    pub(crate) expires: Option<String>,
}
