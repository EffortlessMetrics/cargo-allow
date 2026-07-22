use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimBoundaryV1 {
    pub statement: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<String>,
}

impl ClaimBoundaryV1 {
    pub fn new(statement: impl Into<String>) -> Self {
        Self {
            statement: statement.into(),
            limitations: Vec::new(),
        }
    }

    pub fn with_limitations(mut self, limitations: impl IntoIterator<Item = String>) -> Self {
        self.limitations = limitations.into_iter().collect();
        self
    }
}
