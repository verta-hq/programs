const fs = require("fs");
const path = require("path");
const {
  createFromRoot,
  addPdasVisitor,
  constantPdaSeedNodeFromString,
  variablePdaSeedNode,
  publicKeyTypeNode,
  stringTypeNode,
  numberTypeNode,
} = require("codama");
const { rootNodeFromAnchor } = require("@codama/nodes-from-anchor");
const { renderVisitor } = require("@codama/renderers-js");

const root = path.resolve(__dirname, "..");
const check = process.argv.includes("--check");
const buildIdlPath = path.join(root, "target", "idl", "verta_core.json");
const canonicalIdlPath = path.join(root, "idl", "verta_core.json");
const localPackageDir = path.join(root, "clients", "js", "verta_core");
const localClientDir = path.join(localPackageDir, "src", "generated");
const backendClientDir = path.resolve(
  root,
  "..",
  "backend",
  "src",
  "solana",
  "generated",
  "verta_core",
);

const seed = (value) => constantPdaSeedNodeFromString("utf8", value);
const pubkeySeed = (name) => variablePdaSeedNode(name, publicKeyTypeNode());
const stringSeed = (name) => variablePdaSeedNode(name, stringTypeNode("utf8"));

const pdas = [
  { name: "platformConfig", seeds: [seed("platform_authority")] },
  { name: "titleConfig", seeds: [seed("title_config"), stringSeed("titleId")] },
  { name: "crowdfund", seeds: [seed("crowdfund"), stringSeed("seriesId")] },
  {
    name: "contribution",
    seeds: [seed("contribution"), pubkeySeed("crowdfund"), pubkeySeed("user")],
  },
  {
    name: "escrowVault",
    seeds: [seed("escrow_vault"), pubkeySeed("crowdfund")],
  },
  {
    name: "revenueVault",
    seeds: [seed("revenue_vault"), pubkeySeed("crowdfund")],
  },
  {
    name: "creditEpoch",
    seeds: [
      seed("credit_epoch"),
      variablePdaSeedNode("epochDay", numberTypeNode("u32")),
    ],
  },
];

const listFiles = (dir, prefix = "") => {
  if (!fs.existsSync(dir)) return [];
  return fs
    .readdirSync(dir, { withFileTypes: true })
    .flatMap((entry) => {
      const rel = path.join(prefix, entry.name);
      return entry.isDirectory()
        ? listFiles(path.join(dir, entry.name), rel)
        : [rel];
    })
    .sort();
};

const diffDirs = (left, right) => {
  const leftFiles = listFiles(left);
  const rightFiles = listFiles(right);
  const drift = [];
  const union = new Set([...leftFiles, ...rightFiles]);
  for (const file of union) {
    const l = path.join(left, file);
    const r = path.join(right, file);
    if (!fs.existsSync(l) || !fs.existsSync(r)) {
      drift.push(file);
      continue;
    }
    if (fs.readFileSync(l, "utf8") !== fs.readFileSync(r, "utf8")) {
      drift.push(file);
    }
  }
  return drift;
};

const main = async () => {
  const idl = JSON.parse(fs.readFileSync(buildIdlPath, "utf8"));
  const canonicalIdl = `${JSON.stringify(idl, null, 2)}\n`;

  const codama = createFromRoot(rootNodeFromAnchor(idl));
  const programName = codama.getRoot().program.name;
  const existingPdas = new Set(
    codama.getRoot().program.pdas.map((node) => node.name),
  );
  codama.update(
    addPdasVisitor({
      [programName]: pdas.filter((pda) => !existingPdas.has(pda.name)),
    }),
  );
  await codama.accept(
    renderVisitor(localPackageDir, {
      formatCode: true,
      deleteFolderBeforeRendering: true,
    }),
  );

  if (check) {
    const failures = [];
    const committedIdl = fs.existsSync(canonicalIdlPath)
      ? fs.readFileSync(canonicalIdlPath, "utf8")
      : "";
    if (committedIdl !== canonicalIdl) {
      failures.push(
        `idl/verta_core.json drifted from target/idl/verta_core.json`,
      );
    }
    const drift = diffDirs(localClientDir, backendClientDir);
    if (drift.length > 0) {
      failures.push(
        `backend generated client drifted:\n  ${drift.join("\n  ")}`,
      );
    }
    if (failures.length > 0) {
      console.error(failures.join("\n"));
      console.error("Run `yarn generate:clients` and commit the results.");
      process.exit(1);
    }
    console.log("Generated clients and canonical IDL are in sync.");
  } else {
    fs.mkdirSync(path.dirname(canonicalIdlPath), { recursive: true });
    fs.writeFileSync(canonicalIdlPath, canonicalIdl);
    fs.rmSync(backendClientDir, { recursive: true, force: true });
    fs.mkdirSync(path.dirname(backendClientDir), { recursive: true });
    fs.cpSync(localClientDir, backendClientDir, { recursive: true });
    console.log(
      `Rendered ${programName} client to ${path.relative(root, localClientDir)} and ${path.relative(root, backendClientDir)}`,
    );
  }
};

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
