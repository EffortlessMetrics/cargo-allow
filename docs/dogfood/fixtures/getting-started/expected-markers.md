# Expected-output markers for docs/getting-started.md (#2354).
#
# Each marker must appear in getting-started.md. Markers under "Live renderer"
# are also asserted against the just-built cargo-allow binary in
# first_hour_adoption.rs. Markers under "Documentation routes" are command
# names taught by the guide (list/explain are executed on the brownfield
# adoption path, not re-asserted as substring matches of JSON here).
#
# Paths and temp roots are omitted so markers stay stable across machines.

## Live renderer

### doctor (no policy)

- `cargo-allow.doctor.v1`
- `"command": "doctor"`
- `config: not found`

### audit (clean)

- `"command": "audit"`
- `"findings": 0`
- `"new": 0`

### audit (one unreceipted finding)

- `"command": "audit"`
- `"new": 1`
- `"status": "passed"`

### check (passing baseline)

- `"command": "check"`
- `"status": "passed"`
- `"new": 0`
- `Result: passed (enforcing)`

### check (failing after new debt)

- `"status": "failed"` or human `Result: failed`
- `new: unreceipted`

## Documentation routes

- `cargo-allow list`
- `cargo-allow explain`
