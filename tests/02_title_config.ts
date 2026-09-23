import { expect } from "chai";
import {
  fetchTitleConfig,
  findTitleConfigPda,
  getUpdateTitleConfigInstruction,
  VERTA_CORE_ERROR__INVALID_BASIS_POINTS,
  VERTA_CORE_ERROR__INVALID_RECIPIENT,
  VERTA_CORE_ERROR__UNAUTHORIZED,
} from "../clients/js/verta_core/src/generated";
import { createTitle, ctx, expectCustomError, send } from "./00_setup";

describe("title config", () => {
  const titleId = "bangkok_nights_s1";

  it("lets the operator register a title with a valid split", async () => {
    const titleConfig = await createTitle(titleId, ctx.creator.address);
    const title = (await fetchTitleConfig(ctx.rpc, titleConfig)).data;
    expect(title.titleId).to.equal(titleId);
    expect(title.creatorWallet).to.equal(ctx.creator.address);
    expect(title.creatorBps).to.equal(8_000);
    expect(title.platformBps).to.equal(2_000);
    expect(title.active).to.equal(true);
  });

  it("rejects registration by the admin, an outsider, or with a broken split", async () => {
    await expectCustomError(
      () =>
        createTitle(
          "admin_title",
          ctx.creator.address,
          8_000,
          2_000,
          ctx.admin,
        ),
      VERTA_CORE_ERROR__UNAUTHORIZED,
    );
    await expectCustomError(
      () =>
        createTitle(
          "outsider_title",
          ctx.creator.address,
          8_000,
          2_000,
          ctx.outsider,
        ),
      VERTA_CORE_ERROR__UNAUTHORIZED,
    );
    await expectCustomError(
      () => createTitle("invalid_bps_title", ctx.creator.address, 5_000, 4_000),
      VERTA_CORE_ERROR__INVALID_BASIS_POINTS,
    );
    await expectCustomError(
      () =>
        createTitle(
          "no_wallet_title",
          "11111111111111111111111111111111" as never,
        ),
      VERTA_CORE_ERROR__INVALID_RECIPIENT,
    );
  });

  it("lets only the operator update splits, wallet and active flag", async () => {
    const [titleConfig] = await findTitleConfigPda({ titleId });
    await expectCustomError(
      () =>
        send(
          [
            getUpdateTitleConfigInstruction({
              platformConfig: ctx.platformConfig,
              titleConfig,
              operator: ctx.outsider,
              newCreatorWallet: null,
              newCreatorBps: null,
              newPlatformBps: null,
              newActive: false,
            }),
          ],
          ctx.outsider,
        ),
      VERTA_CORE_ERROR__UNAUTHORIZED,
    );
    await expectCustomError(
      () =>
        send(
          [
            getUpdateTitleConfigInstruction({
              platformConfig: ctx.platformConfig,
              titleConfig,
              operator: ctx.operator,
              newCreatorWallet: null,
              newCreatorBps: 7_000,
              newPlatformBps: 2_000,
              newActive: null,
            }),
          ],
          ctx.operator,
        ),
      VERTA_CORE_ERROR__INVALID_BASIS_POINTS,
    );
    await send(
      [
        getUpdateTitleConfigInstruction({
          platformConfig: ctx.platformConfig,
          titleConfig,
          operator: ctx.operator,
          newCreatorWallet: ctx.productionWallet.address,
          newCreatorBps: 7_000,
          newPlatformBps: 3_000,
          newActive: false,
        }),
      ],
      ctx.operator,
    );
    const title = (await fetchTitleConfig(ctx.rpc, titleConfig)).data;
    expect(title.creatorWallet).to.equal(ctx.productionWallet.address);
    expect(title.creatorBps).to.equal(7_000);
    expect(title.platformBps).to.equal(3_000);
    expect(title.active).to.equal(false);
  });
});
