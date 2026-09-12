# Direct-floor selection evidence

Selection explanations only; the companion JSON owns proof dispositions.
These paths start at each member's selected default/requested features;
they are local activation witnesses, not a complete cross-crate feature graph.

- Product: cargo-allow
- Package roots: cargo-allow
- Selected closure: allow-core, allow-diff, allow-files, allow-inventory, allow-match, allow-policy, allow-policy-legacy, allow-report, allow-rust, cargo-allow, effortless-repo-edit, effortless-repo-protocol, effortless-repo-snapshot
- Starting source commit: 62c25f71bd9e757ba07db0d6988a6ce80af083e9
- Receipt: direct-floor-proof-cargo-allow-v1.json
- Receipt SHA-256: sha256:v1:ecae38fe6718431dfcd412cff0b171b55ab5f0265f9095ef830e9dbcb5899e73
- Manifest-set digest: 0cc7ac8b80d7ba163a305e0217aaabf7fd471a0b582a5c32bcce2fbd8a4674fe
- Starting lock digest: 0d534b066fcc4db1fd01fae560255a82bb906ef214b8a7aa7f30e030f934f8ea
- Executed floor-lock digest: sha256:v1:bf9eb0150977767f7625ff34b1d5fe0d5410668d1c2c443a972d567ea8155c29

| Owner | Table | Optional dependency | Disposition | Reason | Activation witnesses |
| --- | --- | --- | --- | --- | --- |
| allow-files | dependencies | yaml-rust2 | included | enabled by a selected feature path | allow-files/changie -> allow-files/dep:yaml-rust2 |
| allow-rust | dependencies | tree-sitter | included | enabled by a selected feature path | allow-rust/default -> allow-rust/syntax -> allow-rust/dep:tree-sitter |
| allow-rust | dependencies | tree-sitter-rust | included | enabled by a selected feature path | allow-rust/default -> allow-rust/syntax -> allow-rust/dep:tree-sitter-rust |
