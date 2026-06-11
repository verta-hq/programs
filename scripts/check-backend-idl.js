const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");
const generatedIdl = path.join(root, "target", "idl", "verta_core.json");
const backendIdl = path.resolve(
  root,
  "..",
  "backend",
  "src",
  "solana",
  "verta_core.json"
);

function readJson(filePath) {
  try {
    return JSON.parse(fs.readFileSync(filePath, "utf8"));
  } catch (error) {
    console.error(`Unable to read ${filePath}`);
    throw error;
  }
}

function stableJson(value) {
  return JSON.stringify(value, null, 2);
}

const generated = stableJson(readJson(generatedIdl));
const backend = stableJson(readJson(backendIdl));

if (generated !== backend) {
  console.error("backend/src/solana/verta_core.json is out of sync.");
  console.error(
    "Run `anchor build`, then copy target/idl/verta_core.json into the backend."
  );
  process.exit(1);
}

console.log(
  "backend/src/solana/verta_core.json matches target/idl/verta_core.json"
);
