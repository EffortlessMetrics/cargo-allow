//! UTC date acquisition for workflow exception expiry, without external tools.

use std::time::{SystemTime, UNIX_EPOCH};

use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult, SimpleDate};

pub(super) fn today_utc() -> CargoAllowResult<String> {
    date_at(SystemTime::now())
}

fn date_at(now: SystemTime) -> CargoAllowResult<String> {
    let elapsed = now.duration_since(UNIX_EPOCH).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InstrumentFailure,
            "cannot determine UTC date for exception expiry: clock precedes Unix epoch",
        )
        .with_cause(&error)
    })?;
    // Even u64::MAX seconds / 86_400 fits in i64; conversion cannot fail.
    let days = (elapsed.as_secs() / 86_400) as i64;
    let epoch = SimpleDate {
        year: 1970,
        month: 1,
        day: 1,
    };
    let last_supported_date = SimpleDate {
        year: 9999,
        month: 12,
        day: 31,
    };
    // Bound the calendar conversion before its integer year representation.
    if days > epoch.days_until(last_supported_date) {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InstrumentFailure,
            "UTC date for exception expiry exceeds year 9999",
        ));
    }
    Ok(SimpleDate::from_days_since_unix_epoch(days).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn workflow_utc_date_preserves_utc_day_and_leap_boundaries()
    -> Result<(), Box<dyn std::error::Error>> {
        for (seconds, expected) in [
            (0, "1970-01-01"),
            (86_399, "1970-01-01"),
            (86_400, "1970-01-02"),
            (1_709_164_800, "2024-02-29"),
            (1_709_251_199, "2024-02-29"),
            (1_709_251_200, "2024-03-01"),
            (253_402_300_799, "9999-12-31"),
        ] {
            let instant = UNIX_EPOCH
                .checked_add(Duration::from_secs(seconds))
                .ok_or("fixture timestamp is unsupported")?;
            let actual = date_at(instant)?;
            if actual != expected {
                return Err(format!("at {seconds}: expected {expected}, got {actual}").into());
            }
        }
        Ok(())
    }

    #[test]
    fn workflow_utc_date_rejects_unsupported_clocks() -> Result<(), Box<dyn std::error::Error>> {
        let before_epoch = UNIX_EPOCH
            .checked_sub(Duration::from_secs(1))
            .ok_or("pre-epoch fixture timestamp is unsupported")?;
        let after_supported_year = UNIX_EPOCH
            .checked_add(Duration::from_secs(253_402_300_800))
            .ok_or("out-of-range fixture timestamp is unsupported")?;
        for instant in [before_epoch, after_supported_year] {
            match date_at(instant) {
                Err(error) if error.kind() == CargoAllowErrorKind::InstrumentFailure => {}
                other => {
                    return Err(format!("expected clock instrument failure, got {other:?}").into());
                }
            }
        }
        Ok(())
    }
}
