//! End-to-end controls for the retained campaign-issue-closeout verifier
//! (#3846). These drive the real `scripts/verify-campaign-issue-closeout.py`
//! against a local GitHub API stub and assert the reopen/accept/no-op law on
//! real process behavior: an invalid closure reopens the issue with a bounded
//! rejection comment, a valid Complete closeout with sufficient evidence
//! publishes nothing, and a non-denominator issue is a no-op. Cfg(unix): the
//! verifier is a python3/urllib script; no network outside the loopback stub
//! and no credentials are involved.

#![cfg(unix)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn serve_stub(
    pulls: &str,
    compare: &str,
    log_path: &Path,
) -> std::io::Result<(String, thread::JoinHandle<()>)> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port().to_string();
    let pulls = pulls.to_string();
    let compare = compare.to_string();
    let log_path = log_path.to_path_buf();
    let handle = thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buffer = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let Ok(read) = stream.read(&mut chunk) else {
                    break;
                };
                if read == 0 {
                    break;
                }
                if let Some(received) = chunk.get(..read) {
                    buffer.extend_from_slice(received);
                }
                let text = String::from_utf8_lossy(&buffer);
                let Some(header_end) = text.find("\r\n\r\n") else {
                    continue;
                };
                let length = text
                    .split("\r\n")
                    .find_map(|line| {
                        line.split_once(':')
                            .and_then(|(_, rest)| rest.trim().parse::<usize>().ok())
                            .filter(|_| line.to_ascii_lowercase().starts_with("content-length"))
                    })
                    .unwrap_or(0);
                let body_start = header_end + 4;
                if buffer.len() >= body_start + length {
                    break;
                }
            }
            let text = String::from_utf8_lossy(&buffer).to_string();
            let mut parts = text
                .split("\r\n")
                .next()
                .unwrap_or_default()
                .split_whitespace();
            let method = parts.next().unwrap_or_default().to_string();
            let path = parts.next().unwrap_or_default().to_string();
            let (payload, code) = if path.starts_with("/pulls/") {
                (pulls.clone(), 200)
            } else if path.contains("/compare/") {
                (compare.clone(), 200)
            } else if path.contains("/comments") && method == "GET" {
                ("[]".to_string(), 200)
            } else {
                ("{}".to_string(), 200)
            };
            let response = format!(
                "HTTP/1.1 {code} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                payload.len(),
                payload
            );
            if stream.write_all(response.as_bytes()).is_err() {
                continue;
            }
            if let Ok(mut log) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
            {
                let _ = writeln!(log, "{method} {path}");
            }
            let _ = stream.flush();
        }
    });
    Ok((port, handle))
}

fn scratch_dir() -> std::io::Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!(
        "closeout-controls-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(std::io::Error::other)?
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn json_string(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn write_event(dir: &Path, number: u64, body: &str) -> std::io::Result<PathBuf> {
    let event = format!(
        r#"{{"action": "closed", "issue": {{"number": {number}, "body": {}}}}}"#,
        json_string(body)
    );
    let path = dir.join(format!("event-{number}.json"));
    std::fs::write(&path, event)?;
    Ok(path)
}

fn run_verifier(
    event_path: &Path,
    api_url: &str,
    repo_root: &Path,
) -> std::io::Result<std::process::Output> {
    Command::new("python3")
        .arg(repo_root.join("scripts/verify-campaign-issue-closeout.py"))
        .env("GITHUB_EVENT_PATH", event_path)
        .env("GITHUB_API_URL", api_url)
        .env("GITHUB_REPOSITORY", "EffortlessMetrics/cargo-allow")
        .env("GITHUB_TOKEN", "fixture-token")
        .current_dir(repo_root)
        .output()
}

fn stub_log(log_path: &Path) -> String {
    std::fs::read_to_string(log_path).unwrap_or_default()
}

#[test]
fn campaign_closeout_verifier_reopens_invalid_and_accepts_complete() -> TestResult {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let pulls = format!(
        r#"{{"merged_at": "2026-11-02T00:00:00Z", "state": "closed", "base": {{"ref": "main"}}, "merge_commit_sha": "{}"}}"#,
        "a".repeat(40)
    );
    let compare = r#"{"status": "identical"}"#.to_string();
    let dir = scratch_dir()?;
    let log_path = dir.join("stub-requests.log");
    let (port, handle) = serve_stub(&pulls, &compare, &log_path)?;
    let api_url = format!("http://127.0.0.1:{port}");

    // Case 1: a closure with no closeout marker reopens the issue with the
    // bounded rejection comment and never consults pull-request evidence.
    let event = write_event(&dir, 3846, "plain closure without a closeout record")?;
    let output = run_verifier(&event, &api_url, &repo_root)?;
    assert!(
        output.status.success(),
        "verifier exit: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let log = stub_log(&log_path);
    assert!(
        log.contains("PATCH /issues/3846"),
        "invalid closure must reopen the issue; stub log:\n{log}"
    );
    assert!(
        log.contains("\"state\": \"open\""),
        "reopen PATCH must set state open; stub log:\n{log}"
    );
    assert!(
        !log.contains("/pulls/"),
        "invalid closure must fail before pull-request evidence; stub log:\n{log}"
    );

    // Case 2: a valid Complete closeout with sufficient evidence publishes
    // nothing (no reopen PATCH) and verifies the merged pull request.
    let payload = format!(
        r#"{{"schema_id": "cargo-allow.campaign-issue-closeout.v1", "issue": 3846, "result": "Complete", "merged_pr": 4147, "closeout_id": "fixture-closeout-3846-controls", "evidence_surfaces": ["release-authorization-v1"]}}"#
    );
    let body = format!(
        "closing comment\n\n<!-- cargo-allow:campaign-closeout.v1 -->\n```json\n{payload}\n```\n"
    );
    std::fs::remove_file(&log_path).ok();
    let event = write_event(&dir, 3846, &body)?;
    let output = run_verifier(&event, &api_url, &repo_root)?;
    assert!(
        output.status.success(),
        "verifier exit: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let log = stub_log(&log_path);
    assert!(
        log.contains("/pulls/4147"),
        "Complete closeout must verify the declared merged PR; stub log:\n{log}"
    );
    assert!(
        !log.contains("PATCH /issues/"),
        "accepted closeout must not reopen the issue; stub log:\n{log}"
    );

    // Case 3: a non-denominator issue number is a no-op.
    std::fs::remove_file(&log_path).ok();
    let event = write_event(&dir, 999999, "not in the denominator")?;
    let output = run_verifier(&event, &api_url, &repo_root)?;
    assert!(
        output.status.success(),
        "verifier exit: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let log = stub_log(&log_path);
    assert!(
        log.is_empty(),
        "non-denominator issue must be a stub no-op; stub log:\n{log}"
    );

    drop(handle);
    Ok(())
}
