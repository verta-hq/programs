import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { expect } from "chai";
import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
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
  const investor = Keypair.generate();

  const priceSol = new anchor.BN(50_000_000);
  const creatorBps = 8_000;
  const platformBps = 2_000;
  const investorShareBps = 3_000;
  const titleId = "bangkok_nights_s1";
  const crowdfundTitleId = "crowdfund_s1";

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
  const [investorContribution] = PublicKey.findProgramAddressSync(
    [
      Buffer.from("contribution"),
      crowdfund.toBuffer(),
      investor.publicKey.toBuffer(),
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

  before(async () => {
    await Promise.all(
      [
        admin.publicKey,
        user.publicKey,
        delegate.publicKey,
        creator.publicKey,
        platformWallet.publicKey,
        productionWallet.publicKey,
        investor.publicKey,
      ].map((wallet) => airdrop(wallet))
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
        investorShareBps
      )
      .accounts({
        crowdfund,
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
        contribution: investorContribution,
        user: investor.publicKey,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([investor])
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

    await program.methods
      .directPayout("ep_crowdfund_revenue")
      .accounts({
        platformConfig,
        titleConfig: crowdfundTitleConfig,
        userVault,
        creatorWallet: crowdfund,
        platformWallet: platformWallet.publicKey,
        delegate: delegate.publicKey,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([delegate])
      .rpc();

    const afterPayout = await program.account.crowdfund.fetch(crowdfund);
    expect(afterPayout.status).to.deep.equal({ succeeded: {} });
    expect(afterPayout.revenueAccumulated.toNumber()).to.equal(
      beforePayout.revenueAccumulated.toNumber() + creatorShare(priceSol)
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
