pub fn fixture_refresh_drift() -> u32 {
    // Padding lines so the expect attribute drifts beyond the
    // DRIFT_LINE_TOLERANCE (3) relative to last_seen (line 2).
    //
    //
    //
    #[expect(clippy::unwrap_used, reason = "policy:allow-0250: refresh receipt fixture")]
    let value = Some(1).unwrap();
    value
}
