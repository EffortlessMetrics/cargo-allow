use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceIdentityV1 {
    pub path: String,
    pub present: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blob_oid: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceAnchorV1 {
    pub schema_id: String,
    pub repository_root_identity: String,
    pub source: SourceIdentityV1,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<String>,
}

impl SourceAnchorV1 {
    pub fn from_selected_path(repository_root_identity: &str, source: SourceIdentityV1) -> Self {
        Self {
            schema_id: "repo.source-anchor.v1".to_string(),
            repository_root_identity: repository_root_identity.to_string(),
            source,
            limitations: Vec::new(),
        }
    }
}
