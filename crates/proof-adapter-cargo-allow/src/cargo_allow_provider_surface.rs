//! cargo-allow provider surface marker (#2554).

pub struct CargoAllowProviderSurface;

impl CargoAllowProviderSurface {
    pub const MODULE_ID: &'static str = "proof-adapter-cargo-allow::cargo_allow_provider";
}
