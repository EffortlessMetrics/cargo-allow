#![cfg(feature = "changie")]

use std::{
    process::{Command, Stdio},
    time::Duration,
};

use allow_files::changie::{
    ChangieParseDiagnostic, ChangieParseDiagnosticKind, ChangieRepoPath, ChangieSourceDocument,
    parse_config, parse_fragment,
};

const VULNERABLE_YAML_RUST2_VERSION: &str = "0.11.0";
const YAML_RUST2_ISSUE_78_TRIGGER: &[u8] = &[0xff, 0xfe, 0x34, 0x12, 0x34, 0x12, 0x34, 0x12];

fn issue_78_source(path: &str) -> ChangieSourceDocument {
    ChangieSourceDocument::from_bytes(
        ChangieRepoPath::from_repo_relative(path)
            .unwrap_or_else(|error| std::panic::panic_any(format!("repo path: {error}"))),
        YAML_RUST2_ISSUE_78_TRIGGER.to_vec(),
        Some(format!(
            "yaml-rust2-{VULNERABLE_YAML_RUST2_VERSION}-issue-78"
        )),
    )
    .unwrap_or_else(|error| std::panic::panic_any(format!("source document: {error}")))
}

fn assert_non_utf8_rejection(root_present: bool, diagnostics: &[ChangieParseDiagnostic]) {
    assert!(!root_present, "non-UTF-8 input must not produce a tree");
    assert_eq!(
        diagnostics.len(),
        1,
        "the reachability boundary must produce one precise diagnostic: {diagnostics:?}"
    );
    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic.kind, ChangieParseDiagnosticKind::NonUtf8);
    assert!(diagnostic.path.is_none());
    assert!(diagnostic.range.is_none());
}

macro_rules! assert_issue_78_document {
    ($label:literal, $document:expr) => {{
        let document = $document;
        assert_eq!(
            document.source.bytes(),
            YAML_RUST2_ISSUE_78_TRIGGER,
            concat!($label, " bytes")
        );
        assert_eq!(
            document.source.subject(),
            Some("yaml-rust2-0.11.0-issue-78"),
            concat!($label, " subject")
        );
        assert!(
            document.unknown_fields.is_empty(),
            concat!($label, " unknown fields")
        );
        assert!(
            document.unsupported_fields.is_empty(),
            concat!($label, " unsupported fields")
        );
        assert_non_utf8_rejection(document.root.is_some(), &document.diagnostics);
    }};
}

fn assert_issue_78_rejection() {
    assert_issue_78_document!("config", parse_config(issue_78_source(".changie.yaml")));
    assert_issue_78_document!(
        "fragment",
        parse_fragment(issue_78_source(".changes/Fixed-issue-78.yaml"))
    );
}

/// yaml-rust2 #78 hangs in `YamlDecoder::decode` while transcoding this
/// eight-byte UTF-16LE input. Cargo-Allow's shipped Changie parser accepts raw
/// bytes but requires UTF-8 before constructing `Parser`, so the exact upstream
/// trigger is `UpstreamTriggerNotApplicable` to both public parse entry points.
#[test]
fn issue_78_trigger_is_bounded_and_classified_as_non_utf8() {
    const CHILD_ENV: &str = "CARGO_ALLOW_YAML_RUST2_ISSUE_78_CHILD";
    const TEST_NAME: &str = "issue_78_trigger_is_bounded_and_classified_as_non_utf8";

    if std::env::var_os(CHILD_ENV).is_some() {
        assert_issue_78_rejection();
        return;
    }

    let mut child = Command::new(std::env::current_exe().unwrap_or_else(|error| {
        std::panic::panic_any(format!("resolve test executable: {error}"))
    }))
    .args(["--exact", TEST_NAME, "--nocapture"])
    .env(CHILD_ENV, "1")
    .stdout(Stdio::null())
    .stderr(Stdio::piped())
    .spawn()
    .unwrap_or_else(|error| std::panic::panic_any(format!("spawn watchdog child: {error}")));

    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                assert!(
                    status.success(),
                    "yaml-rust2 {VULNERABLE_YAML_RUST2_VERSION} issue #78 child failed: {status}"
                );
                return;
            }
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                panic!(
                    "yaml-rust2 {VULNERABLE_YAML_RUST2_VERSION} issue #78 trigger exceeded the five-second process watchdog"
                );
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("poll yaml-rust2 issue #78 watchdog child: {error}");
            }
        }
    }
}
