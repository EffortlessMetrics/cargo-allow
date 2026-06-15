use allow_core::SimpleDate;
use allow_policy::BASELINE_DEBT_MAX_DAYS;

use super::parse_propose_expires_arg;

#[test]
fn parse_propose_expires_arg_call_presence_observer() {
    let today = SimpleDate::today_utc_approx();
    let valid = today.add_days(BASELINE_DEBT_MAX_DAYS).to_string();

    assert_eq!(parse_propose_expires_arg(&valid), Ok(valid));

    let yesterday = today.add_days(-1).to_string();
    assert_eq!(
        parse_propose_expires_arg(&yesterday),
        Err(format!(
            "generated baseline expiry `{yesterday}` must not be before today"
        ))
    );

    let too_far = today.add_days(BASELINE_DEBT_MAX_DAYS + 1).to_string();
    assert_eq!(
        parse_propose_expires_arg(&too_far),
        Err(format!(
            "generated baseline expiry `{too_far}` must be within {BASELINE_DEBT_MAX_DAYS} days"
        ))
    );
}
