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
