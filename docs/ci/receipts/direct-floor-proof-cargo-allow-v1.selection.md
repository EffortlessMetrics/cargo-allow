# Direct-floor selection evidence

Selection explanations only; the companion JSON owns proof dispositions.
These paths start at each member's selected default/requested features;
they are local activation witnesses, not a complete cross-crate feature graph.

- Product: cargo-allow
- Package roots: cargo-allow
- Selected closure: allow-core, allow-diff, allow-files, allow-inventory, allow-match, allow-policy, allow-policy-legacy, allow-report, allow-rust, cargo-allow, effortless-repo-edit, effortless-repo-protocol, effortless-repo-snapshot
- Starting source commit: 87dbf2dfa2343fd56d7373934daa2f0c92c64c22
- Workspace projection: cargo-allow.direct-floor-product-workspace.v1
- Projected manifest digest: sha256:v1:87bd2c9bbcf9e089fa8b0134e829de5710ecf607c9c1678a1864a50cdbb758ca
- Certified member paths: crates/cargo-allow, crates/allow-core, crates/allow-policy, crates/allow-inventory, crates/allow-files, crates/allow-rust, crates/allow-match, crates/allow-report, crates/allow-diff, crates/allow-policy-legacy, crates/effortless-repo-protocol, crates/effortless-repo-snapshot, crates/effortless-repo-edit
- Execution member paths: crates/cargo-allow, crates/allow-core, crates/allow-policy, crates/allow-inventory, crates/allow-files, crates/allow-rust, crates/allow-match, crates/allow-report, crates/allow-diff, crates/allow-policy-legacy, crates/effortless-repo-protocol, crates/effortless-repo-snapshot, crates/effortless-repo-edit, crates/effortless-rust-source-index, crates/intent-model, crates/intent-protocol, crates/intent-engine
- Executed floor commit: af13ceffef438a3e7c121cea50849c0f5d9e3e3b
- Executed floor tree: 31e7074f52efc50e385032c5ac7566cf7293b029
- Receipt: direct-floor-proof-cargo-allow-v1.json
- Receipt SHA-256: sha256:v1:4c1986cbae13365eff3d983e3bbcfd1eec023d8a1e3afcd582f2f406c06f56b0
- Manifest-set digest: 4430bf35921f23a321655487516a7bffd24fc1b3a23cb39cbda6b371682068ae
- Starting lock digest: 808326f45858ca6758dbf8f920762eb7eb7c439ac130a5f9c93e9e3feec40b09
- Executed floor-lock digest: sha256:v1:c91430458adfd35fd52830c6954ff57d1efa82e55061f7107c844bbd9b8430ce

| Owner | Table | Optional dependency | Disposition | Reason | Activation witnesses |
| --- | --- | --- | --- | --- | --- |
| allow-files | dependencies | yaml-rust2 | included | enabled by a selected feature path | allow-files/changie -> allow-files/dep:yaml-rust2 |
| allow-rust | dependencies | tree-sitter | included | enabled by a selected feature path | allow-rust/default -> allow-rust/syntax -> allow-rust/dep:tree-sitter |
| allow-rust | dependencies | tree-sitter-rust | included | enabled by a selected feature path | allow-rust/default -> allow-rust/syntax -> allow-rust/dep:tree-sitter-rust |

## Original-workspace topology preflight

Original locked-workspace topology contracts only; not direct-floor proof. These tests remain enforced before the product workspace is projected.

- Source commit: 87dbf2dfa2343fd56d7373934daa2f0c92c64c22
- Source tree: ceece8154853dfd84a2e08f00cecdc9774992c54
- Original manifest digest: sha256:v1:b8af414f555e2a879ea5d8e641c0ddf951a341728d200e0f8fc5f1e16aac852a
- Original lock digest: sha256:v1:808326f45858ca6758dbf8f920762eb7eb7c439ac130a5f9c93e9e3feec40b09
- Target: x86_64-unknown-linux-gnu
- Command: cargo test --locked --target x86_64-unknown-linux-gnu --target-dir target/floor-proof/source-workspace-target -p cargo-allow --bin cargo-allow -- --format pretty --color never ci_lane_topology_tests::crate_sets_partition_the_workspace_exactly package_topology_enforcement_tests::topology_classifies_every_workspace_package_exactly product_package_topology_tests::current_v2_authorities_drive_package_candidate publish_order_validation_tests::publish_order_covers_all_workspace_members release_prep_tests::published_release_versions_match_workspace
- Standard-output digest: sha256:v1:c398627ef6222bde0dd5a5480c66648e41f508180ca16bff1472ff1a723e75ab
- Standard-error digest: sha256:v1:f411b5bd65ec516d7d3fc6c2178d2dc37dbf1b4e2252e091138041808bb2121c

- Passed before projection: ci_lane_topology_tests::crate_sets_partition_the_workspace_exactly
- Passed before projection: package_topology_enforcement_tests::topology_classifies_every_workspace_package_exactly
- Passed before projection: product_package_topology_tests::current_v2_authorities_drive_package_candidate
- Passed before projection: publish_order_validation_tests::publish_order_covers_all_workspace_members
- Passed before projection: release_prep_tests::published_release_versions_match_workspace
