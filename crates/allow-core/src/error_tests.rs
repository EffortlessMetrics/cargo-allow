use super::{CargoAllowError, CargoAllowErrorKind};

#[test]
fn cargo_allow_error_fmt_return_value_observer() {
    let err = CargoAllowError::new("policy validation failed");
    assert_eq!(format!("{err}"), "policy validation failed");
    assert_eq!(err.to_string(), "policy validation failed");
}

#[test]
fn new_defaults_to_unknown_kind() {
    let err = CargoAllowError::new("something went wrong");
    assert_eq!(err.kind(), CargoAllowErrorKind::Unknown);
    assert_eq!(err.message(), "something went wrong");
}

#[test]
fn with_kind_sets_structured_kind() {
    let err = CargoAllowError::with_kind(CargoAllowErrorKind::InvalidPolicy, "missing owner");
    assert_eq!(err.kind(), CargoAllowErrorKind::InvalidPolicy);
    assert_eq!(err.message(), "missing owner");
}

#[test]
fn kind_renders_as_stable_lowercase_str() {
    assert_eq!(CargoAllowErrorKind::Usage.as_str(), "usage");
    assert_eq!(
        CargoAllowErrorKind::InvalidConfig.as_str(),
        "invalid_config"
    );
    assert_eq!(
        CargoAllowErrorKind::InvalidPolicy.as_str(),
        "invalid_policy"
    );
    assert_eq!(
        CargoAllowErrorKind::PolicyViolation.as_str(),
        "policy_violation"
    );
    assert_eq!(CargoAllowErrorKind::Artifact.as_str(), "artifact");
    assert_eq!(CargoAllowErrorKind::Unknown.as_str(), "unknown");
}

#[test]
fn display_includes_cause_chain() {
    let inner = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
    let err = CargoAllowError::with_kind(CargoAllowErrorKind::Artifact, "failed to write receipt")
        .with_cause(&inner);
    let rendered = format!("{err}");
    assert!(rendered.contains("failed to write receipt"));
    assert!(rendered.contains("caused by:"));
    assert!(rendered.contains("file missing"));
}

#[test]
fn partialeq_compares_kind_and_message_not_causes() {
    let a = CargoAllowError::with_kind(CargoAllowErrorKind::Scan, "read error");
    let b = CargoAllowError::with_kind(CargoAllowErrorKind::Scan, "read error");
    // Same kind + message -> equal even if causes differ.
    let a_with_cause = a.with_cause(&std::io::Error::other("boom"));
    assert_eq!(a_with_cause, b);
    // Different kind -> not equal.
    let c = CargoAllowError::with_kind(CargoAllowErrorKind::Unknown, "read error");
    assert_ne!(b, c);
}

#[test]
fn from_io_error_preserves_kind_and_message() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
    let err: CargoAllowError = io_err.into();
    assert_eq!(err.kind(), CargoAllowErrorKind::InvalidConfig);
    assert!(err.message().contains("no such file"));
}

#[test]
fn from_io_permission_denied_maps_to_inventory_kind() {
    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
    let err: CargoAllowError = io_err.into();
    assert_eq!(err.kind(), CargoAllowErrorKind::Inventory);
}
