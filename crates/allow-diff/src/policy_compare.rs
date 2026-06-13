use allow_core::SimpleDate;

pub(crate) fn date_extended(base: Option<&str>, head: Option<&str>) -> bool {
    match (base, head) {
        (Some(base), Some(head)) if base == head => false,
        (Some(base), Some("never")) if base != "never" => true,
        (Some(base), Some(head)) => match (SimpleDate::parse(base), SimpleDate::parse(head)) {
            (Some(base_date), Some(head_date)) => head_date > base_date,
            _ => false,
        },
        (Some(base), None) => base != "never",
        _ => false,
    }
}

pub(crate) fn date_shortened(base: Option<&str>, head: Option<&str>) -> bool {
    match (base, head) {
        (_, Some("never")) => false,
        (Some(base), Some(head)) if base == head => false,
        (Some("never"), Some(head)) => SimpleDate::parse(head).is_some(),
        (Some(base), Some(head)) => match (SimpleDate::parse(base), SimpleDate::parse(head)) {
            (Some(base_date), Some(head_date)) => head_date < base_date,
            _ => false,
        },
        (None, Some(head)) => SimpleDate::parse(head).is_some(),
        _ => false,
    }
}

pub(crate) fn removed_values(base: &[String], head: &[String]) -> bool {
    base.iter()
        .any(|item| !head.iter().any(|head| head == item))
}

pub(crate) fn added_values(base: &[String], head: &[String]) -> bool {
    head.iter()
        .any(|item| !base.iter().any(|base| base == item))
}

pub(crate) fn removed_required_text(base: &str, head: &str) -> bool {
    !base.trim().is_empty() && head.trim().is_empty()
}

pub(crate) fn added_required_text(base: &str, head: &str) -> bool {
    base.trim().is_empty() && !head.trim().is_empty()
}

pub(crate) fn changed_required_text(base: &str, head: &str) -> bool {
    let base = base.trim();
    let head = head.trim();
    !base.is_empty() && !head.is_empty() && base != head
}

pub(crate) fn optional_text_removed(base: Option<&str>, head: Option<&str>) -> bool {
    matches!(
        (trimmed_non_empty(base), trimmed_non_empty(head)),
        (Some(_), None)
    )
}

pub(crate) fn optional_text_added(base: Option<&str>, head: Option<&str>) -> bool {
    matches!(
        (trimmed_non_empty(base), trimmed_non_empty(head)),
        (None, Some(_))
    )
}

pub(crate) fn optional_text_changed(base: Option<&str>, head: Option<&str>) -> bool {
    match (trimmed_non_empty(base), trimmed_non_empty(head)) {
        (Some(base), Some(head)) => base != head,
        _ => false,
    }
}

fn trimmed_non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

pub(crate) fn occurrence_limit_loosened(base: Option<u32>, head: Option<u32>) -> bool {
    match (base, head) {
        (Some(_), None) => true,
        (Some(base), Some(head)) => head > base,
        _ => false,
    }
}

pub(crate) fn occurrence_limit_tightened(base: Option<u32>, head: Option<u32>) -> bool {
    match (base, head) {
        (None, Some(_)) => true,
        (Some(base), Some(head)) => head < base,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_extended_detects_later_or_removed_expiry_only() {
        assert!(!date_extended(Some("2026-01-01"), Some("2026-01-01")));
        assert!(date_extended(Some("2026-01-01"), Some("never")));
        assert!(!date_extended(Some("never"), Some("never")));
        assert!(date_extended(Some("2026-01-01"), Some("2026-02-01")));
        assert!(!date_extended(Some("2026-02-01"), Some("2026-01-01")));
        assert!(!date_extended(Some("not-a-date"), Some("2026-01-01")));
        assert!(!date_extended(Some("2026-01-01"), Some("not-a-date")));
        assert!(date_extended(Some("2026-01-01"), None));
        assert!(!date_extended(Some("never"), None));
        assert!(!date_extended(None, Some("2026-01-01")));
        assert!(!date_extended(None, None));
    }

    #[test]
    fn date_shortened_detects_earlier_or_added_expiry_only() {
        assert!(!date_shortened(Some("2026-01-01"), Some("never")));
        assert!(!date_shortened(Some("never"), Some("never")));
        assert!(!date_shortened(Some("2026-01-01"), Some("2026-01-01")));
        assert!(date_shortened(Some("never"), Some("2026-01-01")));
        assert!(!date_shortened(Some("never"), Some("not-a-date")));
        assert!(date_shortened(Some("2026-02-01"), Some("2026-01-01")));
        assert!(!date_shortened(Some("2026-01-01"), Some("2026-02-01")));
        assert!(!date_shortened(Some("not-a-date"), Some("2026-01-01")));
        assert!(!date_shortened(Some("2026-01-01"), Some("not-a-date")));
        assert!(date_shortened(None, Some("2026-01-01")));
        assert!(!date_shortened(None, Some("not-a-date")));
        assert!(!date_shortened(None, None));
    }

    #[test]
    fn value_set_helpers_detect_added_and_removed_items() {
        let base = vec!["one".to_owned(), "two".to_owned()];
        let reordered = vec!["two".to_owned(), "one".to_owned()];
        let removed = vec!["one".to_owned()];
        let added = vec!["one".to_owned(), "two".to_owned(), "three".to_owned()];

        assert!(!removed_values(&base, &reordered));
        assert!(removed_values(&base, &removed));
        assert!(!added_values(&base, &reordered));
        assert!(added_values(&base, &added));
    }

    #[test]
    fn required_text_helpers_trim_and_compare_required_fields() {
        assert!(removed_required_text(" evidence ", "   "));
        assert!(!removed_required_text("   ", "   "));
        assert!(added_required_text("   ", " evidence "));
        assert!(!added_required_text(" evidence ", " more evidence "));
        assert!(changed_required_text(" evidence ", " more evidence "));
        assert!(!changed_required_text(" evidence ", " evidence "));
        assert!(!changed_required_text("   ", " evidence "));
        assert!(!changed_required_text(" evidence ", "   "));
    }

    #[test]
    fn optional_text_helpers_trim_empty_values_and_compare_present_values() {
        assert_eq!(trimmed_non_empty(None), None);
        assert_eq!(trimmed_non_empty(Some("   ")), None);
        assert_eq!(trimmed_non_empty(Some(" value ")), Some("value"));

        assert!(optional_text_removed(Some(" value "), None));
        assert!(optional_text_removed(Some(" value "), Some("   ")));
        assert!(!optional_text_removed(None, Some(" value ")));

        assert!(optional_text_added(None, Some(" value ")));
        assert!(optional_text_added(Some("   "), Some(" value ")));
        assert!(!optional_text_added(Some(" value "), None));

        assert!(optional_text_changed(Some(" old "), Some(" new ")));
        assert!(!optional_text_changed(Some(" value "), Some(" value ")));
        assert!(!optional_text_changed(None, Some(" value ")));
        assert!(!optional_text_changed(Some(" value "), None));
    }

    #[test]
    fn occurrence_limit_helpers_classify_removed_added_and_numeric_changes() {
        assert!(occurrence_limit_loosened(Some(1), None));
        assert!(occurrence_limit_loosened(Some(1), Some(2)));
        assert!(!occurrence_limit_loosened(Some(2), Some(1)));
        assert!(!occurrence_limit_loosened(Some(1), Some(1)));
        assert!(!occurrence_limit_loosened(None, Some(1)));
        assert!(!occurrence_limit_loosened(None, None));

        assert!(occurrence_limit_tightened(None, Some(1)));
        assert!(occurrence_limit_tightened(Some(2), Some(1)));
        assert!(!occurrence_limit_tightened(Some(1), Some(2)));
        assert!(!occurrence_limit_tightened(Some(1), Some(1)));
        assert!(!occurrence_limit_tightened(Some(1), None));
        assert!(!occurrence_limit_tightened(None, None));
    }
}
