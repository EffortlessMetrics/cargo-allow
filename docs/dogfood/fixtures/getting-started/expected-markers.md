# Expected-output markers for docs/getting-started.md (#2354).
#
# Each marker must appear in getting-started.md and in live output from the
# just-built cargo-allow binary (see first_hour_adoption.rs). Paths and temp
# roots are omitted so markers stay stable across machines.

## doctor (no policy)

- `cargo-allow.doctor.v1`
- `"command": "doctor"`
- `config: not found`

## audit (clean)

- `"command": "audit"`
- `"findings": 0`
- `"new": 0`

## audit (one unreceipted finding)

- `"command": "audit"`
- `"new": 1`
- `"status": "passed"`

## check (passing baseline)

- `"command": "check"`
- `"status": "passed"`
- `"new": 0`
- `Result: passed/advisory`

## check (failing after new debt)

- `"status": "failed"` or human `Result: failed`
- `new: unreceipted`

## list / explain route

- `cargo-allow list`
- `cargo-allow explain`
