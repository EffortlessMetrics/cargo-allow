//! Offline characterization for ThreeProductSimplificationAuditV1 (#2208).

const INVENTORY: &str = include_str!("../../../policy/three-product-simplification.toml");

#[test]
fn simplification_inventory_lists_all_classifications() {
    for label in [
        "MvpRequired",
        "MvpSimplify",
        "PostMvpValidatedNeed",
        "PostMvpUnproven",
        "DeferUntilSecondAdopter",
        "RejectOrReplace",
    ] {
        assert!(
            INVENTORY.contains(label),
            "inventory missing classification {label}"
        );
    }
}

#[test]
fn simplification_inventory_records_io_shim_removal() {
    assert!(INVENTORY.contains("cargo-allow-io-shim"));
    assert!(INVENTORY.contains("action = \"removed\""));
}

#[test]
fn cargo_allow_io_module_removed() {
    let io_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/io.rs");
    assert!(
        !io_path.is_file(),
        "io.rs shim should be folded into command_support"
    );
}
