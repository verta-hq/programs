# AGENTS.md — verta programs

Anchor 0.32.2 (Rust 1.89) workspace with a single Solana program, `verta_core`
(`programs/verta_core/`). Program ID (fresh keypair for the launch-readiness
rewrite, same ID on devnet and mainnet):
`A3CuYy9r2tpU4BafDSEbasuvFJAG3Nvet8oKAv6J1Pzr`. The prior programs
(`D2FT…VzUT`, `9o42…t7Pe`) are orphaned. One of four sibling repos; the
`verta-hq/backend` repo consumes the **generated Kit client** this repo emits
(`backend/src/solana/generated/verta_core/`) and signs operator transactions
server-side.

## Commands

```bash
anchor build                        # compile Rust + regenerate target/idl
yarn build                          # anchor build + generate clients (canonical ritual)
anchor test                         # build + LOCAL validator (upgradeable) + mocha suite
node scripts/run-tests.js           # tests only (validator must be running)
yarn generate:clients               # Codama → clients/js (local) + backend generated client + idl/verta_core.json
yarn check:clients                  # fail if the backend client or idl/ drifted from target/idl
yarn smoke:devnet                   # read-only devnet smoke (program executable + PlatformConfig sanity)
yarn init:platform                  # ceremony: initialize_platform (env ADMIN_AUTHORITY_ADDRESS, PLATFORM_OPS_ADDRESS, USDC_MINT)
anchor deploy --provider.cluster devnet --program-keypair target/deploy/verta_core-keypair.json
yarn lint                           # prettier --check (scripts, tests, migrations)
```

Provider cluster is **localnet** (`anchor test` never touches devnet) and the
test validator deploys the program **upgradeable** (`[test] upgradeable = true`)
so `initialize_platform` can verify the deployer against `ProgramData`. The
ts-mocha CLI is dead on Node >= 23 (yargs ESM crash) — tests run through the
programmatic runner `scripts/run-tests.js`. Anchor uses **yarn**
(`.yarnrc` ignores the node>=24 engine flag on `@solana-program/*`, which are
plain JS) and the developer wallet in `Anchor.toml` is for localnet/devnet only
— it must never hold mainnet authority.

The program keypair lives at `target/deploy/verta_core-keypair.json`
(gitignored). Back it up to the ops password manager; losing it only blocks a
from-scratch redeploy to the same ID, never an upgrade.

## Source (`programs/verta_core/src/`)

`lib.rs` (entrypoints) · `constants.rs` · `errors.rs` · `state/mod.rs` ·
`instructions/{admin,series,credits,crowdfunding,mod}.rs`

## Authority model

`PlatformConfig` (`[b"platform_authority"]`) holds two live keys plus a
pending one:

| Field | Holder | Gates |
|---|---|---|
| `admin` | Squads multisig vault (`ADMIN_AUTHORITY_ADDRESS`) | `update_platform_config` (unpause, operator rotation, settlement mint), `propose_admin`, `create_revenue_share_collection`, `finalize_campaign`, `fail_campaign`, `cancel_campaign` |
| `operator` | hot backend ops key (`PLATFORM_OPS_PRIVATE_KEY`) | `create/update_title_config`, `initialize_crowdfund`, `deposit_revenue`, `record_credit_epoch`, `pause` |
| `pending_admin` | set by `propose_admin`, consumed by `accept_admin` (two-step handoff) | — |

`pause` may be signed by the operator **or** the admin; only the admin can
unpause (`update_platform_config { paused: false }`). `paused` blocks every
money-moving instruction: `initialize_crowdfund`, `contribute`,
`finalize_campaign`, `refund`, `mint_revenue_share`, `deposit_revenue`,
`claim_revenue`. `fail_campaign`/`cancel_campaign` stay available while paused
(they only flip status).

`initialize_platform` is **not** permissionless: the payer must be the
program's upgrade authority (`ProgramData.upgrade_authority_address`), and it
takes `admin`/`operator` as args plus the settlement mint as an account.
`settlement_mint` is the only mint `initialize_crowdfund` accepts
(`InvalidSettlementMint`); it may be legacy SPL Token or Token-2022 — every
token account is an `InterfaceAccount`, every CPI is `transfer_checked`, and
each transfer asserts the destination balance delta (`TransferAmountMismatch`).

## Instruction groups

- **admin** — `initialize_platform(admin, operator)`, `update_platform_config(new_operator?, paused?)`
  (+ optional `new_settlement_mint` account), `pause`, `propose_admin`, `accept_admin`
- **credits** — `record_credit_epoch` (operator; `init_if_needed` on
  `[b"credit_epoch", epoch_day_le]` so same-day re-runs overwrite). Daily
  public audit anchor for the DB-authoritative coin economy; design in
  `verta/docs/v-currency-architecture-review.md`
- **series** — `create_title_config`, `update_title_config` (operator; royalty
  splits in BPS, `creator_wallet == crowdfund PDA` marks a graduated title)
- **crowdfunding (USDC)** — `create_revenue_share_collection` (admin, one-time
  global Core collection) · `initialize_crowdfund` (operator; validates
  goal/deadline/bps/wallet; creates escrow + revenue-vault token PDAs,
  authority = crowdfund PDA) → `contribute` (user ATA → escrow; sponsored
  `payer` is recorded as `Contribution.rent_payer`) → `finalize_campaign`
  (admin; escrow → production ATA, ATA created on demand) /
  `fail_campaign` (admin; deadline passed + goal missed) /
  `cancel_campaign` (admin; **before** the deadline only) → `refund`
  (escrow → user ATA, decrements `total_raised`, closes the contribution to
  `rent_payer`) / `mint_revenue_share` (sponsored `payer`) →
  `deposit_revenue` (operator ATA → backer revenue vault + accumulator bump +
  production ATA — the settlement-sweep entry point) → `claim_revenue`
  (revenue vault → holder ATA, created on demand by the sponsored `payer`).
  All amounts are micro-units of the settlement mint (USDC = 6 dp).

No SOL moves through this program anymore: the user vault, delegate unlocks,
`direct_payout` and the SOL `record_revenue` path were removed in the
launch-readiness rewrite.

## Vault & accounting layout (launchpad, USDC)

- **Escrow** is a token PDA `[b"escrow_vault", crowdfund]` (authority =
  crowdfund PDA) — holds contributions; drained to the production wallet's
  ATA at `finalize_campaign`; source of `refund`.
- **Revenue** is a token PDA `[b"revenue_vault", crowdfund]` — holds only the
  backer share; `deposit_revenue` deposits into it and bumps
  `crowdfund.cumulative_revenue_per_bps` atomically, paying the production
  share straight to the production ATA (no co-mingling).
- **Share accounting** is the program-owned `SharePosition` PDA
  (`[b"position", asset]`, `share_bps` + `claimed_checkpoint`), written once at
  mint and immutable — the Core asset's attributes mirror it for display only and
  are never trusted for payouts. `claim_revenue` pays
  `share_bps × (cumulative − checkpoint) / SCALE` to the asset's current
  on-chain owner's ATA. `SCALE = 1e12` (`constants.rs`).

## Client generation (replaces the old IDL copy + mobile discriminator steps)

`scripts/generate-clients.js` turns `target/idl/verta_core.json` into a
Solana Kit client with Codama (`@codama/nodes-from-anchor` +
`@codama/renderers-js`), adds explicit PDA helpers (`findPlatformConfigPda`,
`findTitleConfigPda`, `findCrowdfundPda`, `findContributionPda`,
`findSharePositionPda`, `findEscrowVaultPda`, `findRevenueVaultPda`,
`findCreditEpochPda`), and writes:

1. `clients/js/verta_core/` — gitignored, consumed by `tests/`
2. `../backend/src/solana/generated/verta_core/` — **committed in the backend repo**
3. `idl/verta_core.json` — the canonical IDL, committed here

**After any instruction/account change**: `yarn build`, run the tests, then
commit `idl/` here and `src/solana/generated/` in the backend in the same
cross-repo change. CI runs `yarn check:clients` and fails on drift. Mobile has
no verta_core consumer anymore (it signs backend-built transactions), so
there is no mobile artifact to regenerate.

## Tests (`tests/`)

Kit-based mocha suites against the generated client; no Anchor TS client.
Files run in sorted order: `00_setup.ts` (root hook: funds actors, creates
SPL **and** Token-2022 test mints, derives `ProgramData`; exports the
helpers), `01_platform.ts` (upgrade-authority gate, collection, two-step
admin, operator rotation, pause/unpause roles, settlement-mint gate),
`02_title_config.ts`, `03_crowdfund_lifecycle.ts` (parameter validation,
sponsored contribute + rent payer, pause matrix, finalize roles, expiry +
fail + refund with rent-to-sponsor, cancel-before-deadline, full Token-2022
lifecycle), `04_revenue.ts` (operator-only deposit, accumulator, secondary
sale, overfunding cap, cross-series binding, pro-rata, drift, spoofed asset,
pause) and `05_credit_epoch.ts`. Keep them in sync with instruction changes; never
relax a constraint to make a test pass.

## CI

`.github/workflows/ci.yml` (PR + push to main): Rust 1.89, Solana CLI,
Anchor 0.32.2 (cached), `yarn lint`, `anchor build`, client-drift check,
`anchor test`. Devnet/mainnet deploys stay manual and explicit.

## Deploy / key ceremony (summary — full runbook in `verta/docs/solana-launch-readiness.md`)

1. `anchor deploy --provider.cluster <cluster> --program-keypair target/deploy/verta_core-keypair.json`
   with the deployer wallet as upgrade authority.
2. `SOLANA_RPC_URL=… ANCHOR_WALLET=… ADMIN_AUTHORITY_ADDRESS=… PLATFORM_OPS_ADDRESS=… USDC_MINT=… yarn init:platform`
   (`migrations/deploy.ts`, idempotent) — the deployer signs `initialize_platform`.
3. Hand the upgrade authority to the Squads vault
   (`solana program set-upgrade-authority`).
4. `create_revenue_share_collection` is an admin instruction: build it with
   the backend's `admin-tx.ts` and execute through Squads.

## Documentation sync

This file (and the README where relevant) is part of the change, not an
afterthought: any commit that alters commands, layout, env vars, auth, routes,
or cross-repo workflows MUST update the affected docs in the same commit.
Stale instructions are treated as bugs.
