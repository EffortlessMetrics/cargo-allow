use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use allow_report::{AdvisoryClass, ReportContext, Summary, advisory_count_for_deny_field};

pub(crate) fn validate_deny_statuses(
    statuses: &[String],
    summary: &Summary,
    context: ReportContext<'_>,
) -> CargoAllowResult<()> {
    let supported = AdvisoryClass::receipt_fields(summary, context)
        .into_iter()
        .map(|(class, _)| class.field_name())
        .collect::<Vec<_>>();
    for status in statuses {
        if !supported.iter().any(|field| field == status) {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::Usage,
                format!(
                    "unknown --deny status `{status}`; supported advisory classes: {}",
                    supported.join(", ")
                ),
            ));
        }
    }
    Ok(())
}

/// The requested `--deny` classes whose advisory count is currently positive
/// — the classes that actually escalate this run to a failure. The report
/// label threads them through so `Result: failed (...)` names the real
/// trigger instead of only the blocking status counts (#4631).
pub(crate) fn deny_escalation_fired_classes(
    deny: &[String],
    summary: &Summary,
    context: ReportContext<'_>,
) -> Vec<String> {
    deny.iter()
        .filter(|status| {
            advisory_count_for_deny_field(summary, context, status).is_some_and(|count| count > 0)
        })
        .cloned()
        .collect()
}

#[cfg(test)]
#[path = "check_deny_tests.rs"]
mod tests;
