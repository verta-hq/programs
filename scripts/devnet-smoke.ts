import { fetchEncodedAccount } from "@solana/kit";
import {
  fetchPlatformConfig,
  findPlatformConfigPda,
  VERTA_CORE_PROGRAM_ADDRESS,
} from "../clients/js/verta_core/src/generated";
import { loadSolanaEnv } from "./solana-env";

const main = async (): Promise<void> => {
  const env = loadSolanaEnv();
  const program = await fetchEncodedAccount(
    env.rpc,
    VERTA_CORE_PROGRAM_ADDRESS,
  );
  if (!program.exists || !program.executable) {
    throw new Error(
      `${VERTA_CORE_PROGRAM_ADDRESS} is not an executable program on ${env.rpcUrl}`,
    );
  }
  const [platformConfig] = await findPlatformConfigPda();
  const config = (await fetchPlatformConfig(env.rpc, platformConfig)).data;
  const zero = "11111111111111111111111111111111";
  if (
    config.admin === zero ||
    config.operator === zero ||
    config.settlementMint === zero
  ) {
    throw new Error("PlatformConfig has an unset authority");
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
