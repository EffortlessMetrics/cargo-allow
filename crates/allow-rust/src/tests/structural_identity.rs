use std::fs;
use std::path::{Path, PathBuf};

use allow_core::{Finding, FindingKind, finding_identity_key};

use crate::scan_rust_source;

const FIXTURE_ROOT: &str = "../../tests/fixtures/structural-identity";

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_ROOT)
}

fn read_fixture_pair(name: &str) -> (String, String) {
    let dir = fixture_root().join(name);
    let before = fs::read_to_string(dir.join("before.rs")).unwrap_or_else(|err| {
        std::panic::panic_any(format!("read {} before.rs: {err}", dir.display()))
    });
    let after = fs::read_to_string(dir.join("after.rs")).unwrap_or_else(|err| {
        std::panic::panic_any(format!("read {} after.rs: {err}", dir.display()))
    });
    (before, after)
}

fn scan_fixture_pair(name: &str, path: &Path) -> (Vec<Finding>, Vec<Finding>) {
    let (before, after) = read_fixture_pair(name);
    (
        scan_rust_source(path, &before),
        scan_rust_source(path, &after),
    )
}

fn single_finding<'a>(findings: &'a [Finding], kind: FindingKind, family: &str) -> &'a Finding {
    findings
        .iter()
        .find(|finding| finding.kind == kind && finding.family.as_deref() == Some(family))
        .unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "expected one {kind} {family} finding among {} findings",
                findings.len()
            ))
        })
}

fn single_unsafe_block<'a>(findings: &'a [Finding], container: &str) -> &'a Finding {
    findings
        .iter()
        .find(|finding| {
            finding.kind == FindingKind::Unsafe
                && finding.family.as_deref() == Some("unsafe_block")
                && finding.identity.container.as_deref() == Some(container)
        })
        .unwrap_or_else(|| std::panic::panic_any(format!("missing unsafe_block in {container}")))
}

#[test]
fn refactor_pair_line_move_preserves_structural_identity() {
    let path = PathBuf::from("src/fixture.rs");
    let (before, after) = scan_fixture_pair("line_move", &path);
    let before_finding = single_finding(&before, FindingKind::Panic, "expect");
    let after_finding = single_finding(&after, FindingKind::Panic, "expect");

    assert_eq!(
        before_finding.identity.stable_key(),
        after_finding.identity.stable_key(),
        "line movement should preserve structural stable key"
    );
    assert_ne!(
        before_finding.identity.line_hint, after_finding.identity.line_hint,
        "line_hint should track movement as a review hint"
    );
    assert_eq!(
        before_finding.identity.receiver_fingerprint.as_deref(),
        Some("param:0")
    );
    assert_eq!(before_finding.identity.callee.as_deref(), Some("expect"));
}

#[test]
fn refactor_pair_function_move_preserves_unsafe_block_identity() {
    let path = PathBuf::from("src/fixture.rs");
    let (before, after) = scan_fixture_pair("function_move", &path);

    for container in ["read_left", "read_right"] {
        let before_block = single_unsafe_block(&before, container);
        let after_block = single_unsafe_block(&after, container);
        assert_eq!(
            before_block.identity.stable_key(),
            after_block.identity.stable_key(),
            "reordering {container} should preserve structural identity"
        );
        assert_eq!(before_block.identity.container.as_deref(), Some(container));
    }
}

#[test]
fn refactor_pair_module_move_changes_module_and_container_identity() {
    let path = PathBuf::from("src/fixture.rs");
    let (before, after) = scan_fixture_pair("module_move", &path);
    let before_block = single_unsafe_block(&before, "inner::access");
    let after_block = single_unsafe_block(&after, "access");

    assert_eq!(
        before_block.identity.module.as_deref(),
        Some("inner"),
        "nested module should be recorded"
    );
    assert_eq!(after_block.identity.module.as_deref(), None);
    assert_eq!(
        before_block.identity.container.as_deref(),
        Some("inner::access"),
        "nested free functions should qualify container with module path"
    );
    assert_eq!(after_block.identity.container.as_deref(), Some("access"));
    assert_ne!(
        before_block.identity.stable_key(),
        after_block.identity.stable_key(),
        "module movement should change structural identity"
    );
}

#[test]
fn refactor_pair_rename_local_preserves_structural_identity() {
    let path = PathBuf::from("src/fixture.rs");
    let (before, after) = scan_fixture_pair("rename_local", &path);
    let before_finding = single_finding(&before, FindingKind::Panic, "expect");
    let after_finding = single_finding(&after, FindingKind::Panic, "expect");

    assert_eq!(before_finding.identity.callee.as_deref(), Some("expect"));
    assert_eq!(after_finding.identity.callee.as_deref(), Some("expect"));
    assert_eq!(
        before_finding.identity.receiver_fingerprint.as_deref(),
        Some("param:0")
    );
    assert_eq!(
        after_finding.identity.receiver_fingerprint.as_deref(),
        Some("param:0")
    );
    assert_ne!(
        before_finding.identity.normalized_snippet_hash,
        after_finding.identity.normalized_snippet_hash,
        "local line text still reflects the renamed binding"
    );
}

#[test]
fn refactor_pair_same_callee_different_receiver_changes_identity() {
    let path = PathBuf::from("src/fixture.rs");
    let (before, after) = scan_fixture_pair("callee_same_receiver_diff", &path);
    let before_finding = single_finding(&before, FindingKind::Panic, "unwrap");
    let after_finding = single_finding(&after, FindingKind::Panic, "unwrap");

    assert_eq!(before_finding.identity.callee.as_deref(), Some("unwrap"));
    assert_eq!(after_finding.identity.callee.as_deref(), Some("unwrap"));
    assert_eq!(
        before_finding.identity.receiver_fingerprint.as_deref(),
        Some("param:0")
    );
    assert_eq!(
        after_finding.identity.receiver_fingerprint.as_deref(),
        Some("param:1")
    );
    assert_ne!(
        before_finding.identity.stable_key(),
        after_finding.identity.stable_key()
    );
}

#[test]
fn refactor_pair_same_lint_on_different_items_changes_container_identity() {
    let path = PathBuf::from("src/fixture.rs");
    let (before, after) = scan_fixture_pair("lint_same_different_items", &path);

    let before_parse = before
        .iter()
        .find(|finding| {
            finding.kind == FindingKind::LintException
                && finding.identity.container.as_deref() == Some("parse")
        })
        .unwrap_or_else(|| std::panic::panic_any("missing parse lint finding"));
    let after_parse = after
        .iter()
        .find(|finding| {
            finding.kind == FindingKind::LintException
                && finding.identity.container.as_deref() == Some("parse")
        })
        .unwrap_or_else(|| std::panic::panic_any("missing parse lint finding after reorder"));

    assert_eq!(before_parse.identity.lint.as_deref(), Some("dead_code"));
    assert_eq!(
        before_parse.identity.target_fingerprint.as_deref(),
        Some("policy:allow-0226")
    );
    assert_eq!(
        before_parse.identity.stable_key(),
        after_parse.identity.stable_key(),
        "reordering lint targets should preserve per-item identity"
    );

    let before_render = before
        .iter()
        .find(|finding| {
            finding.kind == FindingKind::LintException
                && finding.identity.container.as_deref() == Some("render")
        })
        .unwrap_or_else(|| std::panic::panic_any("missing render lint finding"));
    assert_eq!(
        before_render.identity.target_fingerprint.as_deref(),
        Some("policy:allow-0225")
    );
    assert_ne!(
        before_parse.identity.stable_key(),
        before_render.identity.stable_key(),
        "same lint on different items should differ by container"
    );
}

#[test]
fn refactor_pair_same_macro_at_different_paths_changes_finding_key() {
    let (before_src, after_src) = read_fixture_pair("macro_same_different_paths");
    let before_path = PathBuf::from("src/load.rs");
    let after_path = PathBuf::from("src/fail.rs");
    let before_findings = scan_rust_source(&before_path, &before_src);
    let after_findings = scan_rust_source(&after_path, &after_src);
    let before_macro = single_finding(&before_findings, FindingKind::Panic, "panic_macro");
    let after_macro = single_finding(&after_findings, FindingKind::Panic, "panic_macro");

    assert_eq!(before_macro.identity.macro_name.as_deref(), Some("panic"));
    assert_eq!(after_macro.identity.macro_name.as_deref(), Some("panic"));
    assert_eq!(
        before_macro.identity.stable_key(),
        after_macro.identity.stable_key(),
        "identical source text should preserve structural stable key across paths"
    );
    assert_ne!(
        finding_identity_key(before_macro),
        finding_identity_key(after_macro),
        "different scan paths should change finding identity key"
    );
}

#[test]
fn refactor_pair_same_index_form_different_targets_changes_identity() {
    let path = PathBuf::from("src/fixture.rs");
    let (before, after) = scan_fixture_pair("index_same_form_diff_targets", &path);
    let before_index = single_finding(&before, FindingKind::Panic, "indexing");
    let after_index = single_finding(&after, FindingKind::Panic, "indexing");

    assert_eq!(
        before_index.identity.receiver_fingerprint.as_deref(),
        Some("param:0")
    );
    assert_eq!(
        after_index.identity.receiver_fingerprint.as_deref(),
        Some("param:1")
    );
    assert_eq!(
        before_index.identity.target_fingerprint.as_deref(),
        Some("0")
    );
    assert_eq!(
        after_index.identity.target_fingerprint.as_deref(),
        Some("0")
    );
    assert_eq!(before_index.identity.symbol.as_deref(), Some("left[0]"));
    assert_eq!(after_index.identity.symbol.as_deref(), Some("right[0]"));
    assert_ne!(
        before_index.identity.stable_key(),
        after_index.identity.stable_key()
    );
}

#[test]
fn refactor_pair_sibling_modules_same_function_name_have_distinct_container_identity() {
    let path = PathBuf::from("src/fixture.rs");
    let (before, _) = scan_fixture_pair("container_same_name_sibling_modules", &path);
    let blocks: Vec<_> = before
        .iter()
        .filter(|f| f.kind == FindingKind::Unsafe && f.family.as_deref() == Some("unsafe_block"))
        .collect();
    assert_eq!(blocks.len(), 2);
    let alpha = blocks
        .iter()
        .find(|f| f.identity.module.as_deref() == Some("alpha"))
        .unwrap_or_else(|| std::panic::panic_any("missing alpha unsafe block"));
    let beta = blocks
        .iter()
        .find(|f| f.identity.module.as_deref() == Some("beta"))
        .unwrap_or_else(|| std::panic::panic_any("missing beta unsafe block"));
    assert_eq!(alpha.identity.container.as_deref(), Some("alpha::access"));
    assert_eq!(beta.identity.container.as_deref(), Some("beta::access"));
    assert_ne!(
        alpha.identity.stable_key(),
        beta.identity.stable_key(),
        "same function name in sibling modules must not collide"
    );
}

#[test]
fn structural_identity_field_matrix_documents_fixture_classifications() {
    let path = PathBuf::from("src/fixture.rs");
    let (before, _after) = scan_fixture_pair("line_move", &path);
    let finding = single_finding(&before, FindingKind::Panic, "expect");

    assert_eq!(finding.identity.language, "rust");
    assert_eq!(finding.identity.ast_kind, "method_call");
    assert!(finding.identity.container.is_some(), "container: stable");
    assert!(finding.identity.callee.is_some(), "callee: stable");
    assert!(
        finding.identity.receiver_fingerprint.is_some(),
        "receiver_fingerprint: stable"
    );
    assert!(
        finding.identity.normalized_snippet_hash.is_some(),
        "normalized_snippet_hash: stable"
    );
    assert!(
        finding.identity.line_hint.is_some(),
        "line_hint: useful hint"
    );
    assert!(
        finding.identity.column_hint.is_some(),
        "column_hint: useful hint"
    );
    assert_eq!(
        finding.identity.crate_name, None,
        "crate_name: missing without manifest context"
    );

    let (before_findings, after_findings) = scan_fixture_pair("module_move", &path);
    let nested = single_unsafe_block(&before_findings, "inner::access");
    let top_level = single_unsafe_block(&after_findings, "access");
    assert_eq!(nested.identity.module.as_deref(), Some("inner"));
    assert_eq!(top_level.identity.module.as_deref(), None);
}
