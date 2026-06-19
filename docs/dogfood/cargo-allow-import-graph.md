# cargo-allow Import Graph Dogfood

In-repository dogfood receipt for the spec-system `import_graph` surface after
the I1 generic import-root model (#1761) and I2 ecosystem adapters (#1763–#1765).
Documents how `cargo-allow check --profile spec-system --mode audit` reports
configured roots, adapter-discovered nodes/edges, and `missing_root` /
`broken_edge` diagnostics on this repository and on characterization fixtures.

Related: [I1 model closeout](../../plans/spec-system/closeouts/import-i1-model.md),
[I2 generic adapters](../../plans/spec-system/closeouts/import-i2-generic-adapters.md),
[I2 Kiro/Spec Kit](../../plans/spec-system/closeouts/import-i2-kiro-spec-kit-adapters.md),
[I2 xtask](../../plans/spec-system/closeouts/import-i2-xtask-adapter.md),
[GOAL-0003 partial progress](../../plans/spec-system/closeouts/goal-0003-partial-progress.md).

cargo-allow does not ship Kiro, Spec Kit, or xtask trees at the repository
root. This receipt uses the committed characterization fixtures under
`tests/fixtures/import/` for adapter proof and the live `.allow/profiles/spec-system.toml`
`[import_roots]` registry for the main-repo audit.

## Import Roots (Main Repository)

Configured in `.allow/profiles/spec-system.toml`:

```text
.allow/imports          owned      cargo-allow
.kiro                   imported   kiro
.specify                imported   spec-kit
.spec                     imported   generic-spec
.rails                    imported   generic-spec
.codex/goals            legacy     codex
xtask                   imported   xtask
```

## Main Repository Audit

```bash
cargo-allow check --profile spec-system --mode audit \
  --format json \
  --output docs/dogfood/receipts/cargo-allow-import-graph-repo.json
```

Observed result (2026-06-18, main `ac9c87ac`):

```text
import_graph.node_count:       9
import_graph.edge_count:       2
import_graph.diagnostic_count: 5
```

Discovered nodes on present roots:

```text
owned-imports              .allow/imports
owned-imports:README.md    .allow/imports/README.md
legacy-goals               .codex/goals
legacy-goals:README.md     .codex/goals/README.md
```

Configured but absent roots (expected `missing_root` diagnostics):

```text
kiro          .kiro
specify       .specify
generic-spec  .spec
generic-rails .rails
xtask         xtask
```

The five `missing_root` diagnostics are advisory import-graph signals, not
source-exception findings. They document configured adapters with no on-disk
tree yet rather than suppressing the roots from the graph.

Committed artifact:

- `docs/dogfood/receipts/cargo-allow-import-graph-repo.json`

## Fixture-Backed Adapter Audits

Run scoped audits from characterization fixture roots using the same
spec-system profile defaults:

### Kiro (`.kiro/`)

```bash
cargo-allow check --root tests/fixtures/import/kiro \
  --profile spec-system --mode audit \
  --format json \
  --output docs/dogfood/receipts/cargo-allow-import-graph-kiro.json
```

Observed:

```text
node_count: 11 | edge_count: 10 | diagnostic_count: 12
```

Representative discovered nodes:

```text
.kiro/specs/auth-feature/requirements.md   imported  high
.kiro/specs/auth-feature/design.md         imported  high
.kiro/specs/auth-feature/tasks.md          generated medium
.kiro/specs/session-timeout/bugfix.md      imported  high
```

Fixture `broken_edge` findings reference front-matter ids
(`FIXTURE-KIRO-REQ-001`, etc.) that are not registered in the main-repo
artifact ledger — expected characterization friction, not a product defect.

### Spec Kit (`.specify/`)

```bash
cargo-allow check --root tests/fixtures/import/spec-kit \
  --profile spec-system --mode audit \
  --format json \
  --output docs/dogfood/receipts/cargo-allow-import-graph-spec-kit.json
```

Observed:

```text
node_count: 12 | edge_count: 11 | diagnostic_count: 12
```

Representative discovered nodes:

```text
.specify/memory/constitution.md        imported  high
.specify/specs/001-auth/spec.md        imported  high
.specify/specs/001-auth/plan.md        imported  medium
.specify/specs/001-auth/tasks.md       generated medium
.specify/templates/spec-template.md    generated medium
```

### xtask command registry

```bash
cargo-allow check --root tests/fixtures/import/xtask \
  --profile spec-system --mode audit \
  --format json \
  --output docs/dogfood/receipts/cargo-allow-import-graph-xtask.json
```

Observed:

```text
node_count: 10 | edge_count: 7 | diagnostic_count: 10
```

Representative discovered nodes:

```text
xtask/commands.toml
xtask:commands.toml:check-file-policy
xtask:commands.toml:check-generated
```

Fixture `broken_edge` findings reference `FIXTURE-XTASK-CMD-*` ids and
`CARGO-ALLOW-SPEC-0002` targets absent from the fixture artifact ledger.

Committed artifacts:

- `docs/dogfood/receipts/cargo-allow-import-graph-kiro.json`
- `docs/dogfood/receipts/cargo-allow-import-graph-spec-kit.json`
- `docs/dogfood/receipts/cargo-allow-import-graph-xtask.json`

## Guard Checks

```bash
cargo-allow check --profile spec-system --mode audit \
  --format json --output target/cargo-allow/spec-system.json
cargo-allow check --mode no-new \
  --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md
```

Both commands pass on the branch that commits this receipt.

## What This Proves

- The I1 `import_graph` surface is observable through the documented spec-system
  audit command on this repository without chat memory.
- I2 Kiro, Spec Kit, and xtask adapters discover nodes, edges, roles, and
  provenance on committed characterization fixtures.
- Configured-but-absent roots emit visible `missing_root` diagnostics on the
  main repository rather than failing silently.
- Fixture `broken_edge` friction stays visible when imported front matter or
  registry ids are not registered in the canonical artifact ledger.

## What This Does Not Prove

- External `ripr` repository migration or R0 preflight execution.
- Full import mode product behavior (#1466).
- Semantic equivalence between imported ecosystems and the cargo-allow artifact
  ledger.
- Release readiness or version bump authorization.
