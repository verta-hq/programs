import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Royalties } from "../target/types/royalties";
import { 
  createMint, 
  getOrCreateAssociatedTokenAccount, 
  mintTo, 
  TOKEN_PROGRAM_ID,
  getAccount
} from "@solana/spl-token";
import { PublicKey, Keypair, SystemProgram, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { expect } from "chai";

describe("royalties", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.royalties as Program<Royalties>;

  // Test wallets
  const admin = Keypair.generate();
  const user = Keypair.generate();
  const creator = Keypair.generate();
  const platform = Keypair.generate();

  // PDA variables
  let platformAuthorityPda: PublicKey;
  let titleConfigPda: PublicKey;
  
  // Constants
  const titleId = "bangkok_nights_s1";
  const episodeId = "ep1";
  const priceAmount = new anchor.BN(250000); // 0.25 USDC
  const creatorBps = 8000;
  const platformBps = 2000;

  // Mock USDC variables
  let mint: PublicKey;
  let userTokenAccount: PublicKey;
  let creatorTokenAccount: PublicKey;
  let platformTokenAccount: PublicKey;

  before(async () => {
    // Airdrop SOL to all wallets
    const wallets = [admin, user, creator, platform];
    for (const wallet of wallets) {
      const sig = await provider.connection.requestAirdrop(wallet.publicKey, 10 * LAMPORTS_PER_SOL);
      await provider.connection.confirmTransaction(sig);
    }

    // Create mock USDC mint
    mint = await createMint(
      provider.connection,
      admin,
      admin.publicKey,
      null,
      6
    );

    // Create ATAs
    userTokenAccount = (await getOrCreateAssociatedTokenAccount(
      provider.connection,
      admin,
      mint,
      user.publicKey
    )).address;

    creatorTokenAccount = (await getOrCreateAssociatedTokenAccount(
      provider.connection,
      admin,
      mint,
      creator.publicKey
    )).address;

    platformTokenAccount = (await getOrCreateAssociatedTokenAccount(
      provider.connection,
      admin,
      mint,
      platform.publicKey
    )).address;

    // Mint USDC to user (1000 USDC)
    await mintTo(
      provider.connection,
      admin,
      mint,
      userTokenAccount,
      admin,
      1000_000_000
    );

    // Derive PDAs
    [platformAuthorityPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("platform_authority")],
      program.programId
    );

    [titleConfigPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("title_config"), Buffer.from(titleId)],
      program.programId
    );
  });

  it("Initializes the platform", async () => {
    await program.methods
      .initializePlatform(admin.publicKey)
      .accounts({
        platformAuthority: platformAuthorityPda,
        payer: admin.publicKey,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([admin])
      .rpc();

    const state = await program.account.platformAuthority.fetch(platformAuthorityPda);
    expect(state.admin.toString()).to.equal(admin.publicKey.toString());
    expect(state.paused).to.be.false;
  });

  it("Creates a title config", async () => {
    await program.methods
      .createTitleConfig(
        titleId,
        admin.publicKey, // authority
        creator.publicKey, // creator wallet
        platform.publicKey, // platform wallet
        priceAmount,
        creatorBps,
        platformBps,
        true // active
      )
      .accounts({
        platformAuthority: platformAuthorityPda,
        titleConfig: titleConfigPda,
        paymentMint: mint,
        admin: admin.publicKey,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([admin])
      .rpc();

    const config = await program.account.titleConfig.fetch(titleConfigPda);
    expect(config.titleId).to.equal(titleId);
    expect(config.creatorBps).to.equal(creatorBps);
    expect(config.platformBps).to.equal(platformBps);
    expect(config.priceAmount.toNumber()).to.equal(priceAmount.toNumber());
    expect(config.active).to.be.true;
  });

  it("Unlocks an episode and splits payment correctly", async () => {
    const [unlockReceiptPda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("unlock_receipt"),
        user.publicKey.toBuffer(),
        titleConfigPda.toBuffer(),
        Buffer.from(episodeId),
      ],
      program.programId
    );

    // Initial balances
    const initialUserBal = (await getAccount(provider.connection, userTokenAccount)).amount;
    const initialCreatorBal = (await getAccount(provider.connection, creatorTokenAccount)).amount;
    const initialPlatformBal = (await getAccount(provider.connection, platformTokenAccount)).amount;

    await program.methods
      .unlockEpisode(episodeId)
      .accounts({
        platformAuthority: platformAuthorityPda,
        titleConfig: titleConfigPda,
        unlockReceipt: unlockReceiptPda,
        paymentMint: mint,
        userTokenAccount: userTokenAccount,
        creatorTokenAccount: creatorTokenAccount,
        platformTokenAccount: platformTokenAccount,
        user: user.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([user])
      .rpc();

    // Final balances
    const finalUserBal = (await getAccount(provider.connection, userTokenAccount)).amount;
    const finalCreatorBal = (await getAccount(provider.connection, creatorTokenAccount)).amount;
    const finalPlatformBal = (await getAccount(provider.connection, platformTokenAccount)).amount;

    const expectedCreatorShare = priceAmount.toNumber() * creatorBps / 10000;
    const expectedPlatformShare = priceAmount.toNumber() * platformBps / 10000;

    expect(Number(initialUserBal) - Number(finalUserBal)).to.equal(priceAmount.toNumber());
    expect(Number(finalCreatorBal) - Number(initialCreatorBal)).to.equal(expectedCreatorShare);
    expect(Number(finalPlatformBal) - Number(initialPlatformBal)).to.equal(expectedPlatformShare);

    // Verify receipt
    const receipt = await program.account.episodeUnlockReceipt.fetch(unlockReceiptPda);
    expect(receipt.user.toString()).to.equal(user.publicKey.toString());
    expect(receipt.episodeId).to.equal(episodeId);
    expect(receipt.paidAmount.toNumber()).to.equal(priceAmount.toNumber());
  });

  it("Fails to unlock the same episode twice", async () => {
    try {
      const [unlockReceiptPda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("unlock_receipt"),
          user.publicKey.toBuffer(),
          titleConfigPda.toBuffer(),
          Buffer.from(episodeId),
        ],
        program.programId
      );

      await program.methods
        .unlockEpisode(episodeId)
        .accounts({
          platformAuthority: platformAuthorityPda,
          titleConfig: titleConfigPda,
          unlockReceipt: unlockReceiptPda,
          paymentMint: mint,
          userTokenAccount: userTokenAccount,
          creatorTokenAccount: creatorTokenAccount,
          platformTokenAccount: platformTokenAccount,
          user: user.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        } as any)
        .signers([user])
        .rpc();
      
      expect.fail("Should have failed with already in use error");
    } catch (err: any) {
      // Anchor/Solana error for PDA already exists
      expect(err.logs.toString()).to.include("already in use");
    }
  });

  it("Fails to unlock when platform is paused", async () => {
    // Pause platform
    await program.methods
      .setPlatformPaused(true)
      .accounts({
        platformAuthority: platformAuthorityPda,
        admin: admin.publicKey,
      } as any)
      .signers([admin])
      .rpc();

    const episode2 = "ep2";
    const [unlockReceiptPda2] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("unlock_receipt"),
        user.publicKey.toBuffer(),
        titleConfigPda.toBuffer(),
        Buffer.from(episode2),
      ],
      program.programId
    );

    try {
      await program.methods
        .unlockEpisode(episode2)
        .accounts({
          platformAuthority: platformAuthorityPda,
          titleConfig: titleConfigPda,
          unlockReceipt: unlockReceiptPda2,
          paymentMint: mint,
          userTokenAccount: userTokenAccount,
          creatorTokenAccount: creatorTokenAccount,
          platformTokenAccount: platformTokenAccount,
          user: user.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        } as any)
        .signers([user])
        .rpc();
      
      expect.fail("Should have failed with PlatformPaused error");
    } catch (err: any) {
      expect(err.error.errorCode.code).to.equal("PlatformPaused");
    }

    // Unpause for subsequent potential tests
    await program.methods
      .setPlatformPaused(false)
      .accounts({
        platformAuthority: platformAuthorityPda,
        admin: admin.publicKey,
      } as any)
      .signers([admin])
      .rpc();
  });

  it("Fails to create title config with invalid BPS", async () => {
    const invalidTitleId = "invalid_bps_title";
    const [invalidPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("title_config"), Buffer.from(invalidTitleId)],
      program.programId
    );

    try {
      await program.methods
        .createTitleConfig(
          invalidTitleId,
          admin.publicKey,
          creator.publicKey,
          platform.publicKey,
          priceAmount,
          5000,
          4000, // Sums to 9000, not 10000
          true
        )
        .accounts({
          platformAuthority: platformAuthorityPda,
          titleConfig: invalidPda,
          paymentMint: mint,
          admin: admin.publicKey,
          systemProgram: SystemProgram.programId,
        } as any)
        .signers([admin])
        .rpc();
      
      expect.fail("Should have failed with InvalidBasisPoints");
    } catch (err: any) {
      expect(err.error.errorCode.code).to.equal("InvalidBasisPoints");
    }
  });
});
