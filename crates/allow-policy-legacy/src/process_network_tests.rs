use super::*;
use crate::test_support::*;
use allow_core::FindingKind;
use std::path::Path;

#[test]
fn migrates_process_allowlist_to_policy_exception_entries() {
    let policy = process_policy_fixture_path();
    let cfg = load_legacy_or_canonical(&policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("process policy migrates: {err}")));

    assert_eq!(cfg.policy, "cargo-allow");
    assert_eq!(cfg.allow.len(), 2);
    let install = cfg
        .allow
        .iter()
        .find(|entry| entry.id == "proc-cargo-install-cargo-deny")
        .unwrap_or_else(|| std::panic::panic_any("expected cargo install process entry"));
    assert_eq!(install.kind, FindingKind::PolicyException);
    assert_eq!(install.family.as_deref(), Some("process_spawn"));
    assert_eq!(install.classification, "network_process");
    assert_eq!(
        install.path.as_deref(),
        Some(Path::new(".github/workflows/ci.yml"))
    );
    assert_eq!(install.selector.ast_kind.as_deref(), Some("process_spawn"));
    assert_eq!(
        install.selector.symbol.as_deref(),
        Some("cargo install cargo-deny --locked")
    );
    assert_eq!(
        install.selector.target_fingerprint.as_deref(),
        Some("process:cargo install cargo-deny --locked")
    );
    assert_eq!(
        install.lifecycle.review_after.as_deref(),
        Some("2026-09-09")
    );
    assert!(
        install
            .evidence
            .iter()
            .any(|item| item == "network_reach:true")
    );

    let local = cfg
        .allow
        .iter()
        .find(|entry| entry.id == "proc-bash-package-proof")
        .unwrap_or_else(|| std::panic::panic_any("expected package proof process entry"));
    assert_eq!(local.classification, "local_process");
    assert_eq!(local.lifecycle.expires.as_deref(), Some("never"));
    assert_eq!(local.lifecycle.review_after.as_deref(), Some("2026-05-09"));
}

#[test]
fn process_compat_synthesizes_matched_new_and_stale_drift() {
    let policy = process_policy_fixture_path();
    let cfg = load_process_compat_config(&policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("process compat config loads: {err}")));
    let findings = process_findings_from_config(&cfg);

    let matched = allow_match::evaluate(&cfg, &findings, allow_match::CheckMode::NoNew);
    assert_eq!(
        matched
            .iter()
            .filter(|outcome| outcome.status == allow_core::MatchStatus::Matched)
            .count(),
        2
    );

    let missing_allow = allow_match::evaluate(
        &cfg,
        &[process_policy_finding(
            ".github/workflows/release.yml",
            "bash scripts/publish.sh",
        )],
        allow_match::CheckMode::NoNew,
    );
    assert!(
        missing_allow
            .iter()
            .any(|outcome| outcome.status == allow_core::MatchStatus::New)
    );

    let stale_allow = allow_match::evaluate(&cfg, &[], allow_match::CheckMode::Audit);
    assert!(
        stale_allow
            .iter()
            .any(|outcome| outcome.status == allow_core::MatchStatus::Stale)
    );
}

#[test]
fn process_policy_requires_legacy_xtask_fields() {
    let policy = malformed_process_policy_fixture_path();
    let err = load_process_compat_config(&policy)
        .expect_err("process policy without network_reach should fail");
    assert!(
        err.to_string()
            .contains("proc-missing missing network_reach")
    );
}

#[test]
fn migrates_network_allowlist_to_policy_exception_entries() {
    let policy = network_policy_fixture_path();
    let cfg = load_legacy_or_canonical(&policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("network policy migrates: {err}")));

    assert_eq!(cfg.policy, "cargo-allow");
    assert_eq!(cfg.allow.len(), 2);
    let public = cfg
        .allow
        .iter()
        .find(|entry| entry.id == "net-crates-io-fetch")
        .unwrap_or_else(|| std::panic::panic_any("expected crates.io network entry"));
    assert_eq!(public.kind, FindingKind::PolicyException);
    assert_eq!(public.family.as_deref(), Some("network_destination"));
    assert_eq!(public.classification, "public_network");
    assert_eq!(
        public.path.as_deref(),
        Some(Path::new("policy/network-allowlist.toml"))
    );
    assert_eq!(
        public.selector.ast_kind.as_deref(),
        Some("network_destination")
    );
    assert_eq!(
        public.selector.symbol.as_deref(),
        Some("crates.io lane build")
    );
    assert_eq!(
        public.selector.target_fingerprint.as_deref(),
        Some("network:crates.io:auth:false:lane:build")
    );
    assert_eq!(public.lifecycle.expires.as_deref(), Some("never"));
    assert_eq!(public.lifecycle.review_after.as_deref(), Some("2026-05-09"));

    let authenticated = cfg
        .allow
        .iter()
        .find(|entry| entry.id == "net-github-api")
        .unwrap_or_else(|| std::panic::panic_any("expected GitHub API network entry"));
    assert_eq!(authenticated.classification, "authenticated_network");
    assert!(
        authenticated
            .evidence
            .iter()
            .any(|item| item == "auth_secret:GITHUB_TOKEN")
    );
}

#[test]
fn network_compat_synthesizes_matched_new_and_stale_drift() {
    let policy = network_policy_fixture_path();
    let cfg = load_network_compat_config(&policy)
        .unwrap_or_else(|err| std::panic::panic_any(format!("network compat config loads: {err}")));
    let findings = network_findings_from_config(&cfg);

    let matched = allow_match::evaluate(&cfg, &findings, allow_match::CheckMode::NoNew);
    assert_eq!(
        matched
            .iter()
            .filter(|outcome| outcome.status == allow_core::MatchStatus::Matched)
            .count(),
        2
    );

    let missing_allow = allow_match::evaluate(
        &cfg,
        &[network_policy_finding("example.com lane test")],
        allow_match::CheckMode::NoNew,
    );
    assert!(
        missing_allow
            .iter()
            .any(|outcome| outcome.status == allow_core::MatchStatus::New)
    );

    let stale_allow = allow_match::evaluate(&cfg, &[], allow_match::CheckMode::Audit);
    assert!(
        stale_allow
            .iter()
            .any(|outcome| outcome.status == allow_core::MatchStatus::Stale)
    );
}

#[test]
fn network_policy_requires_legacy_xtask_fields() {
    let policy = malformed_network_policy_fixture_path();
    let err = load_network_compat_config(&policy)
        .expect_err("network policy without auth_required should fail");
    assert!(
        err.to_string()
            .contains("net-missing missing auth_required")
    );
}
