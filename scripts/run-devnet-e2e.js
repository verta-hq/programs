// Launcher for the devnet E2E (ts-node CLI is dead on Node >= 23; register programmatically).
// Points the Anchor provider at devnet using the workspace wallet.
const path = require("path");

process.env.ANCHOR_PROVIDER_URL =
  process.env.ANCHOR_PROVIDER_URL || "https://api.devnet.solana.com";
process.env.ANCHOR_WALLET =
  process.env.ANCHOR_WALLET ||
  path.join(process.env.HOME, ".config", "solana", "jbias.json");

require("ts-node").register({
  project: path.resolve(__dirname, "..", "tsconfig.json"),
  transpileOnly: true,
});

require(path.resolve(__dirname, "devnet-e2e.ts"));
