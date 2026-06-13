# AGENTS.md — verta programs

Anchor 0.32 (Rust 1.89) workspace with a single Solana program, `verta_core`
(`programs/verta_core/`). Cluster: devnet. Program ID:
`D2FTBiHZkkaGAQzLcivACqEuZnSAKatZzCg8ej5bVzUT` (launchpad rebuild, 2026-06-13;
prior `9o42…t7Pe` orphaned by the fresh-ID wipe). One of four sibling repos; the
`verta-hq/backend` repo loads this program's IDL and signs delegate
transactions server-side.

## Commands

```bash
anchor build                                 # compile Rust + regenerate IDL
anchor test                                  # build + LOCAL validator + mocha suite
anchor deploy --provider.cluster devnet      # devnet deploys are explicit only
node scripts/run-tests.js                    # tests only (validator must be running)
yarn check:backend-idl                       # fail if backend IDL drifted from target/idl
yarn generate:mobile-idl                     # regen mobile discriminator constants
```

Provider cluster is **localnet** (`anchor test` never touches devnet). The
ts-mocha CLI is dead on Node >= 23 (yargs ESM crash) — tests run through the
programmatic runner `scripts/run-tests.js`.

Anchor uses **yarn** and wallet `~/.config/solana/jbias.json` (see
`Anchor.toml`).

## Source (`programs/verta_core/src/`)

`lib.rs` (entrypoints) · `constants.rs` · `errors.rs` · `state/mod.rs` ·
`instructions/{admin,series,vault,autoplay,crowdfunding,mod}.rs`

## Instruction groups

- **admin** — `initialize_platform`, `update_platform_config`
- **series** — `create_title_config`, `update_title_config` (royalty splits in BPS)
- **vault** — `initialize_user_vault`, `deposit_sol`, `withdraw_sol`,
  `set_vault_delegate` (authorizes the backend delegate)
- **autoplay** — `auto_unlock_episode` (on-chain receipt), `direct_payout`
  (no receipt). When a title has graduated from launchpad, both require typed
  crowdfund revenue accounts and route the creator share through
  `record_revenue` logic into the revenue vault / production split.
- **crowdfunding** — `create_revenue_share_collection` (one-time global Core
  collection) · `initialize_crowdfund` → `contribute` →
  `finalize_campaign`/`fail_campaign`/`cancel_campaign` →
  `refund`/`mint_revenue_share` → `record_revenue`/`claim_revenue`

## Vault & accounting layout (launchpad)

- **Escrow** lives in the `crowdfund` PDA (`[b"crowdfund", series_id]`) — holds
  contributions; drained to the production wallet at `finalize_campaign`;
  source of `refund` (rent-guarded).
- **Revenue** lives in a separate `revenue_vault` system PDA
  (`[b"revenue_vault", crowdfund]`) — holds only the backer share; `record_revenue`
  deposits into it and bumps `crowdfund.cumulative_revenue_per_bps` atomically,
  paying the production share straight to the production wallet (no co-mingling).
- **Share accounting** is the program-owned `SharePosition` PDA
  (`[b"position", asset]`, `share_bps` + `claimed_checkpoint`), written once at
  mint and immutable — the Core asset's attributes mirror it for display only and
  are never trusted for payouts. `claim_revenue` pays
  `share_bps × (cumulative − checkpoint) / SCALE` from `revenue_vault` to the
  asset's current on-chain owner. `SCALE = 1e12` (`constants.rs`).

## Critical conventions

- **After any instruction/account change**, run in order:
  1. `anchor build`
  2. Copy `target/idl/verta_core.json` → `backend/src/solana/verta_core.json`
     (verify with `yarn check:backend-idl` — fails on drift)
  3. `yarn generate:mobile-idl` — regenerates
     `mobile/lib/core/solana/verta_core_idl.g.dart` (instruction
     discriminators); mobile must never hand-code discriminator bytes
- `tests/royalties.ts` covers `auto_unlock_episode` (splits + receipt +
  duplicate rejection), `direct_payout` (splits, no receipt), AutoPlayDisabled,
  graduated-crowdfund revenue routing, and BPS validation. Keep it in sync with
  instruction changes.
- Checked math everywhere; `has_one` constraints on admin/user-owned accounts;
  never remove constraints to make tests pass.

## Documentation sync

This file (and the README where relevant) is part of the change, not an
afterthought: any commit that alters commands, layout, env vars, auth, routes,
or cross-repo workflows MUST update the affected docs in the same commit.
Stale instructions are treated as bugs.
