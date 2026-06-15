use super::CargoAllowError;

#[test]
fn cargo_allow_error_fmt_return_value_observer() {
    let err = CargoAllowError::new("policy validation failed");
    assert_eq!(format!("{err}"), "policy validation failed");
    assert_eq!(err.to_string(), "policy validation failed");
}
