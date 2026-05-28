#[derive(Debug, Clone)]
pub(crate) struct LegacyClippyRule {
    pub(crate) id: String,
    pub(crate) path: String,
    pub(crate) lint: String,
    pub(crate) family: String,
    pub(crate) owner: String,
    pub(crate) classification: String,
    pub(crate) reason: String,
    pub(crate) symbol: Option<String>,
    pub(crate) target_fingerprint: Option<String>,
    pub(crate) created: Option<String>,
    pub(crate) review_after: Option<String>,
    pub(crate) expires: Option<String>,
}
