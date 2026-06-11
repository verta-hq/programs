# AGENTS.md — verta programs

Anchor 0.32 (Rust 1.89) workspace with a single Solana program, `verta_core`
(`programs/verta_core/`). Cluster: devnet. Program ID on `main`:
`9o42AnCRp9cnpEzA8np5MKxpstoRG2v9DosaKBn6t7Pe`. One of four sibling repos; the
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
  (no receipt). Both route revenue to a graduated crowdfund via
  `update_crowdfund_revenue` (PDA-checked account punning in `autoplay.rs` —
  silently skips non-crowdfund accounts; review before mainnet).
- **crowdfunding** — `initialize_crowdfund` → `contribute` →
  `finalize_campaign`/`fail_campaign` → `refund`/`claim_equity_nft` →
  `claim_revenue`

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
  crowdfund revenue accrual via the graduated-crowdfund creator wallet, and
  BPS validation. Keep it in sync with instruction changes.
- Checked math everywhere; `has_one` constraints on admin/user-owned accounts;
  never remove constraints to make tests pass.
