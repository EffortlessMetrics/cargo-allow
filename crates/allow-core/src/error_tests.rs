use super::{CargoAllowError, CargoAllowErrorKind};
use std::path::Path;

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
fn with_kind_preserving_metadata_reclassifies_without_loss() -> Result<(), String> {
    let reclassified = structured_error().with_kind_preserving_metadata(CargoAllowErrorKind::Usage);

    assert_eq!(reclassified.kind(), CargoAllowErrorKind::Usage);
    assert_eq!(reclassified.message(), "missing owner");
    let location = reclassified
        .location()
        .ok_or_else(|| "reclassification should preserve location".to_string())?;
    assert_eq!(location.path.as_deref(), Some("legacy/policy.toml"));
    assert_eq!(reclassified.diagnostics().len(), 1);
    assert_eq!(
        reclassified.causes(),
        &[String::from("underlying parse failure")]
    );
    Ok(())
}

fn structured_error() -> CargoAllowError {
    let cause = std::io::Error::other("underlying parse failure");

    CargoAllowError::with_kind(CargoAllowErrorKind::InvalidPolicy, "missing owner")
        .with_toml_span(
            Some(Path::new("legacy/policy.toml")),
            "policy = \"unsafe-allowlist\"\nowner = [",
            Some(36..37),
        )
        .with_diagnostic(super::CargoAllowDiagnostic::error(
            "E0003_INVALID_POLICY",
            "policy_validation",
            Some("allow-1"),
            Some("owner"),
            "allow-1 missing owner",
        ))
        .with_cause(&cause)
}

#[test]
fn message_prefix_preserves_structured_error_metadata() -> Result<(), String> {
    let prefixed = structured_error().with_message_prefix("legacy file `policy.toml`: ");

    assert_eq!(
        prefixed.message(),
        "legacy file `policy.toml`: missing owner"
    );
    assert_eq!(prefixed.kind(), CargoAllowErrorKind::InvalidPolicy);
    let location = prefixed
        .location()
        .ok_or_else(|| "message prefix should preserve location".to_string())?;
    assert_eq!(location.path.as_deref(), Some("legacy/policy.toml"));
    assert_eq!(location.line, 2);
    assert_eq!(location.column, 9);
    assert_eq!(prefixed.diagnostics().len(), 1);
    let diagnostic = prefixed
        .diagnostics()
        .first()
        .ok_or_else(|| "message prefix should preserve diagnostics".to_string())?;
    assert_eq!(diagnostic.code, "E0003_INVALID_POLICY");
    assert_eq!(diagnostic.field.as_deref(), Some("owner"));
    assert_eq!(diagnostic.message, "allow-1 missing owner");
    assert_eq!(
        prefixed.causes(),
        &[String::from("underlying parse failure")]
    );
    Ok(())
}

#[test]
fn message_suffix_preserves_structured_error_metadata() -> Result<(), String> {
    let suffixed = structured_error().with_message_suffix("; regenerate the plan");

    assert_eq!(suffixed.message(), "missing owner; regenerate the plan");
    assert_eq!(suffixed.kind(), CargoAllowErrorKind::InvalidPolicy);
    let location = suffixed
        .location()
        .ok_or_else(|| "message suffix should preserve location".to_string())?;
    assert_eq!(location.path.as_deref(), Some("legacy/policy.toml"));
    assert_eq!(suffixed.diagnostics().len(), 1);
    assert_eq!(
        suffixed.causes(),
        &[String::from("underlying parse failure")]
    );
    Ok(())
}

#[test]
fn empty_message_prefix_leaves_error_unchanged() -> Result<(), String> {
    let unchanged = structured_error().with_message_prefix("");

    assert_eq!(unchanged.message(), "missing owner");
    assert_eq!(unchanged.kind(), CargoAllowErrorKind::InvalidPolicy);
    let location = unchanged
        .location()
        .ok_or_else(|| "empty prefix should preserve location".to_string())?;
    assert_eq!(location.path.as_deref(), Some("legacy/policy.toml"));
    assert_eq!(location.column, 9);
    assert_eq!(unchanged.diagnostics().len(), 1);
    assert_eq!(
        unchanged.causes(),
        &[String::from("underlying parse failure")]
    );
    Ok(())
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
fn every_error_kind_has_a_unique_stable_code() {
    let codes = CargoAllowErrorKind::ALL
        .iter()
        .map(|kind| kind.code())
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(codes.len(), CargoAllowErrorKind::ALL.len());
    assert_eq!(CargoAllowErrorKind::Usage.code(), "E0001_USAGE");
    assert_eq!(
        CargoAllowErrorKind::InvalidPolicy.code(),
        "E0003_INVALID_POLICY"
    );
    assert_eq!(CargoAllowErrorKind::Unknown.code(), "E0009_UNKNOWN");
}

#[test]
fn error_exposes_the_code_of_its_kind() {
    let error = CargoAllowError::with_kind(CargoAllowErrorKind::Artifact, "write failed");

    assert_eq!(error.code(), "E0007_ARTIFACT");
}

#[test]
fn newly_classified_runtime_kinds_have_append_only_codes() {
    assert_eq!(CargoAllowErrorKind::Unsupported.code(), "E0010_UNSUPPORTED");
    assert_eq!(
        CargoAllowErrorKind::InstrumentFailure.code(),
        "E0011_INSTRUMENT_FAILURE"
    );
    assert_eq!(CargoAllowErrorKind::Internal.code(), "E0008_INTERNAL");
    assert_eq!(CargoAllowErrorKind::Unknown.code(), "E0009_UNKNOWN");
}

#[test]
fn error_exposes_structured_diagnostic_details() -> Result<(), String> {
    let error = CargoAllowError::with_kind(CargoAllowErrorKind::InvalidPolicy, "missing owner")
        .with_diagnostic(super::CargoAllowDiagnostic::error(
            "E0003_INVALID_POLICY",
            "policy_validation",
            Some("allow-1"),
            Some("owner"),
            "allow-1 missing owner",
        ));

    let diagnostic = error
        .diagnostics()
        .first()
        .ok_or_else(|| "diagnostic should be present".to_string())?;
    assert_eq!(diagnostic.code, "E0003_INVALID_POLICY");
    assert_eq!(diagnostic.entry_id.as_deref(), Some("allow-1"));
    assert_eq!(diagnostic.field.as_deref(), Some("owner"));
    Ok(())
}

#[test]
fn toml_span_preserves_path_and_one_based_line_column() -> Result<(), String> {
    let error = CargoAllowError::with_kind(CargoAllowErrorKind::InvalidPolicy, "invalid TOML")
        .with_toml_span(
            Some(Path::new("policy/allow.toml")),
            "policy = \"cargo-allow\"\nowner = [",
            Some(29..30),
        );
    let location = error
        .location()
        .ok_or_else(|| "TOML span should produce a structured location".to_string())?;

    assert_eq!(location.path.as_deref(), Some("policy/allow.toml"));
    assert_eq!(location.line, 2);
    assert!(location.column > 0);
    Ok(())
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
fn error_source_walks_attached_causes() {
    use std::error::Error as _;

    let inner = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
    let mid = std::io::Error::other("git failed");
    let err = CargoAllowError::with_kind(CargoAllowErrorKind::Inventory, "failed to read revision")
        .with_cause(&mid)
        .with_cause(&inner);

    let first = err
        .source()
        .unwrap_or_else(|| std::panic::panic_any("expected first cause"));
    assert_eq!(first.to_string(), "git failed");
    let second = first
        .source()
        .unwrap_or_else(|| std::panic::panic_any("expected nested cause"));
    assert_eq!(second.to_string(), "file missing");
    assert!(second.source().is_none());
}

#[test]
fn from_io_error_exposes_source_without_duplicate_display_cause() {
    use std::error::Error as _;

    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
    let err: CargoAllowError = io_err.into();
    assert_eq!(err.message(), "no such file");
    assert!(err.causes().is_empty());
    let source = err
        .source()
        .unwrap_or_else(|| std::panic::panic_any("expected io source"));
    assert_eq!(source.to_string(), "no such file");
    assert!(!format!("{err}").contains("caused by:"));
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
