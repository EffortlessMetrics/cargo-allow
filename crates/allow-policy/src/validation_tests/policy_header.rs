use super::*;

#[test]
fn rejects_blank_policy_schema_version() {
    let err = parse_err(
        r#"
                schema_version = "   "
                policy = "cargo-allow"
            "#,
    );

    assert!(err.contains("policy schema_version must not be empty"));
}

#[test]
fn rejects_policy_schema_version_with_surrounding_whitespace() {
    let err = parse_err(
        r#"
                schema_version = " 0.1 "
                policy = "cargo-allow"
            "#,
    );

    assert!(err.contains("policy schema_version must not have leading or trailing whitespace"));
}

#[test]
fn rejects_unsupported_policy_schema_version() {
    let err = parse_err(
        r#"
                schema_version = "99.0"
                policy = "cargo-allow"
            "#,
    );

    assert!(err.contains("unsupported policy schema_version `99.0`"));
}

#[test]
fn rejects_policy_name_with_surrounding_whitespace() {
    let err = parse_err(
        r#"
                policy = " cargo-allow "
            "#,
    );

    assert!(err.contains("policy name must not have leading or trailing whitespace"));
}

#[test]
fn rejects_blank_policy_owner() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                owner = ""
            "#,
    );

    assert!(err.contains("policy owner must not be empty"));
}

#[test]
fn rejects_policy_owner_with_surrounding_whitespace() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                owner = " core/policy "
            "#,
    );

    assert!(err.contains("policy owner must not have leading or trailing whitespace"));
}

#[test]
fn rejects_blank_policy_status() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                status = "   "
            "#,
    );

    assert!(err.contains("policy status must not be empty"));
}

#[test]
fn accepts_advisory_policy_status() {
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"
                status = "advisory"
            "#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy should parse: {err}")));

    assert_eq!(cfg.status.as_deref(), Some("advisory"));
}

#[test]
fn rejects_policy_status_with_surrounding_whitespace() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                status = " advisory "
            "#,
    );

    assert!(err.contains("policy status must not have leading or trailing whitespace"));
}

#[test]
fn rejects_unsupported_policy_status() {
    let err = parse_err(
        r#"
                policy = "cargo-allow"
                status = "paused"
            "#,
    );

    assert!(err.contains("unsupported policy status `paused`"));
}
