//! CLI-facing error reporting for cargo-allow.
//!
//! `CargoAllowError` already stores a cause chain. This module owns the binary
//! presentation so first-hour failures print `error:` plus each `caused by:`
//! line by walking `Error::source`, instead of relying only on Display.

use allow_core::CargoAllowError;
use std::error::Error;
use std::io::{self, Write};

/// Format a CLI error report: error code, top-level message, then each cause.
pub(crate) fn format_cli_error(err: &CargoAllowError) -> String {
    let mut out = format!("error[{}]: {}", err.code(), err.message());
    let mut current = err.source();
    while let Some(cause) = current {
        let text = cause.to_string();
        // `From<io::Error>` keeps the same text on message and source; skip the
        // duplicate so adopters see one line for a plain IO failure.
        if text != err.message() {
            out.push_str("\n  caused by: ");
            out.push_str(&text);
        }
        current = cause.source();
    }
    out
}

/// Print [`format_cli_error`] to stderr.
pub(crate) fn report_cli_error(err: &CargoAllowError) {
    let report = format_cli_error(err);
    let mut stderr = io::stderr().lock();
    let _ = writeln!(stderr, "{report}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::CargoAllowErrorKind;

    #[test]
    fn format_cli_error_prints_multi_step_cause_chain() {
        let inner = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "broken pipe");
        let mid = std::io::Error::other("git diff failed");
        let err = CargoAllowError::with_kind(
            CargoAllowErrorKind::Inventory,
            "failed to load revision notes",
        )
        .with_cause(&mid)
        .with_cause(&inner);

        let report = format_cli_error(&err);
        assert_eq!(
            report,
            "error[E0004_INVENTORY]: failed to load revision notes\n  caused by: git diff failed\n  caused by: broken pipe"
        );
    }

    #[test]
    fn format_cli_error_skips_duplicate_io_from_conversion() {
        let err: CargoAllowError =
            std::io::Error::new(std::io::ErrorKind::NotFound, "policy missing").into();
        assert_eq!(
            format_cli_error(&err),
            "error[E0002_INVALID_CONFIG]: policy missing"
        );
    }
}
