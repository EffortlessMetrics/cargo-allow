use super::{RevisionRecord, parse_revision_record, validate_revision_ledger};

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

fn parse_ok(input: &str) -> RevisionRecord {
    match parse_revision_record(input) {
        Ok(record) => record,
        Err(err) => std::panic::panic_any(format!("valid record parses: {err}")),
    }
}

fn parse_err(input: &str) -> String {
    match parse_revision_record(input) {
        Ok(_) => std::panic::panic_any("expected revision parse failure"),
        Err(err) => err.to_string(),
    }
}

#[test]
fn parses_minimal_valid_record() {
    let record = parse_ok(VALID);
    assert_eq!(record.id, "CARGO-ALLOW-REV-0001");
    assert_eq!(record.owner, "repo-infra");
    assert_eq!(record.allow_ids, vec!["allow-0042".to_string()]);
    assert_eq!(record.change_kinds, vec!["selector_changed".to_string()]);
    assert_eq!(record.links.len(), 2);
    assert!(record.expires.is_none());
    assert!(record.supersedes.is_none());
    assert!(record.before_fingerprint.is_none());
    assert!(record.after_fingerprint.is_none());
}

#[test]
fn covers_matches_listed_allow_id_and_change_kind() {
    let record = parse_ok(VALID);
    assert!(record.covers("allow-0042", "selector_changed"));
    assert!(!record.covers("allow-0042", "scope_broadened"));
    assert!(!record.covers("allow-9999", "selector_changed"));
}

#[test]
fn covers_transition_ignores_fingerprint_for_non_repeatable_kinds() {
    // selector_changed is not a repeatable weakening kind: (allow_id, change_kind) suffices.
    let record = parse_ok(VALID);
    assert!(record.covers_transition("allow-0042", "selector_changed", None));
    assert!(record.covers_transition("allow-0042", "selector_changed", Some("sha256:abc")));
}

#[test]
fn covers_transition_requires_fingerprint_for_repeatable_kinds() {
    let record = parse_ok(
        r#"
schema_version = "1.0"
id = "CARGO-ALLOW-REV-0007"
created = "2026-06-20"
owner = "repo-infra"
reason = "Raise the occurrence limit for the generated module family."
allow_ids = ["allow-0042"]
change_kinds = ["occurrence_limit_loosened"]
after_fingerprint = "sha256:aaaa"
"#,
    );
    // Matching fingerprint authorizes this specific transition.
    assert!(record.covers_transition(
        "allow-0042",
        "occurrence_limit_loosened",
        Some("sha256:aaaa")
    ));
    // A later, different increase (different fingerprint) is NOT authorized.
    assert!(!record.covers_transition(
        "allow-0042",
        "occurrence_limit_loosened",
        Some("sha256:bbbb")
    ));
    // No observed fingerprint can never satisfy a repeatable kind.
    assert!(!record.covers_transition("allow-0042", "occurrence_limit_loosened", None));
}

#[test]
fn covers_transition_repeatable_kind_without_recorded_fingerprint_never_covers() {
    // Record lacks after_fingerprint but claims a repeatable kind: it cannot pin a transition.
    let record = parse_ok(
        r#"
schema_version = "1.0"
id = "CARGO-ALLOW-REV-0008"
created = "2026-06-20"
owner = "repo-infra"
reason = "Broaden scope after refactor."
allow_ids = ["allow-0042"]
change_kinds = ["scope_broadened"]
"#,
    );
    assert!(record.covers("allow-0042", "scope_broadened"));
    assert!(!record.covers_transition("allow-0042", "scope_broadened", Some("sha256:aaaa")));
}

#[test]
fn accepts_expires_never_and_supersedes_chain() {
    let record = parse_ok(
        r#"
schema_version = "1.0"
id = "CARGO-ALLOW-REV-0002"
created = "2026-06-20"
owner = "repo-infra"
reason = "Supersede the earlier waiver."
allow_ids = ["allow-0042"]
change_kinds = ["scope_broadened"]
after_fingerprint = "sha256:aaaa"
expires = "never"
supersedes = "CARGO-ALLOW-REV-0001"
"#,
    );
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
fn rejects_non_canonical_change_kind() {
    // Not a member of the canonical PolicyChangeKind token set.
    let err = parse_err(&VALID.replace("\"selector_changed\"", "\"SelectorChanged\""));
    assert!(err.contains("unknown change_kind"), "{err}");
}

#[test]
fn rejects_invented_change_kind_token() {
    // Plausible snake_case but not an actual PolicyChangeKind.
    let err = parse_err(&VALID.replace("\"selector_changed\"", "\"selector_loosened\""));
    assert!(err.contains("unknown change_kind"), "{err}");
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
    let first = parse_ok(VALID);
    let second = parse_ok(VALID);
    let err = match validate_revision_ledger(&[first, second]) {
        Ok(()) => std::panic::panic_any("duplicate ids should be rejected"),
        Err(err) => err.to_string(),
    };
    assert!(err.contains("duplicate revision id"), "{err}");
}

#[test]
fn ledger_accepts_distinct_ids() {
    let first = parse_ok(VALID);
    let second = parse_ok(&VALID.replace("0001", "0002"));
    if let Err(err) = validate_revision_ledger(&[first, second]) {
        std::panic::panic_any(format!("distinct ids accepted: {err}"));
    }
}
