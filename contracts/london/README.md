# London Move protocol

This directory contains the `porto_london` Aptos Move package baseline. It follows the published [Move-owned accounting and payout protocol](https://github.com/porto-labs-xyz/docs/blob/main/london-0.1.0/09-move-contract-specification.md): Move is the authority for protocol configuration, funded-period records, rights snapshots and accepted aggregate usage.

`Move.toml` pins the Aptos Framework testnet revision `ad8f79ad0f75e38e869d2e66509b9f7511aa6c86`. The baseline accepts the configured test-asset metadata object address as the immutable settlement-asset binding. It does not transfer, escrow or pay any asset. Escrow proof, allocation, obligation creation and atomic payout completion remain subsequent, separately tested state-machine work.

Run the Move unit tests with Aptos CLI 9.6.0:

```sh
aptos move test --package-dir contracts/london --dev
```

The tests cover role separation, the configured test-asset binding, exact retry, conflicting retry, lane-range overlap and closed-period rejection. They are Move unit-test evidence only, not a deployed testnet transfer, Mainnet settlement or a payout claim.
