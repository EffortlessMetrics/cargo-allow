use std::error::Error;
use std::io;
use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> Result<PathBuf, Box<dyn Error>> {
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?)
}

fn require_python_success(output: std::process::Output, label: &str) -> Result<(), Box<dyn Error>> {
    if output.status.success() {
        return Ok(());
    }
    Err(io::Error::other(format!(
        "{label} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    ))
    .into())
}

#[test]
fn final_registry_preflight_provider_python_adapter_contract() -> Result<(), Box<dyn Error>> {
    let root = repo_root()?;
    let output = Command::new("python")
        .arg(root.join("scripts/test-final-registry-observation.py"))
        .current_dir(&root)
        .output()?;
    require_python_success(output, "registry observation adapter contract")
}

const OUTPUT_SAFETY_HARNESS: &str = r#"
from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
from pathlib import Path

adapter_path = Path(sys.argv[1])
spec = importlib.util.spec_from_file_location("final_registry_observation_safety", adapter_path)
if spec is None or spec.loader is None:
    raise RuntimeError("could not load final registry observer")
adapter = importlib.util.module_from_spec(spec)
spec.loader.exec_module(adapter)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


def expect_exit(action, message: str) -> None:
    try:
        action()
    except SystemExit:
        return
    raise RuntimeError(message)


def write_topology(path: Path) -> None:
    lines = ['topology_id = "CARGO-ALLOW-PKG-TOPOLOGY-V2-0001"', ""]
    for index, ((logical, package, version, family), order) in enumerate(
        zip(adapter.SELECTION, adapter.RELEASE_ORDERS), start=1
    ):
        lines.extend(
            [
                "[[package]]",
                f'logical_id = "{logical}"',
                f'cargo_package_name = "{package}"',
                f'product_family = "{family}"',
                f'package_version = "{version}"',
                f"release_order = {order}",
                "candidate_inclusion = true",
            ]
        )
        if family == "shared":
            lines.append(f'expected_registry_checksum = "sha256:{index:064x}"')
        lines.append("")
    path.write_text("\n".join(lines), encoding="utf-8", newline="\n")


def fake_observe(row, observed_at):
    name = row["cargo_package_name"]
    version = row["package_version"]
    response = {"status": "missing"}
    evidence = {
        "package_name": name,
        "package_version": version,
        "exact_version_request": {
            "url": f"fixture://{name}/{version}",
            "outcome": "not_found",
            "http_status": 404,
            "attempts": 1,
        },
        "crate_name_request": {
            "url": f"fixture://{name}",
            "outcome": "found",
            "http_status": 200,
            "attempts": 1,
            "projection": {"id": name},
        },
        "conclusion": response,
    }
    observation = {
        "package_name": name,
        "package_version": version,
        "version": response,
        "version_provenance": adapter.provenance(
            "fixture", f"fixture://{name}", evidence, observed_at
        ),
        "owner": "permission_not_proven",
        "owner_provenance": adapter.rule_provenance(adapter.OWNER_RULE, observed_at),
        "publish_authority": "not_proven",
        "authority_provenance": adapter.rule_provenance(
            adapter.AUTHORITY_RULE, observed_at
        ),
    }
    return observation, evidence


with tempfile.TemporaryDirectory(prefix="final-registry-output-safety-") as raw_tmp:
    tmp = Path(raw_tmp)

    # Invalid output/input aliasing must be rejected before the input is
    # touched. This includes the topology authority itself.
    topology = tmp / "topology.toml"
    write_topology(topology)
    topology_bytes = topology.read_bytes()
    alias_evidence = tmp / "alias-evidence.json"
    alias_evidence.write_text("stale", encoding="utf-8")
    expect_exit(
        lambda: adapter.main(
            [
                "--topology",
                str(topology),
                "--observations-out",
                str(topology),
                "--evidence-out",
                str(alias_evidence),
            ]
        ),
        "an output path that aliases an input was accepted",
    )
    require(topology.read_bytes() == topology_bytes, "aliased topology input was mutated")

    # A failure before observation must remove every valid prior output.
    early_observations = tmp / "early-observations.json"
    early_evidence = tmp / "early-evidence.json"
    early_observations.write_text("stale", encoding="utf-8")
    early_evidence.write_text("stale", encoding="utf-8")
    expect_exit(
        lambda: adapter.main(
            [
                "--topology",
                str(tmp / "missing-topology.toml"),
                "--observations-out",
                str(early_observations),
                "--evidence-out",
                str(early_evidence),
            ]
        ),
        "missing topology unexpectedly succeeded",
    )
    require(not early_observations.exists(), "early failure retained stale observations")
    require(not early_evidence.exists(), "early failure retained stale evidence")

    # Candidate validation occurs after observation but before publication.
    # A malformed candidate must therefore leave no mixed-generation outputs.
    candidate = tmp / "candidate.json"
    candidate.write_text(json.dumps({"observations": []}), encoding="utf-8")
    merged_observations = tmp / "merged-observations.json"
    merged_evidence = tmp / "merged-evidence.json"
    merged_input = tmp / "merged-input.json"
    for path in (merged_observations, merged_evidence, merged_input):
        path.write_text("stale", encoding="utf-8")

    original_observe = adapter.observe_row
    adapter.observe_row = fake_observe
    try:
        expect_exit(
            lambda: adapter.main(
                [
                    "--topology",
                    str(topology),
                    "--observations-out",
                    str(merged_observations),
                    "--evidence-out",
                    str(merged_evidence),
                    "--candidate-input",
                    str(candidate),
                    "--input-out",
                    str(merged_input),
                ]
            ),
            "malformed candidate unexpectedly succeeded",
        )
    finally:
        adapter.observe_row = original_observe
    for path in (merged_observations, merged_evidence, merged_input):
        require(not path.exists(), f"candidate failure retained stale output {path.name}")

    # If any staged write fails, earlier outputs from the same attempted run
    # are removed and no evaluator input can survive.
    write_observations = tmp / "write-observations.json"
    write_evidence = tmp / "write-evidence.json"
    original_observe = adapter.observe_row
    original_write = adapter.write_json
    calls = 0

    def fail_second_write(path, payload):
        global calls
        calls += 1
        if calls == 2:
            raise OSError("synthetic second-output failure")
        return original_write(path, payload)

    adapter.observe_row = fake_observe
    adapter.write_json = fail_second_write
    try:
        try:
            adapter.main(
                [
                    "--topology",
                    str(topology),
                    "--observations-out",
                    str(write_observations),
                    "--evidence-out",
                    str(write_evidence),
                ]
            )
        except OSError:
            pass
        else:
            raise RuntimeError("synthetic write failure unexpectedly succeeded")
    finally:
        adapter.observe_row = original_observe
        adapter.write_json = original_write
    require(not write_observations.exists(), "write failure retained observations")
    require(not write_evidence.exists(), "write failure retained evidence")

    # A complete run publishes canonical JSON and leaves no stage files.
    final_observations = tmp / "final-observations.json"
    final_evidence = tmp / "final-evidence.json"
    original_observe = adapter.observe_row
    adapter.observe_row = fake_observe
    try:
        code = adapter.main(
            [
                "--topology",
                str(topology),
                "--observations-out",
                str(final_observations),
                "--evidence-out",
                str(final_evidence),
            ]
        )
    finally:
        adapter.observe_row = original_observe
    require(code == 0, "complete output-safety run failed")
    require(len(json.loads(final_observations.read_text(encoding="utf-8"))) == 13,
            "complete run emitted the wrong observation count")
    require(json.loads(final_evidence.read_text(encoding="utf-8"))["schema_version"] == 1,
            "complete run emitted malformed evidence")
    require(not list(tmp.glob(".*.tmp")), "atomic stage file was retained")

print("final registry observation output safety: all controls passed")
"#;

#[test]
fn final_registry_preflight_provider_output_paths_are_fail_closed() -> Result<(), Box<dyn Error>> {
    let root = repo_root()?;
    let output = Command::new("python")
        .arg("-c")
        .arg(OUTPUT_SAFETY_HARNESS)
        .arg(root.join("scripts/final-registry-observation.py"))
        .current_dir(&root)
        .output()?;
    require_python_success(output, "registry observation output-safety contract")
}
