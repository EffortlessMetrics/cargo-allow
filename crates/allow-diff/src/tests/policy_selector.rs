use super::*;

#[test]
fn detects_selector_precision_decrease() {
    let base = config_with(entry("allow-1"));
    let mut weaker = entry("allow-1");
    weaker.selector.normalized_snippet_hash = None;
    weaker.selector.container = None;
    let head = config_with(weaker);

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::SelectorPrecisionDecreased
            && change.message.contains("decreased")
            && change
                .message
                .contains("removed: container, normalized_snippet_hash")
    }));
}

#[test]
fn detects_equal_precision_selector_retarget_as_review_required() {
    let base = config_with(entry("allow-1"));
    let mut retargeted = entry("allow-1");
    retargeted.selector.container = Some("store".to_string());
    retargeted.selector.normalized_snippet_hash = Some("fnv1a64:store".to_string());
    retargeted.selector.line_hint = Some(900);
    let head = config_with(retargeted);

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::SelectorChanged
            && change.severity == PolicyChangeSeverity::Review
            && change.message.contains("selector identity changed")
    }));
    assert!(
        !changes.iter().any(|change| matches!(
            change.kind,
            PolicyChangeKind::SelectorPrecisionDecreased
                | PolicyChangeKind::SelectorPrecisionIncreased
        )),
        "equal-precision retargets should not be hidden by unchanged precision score"
    );
}

#[test]
fn detects_equal_precision_selector_field_swaps_as_review_required() {
    let base = config_with(entry("allow-1"));
    let mut swapped = entry("allow-1");
    swapped.selector.callee = None;
    swapped.selector.macro_name = Some("panic".to_string());
    let head = config_with(swapped);

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::SelectorChanged
            && change.severity == PolicyChangeSeverity::Review
    }));
    assert!(
        !changes.iter().any(|change| matches!(
            change.kind,
            PolicyChangeKind::SelectorPrecisionDecreased
                | PolicyChangeKind::SelectorPrecisionIncreased
        )),
        "callee and macro_name have equal precision weight, but the selector identity changed"
    );
}

#[test]
fn selector_retarget_ignores_line_hint_only_changes() {
    let base = config_with(entry("allow-1"));
    let mut moved = entry("allow-1");
    moved.selector.line_hint = Some(900);
    let head = config_with(moved);

    let changes = policy_changes(&base, &head);

    assert!(
        !changes
            .iter()
            .any(|change| change.kind == PolicyChangeKind::SelectorChanged),
        "line hints are review hints only, not selector identity"
    );
}

#[test]
fn detects_allow_entry_retargeted_to_different_kind_or_family() {
    let base = config_with(entry("allow-1"));
    let mut retargeted = entry("allow-1");
    retargeted.kind = FindingKind::Unsafe;
    retargeted.family = Some("unsafe_block".to_string());
    let head = config_with(retargeted);

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::KindChanged
            && change.severity == PolicyChangeSeverity::Fail
            && change.message.contains("panic -> unsafe")
    }));
    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::FamilyChanged
            && change.severity == PolicyChangeSeverity::Fail
            && change.message.contains("unwrap -> unsafe_block")
    }));
}

#[test]
fn detects_selector_precision_increase_as_improvement() {
    let mut weaker = entry("allow-1");
    weaker.selector.normalized_snippet_hash = None;
    weaker.selector.container = None;
    let base = config_with(weaker);
    let head = config_with(entry("allow-1"));

    let changes = policy_changes(&base, &head);

    assert!(changes.iter().any(|change| {
        change.kind == PolicyChangeKind::SelectorPrecisionIncreased
            && change.severity == PolicyChangeSeverity::Improvement
            && change.message.contains("increased")
            && change
                .message
                .contains("added: container, normalized_snippet_hash")
    }));
}

#[test]
fn selector_precision_scores_structural_selectors_above_glob_only_scope() {
    let strong = entry("allow-1");
    let mut weak = entry("allow-1");
    weak.path = None;
    weak.glob = Some("src/**".to_string());
    weak.selector.ast_kind = None;
    weak.selector.container = None;
    weak.selector.callee = None;
    weak.selector.normalized_snippet_hash = None;

    assert!(selector_precision_score(&strong) > selector_precision_score(&weak));
}

#[test]
fn selector_precision_ignores_line_hints() {
    let mut with_hint = entry("allow-1");
    with_hint.selector.line_hint = Some(900);
    let mut without_hint = entry("allow-1");
    without_hint.selector.line_hint = None;

    assert_eq!(
        selector_precision_score(&with_hint),
        selector_precision_score(&without_hint)
    );
}

#[test]
fn selector_precision_ignores_blank_selector_fields() {
    let mut blank = entry("allow-1");
    blank.selector.ast_kind = Some("   ".to_string());
    blank.selector.container = Some("".to_string());
    blank.selector.callee = Some("   ".to_string());
    blank.selector.macro_name = Some("".to_string());
    blank.selector.lint = Some("   ".to_string());
    blank.selector.symbol = Some("".to_string());
    blank.selector.receiver_fingerprint = Some("   ".to_string());
    blank.selector.target_fingerprint = Some("".to_string());
    blank.selector.normalized_snippet_hash = Some("   ".to_string());

    let mut absent = blank.clone();
    absent.selector.ast_kind = None;
    absent.selector.container = None;
    absent.selector.callee = None;
    absent.selector.macro_name = None;
    absent.selector.lint = None;
    absent.selector.symbol = None;
    absent.selector.receiver_fingerprint = None;
    absent.selector.target_fingerprint = None;
    absent.selector.normalized_snippet_hash = None;

    assert_eq!(
        selector_precision_score(&blank),
        selector_precision_score(&absent)
    );
}
