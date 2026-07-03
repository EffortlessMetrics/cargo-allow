use super::{UncoveredCell, change_note_template, evaluate_with_records};
use allow_core::{AllowConfig, AllowEntry, FindingKind, Lifecycle, Selector};
use allow_diff::policy_changes;
use allow_policy::{RevisionRecord, parse_revision_record};
use std::path::PathBuf;

fn entry_with_limit(id: &str, limit: Option<u32>) -> AllowEntry {
    AllowEntry {
        id: id.to_string(),
        kind: FindingKind::Panic,
        family: Some("unwrap".to_string()),
        path: Some(PathBuf::from("src/lib.rs")),
        glob: None,
        owner: "core".to_string(),
        classification: "reviewed_exception".to_string(),
        reason: "Range is validated before use.".to_string(),
        evidence: vec!["test:range_is_validated".to_string()],
        links: Vec::new(),
        occurrence_limit: limit,
        lifecycle: Lifecycle {
            created: Some("2026-05-26".to_string()),
            review_after: Some("2026-08-01".to_string()),
            expires: Some("2026-09-01".to_string()),
        },
        selector: Selector {
            ast_kind: Some("method_call".to_string()),
            container: Some("load".to_string()),
            callee: Some("unwrap".to_string()),
            normalized_snippet_hash: Some("fnv1a64:1234".to_string()),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn config_with(entry: AllowEntry) -> AllowConfig {
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry);
    cfg
}

fn record(toml: &str) -> RevisionRecord {
    match parse_revision_record(toml) {
        Ok(record) => record,
        Err(err) => std::panic::panic_any(format!("valid revision record: {err}")),
    }
}

fn some<T>(value: Option<T>, message: &str) -> T {
    match value {
        Some(value) => value,
        None => std::panic::panic_any(message.to_string()),
    }
}

fn first<'a, T>(items: &'a [T], message: &str) -> &'a T {
    match items.first() {
        Some(item) => item,
        None => std::panic::panic_any(message.to_string()),
    }
}

/// occurrence_limit 5 -> 10 is `occurrence_limit_loosened` (worsened).
fn loosen_limit_diff() -> (AllowConfig, Vec<allow_diff::PolicyChange>) {
    let base = config_with(entry_with_limit("allow-0042", Some(5)));
    let head = config_with(entry_with_limit("allow-0042", Some(10)));
    let changes = policy_changes(&base, &head);
    (head, changes)
}

#[test]
fn weakening_edit_without_note_is_uncovered() {
    let (head, changes) = loosen_limit_diff();
    let eval = evaluate_with_records(&head, &changes, &[]);
    assert_eq!(eval.weakening_cells, 1, "one worsened cell expected");
    assert!(eval.failed(), "an uncovered weakening edit must fail");
    let cell = first(&eval.uncovered, "uncovered cell");
    assert_eq!(cell.allow_id, "allow-0042");
    assert_eq!(cell.change_kind, "occurrence_limit_loosened");
    assert_eq!(cell.posture, "worsened");
    assert!(
        cell.after_fingerprint.is_some(),
        "repeatable kind must carry a transition fingerprint"
    );
}

#[test]
fn valid_note_with_matching_fingerprint_covers_the_weakening() {
    let (head, changes) = loosen_limit_diff();
    // Discover the exact transition fingerprint enforcement computes.
    let uncovered = evaluate_with_records(&head, &changes, &[]).uncovered;
    let fingerprint = some(
        first(&uncovered, "uncovered cell")
            .after_fingerprint
            .clone(),
        "fingerprint",
    );

    let note = record(&format!(
        r#"
schema_version = "1.0"
id = "CARGO-ALLOW-REV-0001"
created = "2026-06-20"
owner = "repo-infra"
reason = "Raise occurrence limit for the generated module family."
allow_ids = ["allow-0042"]
change_kinds = ["occurrence_limit_loosened"]
after_fingerprint = "{fingerprint}"
"#
    ));

    let eval = evaluate_with_records(&head, &changes, &[note]);
    assert!(!eval.failed(), "matching note must cover the weakening");
    assert!(eval.uncovered.is_empty());
}

#[test]
fn reused_note_on_a_second_increase_fails() {
    // First transition 5 -> 10, note pins its fingerprint.
    let (head_10, changes_10) = loosen_limit_diff();
    let uncovered_10 = evaluate_with_records(&head_10, &changes_10, &[]).uncovered;
    let first_fp = some(
        first(&uncovered_10, "uncovered cell")
            .after_fingerprint
            .clone(),
        "fingerprint",
    );
    let note = record(&format!(
        r#"
schema_version = "1.0"
id = "CARGO-ALLOW-REV-0001"
created = "2026-06-20"
owner = "repo-infra"
reason = "Raise occurrence limit once."
allow_ids = ["allow-0042"]
change_kinds = ["occurrence_limit_loosened"]
after_fingerprint = "{first_fp}"
"#
    ));

    // A later, independent increase 10 -> 20 reuses the same note.
    let base = config_with(entry_with_limit("allow-0042", Some(10)));
    let head_20 = config_with(entry_with_limit("allow-0042", Some(20)));
    let changes_20 = policy_changes(&base, &head_20);

    let eval = evaluate_with_records(&head_20, &changes_20, &[note]);
    assert!(
        eval.failed(),
        "the note's fingerprint pins the 5->10 transition; a 10->20 increase must not reuse it"
    );
    assert_eq!(
        first(&eval.uncovered, "uncovered cell").change_kind,
        "occurrence_limit_loosened"
    );
}

#[test]
fn improvement_is_not_a_weakening_cell() {
    // occurrence_limit 10 -> 5 is `occurrence_limit_tightened` (improved).
    let base = config_with(entry_with_limit("allow-0042", Some(10)));
    let head = config_with(entry_with_limit("allow-0042", Some(5)));
    let changes = policy_changes(&base, &head);
    let eval = evaluate_with_records(&head, &changes, &[]);
    assert_eq!(eval.weakening_cells, 0, "an improvement is not a weakening");
    assert!(!eval.failed());
}

#[test]
fn non_repeatable_kind_covered_by_cell_without_fingerprint() {
    // owner "core" -> "" is `owner_removed` (worsened), a non-repeatable kind:
    // (allow_id, change_kind) coverage alone suffices, no fingerprint required.
    let base_entry = entry_with_limit("allow-0042", Some(5));
    let mut head_entry = entry_with_limit("allow-0042", Some(5));
    head_entry.owner = String::new();
    let head = config_with(head_entry);
    let changes = policy_changes(&config_with(base_entry), &head);

    // Sanity: an owner_removed cell exists.
    let bare = evaluate_with_records(&head, &changes, &[]);
    assert!(
        bare.uncovered
            .iter()
            .any(|c| c.change_kind == "owner_removed"),
        "expected an owner_removed weakening cell: {:?}",
        bare.uncovered
            .iter()
            .map(|c| c.change_kind.as_str())
            .collect::<Vec<_>>()
    );

    let note = record(
        r#"
schema_version = "1.0"
id = "CARGO-ALLOW-REV-0002"
created = "2026-06-20"
owner = "repo-infra"
reason = "Ownership handed to the platform on-call rotation."
allow_ids = ["allow-0042"]
change_kinds = ["owner_removed"]
"#,
    );
    let eval = evaluate_with_records(&head, &changes, &[note]);
    assert!(
        !eval
            .uncovered
            .iter()
            .any(|c| c.change_kind == "owner_removed"),
        "a non-repeatable weakening is covered without a fingerprint"
    );
}

#[test]
fn template_lists_uncovered_cells_and_fingerprints() {
    let cells = vec![
        UncoveredCell {
            allow_id: "allow-0042".to_string(),
            change_kind: "occurrence_limit_loosened".to_string(),
            posture: "worsened",
            after_fingerprint: Some("v1:deadbeef".to_string()),
        },
        UncoveredCell {
            allow_id: "allow-0043".to_string(),
            change_kind: "owner_removed".to_string(),
            posture: "worsened",
            after_fingerprint: None,
        },
    ];
    let template = change_note_template(&cells);
    // One record per cell (no cartesian aggregation), each with its own allow_ids.
    assert!(
        template.contains("allow_ids = [\"allow-0042\"]"),
        "{template}"
    );
    assert!(
        template.contains("allow_ids = [\"allow-0043\"]"),
        "{template}"
    );
    assert!(
        template.contains("change_kinds = [\"occurrence_limit_loosened\"]"),
        "{template}"
    );
    assert!(
        template.contains("change_kinds = [\"owner_removed\"]"),
        "{template}"
    );
    // The repeatable cell carries an ACTIVE after_fingerprint, not a comment.
    assert!(
        template.contains("after_fingerprint = \"v1:deadbeef\""),
        "{template}"
    );
    assert!(template.contains("separate record"), "{template}");
}

#[test]
fn template_record_round_trips_through_the_parser() {
    // A single-cell template, once placeholders are filled, must parse and cover
    // the transition it was generated for (regression for the comment-only bug).
    let (head, changes) = loosen_limit_diff();
    let uncovered = evaluate_with_records(&head, &changes, &[]).uncovered;
    let template = change_note_template(&uncovered);
    let filled = template
        .replace("CARGO-ALLOW-REV-XXXX", "CARGO-ALLOW-REV-0001")
        .replace("YYYY-MM-DD", "2026-06-20")
        .replace("TODO-owner", "repo-infra")
        .replace(
            "TODO: why this weakening edit is justified.",
            "Raise the occurrence limit.",
        );
    let note = record(&filled);
    let eval = evaluate_with_records(&head, &changes, &[note]);
    assert!(
        !eval.failed(),
        "a filled-in single-cell template must cover its own weakening: {filled}"
    );
}

#[test]
fn template_is_a_comment_when_nothing_is_uncovered() {
    let template = change_note_template(&[]);
    assert!(
        template.starts_with("# No uncovered weakening edits"),
        "{template}"
    );
}
