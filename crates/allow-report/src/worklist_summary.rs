use crate::WorklistItem;

pub(crate) fn worklist_risk_count(items: &[WorklistItem<'_>], risk: &str) -> usize {
    items.iter().filter(|item| item.risk == risk).count()
}

pub(crate) fn worklist_difficulty_count(items: &[WorklistItem<'_>], difficulty: &str) -> usize {
    items
        .iter()
        .filter(|item| item.difficulty == difficulty)
        .count()
}
