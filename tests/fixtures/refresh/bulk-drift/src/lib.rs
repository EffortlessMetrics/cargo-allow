pub fn fixture_refresh_drift_first() -> u32 {
    // Padding lines so the expect attribute drifts beyond the
    // DRIFT_LINE_TOLERANCE (3) relative to last_seen (line 1).
    //
    //
    //
    #[expect(clippy::unwrap_used, reason = "policy:allow-11366: bulk refresh receipt fixture")]
    let value = Some(1).unwrap();
    value
}

pub fn fixture_refresh_drift_second() -> u32 {
    // Padding lines so the expect attribute drifts beyond the
    // DRIFT_LINE_TOLERANCE (3) relative to last_seen (line 1).
    //
    //
    //
    //
    #[expect(clippy::unwrap_used, reason = "policy:allow-11368: bulk refresh receipt fixture")]
    let value = Some(2).unwrap();
    value
}
