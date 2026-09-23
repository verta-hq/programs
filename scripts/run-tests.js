// Programmatic mocha runner. The mocha/ts-mocha CLIs crash on Node >= 23
// (require() of yargs' extensionless ESM shim); the programmatic API never
// loads yargs.
const fs = require("fs");
const path = require("path");
const Mocha = require("mocha");

require("ts-node").register({
  project: path.resolve(__dirname, "..", "tsconfig.json"),
  transpileOnly: true,
});

const testsDir = path.resolve(__dirname, "..", "tests");
const files = fs
  .readdirSync(testsDir, { recursive: true })
  .filter((name) => String(name).endsWith(".ts"))
  .map((name) => path.join(testsDir, String(name)))
  .sort();

if (files.length === 0) {
  console.error(`No .ts test files found in ${testsDir}`);
  process.exit(1);
}

const mocha = new Mocha({ timeout: 1_000_000 });
files.forEach((file) => mocha.addFile(file));
mocha.run((failures) => process.exit(failures ? 1 : 0));
