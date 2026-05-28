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
