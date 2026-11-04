#!/usr/bin/env python3
"""Bounded reader for the typed cargo-allow release-identity projection (#3787).

Extracts exact fields from one `cargo-allow release-identity` JSON projection
and binds caller-supplied expected values to them. This module owns no
release grammar, channel inference, tag reconstruction, or path selection:
it validates the projection envelope (schema identity and result), checks
the JSON type of each requested field, compares any expected values for
exact equality, and prints the requested fields as KEY=VALUE lines.

Every defect fails closed with one stderr diagnostic and a non-zero exit:
an unreadable file, invalid JSON, a foreign schema generation, a
non-validated result, a missing or mistyped required field, or an
expected-value disagreement. There is no default value, fallback file, or
inference. Schema changes in the producer break this reader loudly instead
of silently degrading to old parser behavior (#3787 closeout).

Usage:
    python3 scripts/release_identity_projection.py PROJECTION_FILE \
        --require version,tag,github_prerelease \
        [--expect version=0.2.0] [--expect tag=v0.2.0]

Fields and required JSON types:
    version           str (non-empty)
    tag               str (non-empty)
    channel           str (non-empty)
    rc_ordinal        integer or null (emitted empty for null)
    github_prerelease Boolean (emitted as true/false)
    precedence        object; emitted as flat precedence.major, precedence.minor,
                      precedence.patch (integers), precedence.is_stable
                      (true/false), precedence.rc_ordinal (integer or empty)

Expected-value comparison uses the same emitted text form, so an expected
binding is an exact comparison against the projected canonical field.
"""

from __future__ import annotations

import json
import sys

SCHEMA_ID = "cargo-allow.release-identity.v1"
VALIDATED_RESULT = "validated"

STRING_FIELDS = ("version", "tag", "channel")
BOOLEAN_FIELDS = ("github_prerelease",)
OPTIONAL_INTEGER_FIELDS = ("rc_ordinal",)
PRECEDENCE_INTEGER_FIELDS = ("major", "minor", "patch")
PRECEDENCE_BOOLEAN_FIELDS = ("is_stable",)
PRECEDENCE_OPTIONAL_INTEGER_FIELDS = ("rc_ordinal",)
KNOWN_FIELDS = (
    *STRING_FIELDS,
    *BOOLEAN_FIELDS,
    *OPTIONAL_INTEGER_FIELDS,
    "precedence",
)


class ReaderFailure(Exception):
    """One fail-closed reader diagnostic; never carries a fallback value."""


def _fail(message: str) -> "ReaderFailure":
    return ReaderFailure(f"release-identity-projection: {message}")


def _field_text(payload: dict, field: str) -> str:
    """Exact text form of one already type-checked projected field."""
    value = payload[field]
    if field in BOOLEAN_FIELDS:
        return "true" if value else "false"
    if field in OPTIONAL_INTEGER_FIELDS:
        return "" if value is None else str(value)
    return value


def load_projection(path: str) -> dict:
    try:
        raw = open(path, "rb").read()
    except OSError as error:
        raise _fail(f"cannot read projection {path}: {error}") from error
    try:
        payload = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise _fail(f"projection {path} is not valid JSON: {error}") from error
    if not isinstance(payload, dict):
        raise _fail(f"projection {path} is not a JSON object")
    if payload.get("schema") != SCHEMA_ID:
        raise _fail(
            f"projection {path} has unsupported schema "
            f"{payload.get('schema')!r}; expected {SCHEMA_ID} "
            "(no fallback for foreign projection generations)"
        )
    if payload.get("result") != VALIDATED_RESULT:
        raise _fail(
            f"projection {path} is not a validated result "
            f"(result={payload.get('result')!r})"
        )
    return payload


def require_fields(payload: dict, fields: list[str]) -> None:
    for field in fields:
        if field == "precedence":
            precedence = payload.get("precedence")
            if not isinstance(precedence, dict):
                raise _fail("projected field precedence is missing or not an object")
            for sub in PRECEDENCE_INTEGER_FIELDS:
                value = precedence.get(sub)
                if type(value) is not int:
                    raise _fail(f"projected field precedence.{sub} is not an integer")
            for sub in PRECEDENCE_BOOLEAN_FIELDS:
                if type(precedence.get(sub)) is not bool:
                    raise _fail(f"projected field precedence.{sub} is not a Boolean")
            for sub in PRECEDENCE_OPTIONAL_INTEGER_FIELDS:
                value = precedence.get(sub)
                if value is not None and type(value) is not int:
                    raise _fail(
                        f"projected field precedence.{sub} is neither an integer nor null"
                    )
            continue
        if field not in KNOWN_FIELDS:
            raise _fail(f"unknown projected field {field!r}")
        if field not in payload:
            raise _fail(f"projected field {field} is missing")
        value = payload[field]
        if field in STRING_FIELDS:
            if not isinstance(value, str) or not value:
                raise _fail(f"projected field {field} is not a non-empty string")
        elif field in BOOLEAN_FIELDS:
            if type(value) is not bool:
                raise _fail(f"projected field {field} is not a Boolean")
        else:
            if value is not None and type(value) is not int:
                raise _fail(f"projected field {field} is neither an integer nor null")


def bind_expected(payload: dict, expected: list[str]) -> None:
    for binding in expected:
        field, separator, value = binding.partition("=")
        if not separator:
            raise _fail(f"malformed --expect binding {binding!r}; expected FIELD=VALUE")
        if field == "precedence":
            raise _fail(
                "precedence is a nested projection object; it can be required "
                "and emitted but not expectation-bound"
            )
        if field not in KNOWN_FIELDS:
            raise _fail(f"unknown projected field {field!r} in --expect")
        if field not in payload:
            raise _fail(f"projected field {field} is missing for --expect")
        projected = _field_text(payload, field)
        if projected != value:
            raise _fail(
                f"projected field {field} is {projected!r}; caller expected {value!r}"
            )


def emit(payload: dict, fields: list[str]) -> None:
    for field in fields:
        if field == "precedence":
            precedence = payload["precedence"]
            for sub in (
                *PRECEDENCE_INTEGER_FIELDS,
                *PRECEDENCE_BOOLEAN_FIELDS,
                *PRECEDENCE_OPTIONAL_INTEGER_FIELDS,
            ):
                value = precedence[sub]
                text = (
                    "true"
                    if value is True
                    else "false"
                    if value is False
                    else "" if value is None else str(value)
                )
                print(f"precedence.{sub}={text}")
            continue
        print(f"{field}={_field_text(payload, field)}")


def main(argv: list[str]) -> int:
    usage = (
        "usage: release_identity_projection.py FILE --require F1[,F2] "
        "[--expect FIELD=VALUE]..."
    )
    arguments = argv[1:]
    if not arguments or arguments[0].startswith("--"):
        print(usage, file=sys.stderr)
        return 2
    path = arguments[0]
    required: list[str] = []
    expected: list[str] = []
    rest = arguments[1:]
    index = 0
    while index < len(rest):
        option = rest[index]
        if option == "--require" and index + 1 < len(rest):
            required.extend(
                field.strip() for field in rest[index + 1].split(",") if field.strip()
            )
            index += 2
        elif option == "--expect" and index + 1 < len(rest):
            expected.append(rest[index + 1])
            index += 2
        else:
            print(
                f"release-identity-projection: unknown argument {option!r}",
                file=sys.stderr,
            )
            return 2
    if not required:
        print(
            "release-identity-projection: --require with at least one field is required",
            file=sys.stderr,
        )
        return 2
    if len(set(required)) != len(required):
        print(
            "release-identity-projection: duplicate field in --require",
            file=sys.stderr,
        )
        return 2
    try:
        payload = load_projection(path)
        # An --expect binding also proves its field is present and well typed.
        expected_fields = [binding.partition("=")[0] for binding in expected]
        require_fields(payload, required + [
            field for field in expected_fields if field not in required
        ])
        bind_expected(payload, expected)
        emit(payload, required)
    except ReaderFailure as failure:
        print(str(failure), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
