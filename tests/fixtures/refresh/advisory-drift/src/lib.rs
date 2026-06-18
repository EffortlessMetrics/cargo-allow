pub fn fixture_refresh_drift() -> u32 {
    #[expect(clippy::unwrap_used, reason = "policy:fixture-refresh-drift: refresh receipt fixture")]
    let value = Some(1).unwrap();
    value
}
