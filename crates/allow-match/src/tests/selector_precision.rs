use std::fs;
use std::path::PathBuf;

use allow_core::{AllowConfig, AllowEntry, Finding, MatchStatus};
use allow_policy::parse_policy;
use allow_rust::scan_rust_source;

use crate::{CheckMode, STRUCTURAL_MATCH_THRESHOLD, evaluate, score_match};

const FIXTURE_ROOT: &str = "../../tests/fixtures/structural-identity";
const POLICY_PATH: &str = "../../policy/allow.toml";

const STRUCTURAL_FIXTURE_ENTRY_IDS: &[&str] = &[
    "allow-0215",
    "allow-0216",
    "allow-0217",
    "allow-0218",
    "allow-0219",
    "allow-0220",
    "allow-0221",
    "allow-0222",
    "allow-0223",
    "allow-0224",
    "allow-0225",
    "allow-0226",
    "allow-0229",
    "allow-0230",
    "allow-0231",
    "allow-0232",
    "allow-0233",
    "allow-0234",
    "allow-0243",
    "allow-0244",
    "allow-0245",
    "allow-0246",
];

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixture_root() -> PathBuf {
    workspace_root().join(FIXTURE_ROOT)
}

fn read_fixture(name: &str, side: &str) -> String {
    let path = fixture_root().join(name).join(format!("{side}.rs"));
    fs::read_to_string(&path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read {}: {err}", path.display())))
}

fn scan_fixture(name: &str, side: &str) -> Vec<Finding> {
    let rel = PathBuf::from("tests/fixtures/structural-identity")
        .join(name)
        .join(format!("{side}.rs"));
    scan_rust_source(&rel, &read_fixture(name, side))
}

fn structural_fixture_policy() -> AllowConfig {
    let policy_text = fs::read_to_string(workspace_root().join(POLICY_PATH))
        .unwrap_or_else(|err| std::panic::panic_any(format!("read policy: {err}")));
    let full = parse_policy(&policy_text)
        .unwrap_or_else(|err| std::panic::panic_any(format!("parse policy: {err}")));
    let allow = full
        .allow
        .into_iter()
        .filter(|entry| STRUCTURAL_FIXTURE_ENTRY_IDS.contains(&entry.id.as_str()))
        .collect();
    AllowConfig { allow, ..full }
}

fn entry_by_id<'a>(cfg: &'a AllowConfig, id: &str) -> &'a AllowEntry {
    cfg.allow
        .iter()
        .find(|entry| entry.id == id)
        .unwrap_or_else(|| std::panic::panic_any(format!("missing policy entry {id}")))
}

fn finding_matches_entry(finding: &Finding, entry: &AllowEntry) -> bool {
    score_match(entry, finding).is_some_and(|score| score >= STRUCTURAL_MATCH_THRESHOLD)
}

fn assert_unique_match(cfg: &AllowConfig, finding: &Finding, expected_id: &str) {
    let matching: Vec<_> = cfg
        .allow
        .iter()
        .filter(|entry| finding_matches_entry(finding, entry))
        .map(|entry| entry.id.as_str())
        .collect();
    assert_eq!(
        matching,
        vec![expected_id],
        "finding at {} should match only {expected_id}",
        finding.path.display()
    );
}

fn assert_eval_matched(
    cfg: &AllowConfig,
    findings: &[Finding],
    finding_index: usize,
    expected_id: &str,
) {
    let outcomes = evaluate(cfg, findings, CheckMode::NoNew);
    let matched = outcomes
        .iter()
        .filter(|outcome| {
            matches!(
                outcome.status,
                MatchStatus::Matched | MatchStatus::LocationDrift
            ) && outcome.finding_index == Some(finding_index)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matched.len(),
        1,
        "expected one Matched or location_drift outcome for finding index {finding_index}"
    );
    assert_eq!(matched[0].allow_id.as_deref(), Some(expected_id));
    assert!(
        !outcomes
            .iter()
            .any(|outcome| outcome.status == MatchStatus::Ambiguous),
        "evaluate should not report ambiguous matches"
    );
}

fn single_finding_by_container<'a>(findings: &'a [Finding], container: &str) -> &'a Finding {
    findings
        .iter()
        .find(|finding| finding.identity.container.as_deref() == Some(container))
        .unwrap_or_else(|| {
            std::panic::panic_any(format!("missing finding with container `{container}`"))
        })
}

fn single_finding_by_receiver<'a>(findings: &'a [Finding], receiver: &str) -> &'a Finding {
    findings
        .iter()
        .find(|finding| finding.identity.receiver_fingerprint.as_deref() == Some(receiver))
        .unwrap_or_else(|| {
            std::panic::panic_any(format!("missing finding with receiver `{receiver}`"))
        })
}

fn single_finding_by_target_policy<'a>(findings: &'a [Finding], policy_id: &str) -> &'a Finding {
    let target = format!("policy:{policy_id}");
    findings
        .iter()
        .find(|finding| finding.identity.target_fingerprint.as_deref() == Some(target.as_str()))
        .unwrap_or_else(|| std::panic::panic_any(format!("missing finding with target `{target}`")))
}

#[test]
fn selector_precision_receiver_fingerprint_discriminates_parameter_slots() {
    let cfg = structural_fixture_policy();
    let before = scan_fixture("callee_same_receiver_diff", "before");
    let after = scan_fixture("callee_same_receiver_diff", "after");
    let before_finding = single_finding_by_receiver(&before, "param:0");
    let after_finding = single_finding_by_receiver(&after, "param:1");

    assert_unique_match(&cfg, before_finding, "allow-0216");
    assert_unique_match(&cfg, after_finding, "allow-0215");
    assert!(!finding_matches_entry(
        before_finding,
        entry_by_id(&cfg, "allow-0215")
    ));
    assert!(!finding_matches_entry(
        after_finding,
        entry_by_id(&cfg, "allow-0216")
    ));
}

#[test]
fn selector_precision_container_discriminates_unsafe_blocks_in_same_file() {
    let cfg = structural_fixture_policy();
    let before = scan_fixture("function_move", "before");
    let read_left = single_finding_by_container(&before, "read_left");
    let read_right = single_finding_by_container(&before, "read_right");

    assert_unique_match(&cfg, read_left, "allow-0219");
    assert_unique_match(&cfg, read_right, "allow-0220");
}

#[test]
fn selector_precision_container_discriminates_module_qualified_access() {
    let cfg = structural_fixture_policy();
    let before = scan_fixture("module_move", "before");
    let after = scan_fixture("module_move", "after");
    let nested = single_finding_by_container(&before, "inner::access");
    let top_level = single_finding_by_container(&after, "access");

    assert_unique_match(&cfg, nested, "allow-0232");
    assert_unique_match(&cfg, top_level, "allow-0231");
    assert!(!finding_matches_entry(
        nested,
        entry_by_id(&cfg, "allow-0231")
    ));
    assert!(!finding_matches_entry(
        top_level,
        entry_by_id(&cfg, "allow-0232")
    ));
}

#[test]
fn selector_precision_lint_target_fingerprint_and_container_discriminate_items() {
    let cfg = structural_fixture_policy();
    let before = scan_fixture("lint_same_different_items", "before");
    let parse = single_finding_by_target_policy(&before, "allow-0226");
    let render = single_finding_by_target_policy(&before, "allow-0225");

    assert_eq!(parse.identity.container.as_deref(), Some("parse"));
    assert_eq!(render.identity.container.as_deref(), Some("render"));
    assert_unique_match(&cfg, parse, "allow-0226");
    assert_unique_match(&cfg, render, "allow-0225");
    assert_eval_matched(&cfg, &before, 0, "allow-0226");
    assert_eval_matched(&cfg, &before, 1, "allow-0225");
}

#[test]
fn selector_precision_index_receiver_and_symbol_discriminate_targets() {
    let cfg = structural_fixture_policy();
    let before = scan_fixture("index_same_form_diff_targets", "before");
    let after = scan_fixture("index_same_form_diff_targets", "after");
    let left = before
        .iter()
        .find(|finding| finding.identity.symbol.as_deref() == Some("left[0]"))
        .unwrap_or_else(|| std::panic::panic_any("missing left[0] index finding"));
    let right = after
        .iter()
        .find(|finding| finding.identity.symbol.as_deref() == Some("right[0]"))
        .unwrap_or_else(|| std::panic::panic_any("missing right[0] index finding"));

    assert_eq!(
        left.identity.receiver_fingerprint.as_deref(),
        Some("param:0")
    );
    assert_eq!(
        right.identity.receiver_fingerprint.as_deref(),
        Some("param:1")
    );
    assert_eq!(left.identity.target_fingerprint.as_deref(), Some("0"));
    assert_eq!(right.identity.target_fingerprint.as_deref(), Some("0"));
    assert_unique_match(&cfg, left, "allow-0222");
    assert_unique_match(&cfg, right, "allow-0221");
}

#[test]
fn selector_precision_container_discriminates_sibling_module_functions() {
    let cfg = structural_fixture_policy();
    let before = scan_fixture("container_same_name_sibling_modules", "before");
    let alpha = single_finding_by_container(&before, "alpha::access");
    let beta = single_finding_by_container(&before, "beta::access");

    assert_unique_match(&cfg, alpha, "allow-0243");
    assert_unique_match(&cfg, beta, "allow-0244");
}

#[test]
fn selector_precision_snippet_hash_discriminates_rename_only_refactors() {
    let cfg = structural_fixture_policy();
    let before = scan_fixture("rename_local", "before");
    let after = scan_fixture("rename_local", "after");
    let before_finding = before
        .iter()
        .find(|finding| finding.family.as_deref() == Some("expect"))
        .unwrap_or_else(|| std::panic::panic_any("missing expect finding in rename_local before"));
    let after_finding = after
        .iter()
        .find(|finding| finding.family.as_deref() == Some("expect"))
        .unwrap_or_else(|| std::panic::panic_any("missing expect finding in rename_local after"));

    assert_eq!(
        before_finding.identity.receiver_fingerprint.as_deref(),
        Some("param:0")
    );
    assert_eq!(
        after_finding.identity.receiver_fingerprint.as_deref(),
        Some("param:0")
    );
    assert_unique_match(&cfg, before_finding, "allow-0234");
    assert_unique_match(&cfg, after_finding, "allow-0233");
}

#[test]
fn selector_precision_macro_entries_match_path_scoped_findings() {
    let cfg = structural_fixture_policy();
    let before_path =
        PathBuf::from("tests/fixtures/structural-identity/macro_same_different_paths/before.rs");
    let after_path =
        PathBuf::from("tests/fixtures/structural-identity/macro_same_different_paths/after.rs");
    let before = scan_rust_source(
        &before_path,
        &read_fixture("macro_same_different_paths", "before"),
    );
    let after = scan_rust_source(
        &after_path,
        &read_fixture("macro_same_different_paths", "after"),
    );
    let before_macro = before
        .iter()
        .find(|finding| finding.family.as_deref() == Some("panic_macro"))
        .unwrap_or_else(|| std::panic::panic_any("missing before panic macro finding"));
    let after_macro = after
        .iter()
        .find(|finding| finding.family.as_deref() == Some("panic_macro"))
        .unwrap_or_else(|| std::panic::panic_any("missing after panic macro finding"));

    assert_unique_match(&cfg, before_macro, "allow-0230");
    assert_unique_match(&cfg, after_macro, "allow-0229");
}

#[test]
fn selector_precision_line_move_matches_path_scoped_entry() {
    let cfg = structural_fixture_policy();
    let before = scan_fixture("line_move", "before");
    let after = scan_fixture("line_move", "after");
    let before_finding = before
        .iter()
        .find(|finding| finding.family.as_deref() == Some("expect"))
        .unwrap_or_else(|| std::panic::panic_any("missing expect finding in line_move before"));
    let after_finding = after
        .iter()
        .find(|finding| finding.family.as_deref() == Some("expect"))
        .unwrap_or_else(|| std::panic::panic_any("missing expect finding in line_move after"));

    assert_unique_match(&cfg, before_finding, "allow-0224");
    assert_unique_match(&cfg, after_finding, "allow-0223");
}
