<a href="https://aptos.dev">
	<img width="100%" src="./.assets/aptos_banner.png" alt="Aptos Banner" />
</a>

---

# Porto Chain

**Porto Chain** is the settlement and accounting layer for **Porto Labs** — decentralized music
streaming infrastructure. Artists, labels, publishers, and operators run edge nodes that cache and
stream licensed audio, replacing a centralized CDN. Every play is logged on-chain, giving
transparent, auditable, real-time royalties.

Porto is explicitly **not** an NFT music project. Prior crypto music projects tokenized ownership
without replacing delivery. Porto rebuilds the distribution layer itself.

Payout split on every play:

| Recipient | Share |
|---|---|
| Rights holders | 70% |
| Node operators | 15% |
| Protocol treasury | 15% |

This repository, `porto-core`, is a detached fork of [`aptos-core`](https://github.com/aptos-labs/aptos-core)
pinned at tag `aptos-node-v1.45.5`, being stripped down and rebranded into Porto Chain. Forking is
deliberate: consensus is a solved, audited problem, so engineering effort goes into the accounting
layer, not into reinventing BFT.

## Architecture

Porto Chain is an **app-chain**, not a general-purpose L1. Move is used as a verified accounting
VM, not as a dApp platform. Chain state lives in RocksDB via the existing storage layer — chain
state **is** the ledger, with no external database.

Eventual on-chain logic is three modules only, each behind versioned upgrades:

1. **Stream Accounting** — receives attestations, maintains canonical play counts
2. **Payout Splitter** — applies royalty splits, handles multiple rights holders per track
3. **Governance Parameters** — fee percentages, thresholds, upgrade authorization

### Node modes

A single configurable binary runs in one of three modes:

- **CDN mode** — caches content, serves streams, submits attestations
- **Validator mode** — consensus plus accounting VM
- **Full mode** — both (the default at launch)

### Content layer

Simple origin storage (S3-style) with node-level caching. No IPFS — this is licensed music from
opted-in artists, not permissionless file sharing.

## Roadmap

**Phase 1 — London testnet:** 5 nodes, 100 artists, working streams and payouts. Performance and
memory optimization are not a concern at this scale.

Porto's own Move modules (Stream Accounting, Payout Splitter, Governance Parameters) are later
work, after the stripping and rebranding of this fork is complete.

## Upstream

Porto Chain keeps Aptos's `AptosMoveVM`, Move language and VM, AptosBFT consensus, staking and
delegation, governance, and the coin / fungible-asset token standard. See `CLAUDE.md` for the
detailed fork policy (what stays, what goes, naming conventions).

* [Aptos Developer Network](https://aptos.dev)
* [Aptos Foundation](https://aptosfoundation.org/)

## Contributing

You can learn more about contributing to the upstream Aptos project by reading its
[Contribution Guide](https://github.com/aptos-labs/aptos-core/blob/main/CONTRIBUTING.md) and by
viewing its [Code of Conduct](https://github.com/aptos-labs/aptos-core/blob/main/CODE_OF_CONDUCT.md).

Aptos Core is licensed under the [Innovation-Enabling Source Code License](https://github.com/aptos-labs/aptos-core/blob/main/LICENSE).
