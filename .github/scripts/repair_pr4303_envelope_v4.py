from __future__ import annotations

import re
import runpy
from pathlib import Path


def normalize_once(text: str, pattern: str, replacement: str, label: str) -> str:
    updated, count = re.subn(pattern, replacement, text, count=1, flags=re.MULTILINE)
    if count != 1:
        raise SystemExit(f"expected one {label} match, found {count}")
    return updated


authority = Path(
    "crates/allow-report/src/artifacts/release_operation_authority_v1.rs"
)
text = authority.read_text(encoding="utf-8")

# The v3 closure intentionally matches exact source text. Normalize the two
# indentation-only variants first; Rust semantics are indentation-insensitive
# and the closure runs cargo fmt before proof and publication.
text = normalize_once(
    text,
    r'^(?P<indent>[ \t]+)if event\.authority_class != identity\.authority_kind \{\n'
    r'(?P=indent)    return Err\("operation event authority class does not match the immutable operation"\);\n'
    r'(?P=indent)\}$',
    'if event.authority_class != identity.authority_kind {\n'
    '    return Err("operation event authority class does not match the immutable operation");\n'
    '}',
    "authority-class indentation",
)
text = normalize_once(
    text,
    r'^(?P<indent>[ \t]+)Event::AuthorizationSelected => \{\n'
    r'(?P=indent)    let ready = match identity\.operation_class \{$',
    'Event::AuthorizationSelected => {\n'
    '    let ready = match identity.operation_class {',
    "authorization-selection indentation",
)

authority.write_text(text, encoding="utf-8", newline="\n")
runpy.run_path(
    ".github/scripts/repair_pr4303_envelope_v3.py",
    run_name="__main__",
)
