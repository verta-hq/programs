import { expect } from "chai";
import type { KeyPairSigner } from "@solana/kit";
import {
  fetchCreditEpoch,
  findCreditEpochPda,
  getRecordCreditEpochInstruction,
  VERTA_CORE_ERROR__UNAUTHORIZED,
} from "../clients/js/verta_core/src/generated";
import { ctx, expectCustomError, send } from "./00_setup";

const epochDay = 20_700;

const record = async (
  day: number,
  values: { sold: bigint; spent: bigint; refunded: bigint; liability: bigint },
  operator: KeyPairSigner = ctx.operator,
): Promise<void> => {
  const [creditEpoch] = await findCreditEpochPda({ epochDay: day });
  await send(
    [
      getRecordCreditEpochInstruction({
        platformConfig: ctx.platformConfig,
        operator,
        creditEpoch,
        epochDay: day,
        coinsSold: values.sold,
        coinsSpent: values.spent,
        coinsRefunded: values.refunded,
        usdLiabilityMicro: values.liability,
        balancesRoot: new Uint8Array(32),
      }),
    ],
    operator,
  );
};

describe("credit epoch checkpoint", () => {
  it("records a checkpoint from the operator", async () => {
    await record(epochDay, {
      sold: 1_100n,
      spent: 150n,
      refunded: 0n,
      liability: 9_500_000n,
    });
    const [creditEpoch] = await findCreditEpochPda({ epochDay });
    const epoch = (await fetchCreditEpoch(ctx.rpc, creditEpoch)).data;
    expect(epoch.epochDay).to.equal(epochDay);
    expect(epoch.coinsSold).to.equal(1_100n);
    expect(epoch.coinsSpent).to.equal(150n);
    expect(epoch.usdLiabilityMicro).to.equal(9_500_000n);
  });

  it("re-recording the same day overwrites idempotently", async () => {
    await record(epochDay, {
      sold: 1_600n,
      spent: 200n,
      refunded: 500n,
      liability: 9_000_000n,
    });
    const [creditEpoch] = await findCreditEpochPda({ epochDay });
    const epoch = (await fetchCreditEpoch(ctx.rpc, creditEpoch)).data;
    expect(epoch.coinsSold).to.equal(1_600n);
    expect(epoch.coinsRefunded).to.equal(500n);
    expect(epoch.usdLiabilityMicro).to.equal(9_000_000n);
  });

  it("rejects checkpoint writers other than the operator", async () => {
    const values = { sold: 1n, spent: 0n, refunded: 0n, liability: 10_000n };
    await expectCustomError(
      () => record(epochDay + 1, values, ctx.outsider),
      VERTA_CORE_ERROR__UNAUTHORIZED,
    );
    await expectCustomError(
      () => record(epochDay + 1, values, ctx.admin),
      VERTA_CORE_ERROR__UNAUTHORIZED,
    );
  });
});
