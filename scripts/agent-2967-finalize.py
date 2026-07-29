from __future__ import annotations

from pathlib import Path


def replace_once(path: Path, old: str, new: str, label: str) -> None:
    text = path.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


def replace_created_date(artifact_id: str) -> None:
    path = Path(".allow/artifacts/doc-artifacts.toml")
    text = path.read_text(encoding="utf-8")
    marker = f'id = "{artifact_id}"'
    start = text.find(marker)
    if start < 0:
        raise SystemExit(f"artifact ledger missing {artifact_id}")
    end = text.find("\n[[artifact]]", start)
    if end < 0:
        end = len(text)
    block = text[start:end]
    old = 'created = "2026-07-28"'
    if block.count(old) != 1:
        raise SystemExit(f"{artifact_id}: expected one transitional created date")
    block = block.replace(old, 'created = "2026-07-29"', 1)
    path.write_text(text[:start] + block + text[end:], encoding="utf-8")


for artifact_id in ["CARGO-ALLOW-ADR-0003", "CARGO-ALLOW-SPEC-0011"]:
    replace_created_date(artifact_id)

integration = Path("crates/allow-policy/tests/three_product_design.rs")
replace_once(
    integration,
    '''    let historical = ledger
        .artifact
        .iter()
        .find(|artifact| artifact.id == "CARGO-ALLOW-SPEC-0010")
''',
    '''    let package_adr = ledger
        .artifact
        .iter()
        .find(|artifact| artifact.id == "CARGO-ALLOW-ADR-0003")
        .ok_or_else(|| "artifact ledger is missing ADR-0003".to_string())?;
    if package_adr.status != ArtifactStatus::Accepted
        || package_adr.created != "2026-07-29"
        || package_adr.linked_proposal.as_deref() != Some("CARGO-ALLOW-PROP-0010")
        || package_adr.linked_spec.as_deref() != Some("CARGO-ALLOW-SPEC-0011")
    {
        return Err(format!("unexpected package ADR lifecycle: {package_adr:?}"));
    }
    let historical = ledger
        .artifact
        .iter()
        .find(|artifact| artifact.id == "CARGO-ALLOW-SPEC-0010")
''',
    "package ADR lifecycle assertion",
)
replace_once(
    integration,
    '''    if current.status != ArtifactStatus::Accepted
        || current.linked_proposal.as_deref() != Some("CARGO-ALLOW-PROP-0010")
''',
    '''    if current.status != ArtifactStatus::Accepted
        || current.created != "2026-07-29"
        || current.linked_proposal.as_deref() != Some("CARGO-ALLOW-PROP-0010")
''',
    "current spec created-date assertion",
)

for parser_path in [
    Path("crates/intent-model/src/spec_system/support_tiers.rs"),
    Path("crates/allow-policy/src/snapshot_package/spec_system/support_tiers.rs"),
]:
    replace_once(
        parser_path,
        "support-tier claims table with Surface, Tier, Claim, Proof command or Proof or evidence, and Notes columns not found",
        "support-tier claims table with Surface, Tier, Claim, Proof command or Proof or evidence, and Notes or Limitations columns not found",
        f"support-table diagnostic in {parser_path}",
    )

compat_tests = Path("crates/allow-policy/src/spec_system/tests.rs")
text = compat_tests.read_text(encoding="utf-8")
old = "support-tier claims table with Surface, Tier, Claim, Proof command or Proof or evidence, and Notes columns not found"
new = "support-tier claims table with Surface, Tier, Claim, Proof command or Proof or evidence, and Notes or Limitations columns not found"
if text.count(old) != 1:
    raise SystemExit(f"support diagnostic compatibility expectation: expected one match, found {text.count(old)}")
compat_tests.write_text(text.replace(old, new, 1), encoding="utf-8")

spec_system = Path("crates/cargo-allow/src/spec_system.rs")
replace_once(
    spec_system,
    "add a support-tier table with Surface, Tier, Claim, Proof command or Proof or evidence, and Notes columns",
    "add a support-tier table with Surface, Tier, Claim, Proof command or Proof or evidence, and Notes or Limitations columns",
    "cargo-allow support-table suggested action",
)
