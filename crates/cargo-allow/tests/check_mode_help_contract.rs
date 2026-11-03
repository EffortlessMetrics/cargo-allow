//! Help-vs-law agreement for the check-mode documentation (#4236): the
//! rendered `check --help` no-new block must describe the #4238
//! annotation-only expiry law instead of listing expired as a failure
//! status, and the asserted semantics must match the allow-match mode
//! table directly.

use std::process::Command;

use allow_core::MatchStatus;
use allow_match::CheckMode;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn no_new_help_documents_annotation_only_expiry() -> TestResult {
    // The mode table is the authority; assert its semantics directly so the
    // help-text assertions below cannot drift away from the behavior.
    assert!(!CheckMode::NoNew.fails(MatchStatus::Expired));
    assert!(CheckMode::NoNew.fails(MatchStatus::New));
    assert!(CheckMode::Strict.fails(MatchStatus::Expired));

    let output = Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .args(["check", "--help"])
        .output()?;
    assert!(output.status.success());
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(
        !help.contains("new/expired/"),
        "no-new help must not list expired as a failure status; got:\n{help}"
    );
    assert!(
        help.contains("expired is annotation-only"),
        "no-new help must state the #4238 annotation-only law; got:\n{help}"
    );
    assert!(
        help.contains("calendar_expiry_blocks_no_new"),
        "no-new help must name the gate-layer restoration knob; got:\n{help}"
    );
    assert!(
        help.contains("stale/review_due/baseline_debt/expired"),
        "strict help must keep expired in its failure set; got:\n{help}"
    );
    Ok(())
}
