# Direct-floor selection evidence

Selection explanations only; the companion JSON owns proof dispositions.
These paths start at each member's selected default/requested features;
they are local activation witnesses, not a complete cross-crate feature graph.

- Product: cargo-allow
- Package roots: cargo-allow
- Selected closure: allow-core, allow-diff, allow-files, allow-inventory, allow-match, allow-policy, allow-policy-legacy, allow-report, allow-rust, cargo-allow, effortless-repo-edit, effortless-repo-protocol, effortless-repo-snapshot
- Starting source commit: 80d0b63e5446568ad4d76cee5af4cb98d79ab233
- Workspace projection: cargo-allow.direct-floor-product-workspace.v1
- Projected manifest digest: sha256:v1:87bd2c9bbcf9e089fa8b0134e829de5710ecf607c9c1678a1864a50cdbb758ca
- Certified member paths: crates/cargo-allow, crates/allow-core, crates/allow-policy, crates/allow-inventory, crates/allow-files, crates/allow-rust, crates/allow-match, crates/allow-report, crates/allow-diff, crates/allow-policy-legacy, crates/effortless-repo-protocol, crates/effortless-repo-snapshot, crates/effortless-repo-edit
- Execution member paths: crates/cargo-allow, crates/allow-core, crates/allow-policy, crates/allow-inventory, crates/allow-files, crates/allow-rust, crates/allow-match, crates/allow-report, crates/allow-diff, crates/allow-policy-legacy, crates/effortless-repo-protocol, crates/effortless-repo-snapshot, crates/effortless-repo-edit, crates/effortless-rust-source-index, crates/intent-model, crates/intent-protocol, crates/intent-engine
- Executed floor commit: 6cc338c861fe4fedcbb06b59c2dcfa07db77cb1c
- Executed floor tree: c2cd23c5633b6ea8c06daa56685275897c28307a
- Receipt: direct-floor-proof-cargo-allow-v1.json
- Receipt SHA-256: sha256:v1:ab765bfa023af5ccfb5a443c4547e7b12c45df713cad47ce18dd9c310d6298ff
- Manifest-set digest: 0ca4f9eb8992041fde83c1c720bbc1aa51895f603382ca68c5f18255ce52a328
- Starting lock digest: fc4f38834ca26322a1339a96024b85b0c4ea787e7563667e541cf13b63497c41
- Executed floor-lock digest: sha256:v1:4a131e630bae53b6ee4c03fc30baf15936a62575e6d2015acdd862ea062b7c87

| Owner | Table | Optional dependency | Disposition | Reason | Activation witnesses |
| --- | --- | --- | --- | --- | --- |
| allow-files | dependencies | yaml-rust2 | included | enabled by a selected feature path | allow-files/changie -> allow-files/dep:yaml-rust2 |
| allow-rust | dependencies | tree-sitter | included | enabled by a selected feature path | allow-rust/default -> allow-rust/syntax -> allow-rust/dep:tree-sitter |
| allow-rust | dependencies | tree-sitter-rust | included | enabled by a selected feature path | allow-rust/default -> allow-rust/syntax -> allow-rust/dep:tree-sitter-rust |

## Original-workspace topology preflight

Original locked-workspace topology contracts only; not direct-floor proof. These tests remain enforced before the product workspace is projected.

- Source commit: 80d0b63e5446568ad4d76cee5af4cb98d79ab233
- Source tree: d55797c311bf7daf10a818727bcbed51916751d2
- Original manifest digest: sha256:v1:b8af414f555e2a879ea5d8e641c0ddf951a341728d200e0f8fc5f1e16aac852a
- Original lock digest: sha256:v1:fc4f38834ca26322a1339a96024b85b0c4ea787e7563667e541cf13b63497c41
- Target: x86_64-unknown-linux-gnu
- Command: cargo test --locked --target x86_64-unknown-linux-gnu --target-dir target/floor-proof/source-workspace-target -p cargo-allow --bin cargo-allow -- --format pretty --color never ci_lane_topology_tests::crate_sets_partition_the_workspace_exactly package_topology_enforcement_tests::topology_classifies_every_workspace_package_exactly product_package_topology_tests::current_v2_authorities_drive_package_candidate publish_order_validation_tests::publish_order_covers_all_workspace_members release_prep_tests::published_release_versions_match_workspace
- Standard-output digest: sha256:v1:c398627ef6222bde0dd5a5480c66648e41f508180ca16bff1472ff1a723e75ab
- Standard-error digest: sha256:v1:46b7c83b4c56c67bda8702bf943bf25aa60996aaf925714e96c3cd9de2b65a18

- Passed before projection: ci_lane_topology_tests::crate_sets_partition_the_workspace_exactly
- Passed before projection: package_topology_enforcement_tests::topology_classifies_every_workspace_package_exactly
- Passed before projection: product_package_topology_tests::current_v2_authorities_drive_package_candidate
- Passed before projection: publish_order_validation_tests::publish_order_covers_all_workspace_members
- Passed before projection: release_prep_tests::published_release_versions_match_workspace
