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
