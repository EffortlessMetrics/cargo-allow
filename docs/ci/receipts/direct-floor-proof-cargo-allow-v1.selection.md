# Direct-floor selection evidence

Selection explanations only; the companion JSON owns proof dispositions.
These paths start at each member's selected default/requested features;
they are local activation witnesses, not a complete cross-crate feature graph.

- Product: cargo-allow
- Package roots: cargo-allow
- Selected closure: allow-core, allow-diff, allow-files, allow-inventory, allow-match, allow-policy, allow-policy-legacy, allow-report, allow-rust, cargo-allow, effortless-repo-edit, effortless-repo-protocol, effortless-repo-snapshot
- Starting source commit: c2131ec3fe8c0ec88c293ed438f603f4bbaf6c15
- Workspace projection: cargo-allow.direct-floor-product-workspace.v1
- Projected manifest digest: sha256:v1:87bd2c9bbcf9e089fa8b0134e829de5710ecf607c9c1678a1864a50cdbb758ca
- Certified member paths: crates/cargo-allow, crates/allow-core, crates/allow-policy, crates/allow-inventory, crates/allow-files, crates/allow-rust, crates/allow-match, crates/allow-report, crates/allow-diff, crates/allow-policy-legacy, crates/effortless-repo-protocol, crates/effortless-repo-snapshot, crates/effortless-repo-edit
- Execution member paths: crates/cargo-allow, crates/allow-core, crates/allow-policy, crates/allow-inventory, crates/allow-files, crates/allow-rust, crates/allow-match, crates/allow-report, crates/allow-diff, crates/allow-policy-legacy, crates/effortless-repo-protocol, crates/effortless-repo-snapshot, crates/effortless-repo-edit, crates/effortless-rust-source-index, crates/intent-model, crates/intent-protocol, crates/intent-engine
- Executed floor commit: 5b6e003ae22fa9afae6e58f3af786adcb1a51f55
- Executed floor tree: 74e00c1aef82c20f391161c3c111deb3a2afb62b
- Receipt: direct-floor-proof-cargo-allow-v1.json
- Receipt SHA-256: sha256:v1:f0decf7ea520860b256f618cae213ed558419d06bc063dd227d6a379a2535cc7
- Manifest-set digest: 0ca4f9eb8992041fde83c1c720bbc1aa51895f603382ca68c5f18255ce52a328
- Starting lock digest: c6730d656e7686213bf37e4fa4474bbe61824039b31fdfb02d25bcee92e98775
- Executed floor-lock digest: sha256:v1:250e6bf060a788cf55ec5500e9674e79633a0b35989d7e2bc95dc2ff300fc8cd

| Owner | Table | Optional dependency | Disposition | Reason | Activation witnesses |
| --- | --- | --- | --- | --- | --- |
| allow-files | dependencies | yaml-rust2 | included | enabled by a selected feature path | allow-files/changie -> allow-files/dep:yaml-rust2 |
| allow-rust | dependencies | tree-sitter | included | enabled by a selected feature path | allow-rust/default -> allow-rust/syntax -> allow-rust/dep:tree-sitter |
| allow-rust | dependencies | tree-sitter-rust | included | enabled by a selected feature path | allow-rust/default -> allow-rust/syntax -> allow-rust/dep:tree-sitter-rust |

## Original-workspace topology preflight

Original locked-workspace topology contracts only; not direct-floor proof. These tests remain enforced before the product workspace is projected.

- Source commit: c2131ec3fe8c0ec88c293ed438f603f4bbaf6c15
- Source tree: c4c005058daa573bc4216c9a921d9a8ff9d84fc5
- Original manifest digest: sha256:v1:b8af414f555e2a879ea5d8e641c0ddf951a341728d200e0f8fc5f1e16aac852a
- Original lock digest: sha256:v1:c6730d656e7686213bf37e4fa4474bbe61824039b31fdfb02d25bcee92e98775
- Target: x86_64-pc-windows-msvc
- Command: cargo test --locked --target x86_64-pc-windows-msvc --target-dir target/floor-proof/source-workspace-target -p cargo-allow --bin cargo-allow -- --format pretty --color never ci_lane_topology_tests::crate_sets_partition_the_workspace_exactly package_topology_enforcement_tests::topology_classifies_every_workspace_package_exactly product_package_topology_tests::current_v2_authorities_drive_package_candidate publish_order_validation_tests::publish_order_covers_all_workspace_members release_prep_tests::published_release_versions_match_workspace
- Standard-output digest: sha256:v1:f649d93a34f70912f15e62fabdeec32e812446d98fb3bd9c34b81603bcc6f6f0
- Standard-error digest: sha256:v1:740674c6d06ab08f720c73f6af0a7fda0b5282825d79e263a7496a535b28b15b

- Passed before projection: ci_lane_topology_tests::crate_sets_partition_the_workspace_exactly
- Passed before projection: package_topology_enforcement_tests::topology_classifies_every_workspace_package_exactly
- Passed before projection: product_package_topology_tests::current_v2_authorities_drive_package_candidate
- Passed before projection: publish_order_validation_tests::publish_order_covers_all_workspace_members
- Passed before projection: release_prep_tests::published_release_versions_match_workspace
