import { expect } from "chai";
import { ASSOCIATED_TOKEN_PROGRAM_ADDRESS } from "@solana-program/token";
import type { Address, KeyPairSigner } from "@solana/kit";
import {
  fetchCrowdfund,
  fetchSharePosition,
  findContributionPda,
  getClaimRevenueInstruction,
  getPauseInstruction,
  getUpdatePlatformConfigInstruction,
  VERTA_CORE_ERROR__NO_REVENUE_TO_CLAIM,
  VERTA_CORE_ERROR__NOT_OWNER,
  VERTA_CORE_ERROR__PLATFORM_PAUSED,
  VERTA_CORE_ERROR__UNAUTHORIZED,
  VERTA_CORE_ERROR__WRONG_COLLECTION,
} from "../clients/js/verta_core/src/generated";
import {
  ANCHOR_ERROR,
  ata,
  BPS,
  contribute,
  createTitle,
  ctx,
  depositRevenue,
  expectCustomError,
  finalizeCampaign,
  initializeCampaign,
  mintShare,
  nowSeconds,
  SCALE,
  send,
  tokenBalance,
  tokenProgramFor,
  transferCoreAsset,
  USDC,
  type Campaign,
} from "./00_setup";

const backerRevenueBps = 3_000n;
const backerPortion = (amount: bigint): bigint =>
  (amount * backerRevenueBps) / BPS;

const claim = async (
  campaign: Campaign,
  asset: Address,
  position: Address,
  user: KeyPairSigner,
): Promise<void> => {
  await send(
    [
      getClaimRevenueInstruction({
        crowdfund: campaign.address,
        mint: campaign.mint,
        revenueVault: campaign.revenueVault,
        position,
        asset,
        user,
        payer: ctx.sponsor,
        userAta: await ata(campaign.mint, user.address),
        platformConfig: ctx.platformConfig,
        tokenProgram: tokenProgramFor(campaign.mint),
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ADDRESS,
      }),
    ],
    ctx.sponsor,
  );
};

const fundedCampaign = async (
  seriesId: string,
  goal: bigint,
  contributions: Array<[KeyPairSigner, bigint]>,
): Promise<Campaign> => {
  const campaign = await initializeCampaign(
    seriesId,
    goal,
    nowSeconds() + 3_600n,
  );
  for (const [backer, amount] of contributions) {
    await contribute(campaign, backer, amount);
  }
  await finalizeCampaign(campaign);
  await createTitle(seriesId, campaign.address);
  return campaign;
};

describe("revenue accounting", () => {
  let graduated: Campaign;
  const episodeCreatorShare = 40n * USDC;

  it("accepts revenue deposits only from the operator", async () => {
    graduated = await fundedCampaign("graduated_s1", 100n * USDC, [
      [ctx.backer, 100n * USDC],
    ]);
    await expectCustomError(
      () => depositRevenue(graduated, USDC, ctx.outsider),
      VERTA_CORE_ERROR__UNAUTHORIZED,
    );
    await expectCustomError(
      () => depositRevenue(graduated, USDC, ctx.admin),
      VERTA_CORE_ERROR__UNAUTHORIZED,
    );
  });

  it("accrues backer revenue into the vault and pays the production split", async () => {
    const productionAta = await ata(
      graduated.mint,
      ctx.productionWallet.address,
    );
    const before = (await fetchCrowdfund(ctx.rpc, graduated.address)).data;
    const vaultBefore = await tokenBalance(
      graduated.revenueVault,
      graduated.mint,
    );
    const productionBefore = await tokenBalance(productionAta, graduated.mint);

    await depositRevenue(graduated, episodeCreatorShare);

    const after = (await fetchCrowdfund(ctx.rpc, graduated.address)).data;
    const backers = backerPortion(episodeCreatorShare);
    expect(
      (await tokenBalance(graduated.revenueVault, graduated.mint)) -
        vaultBefore,
    ).to.equal(backers);
    expect(
      (await tokenBalance(productionAta, graduated.mint)) - productionBefore,
    ).to.equal(episodeCreatorShare - backers);
    expect(
      after.cumulativeRevenuePerBps - before.cumulativeRevenuePerBps,
    ).to.equal((backers * SCALE) / backerRevenueBps);
  });

  it("lets the current Core asset holder claim inherited revenue after a secondary sale", async () => {
    const { asset, position } = await mintShare(graduated, ctx.backer);
    const buyerAta = await ata(graduated.mint, ctx.buyer.address);
    const vaultBefore = await tokenBalance(
      graduated.revenueVault,
      graduated.mint,
    );
    const expectedClaim = backerPortion(episodeCreatorShare);

    await transferCoreAsset(asset.address, ctx.backer, ctx.buyer.address);

    await expectCustomError(
      () => claim(graduated, asset.address, position, ctx.backer),
      VERTA_CORE_ERROR__NOT_OWNER,
    );
    await claim(graduated, asset.address, position, ctx.buyer);
    expect(await tokenBalance(buyerAta, graduated.mint)).to.equal(
      expectedClaim,
    );
    expect(
      vaultBefore -
        (await tokenBalance(graduated.revenueVault, graduated.mint)),
    ).to.equal(expectedClaim);
    await expectCustomError(
      () => claim(graduated, asset.address, position, ctx.buyer),
      VERTA_CORE_ERROR__NO_REVENUE_TO_CLAIM,
    );

    await depositRevenue(graduated, episodeCreatorShare);
    await claim(graduated, asset.address, position, ctx.buyer);
    expect(await tokenBalance(buyerAta, graduated.mint)).to.equal(
      expectedClaim * 2n,
    );
  });

  it("keeps overfunded minted shares within the campaign backer revenue cap", async () => {
    const campaign = await fundedCampaign("overfunded_shares", 100n * USDC, [
      [ctx.backer, 100n * USDC],
      [ctx.secondBacker, 100n * USDC],
    ]);
    const first = await mintShare(campaign, ctx.backer);
    const second = await mintShare(campaign, ctx.secondBacker);
    const firstPosition = (await fetchSharePosition(ctx.rpc, first.position))
      .data;
    const secondPosition = (await fetchSharePosition(ctx.rpc, second.position))
      .data;
    expect(firstPosition.shareBps + secondPosition.shareBps).to.equal(
      Number(backerRevenueBps),
    );
    expect(firstPosition.shareBps).to.equal(1_500);
    expect(secondPosition.shareBps).to.equal(1_500);
  });

  it("rejects cross-series claim attempts through the position binding", async () => {
    const campaignA = await fundedCampaign("cross_series_a", 100n * USDC, [
      [ctx.backer, 100n * USDC],
    ]);
    const campaignB = await fundedCampaign("cross_series_b", 100n * USDC, [
      [ctx.secondBacker, 100n * USDC],
    ]);
    const { asset, position } = await mintShare(campaignA, ctx.backer);
    await expectCustomError(
      () => claim(campaignB, asset.address, position, ctx.backer),
      VERTA_CORE_ERROR__WRONG_COLLECTION,
    );
  });

  it("enforces 30/70 pro-rata claims and pays late minters from the finalize checkpoint", async () => {
    const campaign = await fundedCampaign("pro_rata_claims", 100n * USDC, [
      [ctx.backer, 30n * USDC],
      [ctx.secondBacker, 70n * USDC],
    ]);
    const revenue = 1n * USDC;
    await depositRevenue(campaign, revenue);

    const first = await mintShare(campaign, ctx.backer);
    const second = await mintShare(campaign, ctx.secondBacker);
    expect(
      (await fetchSharePosition(ctx.rpc, first.position)).data.shareBps,
    ).to.equal(900);
    expect(
      (await fetchSharePosition(ctx.rpc, second.position)).data.shareBps,
    ).to.equal(2_100);

    const pool = backerPortion(revenue);
    const expectedFirst = (pool * 30n) / 100n;
    const expectedSecond = pool - expectedFirst;
    const firstAta = await ata(campaign.mint, ctx.backer.address);
    const secondAta = await ata(campaign.mint, ctx.secondBacker.address);
    const firstBefore = await tokenBalance(firstAta, campaign.mint);
    const secondBefore = await tokenBalance(secondAta, campaign.mint);
    const vaultBefore = await tokenBalance(
      campaign.revenueVault,
      campaign.mint,
    );

    await claim(campaign, first.asset.address, first.position, ctx.backer);
    await claim(
      campaign,
      second.asset.address,
      second.position,
      ctx.secondBacker,
    );

    expect(
      (await tokenBalance(firstAta, campaign.mint)) - firstBefore,
    ).to.equal(expectedFirst);
    expect(
      (await tokenBalance(secondAta, campaign.mint)) - secondBefore,
    ).to.equal(expectedSecond);
    expect(
      vaultBefore - (await tokenBalance(campaign.revenueVault, campaign.mint)),
    ).to.equal(pool);
  });

  it("keeps accumulator drift bounded across many small revenue events", async () => {
    const campaign = await fundedCampaign(
      "small_revenue_precision",
      100n * USDC,
      [[ctx.backer, 100n * USDC]],
    );
    const { asset, position } = await mintShare(campaign, ctx.backer);
    const events = 25n;
    const amount = 7n;
    const backerAta = await ata(campaign.mint, ctx.backer.address);
    const vaultBefore = await tokenBalance(
      campaign.revenueVault,
      campaign.mint,
    );
    const backerBefore = await tokenBalance(backerAta, campaign.mint);

    for (let i = 0n; i < events; i += 1n) {
      await depositRevenue(campaign, amount);
    }
    const pool = backerPortion(amount) * events;
    expect(
      (await tokenBalance(campaign.revenueVault, campaign.mint)) - vaultBefore,
    ).to.equal(pool);

    await claim(campaign, asset.address, position, ctx.backer);
    const claimed =
      (await tokenBalance(backerAta, campaign.mint)) - backerBefore;
    const remaining =
      (await tokenBalance(campaign.revenueVault, campaign.mint)) - vaultBefore;
    expect(Number(pool - claimed)).to.be.at.most(1);
    expect(Number(remaining)).to.be.at.least(0);
    expect(Number(remaining)).to.be.at.most(1);
  });

  it("rejects spoofed non-Core assets before revenue can be claimed", async () => {
    const [contribution] = await findContributionPda({
      crowdfund: graduated.address,
      user: ctx.backer.address,
    });
    await expectCustomError(
      () => claim(graduated, ctx.outsider.address, contribution, ctx.outsider),
      ANCHOR_ERROR.AccountDiscriminatorMismatch,
    );
  });

  it("blocks minting, deposits and claims while paused", async () => {
    const campaign = await fundedCampaign("paused_revenue", 100n * USDC, [
      [ctx.backer, 60n * USDC],
      [ctx.secondBacker, 40n * USDC],
    ]);
    const { asset, position } = await mintShare(campaign, ctx.backer);
    await depositRevenue(campaign, USDC);
    await send(
      [
        getPauseInstruction({
          platformConfig: ctx.platformConfig,
          authority: ctx.operator,
        }),
      ],
      ctx.operator,
    );
    await expectCustomError(
      () => mintShare(campaign, ctx.secondBacker),
      VERTA_CORE_ERROR__PLATFORM_PAUSED,
    );
    await expectCustomError(
      () => depositRevenue(campaign, USDC),
      VERTA_CORE_ERROR__PLATFORM_PAUSED,
    );
    await expectCustomError(
      () => claim(campaign, asset.address, position, ctx.backer),
      VERTA_CORE_ERROR__PLATFORM_PAUSED,
    );
    await send(
      [
        getUpdatePlatformConfigInstruction({
          platformConfig: ctx.platformConfig,
          admin: ctx.admin,
          newOperator: null,
          paused: false,
        }),
      ],
      ctx.admin,
    );
    await claim(campaign, asset.address, position, ctx.backer);
    await mintShare(campaign, ctx.secondBacker);
  });
});
