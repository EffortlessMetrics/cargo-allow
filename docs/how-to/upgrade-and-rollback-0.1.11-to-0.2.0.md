# How-To: Upgrade from 0.1.11 to 0.2.0 and Roll Back

**Document ID**: `HOWTO-UPGRADE-ROLLBACK-0.1.11-TO-0.2.0`  
**Parent Campaign**: [#3768](https://github.com/EffortlessMetrics/cargo-allow/issues/3768)  
**Supported Channels**: `0.1.11` (Stable Rollback Baseline) → `0.2.0` (Next Release Line)  
**Classification**: `UpgradeRollbackGuideV1`  

---

## 1. Overview

`cargo-allow 0.2.0` preserves backward compatibility with `policy/allow.toml` files created under `0.1.11`. Upgrading does not require schema migration or allowlist regeneration. If you need to roll back to `0.1.11`, standard restore procedures ensure zero data loss.

---

## 2. Upgrade Journey (0.1.11 → 0.2.0)

### Step 1: Pre-Upgrade State Capture
Record your existing `0.1.11` ledger state:
```bash
cargo-allow --version
# Output: cargo-allow 0.1.11
cargo-allow check --mode no-new --receipt target/cargo-allow/pre-upgrade.receipt.json
```

### Step 2: Install 0.2.0
```bash
cargo install cargo-allow --version 0.2.0 --locked
cargo-allow --version
# Output: cargo-allow 0.2.0
```

### Step 3: Run Diagnostic Check
Verify policy compatibility and tool status:
```bash
cargo-allow doctor
cargo-allow check --mode no-new --receipt target/cargo-allow/post-upgrade.receipt.json
```

### Step 4: Update CI Workflows
Update your GitHub Actions workflow file (`.github/workflows/ci.yml`):
```yaml
- name: Run cargo-allow check
  run: |
    cargo install cargo-allow --version 0.2.0 --locked
    cargo-allow check --mode no-new --format markdown --output target/cargo-allow/check.md
```

---

## 3. Compatibility Matrix

| Surface | 0.1.11 Behavior | 0.2.0 Behavior | Compatibility Status |
|---|---|---|---|
| `policy/allow.toml` | Schema v1 | Schema v1 / v2 compatible | **ReadUnchanged** |
| `cargo-allow check` | Evaluates all tracked files | Evaluates all tracked files with `--mode no-new` | **ReadWithCompatibleProjection** |
| `cargo-allow why` | Explains finding | Explains finding + generates `--plan` | **ReadWithCompatibleProjection** |
| `cargo-allow add` | Adds allow entry | Adds entry via `--update` or `--from-plan` | **ReadWithCompatibleProjection** |
| `cargo-allow diff` | Diffs against baseline | Diffs against baseline commit | **ReadUnchanged** |
| `cargo-allow doctor` | Basic health check | Enhanced health and environment diagnostics | **ReadWithCompatibleProjection** |

---

## 4. Rollback Journey (0.2.0 → 0.1.11)

If you must roll back to `0.1.11`:

### Step 1: Reinstall 0.1.11
```bash
cargo install cargo-allow --version 0.1.11 --locked
cargo-allow --version
# Output: cargo-allow 0.1.11
```

### Step 2: Clean 0.2.0 Generated Artifacts
```bash
rm -rf target/cargo-allow
```

### Step 3: Revert Workflow Changes
```bash
git checkout HEAD -- .github/workflows/ci.yml
```

### Step 4: Verify 0.1.11 Operation
```bash
cargo-allow check --mode no-new
```

---

## 5. Non-Goals and Claim Boundaries

- **No Silent Policy Widening**: Upgrading never broadens exemptions automatically.
- **Explicit Transactions**: All policy changes continue to require explicit `--update` or `--from-plan` commands.
- **Rollback Safety**: Reinstalling `0.1.11` restores exact prior tool behavior without modifying unrelated repository files.
