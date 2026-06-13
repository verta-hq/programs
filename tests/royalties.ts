import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { expect } from "chai";
import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import { VertaCore } from "../target/types/verta_core";

describe("verta_core royalties", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.vertaCore as Program<VertaCore>;
  const connection = provider.connection;

  const admin = Keypair.generate();
  const user = Keypair.generate();
  const delegate = Keypair.generate();
  const creator = Keypair.generate();
  const platformWallet = Keypair.generate();
  const productionWallet = Keypair.generate();
  const backer = Keypair.generate();
  const secondBacker = Keypair.generate();
  const buyer = Keypair.generate();
  const outsider = Keypair.generate();
  const revenueShareCollection = Keypair.generate();

  const priceSol = new anchor.BN(50_000_000);
  const creatorBps = 8_000;
  const platformBps = 2_000;
  const backerRevenueBps = 3_000;
  const scale = new anchor.BN("1000000000000");
  const titleId = "bangkok_nights_s1";
  const crowdfundTitleId = "crowdfund_s1";
  const mplCoreProgram = new PublicKey(
    "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d"
  );

  const [platformConfig] = PublicKey.findProgramAddressSync(
    [Buffer.from("platform_authority")],
    program.programId
  );
  const [titleConfig] = titleConfigPda(titleId);
  const [crowdfundTitleConfig] = titleConfigPda(crowdfundTitleId);
  const [userVault] = PublicKey.findProgramAddressSync(
    [Buffer.from("user_vault"), user.publicKey.toBuffer()],
    program.programId
  );
  const [crowdfund] = PublicKey.findProgramAddressSync(
    [Buffer.from("crowdfund"), Buffer.from(crowdfundTitleId)],
    program.programId
  );
  const [revenueVault] = PublicKey.findProgramAddressSync(
    [Buffer.from("revenue_vault"), crowdfund.toBuffer()],
    program.programId
  );

  function crowdfundPda(seriesId: string): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("crowdfund"), Buffer.from(seriesId)],
      program.programId
    );
  }

  function revenueVaultPda(campaign: PublicKey): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("revenue_vault"), campaign.toBuffer()],
      program.programId
    );
  }

  function contributionPda(
    campaign: PublicKey,
    userAddress: PublicKey
  ): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [
        Buffer.from("contribution"),
        campaign.toBuffer(),
        userAddress.toBuffer(),
      ],
      program.programId
    );
  }

  function positionPda(asset: PublicKey): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("position"), asset.toBuffer()],
      program.programId
    );
  }
  const [backerContribution] = PublicKey.findProgramAddressSync(
    [
      Buffer.from("contribution"),
      crowdfund.toBuffer(),
      backer.publicKey.toBuffer(),
    ],
    program.programId
  );

  function titleConfigPda(seriesId: string): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("title_config"), Buffer.from(seriesId)],
      program.programId
    );
  }

  function unlockReceiptPda(series: PublicKey, episodeId: string): PublicKey {
    return PublicKey.findProgramAddressSync(
      [
        Buffer.from("unlock_receipt"),
        user.publicKey.toBuffer(),
        series.toBuffer(),
        Buffer.from(episodeId),
      ],
      program.programId
    )[0];
  }

  function creatorShare(amount: anchor.BN): number {
    return amount.muln(creatorBps).divn(10_000).toNumber();
  }

  async function balance(address: PublicKey): Promise<number> {
    return connection.getBalance(address);
  }

  async function airdrop(wallet: PublicKey, sol = 5): Promise<void> {
    const signature = await connection.requestAirdrop(
      wallet,
      sol * LAMPORTS_PER_SOL
    );
    const latestBlockhash = await connection.getLatestBlockhash();
    await connection.confirmTransaction(
      {
        signature,
        ...latestBlockhash,
      },
      "confirmed"
    );
  }

  async function expectAnchorError(
    action: () => Promise<unknown>,
    expectedCode: string
  ): Promise<void> {
    try {
      await action();
      expect.fail(`Expected ${expectedCode}`);
    } catch (error) {
      const anchorError = error as anchor.AnchorError;
      expect(anchorError.error.errorCode.code).to.equal(expectedCode);
    }
  }

  async function initializeCampaign(
    seriesId: string,
    goal: anchor.BN,
    deadline: anchor.BN,
    productionAddress = productionWallet.publicKey,
    backerBps = backerRevenueBps
  ): Promise<{ campaign: PublicKey; vault: PublicKey }> {
    const [campaign] = crowdfundPda(seriesId);
    const [vault] = revenueVaultPda(campaign);

    await program.methods
      .initializeCrowdfund(
        seriesId,
        goal,
        deadline,
        productionAddress,
        backerBps
      )
      .accounts({
        crowdfund: campaign,
        revenueVault: vault,
        admin: admin.publicKey,
        platformConfig,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([admin])
      .rpc();

    return { campaign, vault };
  }

  async function contributeToCampaign(
    campaign: PublicKey,
    userKeypair: Keypair,
    amount: anchor.BN
  ): Promise<PublicKey> {
    const [contribution] = contributionPda(campaign, userKeypair.publicKey);

    await program.methods
      .contribute(amount)
      .accounts({
        crowdfund: campaign,
        contribution,
        user: userKeypair.publicKey,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([userKeypair])
      .rpc();

    return contribution;
  }

  async function finalizeCampaign(
    campaign: PublicKey,
    productionAddress = productionWallet.publicKey
  ): Promise<void> {
    await program.methods
      .finalizeCampaign()
      .accounts({
        crowdfund: campaign,
        admin: admin.publicKey,
        platformConfig,
        productionWallet: productionAddress,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([admin])
      .rpc();
  }

  async function createCrowdfundTitle(
    seriesId: string,
    campaign: PublicKey
  ): Promise<PublicKey> {
    const [config] = titleConfigPda(seriesId);

    await program.methods
      .createTitleConfig(
        seriesId,
        admin.publicKey,
        campaign,
        creatorBps,
        platformBps,
        true
      )
      .accounts({
        platformConfig,
        titleConfig: config,
        admin: admin.publicKey,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([admin])
      .rpc();

    return config;
  }

  async function mintShare(
    campaign: PublicKey,
    contribution: PublicKey,
    holder: Keypair
  ): Promise<{ asset: Keypair; position: PublicKey }> {
    const asset = Keypair.generate();
    const [position] = positionPda(asset.publicKey);

    await program.methods
      .mintRevenueShare()
      .accounts({
        crowdfund: campaign,
        contribution,
        position,
        user: holder.publicKey,
        asset: asset.publicKey,
        collection: revenueShareCollection.publicKey,
        platformConfig,
        coreProgram: mplCoreProgram,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([holder, asset])
      .rpc();

    return { asset, position };
  }

  async function transferRevenueShare(
    asset: PublicKey,
    from: Keypair,
    to: PublicKey
  ): Promise<void> {
    const transferCoreV1 = new TransactionInstruction({
      programId: mplCoreProgram,
      keys: [
        { pubkey: asset, isSigner: false, isWritable: true },
        {
          pubkey: revenueShareCollection.publicKey,
          isSigner: false,
          isWritable: false,
        },
        { pubkey: from.publicKey, isSigner: true, isWritable: true },
        { pubkey: from.publicKey, isSigner: true, isWritable: false },
        { pubkey: to, isSigner: false, isWritable: false },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
        { pubkey: mplCoreProgram, isSigner: false, isWritable: false },
      ],
      data: Buffer.from([14, 0]),
    });

    await provider.sendAndConfirm(new Transaction().add(transferCoreV1), [
      from,
    ]);
  }

  function sleep(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }

  before(async () => {
    await Promise.all(
      [
        admin.publicKey,
        user.publicKey,
        delegate.publicKey,
        creator.publicKey,
        platformWallet.publicKey,
        productionWallet.publicKey,
        backer.publicKey,
        secondBacker.publicKey,
        buyer.publicKey,
        outsider.publicKey,
      ].map((wallet) => airdrop(wallet, 20))
    );
  });

  it("initializes platform, title config, and delegated user vault", async () => {
    await program.methods
      .initializePlatform(
        admin.publicKey,
        platformWallet.publicKey,
        priceSol,
        creatorBps,
        platformBps
      )
      .accounts({
        platformConfig,
        payer: admin.publicKey,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([admin])
      .rpc();

    await program.methods
      .createRevenueShareCollection()
      .accounts({
        platformConfig,
        admin: admin.publicKey,
        collection: revenueShareCollection.publicKey,
        coreProgram: mplCoreProgram,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([admin, revenueShareCollection])
      .rpc();

    await program.methods
      .createTitleConfig(
        titleId,
        admin.publicKey,
        creator.publicKey,
        creatorBps,
        platformBps,
        true
      )
      .accounts({
        platformConfig,
        titleConfig,
        admin: admin.publicKey,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([admin])
      .rpc();

    await program.methods
      .initializeUserVault()
      .accounts({
        userVault,
        user: user.publicKey,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([user])
      .rpc();

    await program.methods
      .depositSol(priceSol.muln(6))
      .accounts({
        userVault,
        user: user.publicKey,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([user])
      .rpc();

    await program.methods
      .setVaultDelegate(delegate.publicKey, true)
      .accounts({
        userVault,
        user: user.publicKey,
      } as any)
      .signers([user])
      .rpc();

    const platform = await program.account.platformConfig.fetch(platformConfig);
    expect(platform.admin.toBase58()).to.equal(admin.publicKey.toBase58());
    expect(platform.platformWallet.toBase58()).to.equal(
      platformWallet.publicKey.toBase58()
    );
    expect(platform.priceSol.toNumber()).to.equal(priceSol.toNumber());

    const title = await program.account.titleConfig.fetch(titleConfig);
    expect(title.titleId).to.equal(titleId);
    expect(title.creatorWallet.toBase58()).to.equal(
      creator.publicKey.toBase58()
    );
    expect(title.creatorBps).to.equal(creatorBps);
    expect(title.platformBps).to.equal(platformBps);
    expect(title.active).to.equal(true);

    const vault = await program.account.userVault.fetch(userVault);
    expect(vault.user.toBase58()).to.equal(user.publicKey.toBase58());
    expect(vault.delegate.toBase58()).to.equal(delegate.publicKey.toBase58());
    expect(vault.autoPlayEnabled).to.equal(true);
  });

  it("auto_unlock_episode pays SOL splits and creates an unlock receipt", async () => {
    const episodeId = "ep_auto_1";
    const unlockReceipt = unlockReceiptPda(titleConfig, episodeId);

    const initialVault = await balance(userVault);
    const initialCreator = await balance(creator.publicKey);
    const initialPlatform = await balance(platformWallet.publicKey);

    await program.methods
      .autoUnlockEpisode(episodeId)
      .accounts({
        platformConfig,
        titleConfig,
        userVault,
        unlockReceipt,
        creatorWallet: creator.publicKey,
        crowdfund: null,
        revenueVault: null,
        productionWallet: null,
        platformWallet: platformWallet.publicKey,
        delegate: delegate.publicKey,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([delegate])
      .rpc();

    const expectedCreatorShare = creatorShare(priceSol);
    const expectedPlatformShare = priceSol.toNumber() - expectedCreatorShare;

    expect(initialVault - (await balance(userVault))).to.equal(
      priceSol.toNumber()
    );
    expect((await balance(creator.publicKey)) - initialCreator).to.equal(
      expectedCreatorShare
    );
    expect(
      (await balance(platformWallet.publicKey)) - initialPlatform
    ).to.equal(expectedPlatformShare);

    const vault = await program.account.userVault.fetch(userVault);
    expect(vault.totalSpent.toNumber()).to.equal(priceSol.toNumber());

    const receipt = await program.account.episodeUnlockReceipt.fetch(
      unlockReceipt
    );
    expect(receipt.series.toBase58()).to.equal(titleConfig.toBase58());
    expect(receipt.user.toBase58()).to.equal(user.publicKey.toBase58());
    expect(receipt.episodeId).to.equal(episodeId);
    expect(receipt.paidAmount.toNumber()).to.equal(priceSol.toNumber());
  });

  it("rejects a duplicate auto unlock receipt for the same episode", async () => {
    const episodeId = "ep_auto_1";
    const unlockReceipt = unlockReceiptPda(titleConfig, episodeId);

    try {
      await program.methods
        .autoUnlockEpisode(episodeId)
        .accounts({
          platformConfig,
          titleConfig,
          userVault,
          unlockReceipt,
          creatorWallet: creator.publicKey,
          crowdfund: null,
          revenueVault: null,
          productionWallet: null,
          platformWallet: platformWallet.publicKey,
          delegate: delegate.publicKey,
          systemProgram: SystemProgram.programId,
        } as any)
        .signers([delegate])
        .rpc();

      expect.fail("Expected duplicate receipt creation to fail");
    } catch (error) {
      expect(String(error)).to.include("already in use");
    }
  });

  it("direct_payout pays SOL splits without creating an unlock receipt", async () => {
    const episodeId = "ep_direct_1";
    const unlockReceipt = unlockReceiptPda(titleConfig, episodeId);
    const initialVault = await balance(userVault);
    const initialCreator = await balance(creator.publicKey);
    const initialPlatform = await balance(platformWallet.publicKey);

    await program.methods
      .directPayout(episodeId)
      .accounts({
        platformConfig,
        titleConfig,
        userVault,
        creatorWallet: creator.publicKey,
        crowdfund: null,
        revenueVault: null,
        productionWallet: null,
        platformWallet: platformWallet.publicKey,
        delegate: delegate.publicKey,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([delegate])
      .rpc();

    const expectedCreatorShare = creatorShare(priceSol);
    const expectedPlatformShare = priceSol.toNumber() - expectedCreatorShare;

    expect(initialVault - (await balance(userVault))).to.equal(
      priceSol.toNumber()
    );
    expect((await balance(creator.publicKey)) - initialCreator).to.equal(
      expectedCreatorShare
    );
    expect(
      (await balance(platformWallet.publicKey)) - initialPlatform
    ).to.equal(expectedPlatformShare);

    const receipt = await connection.getAccountInfo(unlockReceipt);
    expect(receipt).to.equal(null);
  });

  it("blocks delegate payouts when autoplay is disabled", async () => {
    await program.methods
      .setVaultDelegate(delegate.publicKey, false)
      .accounts({
        userVault,
        user: user.publicKey,
      } as any)
      .signers([user])
      .rpc();

    await expectAnchorError(
      () =>
        program.methods
          .directPayout("ep_disabled")
          .accounts({
            platformConfig,
            titleConfig,
            userVault,
            creatorWallet: creator.publicKey,
            crowdfund: null,
            revenueVault: null,
            productionWallet: null,
            platformWallet: platformWallet.publicKey,
            delegate: delegate.publicKey,
            systemProgram: SystemProgram.programId,
          } as any)
          .signers([delegate])
          .rpc(),
      "AutoPlayDisabled"
    );

    await program.methods
      .setVaultDelegate(delegate.publicKey, true)
      .accounts({
        userVault,
        user: user.publicKey,
      } as any)
      .signers([user])
      .rpc();
  });

  it("accrues crowdfund revenue when the creator wallet is the crowdfund PDA", async () => {
    const goal = new anchor.BN(2 * LAMPORTS_PER_SOL);
    const contributionAmount = goal;
    const deadline = new anchor.BN(Math.floor(Date.now() / 1000) + 3_600);

    await program.methods
      .initializeCrowdfund(
        crowdfundTitleId,
        goal,
        deadline,
        productionWallet.publicKey,
        backerRevenueBps
      )
      .accounts({
        crowdfund,
        revenueVault,
        admin: admin.publicKey,
        platformConfig,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([admin])
      .rpc();

    await program.methods
      .contribute(contributionAmount)
      .accounts({
        crowdfund,
        contribution: backerContribution,
        user: backer.publicKey,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([backer])
      .rpc();

    await program.methods
      .finalizeCampaign()
      .accounts({
        crowdfund,
        admin: admin.publicKey,
        platformConfig,
        productionWallet: productionWallet.publicKey,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([admin])
      .rpc();

    await program.methods
      .createTitleConfig(
        crowdfundTitleId,
        admin.publicKey,
        crowdfund,
        creatorBps,
        platformBps,
        true
      )
      .accounts({
        platformConfig,
        titleConfig: crowdfundTitleConfig,
        admin: admin.publicKey,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([admin])
      .rpc();

    const beforePayout = await program.account.crowdfund.fetch(crowdfund);
    const beforeVault = await balance(revenueVault);
    const beforeProduction = await balance(productionWallet.publicKey);

    await program.methods
      .directPayout("ep_crowdfund_revenue")
      .accounts({
        platformConfig,
        titleConfig: crowdfundTitleConfig,
        userVault,
        creatorWallet: crowdfund,
        crowdfund,
        revenueVault,
        productionWallet: productionWallet.publicKey,
        platformWallet: platformWallet.publicKey,
        delegate: delegate.publicKey,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([delegate])
      .rpc();

    const afterPayout = await program.account.crowdfund.fetch(crowdfund);
    const creatorRevenue = creatorShare(priceSol);
    const backerPortion = Math.floor(
      (creatorRevenue * backerRevenueBps) / 10_000
    );
    const productionPortion = creatorRevenue - backerPortion;
    const expectedAccumulatorDelta = new anchor.BN(backerPortion)
      .mul(scale)
      .divn(backerRevenueBps);

    expect(afterPayout.status).to.deep.equal({ succeeded: {} });
    expect((await balance(revenueVault)) - beforeVault).to.equal(backerPortion);
    expect(
      (await balance(productionWallet.publicKey)) - beforeProduction
    ).to.equal(productionPortion);
    expect(afterPayout.cumulativeRevenuePerBps.toString()).to.equal(
      beforePayout.cumulativeRevenuePerBps
        .add(expectedAccumulatorDelta)
        .toString()
    );
  });

  it("lets the current Core asset holder claim inherited revenue after a secondary sale", async () => {
    const { asset, position } = await mintShare(
      crowdfund,
      backerContribution,
      backer
    );
    const beforeBuyer = await balance(buyer.publicKey);
    const beforeVault = await balance(revenueVault);
    const beforeEscrow = await balance(crowdfund);

    await transferRevenueShare(asset.publicKey, backer, buyer.publicKey);

    await expectAnchorError(
      () =>
        program.methods
          .claimRevenue()
          .accounts({
            crowdfund,
            revenueVault,
            position,
            asset: asset.publicKey,
            user: backer.publicKey,
            systemProgram: SystemProgram.programId,
          } as any)
          .signers([backer])
          .rpc(),
      "NotOwner"
    );

    await program.methods
      .claimRevenue()
      .accounts({
        crowdfund,
        revenueVault,
        position,
        asset: asset.publicKey,
        user: buyer.publicKey,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([buyer])
      .rpc();

    const expectedClaim = Math.floor(
      (creatorShare(priceSol) * backerRevenueBps) / 10_000
    );
    expect((await balance(buyer.publicKey)) - beforeBuyer).to.equal(
      expectedClaim
    );
    expect(beforeVault - (await balance(revenueVault))).to.equal(expectedClaim);
    expect(await balance(crowdfund)).to.equal(beforeEscrow);

    await expectAnchorError(
      () =>
        program.methods
          .claimRevenue()
          .accounts({
            crowdfund,
            revenueVault,
            position,
            asset: asset.publicKey,
            user: buyer.publicKey,
            systemProgram: SystemProgram.programId,
          } as any)
          .signers([buyer])
          .rpc(),
      "NoRevenueToClaim"
    );

    const beforeDeltaBuyer = await balance(buyer.publicKey);
    const beforeDeltaVault = await balance(revenueVault);

    await program.methods
      .directPayout("ep_crowdfund_revenue_delta")
      .accounts({
        platformConfig,
        titleConfig: crowdfundTitleConfig,
        userVault,
        creatorWallet: crowdfund,
        crowdfund,
        revenueVault,
        productionWallet: productionWallet.publicKey,
        platformWallet: platformWallet.publicKey,
        delegate: delegate.publicKey,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([delegate])
      .rpc();

    expect((await balance(revenueVault)) - beforeDeltaVault).to.equal(
      expectedClaim
    );

    await program.methods
      .claimRevenue()
      .accounts({
        crowdfund,
        revenueVault,
        position,
        asset: asset.publicKey,
        user: buyer.publicKey,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([buyer])
      .rpc();

    expect((await balance(buyer.publicKey)) - beforeDeltaBuyer).to.equal(
      expectedClaim
    );
    expect(await balance(crowdfund)).to.equal(beforeEscrow);
  });

  it("keeps overfunded minted shares within the campaign backer revenue cap", async () => {
    const seriesId = "overfunded_shares";
    const goal = new anchor.BN(LAMPORTS_PER_SOL);
    const contributionAmount = new anchor.BN(LAMPORTS_PER_SOL);
    const deadline = new anchor.BN(Math.floor(Date.now() / 1000) + 3_600);
    const { campaign } = await initializeCampaign(seriesId, goal, deadline);
    const firstContribution = await contributeToCampaign(
      campaign,
      backer,
      contributionAmount
    );
    const secondContribution = await contributeToCampaign(
      campaign,
      secondBacker,
      contributionAmount
    );

    await finalizeCampaign(campaign);

    const firstShare = await mintShare(campaign, firstContribution, backer);
    const secondShare = await mintShare(
      campaign,
      secondContribution,
      secondBacker
    );
    const firstPosition = await program.account.sharePosition.fetch(
      firstShare.position
    );
    const secondPosition = await program.account.sharePosition.fetch(
      secondShare.position
    );

    expect(firstPosition.shareBps + secondPosition.shareBps).to.equal(
      backerRevenueBps
    );
    expect(firstPosition.shareBps).to.equal(backerRevenueBps / 2);
    expect(secondPosition.shareBps).to.equal(backerRevenueBps / 2);
  });

  it("rejects cross-series claim attempts through the position binding", async () => {
    const seriesA = "cross_series_a";
    const seriesB = "cross_series_b";
    const goal = new anchor.BN(LAMPORTS_PER_SOL);
    const deadline = new anchor.BN(Math.floor(Date.now() / 1000) + 3_600);
    const campaignA = await initializeCampaign(seriesA, goal, deadline);
    const campaignB = await initializeCampaign(seriesB, goal, deadline);
    const contributionA = await contributeToCampaign(
      campaignA.campaign,
      backer,
      goal
    );
    await contributeToCampaign(campaignB.campaign, secondBacker, goal);
    await finalizeCampaign(campaignA.campaign);
    await finalizeCampaign(campaignB.campaign);

    const { asset, position } = await mintShare(
      campaignA.campaign,
      contributionA,
      backer
    );

    await expectAnchorError(
      () =>
        program.methods
          .claimRevenue()
          .accounts({
            crowdfund: campaignB.campaign,
            revenueVault: campaignB.vault,
            position,
            asset: asset.publicKey,
            user: backer.publicKey,
            systemProgram: SystemProgram.programId,
          } as any)
          .signers([backer])
          .rpc(),
      "WrongCollection"
    );
  });

  it("enforces 30/70 pro-rata claims and pays late minters from the finalize checkpoint", async () => {
    const seriesId = "pro_rata_claims";
    const goal = new anchor.BN(LAMPORTS_PER_SOL);
    const firstAmount = new anchor.BN(300_000_000);
    const secondAmount = new anchor.BN(700_000_000);
    const revenueAmount = new anchor.BN(1_000_000);
    const deadline = new anchor.BN(Math.floor(Date.now() / 1000) + 3_600);
    const { campaign, vault } = await initializeCampaign(
      seriesId,
      goal,
      deadline
    );
    const firstContribution = await contributeToCampaign(
      campaign,
      backer,
      firstAmount
    );
    const secondContribution = await contributeToCampaign(
      campaign,
      secondBacker,
      secondAmount
    );
    await finalizeCampaign(campaign);
    const config = await createCrowdfundTitle(seriesId, campaign);

    await program.methods
      .recordRevenue(revenueAmount)
      .accounts({
        platformConfig,
        titleConfig: config,
        userVault,
        crowdfund: campaign,
        revenueVault: vault,
        productionWallet: productionWallet.publicKey,
        delegate: delegate.publicKey,
      } as any)
      .signers([delegate])
      .rpc();

    const firstShare = await mintShare(campaign, firstContribution, backer);
    const secondShare = await mintShare(
      campaign,
      secondContribution,
      secondBacker
    );
    const firstPosition = await program.account.sharePosition.fetch(
      firstShare.position
    );
    const secondPosition = await program.account.sharePosition.fetch(
      secondShare.position
    );
    const expectedBackerPool = Math.floor(
      (revenueAmount.toNumber() * backerRevenueBps) / 10_000
    );
    const expectedFirst = Math.floor(expectedBackerPool * 0.3);
    const expectedSecond = expectedBackerPool - expectedFirst;
    const beforeFirst = await balance(backer.publicKey);
    const beforeSecond = await balance(secondBacker.publicKey);
    const beforeVault = await balance(vault);

    expect(firstPosition.shareBps).to.equal(900);
    expect(secondPosition.shareBps).to.equal(2100);

    await program.methods
      .claimRevenue()
      .accounts({
        crowdfund: campaign,
        revenueVault: vault,
        position: firstShare.position,
        asset: firstShare.asset.publicKey,
        user: backer.publicKey,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([backer])
      .rpc();

    await program.methods
      .claimRevenue()
      .accounts({
        crowdfund: campaign,
        revenueVault: vault,
        position: secondShare.position,
        asset: secondShare.asset.publicKey,
        user: secondBacker.publicKey,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([secondBacker])
      .rpc();

    expect((await balance(backer.publicKey)) - beforeFirst).to.equal(
      expectedFirst
    );
    expect((await balance(secondBacker.publicKey)) - beforeSecond).to.equal(
      expectedSecond
    );
    expect(beforeVault - (await balance(vault))).to.equal(expectedBackerPool);
  });

  it("keeps accumulator drift bounded across many small revenue events", async () => {
    const seriesId = "small_revenue_precision";
    const goal = new anchor.BN(LAMPORTS_PER_SOL);
    const deadline = new anchor.BN(Math.floor(Date.now() / 1000) + 3_600);
    const { campaign, vault } = await initializeCampaign(
      seriesId,
      goal,
      deadline
    );
    const contribution = await contributeToCampaign(campaign, backer, goal);
    await finalizeCampaign(campaign);
    const config = await createCrowdfundTitle(seriesId, campaign);
    const { asset, position } = await mintShare(campaign, contribution, backer);
    const events = 25;
    const amount = new anchor.BN(7);
    const expectedBackerPortionPerEvent = Math.floor(
      (amount.toNumber() * backerRevenueBps) / 10_000
    );
    const beforeVault = await balance(vault);
    const beforeBacker = await balance(backer.publicKey);

    for (let i = 0; i < events; i += 1) {
      await program.methods
        .recordRevenue(amount)
        .accounts({
          platformConfig,
          titleConfig: config,
          userVault,
          crowdfund: campaign,
          revenueVault: vault,
          productionWallet: productionWallet.publicKey,
          delegate: delegate.publicKey,
        } as any)
        .signers([delegate])
        .rpc();
    }

    const totalBackerPool = expectedBackerPortionPerEvent * events;
    expect((await balance(vault)) - beforeVault).to.equal(totalBackerPool);

    await program.methods
      .claimRevenue()
      .accounts({
        crowdfund: campaign,
        revenueVault: vault,
        position,
        asset: asset.publicKey,
        user: backer.publicKey,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([backer])
      .rpc();

    const claimed = (await balance(backer.publicKey)) - beforeBacker;
    const remaining = (await balance(vault)) - beforeVault;
    expect(totalBackerPool - claimed).to.be.at.most(1);
    expect(remaining).to.be.at.least(0);
    expect(remaining).to.be.at.most(1);
  });

  it("rejects spoofed non-Core assets before revenue can be claimed", async () => {
    await expectAnchorError(
      () =>
        program.methods
          .claimRevenue()
          .accounts({
            crowdfund,
            revenueVault,
            position: backerContribution,
            asset: outsider.publicKey,
            user: outsider.publicKey,
            systemProgram: SystemProgram.programId,
          } as any)
          .signers([outsider])
          .rpc(),
      "AccountDiscriminatorMismatch"
    );
  });

  it("refunds a failed campaign exactly once without touching the revenue vault", async () => {
    const seriesId = "failed_refund_path";
    const goal = new anchor.BN(2 * LAMPORTS_PER_SOL);
    const contributionAmount = new anchor.BN(LAMPORTS_PER_SOL / 2);
    const deadline = new anchor.BN(Math.floor(Date.now() / 1000) + 1);
    const { campaign, vault } = await initializeCampaign(
      seriesId,
      goal,
      deadline
    );
    const contribution = await contributeToCampaign(
      campaign,
      backer,
      contributionAmount
    );
    const beforeVault = await balance(vault);
    const beforeCampaign = await balance(campaign);

    await sleep(2_000);

    await program.methods
      .failCampaign()
      .accounts({
        crowdfund: campaign,
        admin: admin.publicKey,
        platformConfig,
      } as any)
      .signers([admin])
      .rpc();

    const beforeBacker = await balance(backer.publicKey);

    await program.methods
      .refund()
      .accounts({
        crowdfund: campaign,
        contribution,
        user: backer.publicKey,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([backer])
      .rpc();

    expect(beforeCampaign - (await balance(campaign))).to.equal(
      contributionAmount.toNumber()
    );
    expect((await balance(backer.publicKey)) - beforeBacker).to.be.at.least(
      contributionAmount.toNumber()
    );
    expect(await connection.getAccountInfo(contribution)).to.equal(null);
    expect(await balance(vault)).to.equal(beforeVault);

    const rentMinimum = await connection.getMinimumBalanceForRentExemption(
      program.account.crowdfund.size
    );
    expect(await balance(campaign)).to.be.at.least(rentMinimum);

    await expectAnchorError(
      () =>
        program.methods
          .refund()
          .accounts({
            crowdfund: campaign,
            contribution,
            user: backer.publicKey,
            systemProgram: SystemProgram.programId,
          } as any)
          .signers([backer])
          .rpc(),
      "AccountNotInitialized"
    );
  });

  it("cancels an active campaign, rejects later contributions, and enables refunds", async () => {
    const seriesId = "cancel_refund_path";
    const goal = new anchor.BN(2 * LAMPORTS_PER_SOL);
    const contributionAmount = new anchor.BN(LAMPORTS_PER_SOL / 2);
    const deadline = new anchor.BN(Math.floor(Date.now() / 1000) + 3_600);
    const { campaign, vault } = await initializeCampaign(
      seriesId,
      goal,
      deadline
    );
    const contribution = await contributeToCampaign(
      campaign,
      secondBacker,
      contributionAmount
    );
    const beforeVault = await balance(vault);
    const beforeCampaign = await balance(campaign);

    await program.methods
      .cancelCampaign()
      .accounts({
        crowdfund: campaign,
        admin: admin.publicKey,
        platformConfig,
      } as any)
      .signers([admin])
      .rpc();

    await expectAnchorError(
      () =>
        program.methods
          .contribute(new anchor.BN(1))
          .accounts({
            crowdfund: campaign,
            contribution,
            user: secondBacker.publicKey,
            systemProgram: SystemProgram.programId,
          } as any)
          .signers([secondBacker])
          .rpc(),
      "CampaignInactive"
    );

    const beforeBacker = await balance(secondBacker.publicKey);

    await program.methods
      .refund()
      .accounts({
        crowdfund: campaign,
        contribution,
        user: secondBacker.publicKey,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([secondBacker])
      .rpc();

    expect(beforeCampaign - (await balance(campaign))).to.equal(
      contributionAmount.toNumber()
    );
    expect(
      (await balance(secondBacker.publicKey)) - beforeBacker
    ).to.be.at.least(contributionAmount.toNumber());
    expect(await balance(vault)).to.equal(beforeVault);
  });

  it("requires the configured delegate to record revenue", async () => {
    await expectAnchorError(
      () =>
        program.methods
          .recordRevenue(new anchor.BN(1))
          .accounts({
            platformConfig,
            titleConfig: crowdfundTitleConfig,
            userVault,
            crowdfund,
            revenueVault,
            productionWallet: productionWallet.publicKey,
            delegate: outsider.publicKey,
          } as any)
          .signers([outsider])
          .rpc(),
      "OnlyDelegate"
    );
  });

  it("record_revenue pays the production split directly and isolates vault balances", async () => {
    const seriesId = "record_revenue_split";
    const goal = new anchor.BN(LAMPORTS_PER_SOL);
    const contributionAmount = goal;
    const deadline = new anchor.BN(Math.floor(Date.now() / 1000) + 3_600);
    const { campaign, vault } = await initializeCampaign(
      seriesId,
      goal,
      deadline
    );
    await contributeToCampaign(campaign, backer, contributionAmount);
    await finalizeCampaign(campaign);
    const config = await createCrowdfundTitle(seriesId, campaign);
    const amount = new anchor.BN(1_000_000);
    const beforeUserVault = await balance(userVault);
    const beforeRevenueVault = await balance(vault);
    const beforeProduction = await balance(productionWallet.publicKey);
    const beforeCampaign = await balance(campaign);

    await program.methods
      .recordRevenue(amount)
      .accounts({
        platformConfig,
        titleConfig: config,
        userVault,
        crowdfund: campaign,
        revenueVault: vault,
        productionWallet: productionWallet.publicKey,
        delegate: delegate.publicKey,
      } as any)
      .signers([delegate])
      .rpc();

    const expectedBackerPortion = Math.floor(
      (amount.toNumber() * backerRevenueBps) / 10_000
    );
    const expectedProductionPortion = amount.toNumber() - expectedBackerPortion;

    expect(beforeUserVault - (await balance(userVault))).to.equal(
      amount.toNumber()
    );
    expect((await balance(vault)) - beforeRevenueVault).to.equal(
      expectedBackerPortion
    );
    expect(
      (await balance(productionWallet.publicKey)) - beforeProduction
    ).to.equal(expectedProductionPortion);
    expect(await balance(campaign)).to.equal(beforeCampaign);
  });

  it("rejects repeated finalization, fail-after-finalize, and late contributions", async () => {
    const finalizedSeriesId = "finalize_status_gates";
    const goal = new anchor.BN(LAMPORTS_PER_SOL);
    const deadline = new anchor.BN(Math.floor(Date.now() / 1000) + 3_600);
    const { campaign } = await initializeCampaign(
      finalizedSeriesId,
      goal,
      deadline
    );
    await contributeToCampaign(campaign, backer, goal);
    await finalizeCampaign(campaign);

    await expectAnchorError(
      () =>
        program.methods
          .finalizeCampaign()
          .accounts({
            crowdfund: campaign,
            admin: admin.publicKey,
            platformConfig,
            productionWallet: productionWallet.publicKey,
            systemProgram: SystemProgram.programId,
          } as any)
          .signers([admin])
          .rpc(),
      "CampaignInactive"
    );

    await expectAnchorError(
      () =>
        program.methods
          .failCampaign()
          .accounts({
            crowdfund: campaign,
            admin: admin.publicKey,
            platformConfig,
          } as any)
          .signers([admin])
          .rpc(),
      "CampaignInactive"
    );

    const lateSeriesId = "late_contribution_gate";
    const lateDeadline = new anchor.BN(Math.floor(Date.now() / 1000) + 1);
    const lateCampaign = await initializeCampaign(
      lateSeriesId,
      goal,
      lateDeadline
    );
    const [lateContribution] = contributionPda(
      lateCampaign.campaign,
      outsider.publicKey
    );

    await sleep(2_000);

    await expectAnchorError(
      () =>
        program.methods
          .contribute(new anchor.BN(1))
          .accounts({
            crowdfund: lateCampaign.campaign,
            contribution: lateContribution,
            user: outsider.publicKey,
            systemProgram: SystemProgram.programId,
          } as any)
          .signers([outsider])
          .rpc(),
      "CampaignEnded"
    );
  });

  it("rejects invalid title split basis points", async () => {
    const invalidTitleId = "invalid_bps_title";
    const [invalidTitleConfig] = titleConfigPda(invalidTitleId);

    await expectAnchorError(
      () =>
        program.methods
          .createTitleConfig(
            invalidTitleId,
            admin.publicKey,
            creator.publicKey,
            5_000,
            4_000,
            true
          )
          .accounts({
            platformConfig,
            titleConfig: invalidTitleConfig,
            admin: admin.publicKey,
            systemProgram: SystemProgram.programId,
          } as any)
          .signers([admin])
          .rpc(),
      "InvalidBasisPoints"
    );
  });
});
