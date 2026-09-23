import { expect } from "chai";
import { generateKeyPairSigner } from "@solana/kit";
import {
  fetchPlatformConfig,
  getAcceptAdminInstruction,
  getCreateRevenueShareCollectionInstruction,
  getInitializePlatformInstruction,
  getPauseInstruction,
  getProposeAdminInstruction,
  getUpdatePlatformConfigInstruction,
  VERTA_CORE_ERROR__INVALID_SETTLEMENT_MINT,
  VERTA_CORE_ERROR__NO_PENDING_ADMIN,
  VERTA_CORE_ERROR__NOT_UPGRADE_AUTHORITY,
  VERTA_CORE_ERROR__UNAUTHORIZED,
} from "../clients/js/verta_core/src/generated";
import {
  campaignPdas,
  ctx,
  expectCustomError,
  initializeCampaign,
  MPL_CORE_PROGRAM_ADDRESS,
  nowSeconds,
  send,
  USDC,
} from "./00_setup";

describe("platform authority", () => {
  const config = async () =>
    (await fetchPlatformConfig(ctx.rpc, ctx.platformConfig)).data;

  it("rejects initialize_platform from a payer that is not the upgrade authority", async () => {
    await expectCustomError(
      () =>
        send(
          [
            getInitializePlatformInstruction({
              platformConfig: ctx.platformConfig,
              payer: ctx.outsider,
              settlementMint: ctx.usdcMint,
              programData: ctx.programData,
              admin: ctx.admin.address,
              operator: ctx.operator.address,
            }),
          ],
          ctx.outsider,
        ),
      VERTA_CORE_ERROR__NOT_UPGRADE_AUTHORITY,
    );
  });

  it("initializes the platform with admin, operator and settlement mint", async () => {
    await send(
      [
        getInitializePlatformInstruction({
          platformConfig: ctx.platformConfig,
          payer: ctx.deployer,
          settlementMint: ctx.usdcMint,
          programData: ctx.programData,
          admin: ctx.admin.address,
          operator: ctx.operator.address,
        }),
      ],
      ctx.deployer,
    );
    const platform = await config();
    expect(platform.admin).to.equal(ctx.admin.address);
    expect(platform.operator).to.equal(ctx.operator.address);
    expect(platform.settlementMint).to.equal(ctx.usdcMint);
    expect(platform.paused).to.equal(false);
  });

  it("only the admin can create the revenue share collection", async () => {
    const outsiderCollection = await generateKeyPairSigner();
    await expectCustomError(
      () =>
        send(
          [
            getCreateRevenueShareCollectionInstruction({
              platformConfig: ctx.platformConfig,
              admin: ctx.outsider,
              collection: outsiderCollection,
              coreProgram: MPL_CORE_PROGRAM_ADDRESS,
            }),
          ],
          ctx.outsider,
        ),
      VERTA_CORE_ERROR__UNAUTHORIZED,
    );
    const collection = await generateKeyPairSigner();
    await send(
      [
        getCreateRevenueShareCollectionInstruction({
          platformConfig: ctx.platformConfig,
          admin: ctx.admin,
          collection,
          coreProgram: MPL_CORE_PROGRAM_ADDRESS,
        }),
      ],
      ctx.admin,
    );
    ctx.collection = collection.address;
    expect((await config()).revenueShareCollection).to.equal(
      collection.address,
    );
  });

  it("hands admin over in two steps and rejects impostors at each step", async () => {
    const nextAdmin = await generateKeyPairSigner();
    await expectCustomError(
      () =>
        send(
          [
            getAcceptAdminInstruction({
              platformConfig: ctx.platformConfig,
              pendingAdmin: nextAdmin,
            }),
          ],
          ctx.sponsor,
        ),
      VERTA_CORE_ERROR__NO_PENDING_ADMIN,
    );
    await expectCustomError(
      () =>
        send(
          [
            getProposeAdminInstruction({
              platformConfig: ctx.platformConfig,
              admin: ctx.outsider,
              newAdmin: nextAdmin.address,
            }),
          ],
          ctx.outsider,
        ),
      VERTA_CORE_ERROR__UNAUTHORIZED,
    );
    await send(
      [
        getProposeAdminInstruction({
          platformConfig: ctx.platformConfig,
          admin: ctx.admin,
          newAdmin: nextAdmin.address,
        }),
      ],
      ctx.admin,
    );
    expect((await config()).pendingAdmin).to.equal(nextAdmin.address);
    expect((await config()).admin).to.equal(ctx.admin.address);
    await expectCustomError(
      () =>
        send(
          [
            getAcceptAdminInstruction({
              platformConfig: ctx.platformConfig,
              pendingAdmin: ctx.outsider,
            }),
          ],
          ctx.outsider,
        ),
      VERTA_CORE_ERROR__UNAUTHORIZED,
    );
    await send(
      [
        getAcceptAdminInstruction({
          platformConfig: ctx.platformConfig,
          pendingAdmin: nextAdmin,
        }),
      ],
      ctx.sponsor,
    );
    expect((await config()).admin).to.equal(nextAdmin.address);
    expect((await config()).pendingAdmin).to.equal(
      "11111111111111111111111111111111",
    );
    await send(
      [
        getProposeAdminInstruction({
          platformConfig: ctx.platformConfig,
          admin: nextAdmin,
          newAdmin: ctx.admin.address,
        }),
      ],
      ctx.sponsor,
    );
    await send(
      [
        getAcceptAdminInstruction({
          platformConfig: ctx.platformConfig,
          pendingAdmin: ctx.admin,
        }),
      ],
      ctx.admin,
    );
    expect((await config()).admin).to.equal(ctx.admin.address);
  });

  it("lets only the admin rotate the operator", async () => {
    const nextOperator = await generateKeyPairSigner();
    await expectCustomError(
      () =>
        send(
          [
            getUpdatePlatformConfigInstruction({
              platformConfig: ctx.platformConfig,
              admin: ctx.operator,
              newOperator: nextOperator.address,
              paused: null,
            }),
          ],
          ctx.operator,
        ),
      VERTA_CORE_ERROR__UNAUTHORIZED,
    );
    await send(
      [
        getUpdatePlatformConfigInstruction({
          platformConfig: ctx.platformConfig,
          admin: ctx.admin,
          newOperator: nextOperator.address,
          paused: null,
        }),
      ],
      ctx.admin,
    );
    expect((await config()).operator).to.equal(nextOperator.address);
    await send(
      [
        getUpdatePlatformConfigInstruction({
          platformConfig: ctx.platformConfig,
          admin: ctx.admin,
          newOperator: ctx.operator.address,
          paused: null,
        }),
      ],
      ctx.admin,
    );
    expect((await config()).operator).to.equal(ctx.operator.address);
  });

  it("lets the operator pause but only the admin unpause", async () => {
    await expectCustomError(
      () =>
        send(
          [
            getPauseInstruction({
              platformConfig: ctx.platformConfig,
              authority: ctx.outsider,
            }),
          ],
          ctx.outsider,
        ),
      VERTA_CORE_ERROR__UNAUTHORIZED,
    );
    await send(
      [
        getPauseInstruction({
          platformConfig: ctx.platformConfig,
          authority: ctx.operator,
        }),
      ],
      ctx.operator,
    );
    expect((await config()).paused).to.equal(true);
    await expectCustomError(
      () =>
        send(
          [
            getUpdatePlatformConfigInstruction({
              platformConfig: ctx.platformConfig,
              admin: ctx.operator,
              newOperator: null,
              paused: false,
            }),
          ],
          ctx.operator,
        ),
      VERTA_CORE_ERROR__UNAUTHORIZED,
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
    expect((await config()).paused).to.equal(false);
    await send(
      [
        getPauseInstruction({
          platformConfig: ctx.platformConfig,
          authority: ctx.admin,
        }),
      ],
      ctx.admin,
    );
    expect((await config()).paused).to.equal(true);
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
    expect((await config()).paused).to.equal(false);
  });

  it("rejects campaigns on any mint other than the settlement mint", async () => {
    await expectCustomError(
      () =>
        initializeCampaign(
          "wrong_mint_gate",
          10n * USDC,
          nowSeconds() + 3_600n,
          {
            mint: ctx.usdc2022Mint,
          },
        ),
      VERTA_CORE_ERROR__INVALID_SETTLEMENT_MINT,
    );
    const pdas = await campaignPdas("wrong_mint_gate");
    const account = await ctx.rpc
      .getAccountInfo(pdas.address, { encoding: "base64" })
      .send();
    expect(account.value).to.equal(null);
  });
});
