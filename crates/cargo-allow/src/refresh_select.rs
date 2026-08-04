use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowErrorKind, CargoAllowResult, Finding,
    LastSeen, MatchOutcome, MatchStatus,
};
use allow_match::classify_match;

pub(crate) fn select_location_drift_refresh(
    cfg: &AllowConfig,
    outcomes: &[MatchOutcome],
    findings: &[Finding],
    allow_id: &str,
) -> CargoAllowResult<(usize, usize, String)> {
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
        CargoAllowError::new(format!(
            "allow entry `{allow_id}` location drift outcome is missing finding index"
        ))
    })?;
    let finding = findings.get(finding_index).ok_or_else(|| {
        CargoAllowError::new(format!(
            "allow entry `{allow_id}` location drift outcome references missing finding index {finding_index}"
        ))
    })?;
    let entry = cfg.allow.get(entry_index).ok_or_else(|| {
        CargoAllowError::new(format!(
            "allow entry `{allow_id}` selection references missing policy index {entry_index}"
        ))
    })?;
    if classify_match(entry, finding).is_none() {
        return Err(CargoAllowError::new(format!(
            "allow entry `{allow_id}` selected finding no longer matches the entry selector"
        )));
    }
    if finding.span.is_none() {
        return Err(CargoAllowError::new(format!(
            "allow entry `{allow_id}` matched finding has no source span for last_seen refresh"
        )));
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
    entry.selector.line_hint = Some(span.line);
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
    fn apply_last_seen_refresh_updates_coordinates_without_touching_lifecycle() {
        let mut entry = drift_entry();
        let lifecycle = entry.lifecycle.clone();
        let finding = drift_finding();

        apply_last_seen_refresh(&mut entry, &finding);

        assert_eq!(
            entry
                .last_seen
                .as_ref()
                .map(|last_seen| (last_seen.line, last_seen.column)),
            Some((22, 4))
        );
        assert_eq!(entry.selector.line_hint, Some(22));
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
}
