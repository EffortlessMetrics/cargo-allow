# Three-Product Move Map

> Generated from `policy/product-move-ledger.toml`. Edit the ledger,
> then update this projection through the checked renderer. Do not maintain a
> second ownership spreadsheet.

## Denominator

- Ledger schema: `cargo-allow.three-product-move-ledger.v1` generation `1`
- Entries: **37**
- Topology authority: Issue **#2612**
- Move/deletion owner: Issue **#2598**
- Current posture: inventory and target ratification only; no implementation moved.

### Status counts

- `RepositoryDecisionRequired`: **1**
- `TargetRatified`: **36**

### Disposition counts

- `CompatibilityAdapter`: **4**
- `DeleteAfterParity`: **1**
- `HistoricalReaderOnly`: **2**
- `MoveToCargoIntentApp`: **3**
- `MoveToIntentEngine`: **9**
- `MoveToIntentModel`: **4**
- `MoveToIntentProtocol`: **2**
- `MoveToProofEngine`: **1**
- `MoveToProofProviderApi`: **1**
- `MoveToRustSourceIndex`: **1**
- `MoveToSharedProtocol`: **2**
- `MoveToSharedSnapshot`: **2**
- `RemainCargoAllowCore`: **3**
- `RemainProviderOwned`: **1**
- `RepositoryDecisionRequired`: **1**

## Executable next frontier

1. **#2580** — `ProductCrateArchitectureV1` from this ledger and #2612.
2. **#2604** — `ProductPackageTopologyV1` without changing the ten-crate candidate.
3. **#2607** — register only shims required by the first source moves.
4. **#2606** — parity, stage, reachability, and cutover receipt contracts.
5. **#2582** — first real move: minimal `repo-protocol` envelope with parity evidence.

## Entries

### `MOVE-ALLOW-POLICY-SPEC-FACADE`

- Current: Hidden public spec_system module and re-export surface
- Target: `cargo-allow / allow-policy::compatibility.spec_system`
- Disposition: `DeleteAfterParity`
- Stage/status: `EmbeddedIntentDeletion` / `TargetRatified`
- Old path: `OldPathStillReachable`
- Removal: #2568 after #2601 cutover
- Next: Inventory consumers, then remove semantic exports after process cutover.
- Deletion output: allow-policy no longer exposes current intent semantics.

### `MOVE-ALLOW-POLICY-SPEC-TESTS`

- Current: Self-hosted spec-system model/compiler/profile tests
- Target: `cargo-intent / intent-engine::tests.compatibility`
- Disposition: `MoveToIntentEngine`
- Stage/status: `IntentEngine` / `TargetRatified`
- Old path: `TestFixtureOnly`
- Removal: #2586/#2606 migration corpus then #2568 cleanup
- Next: Copy/move fixtures into canonical intent parity corpus.
- Deletion output: Old allow-policy-only test ownership becomes deletable.

### `MOVE-CARGO-ALLOW-LEGACY-COMMANDS`

- Current: Legacy profile flags and command dispatch for spec-system audit/check/explain/worklist/doctor/init
- Target: `cargo-allow / cargo-allow::compatibility.intent_commands`
- Disposition: `CompatibilityAdapter`
- Stage/status: `CargoAllowCompatibilityCutover` / `TargetRatified`
- Old path: `OldPathStillReachable`
- Removal: #2601 operation disposition and #2568 deletion
- Next: Map every legacy operation to delegate/retire/historical disposition.
- Deletion output: Legacy command aliases remain projection-only or are removed on schedule.

### `MOVE-CARGO-ALLOW-SPEC-TESTS`

- Current: Legacy profile CLI, schema, bootstrap, doctor, explain, and worklist fixtures
- Target: `cargo-allow / cargo-allow::tests.intent_compatibility`
- Disposition: `HistoricalReaderOnly`
- Stage/status: `CargoAllowCompatibilityCutover` / `TargetRatified`
- Old path: `TestFixtureOnly`
- Removal: #2601/#2605; remove migration-only fixtures under #2568/#2559
- Next: Split canonical cargo-intent tests from cargo-allow compatibility tests.
- Deletion output: Core cargo-allow test suite no longer owns current intent semantics.

### `MOVE-CARGO-INTENT-APP-SPEC-SYSTEM`

- Current: Legacy spec-system command orchestration, private result DTOs, rendering preparation, and bootstrap
- Target: `cargo-intent / cargo-intent::application`
- Disposition: `MoveToCargoIntentApp`
- Stage/status: `CargoIntentFrontDoor` / `TargetRatified`
- Old path: `OldPathStillReachable`
- Removal: #2599 then #2601/#2568
- Next: Decompose request/rendering/bootstrap behavior; do not move the monolith unchanged.
- Deletion output: cargo-allow spec_system application implementation becomes deletable.

### `MOVE-CARGO-INTENT-PRECOMMIT-APP`

- Current: Staged precommit application orchestration, private report schema, rendering, and exits
- Target: `cargo-intent / cargo-intent::cli.change_status`
- Disposition: `MoveToCargoIntentApp`
- Stage/status: `CargoIntentFrontDoor` / `TargetRatified`
- Old path: `OldPathStillReachable`
- Removal: #2564/#2599 then #2601/#2568
- Next: Land canonical cargo-intent staged status and delegate legacy command.
- Deletion output: cargo-allow spec_precommit implementation becomes deletable.

### `MOVE-INTENT-CI-LANES`

- Current: Default workflow spec-system reports and integrated self-hosting checks
- Target: `cargo-intent / cargo-intent::ci`
- Disposition: `CompatibilityAdapter`
- Stage/status: `IndependentPackaging` / `TargetRatified`
- Old path: `OldPathStillReachable`
- Removal: #2569/#2559 after product packages exist
- Next: Split core, intent, proof, shared, compatibility, and integrated proof classes.
- Deletion output: Ordinary cargo-allow core CI no longer runs embedded intent authority.

### `MOVE-INTENT-CONFIG-AUTHORITY`

- Current: Legacy profile config, federation lane registration, and artifact catalog
- Target: `cargo-intent / intent-engine::source.compatibility`
- Disposition: `CompatibilityAdapter`
- Stage/status: `IntentEngine` / `RepositoryDecisionRequired`
- Old path: `OldPathStillReachable`
- Removal: #2600/#2586/#2568 explicit per-field disposition
- Next: Split cargo-allow source-exception and cargo-intent authority without bulk rewrite.
- Deletion output: Old current-intent interpretation becomes compatibility-only.

### `MOVE-INTENT-DOCS-SUPPORT`

- Current: User, agent, CI, profile, self-hosting, and support language for embedded spec-system
- Target: `cargo-intent / cargo-intent::docs`
- Disposition: `CompatibilityAdapter`
- Stage/status: `CargoAllowCompatibilityCutover` / `TargetRatified`
- Old path: `OldPathStillReachable`
- Removal: #2568/#2559 documentation split
- Next: Split product docs and make optional integrated journey explicit.
- Deletion output: Cargo-allow default docs stop implying intent/proof ownership.

### `MOVE-INTENT-ENGINE-GRAPH`

- Current: Compiled graph IR, nodes, edges, diagnostics, source locations, and compiler
- Target: `cargo-intent / intent-engine::compiler.graph`
- Disposition: `MoveToIntentEngine`
- Stage/status: `IntentEngine` / `TargetRatified`
- Old path: `OldPathStillReachable`
- Removal: #2586 parity, then #2568 deletion
- Next: Move compiler and private graph IR into intent-engine.
- Deletion output: allow-policy compiled graph implementation becomes deletable.

### `MOVE-INTENT-ENGINE-PHASE-POLICY`

- Current: Phase/precommit obligation evaluation and runtime-promotion policy
- Target: `cargo-intent / intent-engine::policy.phase_obligations`
- Disposition: `MoveToIntentEngine`
- Stage/status: `IntentEngine` / `TargetRatified`
- Old path: `OldPathStillReachable`
- Removal: #2586/#2564 parity, then #2568 deletion
- Next: Move phase-obligation policy into intent-engine.
- Deletion output: Old allow-policy phase evaluator becomes deletable.

### `MOVE-INTENT-ENGINE-QUERY-VIEW`

- Current: Raw graph traversal and self-hosted explain/query semantics
- Target: `cargo-intent / intent-engine::query`
- Disposition: `MoveToIntentEngine`
- Stage/status: `IntentEngine` / `TargetRatified`
- Old path: `OldPathStillReachable`
- Removal: #2586/#2585 parity then #2568
- Next: Move query semantics into intent-engine and rendering into cargo-intent.
- Deletion output: cargo-allow raw graph traversal becomes deletable.

### `MOVE-INTENT-ENGINE-RIPR-DIALECT`

- Current: RIPR-authored spec/slice dialect parsing and links
- Target: `cargo-intent / intent-engine::source.dialects.ripr`
- Disposition: `MoveToIntentEngine`
- Stage/status: `IntentEngine` / `TargetRatified`
- Old path: `OldPathStillReachable`
- Removal: #2586 parity; proof comparison owned by #2556
- Next: Move authored RIPR dialect support into intent-engine.
- Deletion output: Old RIPR dialect parser in allow-policy becomes deletable.

### `MOVE-INTENT-ENGINE-SOURCE-PROFILE`

- Current: Profile/config resolution, repository source adapters, document requirement adapter, and cross-source validation
- Target: `cargo-intent / intent-engine::source.profile`
- Disposition: `MoveToIntentEngine`
- Stage/status: `IntentEngine` / `TargetRatified`
- Old path: `OldPathStillReachable`
- Removal: #2586 parity, then #2568 deletion
- Next: Move repository-aware profile and validation behavior to intent-engine.
- Deletion output: Repository-aware intent loading leaves allow-policy.

### `MOVE-INTENT-ENGINE-WORKSPACE`

- Current: Hard-coded self-hosted source composition, paired graph comparison, and policy translation
- Target: `cargo-intent / intent-engine::compiler.workspace`
- Disposition: `MoveToIntentEngine`
- Stage/status: `IntentEngine` / `TargetRatified`
- Old path: `OldPathStillReachable`
- Removal: #2586 after #2583/#2587 substrate
- Next: Split generic compiler/query integration from hard-coded fixture composition.
- Deletion output: cargo-allow workspace compiler becomes deletable.

### `MOVE-INTENT-HISTORICAL-ACTIVE-GOAL`

- Current: Legacy active-goal parser and validation
- Target: `cargo-intent / intent-engine::source.compatibility.active_goal`
- Disposition: `HistoricalReaderOnly`
- Stage/status: `IntentEngine` / `TargetRatified`
- Old path: `HistoricalReaderOnly`
- Removal: #2586 compatibility reader disposition; delete from cargo-allow under #2568
- Next: Re-home as an explicitly historical compatibility dialect.
- Deletion output: Cargo-allow active-goal authority is deleted; historical parsing remains bounded.

### `MOVE-INTENT-MODEL-ARTIFACTS-SUPPORT`

- Current: Artifact authority/status records and support-claim contracts
- Target: `cargo-intent / intent-model::artifact_support`
- Disposition: `MoveToIntentModel`
- Stage/status: `IntentModel` / `TargetRatified`
- Old path: `OldPathStillReachable`
- Removal: #2584/#2586 parity and #2568 disposition
- Next: Separate pure artifact/support contracts from repository loading.
- Deletion output: Pure domain definitions leave allow-policy; only bounded structural compatibility remains.

### `MOVE-INTENT-MODEL-MAPPINGS`

- Current: Authored seams, evidence purposes, selectors, and mapping validation
- Target: `cargo-intent / intent-model::evidence_mapping`
- Disposition: `MoveToIntentModel`
- Stage/status: `IntentModel` / `TargetRatified`
- Old path: `OldPathStillReachable`
- Removal: #2584 parity, then #2568 deletion
- Next: Move pure authored mapping contracts into intent-model.
- Deletion output: Old mapping definitions in allow-policy become deletable.

### `MOVE-INTENT-MODEL-REQUIREMENTS`

- Current: Requirement IDs, status, source blocks, and pure requirement parsing contracts
- Target: `cargo-intent / intent-model::requirement`
- Disposition: `MoveToIntentModel`
- Stage/status: `IntentModel` / `TargetRatified`
- Old path: `OldPathStillReachable`
- Removal: #2584 parity, then #2568 deletion
- Next: Move pure requirement types and serialization fixtures into intent-model.
- Deletion output: Old requirement definitions in allow-policy become compatibility-only, then deletable.

### `MOVE-INTENT-MODEL-SLICES`

- Current: Implementation slice, target, claim, evidence, and support disposition contracts
- Target: `cargo-intent / intent-model::implementation_slice`
- Disposition: `MoveToIntentModel`
- Stage/status: `IntentModel` / `TargetRatified`
- Old path: `OldPathStillReachable`
- Removal: #2584 parity, then #2568 deletion
- Next: Move pure slice and disposition types into intent-model.
- Deletion output: Old slice definitions become compatibility-only, then deletable.

### `MOVE-INTENT-PROTOCOL-REPORT-CONTRACTS`

- Current: cargo-allow.spec-system.v1 schema constants, claim boundary, scanner limitations, and exports
- Target: `cargo-intent / intent-protocol::compatibility.legacy_results`
- Disposition: `MoveToIntentProtocol`
- Stage/status: `IntentProtocol` / `TargetRatified`
- Old path: `OldPathStillReachable`
- Removal: #2585 migration map then #2568 removal from core package
- Next: Define intent-protocol views and adapt legacy report output.
- Deletion output: Current intent schema authority leaves allow-report.

### `MOVE-INTENT-SCHEMA-ASSETS`

- Current: Legacy cargo-allow.spec-system.v1 JSON schema and catalog entry
- Target: `cargo-intent / intent-protocol::schemas.compatibility`
- Disposition: `MoveToIntentProtocol`
- Stage/status: `IntentProtocol` / `TargetRatified`
- Old path: `OldPathStillReachable`
- Removal: #2585/#2601/#2568
- Next: Add canonical intent schemas and mark old schema historical.
- Deletion output: Current schema producer registration leaves cargo-allow.

### `MOVE-INTENT-SELF-HOSTED-SOURCES`

- Current: Retained self-hosted requirement, implementation slice, seam, and evidence mapping
- Target: `cargo-intent / intent-engine::fixtures.self_hosted`
- Disposition: `MoveToIntentEngine`
- Stage/status: `IntentEngine` / `TargetRatified`
- Old path: `ExplicitlyDeferredWithinBound`
- Removal: #2586/#2558; paths may remain as retained authored sources
- Next: Compile unchanged sources through cargo-intent and record producer transition.
- Deletion output: Old cargo-allow producer claims become deletable; authored files may remain.

### `MOVE-INTENT-TEMPLATES`

- Current: Spec-system proposal/spec/ADR/plan/closeout/PR bootstrap templates
- Target: `cargo-intent / cargo-intent::assets.templates`
- Disposition: `MoveToCargoIntentApp`
- Stage/status: `CargoIntentFrontDoor` / `TargetRatified`
- Old path: `OldPathStillReachable`
- Removal: #2599/#2601/#2568
- Next: Move canonical bootstrap assets to cargo-intent package ownership.
- Deletion output: Intent templates leave cargo-allow core package.

### `MOVE-ISSUES-INTENT`

- Current: Intent semantic/compiler/query/application/edit implementation ownership
- Target: `cargo-intent / intent-engine::issues`
- Disposition: `MoveToIntentEngine`
- Stage/status: `ArchitectureInventory` / `TargetRatified`
- Old path: `ExplicitlyDeferredWithinBound`
- Removal: #2551/#2584/#2585/#2586/#2599/#2613 own implementation
- Next: Use named crate issues as implementation owners.
- Deletion output: No code deletion; stale issue ownership language is superseded.

### `MOVE-ISSUES-PROOF`

- Current: Proof planning, execution, receipts, currentness, contradiction, gate, and CLI ownership
- Target: `cargo-proof / proof-engine::issues`
- Disposition: `MoveToProofEngine`
- Stage/status: `ArchitectureInventory` / `TargetRatified`
- Old path: `ExplicitlyDeferredWithinBound`
- Removal: #2553/#2588/#2603/#2589 own implementation
- Next: Route proof implementation to named cargo-proof crates.
- Deletion output: No code deletion; stale combined-command ownership is superseded.

### `MOVE-ISSUES-PROVIDERS`

- Current: Cargo-allow, RIPR, and Hawk provider adapter implementation ownership
- Target: `cargo-proof / proof-provider-api::issues.providers`
- Disposition: `MoveToProofProviderApi`
- Stage/status: `ArchitectureInventory` / `TargetRatified`
- Old path: `ExplicitlyDeferredWithinBound`
- Removal: #2554 proof-adapter-cargo-allow; #2556 RIPR; #2555 Hawk; interface #2603
- Next: Use named adapter issues and one provider API.
- Deletion output: No code deletion; private integration proposals are rejected.

### `MOVE-ISSUES-SHARED-CUTOVER`

- Current: Shared substrate, architecture law, package topology, parity, shims, interop, and extraction-readiness owners
- Target: `shared / repo-protocol::issues.control`
- Disposition: `MoveToSharedProtocol`
- Stage/status: `ArchitectureInventory` / `TargetRatified`
- Old path: `ExplicitlyDeferredWithinBound`
- Removal: #2580/#2598/#2604/#2606/#2607/#2612 remain controlling issues
- Next: Keep one consistent controller graph.
- Deletion output: No code deletion.

### `MOVE-PACKAGE-RELEASE-TOPOLOGY`

- Current: Current ten-crate workspace, candidate set, package smoke, and release workflow assumptions
- Target: `cargo-allow / cargo-allow::package_topology`
- Disposition: `RepositoryDecisionRequired`
- Stage/status: `IndependentPackaging` / `TargetRatified`
- Old path: `ExplicitlyDeferredWithinBound`
- Removal: #2604 must ratify before any published product dependency changes
- Next: Classify planned packages without changing the supported release denominator.
- Deletion output: No current package deletion in #2598.

### `MOVE-REPO-PROTOCOL-TOOL-IDENTITY`

- Current: Tool identity, selection request/receipt, compatibility requirement, and executable digest
- Target: `shared / repo-protocol::tool_identity`
- Disposition: `MoveToSharedProtocol`
- Stage/status: `RepoProtocol` / `TargetRatified`
- Old path: `OldPathStillReachable`
- Removal: #2582 inventory/adaptation; product-specific selection remains local if necessary
- Next: Extract neutral identity/envelope, not cargo-allow process policy.
- Deletion output: Duplicate transport-only identity becomes deletable.

### `MOVE-REPO-SNAPSHOT-GIT`

- Current: Generic committed revision, tree/blob reads, snapshot identity, staged-index identity, exact staged bytes, and re-exports
- Target: `shared / repo-snapshot::git`
- Disposition: `MoveToSharedSnapshot`
- Stage/status: `RepoSnapshot` / `TargetRatified`
- Old path: `OldPathStillReachable`
- Removal: #2583 parity/caller migration
- Next: Extract generic Git/source access and preserve cargo-allow diff semantics.
- Deletion output: Generic revision/index implementation and wrappers become deletable.

### `MOVE-REPO-SNAPSHOT-SOURCE-VIEW`

- Current: Filesystem, staged-index, and committed-tree source view and exact reads
- Target: `shared / repo-snapshot::source_view`
- Disposition: `MoveToSharedSnapshot`
- Stage/status: `RepoSnapshot` / `TargetRatified`
- Old path: `OldPathStillReachable`
- Removal: #2583 parity and caller migration
- Next: Move generic source view into repo-snapshot.
- Deletion output: cargo-allow private source-view implementation becomes deletable.

### `MOVE-RUST-SOURCE-INDEX-SUBJECTS`

- Current: RustTest inventory, selector, resolution, source/body identity, package/target/module ownership, and re-exports
- Target: `shared / rust-source-index::rust.tests`
- Disposition: `MoveToRustSourceIndex`
- Stage/status: `RustSourceIndex` / `TargetRatified`
- Old path: `OldPathStillReachable`
- Removal: #2587 parity and consumer migration
- Next: Move source-bound structural subject indexing into rust-source-index.
- Deletion output: allow-rust test_subjects implementation and exports become deletable.

### `REMAIN-ALLOW-DIFF-MOVEMENT`

- Current: Cargo-allow finding, policy, lifecycle, selector, and ledger posture movement
- Target: `cargo-allow / allow-diff::movement`
- Disposition: `RemainCargoAllowCore`
- Stage/status: `RepoSnapshot` / `TargetRatified`
- Old path: `ExplicitlyDeferredWithinBound`
- Removal: Remain after #2583; only generic snapshot dependencies leave
- Next: Keep source-exception movement in allow-diff.
- Deletion output: No semantic deletion; only generic source helpers are removed.

### `REMAIN-ALLOW-RUST-SCANNERS`

- Current: Cargo-allow unsafe/panic/index/lint source-exception scanning and completeness
- Target: `cargo-allow / allow-rust::scanner`
- Disposition: `RemainCargoAllowCore`
- Stage/status: `RustSourceIndex` / `TargetRatified`
- Old path: `ExplicitlyDeferredWithinBound`
- Removal: Remain after #2587
- Next: Keep source-exception scanning in allow-rust.
- Deletion output: No semantic deletion; only structural subject code leaves.

### `REMAIN-CARGO-ALLOW-PROVIDER-PAYLOADS`

- Current: Cargo-allow source-exception reports, why/add plans, receipts, and provider payload semantics
- Target: `cargo-allow / allow-report::provider_payloads`
- Disposition: `RemainProviderOwned`
- Stage/status: `ProviderAdapters` / `TargetRatified`
- Old path: `ExplicitlyDeferredWithinBound`
- Removal: #2567 public provider contract; remain cargo-allow-owned
- Next: Expose through cargo-allow public process contract without moving ontology.
- Deletion output: No deletion; only private transport duplication may be removed.

### `REMAIN-MOVE-LEDGER-VALIDATOR`

- Current: Offline ThreeProductMoveLedgerV1 parser, validator, discovery check, negative fixtures, and projection renderer
- Target: `cargo-allow / allow-policy::product_move`
- Disposition: `RemainCargoAllowCore`
- Stage/status: `ArchitectureInventory` / `TargetRatified`
- Old path: `TestFixtureOnly`
- Removal: Retain through #2559, then delete or reduce after extraction closeout
- Next: Keep the migration denominator checked while moves land.
- Deletion output: Delete or reduce to supported long-lived architecture checks after #2559.

## Transition rules

- A bounded duplicate names parity cases, a cutover receipt, a latest shim stage,
  an owner, and a deletion condition.
- `OldPathStillReachable` is an inventory fact, not approval to retain a second
  evaluator after the selected cutover.
- Repository-authored intent sources may remain at their paths while producer and
  semantic ownership move.
- Cargo-allow provider payloads stay cargo-allow-owned and travel through neutral
  envelopes; no initial `cargo-allow-protocol` crate exists.
- Physical repository extraction still requires #2558, #2605, #2559, and a later
  explicit authorization.

## Claim boundary

Inventory and reviewed ownership dispositions for current repository sources; this ledger does not move code, prove parity, or authorize repository extraction.
