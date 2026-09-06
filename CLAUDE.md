# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

**Aptos CLI releases:** See `.cursor/skills/aptos-cli-release/SKILL.md` for version bumps and `crates/aptos/CHANGELOG.md` updates.

---

## Porto Labs Fork

**This repository is `porto-core`, a detached fork of `aptos-core` pinned at tag `aptos-node-v1.45.5`.**

It is being stripped down and rebranded into **Porto Chain**, the settlement and accounting layer for
Porto Labs. Everything below the "Project Overview" heading describes the upstream Aptos codebase and
remains accurate. This section describes what we are doing to it and why.

Forking is deliberate. Consensus is a solved, audited problem. Engineering effort goes into the
accounting layer, not into reinventing BFT.

### What Porto is

Porto is decentralized music streaming infrastructure. Artists, labels, publishers, and operators run
edge nodes that cache and stream licensed audio, replacing a centralized CDN. Every play is logged
on-chain, giving transparent, auditable, real-time royalties.

Payout split: 70% to rights holders, 15% to node operators, remainder to protocol treasury.

Porto is explicitly **not** an NFT music project. Prior crypto music projects tokenized ownership
without replacing delivery. Porto rebuilds the distribution layer itself. This is a positioning
commitment as much as a technical one, and it drives what gets removed from this codebase.

### Architecture intent

Porto Chain is an **app-chain**, not a general-purpose L1. Move is used as a verified accounting VM,
not as a dApp platform.

Eventual on-chain logic is three modules only:

1. **Stream Accounting** - receives attestations, maintains canonical play counts
2. **Payout Splitter** - applies royalty splits, handles multiple rights holders per track
3. **Governance Parameters** - fee percentages, thresholds, upgrade authorization

Everything behind versioned upgrades. Minimal attack surface.

Node modes from a single configurable binary: CDN mode (caches content, serves streams, submits
attestations), Validator mode (consensus plus accounting VM), Full mode (both, the default at launch).

Content layer is simple origin storage (S3-style) with node-level caching. No IPFS. This is licensed
music from opted-in artists, not permissionless file sharing.

State lives in RocksDB via the existing storage layer. Chain state **is** the ledger. Do not propose
adding an external database.

### What stays (do not remove or gut)

- **AptosMoveVM** and all of Move (`third_party/move/`). We are not forking or modifying the language
  or the VM.
- **AptosBFT consensus** (`consensus/`).
- **Staking and delegation** (in `aptos-move/framework/aptos-framework/`). Node operators stake to run
  nodes. Core to Porto's model even in testnet, and load-bearing for AptosBFT. Removing it breaks
  consensus.
- **Governance.** This is how protocol upgrades are issued. The versioned upgrade path must stay intact.
- **The token standard** (coin / fungible asset in `aptos-framework` and `aptos-stdlib`). This is what
  PRT, the native token, is issued against. Keep the standard, remove only NFT-specific layers.
- **CLI and peripheral deployment tooling** (`crates/aptos/`). Needed for running nodes and pushing
  module upgrades.

### What goes

- **`aptos-move/framework/aptos-token-objects/`** and NFT-specific modules, plus their tests and
  fixtures. Remove the NFT standards, **not** the underlying coin/fungible-asset token standard they
  build on. If they share code, flag the entanglement and wait rather than cutting.
- Anything else built purely for Aptos's own consumer ecosystem that nothing above depends on. Never
  act on this category unilaterally. Surface candidates with reasoning and wait.

### Naming and rebrand policy

This is Aptos code. It is correct for it to be named as such internally.

**Rename to Porto:**

- On-chain module names, addresses, and identifiers
- Token ticker: `APT` becomes `PRT`, including on-chain symbol and metadata
- Genesis config
- User-facing CLI output and branding text

**Leave as Aptos:**

- Rust crate names, internal identifiers, file paths, and code-level naming
- `AptosMoveVM` specifically, everywhere it appears. This is its proper name and stands as the lineage
  marker.
- License headers, copyright notices, and attribution comments referencing Aptos Labs. These are legal
  and lineage text, not branding.

Rule of thumb: if a user or a chain consumer sees it, rebrand it. If it is internal Rust, leave it.

### Working style expected in this fork

- **Piecemeal.** One concern at a time. Do not batch changes across modules.
- **Search and report before changing.** Show the file list, wait for go-ahead, then act.
- **Dry run sweeping or destructive changes** and surface the diff before applying.
- **Test gate after every piece** (see Porto Test Gate below).
- **On test failure: stop.** Report the failures. Do not silently fix and continue. Never work around a
  failing test by weakening or deleting it.
- **Flag entanglement rather than assuming.** If something slated for removal is depended on by
  something we are keeping, say so and wait.

### Porto test gate

Run after every piece of work:

```bash
cargo test -p aptos-framework          # Framework and Move unit tests
cargo test -p e2e-move-tests           # Move e2e tests
cargo test -p smoke-test               # E2E smoke tests
```

Plus `cargo build -p aptos-cached-packages` after any Move framework change, per the Move Framework
Changes section below. This is required, not optional.

### Current phase

Stripping and rebranding only. Porto's own Move modules (Stream Accounting, Payout Splitter, Governance
Parameters) are separate later work. Do not start them.

Phase 1 target is a London testnet: 5 nodes, 100 artists, working streams and payouts. Performance and
memory optimization is not a concern at that scale. Do not optimize prematurely.

### Out of scope until told otherwise

- Documentation pass
- Writing Porto's Move modules
- Performance or memory optimization

---

## Project Overview

Aptos Core is a layer 1 blockchain written primarily in Rust with Move smart contracts. It's a
production-grade system with 200+ workspace crates organized into major subsystems: consensus,
execution, storage, network, mempool, API, and Move VM.

## Essential Commands

### Build & Check
```bash
cargo build -p <package>           # Build a single package
cargo check -p <package>           # Quick compile check (faster than build)
cargo build --profile performance  # Optimized build with LTO
```

### Testing
```bash
cargo test -p <package>                    # Test a single package
cargo test -p <package> -- <test_name>     # Run a specific test
cargo test -p aptos-framework              # Framework tests
cargo test -p smoke-test                   # E2E smoke tests
cargo test -p e2e-move-tests               # Move e2e tests
```

### Linting & Formatting
```bash
./scripts/rust_lint.sh              # Full lint (clippy + fmt + sort + machete)
./scripts/rust_lint.sh --check      # Check-only mode for CI
cargo xclippy                       # Just clippy
cargo +nightly fmt                  # Just formatting
```

### Move Framework Changes
After modifying Move code in `aptos-move/framework/`:
```bash
cargo build -p aptos-cached-packages   # REQUIRED: rebuild cached packages
```

### Development Setup
```bash
./scripts/dev_setup.sh              # Install all build dependencies
./scripts/dev_setup.sh -y           # Include Move Prover tools (z3, boogie)
```

## Architecture Overview

### Core Transaction Flow
1. **API Layer** (`api/`) - REST endpoints receive transactions
2. **Mempool** (`mempool/`) - Transaction validation and ordering
3. **Consensus** (`consensus/`) - Byzantine fault-tolerant ordering
4. **Execution** (`execution/`) - Orchestrates VM execution
5. **Block Executor** (`aptos-move/block-executor/`) - Parallel execution via Block-STM
6. **Move** (`third_party/move/`) - Executes Move bytecode, Compiles Move, Verifies Move
7. **Storage** (`storage/`) - Persistent state (JellyfishMerkleTree)
8. **State Sync** (`state-sync/`) - Blockchain synchronization

### Move Framework Stack
- `aptos-move/framework/move-stdlib/` - Core Move stdlib
- `aptos-move/framework/aptos-stdlib/` - Aptos-specific stdlib
- `aptos-move/framework/aptos-framework/` - Core chain modules (coin, account, staking)
- `aptos-move/framework/aptos-token-objects/` - NFT standards **(Porto: slated for removal)**

### Key Crates
- `aptos-types` - Core type definitions used everywhere
- `aptos-vm` - VM integration and transaction execution
- `aptos-crypto` - Cryptographic primitives (security-critical)
- `aptos-api-types` - API request/response types

## Safety-Critical Code

These directories require extra care and should not be modified without explicit approval:
- `consensus/safety-rules/` - Byzantine fault tolerance
- `crates/aptos-crypto/` - Cryptographic implementations
- `secure/` - Security-critical modules
- `keyless/` - Keyless authentication

## Commit Message Format

```
[area] Brief description (50 char max)

Detailed explanation of why, not what.

Areas: consensus, mempool, network, storage, execution, vm, framework, api, cli, crypto, types
```

## Common Patterns

### Test Organization
- Unit tests: `#[cfg(test)] mod tests { ... }` in source files
- Integration tests: `<crate>/tests/` directories
- Move tests: `#[test]` attributes in `.move` files

### Error Handling
- Prefer thiserror / anyhow `Result` for error handling
- Use `expect()` over `unwrap()` with descriptive messages
- Use checked arithmetic (`checked_add`, `saturating_sub`, etc.)
- Infallible locks via `aptos-infallible` crate

### Pattern Matching
- Always use exhaustive `match` — never use a wildcard `_` arm to silence new enum variants

### Conditional Test Code
```rust
#[cfg(any(test, feature = "fuzzing"))]
fn test_helper() { ... }
```

## Move Coding Conventions

- Struct names: CamlCase (`OrderedMap`)
- Module names: snake_case (`ordered_map`)
- Function names: snake_case (`register_currency`)
- Constants: UPPER_SNAKE_CASE (`TREASURY_ADDRESS`)
- Import types at top-level, use functions qualified by module
