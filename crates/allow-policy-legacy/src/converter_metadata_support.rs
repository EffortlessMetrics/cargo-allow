//! Shared helpers for preserving legacy metadata during migration.

use allow_core::Lifecycle;

use crate::converter_lifecycle_support::lifecycle_from_legacy_fields;
use crate::default_baseline_expires;

pub(crate) struct LegacyEntryMetadata<'a> {
    pub owner: &'a str,
    pub reason: &'a str,
    pub classification: &'a str,
}

pub(crate) fn preserve_metadata(metadata: LegacyEntryMetadata<'_>) -> (String, String, String) {
    (
        metadata.owner.to_string(),
        metadata.reason.to_string(),
        metadata.classification.to_string(),
    )
}

pub(crate) fn preserve_evidence(evidence: &[String], legacy_policy_key: &str) -> Vec<String> {
    if evidence.is_empty() {
        vec![legacy_policy_link(legacy_policy_key)]
    } else {
        evidence.to_vec()
    }
}

pub(crate) fn preserve_evidence_with_fallback(
    evidence: &[String],
    fallback: &[String],
) -> Vec<String> {
    if evidence.is_empty() {
        fallback.to_vec()
    } else {
        evidence.to_vec()
    }
}

pub(crate) fn extend_evidence_with_legacy_policy(
    evidence: &[String],
    legacy_policy_key: &str,
) -> Vec<String> {
    let mut extended = evidence.to_vec();
    extended.push(legacy_policy_link(legacy_policy_key));
    extended
}

pub(crate) fn extend_evidence_with_markers(
    evidence: &[String],
    legacy_policy_key: &str,
    markers: &[String],
) -> Vec<String> {
    let mut extended = extend_evidence_with_legacy_policy(evidence, legacy_policy_key);
    extended.extend(markers.iter().cloned());
    extended
}

pub(crate) fn legacy_policy_link(legacy_policy_key: &str) -> String {
    format!("legacy-policy:{legacy_policy_key}")
}

pub(crate) fn legacy_policy_links(legacy_policy_key: &str) -> Vec<String> {
    vec![legacy_policy_link(legacy_policy_key)]
}

pub(crate) fn map_lifecycle(
    created: Option<String>,
    review_after: Option<String>,
    expires: Option<String>,
) -> Lifecycle {
    lifecycle_from_legacy_fields(created, review_after, expires)
}

pub(crate) fn map_baseline_debt_expires(expires: Option<String>) -> Option<String> {
    match expires.as_deref() {
        Some("never") => Some(default_baseline_expires()),
        None => None,
        Some(value) => Some(value.to_string()),
    }
}

pub(crate) fn map_baseline_debt_lifecycle(
    created: Option<String>,
    review_after: Option<String>,
    expires: Option<String>,
) -> Lifecycle {
    map_lifecycle(created, review_after, map_baseline_debt_expires(expires))
}

pub(crate) fn map_occurrence_limit(count: u32) -> Option<u32> {
    Some(count)
}

pub(crate) fn map_occurrence_limit_none() -> Option<u32> {
    None
}

pub(crate) fn classify_baseline_debt() -> String {
    "baseline_debt".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserve_metadata_copies_owner_reason_and_classification() {
        let (owner, reason, classification) = preserve_metadata(LegacyEntryMetadata {
            owner: "lint",
            reason: "reviewed suppression",
            classification: "reviewed_lint_exception",
        });

        assert_eq!(owner, "lint");
        assert_eq!(reason, "reviewed suppression");
        assert_eq!(classification, "reviewed_lint_exception");
    }

    #[test]
    fn preserve_evidence_uses_legacy_policy_fallback_when_empty() {
        assert_eq!(
            preserve_evidence(&[], "fixture-clippy"),
            vec!["legacy-policy:fixture-clippy".to_string()]
        );
        assert_eq!(
            preserve_evidence(&["test:case".to_string()], "fixture-clippy"),
            vec!["test:case".to_string()]
        );
    }

    #[test]
    fn preserve_evidence_with_fallback_preserves_custom_debt_markers() {
        assert_eq!(
            preserve_evidence_with_fallback(
                &[],
                &[
                    "legacy_policy:no-panic-baseline".to_string(),
                    "baseline_count:3".to_string(),
                ],
            ),
            vec![
                "legacy_policy:no-panic-baseline".to_string(),
                "baseline_count:3".to_string(),
            ]
        );
    }

    #[test]
    fn extend_evidence_with_markers_appends_legacy_policy_and_surface_markers() {
        assert_eq!(
            extend_evidence_with_markers(
                &["test:dependency_surface".to_string()],
                "dependency-workspace",
                &[
                    "surface:workspace_manifest".to_string(),
                    "dep_count_at_baseline:42".to_string(),
                ],
            ),
            vec![
                "test:dependency_surface".to_string(),
                "legacy-policy:dependency-workspace".to_string(),
                "surface:workspace_manifest".to_string(),
                "dep_count_at_baseline:42".to_string(),
            ]
        );
    }

    #[test]
    fn map_lifecycle_applies_never_review_after_fallback() {
        let lifecycle = map_lifecycle(
            Some("2026-05-09".to_string()),
            None,
            Some("never".to_string()),
        );

        assert_eq!(lifecycle.created.as_deref(), Some("2026-05-09"));
        assert_eq!(lifecycle.review_after.as_deref(), Some("2026-05-09"));
        assert_eq!(lifecycle.expires.as_deref(), Some("never"));
    }

    #[test]
    fn map_baseline_debt_lifecycle_rewrites_never_expires_to_default_window() {
        let lifecycle = map_baseline_debt_lifecycle(
            Some("2026-05-09".to_string()),
            Some("2026-06-09".to_string()),
            Some("never".to_string()),
        );

        assert_eq!(lifecycle.created.as_deref(), Some("2026-05-09"));
        assert_eq!(lifecycle.review_after.as_deref(), Some("2026-06-09"));
        assert_eq!(
            lifecycle.expires.as_deref(),
            Some(crate::default_baseline_expires().as_str())
        );
    }

    #[test]
    fn map_occurrence_limit_helpers_cover_counted_and_uncounted_lanes() {
        assert_eq!(map_occurrence_limit(3), Some(3));
        assert_eq!(map_occurrence_limit_none(), None);
    }

    #[test]
    fn classify_baseline_debt_returns_baseline_debt_marker() {
        assert_eq!(classify_baseline_debt(), "baseline_debt");
    }
}
