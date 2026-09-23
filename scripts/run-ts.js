const path = require("path");

const entry = process.argv[2];
if (!entry) {
  console.error("usage: node scripts/run-ts.js <file.ts>");
  process.exit(1);
}

require("ts-node").register({
  project: path.resolve(__dirname, "..", "tsconfig.json"),
  transpileOnly: true,
});

require(path.resolve(process.cwd(), entry));
