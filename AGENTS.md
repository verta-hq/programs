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
`instructions/{admin,series,vault,autoplay,credits,crowdfunding,mod}.rs`

## Instruction groups

- **admin** — `initialize_platform`, `update_platform_config`
- **credits** — `record_credit_epoch` (admin-gated via PlatformConfig,
  `init_if_needed` on the `CreditEpoch` PDA `[b"credit_epoch", epoch_day_le]`
  so same-day re-runs overwrite idempotently). The daily public audit anchor
  for the backend's DB-authoritative coin economy: coins sold/spent/refunded
  + outstanding `usd_liability_micro` + a `balances_root` merkle slot
  (zeroed until that ships). Written by the backend checkpoint sweep after
  the nightly ledger reconciliation passes; design in
  `verta/docs/v-currency-architecture-review.md`
- **series** — `create_title_config`, `update_title_config` (royalty splits in BPS)
- **vault** — `initialize_user_vault`, `deposit_sol`, `withdraw_sol`,
  `set_vault_delegate` (authorizes the backend delegate)
- **autoplay** — `auto_unlock_episode` (on-chain receipt), `direct_payout`
  (no receipt). When a title has graduated from launchpad, both require typed
  crowdfund revenue accounts and route the creator share through
  `record_revenue` logic into the revenue vault / production split.
- **crowdfunding (USDC)** — `create_revenue_share_collection` (one-time
  global Core collection) · `initialize_crowdfund` (stores `crowdfund.mint`,
  creates escrow + revenue-vault **token PDAs**, authority = crowdfund PDA) →
  `contribute` (user ATA → escrow; separate `payer` signer carries rent so
  the platform can sponsor) → `finalize_campaign` (escrow → production ATA) /
  `fail_campaign`/`cancel_campaign` → `refund` (escrow → user ATA) /
  `mint_revenue_share` (also has a sponsored `payer`) →
  `deposit_revenue` (admin ATA → backer revenue-vault + accumulator bump +
  production ATA — the settlement-sweep entry point) → `claim_revenue`
  (revenue-vault → holder ATA). All amounts are micro-USDC (6 dp). The old
  SOL `record_revenue` (vault-coupled) and autoplay crowdfund routing remain
  compiled but are dead for token campaigns

## Vault & accounting layout (launchpad, USDC)

- **Escrow** is a token PDA `[b"escrow_vault", crowdfund]` (authority =
  crowdfund PDA) — holds USDC contributions; drained to the production
  wallet's ATA at `finalize_campaign`; source of `refund`.
- **Revenue** is a token PDA `[b"revenue_vault", crowdfund]` — holds only the
  backer share; `deposit_revenue` (platform admin, from the treasury ATA)
  deposits into it and bumps `crowdfund.cumulative_revenue_per_bps`
  atomically, paying the production share straight to the production ATA
  (no co-mingling).
- **Share accounting** is the program-owned `SharePosition` PDA
  (`[b"position", asset]`, `share_bps` + `claimed_checkpoint`), written once at
  mint and immutable — the Core asset's attributes mirror it for display only and
  are never trusted for payouts. `claim_revenue` pays
  `share_bps × (cumulative − checkpoint) / SCALE` in USDC from the
  revenue-vault token PDA to the asset's current on-chain owner's ATA. `SCALE = 1e12` (`constants.rs`).

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
  graduated-crowdfund revenue routing, BPS validation, and the CreditEpoch
  checkpoint (record, idempotent same-day re-record, non-admin rejection).
  Keep it in sync with instruction changes.
- Checked math everywhere; `has_one` constraints on admin/user-owned accounts;
  never remove constraints to make tests pass.

## Documentation sync

This file (and the README where relevant) is part of the change, not an
afterthought: any commit that alters commands, layout, env vars, auth, routes,
or cross-repo workflows MUST update the affected docs in the same commit.
Stale instructions are treated as bugs.
