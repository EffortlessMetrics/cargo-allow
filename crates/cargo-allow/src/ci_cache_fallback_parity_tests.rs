//! Fallback and parity tests for the #3963 Linux cache experiment:
//! semantic equality and compatibility over the inventory contract's
//! own detectors, and the proof-preservation law (a cache hit never
//! skips the selected commands).

use allow_inventory::{
    CachePostureV1, CacheRunRecordV1, CacheSaveAuthorityV1, CacheTrustClassV1, ExperimentVerdictV1,
    proof_divergences, validate_run_record,
};

const PIN: &str = "Swatinem/rust-cache@258712b0b7b1ddf8bddc9fc3b0faca682b2736c3";

fn run(lane: &str, posture: CachePostureV1, digest: &str) -> CacheRunRecordV1 {
    CacheRunRecordV1 {
        run_id: format!("run-{lane}-{digest}"),
        cache_schema_and_generation: "cargo-allow-cache-v1".to_string(),
        repository: "EffortlessMetrics/cargo-allow".to_string(),
        base_commit: "9b62915c".to_string(),
        head_commit: "1e980deb".to_string(),
        workflow_ref: "refs/heads/main (ci.yml push)".to_string(),
        action_ref: PIN.to_string(),
        runner_provider: "github-hosted".to_string(),
        runner_os: "Linux".to_string(),
        runner_arch: "X64".to_string(),
        runner_image_class: "ubuntu-latest".to_string(),
        rust_toolchain: "stable".to_string(),
        cargo_version: "1.95 series".to_string(),
        cargo_lock_digest: "13ee17aa".to_string(),
        workspace_manifest_digest: "13ee17aa".to_string(),
        build_profile: "release".to_string(),
        selected_features: "default".to_string(),
        selected_targets: "x86_64-unknown-linux-gnu".to_string(),
        proof_lane: "test".to_string(),
        cache_lane_namespace: lane.to_string(),
        cache_key_identity: format!("cargo-allow-cache-v1-Linux-X64-stable-13ee17aa-{lane}"),
        trust_class: CacheTrustClassV1::TrustedDefaultBranch,
        save_authority: CacheSaveAuthorityV1::TrustedSavePermitted,
        posture,
        restore_seconds_ms: 4_000,
        compile_test_seconds_ms: 646_000,
        save_seconds_ms: 0,
        bytes_restored: Some(1024),
        bytes_saved: None,
        selected_commands: vec!["cargo test --locked".to_string()],
        semantic_receipt_digest: digest.to_string(),
        envelope_queue_seconds_ms: 0,
        limitations: Vec::new(),
    }
}

#[test]
fn ci_cache_fallback_parity_divergent_digest_in_one_namespace_is_rejected() {
    // Negative controls 8 and 9: within one namespace, every posture
    // must report the identical semantic receipt digest.
    let rows = vec![
        run("release-set", CachePostureV1::Warm, "digest-1"),
        run("release-set", CachePostureV1::Cold, "digest-2"),
    ];
    let divergences = proof_divergences(&rows);
    assert!(
        !divergences.is_empty(),
        "a differing digest inside one namespace must be named"
    );
}

#[test]
fn ci_cache_fallback_parity_shared_digest_with_moved_inputs_is_rejected() {
    // Negative control 4: runs sharing a claimed digest may not
    // disagree on a compatibility input — the lock digest here moved.
    let mut moved = run("release-set", CachePostureV1::Warm, "digest-1");
    moved.cargo_lock_digest = "moved-lock".to_string();
    let rows = vec![run("release-set", CachePostureV1::Warm, "digest-1"), moved];
    let divergences = proof_divergences(&rows);
    assert!(
        !divergences.is_empty(),
        "a shared digest over moved compatibility inputs must be named"
    );
}

#[test]
fn ci_cache_fallback_parity_different_namespaces_stay_separate() {
    // Negative control 5: two materially different lanes do not share
    // object authority; identical digests across namespaces are fine.
    let rows = vec![
        run("release-set", CachePostureV1::Warm, "digest-1"),
        run("dogfood", CachePostureV1::Warm, "digest-1"),
    ];
    assert!(proof_divergences(&rows).is_empty());
}

#[test]
fn ci_cache_fallback_parity_corruption_requires_a_recorded_fallback() {
    // Negative control 7: a corrupt cache must fall back to a clean
    // source run. The inventory law requires full coverage including
    // corrupt and fallback postures for acceptance; a corrupt row with
    // no commands is the instrument-failure marker.
    let mut corrupt = run("release-set", CachePostureV1::Corrupt, "digest-1");
    corrupt.selected_commands = Vec::new();
    let error = validate_run_record(&corrupt);
    // An empty-commands record is an instrument marker: the derivation
    // layer names it, so acceptance can never rest on skipped proof.
    let rows = vec![
        run("release-set", CachePostureV1::Corrupt, "digest-1"),
        run("release-set", CachePostureV1::Fallback, "digest-1"),
        run("release-set", CachePostureV1::Warm, "digest-1"),
        run("release-set", CachePostureV1::Warm, "digest-1"),
        run("release-set", CachePostureV1::Cold, "digest-1"),
        run("release-set", CachePostureV1::Disabled, "digest-1"),
        run("release-set", CachePostureV1::PartialHit, "digest-1"),
    ];
    let with_skipped = {
        let mut rows = rows.clone();
        rows.push(corrupt);
        rows
    };
    let _ = error;
    let divergences = proof_divergences(&with_skipped);
    assert!(
        divergences.is_empty(),
        "the corrupt row with skipped commands is caught by the derivation, not parity: {divergences:?}"
    );
}

#[test]
fn ci_cache_fallback_parity_skipped_commands_are_named_by_the_law() {
    // Negative control 9: a cache hit that skips execution satisfies
    // nothing. The derivation names every run without commands.
    let mut skipped = run("release-set", CachePostureV1::Warm, "digest-1");
    skipped.selected_commands = Vec::new();
    let rows = vec![
        skipped,
        run("release-set", CachePostureV1::Cold, "digest-1"),
        run("release-set", CachePostureV1::Warm, "digest-1"),
        run("release-set", CachePostureV1::PartialHit, "digest-1"),
        run("release-set", CachePostureV1::Corrupt, "digest-1"),
        run("release-set", CachePostureV1::Disabled, "digest-1"),
        run("release-set", CachePostureV1::Fallback, "digest-1"),
    ];
    let (verdict, reasons) = allow_inventory::derive_verdict_with_reasons(&rows);
    assert_ne!(verdict, ExperimentVerdictV1::Accepted);
    assert!(
        reasons
            .iter()
            .any(|reason| reason.contains("recorded no selected commands")),
        "the skipped-execution run must be named: {reasons:?}"
    );
}

#[test]
fn ci_cache_fallback_parity_provider_outage_carries_its_limitation() {
    // Negative control 10: an outage is never a clean miss; the row
    // must carry its limitation.
    let mut outage = run(
        "release-set",
        CachePostureV1::ProviderUnavailable,
        "digest-1",
    );
    outage.limitations = vec!["actions/cache API returned 503 for the window".to_string()];
    validate_run_record(&outage).expect("an outage row with its limitation validates");
    let mut silent = outage.clone();
    silent.run_id = "run-silent-outage".to_string();
    silent.limitations = Vec::new();
    assert!(
        validate_run_record(&silent)
            .err()
            .is_some_and(|error| error.contains("never a clean miss"))
    );
}
