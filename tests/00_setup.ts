import { expect } from "chai";
import * as fs from "fs";
import {
  AccountRole,
  address,
  airdropFactory,
  appendTransactionMessageInstructions,
  assertIsTransactionWithBlockhashLifetime,
  createKeyPairSignerFromBytes,
  createSolanaRpc,
  createSolanaRpcSubscriptions,
  createTransactionMessage,
  generateKeyPairSigner,
  getAddressEncoder,
  getProgramDerivedAddress,
  getSignatureFromTransaction,
  isSolanaError,
  lamports,
  SOLANA_ERROR__JSON_RPC__SERVER_ERROR_SEND_TRANSACTION_PREFLIGHT_FAILURE,
  pipe,
  sendAndConfirmTransactionFactory,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  signTransactionMessageWithSigners,
  SOLANA_ERROR__INSTRUCTION_ERROR__CUSTOM,
  type AccountSignerMeta,
  type Address,
  type Instruction,
  type KeyPairSigner,
  type Rpc,
  type RpcSubscriptions,
  type SolanaRpcApi,
  type SolanaRpcSubscriptionsApi,
  type TransactionSigner,
} from "@solana/kit";
import {
  getCreateAccountInstruction,
  SYSTEM_PROGRAM_ADDRESS,
} from "@solana-program/system";
import {
  ASSOCIATED_TOKEN_PROGRAM_ADDRESS,
  fetchMaybeToken,
  findAssociatedTokenPda,
  getCreateAssociatedTokenIdempotentInstruction,
  getInitializeMintInstruction,
  getMintSize,
  getMintToInstruction,
  TOKEN_PROGRAM_ADDRESS,
} from "@solana-program/token";
import {
  fetchMaybeToken as fetchMaybeToken2022,
  getInitializeMintInstruction as getInitializeMint2022Instruction,
  getMintSize as getMint2022Size,
  TOKEN_2022_PROGRAM_ADDRESS,
} from "@solana-program/token-2022";
import {
  CrowdfundStatus,
  fetchCrowdfund,
  findContributionPda,
  findCrowdfundPda,
  findEscrowVaultPda,
  findPlatformConfigPda,
  findPositionPda,
  findRevenueVaultPda,
  findTitleConfigPda,
  getContributeInstruction,
  getCreateTitleConfigInstruction,
  getDepositRevenueInstruction,
  getFinalizeCampaignInstruction,
  getInitializeCrowdfundInstruction,
  getMintRevenueShareInstruction,
  getRefundInstruction,
  VERTA_CORE_PROGRAM_ADDRESS,
} from "../clients/js/verta_core/src/generated";

export const MPL_CORE_PROGRAM_ADDRESS = address(
  "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d",
);
export const BPF_LOADER_UPGRADEABLE_ADDRESS = address(
  "BPFLoaderUpgradeab1e11111111111111111111111",
);

export const ANCHOR_ERROR = {
  ConstraintHasOne: 2001,
  ConstraintRaw: 2003,
  ConstraintOwner: 2004,
  ConstraintAddress: 2012,
  ConstraintTokenMint: 2014,
  ConstraintTokenOwner: 2015,
  AccountDiscriminatorMismatch: 3002,
  AccountNotInitialized: 3012,
} as const;

export const SCALE = 1_000_000_000_000n;
export const BPS = 10_000n;
export const USDC = 1_000_000n;

export type Ctx = {
  rpc: Rpc<SolanaRpcApi>;
  rpcSubscriptions: RpcSubscriptions<SolanaRpcSubscriptionsApi>;
  sendAndConfirm: ReturnType<typeof sendAndConfirmTransactionFactory>;
  deployer: KeyPairSigner;
  admin: KeyPairSigner;
  operator: KeyPairSigner;
  sponsor: KeyPairSigner;
  mintAuthority: KeyPairSigner;
  outsider: KeyPairSigner;
  backer: KeyPairSigner;
  secondBacker: KeyPairSigner;
  buyer: KeyPairSigner;
  productionWallet: KeyPairSigner;
  creator: KeyPairSigner;
  platformConfig: Address;
  programData: Address;
  usdcMint: Address;
  usdc2022Mint: Address;
  collection: Address;
};

export const ctx = {} as Ctx;

const wsUrl = (httpUrl: string): string =>
  httpUrl.replace(/^http/, "ws").replace(":8899", ":8900");

export const send = async (
  instructions: Instruction[],
  payer: TransactionSigner,
): Promise<string> => {
  const { value: latestBlockhash } = await ctx.rpc.getLatestBlockhash().send();
  const message = pipe(
    createTransactionMessage({ version: 0 }),
    (m) => setTransactionMessageFeePayerSigner(payer, m),
    (m) => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, m),
    (m) => appendTransactionMessageInstructions(instructions, m),
  );
  const transaction = await signTransactionMessageWithSigners(message);
  assertIsTransactionWithBlockhashLifetime(transaction);
  try {
    await ctx.sendAndConfirm(transaction, { commitment: "confirmed" });
  } catch (error) {
    if (
      process.env.TX_LOGS &&
      isSolanaError(
        error,
        SOLANA_ERROR__JSON_RPC__SERVER_ERROR_SEND_TRANSACTION_PREFLIGHT_FAILURE,
      )
    ) {
      console.error((error.context.logs ?? []).join("\n"));
    }
    throw error;
  }
  return getSignatureFromTransaction(transaction);
};

export const customErrorCode = (error: unknown): number | null => {
  let current: unknown = error;
  while (current !== undefined && current !== null) {
    if (isSolanaError(current, SOLANA_ERROR__INSTRUCTION_ERROR__CUSTOM)) {
      return Number(current.context.code);
    }
    current = (current as { cause?: unknown }).cause;
  }
  return null;
};

export const expectCustomError = async (
  action: () => Promise<unknown>,
  code: number,
): Promise<void> => {
  let failed = false;
  try {
    await action();
  } catch (error) {
    failed = true;
    const got = customErrorCode(error);
    if (got !== code) {
      throw new Error(
        `expected custom error ${code}, got ${got ?? String(error)}`,
      );
    }
  }
  if (!failed) expect.fail(`expected custom error ${code}`);
};

export const sleep = (ms: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, ms));

export const nowSeconds = (): bigint => BigInt(Math.floor(Date.now() / 1000));

export const solBalance = async (owner: Address): Promise<bigint> =>
  (await ctx.rpc.getBalance(owner, { commitment: "confirmed" }).send()).value;

export const tokenProgramFor = (mint: Address): Address =>
  mint === ctx.usdc2022Mint
    ? TOKEN_2022_PROGRAM_ADDRESS
    : TOKEN_PROGRAM_ADDRESS;

export const ata = async (mint: Address, owner: Address): Promise<Address> =>
  (
    await findAssociatedTokenPda({
      mint,
      owner,
      tokenProgram: tokenProgramFor(mint),
    })
  )[0];

export const tokenBalance = async (
  account: Address,
  mint: Address,
): Promise<bigint> => {
  const fetched =
    tokenProgramFor(mint) === TOKEN_2022_PROGRAM_ADDRESS
      ? await fetchMaybeToken2022(ctx.rpc, account)
      : await fetchMaybeToken(ctx.rpc, account);
  return fetched.exists ? fetched.data.amount : 0n;
};

export const fundTokens = async (
  mint: Address,
  owner: Address,
  amount: bigint,
): Promise<Address> => {
  const tokenProgram = tokenProgramFor(mint);
  const account = await ata(mint, owner);
  const instructions: Instruction[] = [
    getCreateAssociatedTokenIdempotentInstruction({
      payer: ctx.admin,
      ata: account,
      owner,
      mint,
      tokenProgram,
    }),
  ];
  if (amount > 0n) {
    instructions.push(
      getMintToInstruction(
        { mint, token: account, mintAuthority: ctx.mintAuthority, amount },
        { programAddress: tokenProgram },
      ),
    );
  }
  await send(instructions, ctx.admin);
  return account;
};

export const createMint = async (
  tokenProgram: Address,
  decimals = 6,
): Promise<Address> => {
  const mint = await generateKeyPairSigner();
  const is2022 = tokenProgram === TOKEN_2022_PROGRAM_ADDRESS;
  const space = is2022 ? getMint2022Size() : getMintSize();
  const rent = await ctx.rpc
    .getMinimumBalanceForRentExemption(BigInt(space))
    .send();
  const initialize = is2022
    ? getInitializeMint2022Instruction({
        mint: mint.address,
        decimals,
        mintAuthority: ctx.mintAuthority.address,
        freezeAuthority: null,
      })
    : getInitializeMintInstruction({
        mint: mint.address,
        decimals,
        mintAuthority: ctx.mintAuthority.address,
        freezeAuthority: null,
      });
  await send(
    [
      getCreateAccountInstruction({
        payer: ctx.admin,
        newAccount: mint,
        lamports: rent,
        space,
        programAddress: tokenProgram,
      }),
      initialize,
    ],
    ctx.admin,
  );
  return mint.address;
};

export type Campaign = {
  seriesId: string;
  address: Address;
  escrow: Address;
  revenueVault: Address;
  mint: Address;
};

export const campaignPdas = async (seriesId: string): Promise<Campaign> => {
  const [campaign] = await findCrowdfundPda({ seriesId });
  const [escrow] = await findEscrowVaultPda({ crowdfund: campaign });
  const [revenueVault] = await findRevenueVaultPda({ crowdfund: campaign });
  return {
    seriesId,
    address: campaign,
    escrow,
    revenueVault,
    mint: ctx.usdcMint,
  };
};

export const initializeCampaign = async (
  seriesId: string,
  goal: bigint,
  deadline: bigint,
  options: {
    mint?: Address;
    operator?: KeyPairSigner;
    productionWallet?: Address;
    backerRevenueBps?: number;
  } = {},
): Promise<Campaign> => {
  const campaign = await campaignPdas(seriesId);
  campaign.mint = options.mint ?? ctx.usdcMint;
  await send(
    [
      getInitializeCrowdfundInstruction({
        crowdfund: campaign.address,
        operator: options.operator ?? ctx.operator,
        mint: campaign.mint,
        escrowVault: campaign.escrow,
        revenueVault: campaign.revenueVault,
        platformConfig: ctx.platformConfig,
        tokenProgram: tokenProgramFor(campaign.mint),
        seriesId,
        goal,
        deadline,
        productionWallet:
          options.productionWallet ?? ctx.productionWallet.address,
        backerRevenueBps: options.backerRevenueBps ?? 3_000,
      }),
    ],
    options.operator ?? ctx.operator,
  );
  return campaign;
};

export const contribute = async (
  campaign: Campaign,
  user: KeyPairSigner,
  amount: bigint,
  options: { payer?: KeyPairSigner; userAta?: Address; fund?: boolean } = {},
): Promise<Address> => {
  const [contribution] = await findContributionPda({
    crowdfund: campaign.address,
    user: user.address,
  });
  const userAta =
    options.userAta ??
    (options.fund === false
      ? await ata(campaign.mint, user.address)
      : await fundTokens(campaign.mint, user.address, amount));
  const payer = options.payer ?? ctx.sponsor;
  await send(
    [
      getContributeInstruction({
        crowdfund: campaign.address,
        contribution,
        user,
        payer,
        mint: campaign.mint,
        userAta,
        escrowVault: campaign.escrow,
        platformConfig: ctx.platformConfig,
        tokenProgram: tokenProgramFor(campaign.mint),
        amount,
      }),
    ],
    payer,
  );
  return contribution;
};

export const finalizeCampaign = async (
  campaign: Campaign,
  admin: KeyPairSigner = ctx.admin,
): Promise<void> => {
  const productionWallet = (await fetchCrowdfund(ctx.rpc, campaign.address))
    .data.productionWallet;
  await send(
    [
      getFinalizeCampaignInstruction({
        crowdfund: campaign.address,
        admin,
        platformConfig: ctx.platformConfig,
        mint: campaign.mint,
        escrowVault: campaign.escrow,
        productionWallet,
        productionAta: await ata(campaign.mint, productionWallet),
        tokenProgram: tokenProgramFor(campaign.mint),
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ADDRESS,
      }),
    ],
    admin,
  );
};

export const refund = async (
  campaign: Campaign,
  user: KeyPairSigner,
  rentPayer: Address,
): Promise<void> => {
  const [contribution] = await findContributionPda({
    crowdfund: campaign.address,
    user: user.address,
  });
  await send(
    [
      getRefundInstruction({
        crowdfund: campaign.address,
        contribution,
        user,
        rentPayer,
        mint: campaign.mint,
        userAta: await ata(campaign.mint, user.address),
        escrowVault: campaign.escrow,
        platformConfig: ctx.platformConfig,
        tokenProgram: tokenProgramFor(campaign.mint),
      }),
    ],
    user,
  );
};

export const depositRevenue = async (
  campaign: Campaign,
  amount: bigint,
  operator: KeyPairSigner = ctx.operator,
): Promise<void> => {
  const productionWallet = (await fetchCrowdfund(ctx.rpc, campaign.address))
    .data.productionWallet;
  const operatorAta = await fundTokens(campaign.mint, operator.address, amount);
  await send(
    [
      getDepositRevenueInstruction({
        platformConfig: ctx.platformConfig,
        operator,
        crowdfund: campaign.address,
        mint: campaign.mint,
        operatorAta,
        revenueVault: campaign.revenueVault,
        productionWallet,
        productionAta: await ata(campaign.mint, productionWallet),
        tokenProgram: tokenProgramFor(campaign.mint),
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ADDRESS,
        amount,
      }),
    ],
    operator,
  );
};

export const createTitle = async (
  titleId: string,
  creatorWallet: Address,
  creatorBps = 8_000,
  platformBps = 2_000,
  operator: KeyPairSigner = ctx.operator,
): Promise<Address> => {
  const [titleConfig] = await findTitleConfigPda({ titleId });
  await send(
    [
      getCreateTitleConfigInstruction({
        platformConfig: ctx.platformConfig,
        titleConfig,
        operator,
        titleId,
        creatorWallet,
        creatorBps,
        platformBps,
        active: true,
      }),
    ],
    operator,
  );
  return titleConfig;
};

export const mintShare = async (
  campaign: Campaign,
  holder: KeyPairSigner,
  payer: KeyPairSigner = ctx.sponsor,
): Promise<{ asset: KeyPairSigner; position: Address }> => {
  const asset = await generateKeyPairSigner();
  const [position] = await findPositionPda({ asset: asset.address });
  const [contribution] = await findContributionPda({
    crowdfund: campaign.address,
    user: holder.address,
  });
  await send(
    [
      getMintRevenueShareInstruction({
        crowdfund: campaign.address,
        contribution,
        position,
        user: holder,
        payer,
        asset,
        collection: ctx.collection,
        platformConfig: ctx.platformConfig,
        coreProgram: MPL_CORE_PROGRAM_ADDRESS,
      }),
    ],
    payer,
  );
  return { asset, position };
};

const signerMeta = (
  role: AccountRole.WRITABLE_SIGNER | AccountRole.READONLY_SIGNER,
  signer: KeyPairSigner,
): AccountSignerMeta => ({
  address: signer.address,
  role,
  signer,
});

export const transferCoreAsset = async (
  asset: Address,
  from: KeyPairSigner,
  to: Address,
): Promise<void> => {
  await send(
    [
      {
        programAddress: MPL_CORE_PROGRAM_ADDRESS,
        accounts: [
          { address: asset, role: AccountRole.WRITABLE },
          { address: ctx.collection, role: AccountRole.READONLY },
          signerMeta(AccountRole.WRITABLE_SIGNER, from),
          signerMeta(AccountRole.READONLY_SIGNER, from),
          { address: to, role: AccountRole.READONLY },
          { address: SYSTEM_PROGRAM_ADDRESS, role: AccountRole.READONLY },
          { address: MPL_CORE_PROGRAM_ADDRESS, role: AccountRole.READONLY },
        ],
        data: new Uint8Array([14, 0]),
      },
    ],
    from,
  );
};

export const statusOf = async (campaign: Campaign): Promise<CrowdfundStatus> =>
  (await fetchCrowdfund(ctx.rpc, campaign.address)).data.status;

before(async () => {
  const rpcUrl = process.env.ANCHOR_PROVIDER_URL ?? "http://127.0.0.1:8899";
  const walletPath = process.env.ANCHOR_WALLET;
  if (!walletPath) throw new Error("ANCHOR_WALLET is required");
  ctx.rpc = createSolanaRpc(rpcUrl);
  ctx.rpcSubscriptions = createSolanaRpcSubscriptions(wsUrl(rpcUrl));
  ctx.sendAndConfirm = sendAndConfirmTransactionFactory({
    rpc: ctx.rpc,
    rpcSubscriptions: ctx.rpcSubscriptions,
  });
  ctx.deployer = await createKeyPairSignerFromBytes(
    new Uint8Array(JSON.parse(fs.readFileSync(walletPath, "utf8")) as number[]),
  );
  [
    ctx.admin,
    ctx.operator,
    ctx.sponsor,
    ctx.mintAuthority,
    ctx.outsider,
    ctx.backer,
    ctx.secondBacker,
    ctx.buyer,
    ctx.productionWallet,
    ctx.creator,
  ] = await Promise.all(
    Array.from({ length: 10 }, () => generateKeyPairSigner()),
  );

  const airdrop = airdropFactory({
    rpc: ctx.rpc,
    rpcSubscriptions: ctx.rpcSubscriptions,
  });
  await Promise.all(
    [
      ctx.admin,
      ctx.operator,
      ctx.sponsor,
      ctx.mintAuthority,
      ctx.outsider,
      ctx.backer,
      ctx.secondBacker,
      ctx.buyer,
      ctx.productionWallet,
      ctx.creator,
    ].map((signer) =>
      airdrop({
        recipientAddress: signer.address,
        lamports: lamports(20_000_000_000n),
        commitment: "confirmed",
      }),
    ),
  );

  [ctx.platformConfig] = await findPlatformConfigPda();
  [ctx.programData] = await getProgramDerivedAddress({
    programAddress: BPF_LOADER_UPGRADEABLE_ADDRESS,
    seeds: [getAddressEncoder().encode(VERTA_CORE_PROGRAM_ADDRESS)],
  });
  ctx.usdcMint = await createMint(TOKEN_PROGRAM_ADDRESS);
  ctx.usdc2022Mint = await createMint(TOKEN_2022_PROGRAM_ADDRESS);
});
