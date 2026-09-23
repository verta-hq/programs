import {
  address,
  appendTransactionMessageInstructions,
  assertIsTransactionWithBlockhashLifetime,
  createTransactionMessage,
  generateKeyPairSigner,
  getAddressEncoder,
  getProgramDerivedAddress,
  getSignatureFromTransaction,
  pipe,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  signTransactionMessageWithSigners,
  type Instruction,
  type KeyPairSigner,
} from "@solana/kit";
import {
  fetchMaybePlatformConfig,
  fetchPlatformConfig,
  findPlatformConfigPda,
  getCreateRevenueShareCollectionInstruction,
  getInitializePlatformInstruction,
  VERTA_CORE_PROGRAM_ADDRESS,
} from "../clients/js/verta_core/src/generated";
import {
  loadSolanaEnv,
  loadWalletSigner,
  requireAddress,
  type SolanaEnv,
} from "../scripts/solana-env";

const BPF_LOADER_UPGRADEABLE_ADDRESS = address(
  "BPFLoaderUpgradeab1e11111111111111111111111",
);
const MPL_CORE_PROGRAM_ADDRESS = address(
  "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d",
);
const ZERO_ADDRESS = "11111111111111111111111111111111";

const send = async (
  env: SolanaEnv,
  payer: KeyPairSigner,
  instructions: Instruction[],
): Promise<string> => {
  const { value: latestBlockhash } = await env.rpc.getLatestBlockhash().send();
  const message = pipe(
    createTransactionMessage({ version: 0 }),
    (m) => setTransactionMessageFeePayerSigner(payer, m),
    (m) => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, m),
    (m) => appendTransactionMessageInstructions(instructions, m),
  );
  const transaction = await signTransactionMessageWithSigners(message);
  assertIsTransactionWithBlockhashLifetime(transaction);
  await env.sendAndConfirm(transaction, { commitment: "confirmed" });
  return getSignatureFromTransaction(transaction);
};

const main = async (): Promise<void> => {
  const env = loadSolanaEnv();
  const deployer = await loadWalletSigner();
  const [platformConfig] = await findPlatformConfigPda();

  const existing = await fetchMaybePlatformConfig(env.rpc, platformConfig);
  if (existing.exists) {
    console.log(`PlatformConfig already initialized at ${platformConfig}`);
  } else {
    const admin = requireAddress("ADMIN_AUTHORITY_ADDRESS");
    const operator = requireAddress("PLATFORM_OPS_ADDRESS");
    const settlementMint = requireAddress("USDC_MINT");
    const [programData] = await getProgramDerivedAddress({
      programAddress: BPF_LOADER_UPGRADEABLE_ADDRESS,
      seeds: [getAddressEncoder().encode(VERTA_CORE_PROGRAM_ADDRESS)],
    });
    const signature = await send(env, deployer, [
      getInitializePlatformInstruction({
        platformConfig,
        payer: deployer,
        settlementMint,
        programData,
        admin,
        operator,
      }),
    ]);
    console.log(`initialize_platform ${signature}`);
  }

  let config = (await fetchPlatformConfig(env.rpc, platformConfig)).data;
  if (config.revenueShareCollection === ZERO_ADDRESS) {
    if (config.admin !== deployer.address) {
      console.log(
        `Revenue share collection not created: admin ${config.admin} must execute create_revenue_share_collection (build it with the backend admin-tx script and run it through Squads).`,
      );
    } else {
      const collection = await generateKeyPairSigner();
      const signature = await send(env, deployer, [
        getCreateRevenueShareCollectionInstruction({
          platformConfig,
          admin: deployer,
          collection,
          coreProgram: MPL_CORE_PROGRAM_ADDRESS,
        }),
      ]);
      console.log(
        `create_revenue_share_collection ${signature} (${collection.address})`,
      );
      config = (await fetchPlatformConfig(env.rpc, platformConfig)).data;
    }
  }

  console.log({
    program: VERTA_CORE_PROGRAM_ADDRESS,
    platformConfig,
    admin: config.admin,
    pendingAdmin: config.pendingAdmin,
    operator: config.operator,
    settlementMint: config.settlementMint,
    revenueShareCollection: config.revenueShareCollection,
    paused: config.paused,
  });
};

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
