// Devnet end-to-end smoke test for the rebuilt launchpad (Phase 9).
// Runs init platform -> create collection -> init crowdfund -> contribute (2 backers)
// -> finalize -> mint (2 passes) -> record revenue -> claim -> secondary transfer -> claim.
// Skips the DAS-dependent collectibles-display leg (needs a DAS-capable RPC).
//
// Run: node scripts/run-devnet-e2e.js   (registers ts-node, points provider at devnet)

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import { VertaCore } from "../target/types/verta_core";

const MPL_CORE = new PublicKey("CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d");
const SCALE = 1_000_000_000_000n;

const log = (msg: string): void => console.log(`  ${msg}`);
const sleep = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms));
// Public devnet RPC throttles hard (429) on call bursts (esp. the mpl-core mint CPI).
// Pace between transactions to stay under the limit.
const pace = (): Promise<void> => sleep(2_500);
const assert = (cond: boolean, msg: string): void => {
  if (!cond) throw new Error(`ASSERT FAILED: ${msg}`);
  console.log(`    ✓ ${msg}`);
};

async function main(): Promise<void> {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.vertaCore as Program<VertaCore>;
  const connection = provider.connection;
  const admin = (provider.wallet as anchor.Wallet).payer; // jbias = admin/delegate/vault owner/payer

  const seriesId = `e2e_${Date.now().toString().slice(-8)}`;
  const priceSol = new anchor.BN(50_000_000);
  const creatorBps = 8_000;
  const platformBps = 2_000;
  const backerRevenueBps = 3_000;

  const platformWallet = Keypair.generate();
  const productionWallet = Keypair.generate();
  const backer = Keypair.generate();
  const secondBacker = Keypair.generate();
  const buyer = Keypair.generate();
  const collection = Keypair.generate();

  const pda = (seeds: (Buffer | Uint8Array)[]): PublicKey =>
    PublicKey.findProgramAddressSync(seeds, program.programId)[0];
  const [platformConfig] = PublicKey.findProgramAddressSync(
    [Buffer.from("platform_authority")],
    program.programId
  );
  const crowdfund = pda([Buffer.from("crowdfund"), Buffer.from(seriesId)]);
  const revenueVault = pda([Buffer.from("revenue_vault"), crowdfund.toBuffer()]);
  const titleConfig = pda([Buffer.from("title_config"), Buffer.from(seriesId)]);
  const userVault = pda([Buffer.from("user_vault"), admin.publicKey.toBuffer()]);
  const contributionPda = (u: PublicKey): PublicKey =>
    pda([Buffer.from("contribution"), crowdfund.toBuffer(), u.toBuffer()]);
  const positionPda = (asset: PublicKey): PublicKey =>
    pda([Buffer.from("position"), asset.toBuffer()]);
  const bal = (a: PublicKey): Promise<number> => connection.getBalance(a);

  const fund = async (to: PublicKey, sol: number): Promise<void> => {
    const tx = new Transaction().add(
      SystemProgram.transfer({
        fromPubkey: admin.publicKey,
        toPubkey: to,
        lamports: Math.round(sol * LAMPORTS_PER_SOL),
      })
    );
    await provider.sendAndConfirm(tx, []);
  };

  console.log(`\nDevnet E2E — program ${program.programId.toBase58()}, series ${seriesId}\n`);
  log(`admin/payer: ${admin.publicKey.toBase58()} (${(await bal(admin.publicKey)) / LAMPORTS_PER_SOL} SOL)`);

  // 0. Fund actors from the provider wallet (devnet airdrops are rate-limited).
  log("funding actor wallets...");
  await fund(backer.publicKey, 0.25);
  await fund(secondBacker.publicKey, 0.25);
  await fund(buyer.publicKey, 0.1);

  // 1. Platform config (idempotent: skip if already present).
  log("initialize platform...");
  const existingPlatform = await connection.getAccountInfo(platformConfig);
  if (!existingPlatform) {
    await program.methods
      .initializePlatform(admin.publicKey, platformWallet.publicKey, priceSol, creatorBps, platformBps)
      .accounts({ platformConfig, payer: admin.publicKey, systemProgram: SystemProgram.programId } as any)
      .rpc();
    assert(true, "platform initialized");
  } else {
    assert(true, "platform already initialized (reused)");
  }

  // 2. Global Revenue Share Pass collection.
  log("create revenue share collection...");
  let collectionKey = collection.publicKey;
  const cfg0: any = await (program.account as any).platformConfig.fetch(platformConfig);
  if (cfg0.revenueShareCollection.equals(PublicKey.default)) {
    await program.methods
      .createRevenueShareCollection()
      .accounts({ platformConfig, admin: admin.publicKey, collection: collection.publicKey, coreProgram: MPL_CORE, systemProgram: SystemProgram.programId } as any)
      .signers([collection])
      .rpc();
    assert(true, `collection created ${collection.publicKey.toBase58()}`);
  } else {
    collectionKey = cfg0.revenueShareCollection;
    assert(true, `collection already set ${collectionKey.toBase58()} (reused)`);
  }

  // 3. Crowdfund campaign (goal 0.02 SOL) + graduated title config (creator_wallet = crowdfund PDA).
  const goal = new anchor.BN(20_000_000);
  const deadline = new anchor.BN(Math.floor(Date.now() / 1000) + 3_600);
  log("initialize crowdfund...");
  await program.methods
    .initializeCrowdfund(seriesId, goal, deadline, productionWallet.publicKey, backerRevenueBps)
    .accounts({ crowdfund, revenueVault, admin: admin.publicKey, platformConfig, systemProgram: SystemProgram.programId } as any)
    .rpc();
  assert(true, "crowdfund initialized");

  await program.methods
    .createTitleConfig(seriesId, admin.publicKey, crowdfund, creatorBps, platformBps, true)
    .accounts({ platformConfig, titleConfig, admin: admin.publicKey, systemProgram: SystemProgram.programId } as any)
    .rpc();
  assert(true, "graduated title config created (creator_wallet = crowdfund PDA)");

  // 4. Contributions: 30% / 70%.
  log("contribute (30/70)...");
  const firstAmount = new anchor.BN(6_000_000);
  const secondAmount = new anchor.BN(14_000_000);
  const firstContribution = contributionPda(backer.publicKey);
  const secondContribution = contributionPda(secondBacker.publicKey);
  await program.methods.contribute(firstAmount).accounts({ crowdfund, contribution: firstContribution, user: backer.publicKey, systemProgram: SystemProgram.programId } as any).signers([backer]).rpc();
  await program.methods.contribute(secondAmount).accounts({ crowdfund, contribution: secondContribution, user: secondBacker.publicKey, systemProgram: SystemProgram.programId } as any).signers([secondBacker]).rpc();
  const cf1: any = await (program.account as any).crowdfund.fetch(crowdfund);
  assert(cf1.totalRaised.toNumber() === goal.toNumber(), `total_raised == goal (${cf1.totalRaised.toNumber()})`);

  // 5. Finalize (releases escrow to production, freezes denominators).
  log("finalize...");
  const prodBefore = await bal(productionWallet.publicKey);
  await program.methods.finalizeCampaign().accounts({ crowdfund, admin: admin.publicKey, platformConfig, productionWallet: productionWallet.publicKey, systemProgram: SystemProgram.programId } as any).rpc();
  const cf2: any = await (program.account as any).crowdfund.fetch(crowdfund);
  assert(cf2.status.succeeded !== undefined, "status == Succeeded");
  assert(cf2.totalIssuedBps === backerRevenueBps, `total_issued_bps frozen == ${backerRevenueBps}`);
  assert((await bal(productionWallet.publicKey)) - prodBefore >= goal.toNumber() - 2_000_000, "escrow released to production wallet");

  // 6. Mint Revenue Share Passes.
  await pace();
  log("mint passes...");
  const firstAsset = Keypair.generate();
  const secondAsset = Keypair.generate();
  await program.methods.mintRevenueShare().accounts({ crowdfund, contribution: firstContribution, position: positionPda(firstAsset.publicKey), user: backer.publicKey, asset: firstAsset.publicKey, collection: collectionKey, platformConfig, coreProgram: MPL_CORE, systemProgram: SystemProgram.programId } as any).signers([backer, firstAsset]).rpc();
  await pace();
  await program.methods.mintRevenueShare().accounts({ crowdfund, contribution: secondContribution, position: positionPda(secondAsset.publicKey), user: secondBacker.publicKey, asset: secondAsset.publicKey, collection: collectionKey, platformConfig, coreProgram: MPL_CORE, systemProgram: SystemProgram.programId } as any).signers([secondBacker, secondAsset]).rpc();
  await pace();
  const pos1: any = await (program.account as any).sharePosition.fetch(positionPda(firstAsset.publicKey));
  const pos2: any = await (program.account as any).sharePosition.fetch(positionPda(secondAsset.publicKey));
  assert(pos1.shareBps === 900, `first share_bps == 900 (${pos1.shareBps})`);
  assert(pos2.shareBps === 2100, `second share_bps == 2100 (${pos2.shareBps})`);
  assert(pos1.shareBps + pos2.shareBps <= backerRevenueBps, "Σ share_bps ≤ backer_revenue_bps");

  // 7. Record revenue via the delegated vault path (set up user vault + delegate first).
  log("record revenue (0.01 SOL creator share)...");
  if (!(await connection.getAccountInfo(userVault))) {
    await program.methods.initializeUserVault().accounts({ userVault, user: admin.publicKey, systemProgram: SystemProgram.programId } as any).rpc();
  }
  await pace();
  await program.methods.setVaultDelegate(admin.publicKey, true).accounts({ userVault, user: admin.publicKey } as any).rpc();
  await pace();
  await program.methods.depositSol(new anchor.BN(30_000_000)).accounts({ userVault, user: admin.publicKey, systemProgram: SystemProgram.programId } as any).rpc();
  await pace();
  const revenueAmount = new anchor.BN(10_000_000);
  const vaultBefore = await bal(revenueVault);
  await program.methods
    .recordRevenue(revenueAmount)
    .accounts({ platformConfig, titleConfig, userVault, crowdfund, revenueVault, productionWallet: productionWallet.publicKey, delegate: admin.publicKey } as any)
    .rpc();
  const expectedBackerPool = Math.floor((revenueAmount.toNumber() * backerRevenueBps) / 10_000);
  assert((await bal(revenueVault)) - vaultBefore === expectedBackerPool, `revenue vault received backer pool (${expectedBackerPool})`);

  // 8. Claims (pro-rata).
  log("claim (both holders)...");
  const expectedFirst = Math.floor(expectedBackerPool * 0.3);
  const expectedSecond = expectedBackerPool - expectedFirst;
  const c1Before = await bal(backer.publicKey);
  await program.methods.claimRevenue().accounts({ crowdfund, revenueVault, position: positionPda(firstAsset.publicKey), asset: firstAsset.publicKey, user: backer.publicKey, systemProgram: SystemProgram.programId } as any).signers([backer]).rpc();
  assert((await bal(backer.publicKey)) - c1Before === expectedFirst, `holder 1 claimed ${expectedFirst}`);
  await pace();
  const c2Before = await bal(secondBacker.publicKey);
  await program.methods.claimRevenue().accounts({ crowdfund, revenueVault, position: positionPda(secondAsset.publicKey), asset: secondAsset.publicKey, user: secondBacker.publicKey, systemProgram: SystemProgram.programId } as any).signers([secondBacker]).rpc();
  assert((await bal(secondBacker.publicKey)) - c2Before === expectedSecond, `holder 2 claimed ${expectedSecond}`);

  // 9. Secondary transfer of holder 1's pass to buyer, then more revenue, then buyer claims.
  await pace();
  log("secondary transfer + post-sale claim...");
  const transferIx = new TransactionInstruction({
    programId: MPL_CORE,
    keys: [
      { pubkey: firstAsset.publicKey, isSigner: false, isWritable: true },
      { pubkey: collectionKey, isSigner: false, isWritable: false },
      { pubkey: backer.publicKey, isSigner: true, isWritable: true },
      { pubkey: backer.publicKey, isSigner: true, isWritable: false },
      { pubkey: buyer.publicKey, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: MPL_CORE, isSigner: false, isWritable: false },
    ],
    data: Buffer.from([14, 0]),
  });
  await provider.sendAndConfirm(new Transaction().add(transferIx), [backer]);
  await pace();
  await program.methods.recordRevenue(revenueAmount).accounts({ platformConfig, titleConfig, userVault, crowdfund, revenueVault, productionWallet: productionWallet.publicKey, delegate: admin.publicKey } as any).rpc();
  await pace();
  const buyerBefore = await bal(buyer.publicKey);
  await program.methods.claimRevenue().accounts({ crowdfund, revenueVault, position: positionPda(firstAsset.publicKey), asset: firstAsset.publicKey, user: buyer.publicKey, systemProgram: SystemProgram.programId } as any).signers([buyer]).rpc();
  assert((await bal(buyer.publicKey)) - buyerBefore === expectedFirst, "buyer (new owner) claims the inherited position");

  // Seller can no longer claim (no longer owns the asset).
  let sellerRejected = false;
  try {
    await program.methods.claimRevenue().accounts({ crowdfund, revenueVault, position: positionPda(firstAsset.publicKey), asset: firstAsset.publicKey, user: backer.publicKey, systemProgram: SystemProgram.programId } as any).signers([backer]).rpc();
  } catch {
    sellerRejected = true;
  }
  assert(sellerRejected, "previous owner can no longer claim after sale");

  console.log(`\n✅ Devnet E2E passed — series ${seriesId}\n`);
}

main().then(
  () => process.exit(0),
  (err) => {
    console.error("\n❌ Devnet E2E failed:\n", err);
    process.exit(1);
  }
);
