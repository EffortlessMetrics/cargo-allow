mod json_assertions;
mod support;

use std::fs;

use json_assertions::{assert_json_str, assert_json_u64};
use support::{
    assert_saved_json_artifact, assert_status, assert_stderr_empty, assert_stdout_empty,
    cargo_allow_command, remove_temp_root, temp_root,
};

#[test]
fn shadow_lane_does_not_fail_check_but_receipt_records_posture() {
    let root = temp_root("lane-posture-shadow");
    write_lane_posture_fixture(
        &root,
        "pub fn demo() { unsafe { std::ptr::null::<u8>().read() }; }\n",
    );

    let receipt_output = root.join("target/cargo-allow/check.receipt.json");
    let result = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--mode")
        .arg("no-new")
        .arg("--format")
        .arg("json")
        .arg("--receipt")
        .arg(&receipt_output)
        .output()
        .unwrap_or_else(|err| panic!("run cargo-allow check: {err}"));

    assert_status("check", &result, true);
    assert_stderr_empty("check", &result, "shadow lane should not add stderr noise");
    let report = serde_json::from_slice::<serde_json::Value>(&result.stdout)
        .unwrap_or_else(|err| panic!("check stdout should be JSON: {err}"));
    assert_json_str(&report, "/status", "passed", "shadow lane report status");
    assert_json_u64(
        &report,
        "/summary/new",
        1,
        "shadow lane new count remains visible",
    );

    let receipt = assert_saved_json_artifact(
        &receipt_output,
        "check receipt",
        "cargo-allow.receipt.v1",
        "check",
    );
    assert_json_str(
        &receipt,
        "/lane_posture/unsafe",
        "shadow",
        "receipt lane_posture.unsafe",
    );
    assert_json_str(
        &receipt,
        "/lane_posture/panic",
        "blocking",
        "receipt lane_posture.panic",
    );

    remove_temp_root(root);
}

#[test]
fn blocking_lane_still_fails_no_new() {
    let root = temp_root("lane-posture-blocking");
    write_lane_posture_fixture(
        &root,
        "pub fn demo() { let _ = Option::<u8>::None.unwrap(); }\n",
    );

    let result = cargo_allow_command()
        .arg("check")
        .arg("--root")
        .arg(&root)
        .arg("--mode")
        .arg("no-new")
        .arg("--kind")
        .arg("panic")
        .arg("--format")
        .arg("json")
        .output()
        .unwrap_or_else(|err| panic!("run cargo-allow check: {err}"));

    assert_status("check", &result, false);
    let report = serde_json::from_slice::<serde_json::Value>(&result.stdout)
        .unwrap_or_else(|err| panic!("check stdout should be JSON: {err}"));
    assert_json_str(&report, "/status", "failed", "blocking lane report status");
    assert_json_u64(&report, "/summary/new", 1, "blocking lane new count");

    remove_temp_root(root);
}

#[test]
fn integration_support_links_stdout_helper() {
    let _ = assert_stdout_empty as fn(&str, &std::process::Output, &str);
}

fn write_lane_posture_fixture(root: &std::path::Path, source: &str) {
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|err| panic!("create src dir: {err}"));
    fs::write(root.join("src/lib.rs"), source)
        .unwrap_or_else(|err| panic!("write source fixture: {err}"));
    fs::create_dir_all(root.join("policy"))
        .unwrap_or_else(|err| panic!("create policy dir: {err}"));
    fs::write(
        root.join("policy/allow.toml"),
        r#"policy = "cargo-allow"

[workspace]
ignored = [".git/**", "target/**", "policy/**"]

[lanes.panic]
mode = "blocking"

[lanes.unsafe]
mode = "shadow"
"#,
    )
    .unwrap_or_else(|err| panic!("write policy: {err}"));
}
