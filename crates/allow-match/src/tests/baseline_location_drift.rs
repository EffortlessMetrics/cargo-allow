use super::*;

#[test]
fn baseline_debt_location_drift_is_mode_aware() -> Result<(), String> {
    let finding = finding_with_hash("fnv1a64:actual");
    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.classification = "baseline_debt".to_string();
    entry.last_seen = Some(LastSeen {
        line: 7,
        column: 12,
    });
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry);

    let no_new = evaluate(&cfg, std::slice::from_ref(&finding), CheckMode::NoNew);
    let no_new_outcome = no_new
        .iter()
        .find(|outcome| outcome.finding_index == Some(0))
        .ok_or_else(|| "no-new finding outcome should be present".to_string())?;
    assert_eq!(no_new_outcome.status, MatchStatus::LocationDrift);
    assert!(!CheckMode::NoNew.fails(no_new_outcome.status));
    assert!(no_new_outcome.message.contains("last_seen changed from 7:12 to 50:12"));

    for mode in [CheckMode::Strict, CheckMode::Release] {
        let outcomes = evaluate(&cfg, std::slice::from_ref(&finding), mode);
        let outcome = outcomes
            .iter()
            .find(|outcome| outcome.finding_index == Some(0))
            .ok_or_else(|| format!("{} finding outcome should be present", mode.as_str()))?;

        assert_eq!(outcome.status, MatchStatus::BaselineDebt);
        assert!(mode.fails(outcome.status));
        assert!(outcome.message.contains("is baseline debt"));
        assert!(outcome.message.contains("last_seen changed from 7:12 to 50:12"));
    }

    Ok(())
}

#[test]
fn baseline_debt_location_drift_remains_per_finding() -> Result<(), String> {
    let anchored = finding_with_hash("fnv1a64:actual");
    let mut moved = finding_with_hash("fnv1a64:actual");
    moved.span = Some(Span { line: 9, column: 3 });

    let mut entry = entry_with_hash("fnv1a64:actual");
    entry.classification = "baseline_debt".to_string();
    entry.last_seen = Some(LastSeen {
        line: 50,
        column: 12,
    });
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(entry);

    let outcomes = evaluate(&cfg, &[moved, anchored], CheckMode::Release);
    let moved_outcome = outcomes
        .iter()
        .find(|outcome| outcome.finding_index == Some(0))
        .ok_or_else(|| "moved finding outcome should be present".to_string())?;
    let anchored_outcome = outcomes
        .iter()
        .find(|outcome| outcome.finding_index == Some(1))
        .ok_or_else(|| "anchored finding outcome should be present".to_string())?;

    assert_eq!(moved_outcome.status, MatchStatus::BaselineDebt);
    assert_eq!(anchored_outcome.status, MatchStatus::BaselineDebt);
    assert!(moved_outcome.message.contains("is baseline debt"));
    assert!(moved_outcome.message.contains("last_seen changed from 50:12 to 9:3"));
    assert!(anchored_outcome.message.contains("is baseline debt"));
    assert!(!anchored_outcome.message.contains("last_seen changed"));

    Ok(())
}
