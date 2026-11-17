# Direct-floor selection evidence

Selection explanations only; the companion JSON owns proof dispositions.
These paths start at each member's selected default/requested features;
they are local activation witnesses, not a complete cross-crate feature graph.

- Product: cargo-allow
- Package roots: cargo-allow
- Selected closure: allow-core, allow-diff, allow-files, allow-inventory, allow-match, allow-policy, allow-policy-legacy, allow-report, allow-rust, cargo-allow, effortless-repo-edit, effortless-repo-protocol, effortless-repo-snapshot
- Starting source commit: 7f5779b1ad17bdcce2ced0c0bbdc33622de1cb11
- Workspace projection: cargo-allow.direct-floor-product-workspace.v1
- Projected manifest digest: sha256:v1:36b8acc4eddb70fedd8bbe35dd0869ab16cde657fdac6f6fe58ec156bd325b9a
- Certified member paths: crates/cargo-allow, crates/allow-core, crates/allow-policy, crates/allow-inventory, crates/allow-files, crates/allow-rust, crates/allow-match, crates/allow-report, crates/allow-diff, crates/allow-policy-legacy, crates/effortless-repo-protocol, crates/effortless-repo-snapshot, crates/effortless-repo-edit
- Execution member paths: crates/cargo-allow, crates/allow-core, crates/allow-policy, crates/allow-inventory, crates/allow-files, crates/allow-rust, crates/allow-match, crates/allow-report, crates/allow-diff, crates/allow-policy-legacy, crates/effortless-repo-protocol, crates/effortless-repo-snapshot, crates/effortless-repo-edit, crates/effortless-rust-source-index, crates/intent-model, crates/intent-protocol, crates/intent-engine
- Executed floor commit: 10728b355f5fad7a03e6c8896432aef21a87f447
- Executed floor tree: 63c7849ce43ec852a93f008841f7786768815337
- Receipt: direct-floor-proof-cargo-allow-v1.json
- Receipt SHA-256: sha256:v1:9d922107a71f50d1bdf49269805118f1110f9282aa0eff4b9b5a897d70795d9d
- Manifest-set digest: 18db841f212518f4d9575251c764300c2962c7bbba0d96f72a68745a0698f8db
- Starting lock digest: 0f1dc82202812f4db3d2e6820329b2da2f28deae57b393e79cbb68d05ff1b06c
- Executed floor-lock digest: sha256:v1:09c30b0888a314663d5e3610efe40fa9d7e7a8d0e5bf590618e49108abb40076

| Owner | Table | Optional dependency | Disposition | Reason | Activation witnesses |
| --- | --- | --- | --- | --- | --- |
| allow-files | dependencies | yaml-rust2 | included | enabled by a selected feature path | allow-files/changie -> allow-files/dep:yaml-rust2 |
| allow-rust | dependencies | tree-sitter | included | enabled by a selected feature path | allow-rust/default -> allow-rust/syntax -> allow-rust/dep:tree-sitter |
| allow-rust | dependencies | tree-sitter-rust | included | enabled by a selected feature path | allow-rust/default -> allow-rust/syntax -> allow-rust/dep:tree-sitter-rust |

## Original-workspace topology preflight

Original locked-workspace topology contracts only; not direct-floor proof. These tests remain enforced before the product workspace is projected.

- Source commit: 7f5779b1ad17bdcce2ced0c0bbdc33622de1cb11
- Source tree: 3500ae2bc2c809544dcd5497f7789d6dde541bad
- Original manifest digest: sha256:v1:6e7ff2802c635fe2ad17a5ca85826931d5a580eeff97f7291b86d309d1b84319
- Original lock digest: sha256:v1:0f1dc82202812f4db3d2e6820329b2da2f28deae57b393e79cbb68d05ff1b06c
- Target: x86_64-unknown-linux-gnu
- Command: cargo test --locked --target x86_64-unknown-linux-gnu --target-dir target/floor-proof/source-workspace-target -p cargo-allow --bin cargo-allow -- --format pretty --color never ci_lane_topology_tests::crate_sets_partition_the_workspace_exactly package_topology_enforcement_tests::topology_classifies_every_workspace_package_exactly product_package_topology_tests::current_v2_authorities_drive_package_candidate publish_order_validation_tests::publish_order_covers_all_workspace_members release_prep_tests::published_release_versions_match_workspace
- Standard-output digest: sha256:v1:bb9e5bece6b405f7336e82006ab8d24d3537195e6a0a36de9d012c1c9cf8db38
- Standard-error digest: sha256:v1:b593c3840ca954ea81438077f2be1f5ba4835ab43f1542f67749fddce93a29c6

- Passed before projection: ci_lane_topology_tests::crate_sets_partition_the_workspace_exactly
- Passed before projection: package_topology_enforcement_tests::topology_classifies_every_workspace_package_exactly
- Passed before projection: product_package_topology_tests::current_v2_authorities_drive_package_candidate
- Passed before projection: publish_order_validation_tests::publish_order_covers_all_workspace_members
- Passed before projection: release_prep_tests::published_release_versions_match_workspace
