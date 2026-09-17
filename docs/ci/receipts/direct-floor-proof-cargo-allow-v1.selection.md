# Direct-floor selection evidence

Selection explanations only; the companion JSON owns proof dispositions.
These paths start at each member's selected default/requested features;
they are local activation witnesses, not a complete cross-crate feature graph.

- Product: cargo-allow
- Package roots: cargo-allow
- Selected closure: allow-core, allow-diff, allow-files, allow-inventory, allow-match, allow-policy, allow-policy-legacy, allow-report, allow-rust, cargo-allow, effortless-repo-edit, effortless-repo-protocol, effortless-repo-snapshot
- Starting source commit: aa9a3a761f3db80e85734c70afeb183266da4e0d
- Workspace projection: cargo-allow.direct-floor-product-workspace.v1
- Projected manifest digest: sha256:v1:365c40e42b0775c5561f284b3362527dd353eb4b25df6c1d31bc4bd5a6e9add7
- Certified member paths: crates/cargo-allow, crates/allow-core, crates/allow-policy, crates/allow-inventory, crates/allow-files, crates/allow-rust, crates/allow-match, crates/allow-report, crates/allow-diff, crates/allow-policy-legacy, crates/effortless-repo-protocol, crates/effortless-repo-snapshot, crates/effortless-repo-edit
- Execution member paths: crates/cargo-allow, crates/allow-core, crates/allow-policy, crates/allow-inventory, crates/allow-files, crates/allow-rust, crates/allow-match, crates/allow-report, crates/allow-diff, crates/allow-policy-legacy, crates/effortless-repo-protocol, crates/effortless-repo-snapshot, crates/effortless-repo-edit, crates/effortless-rust-source-index, crates/intent-model, crates/intent-protocol, crates/intent-engine
- Executed floor commit: 29e53e27a57c18fa2cafda12f6cfb431de38902f
- Executed floor tree: fc681788048419dc3e87bf31c18cff70febe34fd
- Receipt: direct-floor-proof-cargo-allow-v1.json
- Receipt SHA-256: sha256:v1:cb51e591877de3878492dd8cee8a86a2e46d5a5c5f050d5c759fffe2f701d599
- Manifest-set digest: e7f431c883f3d492e7f6be40e377562468334f81701d6ed7a8058acbcc135a52
- Starting lock digest: dccad79fa88bb2cfe03674b5e69e907d3f00dad94d6f66595b0cd35e75e91120
- Executed floor-lock digest: sha256:v1:17b58d88e844a2e5825e7f95221fb92a3d7840cefb29ca99b7c384ebc52c13a3

| Owner | Table | Optional dependency | Disposition | Reason | Activation witnesses |
| --- | --- | --- | --- | --- | --- |
| allow-files | dependencies | yaml-rust2 | included | enabled by a selected feature path | allow-files/changie -> allow-files/dep:yaml-rust2 |
| allow-rust | dependencies | tree-sitter | included | enabled by a selected feature path | allow-rust/default -> allow-rust/syntax -> allow-rust/dep:tree-sitter |
| allow-rust | dependencies | tree-sitter-rust | included | enabled by a selected feature path | allow-rust/default -> allow-rust/syntax -> allow-rust/dep:tree-sitter-rust |

## Original-workspace topology preflight

Original locked-workspace topology contracts only; not direct-floor proof. These tests remain enforced before the product workspace is projected.

- Source commit: aa9a3a761f3db80e85734c70afeb183266da4e0d
- Source tree: 7870c02b5f5574eaa08ce176c8893cfd5ddf45dd
- Original manifest digest: sha256:v1:d702763d9eb363ffbacb153ff6961a3aafb387e0ea979e202d6899f50ca1ab4f
- Original lock digest: sha256:v1:dccad79fa88bb2cfe03674b5e69e907d3f00dad94d6f66595b0cd35e75e91120
- Target: x86_64-pc-windows-msvc
- Command: cargo test --locked --target x86_64-pc-windows-msvc --target-dir target/floor-proof/source-workspace-target -p cargo-allow --bin cargo-allow -- --format pretty --color never ci_lane_topology_tests::crate_sets_partition_the_workspace_exactly package_topology_enforcement_tests::topology_classifies_every_workspace_package_exactly product_package_topology_tests::current_v2_authorities_drive_package_candidate publish_order_validation_tests::publish_order_covers_all_workspace_members release_prep_tests::published_release_versions_match_workspace
- Standard-output digest: sha256:v1:df62ef4496b6f99adf9944675a144653a71912641d9f4a018405ffbffe01b852
- Standard-error digest: sha256:v1:714de079e99cbb7e64d5ad66ef538e2ae147d3e4ec6bb70946e4599918fc0a48

- Passed before projection: ci_lane_topology_tests::crate_sets_partition_the_workspace_exactly
- Passed before projection: package_topology_enforcement_tests::topology_classifies_every_workspace_package_exactly
- Passed before projection: product_package_topology_tests::current_v2_authorities_drive_package_candidate
- Passed before projection: publish_order_validation_tests::publish_order_covers_all_workspace_members
- Passed before projection: release_prep_tests::published_release_versions_match_workspace
