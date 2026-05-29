use allow_core::Lifecycle;

use crate::default_baseline_created;

pub(crate) fn lifecycle_from_legacy_fields(
    created: Option<String>,
    review_after: Option<String>,
    expires: Option<String>,
) -> Lifecycle {
    let review_after = review_after.or_else(|| {
        (expires.as_deref() == Some("never"))
            .then(|| created.clone().unwrap_or_else(default_baseline_created))
    });
    Lifecycle {
        created,
        review_after,
        expires,
    }
}
