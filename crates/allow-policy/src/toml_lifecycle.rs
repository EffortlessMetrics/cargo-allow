use allow_core::Lifecycle;
use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LifecycleToml {
    created: Option<String>,
    review_after: Option<String>,
    expires: Option<String>,
}

impl LifecycleToml {
    pub(crate) fn into_lifecycle(self) -> Lifecycle {
        Lifecycle {
            created: self.created,
            review_after: self.review_after,
            expires: self.expires,
        }
    }
}
