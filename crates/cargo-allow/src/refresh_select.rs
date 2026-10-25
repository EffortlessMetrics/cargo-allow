use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowErrorKind, CargoAllowResult, Finding,
    LastSeen, MatchOutcome, MatchStatus,
};
use allow_match::classify_match;

/// One selected location-drift refresh: `(allow entry index, finding index,
/// drift message)`.
pub(crate) type DriftRefreshSelection = (usize, usize, String);

pub(crate) fn select_location_drift_refresh(
    cfg: &AllowConfig,
    outcomes: &[MatchOutcome],
    findings: &[Finding],
    allow_id: &str,
) -> CargoAllowResult<DriftRefreshSelection> {
    let entry_index = cfg
        .allow
        .iter()
        .position(|entry| entry.id == allow_id)
        .ok_or_else(|| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::Usage,
                format!(
                    "allow entry id `{allow_id}` was not found in policy; \
                     run `cargo-allow list --format json` to see valid entry IDs"
                ),
            )
        })?;
    let outcome = outcomes
        .iter()
        .find(|outcome| outcome.allow_id.as_deref() == Some(allow_id))
        .ok_or_else(|| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::Usage,
                format!("allow entry `{allow_id}` did not produce a match outcome"),
            )
        })?;
    validate_drift_selection(cfg, findings, allow_id, entry_index, outcome)
}

/// Bulk `--all` selection (#4651): every location-drifted entry, in policy
/// order, driven through the same per-entry validation the `--allow-id`
/// path applies. The first outcome per entry wins, exactly like the
/// single-path `find`. An empty drift set fails closed with a typed usage
/// error instead of reporting success: the refresh render contract is
/// per-entry, so a bulk run with nothing to do has no receipt to emit, and
/// operators should consult `worklist --status location_drift` first.
pub(crate) fn select_all_location_drift_refresh(
    cfg: &AllowConfig,
    outcomes: &[MatchOutcome],
    findings: &[Finding],
) -> CargoAllowResult<Vec<DriftRefreshSelection>> {
    let mut selected = Vec::new();
    for (entry_index, entry) in cfg.allow.iter().enumerate() {
        let Some(outcome) = outcomes
            .iter()
            .find(|outcome| outcome.allow_id.as_deref() == Some(entry.id.as_str()))
        else {
            continue;
        };
        if outcome.status != MatchStatus::LocationDrift {
            continue;
        }
        selected.push(validate_drift_selection(
            cfg,
            findings,
            &entry.id,
            entry_index,
            outcome,
        )?);
    }
    if selected.is_empty() {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            "no allow entries have advisory location drift to refresh; \
             run `cargo-allow worklist --status location_drift --format json` to find refreshable entries",
        ));
    }
    Ok(selected)
}

fn validate_drift_selection(
    cfg: &AllowConfig,
    findings: &[Finding],
    allow_id: &str,
    entry_index: usize,
    outcome: &MatchOutcome,
) -> CargoAllowResult<DriftRefreshSelection> {
    if outcome.status != MatchStatus::LocationDrift {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            format!(
                "allow entry `{allow_id}` has status `{}`; refresh requires advisory location_drift; \
                 run `cargo-allow worklist --status location_drift --format json` to find refreshable entries",
                outcome.status.as_str()
            ),
        ));
    }
    let finding_index = outcome.finding_index.ok_or_else(|| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Internal,
            format!("allow entry `{allow_id}` location drift outcome is missing finding index"),
        )
    })?;
    let finding = findings.get(finding_index).ok_or_else(|| {
        CargoAllowError::with_kind(CargoAllowErrorKind::Internal, format!(
            "allow entry `{allow_id}` location drift outcome references missing finding index {finding_index}"
        ))
    })?;
    let entry = cfg.allow.get(entry_index).ok_or_else(|| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Internal,
            format!(
                "allow entry `{allow_id}` selection references missing policy index {entry_index}"
            ),
        )
    })?;
    if classify_match(entry, finding).is_none() {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Internal,
            format!(
                "allow entry `{allow_id}` selected finding no longer matches the entry selector"
            ),
        ));
    }
    if finding.span.is_none() {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Internal,
            format!(
                "allow entry `{allow_id}` matched finding has no source span for last_seen refresh"
            ),
        ));
    }
    Ok((entry_index, finding_index, outcome.message.clone()))
}

pub(crate) fn apply_last_seen_refresh(entry: &mut AllowEntry, finding: &Finding) {
    let Some(span) = &finding.span else {
        return;
    };
    entry.last_seen = Some(LastSeen {
        line: span.line,
        column: span.column,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{
        CargoAllowErrorKind, FindingKind, Lifecycle, Selector, Span, StructuralIdentity,
    };

    fn drift_entry() -> AllowEntry {
        AllowEntry {
            id: "allow-drift".to_string(),
            kind: FindingKind::LintException,
            family: Some("expect".to_string()),
            path: Some("src/lib.rs".into()),
            glob: None,
            owner: "lint".to_string(),
            classification: "reviewed_lint_exception".to_string(),
            reason: "fixture".to_string(),
            evidence: Vec::new(),
            links: Vec::new(),
            occurrence_limit: None,
            lifecycle: Lifecycle {
                created: Some("2026-05-09".to_string()),
                review_after: Some("2026-09-09".to_string()),
                expires: Some("2026-12-31".to_string()),
            },
            selector: Selector {
                ast_kind: Some("attribute".to_string()),
                line_hint: Some(14),
                ..Selector::default()
            },
            last_seen: Some(LastSeen {
                line: 14,
                column: 8,
            }),
        }
    }

    fn drift_finding() -> Finding {
        Finding {
            kind: FindingKind::LintException,
            family: Some("expect".to_string()),
            path: "src/lib.rs".into(),
            identity: StructuralIdentity::new("rust", "attribute"),
            message: "fixture".to_string(),
            ledger: None,
            span: Some(Span {
                line: 22,
                column: 4,
            }),
        }
    }

    #[test]
    fn apply_last_seen_refresh_updates_coordinates_without_touching_selector_or_lifecycle() {
        let mut entry = drift_entry();
        let lifecycle = entry.lifecycle.clone();
        let selector = entry.selector.clone();
        let finding = drift_finding();

        apply_last_seen_refresh(&mut entry, &finding);

        assert_eq!(
            entry
                .last_seen
                .as_ref()
                .map(|last_seen| (last_seen.line, last_seen.column)),
            Some((22, 4))
        );
        assert_eq!(entry.selector, selector);
        assert_eq!(entry.lifecycle, lifecycle);
    }

    #[test]
    fn select_location_drift_refresh_requires_location_drift_status() {
        let mut cfg = AllowConfig::empty();
        cfg.allow.push(drift_entry());
        let findings = vec![drift_finding()];
        let outcomes = vec![MatchOutcome {
            status: MatchStatus::Matched,
            allow_id: Some("allow-drift".to_string()),
            candidate_ids: Vec::new(),
            finding_index: Some(0),
            message: "matched".to_string(),
            score: 1,
        }];

        let err = select_location_drift_refresh(&cfg, &outcomes, &findings, "allow-drift")
            .expect_err("matched status should not refresh");

        assert!(err.to_string().contains("location_drift"));
        assert_eq!(err.kind(), CargoAllowErrorKind::Usage);
    }

    #[test]
    fn select_location_drift_refresh_rejects_unknown_allow_id_as_usage() {
        let err = select_location_drift_refresh(&AllowConfig::empty(), &[], &[], "missing-entry")
            .expect_err("unknown allow IDs should fail as usage errors");

        assert!(err.to_string().contains("missing-entry"));
        assert_eq!(err.kind(), CargoAllowErrorKind::Usage);
    }

    #[test]
    fn select_location_drift_refresh_rejects_missing_outcome_as_usage() {
        let mut cfg = AllowConfig::empty();
        cfg.allow.push(drift_entry());

        let err = select_location_drift_refresh(&cfg, &[], &[], "allow-drift")
            .expect_err("missing outcomes should fail as usage errors");

        assert!(err.to_string().contains("did not produce a match outcome"));
        assert_eq!(err.kind(), CargoAllowErrorKind::Usage);
    }

    #[test]
    fn select_location_drift_refresh_rejects_selector_mismatch() {
        let mut cfg = AllowConfig::empty();
        cfg.allow.push(drift_entry());
        let mut mismatched = drift_finding();
        mismatched.identity.ast_kind = "macro_call".to_string();
        let findings = vec![mismatched];
        let outcomes = vec![MatchOutcome {
            status: MatchStatus::LocationDrift,
            allow_id: Some("allow-drift".to_string()),
            candidate_ids: Vec::new(),
            finding_index: Some(0),
            message: "last_seen changed".to_string(),
            score: 1,
        }];

        let err = select_location_drift_refresh(&cfg, &outcomes, &findings, "allow-drift")
            .expect_err("selector mismatch should not refresh");

        assert!(err.to_string().contains("no longer matches"));
    }

    #[test]
    fn select_all_location_drift_refresh_returns_policy_order_and_skips_non_drift() {
        let mut cfg = AllowConfig::empty();
        let mut fresh = drift_entry();
        fresh.id = "allow-fresh".to_string();
        fresh.last_seen = Some(LastSeen {
            line: 22,
            column: 4,
        });
        cfg.allow.push(fresh);
        cfg.allow.push(drift_entry());
        let findings = vec![drift_finding()];
        // Outcome order deliberately differs from policy order: the fresh
        // entry is reported first, so selecting by policy position proves
        // the bulk selector walks the policy rather than the outcome list.
        let outcomes = vec![
            MatchOutcome {
                status: MatchStatus::Matched,
                allow_id: Some("allow-fresh".to_string()),
                candidate_ids: Vec::new(),
                finding_index: Some(0),
                message: "matched".to_string(),
                score: 1,
            },
            MatchOutcome {
                status: MatchStatus::LocationDrift,
                allow_id: Some("allow-drift".to_string()),
                candidate_ids: Vec::new(),
                finding_index: Some(0),
                message: "last_seen changed".to_string(),
                score: 1,
            },
        ];

        let selected = select_all_location_drift_refresh(&cfg, &outcomes, &findings)
            .unwrap_or_else(|err| std::panic::panic_any(format!("bulk selection: {err}")));

        assert_eq!(selected.len(), 1, "the fresh entry must not be selected");
        assert_eq!(selected[0].0, 1, "selections use policy positions");
        assert_eq!(selected[0].1, 0);
        assert_eq!(selected[0].2, "last_seen changed");
    }

    #[test]
    fn select_all_location_drift_refresh_fails_closed_when_no_entry_is_drifted() {
        let mut cfg = AllowConfig::empty();
        cfg.allow.push(drift_entry());
        let findings = vec![drift_finding()];
        let outcomes = vec![MatchOutcome {
            status: MatchStatus::Matched,
            allow_id: Some("allow-drift".to_string()),
            candidate_ids: Vec::new(),
            finding_index: Some(0),
            message: "matched".to_string(),
            score: 1,
        }];

        let err = select_all_location_drift_refresh(&cfg, &outcomes, &findings)
            .expect_err("an empty drift set should fail closed");

        assert_eq!(err.kind(), CargoAllowErrorKind::Usage);
        assert!(
            err.to_string()
                .contains("no allow entries have advisory location drift")
        );
    }

    #[test]
    fn select_all_location_drift_refresh_applies_per_entry_invariants() {
        let mut cfg = AllowConfig::empty();
        cfg.allow.push(drift_entry());
        let findings = vec![drift_finding()];
        let outcomes = vec![MatchOutcome {
            status: MatchStatus::LocationDrift,
            allow_id: Some("allow-drift".to_string()),
            candidate_ids: Vec::new(),
            finding_index: None,
            message: "last_seen changed".to_string(),
            score: 1,
        }];

        let err = select_all_location_drift_refresh(&cfg, &outcomes, &findings)
            .expect_err("a drift outcome without a finding index should fail closed");

        assert_eq!(err.kind(), CargoAllowErrorKind::Internal);
        assert!(err.to_string().contains("missing finding index"));
    }
}
