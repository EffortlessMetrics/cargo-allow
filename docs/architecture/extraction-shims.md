# Extraction Shims

Human projection of `policy/extraction-shims.toml` (#2607 / `CARGO-ALLOW-SHIM-REGISTRY-0001`).

## Claim boundary

The registry records transitional forwarding surfaces. The repo-snapshot entries
include live public compatibility re-exports, while the repo-edit core entries
include live private forwarding modules for mutation locks, path containment,
and atomic writes. These forwards preserve the cargo-allow command boundary;
they do not prove parity acceptance or permit shim deletion before the relevant
#2606 cutover receipt.

The source checks cover the core repo-edit forwards. Command-specific apply
forwards remain separately bounded by their own shim and cutover records.
