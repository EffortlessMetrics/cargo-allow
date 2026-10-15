//! CLI-facing error reporting for cargo-allow.
//!
//! `CargoAllowError` already stores a cause chain. This module owns the binary
//! presentation so first-hour failures print `error:` plus each `caused by:`
//! line by walking `Error::source`, instead of relying only on Display.

use allow_core::CargoAllowError;
use std::error::Error;
use std::io::{self, Write};

/// Format a CLI error report: error code, top-level message, then each cause.
///
/// Hard-error paths historically printed only `error[...]: message` while
/// report paths emitted the full Result/Why/Next grammar (#4345). Append one
/// structured `Next:` action per error kind so every hard failure tells the
/// operator the next safe step. Messages that already carry a `Next:` line
/// (for example located policy parse repairs) keep theirs and gain no second.
pub(crate) fn format_cli_error(err: &CargoAllowError) -> String {
    // When a typed next step is present it replaces any message-carried
    // `Next:` line: the diagnosis is the message up to that line, so the
    // rendered report still carries exactly one Next (#4488).
    let diagnosis = match err.next_step() {
        Some(_) => err.message().split("\nNext:").next().unwrap_or(""),
        None => err.message(),
    };
    let mut out = format!("error[{}]: {}", err.code(), diagnosis);
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
    // A typed next step wins: it is the remedy the raise site chose for this
    // exact failure (#4488). Exactly one `Next:` line is rendered in every
    // path: typed, then message-carried, then the per-kind generic.
    if let Some(next_step) = err.next_step() {
        out.push_str("\nNext: ");
        out.push_str(next_step);
    } else if !err.message().contains("Next:") {
        out.push_str("\nNext: ");
        out.push_str(next_action_for_kind(err.kind()));
    }
    out
}

/// The next safe operator action for each error kind.
///
/// Actions are deliberately conservative: inspect, repair the named input,
/// or re-run a read-only diagnostic. None mutates policy or source state.
fn next_action_for_kind(kind: allow_core::CargoAllowErrorKind) -> &'static str {
    use allow_core::CargoAllowErrorKind::*;
    match kind {
        Usage => {
            "review the command syntax with `cargo-allow <command> --help`, correct the options, and retry."
        }
        InvalidConfig => {
            "run `cargo-allow init` to create a starter policy or pass `--config <path>` to an existing cargo-allow policy, then retry."
        }
        InvalidPolicy => {
            "repair the policy entry or value at the reported location (see docs/error-codes.md), then retry."
        }
        Inventory => {
            "verify the named revision, path, or repository exists (for example `git rev-parse <ref>`), confirm you are in the intended repository, and retry."
        }
        Scan => {
            "check the named source file is readable source-tree text, then retry; run `cargo-allow doctor` if the inventory looks wrong."
        }
        PolicyViolation => {
            "run `cargo-allow worklist --format json` to see the blocking items, receipt or narrow them, then re-run the gate."
        }
        Artifact => {
            "check the output path is writable and its parent directory exists, then retry."
        }
        Unsupported => {
            "use a supported alternative for this platform or contract (see the command `--help`), or run without the unsupported option."
        }
        InstrumentFailure => {
            "check the named external tool is installed and runnable, then retry; cargo-allow does not require external tools for its own scan."
        }
        Internal => {
            "re-run with the same inputs; if it persists, report the full error text as an internal defect."
        }
        Unknown => {
            "run `cargo-allow doctor` to inspect the selected root, policy, and inventory, then retry."
        }
        // CargoAllowErrorKind is non-exhaustive; future kinds get the same
        // conservative diagnostic step until they earn a specific action.
        _ => {
            "run `cargo-allow doctor` to inspect the selected root, policy, and inventory, then retry."
        }
    }
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
            "error[E0004_INVENTORY]: failed to load revision notes\n  caused by: git diff failed\n  caused by: broken pipe\nNext: verify the named revision, path, or repository exists (for example `git rev-parse <ref>`), confirm you are in the intended repository, and retry."
        );
    }

    #[test]
    fn format_cli_error_appends_next_for_every_kind() {
        for kind in [
            CargoAllowErrorKind::Usage,
            CargoAllowErrorKind::InvalidConfig,
            CargoAllowErrorKind::InvalidPolicy,
            CargoAllowErrorKind::Inventory,
            CargoAllowErrorKind::Scan,
            CargoAllowErrorKind::PolicyViolation,
            CargoAllowErrorKind::Artifact,
            CargoAllowErrorKind::Unsupported,
            CargoAllowErrorKind::InstrumentFailure,
            CargoAllowErrorKind::Internal,
            CargoAllowErrorKind::Unknown,
        ] {
            let err = CargoAllowError::with_kind(kind, "something failed");
            let report = format_cli_error(&err);
            assert!(
                report.contains("\nNext: "),
                "kind {kind:?} missing Next: {report}"
            );
        }
    }

    #[test]
    fn format_cli_error_keeps_existing_next_line() {
        let err = CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidPolicy,
            "failed to parse policy TOML\nNext: repair the policy TOML at the reported location and retry.",
        );
        let report = format_cli_error(&err);
        assert_eq!(report.matches("Next:").count(), 1, "{report}");
    }

    #[test]
    fn format_cli_error_prefers_typed_next_step() {
        // #4488: the typed remedy wins over both the message-carried and the
        // per-kind generic guidance, and renders exactly one Next line.
        let err = CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            "diff requires --base <revision>",
        )
        .with_next_step("pass --base <revision>, then retry.");
        let report = format_cli_error(&err);
        assert_eq!(report.matches("Next:").count(), 1, "{report}");
        assert!(
            report.ends_with("Next: pass --base <revision>, then retry."),
            "{report}"
        );
        assert!(!report.contains("review the command syntax"), "{report}");

        let plain = CargoAllowError::with_kind(CargoAllowErrorKind::Usage, "plain failure");
        assert_eq!(plain.next_step(), None);

        // Typed + message-carried: the typed step replaces the carried one.
        let both = CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            "stale plan\nNext: embedded guidance.",
        )
        .with_next_step("typed guidance.");
        let both_report = format_cli_error(&both);
        assert_eq!(both_report.matches("Next:").count(), 1, "{both_report}");
        assert!(
            both_report.contains("Next: typed guidance."),
            "{both_report}"
        );
        assert!(!both_report.contains("embedded guidance"), "{both_report}");
    }

    #[test]
    fn format_cli_error_skips_duplicate_io_from_conversion() {
        let err: CargoAllowError =
            std::io::Error::new(std::io::ErrorKind::NotFound, "policy missing").into();
        assert_eq!(
            format_cli_error(&err),
            "error[E0002_INVALID_CONFIG]: policy missing\nNext: run `cargo-allow init` to create a starter policy or pass `--config <path>` to an existing cargo-allow policy, then retry."
        );
    }
}
