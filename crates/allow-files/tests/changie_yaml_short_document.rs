#![cfg(feature = "changie")]

use std::{
    sync::mpsc::{self, RecvTimeoutError},
    thread,
    time::Duration,
};

use allow_files::changie::{
    parse_config, parse_fragment, ChangieParseDiagnostic, ChangieParseDiagnosticKind,
    ChangieRepoPath, ChangieSourceDocument,
};

const VULNERABLE_YAML_RUST2_VERSION: &str = "0.11.0";
const YAML_RUST2_ISSUE_78_TRIGGER: &[u8] =
    &[0xff, 0xfe, 0x34, 0x12, 0x34, 0x12, 0x34, 0x12];

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

fn assert_non_utf8_rejection(
    root_present: bool,
    diagnostics: &[ChangieParseDiagnostic],
) {
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
    assert_eq!(
        diagnostic.message,
        "document is not valid UTF-8; no tree was produced"
    );
}

/// yaml-rust2 #78 hangs in `YamlDecoder::decode` while transcoding this
/// eight-byte UTF-16LE input. Cargo-Allow's shipped Changie parser accepts raw
/// bytes but requires UTF-8 before constructing `Parser`, so the exact upstream
/// trigger is `UpstreamTriggerNotApplicable` to both public parse entry points.
#[test]
fn issue_78_trigger_is_bounded_and_rejected_before_yaml_decoder() {
    let (sender, receiver) = mpsc::channel();
    let worker = thread::spawn(move || {
        let config = parse_config(issue_78_source(".changie.yaml"));
        let fragment = parse_fragment(issue_78_source(".changes/Fixed-issue-78.yaml"));
        assert!(
            sender.send((config, fragment)).is_ok(),
            "characterization receiver closed before parsing completed"
        );
    });

    let (config, fragment) = match receiver.recv_timeout(Duration::from_secs(5)) {
        Ok(parsed) => parsed,
        Err(RecvTimeoutError::Timeout) => std::panic::panic_any(format!(
            "yaml-rust2 {VULNERABLE_YAML_RUST2_VERSION} issue #78 trigger exceeded the watchdog; the shipped path may now reach the decoder hang"
        )),
        Err(RecvTimeoutError::Disconnected) => std::panic::panic_any(
            "characterization worker disconnected before returning a parser result",
        ),
    };
    assert!(
        worker.join().is_ok(),
        "characterization worker panicked after returning its parser result"
    );

    assert_eq!(config.source.bytes(), YAML_RUST2_ISSUE_78_TRIGGER);
    assert!(config.unknown_fields.is_empty());
    assert!(config.unsupported_fields.is_empty());
    assert_non_utf8_rejection(config.root.is_some(), &config.diagnostics);

    assert_eq!(fragment.source.bytes(), YAML_RUST2_ISSUE_78_TRIGGER);
    assert!(fragment.unknown_fields.is_empty());
    assert!(fragment.unsupported_fields.is_empty());
    assert_non_utf8_rejection(fragment.root.is_some(), &fragment.diagnostics);
}
