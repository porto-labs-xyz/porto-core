# Porto Core agent charter

## Product boundary

Porto Core implements London 0.1.0 as an Aptos Move protocol and dapp. Move is the accounting and payout authority. Rust operates the coordinator, node, private evidence retention, indexer and verifier. TypeScript is reserved for the web client.

Use the current London specification in `porto-labs-xyz/docs`, especially `london-0.1.0/09-move-contract-specification.md`. Do not reintroduce a RocksDB financial ledger, a commitment-only contract, PRT, a Porto chain, staking, an independent attestor network or a six-contract settlement system.

## Agent workflow

1. Work only on a GitHub issue in the `London 0.1.0` milestone.
2. Read the issue's owned paths, dependencies, acceptance IDs and forbidden scope.
3. Use an isolated branch and worktree. Do not edit a path owned by another active lane.
4. Update fixtures and schemas with any contract change.
5. Run the relevant Rust, Move and TypeScript checks. Report testnet, fixture and production evidence separately.
6. Open a pull request. Never merge, deploy, change GitHub protections or move funds.

## Safety and evidence

All money values use integer units. External chain actions are durable intents, not atomic with off-chain effects. Testnet proves integration only. A submitted transaction, an indexer display or a receipt does not prove settlement, listener attention or production readiness.
