use super::{parse_policy, starter_policy};

#[test]
fn starter_policy_defaults_to_no_new_mode() {
    let policy = starter_policy(false, "policy/allow.toml");
    assert!(policy.contains("owner = \"core/policy\""));
    assert!(policy.contains("default_mode = \"no-new\""));
    assert!(policy.contains("stale_entries_fail = false"));

    let cfg = parse_policy(&policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("starter policy parses: {err}")));

    assert_eq!(cfg.workspace.default_mode, "no-new");
    assert!(!cfg.requirements.stale_entries_fail);
    assert!(cfg.requirements.unsafe_evidence_required);
}

#[test]
fn strict_starter_policy_enables_strict_defaults() {
    let policy = starter_policy(true, "policy/allow.toml");
    assert!(policy.contains("default_mode = \"strict\""));
    assert!(policy.contains("stale_entries_fail = true"));

    let cfg = parse_policy(&policy).unwrap_or_else(|err| {
        std::panic::panic_any(format!("strict starter policy parses: {err}"))
    });

    assert_eq!(cfg.workspace.default_mode, "strict");
    assert!(cfg.requirements.stale_entries_fail);
    assert!(cfg.requirements.unsafe_evidence_required);
}
