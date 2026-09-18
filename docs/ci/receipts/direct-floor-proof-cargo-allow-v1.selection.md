# Direct-floor selection evidence

Selection explanations only; the companion JSON owns proof dispositions.
These paths start at each member's selected default/requested features;
they are local activation witnesses, not a complete cross-crate feature graph.

- Product: cargo-allow
- Package roots: cargo-allow
- Selected closure: allow-core, allow-diff, allow-files, allow-inventory, allow-match, allow-policy, allow-policy-legacy, allow-report, allow-rust, cargo-allow, effortless-repo-edit, effortless-repo-protocol, effortless-repo-snapshot
- Starting source commit: d60511ad507beb50771f2a514c03a5630b83b3da
- Workspace projection: cargo-allow.direct-floor-product-workspace.v1
- Projected manifest digest: sha256:v1:365c40e42b0775c5561f284b3362527dd353eb4b25df6c1d31bc4bd5a6e9add7
- Certified member paths: crates/cargo-allow, crates/allow-core, crates/allow-policy, crates/allow-inventory, crates/allow-files, crates/allow-rust, crates/allow-match, crates/allow-report, crates/allow-diff, crates/allow-policy-legacy, crates/effortless-repo-protocol, crates/effortless-repo-snapshot, crates/effortless-repo-edit
- Execution member paths: crates/cargo-allow, crates/allow-core, crates/allow-policy, crates/allow-inventory, crates/allow-files, crates/allow-rust, crates/allow-match, crates/allow-report, crates/allow-diff, crates/allow-policy-legacy, crates/effortless-repo-protocol, crates/effortless-repo-snapshot, crates/effortless-repo-edit, crates/effortless-rust-source-index, crates/intent-model, crates/intent-protocol, crates/intent-engine
- Executed floor commit: 69faa7df48682240551794b8e971bbf771fdadac
- Executed floor tree: ec4b5dc4877366403dc2e45d1c05cc2a5474b022
- Receipt: direct-floor-proof-cargo-allow-v1.json
- Receipt SHA-256: sha256:v1:fd378e079c18c67e09299fabfddf754b274dfea4df198ff7e0d5d08dd5d2134c
- Manifest-set digest: 2bac8c4a8f2c583948dc365a5c77cb6f694e47f75a9ece6fd24980417ec2474f
- Starting lock digest: c6730d656e7686213bf37e4fa4474bbe61824039b31fdfb02d25bcee92e98775
- Executed floor-lock digest: sha256:v1:10824cb8d608d369ae221bdb1740957be162b887e7e2a0591fa5757a41b7da80

| Owner | Table | Optional dependency | Disposition | Reason | Activation witnesses |
| --- | --- | --- | --- | --- | --- |
| allow-files | dependencies | yaml-rust2 | included | enabled by a selected feature path | allow-files/changie -> allow-files/dep:yaml-rust2 |
| allow-rust | dependencies | tree-sitter | included | enabled by a selected feature path | allow-rust/default -> allow-rust/syntax -> allow-rust/dep:tree-sitter |
| allow-rust | dependencies | tree-sitter-rust | included | enabled by a selected feature path | allow-rust/default -> allow-rust/syntax -> allow-rust/dep:tree-sitter-rust |

## Original-workspace topology preflight

Original locked-workspace topology contracts only; not direct-floor proof. These tests remain enforced before the product workspace is projected.

- Source commit: d60511ad507beb50771f2a514c03a5630b83b3da
- Source tree: b5e0fb93afd647707d5cc216337f0e3bc85d1eba
- Original manifest digest: sha256:v1:d702763d9eb363ffbacb153ff6961a3aafb387e0ea979e202d6899f50ca1ab4f
- Original lock digest: sha256:v1:c6730d656e7686213bf37e4fa4474bbe61824039b31fdfb02d25bcee92e98775
- Target: x86_64-pc-windows-msvc
- Command: cargo test --locked --target x86_64-pc-windows-msvc --target-dir target/floor-proof/source-workspace-target -p cargo-allow --bin cargo-allow -- --format pretty --color never ci_lane_topology_tests::crate_sets_partition_the_workspace_exactly package_topology_enforcement_tests::topology_classifies_every_workspace_package_exactly product_package_topology_tests::current_v2_authorities_drive_package_candidate publish_order_validation_tests::publish_order_covers_all_workspace_members release_prep_tests::published_release_versions_match_workspace
- Standard-output digest: sha256:v1:b446b68124e25fb6cbc2b35e8619da8d53c19f4045b0ed2540a840ce1d96ead4
- Standard-error digest: sha256:v1:4ee30044f8bb33c7fc51440fccd365be30b0f027ba670fd915cf1d0577b23e3b

- Passed before projection: ci_lane_topology_tests::crate_sets_partition_the_workspace_exactly
- Passed before projection: package_topology_enforcement_tests::topology_classifies_every_workspace_package_exactly
- Passed before projection: product_package_topology_tests::current_v2_authorities_drive_package_candidate
- Passed before projection: publish_order_validation_tests::publish_order_covers_all_workspace_members
- Passed before projection: release_prep_tests::published_release_versions_match_workspace
