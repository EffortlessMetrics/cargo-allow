#!/usr/bin/env python3
"""Validation contract for Gemini CLI context and agent operating surface (#3731)."""

import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent


def test_gemini_settings() -> None:
    settings_file = ROOT / ".gemini" / "settings.json"
    assert settings_file.is_file(), f"missing settings file: {settings_file}"
    raw = json.loads(settings_file.read_text(encoding="utf-8"))
    assert "$schema" in raw, "missing $schema key in .gemini/settings.json"
    assert "context" in raw, "missing context key in .gemini/settings.json"
    context = raw["context"]
    assert context.get("fileName") == "GEMINI.md", "context.fileName must be GEMINI.md"
    file_filtering = context.get("fileFiltering", {})
    assert file_filtering.get("respectGitIgnore") is True, "must respect .gitignore"
    assert file_filtering.get("respectGeminiIgnore") is True, "must respect .geminiignore"
    
    # Negative control: no credentials, yolo mode, or unknown execution bypasses
    forbidden_keys = {"apiKey", "token", "yolo", "autoApprove", "model"}
    for key in forbidden_keys:
        assert key not in raw, f"forbidden key {key} in .gemini/settings.json"
        assert key not in context, f"forbidden key {key} in context block"


def test_gemini_ignore() -> None:
    ignore_file = ROOT / ".geminiignore"
    assert ignore_file.is_file(), f"missing ignore file: {ignore_file}"
    content = ignore_file.read_text(encoding="utf-8")
    assert "target/" in content, ".geminiignore must ignore target/"
    assert ".system_generated/" in content, ".geminiignore must ignore .system_generated/"
    
    # Negative control: must not ignore essential repository roots
    for critical in ("AGENTS.md", "CLAUDE.md", "policy/", "crates/", "docs/", "scripts/"):
        assert critical not in content, f".geminiignore must not ignore {critical}"


def test_gemini_md_imports() -> None:
    gemini_md = ROOT / "GEMINI.md"
    assert gemini_md.is_file(), f"missing GEMINI.md: {gemini_md}"
    content = gemini_md.read_text(encoding="utf-8")
    
    expected_imports = [
        "@./AGENTS.md",
        "@./CLAUDE.md",
        "@./docs/campaigns/cargo-allow-0.2.0.md",
    ]
    for imp in expected_imports:
        assert imp in content, f"GEMINI.md must import {imp}"
        rel_path = imp.removeprefix("@./")
        target = ROOT / rel_path
        assert target.is_file(), f"imported target does not exist: {target}"
    
    # Ensure operating guidelines and boundaries are present
    assert "#3768" in content, "GEMINI.md must reference #3768 campaign controller"
    assert "/memory reload" in content, "GEMINI.md must mention memory reload command"
    assert "/skills reload" in content, "GEMINI.md must mention skills reload command"
    assert "review-current-head" in content, "GEMINI.md must reference review-current-head skill"
    assert "cargo-allow-0.2-campaign" in content, "GEMINI.md must reference cargo-allow-0.2-campaign skill"
    assert "v0.2.0-rc.1" in content, "GEMINI.md must mention rc.1 immutability"


def test_agents_md_routing_and_lanes() -> None:
    agents_md = ROOT / "AGENTS.md"
    assert agents_md.is_file(), f"missing AGENTS.md: {agents_md}"
    content = agents_md.read_text(encoding="utf-8")
    
    # Check #3768 priorities
    assert "#3768" in content, "AGENTS.md must reference #3768 priority train"
    assert "0. Agent context, skill, and readiness controls" in content
    assert "1. RC.1 external reconciliation" in content
    assert "2. Release safety kernel" in content
    assert "3. Installed usability and pilots" in content
    assert "4. Candidate preparation, verification, and CI economy" in content
    assert "5. Final 0.2.0 candidate refreeze" in content
    assert "6. Hard STOP for separate explicit release authorization" in content
    assert "7. Final release execution and closeout only under #2502 authority" in content
    
    # Check session lane classes
    for lane in (
        "ReversibleImplementation",
        "ReadOnlyReview",
        "ExternalObservation",
        "RootDecision",
        "IrreversibleOperation",
        "BlockedOrStale",
    ):
        assert lane in content, f"AGENTS.md must define lane {lane}"
    
    # Check release immutability law
    assert "Release Immutability Law" in content, "AGENTS.md must define Release Immutability Law"
    assert "never delete, move, retag, or overwrite" in content


def test_campaign_document() -> None:
    doc = ROOT / "docs" / "campaigns" / "cargo-allow-0.2.0.md"
    assert doc.is_file(), f"missing campaign document: {doc}"
    content = doc.read_text(encoding="utf-8")
    assert "#3768" in content, "campaign document must link #3768 controller"
    assert "cargo-allow 0.2.0" in content
    assert "0.2.0-rc.1" in content
    assert "0.1.11" in content
    assert "0.1.0" in content
    assert "#3731" in content
    assert "#3770" in content
    assert "#3747" in content


def test_pull_request_template() -> None:
    template = ROOT / ".github" / "PULL_REQUEST_TEMPLATE.md"
    assert template.is_file(), f"missing PR template: {template}"
    content = template.read_text(encoding="utf-8")
    
    required_fields = [
        "#3768 campaign rail / controlling child",
        "Execution role: author | reviewer | observer | reconciliation",
        "Predecessor evidence consumed",
        "Current base / head / merge-base",
        "Scope and non-goals",
        "Changed seams and semantic owners",
        "Highest-risk false-green route",
        "Negative controls",
        "External state observed",
        "Incident / recovery lineage",
        "Irreversible actions performed",
        "Claim boundary",
        "Post-merge child / controller handoff",
    ]
    for field in required_fields:
        assert field in content, f"PR template missing required field: {field}"


def main() -> int:
    test_gemini_settings()
    test_gemini_ignore()
    test_gemini_md_imports()
    test_agents_md_routing_and_lanes()
    test_campaign_document()
    test_pull_request_template()
    print("agent context contract tests: passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
