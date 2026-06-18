use std::fs;
use std::path::PathBuf;

use allow_core::{AllowConfig, AllowEntry, Finding, FindingKind};
use allow_policy::parse_policy;
use allow_rust::scan_rust_source;

use crate::policy_change::{PolicyChangeKind, PolicyChangeSeverity};
use crate::{FindingPostureKind, finding_identity_key, finding_posture_changes, policy_changes};

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

fn single_finding_by_receiver<'a>(findings: &'a [Finding], receiver: &str) -> &'a Finding {
    findings
        .iter()
        .find(|finding| finding.identity.receiver_fingerprint.as_deref() == Some(receiver))
        .unwrap_or_else(|| {
            std::panic::panic_any(format!("missing finding with receiver `{receiver}`"))
        })
}

fn config_with(entry: AllowEntry) -> AllowConfig {
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry);
    cfg
}

#[test]
fn fixture_policy_weakening_dropping_container_reports_precision_decrease() {
    let cfg = structural_fixture_policy();
    let base = entry_by_id(&cfg, "allow-0215").clone();
    let mut head = base.clone();
    head.selector.container = None;
    head.selector.normalized_snippet_hash = None;

    let changes = policy_changes(&config_with(base), &config_with(head));

    let change = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::SelectorPrecisionDecreased)
        .unwrap_or_else(|| std::panic::panic_any("expected selector precision decrease"));
    assert_eq!(change.allow_id, "allow-0215");
    assert_eq!(change.severity, PolicyChangeSeverity::Fail);
    assert!(
        change
            .message
            .contains("removed: container, normalized_snippet_hash")
    );
    let detail = change
        .selector_precision
        .as_ref()
        .unwrap_or_else(|| std::panic::panic_any("expected selector precision detail"));
    assert!(detail.before > detail.after);
    assert_eq!(
        detail.removed_fields,
        vec!["container", "normalized_snippet_hash"]
    );
}

#[test]
fn fixture_policy_improvement_adding_identity_fields_reports_precision_increase() {
    let cfg = structural_fixture_policy();
    let head = entry_by_id(&cfg, "allow-0215").clone();
    let mut base = head.clone();
    base.selector.container = None;
    base.selector.normalized_snippet_hash = None;

    let changes = policy_changes(&config_with(base), &config_with(head));

    let change = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::SelectorPrecisionIncreased)
        .unwrap_or_else(|| std::panic::panic_any("expected selector precision increase"));
    assert_eq!(change.allow_id, "allow-0215");
    assert_eq!(change.severity, PolicyChangeSeverity::Improvement);
    assert!(
        change
            .message
            .contains("added: container, normalized_snippet_hash")
    );
    let detail = change
        .selector_precision
        .as_ref()
        .unwrap_or_else(|| std::panic::panic_any("expected selector precision detail"));
    assert!(detail.after > detail.before);
    assert_eq!(
        detail.added_fields,
        vec!["container", "normalized_snippet_hash"]
    );
}

#[test]
fn fixture_policy_equal_precision_receiver_retarget_reports_selector_identity_change() {
    let cfg = structural_fixture_policy();
    let base = entry_by_id(&cfg, "allow-0215").clone();
    let mut head = base.clone();
    head.selector.receiver_fingerprint = Some("param:0".to_string());
    head.selector.normalized_snippet_hash = Some("fnv1a64:dc9ff63ad6f05c95".to_string());

    let changes = policy_changes(&config_with(base), &config_with(head));

    let change = changes
        .iter()
        .find(|change| change.kind == PolicyChangeKind::SelectorChanged)
        .unwrap_or_else(|| std::panic::panic_any("expected selector identity change"));
    assert_eq!(change.allow_id, "allow-0215");
    assert_eq!(change.severity, PolicyChangeSeverity::Review);
    assert_eq!(
        change
            .selector_identity
            .as_ref()
            .map(|identity| identity.changed_fields.clone()),
        Some(vec!["receiver_fingerprint", "normalized_snippet_hash"])
    );
    assert!(
        !changes.iter().any(|change| matches!(
            change.kind,
            PolicyChangeKind::SelectorPrecisionDecreased
                | PolicyChangeKind::SelectorPrecisionIncreased
        )),
        "equal-precision retarget should not be classified as precision change"
    );
}

#[test]
fn fixture_finding_posture_reports_identity_loss_between_refactor_sides() {
    let path =
        PathBuf::from("tests/fixtures/structural-identity/callee_same_receiver_diff/before.rs");
    let before = scan_rust_source(&path, &read_fixture("callee_same_receiver_diff", "before"));
    let after = scan_rust_source(&path, &read_fixture("callee_same_receiver_diff", "after"));
    let before_finding = single_finding_by_receiver(&before, "param:0");
    let after_finding = single_finding_by_receiver(&after, "param:1");

    assert_eq!(before_finding.kind, FindingKind::Panic);
    assert_eq!(after_finding.kind, FindingKind::Panic);
    assert_ne!(
        finding_identity_key(before_finding),
        finding_identity_key(after_finding),
        "receiver slot change should change finding identity key"
    );

    let changes = finding_posture_changes(&before, &after);

    assert!(
        changes.iter().any(|change| {
            change.kind == FindingPostureKind::Removed
                && change.identity.receiver_fingerprint.as_deref() == Some("param:0")
        }),
        "baseline receiver identity should be removed"
    );
    assert!(
        changes.iter().any(|change| {
            change.kind == FindingPostureKind::New
                && change.identity.receiver_fingerprint.as_deref() == Some("param:1")
        }),
        "candidate receiver identity should be new"
    );
}

#[test]
fn fixture_finding_posture_preserves_identity_across_line_move_refactor() {
    let path = PathBuf::from("tests/fixtures/structural-identity/line_move/before.rs");
    let before = scan_rust_source(&path, &read_fixture("line_move", "before"));
    let after = scan_rust_source(&path, &read_fixture("line_move", "after"));

    let changes = finding_posture_changes(&before, &after);

    assert!(
        changes.is_empty(),
        "line movement should not produce finding posture changes: {changes:?}"
    );
}
