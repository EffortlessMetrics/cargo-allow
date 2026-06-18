use allow_core::{CargoAllowError, CargoAllowResult};
use allow_report::{
    ADVISORY_DENY_FIELD_NAMES, ReportContext, Summary, advisory_count_for_deny_field,
};

pub(crate) fn validate_deny_statuses(statuses: &[String]) -> CargoAllowResult<()> {
    for status in statuses {
        if advisory_count_for_deny_field(&Summary::default(), ReportContext::default(), status)
            .is_none()
        {
            return Err(CargoAllowError::new(format!(
                "unknown --deny status `{status}`; supported advisory classes: {}",
                ADVISORY_DENY_FIELD_NAMES.join(", ")
            )));
        }
    }
    Ok(())
}

pub(crate) fn deny_escalation_failed(
    deny: &[String],
    summary: &Summary,
    context: ReportContext<'_>,
) -> bool {
    deny.iter().any(|status| {
        advisory_count_for_deny_field(summary, context, status).is_some_and(|count| count > 0)
    })
}

#[cfg(test)]
#[path = "check_deny_tests.rs"]
mod tests;
