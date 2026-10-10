#!/usr/bin/env python3
"""Deterministic checksum contract tests for the topology publisher."""

from __future__ import annotations

from contextlib import redirect_stdout
import hashlib
import importlib.util
import io
import json
import os
import re
import shutil
import shlex
import subprocess
from pathlib import Path
import sys
import tempfile
import textwrap
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
        # Call absolute tool paths through functions: an executable shebang
        # cannot represent a Bash installation path containing spaces.
        tool_paths = {"python3": python, "mkdir": shutil.which("mkdir"), "cat": shutil.which("cat")}
        if not all(tool_paths.values()):
            raise AssertionError("workflow contract requires mkdir and cat")
        commands = "\n".join(
            f"{name}() {{ {shlex.quote(Path(path).as_posix())} \"$@\"; }}"
            for name, path in tool_paths.items() if path is not None
        )
        commands += '\ngit() {\ncase "$*" in\n'
        commands += '  "rev-parse HEAD^{commit}") printf "%s\\n" "$OBSERVED_COMMIT" ;;\n'
        commands += '  "rev-parse HEAD^{tree}") printf "%s\\n" "$OBSERVED_TREE" ;;\n'
        commands += '  *) return 97 ;;\nesac\n}\n'

        def invoke(script: str, name: str, *, runner: str = bash, **overrides: str) -> tuple[subprocess.CompletedProcess[str], Path]:
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
                "PATH": "",
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
                "GITHUB_OUTPUT": (directory / "outputs").as_posix(),
            }
            if os.name == "nt" and "SYSTEMROOT" in os.environ:
                env["SYSTEMROOT"] = os.environ["SYSTEMROOT"]
            env.update(overrides)
            completed = subprocess.run(
                [runner, "--noprofile", "--norc", "-c", commands + script],
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

        publishing_cases = [("clean", "false", bash), ("recovery", "true", bash)]
        if os.name == "posix":
            # Native Windows exercises its real Git Bash path above; POSIX
            # can also exercise a spaced interpreter path through a symlink.
            spaced = root / "Program Files" / "bash"
            spaced.parent.mkdir()
            spaced.symlink_to(bash)
            publishing_cases.append(("spaced-bash-recovery", "true", str(spaced)))
        for name, recovery, runner in publishing_cases:
            result, directory = invoke(
                publish, f"authorized-{name}", runner=runner, DRY_RUN="false", RECOVERY=recovery,
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

def exercise_publication_handoff() -> None:
    """Execute the downloaded-receipt → package → manifest workflow handoff.

    Cargo packaging, typed Cargo projections and Git identities are local
    instruments. The actual publisher, workflow blocks and manifest encoder
    run unchanged; an unexpected provider or process call fails the fixture.
    This mode runs in the existing release-binary contract lane, whose normal
    manifest cases also exercise the real typed Cargo authorities.
    """
    workflow = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
    names = (
        "Verify publish receipt agrees with the publish outputs",
        "Package crates for manifest checksums",
        "Generate release manifest",
    )
    scripts = {name: workflow_run_step(workflow, name) for name in names}
    bash = shutil.which("bash")
    python = shutil.which("python3")
    utilities = {name: shutil.which(name) for name in
                 ("dirname", "mkdir", "date", "sha256sum", "awk", "basename", "cat")}
    if bash is None or python is None or not all(utilities.values()):
        raise AssertionError("manifest handoff requires Bash, Python and release-shell utilities")
    commands = "\n".join(
        f'{name}() {{ {shlex.quote(Path(path).as_posix())} "$@"; }}'
        for name, path in utilities.items() if path is not None
    )
    commands += f'\npython3() {{ {shlex.quote(Path(python).as_posix())} scripts/workflow-python.py "$@"; }}\n'
    commands += 'bash() { ( source "$@" ); }\n'
    commands += 'git() {\n'
    commands += '  if [ "$*" = "rev-parse $GITHUB_SHA^{tree}" ]; then printf "%s\\n" "$FIXTURE_TREE";\n'
    commands += '  else printf "unexpected Git operation\\n" >&2; return 97; fi\n}\n'
    commit, tree = "a" * 40, "b" * 40
    authorization = "sha256:" + "c" * 64
    topology_path = ROOT / "policy/product-package-topology-v2.toml"
    topology, selected_rows = PUBLISHER.load_rows(topology_path, "cargo-allow")
    package_rows = [row for row in selected_rows if row["product_family"] == "cargo-allow"]
    package_bytes = {
        row["cargo_package_name"]: f"fixture package {row['cargo_package_name']}\n".encode()
        for row in selected_rows
    }
    digest = lambda data: "sha256:" + hashlib.sha256(data).hexdigest()
    published = {
        "schema_id": "cargo-allow.topology-publish-receipt.v1",
        "schema_version": 1,
        "mode": "cargo-allow",
        "publish": True,
        "authorization": authorization,
        "topology_id": topology["topology_id"],
        "topology_sha256": hashlib.sha256(topology_path.read_bytes()).hexdigest(),
        "cargo_lock_sha256": hashlib.sha256((ROOT / "Cargo.lock").read_bytes()).hexdigest(),
        "commit": commit,
        "tree": tree,
        "complete": True,
        "incident_state": "none",
        "first_irreversible_row": package_rows[0]["release_order"],
        "rows": [
            {
                "logical_id": row["logical_id"],
                "name": row["cargo_package_name"],
                "version": row["package_version"],
                "family": "cargo-allow",
                "release_order": row["release_order"],
                "crate": f"target/package/{row['cargo_package_name']}-{row['package_version']}.crate",
                "local_checksum": digest(package_bytes[row["cargo_package_name"]]),
                "registry_checksum": digest(package_bytes[row["cargo_package_name"]]),
                "state": "published_verified",
            }
            for row in package_rows
        ],
    }
    shim = textwrap.dedent("""
        import hashlib, importlib.util, json, os, pathlib, subprocess, sys
        assert not os.environ.get("CARGO_REGISTRY_TOKEN")
        def record(value):
            with pathlib.Path("instrument-calls.jsonl").open("a", encoding="utf-8") as out:
                out.write(json.dumps(value) + "\\n")
        def forbidden(*args, **kwargs):
            raise AssertionError("unexpected external operation")
        command = sys.argv[1:]
        if command[0] == "scripts/release-topology-publisher.py":
            spec = importlib.util.spec_from_file_location("publisher", command[0])
            publisher = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(publisher)
            _, rows = publisher.load_rows(publisher.DEFAULT_TOPOLOGY, "cargo-allow")
            publisher.git_identity = lambda kind: os.environ["FIXTURE_" + kind.upper()]
            publisher.cargo_packages = lambda: {
                row["cargo_package_name"]: {"name": row["cargo_package_name"],
                    "version": row["package_version"], "dependencies": []} for row in rows
            }
            def package(selected, packages):
                record(["package", sorted(selected)])
                if os.environ.get("FAIL_PACKAGE") == "true":
                    raise SystemExit("fixture packaging failed")
                directory = publisher.package_target_dir() / "package"
                directory.mkdir(parents=True, exist_ok=True)
                for name in selected:
                    data = ("fixture package " + name + "\\n").encode()
                    if name == os.environ.get("CORRUPT_PACKAGE"):
                        data += b"changed"
                    (directory / (name + "-" + packages[name]["version"] + ".crate")).write_bytes(data)
            publisher.package_workspace = package
            publisher.run = forbidden
            publisher.crate_api = forbidden
            publisher.registry_checksum = forbidden
            publisher.urlopen = forbidden
            if os.environ.get("PUBLISH_EXISTING") == "true":
                # Exercise the real publisher with read-only registry instruments.
                # Any attempted Cargo upload still reaches forbidden().
                os.environ["CARGO_REGISTRY_TOKEN"] = "fixture-token"
                by_name = {row["cargo_package_name"]: row for row in rows}
                def existing_checksum(name, version):
                    record(["registry", name, version])
                    row = by_name[name]
                    assert version == row["package_version"]
                    if row["product_family"] == "shared":
                        return row["expected_registry_checksum"].removeprefix("sha256:")
                    return hashlib.sha256(("fixture package " + name + "\\n").encode()).hexdigest()
                publisher.registry_checksum = existing_checksum
            sys.argv = command
            raise SystemExit(publisher.main())
        if command[0] == "-c":
            sys.argv = [command[0], *command[2:]]
            exec(compile(command[1], "checked-in workflow expression", "exec"))
        elif command[0] == "-":
            def typed_projection(args, **kwargs):
                record(args)
                if args[:7] != ["cargo", "run", "--quiet", "-p", "cargo-allow", "--locked", "--"]:
                    raise AssertionError("unexpected process in manifest encoder")
                if os.environ.get("FAIL_TYPED_PROJECTION") == "true":
                    return subprocess.CompletedProcess(args, 1, "", "fixture projection failed")
                if args[7] == "release-identity" and args[8:] == ["--version", "0.2.0"]:
                    return subprocess.CompletedProcess(args, 0, '{"result": "validated"}', "")
                if args[7] == "reconcile-package-publication":
                    assert args[args.index("--expected-checksum") + 1] == args[args.index("--observed-registry-checksum") + 1]
                    return subprocess.CompletedProcess(args, 0, '{"manifest_ready": true, "classification": "complete_exact"}', "")
                raise AssertionError("unexpected typed projection")
            subprocess.run = typed_projection
            sys.argv = command
            exec(compile(sys.stdin.read(), "checked-in manifest encoder", "exec"))
        else:
            raise AssertionError("unexpected Python entrypoint: " + repr(command))
        """)
    scenarios = 0
    with tempfile.TemporaryDirectory(prefix="release-manifest-handoff-") as temporary:
        root = Path(temporary)

        def fixture(name: str) -> tuple[Path, Path, dict[str, str]]:
            directory = root / name
            (directory / "scripts").mkdir(parents=True)
            (directory / "policy").mkdir()
            shutil.copy2(ROOT / "scripts/release-topology-publisher.py", directory / "scripts")
            shutil.copy2(ROOT / "scripts/generate-release-manifest.sh", directory / "scripts")
            shutil.copy2(topology_path, directory / "policy")
            shutil.copy2(ROOT / "Cargo.lock", directory)
            (directory / "scripts/workflow-python.py").write_text(shim, encoding="utf-8")
            evidence = directory / "target/cargo-allow"
            evidence.mkdir(parents=True)
            receipt = evidence / "topology-publish.receipt.json"
            receipt.write_text(json.dumps(published, indent=2) + "\n", encoding="utf-8")
            (evidence / "release-publish.receipt.json").write_text(json.dumps({
                "version": "0.2.0", "authorization_digest": authorization,
            }), encoding="utf-8")
            assets = evidence / "release-assets"
            assets.mkdir()
            binary = {
                "schema_version": 1, "version": "0.2.0", "tag": "v0.2.0",
                "commit": commit, "tree": tree,
                "target_triple": "x86_64-unknown-linux-gnu",
                "archive_name": "cargo-allow-v0.2.0-x86_64-unknown-linux-gnu.tar.gz",
                "archive_format": "tar.gz", "executable_name": "cargo-allow",
                "archive_sha256": "sha256:" + "d" * 64,
                "executable_sha256": "sha256:" + "e" * 64,
            }
            for filename, schema in (
                ("release-binary.receipt.json", "cargo-allow.release-binary-package.v1"),
                ("release-binary-install.receipt.json", "cargo-allow.release-binary-install.v1"),
            ):
                (assets / filename).write_text(json.dumps({
                    **binary, "schema_id": schema, "attestation_verified": True,
                }), encoding="utf-8")
            context = {
                "needs.publish.outputs.version": "0.2.0",
                "needs.publish.outputs.authorization_digest": authorization,
                "needs.publish.outputs.auth_source": "crates_io_api_token",
                "github.repository": "EffortlessMetrics/cargo-allow",
                "github.ref_name": "v0.2.0", "github.sha": commit, "github.run_id": "4427",
                "steps.publication_receipt.outputs.topology_sha256": "",
            }
            return directory, receipt, context

        def invoke(directory: Path, context: dict[str, str], name: str, **overrides: str) -> subprocess.CompletedProcess[str]:
            env = {
                "PATH": "", "FIXTURE_COMMIT": commit, "FIXTURE_TREE": tree,
                "RELEASE_VERSION": "0.2.0", "GITHUB_SHA": commit,
                "GITHUB_OUTPUT": (directory / "outputs").as_posix(),
            }
            if os.name == "nt" and "SYSTEMROOT" in os.environ:
                env["SYSTEMROOT"] = os.environ["SYSTEMROOT"]
            step = workflow.split(f"      - name: {name}\n", 1)[1].split("\n      - ", 1)[0]
            if "        env:\n" in step:
                for line in step.split("        env:\n", 1)[1].splitlines():
                    if not line.startswith("          "):
                        break
                    if line.lstrip().startswith("#"):
                        continue
                    key, value = line.strip().split(": ", 1)
                    match = re.fullmatch(r"\$\{\{ (.*?) \}\}", value)
                    env[key] = context[match[1]] if match else value.strip('"')
            env.update(overrides)
            return subprocess.run(
                [bash, "--noprofile", "--norc", "-c", commands + scripts[name]],
                cwd=directory, env=env, text=True, capture_output=True, timeout=30,
            )

        def bind(directory: Path, context: dict[str, str]) -> None:
            result = invoke(directory, context, names[0])
            assert result.returncode == 0, result.stdout + result.stderr
            outputs = directory / "outputs"
            if outputs.exists():
                values = dict(line.split("=", 1) for line in outputs.read_text().splitlines())
                context["steps.publication_receipt.outputs.topology_sha256"] = values["topology_sha256"]

        def fail_at(directory: Path, context: dict[str, str], name: str, expected: str, **overrides: str) -> None:
            result = invoke(directory, context, name, **overrides)
            diagnostic = result.stdout + result.stderr
            assert result.returncode != 0, f"{expected}: unexpectedly succeeded"
            assert expected in diagnostic, diagnostic
            assert not (directory / "target/cargo-allow/release-manifest-v2.json").exists()

        directory, receipt, context = fixture("published")
        original = receipt.read_bytes()
        bind(directory, context)
        result = invoke(directory, context, names[1])
        assert result.returncode == 0, result.stdout + result.stderr
        assert receipt.read_bytes() == original, "package step overwrote downloaded publication receipt"
        assert context["steps.publication_receipt.outputs.topology_sha256"] == digest(original)
        packaged = json.loads((directory / "target/cargo-allow/topology-repackage.receipt.json").read_text())
        assert packaged["publish"] is False and packaged["authorization"] == ""
        assert all(row["state"] == "packaged" for row in packaged["rows"])
        result = invoke(directory, context, names[2])
        assert result.returncode == 0, result.stdout + result.stderr
        manifest = json.loads((directory / "target/cargo-allow/release-manifest-v2.json").read_text())
        assert receipt.read_bytes() == original
        assert manifest["authorization_digest"] == authorization
        assert manifest["payload"]["publication_posture"] == "published"
        assert manifest["payload"]["candidate_digest"] == digest(original)
        assert manifest["payload"]["consumed_evidence"][0]["sha256"] == digest(original)
        assert [row["registry_checksum"] for row in manifest["payload"]["package_rows"]] == [
            row["registry_checksum"] for row in published["rows"]
        ]
        scenarios += 1

        # Seed a second handoff with an actual completed publisher receipt.
        # Every immutable row already exists exactly, so no upload marker exists.
        directory, receipt, context = fixture("all-verified-existing")
        package_script = scripts[names[1]]
        scripts[names[1]] = (
            "python3 scripts/release-topology-publisher.py --mode cargo-allow "
            "--publish --authorization " + authorization
            + " --receipt target/cargo-allow/topology-publish.receipt.json\n"
        )
        try:
            result = invoke(directory, context, names[1], PUBLISH_EXISTING="true")
        finally:
            scripts[names[1]] = package_script
        assert result.returncode == 0, result.stdout + result.stderr
        observed = json.loads(receipt.read_text())
        assert observed["publish"] is True and observed["complete"] is True
        assert observed["incident_state"] == "none"
        assert observed["first_irreversible_row"] is None
        assert all(row["state"] == "verified_existing" for row in observed["rows"])
        original = receipt.read_bytes()
        bind(directory, context)
        for name in names[1:]:
            result = invoke(directory, context, name)
            assert result.returncode == 0, result.stdout + result.stderr
        assert receipt.read_bytes() == original
        manifest = json.loads((directory / "target/cargo-allow/release-manifest-v2.json").read_text())
        assert manifest["payload"]["publication_posture"] == "published"
        assert manifest["payload"]["candidate_digest"] == digest(original)
        assert [row["registry_checksum"] for row in manifest["payload"]["package_rows"]] == [
            row["registry_checksum"] for row in observed["rows"]
        ]
        scenarios += 1

        directory, receipt, context = fixture("mixed-existing-and-published")
        value = json.loads(receipt.read_text())
        value["rows"][0]["state"] = "verified_existing"
        value["first_irreversible_row"] = value["rows"][1]["release_order"]
        receipt.write_text(json.dumps(value), encoding="utf-8")
        original = receipt.read_bytes()
        bind(directory, context)
        for name in names[1:]:
            result = invoke(directory, context, name)
            assert result.returncode == 0, result.stdout + result.stderr
        assert receipt.read_bytes() == original
        scenarios += 1

        # Both consumers must reject absent or malformed markers; explicit null
        # is meaningful only when every validated row was already present.
        for name, mutate in (
            ("null-marker-with-upload", lambda value: value.update(first_irreversible_row=None)),
            ("null-marker-mixed", lambda value: (
                value.update(first_irreversible_row=None),
                value["rows"][0].update(state="verified_existing"),
            )),
            ("missing-marker", lambda value: value.pop("first_irreversible_row")),
            ("zero-marker", lambda value: value.update(first_irreversible_row=0)),
            ("negative-marker", lambda value: value.update(first_irreversible_row=-1)),
            ("boolean-marker", lambda value: value.update(first_irreversible_row=True)),
            ("false-marker", lambda value: value.update(first_irreversible_row=False)),
            ("float-marker", lambda value: value.update(first_irreversible_row=1.0)),
            ("string-marker", lambda value: value.update(first_irreversible_row="100")),
            ("existing-missing-marker", lambda value: (
                value.pop("first_irreversible_row"),
                [row.update(state="verified_existing") for row in value["rows"]],
            )),
        ):
            directory, receipt, context = fixture(name)
            value = json.loads(receipt.read_text())
            mutate(value)
            receipt.write_text(json.dumps(value), encoding="utf-8")
            original = receipt.read_bytes()
            bind(directory, context)
            for consumer in names[1:]:
                fail_at(directory, context, consumer, "irreversible-row marker")
                assert receipt.read_bytes() == original
                assert not (directory / "instrument-calls.jsonl").exists()
                scenarios += 1

        directory, receipt, context = fixture("ambient-target")
        original = receipt.read_bytes()
        bind(directory, context)
        ambient_target = directory / "external-target"
        for name in names[1:]:
            result = invoke(directory, context, name, CARGO_TARGET_DIR=str(ambient_target))
            assert result.returncode == 0, result.stdout + result.stderr
        assert receipt.read_bytes() == original
        assert not ambient_target.exists(), "ambient target redirected release archive evidence"
        manifest = json.loads((directory / "target/cargo-allow/release-manifest-v2.json").read_text())
        assert manifest["payload"]["publication_posture"] == "published"
        assert manifest["payload"]["candidate_digest"] == digest(original)
        scenarios += 1

        for name, mutate, expected in (
            ("candidate", lambda value: value.update(publish=False), "publication receipt is not complete published evidence"),
            ("incomplete", lambda value: value.update(complete=False), "publication receipt is not complete published evidence"),
            ("incident", lambda value: value.update(incident_state="partial"), "publication receipt is not complete published evidence"),
            ("checksum-conflict", lambda value: value["rows"][0].update(registry_checksum="sha256:" + "0" * 64), "publication row checksum disagreement"),
            ("missing-row", lambda value: value["rows"].pop(), "publication package set differs"),
        ):
            directory, receipt, context = fixture(name)
            value = json.loads(receipt.read_text())
            mutate(value)
            receipt.write_text(json.dumps(value), encoding="utf-8")
            original = receipt.read_bytes()
            bind(directory, context)
            fail_at(directory, context, names[1], expected)
            assert receipt.read_bytes() == original
            scenarios += 1

        for name, overrides, expected in (
            ("repackaged-mismatch", {"CORRUPT_PACKAGE": package_rows[0]["cargo_package_name"]}, "repackaged bytes differ from publication"),
            ("package-failure", {"FAIL_PACKAGE": "true"}, "fixture packaging failed"),
        ):
            directory, receipt, context = fixture(name)
            original = receipt.read_bytes()
            bind(directory, context)
            fail_at(directory, context, names[1], expected, **overrides)
            assert receipt.read_bytes() == original
            scenarios += 1

        directory, receipt, context = fixture("tampered-before-package")
        bind(directory, context)
        receipt.write_bytes(receipt.read_bytes() + b"\n")
        fail_at(directory, context, names[1], "publication receipt digest differs")
        assert not (directory / "instrument-calls.jsonl").exists()
        scenarios += 1

        directory, receipt, context = fixture("tampered-before-manifest")
        bind(directory, context)
        result = invoke(directory, context, names[1])
        assert result.returncode == 0, result.stdout + result.stderr
        receipt.write_bytes(receipt.read_bytes() + b"\n")
        fail_at(directory, context, names[2], "topology receipt digest differs")
        scenarios += 1

        directory, receipt, context = fixture("candidate-at-public-entry")
        value = json.loads(receipt.read_text())
        value["publish"] = False
        receipt.write_text(json.dumps(value), encoding="utf-8")
        context["steps.publication_receipt.outputs.topology_sha256"] = digest(receipt.read_bytes())
        fail_at(directory, context, names[2], "public release requires published topology evidence")
        scenarios += 1

        directory, receipt, context = fixture("missing-publication")
        context["steps.publication_receipt.outputs.topology_sha256"] = digest(receipt.read_bytes())
        receipt.unlink()
        fail_at(directory, context, names[2], "invalid receipt")
        scenarios += 1

        for name, overrides, expected in (
            ("missing-bound-digest", {"EXPECTED_TOPOLOGY_RECEIPT_SHA256": ""}, "public release requires the downloaded topology receipt digest"),
            ("wrong-authorization", {"AUTHORIZATION_DIGEST": "sha256:" + "f" * 64}, "publication authorization differs"),
            ("missing-authorization", {"AUTHORIZATION_DIGEST": ""}, "publication authorization differs"),
            ("invalid-entry-mode", {"REQUIRE_PUBLISHED_TOPOLOGY": "unknown"}, "REQUIRE_PUBLISHED_TOPOLOGY must be true or false"),
        ):
            directory, receipt, context = fixture(name)
            context["steps.publication_receipt.outputs.topology_sha256"] = digest(receipt.read_bytes())
            fail_at(directory, context, names[2], expected, **overrides)
            scenarios += 1

        for name, expected in (
            ("archive-drift", "package archive differs from publication"),
            ("typed-instrument-failure", "not a validated typed release identity"),
        ):
            directory, receipt, context = fixture(name)
            bind(directory, context)
            result = invoke(directory, context, names[1])
            assert result.returncode == 0, result.stdout + result.stderr
            if name == "archive-drift":
                (directory / published["rows"][0]["crate"]).write_bytes(b"changed after packaging")
            overrides = {"FAIL_TYPED_PROJECTION": "true"} if name == "typed-instrument-failure" else {}
            fail_at(directory, context, names[2], expected, **overrides)
            scenarios += 1

        for name in ("same-output-path", "hard-linked-output"):
            directory, receipt, context = fixture(name)
            original = receipt.read_bytes()
            bind(directory, context)
            package_script = scripts[names[1]]
            if name == "same-output-path":
                scripts[names[1]] = package_script.replace(
                    "--receipt target/cargo-allow/topology-repackage.receipt.json",
                    "--receipt target/cargo-allow/topology-publish.receipt.json",
                )
            else:
                os.link(receipt, directory / "target/cargo-allow/topology-repackage.receipt.json")
            try:
                fail_at(directory, context, names[1], "must not overwrite the publication receipt")
                assert receipt.read_bytes() == original
                assert not (directory / "instrument-calls.jsonl").exists()
            finally:
                scripts[names[1]] = package_script
            scenarios += 1

        directory, receipt, context = fixture("explicit-candidate-manifest")
        bind(directory, context)
        result = invoke(directory, context, names[1])
        assert result.returncode == 0, result.stdout + result.stderr
        candidate = directory / "target/cargo-allow/topology-repackage.receipt.json"
        result = invoke(
            directory, context, names[2], REQUIRE_PUBLISHED_TOPOLOGY="false",
            EXPECTED_TOPOLOGY_RECEIPT_SHA256=digest(candidate.read_bytes()),
            TOPOLOGY_RECEIPT="target/cargo-allow/topology-repackage.receipt.json",
            AUTHORIZATION_DIGEST="",
        )
        assert result.returncode == 0, result.stdout + result.stderr
        manifest = json.loads((directory / "target/cargo-allow/release-manifest-v2.json").read_text())
        assert manifest["payload"]["publication_posture"] == "unpublished"
        assert "authorization_digest" not in manifest
        scenarios += 1

    print(f"release publication handoff contract: {scenarios} scenarios passed")


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
    if sys.argv[1:] == ["--manifest-handoff"]:
        exercise_publication_handoff()
    elif sys.argv[1:]:
        raise SystemExit("supported test mode: --manifest-handoff")
    else:
        main()
