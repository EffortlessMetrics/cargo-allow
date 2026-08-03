use super::*;

fn facts() -> AdoptionFacts {
    AdoptionFacts {
        tool_version: "0.2.0".into(),
        repository_identity: "repo:example".into(),
        selected_root: "H:/checkout".into(),
        channel: "crates.io".into(),
        executable_identity: "cargo-allow".into(),
        inventory: AdoptionInventoryFacts {
            mode: InventoryMode::GitTracked,
            completeness: InventoryCompleteness::Complete,
            limitations: Vec::new(),
        },
        policy: AdoptionPolicyFacts {
            state: PolicyState::Valid,
            path: Some("H:/checkout/policy/allow.toml".into()),
            schema_version: Some("0.1".into()),
            digest: Some("sha256:policy".into()),
            total_findings: 0,
            new_unreceipted_findings: 0,
            stale_entries: 0,
            location_drift_entries: 0,
            broken_evidence_entries: 0,
            review_due_entries: 0,
            expired_entries: 0,
            occurrence_headroom_entries: 0,
            mirror_divergence: false,
        },
        policy_config_diagnostic: None,
        unsupported_repository_state: false,
        instrument_failure: None,
        strict_gate_requested: false,
        ci_guidance_completed: true,
    }
}

#[test]
fn core_adoption_plan_table_covers_first_hour_and_fail_closed_states() -> Result<(), String> {
    let mut clean_no_policy = facts();
    clean_no_policy.policy.state = PolicyState::Absent;
    clean_no_policy.policy.path = None;

    let mut findings_no_policy = clean_no_policy.clone();
    findings_no_policy.policy.total_findings = 2;

    let mut strict_clean = clean_no_policy.clone();
    strict_clean.strict_gate_requested = true;

    let mut healthy_new = facts();
    healthy_new.policy.new_unreceipted_findings = 1;

    let mut stale = facts();
    stale.policy.stale_entries = 1;

    let mut drift = facts();
    drift.policy.location_drift_entries = 1;

    let mut broken_evidence = facts();
    broken_evidence.policy.broken_evidence_entries = 1;

    let mut review_due = facts();
    review_due.policy.review_due_entries = 1;

    let mut expired = facts();
    expired.policy.expired_entries = 1;

    let mut mirror = facts();
    mirror.policy.mirror_divergence = true;

    let mut partial = facts();
    partial.inventory.completeness = InventoryCompleteness::Partial;
    partial.inventory.limitations.push("skipped path".into());

    let mut invalid = facts();
    invalid.policy.state = PolicyState::Invalid;
    invalid.policy_config_diagnostic = Some("invalid schema".into());

    let mut unsupported = facts();
    unsupported.unsupported_repository_state = true;

    let mut instrument_failure = facts();
    instrument_failure.instrument_failure = Some("git unavailable".into());

    let cases = [
        (
            "clean_no_policy",
            clean_no_policy,
            BootstrapDisposition::CleanNoPolicy,
            AdoptionActionKind::ContinueAdvisoryAudit,
            WritePosture::ReadOnly,
        ),
        (
            "strict_clean_no_policy",
            strict_clean,
            BootstrapDisposition::CleanNoPolicy,
            AdoptionActionKind::PreviewInit,
            WritePosture::PreviewOnly,
        ),
        (
            "findings_no_policy",
            findings_no_policy,
            BootstrapDisposition::FindingsNoPolicy,
            AdoptionActionKind::PreviewPropose,
            WritePosture::PreviewOnly,
        ),
        (
            "healthy_policy",
            facts(),
            BootstrapDisposition::ExistingPolicyHealthy,
            AdoptionActionKind::RunNoNewCheck,
            WritePosture::ReadOnly,
        ),
        (
            "healthy_policy_new_finding",
            healthy_new,
            BootstrapDisposition::ExistingPolicyHasNewFindings,
            AdoptionActionKind::InspectNewFinding,
            WritePosture::ReadOnly,
        ),
        (
            "stale_entry",
            stale,
            BootstrapDisposition::ExistingPolicyNeedsRepair,
            AdoptionActionKind::ApplyStaleSafeFindingPlan,
            WritePosture::MayWrite,
        ),
        (
            "location_drift",
            drift,
            BootstrapDisposition::ExistingPolicyNeedsRepair,
            AdoptionActionKind::PreviewRefresh,
            WritePosture::PreviewOnly,
        ),
        (
            "broken_evidence",
            broken_evidence,
            BootstrapDisposition::ExistingPolicyNeedsRepair,
            AdoptionActionKind::RepairEvidence,
            WritePosture::ReadOnly,
        ),
        (
            "review_due",
            review_due,
            BootstrapDisposition::ExistingPolicyNeedsRepair,
            AdoptionActionKind::InspectAllow,
            WritePosture::ReadOnly,
        ),
        (
            "expired_entry",
            expired,
            BootstrapDisposition::ExistingPolicyNeedsRepair,
            AdoptionActionKind::InspectAllow,
            WritePosture::ReadOnly,
        ),
        (
            "mirror_divergence",
            mirror,
            BootstrapDisposition::ExistingPolicyNeedsRepair,
            AdoptionActionKind::ReconcileMirror,
            WritePosture::ReadOnly,
        ),
        (
            "partial_inventory",
            partial,
            BootstrapDisposition::PartialInventory,
            AdoptionActionKind::DiagnoseInventory,
            WritePosture::ReadOnly,
        ),
        (
            "invalid_policy",
            invalid,
            BootstrapDisposition::InvalidPolicy,
            AdoptionActionKind::RepairPolicy,
            WritePosture::ReadOnly,
        ),
        (
            "unsupported_repository",
            unsupported,
            BootstrapDisposition::UnsupportedRepositoryState,
            AdoptionActionKind::DiagnoseInventory,
            WritePosture::ReadOnly,
        ),
        (
            "instrument_failure",
            instrument_failure,
            BootstrapDisposition::InstrumentFailure,
            AdoptionActionKind::DiagnoseInventory,
            WritePosture::ReadOnly,
        ),
    ];

    for (name, input, disposition, action, posture) in cases {
        let plan = recommend_core_adoption_plan(&input);
        if plan.bootstrap_disposition != disposition {
            return Err(format!("{name}: unexpected disposition"));
        }
        if plan.primary_action.kind != action {
            return Err(format!("{name}: unexpected primary action"));
        }
        if plan.primary_action.write_posture != posture {
            return Err(format!("{name}: unexpected write posture"));
        }
        if plan.primary_action.kind.as_str().is_empty() {
            return Err(format!("{name}: action vocabulary was empty"));
        }
        if plan.claim_boundary.is_empty() || plan.explicit_non_effects.len() != 3 {
            return Err(format!("{name}: claim boundary was incomplete"));
        }
    }
    Ok(())
}

#[test]
fn core_adoption_plan_orders_repair_follow_ups_deterministically() -> Result<(), String> {
    let mut input = facts();
    input.ci_guidance_completed = false;
    input.policy.stale_entries = 1;
    input.policy.location_drift_entries = 1;
    input.policy.broken_evidence_entries = 1;
    input.policy.review_due_entries = 1;
    input.policy.expired_entries = 1;
    input.policy.occurrence_headroom_entries = 1;
    input.policy.mirror_divergence = true;
    let plan = recommend_core_adoption_plan(&input);
    let kinds: Vec<AdoptionActionKind> = plan
        .follow_up_actions
        .iter()
        .map(|action| action.kind)
        .collect();
    let expected = vec![
        AdoptionActionKind::PreviewRefresh,
        AdoptionActionKind::RepairEvidence,
        AdoptionActionKind::InspectAllow,
        AdoptionActionKind::PreviewPrune,
        AdoptionActionKind::ReconcileMirror,
        AdoptionActionKind::RunNoNewCheck,
        AdoptionActionKind::ConfigureCi,
    ];
    if kinds != expected {
        return Err(format!("unexpected follow-up order: {kinds:?}"));
    }
    if plan.may_write_paths != vec!["policy/allow.toml".to_string()] {
        return Err("stale plan did not expose the normalized policy write path".into());
    }
    Ok(())
}

#[test]
fn core_adoption_plan_rejects_unknown_inputs() -> Result<(), String> {
    let mut inventory_unknown = facts();
    inventory_unknown.inventory.mode = InventoryMode::Unknown;
    let plan = recommend_core_adoption_plan(&inventory_unknown);
    if plan.bootstrap_disposition != BootstrapDisposition::UnsupportedRepositoryState
        || plan.primary_action.kind != AdoptionActionKind::DiagnoseInventory
    {
        return Err("unknown inventory did not fail closed".into());
    }

    let mut policy_unknown = facts();
    policy_unknown.policy.state = PolicyState::Unknown;
    let plan = recommend_core_adoption_plan(&policy_unknown);
    if plan.bootstrap_disposition != BootstrapDisposition::InvalidPolicy
        || plan.primary_action.kind != AdoptionActionKind::RepairPolicy
    {
        return Err("unknown policy did not fail closed".into());
    }
    Ok(())
}

#[test]
fn core_adoption_plan_is_independent_of_absolute_checkout_path() -> Result<(), String> {
    let first = facts();
    let mut second = first.clone();
    second.selected_root = "D:/another-checkout".into();
    second.policy.path = Some("D:/another-checkout/policy/allow.toml".into());
    let first_plan = recommend_core_adoption_plan(&first);
    let second_plan = recommend_core_adoption_plan(&second);
    if first_plan != second_plan {
        return Err("absolute checkout path changed the semantic plan".into());
    }
    let encoded = serde_json::to_vec(&first_plan).map_err(|error| error.to_string())?;
    let text = String::from_utf8(encoded).map_err(|error| error.to_string())?;
    if text.contains("H:/checkout") || text.contains("D:/another-checkout") {
        return Err("semantic plan leaked an absolute checkout path".into());
    }
    Ok(())
}

#[test]
fn core_adoption_plan_schema_is_versioned_and_non_mutating() -> Result<(), String> {
    let plan = recommend_core_adoption_plan(&facts());
    if plan.schema_id != CORE_ADOPTION_PLAN_SCHEMA_ID || plan.schema_version != 1 {
        return Err("unexpected adoption plan schema identity".into());
    }
    if plan.explicit_non_effects.iter().any(|item| item.is_empty()) {
        return Err("empty non-effect marker".into());
    }
    Ok(())
}
