use super::{parse_revision_record, validate_revision_ledger};

const VALID: &str = r#"
schema_version = "1.0"
id = "CARGO-ALLOW-REV-0001"
created = "2026-06-20"
owner = "repo-infra"
reason = "Narrow selector after parser refactor."

allow_ids = ["allow-0042"]
change_kinds = ["selector_changed"]

links = ["issue:123", "pr:456"]
"#;

fn parse_err(input: &str) -> String {
    match parse_revision_record(input) {
        Ok(_) => std::panic::panic_any("expected revision parse failure"),
        Err(err) => err.to_string(),
    }
}

#[test]
fn parses_minimal_valid_record() {
    let record = parse_revision_record(VALID).expect("valid record parses");
    assert_eq!(record.id, "CARGO-ALLOW-REV-0001");
    assert_eq!(record.owner, "repo-infra");
    assert_eq!(record.allow_ids, vec!["allow-0042".to_string()]);
    assert_eq!(record.change_kinds, vec!["selector_changed".to_string()]);
    assert_eq!(record.links.len(), 2);
    assert!(record.expires.is_none());
    assert!(record.supersedes.is_none());
}

#[test]
fn covers_matches_listed_allow_id_and_change_kind() {
    let record = parse_revision_record(VALID).expect("valid record parses");
    assert!(record.covers("allow-0042", "selector_changed"));
    assert!(!record.covers("allow-0042", "scope_broadened"));
    assert!(!record.covers("allow-9999", "selector_changed"));
}

#[test]
fn accepts_expires_never_and_supersedes_chain() {
    let record = parse_revision_record(
        r#"
schema_version = "1.0"
id = "CARGO-ALLOW-REV-0002"
created = "2026-06-20"
owner = "repo-infra"
reason = "Supersede the earlier waiver."
allow_ids = ["allow-0042"]
change_kinds = ["scope_broadened"]
expires = "never"
supersedes = "CARGO-ALLOW-REV-0001"
"#,
    )
    .expect("record parses");
    assert_eq!(record.expires.as_deref(), Some("never"));
    assert_eq!(record.supersedes.as_deref(), Some("CARGO-ALLOW-REV-0001"));
}

#[test]
fn rejects_unsupported_schema_version() {
    let err = parse_err(&VALID.replace("\"1.0\"", "\"2.0\""));
    assert!(err.contains("unsupported revision schema_version"), "{err}");
}

#[test]
fn rejects_id_without_prefix() {
    let err = parse_err(&VALID.replace("CARGO-ALLOW-REV-0001", "REV-1"));
    assert!(err.contains("must start with `CARGO-ALLOW-REV-`"), "{err}");
}

#[test]
fn rejects_invalid_created_date() {
    let err = parse_err(&VALID.replace("2026-06-20", "2026-02-31"));
    assert!(err.contains("invalid created date"), "{err}");
}

#[test]
fn rejects_missing_reason() {
    let err = parse_err(&VALID.replace("reason = \"Narrow selector after parser refactor.\"", ""));
    assert!(err.contains("missing required field `reason`"), "{err}");
}

#[test]
fn rejects_empty_allow_ids() {
    let err = parse_err(&VALID.replace("allow_ids = [\"allow-0042\"]", "allow_ids = []"));
    assert!(err.contains("requires at least one allow_id"), "{err}");
}

#[test]
fn rejects_empty_change_kinds_as_blanket_waiver() {
    let err =
        parse_err(&VALID.replace("change_kinds = [\"selector_changed\"]", "change_kinds = []"));
    assert!(err.contains("blanket waivers are not allowed"), "{err}");
}

#[test]
fn rejects_non_snake_case_change_kind() {
    let err = parse_err(&VALID.replace("\"selector_changed\"", "\"SelectorChanged\""));
    assert!(err.contains("invalid change_kind token"), "{err}");
}

#[test]
fn rejects_consecutive_underscores_in_change_kind() {
    let err = parse_err(&VALID.replace("\"selector_changed\"", "\"selector__changed\""));
    assert!(err.contains("invalid change_kind token"), "{err}");
}

#[test]
fn rejects_bare_prefix_id() {
    let err = parse_err(&VALID.replace("CARGO-ALLOW-REV-0001", "CARGO-ALLOW-REV-"));
    assert!(err.contains("carry a suffix"), "{err}");
}

#[test]
fn rejects_unrecognized_link_prefix() {
    let err = parse_err(&VALID.replace("\"issue:123\"", "\"slack:123\""));
    assert!(err.contains("must use a recognized prefix"), "{err}");
}

#[test]
fn rejects_invalid_expires_date() {
    let err = parse_err(&VALID.replace(
        "links = [\"issue:123\", \"pr:456\"]",
        "expires = \"someday\"",
    ));
    assert!(err.contains("invalid expires date"), "{err}");
}

#[test]
fn rejects_unknown_fields() {
    let err = parse_err(&format!("{VALID}\nrubber_stamp = true\n"));
    assert!(
        err.contains("unknown field") || err.contains("rubber_stamp"),
        "{err}"
    );
}

#[test]
fn ledger_rejects_duplicate_ids() {
    let first = parse_revision_record(VALID).expect("valid record parses");
    let second = parse_revision_record(VALID).expect("valid record parses");
    let err = validate_revision_ledger(&[first, second])
        .expect_err("duplicate ids rejected")
        .to_string();
    assert!(err.contains("duplicate revision id"), "{err}");
}

#[test]
fn ledger_accepts_distinct_ids() {
    let first = parse_revision_record(VALID).expect("valid record parses");
    let second =
        parse_revision_record(&VALID.replace("0001", "0002")).expect("valid record parses");
    validate_revision_ledger(&[first, second]).expect("distinct ids accepted");
}
