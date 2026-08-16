//! Characterization for cargo-intent provider discovery (#2601-A).

const FIXTURE: &str =
    include_str!("../../../tests/compat/fixtures/intent-provider-discovery-v1.toml");

#[test]
fn intent_provider_discovery_fixture_pins_contract() {
    for needle in [
        "cargo-allow.intent-provider-discovery.v1",
        "explicit_environment",
        "compatibility_config",
        "path_lookup",
        "CARGO_INTENT_BIN",
        ".allow/compatibility/intent-delegation.toml",
    ] {
        assert!(FIXTURE.contains(needle), "fixture missing {needle}");
    }
}
