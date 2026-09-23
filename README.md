# Verta Solana Programs

Anchor workspace for `verta_core`, the on-chain half of the Verta launchpad:
USDC crowdfunding escrow, Revenue Share Passes (Metaplex Core), per-title
royalty splits, and the daily coin-economy checkpoint. Operational details,
the authority model and the client-generation ritual live in
[AGENTS.md](AGENTS.md).

## Layout

- `programs/verta_core/` — the program (Rust, Anchor 0.32.2)
- `tests/` — Kit-based mocha suites run against a local validator
- `scripts/generate-clients.js` — Codama client generation (backend + local)
- `idl/verta_core.json` — canonical IDL committed after every build
- `migrations/deploy.ts` — `initialize_platform` ceremony script

## Quick start

```bash
yarn install
yarn build          # anchor build + generate clients
anchor test         # local validator + full suite
```

Toolchain: Rust 1.89 (`rust-toolchain.toml`), Solana CLI 3.x, Anchor 0.32.2
via `avm`, Node 22 + yarn.
