#!/usr/bin/env python3
"""Deterministic checksum contract tests for the topology publisher."""

from __future__ import annotations

from contextlib import redirect_stdout
import importlib.util
import io
import json
import os
import re
import shutil
import subprocess
from pathlib import Path
import sys
import tempfile
from typing import Any, Callable


ROOT = Path(__file__).resolve().parent.parent
PUBLISHER_PATH = ROOT / "scripts/release-topology-publisher.py"
SPEC = importlib.util.spec_from_file_location("release_topology_publisher", PUBLISHER_PATH)
if SPEC is None or SPEC.loader is None:
    raise SystemExit("could not load release topology publisher")
PUBLISHER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PUBLISHER)

DIGEST = "a" * 64
CANONICAL = f"sha256:{DIGEST}"


def expect_failure(action: Callable[[], Any]) -> None:
    try:
        action()
    except SystemExit:
        return
    raise AssertionError("expected checksum validation failure")


def shared_fixture_rows() -> list[dict[str, Any]]:
    return [
        {
            "logical_id": f"shared-{index}",
            "cargo_package_name": f"effortless-fixture-{index}",
            "package_version": "0.1.0",
            "product_family": "shared",
            "release_order": index,
        }
        for index in (80, 85, 90)
    ]


def exercise_shared_registry_preflight() -> None:
    original = PUBLISHER.registry_checksum
    original_api = PUBLISHER.crate_api
    original_package = PUBLISHER.package_crate
    original_write = PUBLISHER.write_receipt
    try:
        PUBLISHER.package_crate = lambda name, version: (
            ROOT / f"target/package/{name}-{version}.crate",
            DIGEST,
        )
        PUBLISHER.write_receipt = lambda _path, _receipt: None
        receipt = {"incident_state": "none"}
        calls: list[str] = []
        PUBLISHER.registry_checksum = lambda name, _version: calls.append(name) or DIGEST
        rows_with_expected = [
            dict(r, expected_registry_checksum=CANONICAL) for r in shared_fixture_rows()
        ]
        PUBLISHER.shared_registry_preflight(
            rows_with_expected, publish=True, receipt=receipt, receipt_path=ROOT / "unused"
        )
        assert calls == [row["cargo_package_name"] for row in shared_fixture_rows()]
        assert receipt["shared_registry_preflight_complete"] is True
        assert all(item["state"] == "already_published_exact" for item in receipt["shared_registry_preflight"])

        # A visible shared version without retained expected evidence is never
        # labeled exact (#3744 negative control 5); a rehearsal records the
        # evidence_unavailable state instead of failing.
        receipt = {"incident_state": "none"}
        PUBLISHER.shared_registry_preflight(
            shared_fixture_rows(), publish=False, receipt=receipt, receipt_path=ROOT / "unused"
        )
        assert receipt["shared_registry_preflight_complete"] is True
        assert all(item["state"] == "evidence_unavailable" for item in receipt["shared_registry_preflight"])

        # A publishing run fails closed on missing retained evidence before any
        # upload (#3744 negative control 2).
        expect_failure(
            lambda: PUBLISHER.shared_registry_preflight(
                shared_fixture_rows(), publish=True, receipt={"incident_state": "none"}, receipt_path=ROOT / "unused"
            )
        )

        # A malformed retained expected checksum is evidence_unavailable, not a
        # crash and not an exact match.
        rows_malformed_expected = [
            dict(r, expected_registry_checksum="sha256:sha256:") for r in shared_fixture_rows()
        ]
        receipt = {"incident_state": "none"}
        PUBLISHER.shared_registry_preflight(
            rows_malformed_expected, publish=False, receipt=receipt, receipt_path=ROOT / "unused"
        )
        assert all(item["state"] == "evidence_unavailable" for item in receipt["shared_registry_preflight"])
        expect_failure(
            lambda: PUBLISHER.shared_registry_preflight(
                rows_malformed_expected, publish=True, receipt={"incident_state": "none"}, receipt_path=ROOT / "unused"
            )
        )

        # Expected checksum conflict fails closed
        rows_with_conflict = [
            dict(r, expected_registry_checksum="sha256:" + ("b" * 64)) for r in shared_fixture_rows()
        ]
        expect_failure(
            lambda: PUBLISHER.shared_registry_preflight(
                rows_with_conflict, publish=True, receipt={"incident_state": "none"}, receipt_path=ROOT / "unused"
            )
        )

        PUBLISHER.registry_checksum = lambda _name, _version: None
        expect_failure(
            lambda: PUBLISHER.shared_registry_preflight(
                shared_fixture_rows(), publish=True, receipt={"incident_state": "none"}, receipt_path=ROOT / "unused"
            )
        )

        PUBLISHER.registry_checksum = original
        PUBLISHER.crate_api = lambda _name, _version: []
        expect_failure(lambda: PUBLISHER.registry_checksum("fixture", "0.1.0"))
        PUBLISHER.crate_api = lambda _name, _version: {"version": {}}
        expect_failure(lambda: PUBLISHER.registry_checksum("fixture", "0.1.0"))
        PUBLISHER.crate_api = lambda _name, _version: {
            "version": {"num": "0.1.1", "checksum": DIGEST}
        }
        expect_failure(lambda: PUBLISHER.registry_checksum("fixture", "0.1.0"))
    finally:
        PUBLISHER.registry_checksum = original
        PUBLISHER.crate_api = original_api
        PUBLISHER.package_crate = original_package
        PUBLISHER.write_receipt = original_write


def exercise_preflight_schema_contract() -> None:
    schema = json.loads(
        (ROOT / "docs/schemas/topology-publish-receipt.schema.json").read_text(
            encoding="utf-8"
        )
    )
    items = schema["properties"]["shared_registry_preflight"]["items"]
    branches = items["oneOf"]
    assert any(
        branch["properties"]["state"].get("const") == "missing"
        and branch["properties"]["registry_checksum"].get("type") == "null"
        for branch in branches
    )
    assert any(
        branch["properties"]["state"].get("enum")
        == ["already_published_exact", "checksum_conflict"]
        and branch["properties"]["registry_checksum"].get("type") == "string"
        for branch in branches
    )
    assert any(
        branch["properties"]["state"].get("const") == "evidence_unavailable"
        and branch["properties"]["registry_checksum"].get("type") == "string"
        for branch in branches
    )


def exercise_shared_topology_contract() -> None:
    """Keep the shared rehearsal bound to the four V2 package identities.

    The cargo-allow release candidate intentionally overlaps only three shared
    packages.  The standalone shared rehearsal must remain a distinct,
    complete four-package candidate, including the source-index package that
    is not part of the cargo-allow install closure.
    """
    topology, rows = PUBLISHER.load_rows(PUBLISHER.DEFAULT_TOPOLOGY, "shared")
    assert topology["topology_id"]
    assert [row["cargo_package_name"] for row in rows] == [
        "effortless-repo-protocol",
        "effortless-repo-snapshot",
        "effortless-repo-edit",
        "effortless-rust-source-index",
    ]
    assert [row["release_order"] for row in rows] == [80, 85, 90, 230]
    assert all(row["product_family"] == "shared" for row in rows)
    assert all(row["package_version"] == "0.1.0" for row in rows)

    cargo_allow_rows = PUBLISHER.load_rows(PUBLISHER.DEFAULT_TOPOLOGY, "cargo-allow")[1]
    assert "effortless-rust-source-index" not in {
        row["cargo_package_name"] for row in cargo_allow_rows
    }


def row(state: str, *, local: str, registry: str | None) -> dict[str, Any]:
    return PUBLISHER.receipt_row(
        {
            "logical_id": "fixture-package",
            "cargo_package_name": "fixture-package",
            "package_version": "9.9.9",
            "product_family": "cargo-allow",
            "release_order": 1,
        },
        crate_path=ROOT / "target/package/fixture-package-9.9.9.crate",
        local_checksum=local,
        registry_checksum=registry,
        state=state,
    )


def assert_root_schema_accepts(schema_path: Path, artifact_path: Path) -> dict[str, Any]:
    schema = json.loads(schema_path.read_text(encoding="utf-8"))
    artifact = json.loads(artifact_path.read_text(encoding="utf-8"))
    required = set(schema.get("required", []))
    missing = required - artifact.keys()
    assert not missing, f"artifact misses required schema properties: {sorted(missing)}"
    properties = schema.get("properties", {})
    if schema.get("additionalProperties") is False:
        unexpected = artifact.keys() - properties.keys()
        assert not unexpected, f"artifact has undeclared schema properties: {sorted(unexpected)}"
    for name, rules in properties.items():
        if name not in artifact:
            continue
        if "const" in rules:
            assert artifact[name] == rules["const"], f"{name} violates schema const"
        if "enum" in rules:
            assert artifact[name] in rules["enum"], f"{name} violates schema enum"
    return artifact


def exercise_main_receipt_shapes() -> None:
    original = {
        name: getattr(PUBLISHER, name)
        for name in (
            "cargo_packages",
            "git_identity",
            "load_rows",
            "package_crate",
            "package_workspace",
            "registry_checksum",
            "sha256_text",
            "validate_rows",
            "shared_registry_preflight",
        )
    }
    original_argv = sys.argv
    def fixture_rows(mode: str) -> list[dict[str, Any]]:
        if mode == "shared":
            return [
                {
                    "logical_id": f"shared-{index}",
                    "cargo_package_name": f"effortless-fixture-{index}",
                    "package_version": "0.1.0",
                    "product_family": "shared",
                    "release_order": index,
                }
                for index in range(1, 5)
            ]
        return [
            {
                "logical_id": "fixture-package",
                "cargo_package_name": "fixture-package",
                "package_version": "9.9.9",
                "product_family": "cargo-allow",
                "release_order": 1,
            }
        ]

    try:
        PUBLISHER.cargo_packages = lambda: {
            "fixture-package": {},
            **{f"effortless-fixture-{index}": {} for index in range(1, 5)},
        }
        PUBLISHER.git_identity = lambda _kind: DIGEST
        PUBLISHER.load_rows = lambda _path, mode: (
            {"topology_id": "fixture-topology"},
            fixture_rows(mode),
        )
        PUBLISHER.package_crate = lambda name, version: (
            ROOT / f"target/package/{name}-{version}.crate",
            DIGEST,
        )
        PUBLISHER.package_workspace = lambda _selected, _packages: None
        PUBLISHER.registry_checksum = lambda _name, _version: None
        PUBLISHER.sha256_text = lambda _path: DIGEST
        PUBLISHER.validate_rows = lambda _rows, _packages: None
        PUBLISHER.shared_registry_preflight = lambda *_args, **_kwargs: None

        with tempfile.TemporaryDirectory() as directory:
            receipt = Path(directory) / "topology.json"
            sys.argv = [
                str(PUBLISHER_PATH),
                "--mode",
                "cargo-allow",
                "--receipt",
                str(receipt),
            ]
            with redirect_stdout(io.StringIO()):
                assert PUBLISHER.main() == 0
            topology = assert_root_schema_accepts(
                ROOT / "docs/schemas/topology-publish-receipt.schema.json", receipt
            )
            assert "package_only" not in topology

            sys.argv = [
                str(PUBLISHER_PATH),
                "--mode",
                "shared",
                "--package-only",
                "--receipt",
                str(receipt),
            ]
            with redirect_stdout(io.StringIO()):
                assert PUBLISHER.main() == 0
            shared = assert_root_schema_accepts(
                ROOT / "docs/schemas/shared-package-candidate.v1.schema.json", receipt
            )
            assert shared["package_only"] is True
    finally:
        sys.argv = original_argv
        for name, value in original.items():
            setattr(PUBLISHER, name, value)


def exercise_cargo_allow_checksum_equality() -> None:
    original = {
        name: getattr(PUBLISHER, name)
        for name in (
            "cargo_packages",
            "git_identity",
            "load_rows",
            "package_crate",
            "package_workspace",
            "registry_checksum",
            "sha256_text",
            "validate_rows",
            "shared_registry_preflight",
            "run",
            "wait_for_checksum",
        )
    }
    original_argv = sys.argv
    try:
        PUBLISHER.cargo_packages = lambda: {"cargo-allow": {}}
        PUBLISHER.git_identity = lambda _kind: DIGEST
        PUBLISHER.load_rows = lambda _path, mode: (
            {"topology_id": "fixture-topology"},
            [
                {
                    "logical_id": "cargo-allow",
                    "cargo_package_name": "cargo-allow",
                    "package_version": "0.2.0-rc.1",
                    "product_family": "cargo-allow",
                    "release_order": 100,
                }
            ],
        )
        PUBLISHER.package_crate = lambda name, version: (
            ROOT / f"target/package/{name}-{version}.crate",
            DIGEST,
        )
        PUBLISHER.package_workspace = lambda _selected, _packages: None
        PUBLISHER.sha256_text = lambda _path: DIGEST
        PUBLISHER.validate_rows = lambda _rows, _packages: None
        PUBLISHER.shared_registry_preflight = lambda *_args, **_kwargs: None
        PUBLISHER.run = lambda *args, **kwargs: ""

        # Existing row with matching checksum -> verified_existing
        PUBLISHER.registry_checksum = lambda _name, _version: DIGEST
        with tempfile.TemporaryDirectory() as directory:
            receipt = Path(directory) / "topology.json"
            sys.argv = [
                str(PUBLISHER_PATH),
                "--mode",
                "cargo-allow",
                "--receipt",
                str(receipt),
            ]
            with redirect_stdout(io.StringIO()):
                assert PUBLISHER.main() == 0
            data = json.loads(receipt.read_text(encoding="utf-8"))
            assert data["rows"][0]["state"] == "verified_existing"

        # Existing row with conflicting checksum -> fails closed
        PUBLISHER.registry_checksum = lambda _name, _version: "b" * 64
        with tempfile.TemporaryDirectory() as directory:
            receipt = Path(directory) / "topology.json"
            sys.argv = [
                str(PUBLISHER_PATH),
                "--mode",
                "cargo-allow",
                "--receipt",
                str(receipt),
            ]
            expect_failure(lambda: PUBLISHER.main())

        # Newly published row with conflicting post-upload checksum -> fails closed.
        # A fixture token satisfies the publisher's upload gate so the upload seam
        # (first irreversible row, stubbed publish, post-upload checksum
        # verification) genuinely executes; run() and wait_for_checksum are
        # stubbed below, so nothing leaves the machine.
        PUBLISHER.registry_checksum = lambda _name, _version: None
        PUBLISHER.wait_for_checksum = lambda _name, _version: "b" * 64
        os.environ["CARGO_REGISTRY_TOKEN"] = "fixture-token"
        try:
            with tempfile.TemporaryDirectory() as directory:
                receipt = Path(directory) / "topology.json"
                sys.argv = [
                    str(PUBLISHER_PATH),
                    "--mode",
                    "cargo-allow",
                    "--publish",
                    "--authorization",
                    "issue:3760",
                    "--receipt",
                    str(receipt),
                ]
                expect_failure(lambda: PUBLISHER.main())
                data = json.loads(receipt.read_text(encoding="utf-8"))
                assert data["first_irreversible_row"] == 100
                assert data["incident_state"] == "partial"
        finally:
            os.environ.pop("CARGO_REGISTRY_TOKEN", None)
    finally:
        sys.argv = original_argv
        for name, value in original.items():
            setattr(PUBLISHER, name, value)


def exercise_package_artifact_directory() -> None:
    original_root = PUBLISHER.ROOT
    original_run = PUBLISHER.run
    original_target = os.environ.get("CARGO_TARGET_DIR")
    try:
        with tempfile.TemporaryDirectory() as directory:
            fixture = Path(directory).resolve()
            PUBLISHER.ROOT = fixture / "subject"
            local_package = PUBLISHER.ROOT / "target/package/fixture-package-9.9.9.crate"
            local_package.parent.mkdir(parents=True)
            local_package.write_bytes(b"stale local archive")
            external_target = fixture / "external-target"
            os.environ["CARGO_TARGET_DIR"] = str(external_target)
            commands: list[list[str]] = []

            def package(command: list[str]) -> str:
                commands.append(command)
                target = external_target
                if "--target-dir" in command:
                    target = Path(command[command.index("--target-dir") + 1])
                archive = target / "package/fixture-package-9.9.9.crate"
                archive.parent.mkdir(parents=True, exist_ok=True)
                archive.write_bytes(b"fresh candidate archive")
                return ""

            PUBLISHER.run = package
            PUBLISHER.package_workspace({"fixture-package"}, {"fixture-package": {}, "other": {}})
            archive, digest = PUBLISHER.package_crate("fixture-package", "9.9.9")
            if archive != local_package or archive.read_bytes() != b"fresh candidate archive":
                raise RuntimeError("publisher read stale bytes after packaging to another target")
            if digest != PUBLISHER.sha256_file(local_package):
                raise RuntimeError("publisher checksum does not bind the fresh archive")
            if external_target.exists():
                raise RuntimeError("ambient target redirected the publisher's package artifacts")
            if len(commands) != 1 or commands[0][-2:] != ["--exclude", "other"]:
                raise RuntimeError("package selection changed while binding the output directory")
            local_package.unlink()
            expect_failure(lambda: PUBLISHER.package_crate("fixture-package", "9.9.9"))
    finally:
        PUBLISHER.ROOT = original_root
        PUBLISHER.run = original_run
        if original_target is None:
            os.environ.pop("CARGO_TARGET_DIR", None)
        else:
            os.environ["CARGO_TARGET_DIR"] = original_target



def workflow_run_step(workflow: str, name: str) -> str:
    """Read the actual checked-in shell block, without a second workflow model."""
    marker = f"      - name: {name}\n"
    if workflow.count(marker) != 1:
        raise AssertionError(f"expected one workflow step named {name!r}")
    step = workflow.split(marker, 1)[1].split("\n      - ", 1)[0]
    run_marker = "        run: |\n"
    if step.count(run_marker) != 1:
        raise AssertionError(f"expected one literal run block for {name!r}")
    lines = step.split(run_marker, 1)[1].splitlines()
    script = []
    for line in lines:
        if not line.strip():
            script.append("")
        elif line.startswith("          "):
            script.append(line[10:])
        else:
            break
    if not script:
        raise AssertionError(f"empty run block for {name!r}")
    return "\n".join(script) + "\n"


def exercise_workflow_dispatch(workflow: str | None = None) -> None:
    """Execute rehearsal/publication branches with no credential or real publisher."""
    if workflow is None:
        workflow = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
    bash = shutil.which("bash")
    python = shutil.which("python3")
    if bash is None or python is None:
        raise AssertionError("workflow contract requires bash and python3")
    authorize = workflow_run_step(workflow, "Validate release authorization before token access")
    publish = workflow_run_step(workflow, "Publish cargo-allow topology rows in dependency order")
    context = {
        "needs.preflight.outputs.version": "0.2.0",
        "needs.preflight.outputs.tag": "",
        "needs.preflight.outputs.commit": "a" * 40,
        "needs.preflight.outputs.tree": "b" * 40,
        "github.run_id": "4423",
    }
    for key, value in context.items():
        authorize = authorize.replace("${{ " + key + " }}", value)
    authorize = authorize.replace("${{ needs.preflight.outputs.recovery }}", "${RECOVERY}")
    if re.search(r"\$\{\{", authorize + publish):
        raise AssertionError("workflow contract has an unbound expression")

    scenarios = 0
    with tempfile.TemporaryDirectory(prefix="release-workflow-dispatch-") as temporary:
        root = Path(temporary)
        bin_dir = root / "bin"
        bin_dir.mkdir()
        # All external tools used by these branches are local stand-ins. No
        # ambient credentials or environment are forwarded to the shell.
        (bin_dir / "python3").symlink_to(python)
        (bin_dir / "mkdir").symlink_to(shutil.which("mkdir") or "/bin/mkdir")
        (bin_dir / "cat").symlink_to(shutil.which("cat") or "/bin/cat")
        git = bin_dir / "git"
        git.write_text(
            '#!' + bash + '\ncase "$*" in\n'
            '  "rev-parse HEAD^{commit}") printf "%s\\n" "$OBSERVED_COMMIT" ;;\n'
            '  "rev-parse HEAD^{tree}") printf "%s\\n" "$OBSERVED_TREE" ;;\n'
            '  *) exit 97 ;;\nesac\n', encoding="utf-8"
        )
        git.chmod(0o755)

        def invoke(script: str, name: str, **overrides: str) -> tuple[subprocess.CompletedProcess[str], Path]:
            directory = root / name
            directory.mkdir()
            scripts = directory / "scripts"
            scripts.mkdir()
            (scripts / "release-topology-publisher.py").write_text(
                "import json, os, pathlib, sys\n"
                "assert not os.environ.get('CARGO_REGISTRY_TOKEN')\n"
                "path = pathlib.Path('publisher-calls.jsonl')\n"
                "with path.open('a', encoding='utf-8') as stream:\n"
                "    stream.write(json.dumps(sys.argv[1:]) + '\\n')\n",
                encoding="utf-8",
            )
            env = {
                "PATH": str(bin_dir),
                "EVENT": "workflow_dispatch",
                "TAG": "",
                "VERSION": "0.2.0",
                "COMMIT": "a" * 40,
                "TREE": "b" * 40,
                "RECOVERY": "false",
                "DRY_RUN": "true",
                "RELEASE_VERSION": "0.2.0",
                "RELEASE_COMMIT": "a" * 40,
                "RELEASE_TREE": "b" * 40,
                "OBSERVED_COMMIT": "a" * 40,
                "OBSERVED_TREE": "b" * 40,
                "GATE_AUTHORIZATION_DIGEST": "",
                "RECOVERY_AUTHORIZATION": "",
                "RECOVERY_RECEIPT": "incident-receipt.json",
                "GITHUB_OUTPUT": str(directory / "outputs"),
            }
            env.update(overrides)
            completed = subprocess.run(
                [bash, "--noprofile", "--norc", "-c", script],
                cwd=directory, env=env, text=True, capture_output=True, timeout=10,
            )
            return completed, directory

        for recovery in ("false", "true"):
            result, directory = invoke(authorize, f"identity-{recovery}", RECOVERY=recovery)
            assert result.returncode == 0, result.stdout + result.stderr
            identity = json.loads((directory / "target/cargo-allow/release-operation-identity.json").read_text())
            assert identity["commit"] == "a" * 40 and identity["tree"] == "b" * 40
            assert identity["version"] == "0.2.0" and identity["authorization_digest"] == ""
            assert identity["recovery"] is (recovery == "true")
            outputs = dict(line.split("=", 1) for line in (directory / "outputs").read_text().splitlines())
            assert outputs["valid"] == "false" and outputs["authorization_digest"] == ""
            assert outputs["rehearsal"] == ("false" if recovery == "true" else "true")
            assert outputs["recovery"] == recovery
            assert not (directory / "publisher-calls.jsonl").exists()
            scenarios += 1

        result, directory = invoke(publish, "dry-run")
        assert result.returncode == 0, result.stdout + result.stderr
        calls = [json.loads(line) for line in (directory / "publisher-calls.jsonl").read_text().splitlines()]
        assert calls == [["--mode", "cargo-allow", "--receipt", "target/cargo-allow/topology-publish.receipt.json"]]
        scenarios += 1

        for name, overrides in (
            ("clean-without-authority", {"DRY_RUN": "false"}),
            ("recovery-without-authority", {"DRY_RUN": "false", "RECOVERY": "true", "GATE_AUTHORIZATION_DIGEST": CANONICAL}),
            ("missing-version", {"RELEASE_VERSION": ""}),
            ("recovery-wrong-commit", {"DRY_RUN": "false", "RECOVERY": "true", "RECOVERY_AUTHORIZATION": "incident:4423", "OBSERVED_COMMIT": "c" * 40}),
            ("recovery-wrong-tree", {"DRY_RUN": "false", "RECOVERY": "true", "RECOVERY_AUTHORIZATION": "incident:4423", "OBSERVED_TREE": "c" * 40}),
        ):
            result, directory = invoke(publish, name, **overrides)
            assert result.returncode != 0, f"{name} unexpectedly succeeded"
            assert not (directory / "publisher-calls.jsonl").exists(), f"{name} reached publisher"
            scenarios += 1

        for recovery in ("false", "true"):
            result, directory = invoke(
                publish, f"authorized-{recovery}", DRY_RUN="false", RECOVERY=recovery,
                GATE_AUTHORIZATION_DIGEST=CANONICAL, RECOVERY_AUTHORIZATION="incident:4423",
            )
            assert result.returncode == 0, result.stdout + result.stderr
            calls = [json.loads(line) for line in (directory / "publisher-calls.jsonl").read_text().splitlines()]
            expected = ["--mode", "cargo-allow", "--publish", "--authorization", "incident:4423" if recovery == "true" else CANONICAL,
                        "--receipt", "target/cargo-allow/topology-publish.receipt.json"]
            if recovery == "true":
                expected += ["--recovery-receipt", "incident-receipt.json"]
            assert calls == [expected]
            scenarios += 1
    print(f"release workflow dispatch contract: {scenarios} scenarios passed")

def main() -> None:
    assert PUBLISHER.receipt_checksum(DIGEST, field="fresh local checksum") == CANONICAL
    assert PUBLISHER.receipt_checksum(CANONICAL, field="published registry checksum") == CANONICAL
    assert PUBLISHER.receipt_checksum(None, field="missing registry checksum") is None

    fresh = row("missing", local=DIGEST, registry=None)
    published = row("published_verified", local=DIGEST, registry=DIGEST)
    recovered = row("recovered_already_published_exact", local=CANONICAL, registry=CANONICAL)
    for receipt_row in (fresh, published, recovered):
        assert receipt_row["local_checksum"] == CANONICAL
        if receipt_row["registry_checksum"] is not None:
            assert receipt_row["registry_checksum"] == CANONICAL

    accepted = PUBLISHER.recovery_rows({"rows": [recovered]})
    assert accepted[("fixture-package", "9.9.9")]["local_checksum"] == CANONICAL
    prior = dict(recovered)
    prior["state"] = "published_verified"
    assert PUBLISHER.recovery_row_is_exact(prior, CANONICAL)

    for registry_checksum in (None, "sha256:" + ("b" * 64)):
        incomplete = dict(prior)
        incomplete["registry_checksum"] = registry_checksum
        assert not PUBLISHER.recovery_row_is_exact(incomplete, CANONICAL)

    for malformed in (
        DIGEST,
        "sha256:" + ("A" * 64),
        "sha256:" + ("a" * 63),
        "sha256:sha256:" + DIGEST,
    ):
        invalid = dict(recovered)
        invalid["local_checksum"] = malformed
        expect_failure(lambda invalid=invalid: PUBLISHER.recovery_rows({"rows": [invalid]}))

    exercise_workflow_dispatch()
    exercise_package_artifact_directory()
    exercise_main_receipt_shapes()
    exercise_shared_registry_preflight()
    exercise_preflight_schema_contract()
    exercise_shared_topology_contract()
    exercise_cargo_allow_checksum_equality()
    source = PUBLISHER_PATH.read_text(encoding="utf-8")
    main_start = source.index("def main()")
    assert source.index("shared_registry_preflight(", main_start) < source.index(
        'run(["cargo", "publish"', main_start
    )

    print("topology publisher checksum contract: passed")


if __name__ == "__main__":
    main()
