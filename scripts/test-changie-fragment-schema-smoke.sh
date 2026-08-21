#!/usr/bin/env bash
# Independent-validation smoke for the config-derived fragment JSON
# Schema (#3613). Validates the emitted schema with an independent JSON
# Schema validator (python jsonschema when available) and checks the
# association descriptor shape for YAML Language Server compatibility.
# No Go, Aqua, Changie, network, or process beyond cargo itself.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

work="${ROOT}/target/changie-fragment-schema-smoke"
rm -rf "${work}"
mkdir -p "${work}"

cargo run -q -p cargo-allow --locked -- changie schema --fragments \
  --output "${work}/fragment.schema.json"
cargo run -q -p cargo-allow --locked -- changie schema --fragments --association \
  --output "${work}/association.json"

python3 - "${work}/fragment.schema.json" "${work}/association.json" <<'PY'
import json
import sys
from pathlib import Path

schema = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
association = json.loads(Path(sys.argv[2]).read_text(encoding="utf-8"))

# The schema must be structurally valid JSON Schema (meta-shape check that
# does not require the jsonschema package).
assert schema["$schema"].endswith("draft/2020-12/schema"), schema["$schema"]
assert schema["type"] == "object"
assert "properties" in schema
assert "kind" in schema["properties"]
# Independent validation when jsonschema is present.
try:
    import jsonschema
    jsonschema.Draft202012Validator.check_schema(schema)
    validator = jsonschema.Draft202012Validator(schema)
    valid = {"kind": "Fixed", "body": "text"}
    assert not list(validator.iter_errors(valid)), list(validator.iter_errors(valid))
except ImportError:
    print("jsonschema unavailable; structural checks only")

# Association shape for yaml-language-server style binding.
for field in (
    "schema",
    "compatibility_generation",
    "config_path",
    "schema_digest",
    "fragment_path_patterns",
    "source_subject",
    "completeness",
):
    assert field in association, f"association missing {field}"
assert association["fragment_path_patterns"], "no fragment patterns"
assert association["config_path"].startswith(".changie"), association["config_path"]
print("fragment schema smoke passed")
PY
