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
