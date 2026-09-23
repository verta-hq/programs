import * as fs from "fs";
import {
  address,
  createKeyPairSignerFromBytes,
  createSolanaRpc,
  createSolanaRpcSubscriptions,
  sendAndConfirmTransactionFactory,
  type Address,
  type KeyPairSigner,
  type Rpc,
  type RpcSubscriptions,
  type SolanaRpcApi,
  type SolanaRpcSubscriptionsApi,
} from "@solana/kit";

export type SolanaEnv = {
  rpcUrl: string;
  rpc: Rpc<SolanaRpcApi>;
  rpcSubscriptions: RpcSubscriptions<SolanaRpcSubscriptionsApi>;
  sendAndConfirm: ReturnType<typeof sendAndConfirmTransactionFactory>;
};

const requireEnv = (name: string): string => {
  const value = process.env[name];
  if (!value) throw new Error(`${name} is required`);
  return value;
};

export const requireAddress = (name: string): Address =>
  address(requireEnv(name));

export const loadSolanaEnv = (): SolanaEnv => {
  const rpcUrl = process.env.SOLANA_RPC_URL ?? process.env.ANCHOR_PROVIDER_URL;
  if (!rpcUrl)
    throw new Error("SOLANA_RPC_URL (or ANCHOR_PROVIDER_URL) is required");
  const wsUrl =
    process.env.SOLANA_WS_URL ??
    rpcUrl.replace(/^http/, "ws").replace(":8899", ":8900");
  const rpc = createSolanaRpc(rpcUrl);
  const rpcSubscriptions = createSolanaRpcSubscriptions(wsUrl);
  return {
    rpcUrl,
    rpc,
    rpcSubscriptions,
    sendAndConfirm: sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions }),
  };
};

export const loadWalletSigner = async (): Promise<KeyPairSigner> => {
  const walletPath = process.env.ANCHOR_WALLET ?? requireEnv("SOLANA_WALLET");
  const bytes = JSON.parse(fs.readFileSync(walletPath, "utf8")) as number[];
  return createKeyPairSignerFromBytes(new Uint8Array(bytes));
};
