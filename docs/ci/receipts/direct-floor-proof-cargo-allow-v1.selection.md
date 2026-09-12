# Direct-floor selection evidence

Selection explanations only; the companion JSON owns proof dispositions.
These paths start at each member's selected default/requested features;
they are local activation witnesses, not a complete cross-crate feature graph.

- Product: cargo-allow
- Package roots: cargo-allow
- Selected closure: allow-core, allow-diff, allow-files, allow-inventory, allow-match, allow-policy, allow-policy-legacy, allow-report, allow-rust, cargo-allow, effortless-repo-edit, effortless-repo-protocol, effortless-repo-snapshot
- Starting source commit: badbec1b40f3170bf7b1402fe57e8f119c4173dd
- Receipt: direct-floor-proof-cargo-allow-v1.json
- Receipt SHA-256: sha256:v1:e5417be37feb8c64bd20c419ea5fa0e6a894bf0e59efb877c2f066e96afaad90
- Manifest-set digest: adc5877bce43d8d972a162d62069cb0fd1ea983713f4f0e1a66d357d7ddc4d75
- Starting lock digest: 63bec58292d7d47d0c2f94ebd34a71bf2f1a78444e1cf38ce92bfc58c989dc15
- Executed floor-lock digest: sha256:v1:94da3b62338ace9a88b69bea38b236a97eb461a31cdd5ba991845ebce0a5c9ae

| Owner | Table | Optional dependency | Disposition | Reason | Activation witnesses |
| --- | --- | --- | --- | --- | --- |
| allow-files | dependencies | yaml-rust2 | included | enabled by a selected feature path | allow-files/changie -> allow-files/dep:yaml-rust2 |
| allow-rust | dependencies | tree-sitter | included | enabled by a selected feature path | allow-rust/default -> allow-rust/syntax -> allow-rust/dep:tree-sitter |
| allow-rust | dependencies | tree-sitter-rust | included | enabled by a selected feature path | allow-rust/default -> allow-rust/syntax -> allow-rust/dep:tree-sitter-rust |
