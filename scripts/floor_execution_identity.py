"""Observe and validate the toolchain used by the direct-floor collector."""

import json
import re
import subprocess
import sys


def stable_version(value, lengths):
    parts = value.split(".")
    if len(parts) not in lengths or any(
        re.fullmatch(r"0|[1-9][0-9]*", part) is None for part in parts
    ):
        raise ValueError(f"not a stable numeric version: {value!r}")
    return tuple(int(part) for part in parts)


def identity(msrv, rustc, cargo):
    requested = stable_version(msrv, (2, 3))
    observed = []
    for tool, output in (("rustc", rustc), ("cargo", cargo)):
        fields = {}
        for line in output.splitlines():
            key, separator, value = line.partition(": ")
            if separator:
                if key in fields:
                    raise ValueError(f"duplicate {tool} identity field: {key}")
                fields[key] = value
        release = fields.get("release", "")
        version = stable_version(release, (3,))
        if version[:len(requested)] != requested:
            raise ValueError(f"observed {tool} {release} does not match MSRV {msrv}")
        host = fields.get("host", "")
        if re.fullmatch(r"[A-Za-z0-9_]+(?:-[A-Za-z0-9_]+){2,}", host) is None:
            raise ValueError(f"missing or malformed {tool} host: {host!r}")
        observed.append((release, host))
    rust_release, rust_host = observed[0]
    cargo_release, cargo_host = observed[1]
    if rust_host != cargo_host:
        raise ValueError("rustc and cargo host identities differ")
    return {
        "toolchain": rust_release,
        "target": "host:" + rust_host,
        "host": rust_host,
        "cargo": cargo_release,
    }


def main():
    outputs = [
        subprocess.run([tool, "-vV"], check=True, capture_output=True, text=True).stdout
        for tool in ("rustc", "cargo")
    ]
    print(json.dumps(identity(sys.argv[1], *outputs)))


if __name__ == "__main__":
    main()
