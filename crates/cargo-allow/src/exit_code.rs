//! Binary process exit codes for CLI errors.
//!
//! Clap parse failures exit 2 before `main` sees them. Structured
//! [`CargoAllowErrorKind::Usage`] errors that reach `main` use the same exit
//! code so invocation failures share one process-level meaning.

use allow_core::{CargoAllowError, CargoAllowErrorKind};

/// Process exit code for a structured CLI error that reached `main`.
///
/// Mapping is by [`CargoAllowErrorKind`] only — never by message text.
pub(crate) fn exit_code_for_error(err: &CargoAllowError) -> i32 {
    exit_code_for_error_kind(err.kind())
}

/// Process exit code for a structured error kind.
///
/// - [`CargoAllowErrorKind::Usage`] → 2 (operator invocation / argument contract)
/// - every other kind → 1 (policy gate, runtime, config, IO, invariant, unknown)
///
/// Command handlers that call `process::exit` for policy-gate failures keep that
/// path; this helper only covers errors returned through `cli::run`.
pub(crate) fn exit_code_for_error_kind(kind: CargoAllowErrorKind) -> i32 {
    match kind {
        CargoAllowErrorKind::Usage => 2,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_kind_maps_to_exit_2() {
        assert_eq!(exit_code_for_error_kind(CargoAllowErrorKind::Usage), 2);
        let err = CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            "--glob is mutually exclusive with --path/--line",
        );
        assert_eq!(exit_code_for_error(&err), 2);
    }

    #[test]
    fn non_usage_kinds_map_to_exit_1() {
        for kind in CargoAllowErrorKind::ALL {
            if *kind == CargoAllowErrorKind::Usage {
                continue;
            }
            assert_eq!(exit_code_for_error_kind(*kind), 1, "{kind:?} should exit 1");
        }
    }
}
