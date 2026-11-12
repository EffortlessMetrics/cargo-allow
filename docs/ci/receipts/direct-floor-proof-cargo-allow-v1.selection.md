# Direct-floor selection evidence

Selection explanations only; the companion JSON owns proof dispositions.
These paths start at each member's selected default/requested features;
they are local activation witnesses, not a complete cross-crate feature graph.

- Product: cargo-allow
- Package roots: cargo-allow
- Selected closure: allow-core, allow-diff, allow-files, allow-inventory, allow-match, allow-policy, allow-policy-legacy, allow-report, allow-rust, cargo-allow, effortless-repo-edit, effortless-repo-protocol, effortless-repo-snapshot
- Starting source commit: b06bff90da872ed9041053aad7cd805b7d5de221
- Workspace projection: cargo-allow.direct-floor-product-workspace.v1
- Projected manifest digest: sha256:v1:87bd2c9bbcf9e089fa8b0134e829de5710ecf607c9c1678a1864a50cdbb758ca
- Certified member paths: crates/cargo-allow, crates/allow-core, crates/allow-policy, crates/allow-inventory, crates/allow-files, crates/allow-rust, crates/allow-match, crates/allow-report, crates/allow-diff, crates/allow-policy-legacy, crates/effortless-repo-protocol, crates/effortless-repo-snapshot, crates/effortless-repo-edit
- Execution member paths: crates/cargo-allow, crates/allow-core, crates/allow-policy, crates/allow-inventory, crates/allow-files, crates/allow-rust, crates/allow-match, crates/allow-report, crates/allow-diff, crates/allow-policy-legacy, crates/effortless-repo-protocol, crates/effortless-repo-snapshot, crates/effortless-repo-edit, crates/effortless-rust-source-index, crates/intent-model, crates/intent-protocol, crates/intent-engine
- Executed floor commit: 2e99ae81311eca23575a385a1f0f709b2a00d525
- Executed floor tree: 712d85f96edaf89e99cdc899483cdbe78f9b8194
- Receipt: direct-floor-proof-cargo-allow-v1.json
- Receipt SHA-256: sha256:v1:4f648fefb010629facb0b4de4aa3973b520168a6c5570354a57399f127ff58c4
- Manifest-set digest: 71bcdc199f3925f034b42c4e90d16150f4b45eeffb3325799d75ec0bce1e3ea6
- Starting lock digest: 0f1dc82202812f4db3d2e6820329b2da2f28deae57b393e79cbb68d05ff1b06c
- Executed floor-lock digest: sha256:v1:16235629fe25240b15f560a6229c6b0c40d8b7275599740cb187d00f3d8660a3

| Owner | Table | Optional dependency | Disposition | Reason | Activation witnesses |
| --- | --- | --- | --- | --- | --- |
| allow-files | dependencies | yaml-rust2 | included | enabled by a selected feature path | allow-files/changie -> allow-files/dep:yaml-rust2 |
| allow-rust | dependencies | tree-sitter | included | enabled by a selected feature path | allow-rust/default -> allow-rust/syntax -> allow-rust/dep:tree-sitter |
| allow-rust | dependencies | tree-sitter-rust | included | enabled by a selected feature path | allow-rust/default -> allow-rust/syntax -> allow-rust/dep:tree-sitter-rust |

## Original-workspace topology preflight

Original locked-workspace topology contracts only; not direct-floor proof. These tests remain enforced before the product workspace is projected.

- Source commit: b06bff90da872ed9041053aad7cd805b7d5de221
- Source tree: 85f73d00967f19e20e8a24289aba72bead45408c
- Original manifest digest: sha256:v1:b8af414f555e2a879ea5d8e641c0ddf951a341728d200e0f8fc5f1e16aac852a
- Original lock digest: sha256:v1:0f1dc82202812f4db3d2e6820329b2da2f28deae57b393e79cbb68d05ff1b06c
- Target: x86_64-unknown-linux-gnu
- Command: cargo test --locked --target x86_64-unknown-linux-gnu --target-dir target/floor-proof/source-workspace-target -p cargo-allow --bin cargo-allow -- --format pretty --color never ci_lane_topology_tests::crate_sets_partition_the_workspace_exactly package_topology_enforcement_tests::topology_classifies_every_workspace_package_exactly product_package_topology_tests::current_v2_authorities_drive_package_candidate publish_order_validation_tests::publish_order_covers_all_workspace_members release_prep_tests::published_release_versions_match_workspace
- Standard-output digest: sha256:v1:bb6d576a12bffba6cc46ad405bd0d49b1cc91543b61dfa6ae4bc7c445fc91941
- Standard-error digest: sha256:v1:2d0811ec9b9c5a213e5c17897aff37768e7a4fa909240c139c29958a19786c08

- Passed before projection: ci_lane_topology_tests::crate_sets_partition_the_workspace_exactly
- Passed before projection: package_topology_enforcement_tests::topology_classifies_every_workspace_package_exactly
- Passed before projection: product_package_topology_tests::current_v2_authorities_drive_package_candidate
- Passed before projection: publish_order_validation_tests::publish_order_covers_all_workspace_members
- Passed before projection: release_prep_tests::published_release_versions_match_workspace
