# Porto Core

Porto Core is the London 0.1.0 Aptos Move protocol and music-distribution dapp.

Move owns funded-period accounting, accepted aggregate usage, bounded settlement, unpaid obligations and atomic payout completion. Rust owns the coordinator, participant streaming node, event indexer and independent verifier. TypeScript owns the web client.

The implementation starts on Aptos testnet with a configured test asset. Testnet verifies integration only. It is not evidence of production funding, native-USDC settlement or a completed pilot.

## Layout

| Path | Responsibility |
| --- | --- |
| `contracts/london` | Aptos Move protocol package |
| `crates/core-domain` | Rust domain types and deterministic calculation primitives |
| `crates/services` | Rust coordinator, APIs and projections |
| `crates/node` | Rust participant streaming node |
| `crates/verifier` | Rust independent verifier and CLI |
| `apps/web` | TypeScript dapp client, derived from the approved prototype |
| `packages/contracts` | Versioned schemas, OpenAPI, fixtures and generated client contracts |
| `ops/testnet` | Testnet release profiles and operational runbooks |

## Delivery rules

All work begins from a GitHub issue in the `London 0.1.0` milestone. Read [the agent charter](docs/AGENT_CHARTER.md) and the current [London Move protocol specification](https://github.com/porto-labs-xyz/docs/blob/main/london-0.1.0/09-move-contract-specification.md) before changing code.

No agent self-merges, deploys, funds an account or treats a testnet transaction as a real payout.
