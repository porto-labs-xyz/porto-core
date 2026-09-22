# London 0.1.0 agent charter

## Ownership lanes

| Lane | Owns | Must not own |
| --- | --- | --- |
| Move protocol | `contracts/london`, protocol fixtures and Move tests | Web UI, node transport or off-chain financial authority |
| Rust services | `crates/core-domain`, `crates/services`, indexer and coordinator contracts | Move payout completion or client presentation |
| Rust node | `crates/node` | Grant issuance, allocation or payout decisions |
| TypeScript dapp | `apps/web` | Protocol rules, receipt authority or money-state invention |
| Integration | cross-lane fixtures, release profiles, CI and acceptance evidence | Lane feature work without an issue |

## Gate order

1. Shared schema and fixture baseline.
2. Aptos framework and test-asset ABI pin.
3. Move accounting and payout state machine.
4. Rust coordinator and node protocol integration.
5. TypeScript dapp integration with explicit testnet state.
6. Testnet end-to-end evidence, then a separately authorised Mainnet review.

Every issue states owned paths, dependencies, acceptance criteria and proof level. A green build is not a completed financial or pilot claim.
