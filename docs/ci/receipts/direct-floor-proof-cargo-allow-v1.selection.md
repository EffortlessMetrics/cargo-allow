# Direct-floor selection evidence

Selection explanations only; the companion JSON owns proof dispositions.
These paths start at each member's selected default/requested features;
they are local activation witnesses, not a complete cross-crate feature graph.

- Product: cargo-allow
- Package roots: cargo-allow
- Selected closure: allow-core, allow-diff, allow-files, allow-inventory, allow-match, allow-policy, allow-policy-legacy, allow-report, allow-rust, cargo-allow, effortless-repo-edit, effortless-repo-protocol, effortless-repo-snapshot
- Starting source commit: 7a2cfbce2ec419086300a02a6f58fdddd45220f4
- Receipt: direct-floor-proof-cargo-allow-v1.json
- Receipt SHA-256: sha256:v1:14fe03a4666f7d3c1783c3589f1c78c84d2733364462904be76192f7a78f0e9c
- Manifest-set digest: adc5877bce43d8d972a162d62069cb0fd1ea983713f4f0e1a66d357d7ddc4d75
- Starting lock digest: 69ea6eb64d8e325e7ed83894392e001a1bc821421259f1d3b32337b9e5238047
- Executed floor-lock digest: sha256:v1:94da3b62338ace9a88b69bea38b236a97eb461a31cdd5ba991845ebce0a5c9ae

| Owner | Table | Optional dependency | Disposition | Reason | Activation witnesses |
| --- | --- | --- | --- | --- | --- |
| allow-files | dependencies | yaml-rust2 | included | enabled by a selected feature path | allow-files/changie -> allow-files/dep:yaml-rust2 |
| allow-rust | dependencies | tree-sitter | included | enabled by a selected feature path | allow-rust/default -> allow-rust/syntax -> allow-rust/dep:tree-sitter |
| allow-rust | dependencies | tree-sitter-rust | included | enabled by a selected feature path | allow-rust/default -> allow-rust/syntax -> allow-rust/dep:tree-sitter-rust |
