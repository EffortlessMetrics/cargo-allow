use std::collections::BTreeSet;

pub(crate) fn safety_comment_lines(source: &str) -> BTreeSet<u32> {
    source
        .lines()
        .enumerate()
        .filter_map(|(line_idx, line)| is_safety_comment(line).then_some((line_idx + 1) as u32))
        .collect()
}

pub(crate) fn has_nearby_safety_comment(safety_comments: &BTreeSet<u32>, line_no: u32) -> bool {
    let first = line_no.saturating_sub(3).max(1);
    (first..=line_no).any(|line| safety_comments.contains(&line))
}

fn is_safety_comment(line: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*') {
        return trimmed.contains("SAFETY:");
    }
    line.split_once("//")
        .is_some_and(|(_, comment)| comment.contains("SAFETY:"))
}
