use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SimpleDate {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl SimpleDate {
    pub fn parse(input: &str) -> Option<Self> {
        let mut parts = input.trim().split('-');
        let year = parts.next()?.parse().ok()?;
        let month = parts.next()?.parse().ok()?;
        let day = parts.next()?.parse().ok()?;
        if parts.next().is_some() || !valid_ymd(year, month, day) {
            return None;
        }
        Some(Self { year, month, day })
    }

    pub fn days_until(self, other: Self) -> i64 {
        other.days_since_unix_epoch() - self.days_since_unix_epoch()
    }

    pub fn add_days(self, days: i64) -> Self {
        Self::from_days_since_unix_epoch(self.days_since_unix_epoch() + days)
    }

    pub(crate) fn days_since_unix_epoch(self) -> i64 {
        // Howard Hinnant's civil date algorithm. This keeps lifecycle validation
        // deterministic without adding a date dependency to the core crate.
        let mut year = i64::from(self.year);
        let month = i64::from(self.month);
        let day = i64::from(self.day);
        if month <= 2 {
            year -= 1;
        }
        let era = if year >= 0 { year } else { year - 399 } / 400;
        let year_of_era = year - era * 400;
        let month_prime = month + if month > 2 { -3 } else { 9 };
        let day_of_year = (153 * month_prime + 2) / 5 + day - 1;
        let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
        era * 146_097 + day_of_era - 719_468
    }

    pub(crate) fn from_days_since_unix_epoch(days: i64) -> Self {
        // Inverse of the civil date algorithm above.
        let z = days + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let day_of_era = z - era * 146_097;
        let year_of_era =
            (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let mut year = year_of_era + era * 400;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let month_prime = (5 * day_of_year + 2) / 153;
        let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
        let month = month_prime + if month_prime < 10 { 3 } else { -9 };
        if month <= 2 {
            year += 1;
        }
        Self {
            year: year as i32,
            month: month as u32,
            day: day as u32,
        }
    }

    pub fn today_utc_approx() -> Self {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        Self::from_days_since_unix_epoch((seconds / 86_400) as i64)
    }

    pub fn is_before_date_str(date: Option<&str>, today: Self) -> bool {
        date.and_then(Self::parse).is_some_and(|date| date < today)
    }

    pub fn is_due_date_str(date: Option<&str>, today: Self) -> bool {
        date.and_then(Self::parse).is_some_and(|date| date <= today)
    }
}

fn valid_ymd(year: i32, month: u32, day: u32) -> bool {
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap_year(year) => 29,
        2 => 28,
        _ => return false,
    };
    day > 0 && day <= max_day
}

fn leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

impl fmt::Display for SimpleDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}
