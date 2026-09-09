//! Dependency graph evidence adjacency law tests (#3920 PR C): the
//! enrichment binds evidence by exact identity, never upgrades a
//! movement to clean, retains advisory residue as a decision, stales
//! on identity movement, and rejects free-text commentary.

use crate::{
    DEPENDENCY_GRAPH_EVIDENCE_MAX_RECORDS, DEPENDENCY_GRAPH_EVIDENCE_MAX_ROWS,
    DependencyEvidenceAuthorityV1, DependencyEvidenceBundleV1, DependencyEvidenceDispositionV1,
    DependencyEvidenceRecordV1, DependencyGraphDeltaIdentityV1, DependencyGraphDeltaKindV1,
    DependencyGraphDeltaReceiptV1, DependencyGraphDeltaRowV1, DependencyGraphEvidenceReceiptV1,
    DependencyGraphEvidenceResultV1, attach_dependency_graph_evidence,
};

fn delta_identity() -> DependencyGraphDeltaIdentityV1 {
    DependencyGraphDeltaIdentityV1 {
        base_commit: "aaa111".to_string(),
        head_commit: "bbb222".to_string(),
        base_manifest_set_digest: "sha256:v1:base".to_string(),
        head_manifest_set_digest: "sha256:v1:head".to_string(),
        base_lock_digest: "sha256:v1:base-lock".to_string(),
        head_lock_digest: "sha256:v1:head-lock".to_string(),
        product: "cargo-allow".to_string(),
        target: "x86_64-unknown-linux-gnu".to_string(),
    }
}

fn row(
    kind: DependencyGraphDeltaKindV1,
    name: &str,
    head_version: &str,
) -> DependencyGraphDeltaRowV1 {
    DependencyGraphDeltaRowV1 {
        kind,
        class: crate::DependencyClassV1::Normal,
        package_name: name.to_string(),
        base_version: if head_version.is_empty() {
            "1.0.0".to_string()
        } else {
            String::new()
        },
        head_version: head_version.to_string(),
        base_requirement: String::new(),
        head_requirement: String::new(),
        base_source: String::new(),
        head_source: String::new(),
        base_checksum: String::new(),
        head_checksum: String::new(),
    }
}

fn receipt(rows: Vec<DependencyGraphDeltaRowV1>) -> DependencyGraphDeltaReceiptV1 {
    DependencyGraphDeltaReceiptV1 {
        schema_id: crate::DEPENDENCY_GRAPH_DELTA_SCHEMA_ID.to_string(),
        schema_version: crate::DEPENDENCY_GRAPH_DELTA_SCHEMA_VERSION,
        identity: delta_identity(),
        rows,
        complete: true,
        limitations: vec![],
        claim_boundary: "bounded".to_string(),
    }
}

fn bundle(records: Vec<DependencyEvidenceRecordV1>) -> DependencyEvidenceBundleV1 {
    DependencyEvidenceBundleV1 {
        authorities_in_scope: vec![
            DependencyEvidenceAuthorityV1::MinimumVersion,
            DependencyEvidenceAuthorityV1::CargoDeny,
        ],
        base_commit: "aaa111".to_string(),
        head_commit: "bbb222".to_string(),
        product: "cargo-allow".to_string(),
        target: "x86_64-unknown-linux-gnu".to_string(),
        records,
    }
}

fn record(
    authority: DependencyEvidenceAuthorityV1,
    package: &str,
    version: &str,
    finding: bool,
    advisory: bool,
) -> DependencyEvidenceRecordV1 {
    DependencyEvidenceRecordV1 {
        authority,
        package_name: package.to_string(),
        version: version.to_string(),
        reference: format!("{}:opaque-ref", authority.reference_scheme()),
        finding,
        advisory,
    }
}

#[test]
fn dependency_graph_evidence_cargo_deny_pass_never_erases_a_downgrade() {
    // A current cargo-deny pass on a downgraded package leaves the
    // row a decision: the downgrade remains independently visible and
    // the receipt result is never clean (negative control 11).
    let delta = receipt(vec![row(
        DependencyGraphDeltaKindV1::PackageDowngraded,
        "toml",
        "0.8.1",
    )]);
    let bundle = bundle(vec![record(
        DependencyEvidenceAuthorityV1::CargoDeny,
        "toml",
        "0.8.1",
        false,
        false,
    )]);
    let enriched = attach_dependency_graph_evidence(&delta, &bundle).expect("enrichment succeeds");
    let toml_row = &enriched.rows[0];
    assert_eq!(toml_row.kind, DependencyGraphDeltaKindV1::PackageDowngraded);
    assert_eq!(
        toml_row.disposition,
        DependencyEvidenceDispositionV1::EvidenceMissing,
        "minimum_version is in scope with no record: the row is not current"
    );
    assert_eq!(
        enriched.result,
        DependencyGraphEvidenceResultV1::DecisionRequired
    );
}

#[test]
fn dependency_graph_evidence_policy_finding_dominates_current_coverage() {
    // One authority flagging the row makes it a policy finding even
    // when every other authority is current.
    let delta = receipt(vec![row(
        DependencyGraphDeltaKindV1::PackageUpgraded,
        "serde",
        "1.0.228",
    )]);
    let bundle = bundle(vec![
        record(
            DependencyEvidenceAuthorityV1::CargoDeny,
            "serde",
            "1.0.228",
            true,
            false,
        ),
        record(
            DependencyEvidenceAuthorityV1::MinimumVersion,
            "serde",
            "1.0.228",
            false,
            false,
        ),
    ]);
    let enriched = attach_dependency_graph_evidence(&delta, &bundle).expect("enrichment succeeds");
    assert_eq!(
        enriched.rows[0].disposition,
        DependencyEvidenceDispositionV1::PolicyFinding
    );
    assert_eq!(
        enriched.result,
        DependencyGraphEvidenceResultV1::DecisionRequired
    );
}

#[test]
fn dependency_graph_evidence_advisory_residue_stays_a_decision() {
    // Advisory evidence is retained as NeedsDecision and never lets
    // the receipt report clean, even with every row otherwise current.
    let delta = receipt(vec![row(
        DependencyGraphDeltaKindV1::PackageUpgraded,
        "serde",
        "1.0.228",
    )]);
    let mut bundle = bundle(vec![
        record(
            DependencyEvidenceAuthorityV1::CargoDeny,
            "serde",
            "1.0.228",
            false,
            true,
        ),
        record(
            DependencyEvidenceAuthorityV1::MinimumVersion,
            "serde",
            "1.0.228",
            false,
            false,
        ),
    ]);
    let enriched = attach_dependency_graph_evidence(&delta, &bundle).expect("enrichment succeeds");
    assert_eq!(
        enriched.rows[0].disposition,
        DependencyEvidenceDispositionV1::NeedsDecision
    );
    assert_eq!(
        enriched.result,
        DependencyGraphEvidenceResultV1::DecisionRequired
    );
    assert!(
        enriched.rows[0]
            .attachments
            .iter()
            .any(|attachment| attachment.advisory)
    );
    let human = crate::render_dependency_graph_evidence(
        &enriched,
        crate::DependencyGraphEvidenceFormat::Human,
    )
    .expect("advisory human view renders");
    assert!(human.contains("(advisory)"), "advisory residue is labeled");

    // With the same record resolved as current evidence the same
    // shape is fully current.
    bundle.records[0].advisory = false;
    let enriched = attach_dependency_graph_evidence(&delta, &bundle).expect("enrichment succeeds");
    assert_eq!(
        enriched.result,
        DependencyGraphEvidenceResultV1::EvidenceCurrent
    );
}

#[test]
fn dependency_graph_evidence_identity_movement_fails_closed() {
    // A bundle bound to a different base/head/product/target is stale
    // for this delta: it is malformed input for the enrichment, never
    // a receipt (negative control 10).
    let delta = receipt(vec![row(
        DependencyGraphDeltaKindV1::PackageUpgraded,
        "serde",
        "1.0.228",
    )]);
    let mut stale = bundle(vec![]);
    stale.head_commit = "ccc333".to_string();
    assert!(
        attach_dependency_graph_evidence(&delta, &stale).is_err(),
        "identity movement stales the bundle"
    );
    let mut wrong_product = bundle(vec![]);
    wrong_product.product = "cargo-intent".to_string();
    assert!(
        attach_dependency_graph_evidence(&delta, &wrong_product).is_err(),
        "a wrong-product bundle never enriches this delta"
    );
}

#[test]
fn dependency_graph_evidence_rejects_free_text_references() {
    // Model or bot commentary cannot enter: references with
    // whitespace or a foreign scheme fail closed as malformed input.
    let delta = receipt(vec![row(
        DependencyGraphDeltaKindV1::PackageUpgraded,
        "serde",
        "1.0.228",
    )]);
    let mut comment = bundle(vec![record(
        DependencyEvidenceAuthorityV1::CargoDeny,
        "serde",
        "1.0.228",
        false,
        false,
    )]);
    comment.records[0].reference =
        "_📐 Maintainability | _🟠 Major_ | the dependency looks fine to me".to_string();
    assert!(
        attach_dependency_graph_evidence(&delta, &comment).is_err(),
        "commentary is never graph authority"
    );
    let mut wrong_scheme = bundle(vec![record(
        DependencyEvidenceAuthorityV1::CargoDeny,
        "serde",
        "1.0.228",
        false,
        false,
    )]);
    wrong_scheme.records[0].reference = "issue:12345".to_string();
    assert!(
        attach_dependency_graph_evidence(&delta, &wrong_scheme).is_err(),
        "the reference scheme must match the authority"
    );
}

#[test]
fn dependency_graph_evidence_binds_exact_identity_only() {
    // A record for a different resolved version of the same package
    // does not bind: the row's evidence is missing, not current.
    let delta = receipt(vec![row(
        DependencyGraphDeltaKindV1::PackageUpgraded,
        "serde",
        "1.0.228",
    )]);
    let stale_version = bundle(vec![record(
        DependencyEvidenceAuthorityV1::CargoDeny,
        "serde",
        "1.0.200",
        false,
        false,
    )]);
    let enriched =
        attach_dependency_graph_evidence(&delta, &stale_version).expect("enrichment succeeds");
    assert_eq!(
        enriched.rows[0].disposition,
        DependencyEvidenceDispositionV1::EvidenceMissing
    );
}

#[test]
fn dependency_graph_evidence_removed_rows_bind_the_base_identity() {
    // A removed package's evidence binds the base-side version: the
    // package left the graph, so there is no head version to match.
    let delta = receipt(vec![row(
        DependencyGraphDeltaKindV1::PackageRemoved,
        "ghost",
        "",
    )]);
    let enriched = attach_dependency_graph_evidence(
        &delta,
        &bundle(vec![record(
            DependencyEvidenceAuthorityV1::CargoDeny,
            "ghost",
            "1.0.0",
            false,
            false,
        )]),
    )
    .expect("enrichment succeeds");
    assert!(
        enriched.rows[0]
            .attachments
            .iter()
            .any(|attachment| attachment.disposition
                == DependencyEvidenceDispositionV1::EvidenceCurrent),
        "the base-identity record binds the removed row"
    );
}

#[test]
fn dependency_graph_evidence_unmoved_rows_stay_observed_without_authority() {
    // A no-semantic-change row with no authority record remains an
    // observed (textual) movement, never a fabricated clean state.
    let delta = receipt(vec![row(
        DependencyGraphDeltaKindV1::NoSemanticGraphChange,
        "flat",
        "1.0",
    )]);
    let enriched =
        attach_dependency_graph_evidence(&delta, &bundle(vec![])).expect("enrichment succeeds");
    assert_eq!(
        enriched.rows[0].disposition,
        DependencyEvidenceDispositionV1::ObservedMovement
    );
    assert_eq!(
        enriched.result,
        DependencyGraphEvidenceResultV1::DecisionRequired
    );
    let human = crate::render_dependency_graph_evidence(
        &enriched,
        crate::DependencyGraphEvidenceFormat::Human,
    )
    .expect("unmoved human view renders");
    let em_dash: char = '\u{2014}';
    assert!(
        human.contains(em_dash),
        "a row with no attachments renders the empty placeholder"
    );
}

#[test]
fn dependency_graph_evidence_empty_denominator_is_not_clean() {
    // A zero-row complete delta has an empty denominator: the result
    // is a decision, never a clean pass (zero-denominator law).
    let delta = receipt(vec![]);
    let enriched =
        attach_dependency_graph_evidence(&delta, &bundle(vec![])).expect("enrichment succeeds");
    assert_eq!(
        enriched.result,
        DependencyGraphEvidenceResultV1::DecisionRequired
    );
}

#[test]
fn dependency_graph_evidence_incomplete_delta_never_reports_clean() {
    let mut delta = receipt(vec![row(
        DependencyGraphDeltaKindV1::PackageUpgraded,
        "serde",
        "1.0.228",
    )]);
    delta.complete = false;
    let enriched =
        attach_dependency_graph_evidence(&delta, &bundle(vec![])).expect("enrichment succeeds");
    assert!(!enriched.complete);
    assert_eq!(
        enriched.result,
        DependencyGraphEvidenceResultV1::DecisionRequired
    );
}

#[test]
fn dependency_graph_evidence_is_deterministic_and_round_trips() {
    let delta = receipt(vec![
        row(
            DependencyGraphDeltaKindV1::PackageUpgraded,
            "serde",
            "1.0.228",
        ),
        row(DependencyGraphDeltaKindV1::PackageAdded, "toml", "1.1.4"),
    ]);
    let evidence = bundle(vec![
        record(
            DependencyEvidenceAuthorityV1::CargoDeny,
            "serde",
            "1.0.228",
            false,
            false,
        ),
        record(
            DependencyEvidenceAuthorityV1::MinimumVersion,
            "toml",
            "1.1.4",
            false,
            false,
        ),
    ]);
    let first = attach_dependency_graph_evidence(&delta, &evidence).expect("first succeeds");
    let second = attach_dependency_graph_evidence(&delta, &evidence).expect("second succeeds");
    assert_eq!(first, second);
    let json =
        crate::render_dependency_graph_evidence(&first, crate::DependencyGraphEvidenceFormat::Json)
            .expect("json renders");
    let parsed: DependencyGraphEvidenceReceiptV1 =
        serde_json::from_str(&json).expect("json round-trips");
    assert_eq!(parsed, first);
    let human = crate::render_dependency_graph_evidence(
        &first,
        crate::DependencyGraphEvidenceFormat::Human,
    )
    .expect("human renders");
    assert!(human.contains("`serde`"));
    assert!(human.contains("evidence_current") || human.contains("decision_required"));
}

#[test]
fn dependency_graph_evidence_vocabulary_and_bounds_are_total() {
    // Every authority and disposition names itself and declares the
    // typed reference scheme its records must carry; oversize inputs
    // fail closed on the bound instead of widening the denominator.
    for (authority, name, scheme) in [
        (
            DependencyEvidenceAuthorityV1::MinimumVersion,
            "minimum_version",
            "receipt",
        ),
        (
            DependencyEvidenceAuthorityV1::DependencyFeature,
            "dependency_feature",
            "policy",
        ),
        (
            DependencyEvidenceAuthorityV1::CargoDeny,
            "cargo_deny",
            "deny",
        ),
        (
            DependencyEvidenceAuthorityV1::PackageCandidate,
            "package_candidate",
            "receipt",
        ),
        (
            DependencyEvidenceAuthorityV1::SupportMatrix,
            "support_matrix",
            "policy",
        ),
    ] {
        assert_eq!(authority.as_str(), name);
        assert_eq!(authority.reference_scheme(), scheme);
    }
    for (disposition, name) in [
        (
            DependencyEvidenceDispositionV1::ObservedMovement,
            "observed_movement",
        ),
        (
            DependencyEvidenceDispositionV1::PolicyFinding,
            "policy_finding",
        ),
        (
            DependencyEvidenceDispositionV1::EvidenceCurrent,
            "evidence_current",
        ),
        (
            DependencyEvidenceDispositionV1::EvidenceMissing,
            "evidence_missing",
        ),
        (
            DependencyEvidenceDispositionV1::NeedsDecision,
            "needs_decision",
        ),
    ] {
        assert_eq!(disposition.as_str(), name);
    }
    assert_eq!(
        DependencyGraphEvidenceResultV1::DecisionRequired.as_str(),
        "decision_required"
    );
    assert_eq!(
        DependencyGraphEvidenceResultV1::EvidenceCurrent.as_str(),
        "evidence_current"
    );

    let oversized = receipt(
        (0..=DEPENDENCY_GRAPH_EVIDENCE_MAX_ROWS)
            .map(|index| {
                row(
                    DependencyGraphDeltaKindV1::PackageUpgraded,
                    &format!("pkg-{index}"),
                    "1.0.0",
                )
            })
            .collect(),
    );
    assert!(
        attach_dependency_graph_evidence(&oversized, &bundle(vec![])).is_err(),
        "a delta beyond the row bound fails closed"
    );

    let oversized_bundle = bundle(
        (0..=DEPENDENCY_GRAPH_EVIDENCE_MAX_RECORDS)
            .map(|index| {
                record(
                    DependencyEvidenceAuthorityV1::CargoDeny,
                    &format!("pkg-{index}"),
                    "1.0.228",
                    false,
                    false,
                )
            })
            .collect(),
    );
    let small = receipt(vec![row(
        DependencyGraphDeltaKindV1::PackageUpgraded,
        "serde",
        "1.0.228",
    )]);
    assert!(
        attach_dependency_graph_evidence(&small, &oversized_bundle).is_err(),
        "a bundle beyond the record bound fails closed"
    );
}
