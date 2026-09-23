import { expect } from "chai";
import { TOKEN_PROGRAM_ADDRESS } from "@solana-program/token";
import {
  CrowdfundStatus,
  fetchContribution,
  fetchCrowdfund,
  fetchMaybeContribution,
  findContributionPda,
  getCancelCampaignInstruction,
  getFailCampaignInstruction,
  getPauseInstruction,
  getUpdatePlatformConfigInstruction,
  VERTA_CORE_ERROR__CAMPAIGN_DEADLINE_PASSED,
  VERTA_CORE_ERROR__CAMPAIGN_ENDED,
  VERTA_CORE_ERROR__CAMPAIGN_INACTIVE,
  VERTA_CORE_ERROR__GOAL_NOT_REACHED,
  VERTA_CORE_ERROR__INVALID_AMOUNT,
  VERTA_CORE_ERROR__INVALID_BASIS_POINTS,
  VERTA_CORE_ERROR__INVALID_DEADLINE,
  VERTA_CORE_ERROR__INVALID_RECIPIENT,
  VERTA_CORE_ERROR__INVALID_SETTLEMENT_MINT,
  VERTA_CORE_ERROR__PLATFORM_PAUSED,
  VERTA_CORE_ERROR__UNAUTHORIZED,
} from "../clients/js/verta_core/src/generated";
import {
  ANCHOR_ERROR,
  ata,
  contribute,
  createMint,
  ctx,
  depositRevenue,
  expectCustomError,
  finalizeCampaign,
  fundTokens,
  initializeCampaign,
  nowSeconds,
  refund,
  send,
  sleep,
  solBalance,
  statusOf,
  tokenBalance,
  USDC,
  type Campaign,
} from "./00_setup";

const pause = () =>
  send(
    [
      getPauseInstruction({
        platformConfig: ctx.platformConfig,
        authority: ctx.operator,
      }),
    ],
    ctx.operator,
  );
const unpause = () =>
  send(
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
const setSettlementMint = (mint: string) =>
  send(
    [
      getUpdatePlatformConfigInstruction({
        platformConfig: ctx.platformConfig,
        admin: ctx.admin,
        newSettlementMint: mint as never,
        newOperator: null,
        paused: null,
      }),
    ],
    ctx.admin,
  );
const cancel = (campaign: Campaign, admin = ctx.admin) =>
  send(
    [
      getCancelCampaignInstruction({
        crowdfund: campaign.address,
        admin,
        platformConfig: ctx.platformConfig,
      }),
    ],
    admin,
  );
const fail = (campaign: Campaign, admin = ctx.admin) =>
  send(
    [
      getFailCampaignInstruction({
        crowdfund: campaign.address,
        admin,
        platformConfig: ctx.platformConfig,
      }),
    ],
    admin,
  );

describe("crowdfund lifecycle", () => {
  const goal = 200n * USDC;
  let open: Campaign;

  after(async () => {
    await setSettlementMint(ctx.usdcMint);
  });

  it("validates campaign parameters and the operator signer", async () => {
    const deadline = nowSeconds() + 3_600n;
    await expectCustomError(
      () => initializeCampaign("zero_goal", 0n, deadline),
      VERTA_CORE_ERROR__INVALID_AMOUNT,
    );
    await expectCustomError(
      () => initializeCampaign("past_deadline", goal, nowSeconds() - 10n),
      VERTA_CORE_ERROR__INVALID_DEADLINE,
    );
    await expectCustomError(
      () =>
        initializeCampaign("zero_bps", goal, deadline, { backerRevenueBps: 0 }),
      VERTA_CORE_ERROR__INVALID_BASIS_POINTS,
    );
    await expectCustomError(
      () =>
        initializeCampaign("over_bps", goal, deadline, {
          backerRevenueBps: 10_001,
        }),
      VERTA_CORE_ERROR__INVALID_BASIS_POINTS,
    );
    await expectCustomError(
      () =>
        initializeCampaign("no_production", goal, deadline, {
          productionWallet: "11111111111111111111111111111111" as never,
        }),
      VERTA_CORE_ERROR__INVALID_RECIPIENT,
    );
    await expectCustomError(
      () =>
        initializeCampaign("outsider_init", goal, deadline, {
          operator: ctx.outsider,
        }),
      VERTA_CORE_ERROR__UNAUTHORIZED,
    );
    await expectCustomError(
      () =>
        initializeCampaign("admin_init", goal, deadline, {
          operator: ctx.admin,
        }),
      VERTA_CORE_ERROR__UNAUTHORIZED,
    );
  });

  it("contributes through a sponsored payer and records the rent payer", async () => {
    open = await initializeCampaign(
      "lifecycle_open",
      goal,
      nowSeconds() + 3_600n,
    );
    expect(await statusOf(open)).to.equal(CrowdfundStatus.Active);

    await contribute(open, ctx.backer, 100n * USDC);
    const [contributionAddress] = await findContributionPda({
      crowdfund: open.address,
      user: ctx.backer.address,
    });
    const contribution = (await fetchContribution(ctx.rpc, contributionAddress))
      .data;
    expect(contribution.user).to.equal(ctx.backer.address);
    expect(contribution.rentPayer).to.equal(ctx.sponsor.address);
    expect(contribution.amountContributed).to.equal(100n * USDC);
    expect(await tokenBalance(open.escrow, open.mint)).to.equal(100n * USDC);
    expect(
      (await fetchCrowdfund(ctx.rpc, open.address)).data.totalRaised,
    ).to.equal(100n * USDC);

    await expectCustomError(
      () => contribute(open, ctx.backer, 0n),
      VERTA_CORE_ERROR__INVALID_AMOUNT,
    );
    const otherMint = await createMint(TOKEN_PROGRAM_ADDRESS);
    const otherAta = await fundTokens(otherMint, ctx.backer.address, 5n * USDC);
    await expectCustomError(
      () => contribute(open, ctx.backer, 5n * USDC, { userAta: otherAta }),
      ANCHOR_ERROR.ConstraintTokenMint,
    );
  });

  it("blocks contributions, campaign creation and finalization while paused", async () => {
    await pause();
    await expectCustomError(
      () => contribute(open, ctx.backer, USDC),
      VERTA_CORE_ERROR__PLATFORM_PAUSED,
    );
    await expectCustomError(
      () => initializeCampaign("paused_init", goal, nowSeconds() + 3_600n),
      VERTA_CORE_ERROR__PLATFORM_PAUSED,
    );
    await expectCustomError(
      () => finalizeCampaign(open),
      VERTA_CORE_ERROR__PLATFORM_PAUSED,
    );
    await unpause();
  });

  it("finalizes only by the admin once the goal is met and creates the production ATA", async () => {
    await expectCustomError(
      () => finalizeCampaign(open, ctx.outsider),
      VERTA_CORE_ERROR__UNAUTHORIZED,
    );
    await expectCustomError(
      () => finalizeCampaign(open, ctx.operator),
      VERTA_CORE_ERROR__UNAUTHORIZED,
    );
    await expectCustomError(
      () => finalizeCampaign(open),
      VERTA_CORE_ERROR__GOAL_NOT_REACHED,
    );

    await contribute(open, ctx.secondBacker, 100n * USDC);
    const productionAta = await ata(open.mint, ctx.productionWallet.address);
    const before = await tokenBalance(productionAta, open.mint);

    await finalizeCampaign(open);

    expect(await statusOf(open)).to.equal(CrowdfundStatus.Succeeded);
    expect(await tokenBalance(open.escrow, open.mint)).to.equal(0n);
    expect((await tokenBalance(productionAta, open.mint)) - before).to.equal(
      goal,
    );
    const campaign = (await fetchCrowdfund(ctx.rpc, open.address)).data;
    expect(campaign.totalIssuedBps).to.equal(3_000);

    await expectCustomError(
      () => finalizeCampaign(open),
      VERTA_CORE_ERROR__CAMPAIGN_INACTIVE,
    );
    await expectCustomError(
      () => fail(open),
      VERTA_CORE_ERROR__CAMPAIGN_INACTIVE,
    );
    await expectCustomError(
      () => contribute(open, ctx.backer, USDC),
      VERTA_CORE_ERROR__CAMPAIGN_INACTIVE,
    );
  });

  it("fails an expired campaign and refunds with the rent returning to the sponsor", async () => {
    const expiring = await initializeCampaign(
      "lifecycle_expiring",
      goal,
      nowSeconds() + 2n,
    );
    await contribute(expiring, ctx.backer, 50n * USDC);
    await sleep(3_000);

    await expectCustomError(
      () => cancel(expiring),
      VERTA_CORE_ERROR__CAMPAIGN_DEADLINE_PASSED,
    );
    await expectCustomError(
      () => contribute(expiring, ctx.backer, USDC),
      VERTA_CORE_ERROR__CAMPAIGN_ENDED,
    );
    await expectCustomError(
      () => fail(expiring, ctx.outsider),
      VERTA_CORE_ERROR__UNAUTHORIZED,
    );
    await fail(expiring);
    expect(await statusOf(expiring)).to.equal(CrowdfundStatus.Refunded);

    const backerAta = await ata(expiring.mint, ctx.backer.address);
    const backerBefore = await tokenBalance(backerAta, expiring.mint);
    const sponsorBefore = await solBalance(ctx.sponsor.address);
    const [contributionAddress] = await findContributionPda({
      crowdfund: expiring.address,
      user: ctx.backer.address,
    });

    await expectCustomError(
      () => refund(expiring, ctx.backer, ctx.outsider.address),
      VERTA_CORE_ERROR__INVALID_RECIPIENT,
    );
    await expectCustomError(
      () => refund(expiring, ctx.outsider, ctx.sponsor.address),
      ANCHOR_ERROR.AccountNotInitialized,
    );

    await refund(expiring, ctx.backer, ctx.sponsor.address);

    expect(
      (await tokenBalance(backerAta, expiring.mint)) - backerBefore,
    ).to.equal(50n * USDC);
    expect(await tokenBalance(expiring.escrow, expiring.mint)).to.equal(0n);
    expect(
      (await fetchCrowdfund(ctx.rpc, expiring.address)).data.totalRaised,
    ).to.equal(0n);
    expect(
      (await fetchMaybeContribution(ctx.rpc, contributionAddress)).exists,
    ).to.equal(false);
    expect((await solBalance(ctx.sponsor.address)) > sponsorBefore).to.equal(
      true,
    );

    await expectCustomError(
      () => refund(expiring, ctx.backer, ctx.sponsor.address),
      ANCHOR_ERROR.AccountNotInitialized,
    );
  });

  it("cancels only before the deadline and blocks refunds while paused", async () => {
    const cancelled = await initializeCampaign(
      "lifecycle_cancel",
      goal,
      nowSeconds() + 3_600n,
    );
    await contribute(cancelled, ctx.secondBacker, 25n * USDC);

    await expectCustomError(
      () => cancel(cancelled, ctx.outsider),
      VERTA_CORE_ERROR__UNAUTHORIZED,
    );
    await expectCustomError(
      () => cancel(cancelled, ctx.operator),
      VERTA_CORE_ERROR__UNAUTHORIZED,
    );
    await cancel(cancelled);
    expect(await statusOf(cancelled)).to.equal(CrowdfundStatus.Cancelled);
    await expectCustomError(
      () => contribute(cancelled, ctx.secondBacker, USDC),
      VERTA_CORE_ERROR__CAMPAIGN_INACTIVE,
    );

    await pause();
    await expectCustomError(
      () => refund(cancelled, ctx.secondBacker, ctx.sponsor.address),
      VERTA_CORE_ERROR__PLATFORM_PAUSED,
    );
    await unpause();

    const backerAta = await ata(cancelled.mint, ctx.secondBacker.address);
    const before = await tokenBalance(backerAta, cancelled.mint);
    await refund(cancelled, ctx.secondBacker, ctx.sponsor.address);
    expect((await tokenBalance(backerAta, cancelled.mint)) - before).to.equal(
      25n * USDC,
    );
  });

  it("runs the full lifecycle on a Token-2022 settlement mint", async () => {
    await setSettlementMint(ctx.usdc2022Mint);
    await expectCustomError(
      () => initializeCampaign("spl_while_2022", goal, nowSeconds() + 3_600n),
      VERTA_CORE_ERROR__INVALID_SETTLEMENT_MINT,
    );

    const campaign = await initializeCampaign(
      "lifecycle_2022",
      100n * USDC,
      nowSeconds() + 3_600n,
      {
        mint: ctx.usdc2022Mint,
      },
    );
    await contribute(campaign, ctx.backer, 100n * USDC);
    expect(await tokenBalance(campaign.escrow, campaign.mint)).to.equal(
      100n * USDC,
    );

    const productionAta = await ata(
      campaign.mint,
      ctx.productionWallet.address,
    );
    await finalizeCampaign(campaign);
    expect(await tokenBalance(productionAta, campaign.mint)).to.equal(
      100n * USDC,
    );
    expect(await statusOf(campaign)).to.equal(CrowdfundStatus.Succeeded);

    await depositRevenue(campaign, 10n * USDC);
    expect(await tokenBalance(campaign.revenueVault, campaign.mint)).to.equal(
      3n * USDC,
    );
    expect(await tokenBalance(productionAta, campaign.mint)).to.equal(
      107n * USDC,
    );

    await setSettlementMint(ctx.usdcMint);
  });
});
