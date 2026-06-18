use allow_core::LastSeen;

#[derive(Debug, Clone)]
pub(crate) struct LegacyNoPanicBaselineEntry {
    pub(crate) index: usize,
    pub(crate) path: String,
    pub(crate) family: String,
    pub(crate) selector_kind: String,
    pub(crate) selector_callee: String,
    pub(crate) snippet: String,
    pub(crate) count: u32,
    pub(crate) owner: String,
    pub(crate) reason: String,
    pub(crate) evidence: Vec<String>,
    pub(crate) created: Option<String>,
    pub(crate) review_after: Option<String>,
    pub(crate) expires: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct LegacyNoPanicAllowEntry {
    pub(crate) index: usize,
    pub(crate) id: String,
    pub(crate) path: String,
    pub(crate) family: String,
    pub(crate) selector_kind: String,
    pub(crate) selector_callee: Option<String>,
    pub(crate) selector_container: Option<String>,
    pub(crate) owner: String,
    pub(crate) classification: String,
    pub(crate) reason: String,
    pub(crate) evidence: Vec<String>,
    pub(crate) created: Option<String>,
    pub(crate) review_after: Option<String>,
    pub(crate) expires: Option<String>,
    pub(crate) line_hint: Option<u32>,
    pub(crate) last_seen: Option<LastSeen>,
    pub(crate) selector_semantics: crate::semantic_selector_fields::LegacySemanticSelectorExtras,
}
