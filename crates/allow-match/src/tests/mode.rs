use super::*;

#[test]
fn check_mode_parse_defaults_unknown_values_to_no_new() {
    assert_eq!(CheckMode::parse("audit"), CheckMode::Audit);
    assert_eq!(CheckMode::parse("strict"), CheckMode::Strict);
    assert_eq!(CheckMode::parse("release"), CheckMode::Release);
    assert_eq!(CheckMode::parse("no-new"), CheckMode::NoNew);
    assert_eq!(CheckMode::parse("unexpected"), CheckMode::NoNew);
}

#[test]
fn check_mode_failure_policy_matches_enforcement_levels() {
    assert!(!CheckMode::Audit.fails(MatchStatus::New));
    assert!(CheckMode::NoNew.fails(MatchStatus::New));
    assert!(!CheckMode::NoNew.fails(MatchStatus::Stale));
    assert!(CheckMode::NoNew.fails(MatchStatus::Expired));
    assert!(CheckMode::Strict.fails(MatchStatus::Stale));
    assert!(CheckMode::Release.fails(MatchStatus::BaselineDebt));
    assert!(!CheckMode::Strict.fails(MatchStatus::Matched));
    assert!(CheckMode::Strict.fails(MatchStatus::ReviewDue));
    assert!(CheckMode::Release.fails(MatchStatus::ReviewDue));
    assert!(!CheckMode::Audit.fails(MatchStatus::ReviewDue));
    assert!(!CheckMode::NoNew.fails(MatchStatus::ReviewDue));
}

#[test]
fn check_mode_parsing_and_failure_policy_are_covered() {
    assert_eq!(CheckMode::parse("audit"), CheckMode::Audit);
    assert_eq!(CheckMode::parse("strict"), CheckMode::Strict);
    assert_eq!(CheckMode::parse("release"), CheckMode::Release);
    assert_eq!(CheckMode::parse("unknown"), CheckMode::NoNew);

    assert!(!CheckMode::Audit.fails(MatchStatus::New));
    assert!(CheckMode::NoNew.fails(MatchStatus::New));
    assert!(CheckMode::Strict.fails(MatchStatus::Stale));
    assert!(CheckMode::Release.fails(MatchStatus::BaselineDebt));
    assert!(!CheckMode::Release.fails(MatchStatus::Matched));
}
