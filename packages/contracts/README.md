# Shared contracts

This is the single source of truth for versioned schemas, golden fixtures and generated HTTP/client contracts. `schemas/london.v1.json` describes the initial shared boundary for configuration, funded periods, immutable rights snapshots and bounded aggregate-usage batches. The fixtures use opaque UUIDs, pinned-width Aptos addresses and canonical unsigned integer decimal strings for every monetary quantity.

Run the compatibility boundary check with:

```sh
npm run compatibility --prefix packages/contracts
```

The command verifies the schema identity, integer-money vectors, rights split, range shape and retry/overlap vectors. It is fixture-pass evidence only. It neither deploys nor funds an account, and does not establish on-chain settlement.
