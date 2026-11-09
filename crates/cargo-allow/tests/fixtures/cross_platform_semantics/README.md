# cross_platform_semantics fixture

One deliberately small repository/policy scenario for the #2276 normalized
semantic determinism oracle (`crates/cargo-allow/tests/release_platform_semantics.rs`).
The oracle runs the real inventory -> scan -> match -> render pipeline over this
directory on every host and compares a canonical projection against a checked-in
golden, so the `test-core-platforms` matrix proves byte-identical semantic
output across Linux, Windows, and macOS runners.

## Scenario surfaces

- `tree/alpha.rs` (LF) and `tree/alpha_crlf.rs` (CRLF) carry the identical
  `unwrap` line, pinning that LF and CRLF source text produce the same
  normalized snippet hash and identity.
- `tree/nested/gamma.rs` exercises a nested repository-relative path, an
  `expect` call with an expired exact structural receipt, and an
  `unreachable!` finding left intentionally unmatched (new posture).
- `policy.toml` is the fixture policy: live matched, review-due, expired, and
  new postures plus its own non-Rust self-receipt. This `README.md` carries no
  finding: the scanner treats every `crates/**/README.md` as built-in
  documentation metadata (`allow-files` path rules), which the projection
  pins as a five-file inventory with one non-Rust finding.
- Repository-relative path normalization, slash/backslash spelling, Windows
  drive and UNC inputs, Unicode NFC/NFD equivalence, and case-sensitive
  distinctions are exercised at the owning `allow-core` seams by the same test
  file (inputs that cannot be materialized identically on every host).

## Documented projection exclusions

- absolute scan root, runner OS/architecture, wall-clock timestamps (the
  evaluation `as_of` date is pinned inside the test), and native executable
  path are environment, not semantics;
- line/column spans are documented hints, not identity (excluded from
  `finding_identity_key` by the product);
- raw path bytes (the fixture pins `normalize_path` output only);
- deleted-tracked inventory paths are covered by the retained
  `inventory_deleted_tracked` integration test rather than duplicated here.

No symlink, executable-bit, or case-pair file is materialized: case pairs
cannot exist on case-insensitive hosts, so those distinctions are pinned at
the `allow-core` seams instead of behind `cfg`-gated passing aggregates.
