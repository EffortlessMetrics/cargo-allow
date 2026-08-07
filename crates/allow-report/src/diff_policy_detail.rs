use crate::DiffPolicyChange;

pub(crate) fn policy_change_detail(change: &DiffPolicyChange<'_>) -> Option<String> {
    let mut details = Vec::new();

    if let Some(identity) = change.exception_identity {
        details.push(format!(
            "exception_identity.{}: {} -> {}",
            identity.field,
            option_text(identity.before),
            option_text(identity.after)
        ));
    }

    if let Some(selector_identity) = change.selector_identity {
        details.push(format!(
            "selector_identity changed: {}",
            list_text(selector_identity.changed_fields)
        ));
    }

    if let Some(selector_precision) = change.selector_precision {
        details.push(format!(
            "selector_precision: {} -> {}; removed: {}; added: {}",
            selector_precision.before,
            selector_precision.after,
            list_text(selector_precision.removed_fields),
            list_text(selector_precision.added_fields)
        ));
    }

    if let Some(scope) = change.scope {
        details.push(format!(
            "scope.{}: {} -> {}",
            scope.field,
            option_text(scope.before),
            option_text(scope.after)
        ));
    }

    if let Some(limit) = change.occurrence_limit {
        details.push(format!(
            "occurrence_limit: {} -> {}",
            option_u32_text(limit.before),
            option_u32_text(limit.after)
        ));
    }

    if let Some(lifecycle) = change.lifecycle {
        details.push(format!(
            "lifecycle.{}: {} -> {}",
            lifecycle.field,
            option_text(lifecycle.before),
            option_text(lifecycle.after)
        ));
    }

    if let Some(evidence) = change.evidence {
        details.push(format!(
            "evidence.{}: removed: {}; added: {}",
            evidence.field,
            owned_list_text(evidence.removed),
            owned_list_text(evidence.added)
        ));
    }

    if let Some(metadata) = change.metadata {
        details.push(format!(
            "metadata.{}: {} -> {}",
            metadata.field,
            option_text(metadata.before),
            option_text(metadata.after)
        ));
    }

    if let Some(requirement) = change.requirement {
        details.push(format!(
            "requirement.{}: {} -> {}",
            requirement.field, requirement.before, requirement.after
        ));
    }

    if let Some(policy_status) = change.policy_status {
        details.push(format!(
            "policy_status: {} -> {}",
            option_text(policy_status.before),
            option_text(policy_status.after)
        ));
    }

    (!details.is_empty()).then(|| details.join("; "))
}

fn option_text(value: Option<&str>) -> &str {
    value.unwrap_or("none")
}

fn option_u32_text(value: Option<u32>) -> String {
    value.map_or_else(|| "none".to_string(), |value| value.to_string())
}

fn list_text(values: &[&str]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(", ")
    }
}

fn owned_list_text(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DiffEvidenceChange, DiffExceptionIdentityChange, DiffLifecycleChange, DiffMetadataChange,
        DiffOccurrenceLimitChange, DiffPolicyStatusChange, DiffRequirementChange, DiffScopeChange,
        DiffSelectorIdentityChange, DiffSelectorPrecisionChange,
    };

    fn base_change() -> DiffPolicyChange<'static> {
        DiffPolicyChange {
            severity: "review",
            movement: "retained",
            posture_delta: "review_required",
            changed_in_diff: true,
            subject: None,
            ledger_id: None,
            lane: None,
            allow_id: "allow-0001",
            kind: "policy_changed",
            message: "allow-0001 policy changed",
            exception_identity: None,
            selector_identity: None,
            selector_precision: None,
            scope: None,
            occurrence_limit: None,
            lifecycle: None,
            evidence: None,
            metadata: None,
            requirement: None,
            policy_status: None,
        }
    }

    #[test]
    fn policy_change_detail_returns_none_without_detail_payloads() {
        assert_eq!(policy_change_detail(&base_change()), None);
    }

    #[test]
    fn policy_change_detail_formats_all_detail_payloads_in_order() {
        let changed_fields = ["container", "normalized_snippet_hash"];
        let removed_fields = ["container"];
        let added_fields = ["normalized_snippet_hash"];
        let removed_evidence = vec!["test:old-proof".to_string()];
        let added_evidence = vec!["test:new-proof".to_string()];
        let change = DiffPolicyChange {
            exception_identity: Some(DiffExceptionIdentityChange {
                field: "kind",
                before: Some("panic"),
                after: Some("unsafe"),
            }),
            selector_identity: Some(DiffSelectorIdentityChange {
                changed_fields: &changed_fields,
            }),
            selector_precision: Some(DiffSelectorPrecisionChange {
                before: 82,
                after: 41,
                removed_fields: &removed_fields,
                added_fields: &added_fields,
            }),
            scope: Some(DiffScopeChange {
                field: "path",
                before: Some("src/lib.rs"),
                after: None,
            }),
            occurrence_limit: Some(DiffOccurrenceLimitChange {
                before: Some(1),
                after: None,
            }),
            lifecycle: Some(DiffLifecycleChange {
                field: "expires",
                before: None,
                after: Some("2026-12-01"),
            }),
            evidence: Some(DiffEvidenceChange {
                field: "evidence",
                removed: &removed_evidence,
                added: &added_evidence,
            }),
            metadata: Some(DiffMetadataChange {
                field: "owner",
                before: Some("core"),
                after: None,
            }),
            requirement: Some(DiffRequirementChange {
                field: "owner_required",
                before: true,
                after: false,
            }),
            policy_status: Some(DiffPolicyStatusChange {
                before: Some("active"),
                after: Some("advisory"),
            }),
            ..base_change()
        };

        assert_eq!(
            policy_change_detail(&change),
            Some(
                "exception_identity.kind: panic -> unsafe; \
                 selector_identity changed: container, normalized_snippet_hash; \
                 selector_precision: 82 -> 41; removed: container; added: normalized_snippet_hash; \
                 scope.path: src/lib.rs -> none; \
                 occurrence_limit: 1 -> none; \
                 lifecycle.expires: none -> 2026-12-01; \
                 evidence.evidence: removed: test:old-proof; added: test:new-proof; \
                 metadata.owner: core -> none; \
                 requirement.owner_required: true -> false; \
                 policy_status: active -> advisory"
                    .to_string()
            )
        );
    }

    #[test]
    fn option_and_list_helpers_render_none_empty_and_joined_values() {
        assert_eq!(option_text(Some("active")), "active");
        assert_eq!(option_text(None), "none");
        assert_eq!(option_u32_text(Some(7)), "7");
        assert_eq!(option_u32_text(None), "none");

        assert_eq!(list_text(&[]), "none");
        assert_eq!(list_text(&["container", "callee"]), "container, callee");

        let empty_owned: Vec<String> = Vec::new();
        assert_eq!(owned_list_text(&empty_owned), "none");

        let owned = vec!["test:old".to_string(), "test:new".to_string()];
        assert_eq!(owned_list_text(&owned), "test:old, test:new");
    }
}
