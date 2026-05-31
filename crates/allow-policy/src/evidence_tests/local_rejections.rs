use std::fs;

use super::{escape_toml_string, remove_test_dir, unique_test_dir};
use crate::{parse_policy, validate_local_evidence_references};

#[test]
fn rejects_missing_local_evidence_references() {
    let root = unique_test_dir("evidence-missing");
    fs::create_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create root: {err}")));
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "allow-doc"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = ["doc:docs/missing.md"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy parses: {err}")));

    let err = validate_local_evidence_references(&root, &cfg).unwrap_err();
    assert!(err.to_string().contains("allow-doc evidence"));
    assert!(err.to_string().contains("missing local file"));
    remove_test_dir(root);
}

#[test]
fn rejects_directory_local_evidence_references() {
    let root = unique_test_dir("evidence-directory");
    fs::create_dir_all(root.join("docs/safety"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create evidence dir: {err}")));
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "allow-doc-dir"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = ["doc:docs/safety"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy parses: {err}")));

    let err = validate_local_evidence_references(&root, &cfg).unwrap_err();
    assert!(err.to_string().contains("allow-doc-dir evidence"));
    assert!(err.to_string().contains("not a directory"));
    remove_test_dir(root);
}

#[cfg(unix)]
#[test]
fn rejects_symlinked_local_evidence_references() {
    use std::os::unix::fs::symlink;

    let root = unique_test_dir("evidence-symlink");
    fs::create_dir_all(root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create docs dir: {err}")));
    fs::write(root.join("docs/real.md"), "review notes")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write evidence: {err}")));
    symlink(root.join("docs/real.md"), root.join("docs/link.md"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create symlink evidence: {err}")));
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "allow-doc-link"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = ["doc:docs/link.md"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy parses: {err}")));

    let err = validate_local_evidence_references(&root, &cfg).unwrap_err();
    assert!(err.to_string().contains("allow-doc-link evidence"));
    assert!(err.to_string().contains("symlink"));
    remove_test_dir(root);
}

#[cfg(unix)]
#[test]
fn rejects_symlinked_local_evidence_path_components() {
    use std::os::unix::fs::symlink;

    let root = unique_test_dir("evidence-symlink-component");
    fs::create_dir_all(root.join("actual-docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create actual docs dir: {err}")));
    fs::write(root.join("actual-docs/safety.md"), "review notes")
        .unwrap_or_else(|err| std::panic::panic_any(format!("write evidence: {err}")));
    symlink(root.join("actual-docs"), root.join("docs"))
        .unwrap_or_else(|err| std::panic::panic_any(format!("create symlinked docs dir: {err}")));
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "allow-doc-link-dir"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = ["doc:docs/safety.md"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy parses: {err}")));

    let err = validate_local_evidence_references(&root, &cfg).unwrap_err();
    assert!(err.to_string().contains("allow-doc-link-dir evidence"));
    assert!(err.to_string().contains("symlink component"));
    remove_test_dir(root);
}

#[test]
fn rejects_missing_unsafe_review_evidence_references() {
    let root = unique_test_dir("unsafe-review-evidence-missing");
    fs::create_dir_all(&root)
        .unwrap_or_else(|err| std::panic::panic_any(format!("create root: {err}")));
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "allow-unsafe-review"
                kind = "unsafe"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = ["unsafe-review:docs/evidence/unsafe-review/missing.json"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "unsafe_block"
                container = "load"
            "#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy parses: {err}")));

    let err = validate_local_evidence_references(&root, &cfg).unwrap_err();
    assert!(err.to_string().contains("allow-unsafe-review evidence"));
    assert!(err.to_string().contains("missing local file"));
    remove_test_dir(root);
}

#[test]
fn rejects_escaping_local_evidence_references() {
    let cfg = parse_policy(
        r#"
                policy = "cargo-allow"
                [[allow]]
                id = "allow-doc"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = ["doc:../outside.md"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
    )
    .unwrap_or_else(|err| std::panic::panic_any(format!("policy parses: {err}")));

    let err = validate_local_evidence_references(".", &cfg).unwrap_err();
    assert!(
        err.to_string()
            .contains("must not contain parent directory segments")
    );
}

#[test]
fn rejects_non_source_tree_relative_local_evidence_references() {
    let cases = [
        ("doc:", "has empty path"),
        ("doc:/absolute/safety.md", "source-tree-relative"),
        ("doc:C:/absolute/safety.md", "source-tree-relative"),
        (
            "doc:docs\\..\\outside.md",
            "must not contain parent directory segments",
        ),
    ];

    for (evidence, expected_message) in cases {
        let cfg = parse_policy(&format!(
            r#"
                policy = "cargo-allow"
                [[allow]]
                id = "allow-doc"
                kind = "panic"
                path = "src/lib.rs"
                owner = "core"
                classification = "reviewed"
                reason = "fixture"
                evidence = ["{}"]
                expires = "2026-08-01"
                [allow.selector]
                ast_kind = "method_call"
                callee = "unwrap"
            "#,
            escape_toml_string(evidence)
        ))
        .unwrap_or_else(|err| std::panic::panic_any(format!("policy parses: {err}")));

        let err = validate_local_evidence_references(".", &cfg).unwrap_err();
        assert!(
            err.to_string().contains(expected_message),
            "expected `{evidence}` error `{err}` to contain `{expected_message}`"
        );
    }
}
