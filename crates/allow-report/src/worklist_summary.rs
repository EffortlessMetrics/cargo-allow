use crate::WorklistItem;
use std::collections::BTreeMap;

pub(crate) fn worklist_risk_count(items: &[WorklistItem<'_>], risk: &str) -> usize {
    items.iter().filter(|item| item.risk == risk).count()
}

pub(crate) fn worklist_difficulty_count(items: &[WorklistItem<'_>], difficulty: &str) -> usize {
    items
        .iter()
        .filter(|item| item.difficulty == difficulty)
        .count()
}

pub(crate) fn worklist_kind_counts<'a>(items: &'a [WorklistItem<'a>]) -> BTreeMap<&'a str, usize> {
    let mut counts = BTreeMap::new();
    for item in items {
        *counts.entry(item.kind).or_insert(0) += 1;
    }
    counts
}
