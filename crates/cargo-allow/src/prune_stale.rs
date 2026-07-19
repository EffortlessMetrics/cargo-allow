use super::PruneCandidate;
use allow_core::{AllowConfig, MatchOutcome, MatchStatus};
use std::collections::BTreeSet;

pub(super) fn prune_stale_candidates(
    cfg: &AllowConfig,
    outcomes: &[MatchOutcome],
) -> Vec<PruneCandidate> {
    outcomes
        .iter()
        .filter(|outcome| outcome.status == MatchStatus::Stale)
        .filter_map(|outcome| {
            let id = outcome.allow_id.as_deref()?;
            let entry = cfg.allow.iter().find(|entry| entry.id == id)?;
            Some(PruneCandidate {
                id: entry.id.clone(),
                kind: entry.kind,
                family: entry.family.clone(),
                owner: entry.owner.clone(),
                classification: entry.classification.clone(),
                scope: entry.path_or_glob(),
                reason: entry.reason.clone(),
            })
        })
        .collect()
}

pub(super) fn config_without_prune_candidates(
    cfg: &AllowConfig,
    candidates: &[PruneCandidate],
) -> AllowConfig {
    let mut pruned = cfg.clone();
    pruned
        .allow
        .retain(|entry| !candidates.iter().any(|candidate| candidate.id == entry.id));
    pruned
}

pub(super) fn removed_toml_blocks(
    rendered_policy: &str,
    candidates: &[PruneCandidate],
) -> Vec<String> {
    let ids = candidates
        .iter()
        .map(|candidate| candidate.id.as_str())
        .collect::<BTreeSet<_>>();
    allow_blocks(rendered_policy)
        .into_iter()
        .filter(|block| ids.iter().any(|id| block_contains_allow_id(block, id)))
        .map(str::to_string)
        .collect()
}

fn allow_blocks(rendered_policy: &str) -> Vec<&str> {
    let mut blocks = Vec::new();
    let mut start = None;
    let mut offset = 0;
    for line in rendered_policy.split_inclusive('\n') {
        let line_text = line.trim_end_matches('\n').trim_end_matches('\r');
        if line_text == "[[allow]]"
            && let Some(previous) = start.replace(offset)
            && let Some(block) = rendered_policy.get(previous..offset)
        {
            blocks.push(block.trim_end());
        }
        offset += line.len();
    }
    if let Some(previous) = start
        && let Some(block) = rendered_policy.get(previous..)
    {
        blocks.push(block.trim_end());
    }
    blocks
}

fn block_contains_allow_id(block: &str, id: &str) -> bool {
    block
        .lines()
        .any(|line| line.trim() == format!("id = \"{}\"", escape_toml_basic(id)))
}

fn escape_toml_basic(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::FindingKind;

    #[test]
    fn allow_blocks_call_presence_observer() {
        let rendered = r#"schema_version = "0.1"

[[allow]]
id = "allow-one"
reason = "first"

[[allow]]
id = "allow-two"
reason = "second"
"#;

        let blocks = allow_blocks(rendered);

        assert_eq!(blocks.len(), 2);
        assert_eq!(
            blocks.first().copied(),
            Some("[[allow]]\nid = \"allow-one\"\nreason = \"first\"")
        );
        assert_eq!(
            blocks.get(1).copied(),
            Some("[[allow]]\nid = \"allow-two\"\nreason = \"second\"")
        );
    }

    #[test]
    fn block_contains_allow_id_call_presence_observer() {
        let block = "[[allow]]\nid = \"allow-\\\"quoted\\\"\"\nreason = \"fixture\"";

        assert!(block_contains_allow_id(block, "allow-\"quoted\""));
        assert!(!block_contains_allow_id(block, "allow-quoted"));
        assert!(!block_contains_allow_id(
            "[[allow]]\n# id = \"allow-\\\"quoted\\\"\"",
            "allow-\"quoted\""
        ));
    }

    #[test]
    fn escape_toml_basic_call_presence_observer() {
        assert_eq!(
            escape_toml_basic("slash\\quote\"newline\nreturn\rtab\tplain"),
            "slash\\\\quote\\\"newline\\nreturn\\rtab\\tplain"
        );
    }

    #[test]
    fn escape_toml_basic_match_arm_observer() {
        let cases = [
            ('\\', "\\\\"),
            ('"', "\\\""),
            ('\n', "\\n"),
            ('\r', "\\r"),
            ('\t', "\\t"),
            ('x', "x"),
        ];

        for (input, expected) in cases {
            assert_eq!(escape_toml_basic(&input.to_string()), expected);
        }
    }

    #[test]
    fn removed_toml_blocks_call_presence_observer() {
        let rendered = r#"schema_version = "0.1"

[[allow]]
id = "allow-keep"
path = "docs/keep.md"

[[allow]]
id = "allow-remove"
path = "docs/remove.md"

[[allow]]
id = "allow-remove-suffix"
path = "docs/suffix.md"
"#;
        let candidates = vec![prune_candidate("allow-remove")];

        let blocks = removed_toml_blocks(rendered, &candidates);

        assert_eq!(blocks.len(), 1);
        let block = blocks
            .first()
            .unwrap_or_else(|| std::panic::panic_any("expected removed block"));
        assert!(block.contains("id = \"allow-remove\""));
        assert!(!block.contains("allow-keep"));
        assert!(!block.contains("allow-remove-suffix"));
    }

    fn prune_candidate(id: &str) -> PruneCandidate {
        PruneCandidate {
            id: id.to_string(),
            kind: FindingKind::NonRustFile,
            family: Some("documentation".to_string()),
            owner: "docs".to_string(),
            classification: "reviewed".to_string(),
            scope: "docs/remove.md".to_string(),
            reason: "fixture".to_string(),
        }
    }
}
